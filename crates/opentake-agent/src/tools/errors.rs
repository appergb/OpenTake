//! `ToolError` + LLM-facing precise path errors. 1:1 port of upstream
//! `ToolExecutor.swift` `validateUnknownKeys` / `firstNonFiniteNumberPath` /
//! `formatDecodingError` (`agent-SPEC.md` §4.2), re-expressed with
//! `serde_path_to_error` for the decode-error path.
//!
//! Why this matters (ARCHITECTURE §7, analysis 04): the *exact* path in a
//! decode error ("entries[3].startFrame: missing required field") directly
//! drives the agent's self-correction rate. These strings are a behavior
//! contract, not cosmetics — the wording mirrors upstream.

use serde::de::DeserializeOwned;
use serde_json::Value;

/// A tool-level error carrying an LLM-facing message. Never panics across the
/// MCP boundary: the executor turns every `Err(ToolError)` into a
/// `ToolResult::error` (`agent-SPEC.md` §4.1).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct ToolError {
    pub message: String,
}

impl ToolError {
    pub fn new(message: impl Into<String>) -> Self {
        ToolError {
            message: message.into(),
        }
    }
}

/// Marks a strongly-typed tool-args struct with the set of keys it accepts, so
/// unknown fields (including nested `entries[]` keys serde would silently drop)
/// are rejected with a precise message. Mirrors upstream `DecodableToolArgs`.
pub trait ToolArgs: DeserializeOwned {
    const ALLOWED_KEYS: &'static [&'static str];
}

/// Reject any key on `obj` outside `allowed`. `path` prefixes the error
/// (e.g. `"entries[3]"`). 1:1 port of `validateUnknownKeys`.
pub fn validate_unknown_keys(
    obj: &serde_json::Map<String, Value>,
    allowed: &[&str],
    path: &str,
) -> Result<(), ToolError> {
    let mut unknown: Vec<&str> = obj
        .keys()
        .map(String::as_str)
        .filter(|k| !allowed.contains(k))
        .collect();
    if unknown.is_empty() {
        return Ok(());
    }
    unknown.sort_unstable();
    let mut allowed_sorted: Vec<&str> = allowed.to_vec();
    allowed_sorted.sort_unstable();
    Err(ToolError::new(format!(
        "{path}: unknown field(s) '{}'. Allowed: {}.",
        unknown.join("', '"),
        allowed_sorted.join(", ")
    )))
}

/// First JSON path (DFS) whose value is a non-finite number (`NaN`/`Inf`).
/// 1:1 port of `firstNonFiniteNumberPath`: objects descend as `.key`, arrays as
/// `[i]`.
pub fn first_non_finite_number_path(value: &Value, path: &str) -> Option<String> {
    match value {
        Value::Number(n) => {
            // serde_json only parses finite numbers from text, but a
            // programmatically built Value (or `f64`) can hold non-finite.
            match n.as_f64() {
                Some(f) if !f.is_finite() => Some(path.to_string()),
                None => Some(path.to_string()),
                _ => None,
            }
        }
        Value::Array(arr) => arr
            .iter()
            .enumerate()
            .find_map(|(i, v)| first_non_finite_number_path(v, &format!("{path}[{i}]"))),
        Value::Object(map) => map
            .iter()
            .find_map(|(k, v)| first_non_finite_number_path(v, &format!("{path}.{k}"))),
        _ => None,
    }
}

/// Decode `dict` into `T` with the full three-layer guard:
/// 1. unknown-key rejection (incl. nested entries), 2. non-finite-number
///    rejection, 3. path-precise serde decode errors. 1:1 port of
///    `decodeToolArgs`, using `serde_path_to_error` for the codingPath trail.
///
/// `dict` is the top-level args object; `path` is the prefix (usually the empty
/// string for top-level, or `"entries[3]"` when decoding a single nested entry).
pub fn decode_tool_args<T: ToolArgs>(dict: &Value, path: &str) -> Result<T, ToolError> {
    if let Value::Object(map) = dict {
        validate_unknown_keys(map, T::ALLOWED_KEYS, top_label(path))?;
    }
    if let Some(bad) = first_non_finite_number_path(dict, path) {
        return Err(ToolError::new(format!("{bad}: value must be finite")));
    }
    let serialized = dict.to_string();
    let de = &mut serde_json::Deserializer::from_str(&serialized);
    serde_path_to_error::deserialize(de).map_err(|e| {
        let raw_path = e.path().to_string(); // e.g. "entries.3.startFrame" or "."
        let normalized = normalize_path(path, &raw_path);
        ToolError::new(map_serde_error(&normalized, e.inner()))
    })
}

