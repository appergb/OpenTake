//! Context-signal engine: build a `ContextSignal` and attach it to a tool result
//! (`agent-SPEC.md` §6.1). Runs in the execution shell after the tool's `run`,
//! before short-id shortening. Only attaches when the tool has a matching signal
//! (`get_timeline` etc.); pure-CRUD tools get nothing.
//!
//! Plugin overrides (`agent-SPEC.md` §7.6, priority plugin > manual > auto):
//! a declared `video_type` / `track_roles` override auto-detection; the plugin's
//! stages append to stage_guidance; `dont` rules run alongside built-in rules.

use opentake_domain::{ContextSignal, Timeline, TrackRoleAssignment, VideoType};
use serde_json::json;

use crate::plugin::registry::{parse_track_role, parse_video_type, LoadedPlugin};
use crate::plugin::rules::plugin_rules;
use crate::signal::{classify, rules::builtin_rules, rules::OpContext, stages, track_roles};
use crate::tools::names::ToolName;
use crate::tools::result::{Block, ToolResult};

/// Build the full context signal for the current timeline, applying plugin
/// overrides. `manual_video_type` is the project setting (priority below plugin,
/// above auto).
pub fn build_signal(
    timeline: &Timeline,
    plugin: Option<&LoadedPlugin>,
    manual_video_type: Option<VideoType>,
) -> ContextSignal {
    // Video type: plugin > manual > auto.
    let (auto_type, auto_conf) = classify::classify(timeline);
    let plugin_type = plugin.and_then(|p| parse_video_type(&p.manifest.video_type.primary));
    let (video_type, confidence) = match (plugin_type, manual_video_type) {
        (Some(t), _) => (t, 1.0),
        (None, Some(t)) => (t, 1.0),
        (None, None) => (auto_type, auto_conf),
    };

    // Track roles: plugin overrides win on matching track labels (V1/A1/...).
    let mut roles = track_roles::detect_track_roles(timeline);
    if let Some(p) = plugin {
        apply_plugin_track_roles(&mut roles, timeline, p);
    }

    let stage = stages::infer_stage(timeline);
    let mut guidance = stages::stage_guidance(stage);
    // Append plugin stages to next_actions, tagged with the plugin id.
    if let Some(p) = plugin {
        for st in &p.manifest.workflow.stages {
            for action in &st.actions {
                if !action.tip.is_empty() {
                    guidance.next_actions.push(format!(
                        "[plugin:{}] {}: {}",
                        p.id(),
                        action.tool,
                        action.tip
                    ));
                }
            }
        }
    }

    let skeleton = stages::editing_skeleton(video_type);
    let hints = track_roles::track_hints(&roles);

    ContextSignal {
        video_type,
        confidence,
        track_roles: roles,
        editing_stage: stage,
        stage_guidance: guidance,
        editing_skeleton: skeleton,
        track_hints: hints,
    }
}

/// Apply a plugin's `track_roles` map (keys like "V1"/"A1") over the detected
/// assignments. The label is matched against the encode-layer track label.
fn apply_plugin_track_roles(
    roles: &mut [TrackRoleAssignment],
    timeline: &Timeline,
    plugin: &LoadedPlugin,
) {
    for assignment in roles.iter_mut() {
        let label = track_label(timeline, assignment.track_index);
        if let Some(pr) = plugin.manifest.track_roles.get(&label) {
            if let Some(role) = parse_track_role(&pr.role) {
                assignment.role = role;
            }
        }
    }
}

/// Track label ("V1"/"A1"/...) — mirrors `encode_timeline::track_label` but kept
/// local to avoid a cross-module pub dependency.
fn track_label(timeline: &Timeline, index: usize) -> String {
    if index >= timeline.tracks.len() {
        return String::new();
    }
    let kind = timeline.tracks[index].kind;
    let prefix = match kind {
        opentake_domain::ClipType::Video => "V",
        opentake_domain::ClipType::Audio => "A",
        opentake_domain::ClipType::Image => "I",
        opentake_domain::ClipType::Text => "T",
        opentake_domain::ClipType::Lottie => "L",
    };
    let n = timeline.tracks[..=index]
        .iter()
        .filter(|t| t.kind == kind)
        .count();
    format!("{prefix}{n}")
}

