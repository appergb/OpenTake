//! Convert the transport-neutral [`ToolResult`] into rmcp's [`CallToolResult`]
//! (`agent-SPEC.md` §8.3). Successful blocks retain their native MCP mapping.
//! Errors pass through the same fail-closed LLM boundary used by in-app chat.

use std::sync::atomic::{AtomicU64, Ordering};

use rmcp::model::{CallToolResult, ContentBlock};
use serde::Serialize;
use serde_json::Value;

use crate::tools::descriptions::input_schema;
use crate::tools::names::ToolName;
use crate::tools::result::{Block, PublicErrorKind, ToolResult};

/// Map one neutral [`Block`] to an rmcp [`ContentBlock`].
fn block_to_content(block: Block) -> ContentBlock {
    match block {
        Block::Text { text } => ContentBlock::text(text),
        Block::Image { base64, media_type } => ContentBlock::image(base64, media_type),
    }
}

/// Map a [`ToolResult`] to an rmcp [`CallToolResult`].
pub fn to_call_tool_result(result: ToolResult) -> CallToolResult {
    if result.is_error {
        let payload = safe_tool_result_for_llm(&result).to_string();
        return CallToolResult::error(vec![ContentBlock::text(payload)]);
    }
    let content: Vec<ContentBlock> = result.content.into_iter().map(block_to_content).collect();
    CallToolResult::success(content)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SafeLlmError {
    is_error: bool,
    code: String,
    message: &'static str,
    details: Option<String>,
    remediation: &'static str,
    error_id: String,
}

static LLM_ERROR_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Build the one fail-closed tool-result representation shared by MCP and the
/// in-app chat loop. Successful summaries keep their established shape. Error
/// text is private unless a dispatcher-owned typed marker explicitly permits
/// disclosure and this boundary independently accepts the detail.
pub fn safe_tool_result_for_llm(result: &ToolResult) -> Value {
    if !result.is_error {
        return serde_json::json!({
            "summary": result.text_joined(),
            "isError": false,
        });
    }

    let private_detail = result
        .content
        .iter()
        .filter_map(|block| match block {
            Block::Text { text } => Some(text.as_str()),
            Block::Image { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let error_id = format!(
        "agent-{}",
        LLM_ERROR_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let public_kind = result.public_error_kind();
    let public_detail = public_kind.and_then(|kind| safe_public_detail(kind, &private_detail));
    let diagnostic_code = public_kind
        .map(PublicErrorKind::code)
        .unwrap_or_else(|| private_diagnostic_code(&private_detail));
    let error = match public_kind {
        Some(kind) => SafeLlmError {
            is_error: true,
            code: kind.code().to_string(),
            message: kind.message(),
            details: public_detail,
            remediation: kind.remediation(),
            error_id,
        },
        None => SafeLlmError {
            is_error: true,
            code: "MCP_TOOL_ERROR_REDACTED".to_string(),
            message: "The tool failed, but sensitive internal details were withheld.",
            details: None,
            remediation:
                "Verify the referenced source or arguments, then retry. If the failure persists, record errorId when reporting the operation.",
            error_id,
        },
    };
    tracing::warn!(
        target: "opentake::agent::private",
        error_code = %error.code,
        diagnostic_code,
        error_id = %error.error_id,
        detail_redacted = public_kind.is_none() || error.details.is_none(),
        "tool call returned an error at an LLM boundary"
    );
    serde_json::to_value(error).unwrap_or_else(|_| {
        serde_json::json!({
            "isError": true,
            "code": "MCP_TOOL_ERROR_REDACTED",
            "message": "The tool failed.",
            "details": null,
            "remediation": "Retry the operation.",
            "errorId": "serialization"
        })
    })
}

fn private_diagnostic_code(private_detail: &str) -> &'static str {
    for code in [
        "MCP_MEDIA_PROBE_FAILED",
        "MCP_SOURCE_PATH_UNREADABLE",
        "MCP_SOURCE_IMPORT_FAILED",
    ] {
        if private_detail
            .strip_prefix(code)
            .is_some_and(|suffix| suffix.starts_with(':'))
        {
            return code;
        }
    }
    "MCP_TOOL_FAILURE"
}

/// Produce details from a fixed grammar instead of copying the original error.
/// Even explicitly public errors cannot pass arbitrary user/provider text.
fn safe_public_detail(kind: PublicErrorKind, private_detail: &str) -> Option<String> {
    match kind {
        PublicErrorKind::UnknownTool => Some("Unknown tool name.".to_string()),
        PublicErrorKind::InvalidArguments(tool) => {
            Some(safe_invalid_argument_detail(tool, private_detail))
        }
        PublicErrorKind::ResourceNotFound(tool) => Some(format!(
            "{} could not resolve the referenced project resource.",
            tool.as_str()
        )),
        PublicErrorKind::CapabilityUnavailable(tool) => Some(format!(
            "{} cannot inspect this source in the current build or source state.",
            tool.as_str()
        )),
        PublicErrorKind::PathAuthorityRequired(tool) => Some(format!(
            "{} cannot use a model-supplied local path without user-granted file access.",
            tool.as_str()
        )),
        PublicErrorKind::AnalysisLowConfidence(tool) => Some(format!(
            "{} could not identify the selected subject reliably.",
            tool.as_str()
        )),
    }
}

fn safe_invalid_argument_detail(tool: ToolName, private_detail: &str) -> String {
    let (raw_path, reason) = private_detail
        .split_once(':')
        .unwrap_or(("arguments", private_detail));
    let schema = input_schema(tool);
    let path = safe_schema_path(&schema, raw_path.trim());
    let category = if reason.contains("missing required field") {
        "missing a required field"
    } else if reason.contains("unknown field") {
        "contains an unknown field"
    } else if reason.contains("expected") {
        "has the wrong type"
    } else if reason.contains("value must be finite") {
        "must be finite"
    } else {
        "has an invalid value"
    };
    format!("{path}: {category}")
}

/// Rebuild a path only from properties declared by this tool's JSON Schema.
/// Open maps such as `params` stop at the declared container, so caller-owned
/// keys can never be copied to the model.
fn safe_schema_path(schema: &Value, raw_path: &str) -> String {
    if raw_path.is_empty() || raw_path.len() > 160 || !raw_path.is_ascii() {
        return "arguments".to_string();
    }
    let bytes = raw_path.as_bytes();
    let mut index = 0;
    let mut current = schema;
    let mut output = String::new();
    let mut first = true;

    while index < bytes.len() {
        let field_start = index;
        if !bytes[index].is_ascii_alphabetic() {
            break;
        }
        index += 1;
        while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
        {
            index += 1;
        }
        let field = &raw_path[field_start..index];
        if first && field == "arguments" {
            output.push_str("arguments");
        } else {
            let Some(next) = schema_property(current, field) else {
                break;
            };
            if !output.is_empty() {
                output.push('.');
            }
            output.push_str(field);
            current = next;
        }
        first = false;

        while index < bytes.len() && bytes[index] == b'[' {
            let Some(items) = schema_items(current) else {
                return if output.is_empty() {
                    "arguments".to_string()
                } else {
                    output
                };
            };
            let bracket_start = index;
            index += 1;
            let digits_start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if digits_start == index || index == bytes.len() || bytes[index] != b']' {
                return if output.is_empty() {
                    "arguments".to_string()
                } else {
                    output
                };
            }
            index += 1;
            output.push_str(&raw_path[bracket_start..index]);
            current = items;
        }

        if index == bytes.len() {
            break;
        }
        if bytes[index] != b'.' {
            break;
        }
        index += 1;
    }

    if output.is_empty() {
        "arguments".to_string()
    } else {
        output
    }
}

fn schema_property<'a>(schema: &'a Value, field: &str) -> Option<&'a Value> {
    if let Some(property) = schema.get("properties").and_then(|value| value.get(field)) {
        return Some(property);
    }
    ["allOf", "anyOf", "oneOf"].into_iter().find_map(|key| {
        schema
            .get(key)
            .and_then(Value::as_array)
            .and_then(|branches| {
                branches
                    .iter()
                    .find_map(|branch| schema_property(branch, field))
            })
    })
}

fn schema_items(schema: &Value) -> Option<&Value> {
    if let Some(items) = schema.get("items") {
        return Some(items);
    }
    ["allOf", "anyOf", "oneOf"].into_iter().find_map(|key| {
        schema
            .get(key)
            .and_then(Value::as_array)
            .and_then(|branches| branches.iter().find_map(schema_items))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ok_result_maps_to_success_text() {
        let r = ToolResult::ok("hello");
        let c = to_call_tool_result(r);
        assert_ne!(c.is_error, Some(true));
        assert_eq!(c.content.len(), 1);
    }

    #[test]
    fn private_error_result_maps_to_redacted_error() {
        let r = ToolResult::error("nope");
        let c = to_call_tool_result(r);
        assert_eq!(c.is_error, Some(true));
        let wire = serde_json::to_string(&c).unwrap();
        assert!(wire.contains("MCP_TOOL_ERROR_REDACTED"));
        assert!(!wire.contains("nope"));
    }

    #[test]
    fn private_diagnostic_code_extracts_only_fixed_internal_prefixes() {
        assert_eq!(
            private_diagnostic_code("MCP_MEDIA_PROBE_FAILED: private decoder detail"),
            "MCP_MEDIA_PROBE_FAILED"
        );
        assert_eq!(
            private_diagnostic_code("MCP_MEDIA_PROBE_FAILED_BYPASS: private"),
            "MCP_TOOL_FAILURE"
        );
    }

    #[test]
    fn explicitly_public_preflight_error_keeps_safe_detail() {
        let r = ToolResult::public_error(
            PublicErrorKind::InvalidArguments(ToolName::AddClips),
            "entries[3].startFrame: expected i32, got something else",
        );
        let c = to_call_tool_result(r);
        let wire = serde_json::to_string(&c).unwrap();
        assert!(wire.contains("MCP_INVALID_ARGUMENTS"));
        assert!(wire.contains("entries[3].startFrame"));
        assert!(wire.contains("wrong type"));
        assert!(!wire.contains("expected i32"));
    }

    #[test]
    fn typed_unavailable_error_exposes_only_fixed_recovery_contract() {
        let private = "inspect_media: /Users/alice/private.mov is offline";
        let result = ToolResult::public_error(
            PublicErrorKind::CapabilityUnavailable(ToolName::InspectMedia),
            private,
        );
        let value = safe_tool_result_for_llm(&result);
        assert_eq!(value["code"], "MCP_CAPABILITY_UNAVAILABLE");
        assert_eq!(
            value["message"],
            "This capability is unavailable for the referenced media."
        );
        assert_eq!(
            value["details"],
            "inspect_media cannot inspect this source in the current build or source state."
        );
        assert!(!value.to_string().contains("/Users/alice"));
    }

    #[test]
    fn low_confidence_error_has_a_fixed_safe_retry_contract() {
        let result = ToolResult::public_error(
            PublicErrorKind::AnalysisLowConfidence(ToolName::TrackMotion),
            "confidence=0.02 path=/private/source.mp4",
        );
        let value = safe_tool_result_for_llm(&result);
        assert_eq!(value["code"], "MCP_ANALYSIS_LOW_CONFIDENCE");
        assert_eq!(
            value["details"],
            "track_motion could not identify the selected subject reliably."
        );
        assert!(!value.to_string().contains("/private/source.mp4"));
    }

    #[test]
    fn explicitly_public_marker_cannot_bypass_detail_guard() {
        let private = "/Users/alice/private.mp4";
        let r = ToolResult::public_error(
            PublicErrorKind::InvalidArguments(ToolName::ImportMedia),
            format!("source.path: {private}"),
        );
        let wire = serde_json::to_string(&to_call_tool_result(r)).unwrap();
        assert!(wire.contains("MCP_INVALID_ARGUMENTS"));
        assert!(wire.contains("source.path"));
        assert!(!wire.contains(private));
    }

    #[test]
    fn public_detail_grammar_rejects_arbitrary_prefix() {
        let private = "quota exhausted for customer alice: wrong type";
        let r = ToolResult::public_error(
            PublicErrorKind::InvalidArguments(ToolName::AddClips),
            private,
        );
        let value = safe_tool_result_for_llm(&r);
        assert_eq!(value["code"], "MCP_INVALID_ARGUMENTS");
        assert_eq!(value["details"], "arguments: has an invalid value");
        assert!(!value.to_string().contains(private));
    }

    #[test]
    fn dynamic_param_key_is_truncated_at_static_schema_container() {
        let private = "params.ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890";
        let r = ToolResult::public_error(
            PublicErrorKind::InvalidArguments(ToolName::EditMotionGraphic),
            format!("{private}: expected string, number, or bool"),
        );
        let value = safe_tool_result_for_llm(&r);
        assert_eq!(value["details"], "params: has the wrong type");
        assert!(!value.to_string().contains(private));
    }

    #[test]
    fn image_block_maps_to_image_content() {
        let r = ToolResult::blocks(vec![Block::image("AAAA", "image/png")]);
        let c = to_call_tool_result(r);
        assert_eq!(c.content.len(), 1);
    }
}
