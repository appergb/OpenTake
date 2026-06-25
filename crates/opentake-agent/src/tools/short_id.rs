//! Short-id system: outbound shortening / inbound expansion (`agent-SPEC.md`
//! §3). 1:1 port of upstream `ToolExecutor+ShortId.swift`.
//!
//! Entity ids are full UUIDs (~36 chars) and dominate large `get_timeline` /
//! `get_transcript` payloads. We emit the shortest project-unique prefix
//! (≥ 8 chars) and accept any prefix back: tools always run on full ids
//! (resolved on input), and every text response has its known ids shortened on
//! the way out. The system prompt instructs the model to pass prefixes back
//! verbatim (`prompt::base`).

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use opentake_domain::{MediaManifest, Timeline};
use regex::Regex;

use crate::tools::errors::ToolError;
use crate::tools::result::{Block, ToolResult};

/// Minimum prefix length (upstream `idPrefixFloor`).
const ID_PREFIX_FLOOR: usize = 8;

/// Scalar argument keys whose string value is an id prefix to expand
/// (upstream `scalarIdKeys`).
const SCALAR_ID_KEYS: &[&str] = &[
    "clipId",
    "sourceClipId",
    "mediaRef",
    "startFrameMediaRef",
    "endFrameMediaRef",
    "sourceVideoMediaRef",
    "videoSourceMediaRef",
    "folderId",
    "parentFolderId",
];

/// Array argument keys whose string elements are id prefixes to expand
/// (upstream `arrayIdKeys`).
const ARRAY_ID_KEYS: &[&str] = &[
    "clipIds",
    "assetIds",
    "folderIds",
    "referenceMediaRefs",
    "referenceImageMediaRefs",
    "referenceVideoMediaRefs",
    "referenceAudioMediaRefs",
];

fn uuid_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}")
            .expect("valid uuid regex")
    })
}

/// Every entity id the agent can see or name back, collected from the timeline
/// and media manifest. One set serves both directions (upstream
/// `currentIdUniverse`). The signal/context layer feeds the live timeline and
/// manifest in here.
pub fn current_id_universe(timeline: &Timeline, manifest: &MediaManifest) -> HashSet<String> {
    let mut ids = HashSet::new();
    for track in &timeline.tracks {
        if !track.id.is_empty() {
            ids.insert(track.id.clone());
        }
        for clip in &track.clips {
            if !clip.id.is_empty() {
                ids.insert(clip.id.clone());
            }
            if let Some(g) = &clip.caption_group_id {
                ids.insert(g.clone());
            }
            if let Some(g) = &clip.link_group_id {
                ids.insert(g.clone());
            }
        }
    }
    for entry in &manifest.entries {
        if !entry.id.is_empty() {
            ids.insert(entry.id.clone());
        }
    }
    for folder in &manifest.folders {
        if !folder.id.is_empty() {
            ids.insert(folder.id.clone());
        }
    }
    ids
}

/// Map each id to its shortest prefix (≥ 8 chars) that no other id shares.
/// 1:1 port of `shortIdMap`. UUIDs are ASCII so byte slicing is safe; a
/// non-ASCII id (should not happen) is handled char-wise to avoid panics.
pub fn short_id_map(ids: &HashSet<String>) -> HashMap<String, String> {
    let mut out = HashMap::with_capacity(ids.len());
    for id in ids {
        let char_len = id.chars().count();
        let mut len = ID_PREFIX_FLOOR.min(char_len);
        while len < char_len {
            let prefix = take_chars(id, len);
            let collides = ids
                .iter()
                .any(|other| other != id && other.starts_with(&prefix));
            if collides {
                len += 1;
            } else {
                break;
            }
        }
        out.insert(id.clone(), take_chars(id, len));
    }
    out
}