/// Whether a tool carries a context_signal (`agent-SPEC.md` §6.1 table). Pure
/// CRUD (folders group) returns false.
pub fn tool_emits_signal(tool: ToolName) -> bool {
    matches!(
        tool,
        ToolName::GetTimeline
            | ToolName::InspectMedia
            | ToolName::AddClips
            | ToolName::InsertClips
            | ToolName::GetTranscript
            | ToolName::SearchMedia
            | ToolName::AddTexts
            | ToolName::AddCaptions
            // write tools that can trigger rule warnings:
            | ToolName::RemoveClips
            | ToolName::MoveClips
            | ToolName::SplitClip
            | ToolName::RippleDeleteRanges
            | ToolName::SetClipProperties
            | ToolName::SetKeyframes
    )
}

/// Attach a `context_signal` JSON block to a result, after the main content
/// (`agent-SPEC.md` §6.1). For `get_timeline` the full signal is attached; for
/// write tools, the rule warnings (built-in + plugin) are attached when present.
/// `op` is the executed operation for rule evaluation.
pub fn attach(
    tool: ToolName,
    mut result: ToolResult,
    timeline: &Timeline,
    plugin: Option<&LoadedPlugin>,
    manual_video_type: Option<VideoType>,
    op: &OpContext,
) -> ToolResult {
    if result.is_error || !tool_emits_signal(tool) {
        return result;
    }
    let signal = build_signal(timeline, plugin, manual_video_type);

    match tool {
        ToolName::GetTimeline => {
            // Full signal for the session-start read.
            if let Ok(json) = serde_json::to_value(&signal) {
                result.push(Block::text(json!({"context_signal": json}).to_string()));
            }
        }
        ToolName::GetTranscript
        | ToolName::SearchMedia
        | ToolName::InspectMedia
        | ToolName::AddTexts
        | ToolName::AddCaptions
        | ToolName::AddClips
        | ToolName::InsertClips => {
            // Lighter signal: track roles + stage guidance + any warnings.
            let mut warnings = builtin_rules(tool, op, &signal.track_roles, timeline);
            warnings.extend(plugin_rules(plugin, &signal.track_roles, timeline));
            let payload = json!({
                "context_signal": {
                    "video_type": signal.video_type,
                    "track_roles": signal.track_roles,
                    "stage_guidance": signal.stage_guidance,
                    "warnings": warnings,
                }
            });
            result.push(Block::text(payload.to_string()));
        }
        _ => {
            // Pure write tools: attach only rule warnings, and only if any.
            let mut warnings = builtin_rules(tool, op, &signal.track_roles, timeline);
            warnings.extend(plugin_rules(plugin, &signal.track_roles, timeline));
            if !warnings.is_empty() {
                result.push(Block::text(
                    json!({"context_signal": {"warnings": warnings}}).to_string(),
                ));
            }
        }
    }
    result
}