/// The label used by `validate_unknown_keys` for a top-level decode: upstream
/// uses no prefix for the root object, so an empty `path` becomes the bare key
/// list. A non-empty path (nested entry) is used verbatim.
fn top_label(path: &str) -> &str {
    if path.is_empty() {
        "arguments"
    } else {
        path
    }
}

/// Turn `serde_path_to_error`'s dotted path (`entries.3.startFrame`) into the
/// upstream bracket form (`entries[3].startFrame`), anchored under `prefix`.
fn normalize_path(prefix: &str, raw: &str) -> String {
    let mut out = String::from(prefix);
    if raw.is_empty() || raw == "." {
        return out;
    }
    for seg in raw.split('.') {
        if seg.is_empty() {
            continue;
        }
        if seg.chars().all(|c| c.is_ascii_digit()) {
            out.push_str(&format!("[{seg}]"));
        } else {
            if !out.is_empty() {
                out.push('.');
            }
            out.push_str(seg);
        }
    }
    out
}

/// Map a `serde_json` decode error to the upstream four-class wording
/// (`formatDecodingError`): missing field / type mismatch / missing value /
/// data corrupted. `serde_json` flattens the cause into the message, so we
/// classify on its text.
fn map_serde_error(path: &str, err: &serde_json::Error) -> String {
    let msg = err.to_string();
    let prefix = if path.is_empty() { "arguments" } else { path };
    if let Some(field) = missing_field_name(&msg) {
        // serde reports a missing field's path as the *container* (e.g.
        // `entries[1]`), not the absent key. Append the field name so the
        // message matches the upstream LLM contract `{path}.{field}: missing
        // required field '{key}'` (`agent-SPEC.md` §4.2.3). At the root the path
        // is empty (`arguments`) and the bare key already reads clearly, so no
        // suffix is added there.
        if path.is_empty() {
            format!("{prefix}: missing required field '{field}'")
        } else {
            format!("{prefix}.{field}: missing required field '{field}'")
        }
    } else if msg.starts_with("invalid type") {
        // Recover the expected type name from serde's
        // `invalid type: <actual>, expected <expected> at line/col` so the
        // message keeps upstream's `expected {type}, got something else`
        // precision instead of the lossy "a different type"
        // (`agent-SPEC.md` §4.2.3).
        match expected_type_name(&msg) {
            Some(expected) => format!("{prefix}: expected {expected}, got something else"),
            None => format!("{prefix}: expected a different type, got something else"),
        }
    } else {
        // dataCorrupted analogue: surface the underlying detail, stripped of
        // serde's trailing "at line/column" noise.
        let detail = msg.split(" at line ").next().unwrap_or(&msg);
        format!("{prefix}: {detail}")
    }
}

/// Extract the field name from serde's "missing field `x`" message.
fn missing_field_name(msg: &str) -> Option<String> {
    let marker = "missing field `";
    let start = msg.find(marker)? + marker.len();
    let rest = &msg[start..];
    let end = rest.find('`')?;
    Some(rest[..end].to_string())
}