fn take_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Replace every known full UUID in the result's text blocks with its short
/// prefix. Unknown UUIDs (e.g. embedded in a filename) pass through untouched.
/// 1:1 port of `shorteningIds`. Done on the post-run state so newly created ids
/// in summaries are shortened too (`agent-SPEC.md` §3.3).
pub fn shorten_ids(result: ToolResult, ids: &HashSet<String>) -> ToolResult {
    let map = short_id_map(ids);
    if map.is_empty() {
        return result;
    }
    let re = uuid_regex();
    let content = result
        .content
        .into_iter()
        .map(|block| match block {
            Block::Text { text } => {
                let replaced = re
                    .replace_all(&text, |caps: &regex::Captures<'_>| {
                        let m = caps.get(0).expect("group 0").as_str();
                        map.get(m).cloned().unwrap_or_else(|| m.to_string())
                    })
                    .into_owned();
                Block::Text { text: replaced }
            }
            other => other,
        })
        .collect();
    ToolResult {
        content,
        is_error: result.is_error,
    }
}

/// Expand id-prefix arguments back to full ids before a tool runs. Throws on an
/// ambiguous prefix; leaves unknown values untouched so the tool emits its own
/// not-found error. 1:1 port of `expandingIdPrefixes`. Recurses through nested
/// objects/arrays so `entries[].mediaRef`, `moves[].clipId` etc. are covered.
pub fn expand_id_prefixes(
    args: &serde_json::Value,
    universe: &HashSet<String>,
) -> Result<serde_json::Value, ToolError> {
    expand_value(args, universe)
}

fn expand_value(
    value: &serde_json::Value,
    universe: &HashSet<String>,
) -> Result<serde_json::Value, ToolError> {
    match value {
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (key, v) in map {
                let new_v = if SCALAR_ID_KEYS.contains(&key.as_str()) {
                    if let serde_json::Value::String(s) = v {
                        serde_json::Value::String(expand_one(s, universe)?)
                    } else {
                        expand_value(v, universe)?
                    }
                } else if ARRAY_ID_KEYS.contains(&key.as_str()) {
                    if let serde_json::Value::Array(arr) = v {
                        let mut new_arr = Vec::with_capacity(arr.len());
                        for el in arr {
                            if let serde_json::Value::String(s) = el {
                                new_arr.push(serde_json::Value::String(expand_one(s, universe)?));
                            } else {
                                new_arr.push(expand_value(el, universe)?);
                            }
                        }
                        serde_json::Value::Array(new_arr)
                    } else {
                        expand_value(v, universe)?
                    }
                } else {
                    expand_value(v, universe)?
                };
                out.insert(key.clone(), new_v);
            }
            Ok(serde_json::Value::Object(out))
        }
        serde_json::Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for el in arr {
                out.push(expand_value(el, universe)?);
            }
            Ok(serde_json::Value::Array(out))
        }
        other => Ok(other.clone()),
    }
}