/// Convenience: extract the `context_signal` JSON from the last block of a
/// result (used by tests and by the chat layer).
pub fn extract_signal(result: &ToolResult) -> Option<serde_json::Value> {
    for block in result.content.iter().rev() {
        if let Block::Text { text } = block {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
                if v.get("context_signal").is_some() {
                    return Some(v["context_signal"].clone());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{Clip, ClipType, Track, TrackRole};

    fn talking_head_timeline() -> Timeline {
        let mut tl = Timeline::new();
        let mut v = Track::new("v1", ClipType::Video);
        v.clips.push(Clip::new("c1", "asset", 0, 30 * 20));
        let mut a = Track::new("a1", ClipType::Audio);
        a.clips.push(Clip::new("au", "asset", 0, 30 * 20));
        tl.tracks.push(v);
        tl.tracks.push(a);
        tl
    }

    #[test]
    fn get_timeline_attaches_full_signal() {
        let tl = talking_head_timeline();
        let r = ToolResult::ok("{...timeline...}");
        let out = attach(
            ToolName::GetTimeline,
            r,
            &tl,
            None,
            None,
            &OpContext::default(),
        );
        let sig = extract_signal(&out).expect("signal present");
        assert_eq!(sig["video_type"], json!("talking_head"));
        assert!(sig["track_roles"].as_array().unwrap().len() >= 2);
        assert!(sig["editing_skeleton"]["approach"] == json!("audio_driven"));
        assert!(sig["track_hints"].as_array().unwrap().len() >= 2);
    }

    #[test]
    fn error_result_gets_no_signal() {
        let tl = talking_head_timeline();
        let r = ToolResult::error("boom");
        let out = attach(
            ToolName::GetTimeline,
            r,
            &tl,
            None,
            None,
            &OpContext::default(),
        );
        assert!(extract_signal(&out).is_none());
    }

    #[test]
    fn pure_crud_tool_gets_no_signal() {
        let tl = talking_head_timeline();
        let r = ToolResult::ok("folders...");
        let out = attach(
            ToolName::ListFolders,
            r,
            &tl,
            None,
            None,
            &OpContext::default(),
        );
        assert!(extract_signal(&out).is_none());
    }

    #[test]
    fn plugin_video_type_overrides_auto() {
        let tl = talking_head_timeline(); // auto = talking_head
        let json =
            r#"{"schema_version":"1.0","id":"wp","name":"WP","video_type":{"primary":"montage"}}"#;
        let plugin =
            crate::plugin::registry::PluginRegistry::load_from_strings(json, "", ".").unwrap();
        let sig = build_signal(&tl, Some(&plugin), None);
        assert_eq!(sig.video_type, VideoType::Montage); // plugin wins
        assert_eq!(sig.confidence, 1.0);
    }

    #[test]
    fn manual_video_type_overrides_auto_when_no_plugin() {
        let tl = talking_head_timeline();
        let sig = build_signal(&tl, None, Some(VideoType::Vlog));
        assert_eq!(sig.video_type, VideoType::Vlog);
        assert_eq!(sig.confidence, 1.0);
    }

    #[test]
    fn plugin_track_roles_override_detection() {
        let mut tl = Timeline::new();
        // Single video track auto-detects MainCamera; plugin says BRollOverlay.
        let mut v = Track::new("v1", ClipType::Video);
        v.clips.push(Clip::new("c", "a", 0, 30 * 15));
        tl.tracks.push(v);
        let json = r#"{"schema_version":"1.0","id":"wp","name":"WP","track_roles":{"V1":{"role":"BRollOverlay"}}}"#;
        let plugin =
            crate::plugin::registry::PluginRegistry::load_from_strings(json, "", ".").unwrap();
        let sig = build_signal(&tl, Some(&plugin), None);
        assert_eq!(sig.track_roles[0].role, TrackRole::BRoll);
    }

    #[test]
    fn plugin_stages_appended_to_guidance() {
        let tl = talking_head_timeline();
        let json = r#"{"schema_version":"1.0","id":"wp","name":"WP","workflow":{"stages":[{"id":"s","order":0,"actions":[{"tool":"split_clip","tip":"在气口处分割"}]}]}}"#;
        let plugin =
            crate::plugin::registry::PluginRegistry::load_from_strings(json, "", ".").unwrap();
        let sig = build_signal(&tl, Some(&plugin), None);
        assert!(sig
            .stage_guidance
            .next_actions
            .iter()
            .any(|a| a.contains("[plugin:wp]") && a.contains("在气口处分割")));
    }

    #[test]
    fn remove_on_voice_track_attaches_warning() {
        let tl = talking_head_timeline();
        let op = OpContext {
            track_index: Some(1), // audio track -> Voice role
            clip_ids: vec!["au".into()],
            ..Default::default()
        };
        let r = ToolResult::ok("{removed}");
        let out = attach(ToolName::RemoveClips, r, &tl, None, None, &op);
        let sig = extract_signal(&out).expect("signal");
        let warnings = sig["warnings"].as_array().unwrap();
        assert!(warnings
            .iter()
            .any(|w| w.as_str().unwrap().contains("主干内容")));
    }

    #[test]
    fn split_with_no_warning_omits_signal_block() {
        // split on a non-voice track with mid_word=false -> no warnings -> no block.
        let mut tl = Timeline::new();
        let mut v = Track::new("v1", ClipType::Video);
        v.clips.push(Clip::new("c", "a", 0, 30 * 2)); // short -> BRoll role
        tl.tracks.push(v);
        let op = OpContext {
            track_index: Some(0),
            mid_word: Some(false),
            ..Default::default()
        };
        let r = ToolResult::ok("{split}");
        let out = attach(ToolName::SplitClip, r, &tl, None, None, &op);
        assert!(extract_signal(&out).is_none());
    }
}