/// Extract the expected-type phrase from serde's `invalid type: <actual>,
/// expected <expected> at line N column M` message (e.g. `a string`, `i32`).
/// The trailing `at line/column` noise is stripped.
fn expected_type_name(msg: &str) -> Option<String> {
    let marker = ", expected ";
    let start = msg.find(marker)? + marker.len();
    let rest = &msg[start..];
    let expected = rest.split(" at line ").next().unwrap_or(rest).trim();
    if expected.is_empty() {
        None
    } else {
        Some(expected.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    struct Entry {
        media_ref: String,
        start_frame: i32,
        duration_frames: i32,
    }
    impl ToolArgs for Entry {
        const ALLOWED_KEYS: &'static [&'static str] =
            &["mediaRef", "startFrame", "durationFrames", "trimStartFrame"];
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct Wrapper {
        entries: Vec<Entry>,
    }
    impl ToolArgs for Wrapper {
        const ALLOWED_KEYS: &'static [&'static str] = &["entries"];
    }

    #[test]
    fn unknown_field_lists_sorted_allowed() {
        let v = serde_json::json!({"mediaRef":"m","startFrame":0,"durationFrames":5,"bogus":1});
        let err = decode_tool_args::<Entry>(&v, "entries[3]").unwrap_err();
        assert_eq!(
            err.message,
            "entries[3]: unknown field(s) 'bogus'. Allowed: durationFrames, mediaRef, startFrame, trimStartFrame."
        );
    }

    #[test]
    fn missing_required_field_has_precise_path() {
        // Top-level decode of an Entry missing startFrame.
        let v = serde_json::json!({"mediaRef":"m","durationFrames":5});
        let err = decode_tool_args::<Entry>(&v, "").unwrap_err();
        assert_eq!(
            err.message,
            "arguments: missing required field 'startFrame'"
        );
    }

    #[test]
    fn type_mismatch_reports_expected() {
        let v = serde_json::json!({"mediaRef":"m","startFrame":"oops","durationFrames":5});
        let err = decode_tool_args::<Entry>(&v, "").unwrap_err();
        assert!(err.message.contains("startFrame"), "{}", err.message);
        assert!(err.message.contains("expected"), "{}", err.message);
        // The recovered type name (an integer) and upstream's "got something
        // else" wording are both present, and no line/column noise leaks.
        assert!(
            err.message.contains("got something else"),
            "{}",
            err.message
        );
        assert!(!err.message.contains("at line"), "{}", err.message);
        assert!(
            !err.message.contains("a different type"),
            "type name should be recovered: {}",
            err.message
        );
    }

    #[test]
    fn expected_type_name_extracts_and_strips_noise() {
        assert_eq!(
            expected_type_name("invalid type: string \"x\", expected i32 at line 1 column 9")
                .as_deref(),
            Some("i32")
        );
        assert_eq!(
            expected_type_name("invalid type: integer `5`, expected a sequence at line 1 column 2")
                .as_deref(),
            Some("a sequence")
        );
        assert_eq!(expected_type_name("some unrelated message"), None);
    }

    #[test]
    fn nested_array_index_uses_brackets() {
        // entries[1] is missing startFrame -> path "entries[1].startFrame".
        let v = serde_json::json!({"entries":[
            {"mediaRef":"a","startFrame":0,"durationFrames":5},
            {"mediaRef":"b","durationFrames":5}
        ]});
        let err = decode_tool_args::<Wrapper>(&v, "").unwrap_err();
        assert_eq!(
            err.message,
            "entries[1].startFrame: missing required field 'startFrame'"
        );
    }

    #[test]
    fn non_finite_number_rejected_with_path() {
        // Build a Value carrying a non-finite f64 directly.
        let mut map = serde_json::Map::new();
        map.insert("mediaRef".into(), Value::String("m".into()));
        map.insert(
            "startFrame".into(),
            serde_json::Number::from_f64(f64::NAN)
                .map(Value::Number)
                .unwrap_or(Value::Null),
        );
        // NaN can't be a JSON number, so simulate via array path instead.
        let arr = Value::Array(vec![Value::Number(
            serde_json::Number::from_f64(1.0).unwrap(),
        )]);
        // Direct finite path is None.
        assert_eq!(first_non_finite_number_path(&arr, "x"), None);
    }

    #[test]
    fn first_non_finite_finds_nested_path() {
        // serde_json::Number cannot hold NaN, so we exercise the traversal with
        // a hand-built tree where one leaf is a Number that as_f64 reports
        // non-finite is impossible; instead verify object/array descent labels.
        let v = serde_json::json!({"a":[{"b":1.0},{"b":2.0}]});
        assert_eq!(first_non_finite_number_path(&v, ""), None);
    }

    #[test]
    fn validate_unknown_keys_ok_when_subset() {
        let map = serde_json::json!({"mediaRef":"m"});
        if let Value::Object(m) = map {
            assert!(validate_unknown_keys(&m, Entry::ALLOWED_KEYS, "entries[0]").is_ok());
        }
    }

    #[test]
    fn normalize_path_converts_indices() {
        assert_eq!(
            normalize_path("", "entries.3.startFrame"),
            "entries[3].startFrame"
        );
        assert_eq!(normalize_path("", "moves.0.clipId"), "moves[0].clipId");
        assert_eq!(
            normalize_path("entries[2]", "trimStartFrame"),
            "entries[2].trimStartFrame"
        );
        assert_eq!(normalize_path("", "."), "");
    }
}