/// Expand one prefix: full id passes through, a unique prefix resolves, an
/// unknown value passes through (tool reports not-found), an ambiguous prefix
/// errors. 1:1 port of `expandOne`.
fn expand_one(reference: &str, universe: &HashSet<String>) -> Result<String, ToolError> {
    if universe.contains(reference) {
        return Ok(reference.to_string());
    }
    let matches: Vec<&String> = universe
        .iter()
        .filter(|id| id.starts_with(reference))
        .collect();
    match matches.len() {
        1 => Ok(matches[0].clone()),
        0 => Ok(reference.to_string()),
        n => Err(ToolError::new(format!(
            "Ambiguous id '{reference}' matches {n} items; re-read with get_timeline or get_media for current ids."
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Two UUIDs sharing the first 12 chars, then diverging at index 14/15.
    const A: &str = "11111111-1111-aaaa-0000-000000000000";
    const B: &str = "11111111-1111-bbbb-0000-000000000000";
    // A UUID unique from the very floor.
    const C: &str = "22222222-2222-2222-2222-222222222222";

    fn universe(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn unique_id_shortens_to_floor() {
        let u = universe(&[C]);
        let map = short_id_map(&u);
        assert_eq!(map[C], &C[..8]); // exactly the 8-char floor
    }

    #[test]
    fn shared_prefix_extends_until_unique() {
        // A and B share "11111111-1111-" (14 chars), diverge at index 14
        // ('a' vs 'b'). Shortest unique prefix is 15 chars.
        let u = universe(&[A, B]);
        let map = short_id_map(&u);
        assert_eq!(map[A], &A[..15]);
        assert_eq!(map[B], &B[..15]);
        assert_ne!(map[A], map[B]);
    }

    #[test]
    fn expand_unique_prefix_resolves_to_full() {
        let u = universe(&[C]);
        let got = expand_one(&C[..10], &u).unwrap();
        assert_eq!(got, C);
    }

    #[test]
    fn expand_full_id_passes_through() {
        let u = universe(&[C]);
        assert_eq!(expand_one(C, &u).unwrap(), C);
    }

    #[test]
    fn expand_unknown_passes_through() {
        let u = universe(&[C]);
        let got = expand_one("ffffffff", &u).unwrap();
        assert_eq!(got, "ffffffff"); // tool reports not-found itself
    }

    #[test]
    fn expand_ambiguous_prefix_errors() {
        let u = universe(&[A, B]);
        // "11111111" matches both A and B.
        let err = expand_one("11111111", &u).unwrap_err();
        assert!(
            err.message.contains("Ambiguous id '11111111'"),
            "{}",
            err.message
        );
        assert!(err.message.contains("matches 2 items"), "{}", err.message);
    }

    #[test]
    fn shorten_replaces_known_uuid_in_text() {
        let u = universe(&[C]);
        let r = ToolResult::ok(format!("clip {C} added"));
        let out = shorten_ids(r, &u);
        assert_eq!(out.text_joined(), format!("clip {} added", &C[..8]));
    }

    #[test]
    fn shorten_leaves_unknown_uuid_untouched() {
        // A filename-embedded UUID not in the universe.
        let other = "99999999-9999-9999-9999-999999999999";
        let u = universe(&[C]);
        let r = ToolResult::ok(format!("file {other}.mp4"));
        let out = shorten_ids(r, &u);
        assert_eq!(out.text_joined(), format!("file {other}.mp4"));
    }

    #[test]
    fn expand_recurses_into_nested_entries() {
        let u = universe(&[C]);
        let args = serde_json::json!({
            "entries": [{"mediaRef": &C[..9], "startFrame": 0}]
        });
        let out = expand_id_prefixes(&args, &u).unwrap();
        assert_eq!(out["entries"][0]["mediaRef"], serde_json::json!(C));
        assert_eq!(out["entries"][0]["startFrame"], serde_json::json!(0));
    }

    #[test]
    fn expand_array_id_keys() {
        let u = universe(&[A, B]);
        let args = serde_json::json!({"clipIds": [&A[..15], &B[..15]]});
        let out = expand_id_prefixes(&args, &u).unwrap();
        assert_eq!(out["clipIds"][0], serde_json::json!(A));
        assert_eq!(out["clipIds"][1], serde_json::json!(B));
    }

    #[test]
    fn expand_ambiguous_in_array_errors() {
        let u = universe(&[A, B]);
        let args = serde_json::json!({"clipIds": ["11111111"]});
        let err = expand_id_prefixes(&args, &u).unwrap_err();
        assert!(err.message.contains("Ambiguous"), "{}", err.message);
    }

    #[test]
    fn universe_collects_from_timeline_and_manifest() {
        use opentake_domain::{Clip, ClipType, MediaFolder, MediaManifest, Timeline, Track};
        let mut tl = Timeline::new();
        let mut t = Track::new("track-1", ClipType::Video);
        let mut c = Clip::new("clip-1", "asset-1", 0, 30);
        c.link_group_id = Some("link-1".into());
        c.caption_group_id = Some("cap-1".into());
        t.clips.push(c);
        tl.tracks.push(t);
        let mut m = MediaManifest::new();
        m.folders.push(MediaFolder::new("folder-1", "B-Roll"));
        let ids = current_id_universe(&tl, &m);
        for want in ["track-1", "clip-1", "link-1", "cap-1", "folder-1"] {
            assert!(ids.contains(want), "missing {want}");
        }
    }
}
