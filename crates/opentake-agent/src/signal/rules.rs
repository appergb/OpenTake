//! Built-in editing-rule validation (`agent-SPEC.md` §6.6.1; warning text
//! VERBATIM). Runs after a write tool to flag operations that disagree with
//! ClipSkills heuristics. The MVP implements the structurally-decidable rules;
//! semantically-heavy ones (breath detection, information density, clock theory)
//! are surfaced as soft warnings once `opentake-media` lands.
//!
//! Built-in + plugin rules both apply (order: built-in → plugin), combined into
//! one warning list (`agent-SPEC.md` §6.6).

use opentake_domain::{Timeline, TrackRole, TrackRoleAssignment};

use crate::tools::names::ToolName;

/// What an executed write tool did, distilled to what the rule checks need.
/// Built by the dispatch layer from the resolved args + before/after timelines.
#[derive(Debug, Clone, Default)]
pub struct OpContext {
    /// Track index the operation primarily touched (for role lookup).
    pub track_index: Option<usize>,
    /// Clip ids the operation removed / split / trimmed.
    pub clip_ids: Vec<String>,
    /// For B-roll checks: mediaRefs being added, paired with whether they
    /// already exist elsewhere on the timeline.
    pub added_media_refs: Vec<String>,
    /// Whether an added clip's destination track is muted (B-roll silence rule).
    pub added_track_muted: Option<bool>,
    /// Whether a split/trim point is known to fall mid-word (None = unknown).
    pub mid_word: Option<bool>,
}

/// Look up a track's role from the signal assignments.
fn role_of(track_index: usize, roles: &[TrackRoleAssignment]) -> Option<TrackRole> {
    roles
        .iter()
        .find(|a| a.track_index == track_index)
        .map(|a| a.role)
}

/// Run built-in rules for a tool + op, returning warnings (text verbatim from
/// `agent-SPEC.md` §6.6.1). `roles` are the detected track roles; `timeline` is
/// the post-op state for duplicate-media detection.
pub fn builtin_rules(
    tool: ToolName,
    op: &OpContext,
    roles: &[TrackRoleAssignment],
    timeline: &Timeline,
) -> Vec<String> {
    let mut warnings = Vec::new();

    // --- 口播精剪 (VoiceOver/MainCamera track on remove/split/trim) ---
    let on_voice_track = op
        .track_index
        .and_then(|i| role_of(i, roles))
        .map(|r| matches!(r, TrackRole::Voice | TrackRole::MainCamera))
        .unwrap_or(false);

    match tool {
        ToolName::SplitClip => {
            // 不在词中间切：only when we positively know it's mid-word.
            if op.mid_word == Some(true) {
                warnings.push("切点位于词中间，会导致漏字。请移到句界（语义完整处）。".to_string());
            } else if op.mid_word.is_none() && on_voice_track {
                // Unknown without word-level timestamps → soft reminder.
                warnings.push(
                    "该处为气口，请判断：保留(衔接不自然)/扩充(太急促)/叠化(去不掉时)".to_string(),
                );
            }
        }
        ToolName::RemoveClips => {
            if on_voice_track && !op.clip_ids.is_empty() {
                warnings
                    .push("该 clip 为主干内容，删除会破坏叙事。确认这是啰嗦/卡顿？".to_string());
            }
        }
        // --- B-roll 匹配 (add_clips / search_media) ---
        ToolName::AddClips => {
            // 不重复: an added mediaRef already present elsewhere on the timeline.
            for media_ref in &op.added_media_refs {
                let count = timeline
                    .tracks
                    .iter()
                    .flat_map(|t| t.clips.iter())
                    .filter(|c| &c.media_ref == media_ref)
                    .count();
                if count > 1 {
                    // First reuse frame for the message.
                    let frame = timeline
                        .tracks
                        .iter()
                        .flat_map(|t| t.clips.iter())
                        .filter(|c| &c.media_ref == media_ref)
                        .map(|c| c.start_frame)
                        .min()
                        .unwrap_or(0);
                    warnings.push(format!(
                        "该素材已于 frame {frame} 处使用。避免同一素材重复出现。"
                    ));
                }
            }
            // 静音: a B-roll clip added to a non-muted track.
            if op.added_track_muted == Some(false)
                && op
                    .track_index
                    .and_then(|i| role_of(i, roles))
                    .map(|r| r == TrackRole::BRoll)
                    .unwrap_or(false)
            {
                warnings.push("B-roll 通常无声，已自动静音该轨。".to_string());
            }
        }
        _ => {}
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{Clip, ClipType, Timeline, Track};

    fn roles_voice_at(index: usize) -> Vec<TrackRoleAssignment> {
        vec![TrackRoleAssignment {
            track_index: index,
            role: TrackRole::Voice,
        }]
    }

    #[test]
    fn split_mid_word_warns_verbatim() {
        let op = OpContext {
            mid_word: Some(true),
            ..Default::default()
        };
        let w = builtin_rules(ToolName::SplitClip, &op, &[], &Timeline::new());
        assert_eq!(
            w,
            vec!["切点位于词中间，会导致漏字。请移到句界（语义完整处）。"]
        );
    }

    #[test]
    fn split_unknown_on_voice_gives_breath_reminder() {
        let op = OpContext {
            track_index: Some(0),
            mid_word: None,
            ..Default::default()
        };
        let w = builtin_rules(
            ToolName::SplitClip,
            &op,
            &roles_voice_at(0),
            &Timeline::new(),
        );
        assert_eq!(
            w,
            vec!["该处为气口，请判断：保留(衔接不自然)/扩充(太急促)/叠化(去不掉时)"]
        );
    }

    #[test]
    fn remove_on_voice_track_warns_backbone() {
        let op = OpContext {
            track_index: Some(0),
            clip_ids: vec!["c1".into()],
            ..Default::default()
        };
        let w = builtin_rules(
            ToolName::RemoveClips,
            &op,
            &roles_voice_at(0),
            &Timeline::new(),
        );
        assert_eq!(
            w,
            vec!["该 clip 为主干内容，删除会破坏叙事。确认这是啰嗦/卡顿？"]
        );
    }

    #[test]
    fn add_duplicate_media_warns_with_frame() {
        let mut tl = Timeline::new();
        let mut t = Track::new("v1", ClipType::Video);
        t.clips.push(Clip::new("c1", "asset-x", 10, 30));
        t.clips.push(Clip::new("c2", "asset-x", 200, 30)); // same mediaRef
        tl.tracks.push(t);
        let op = OpContext {
            added_media_refs: vec!["asset-x".into()],
            ..Default::default()
        };
        let w = builtin_rules(ToolName::AddClips, &op, &[], &tl);
        assert_eq!(
            w,
            vec!["该素材已于 frame 10 处使用。避免同一素材重复出现。"]
        );
    }

    #[test]
    fn add_unique_media_no_warning() {
        let mut tl = Timeline::new();
        let mut t = Track::new("v1", ClipType::Video);
        t.clips.push(Clip::new("c1", "asset-y", 0, 30));
        tl.tracks.push(t);
        let op = OpContext {
            added_media_refs: vec!["asset-y".into()],
            ..Default::default()
        };
        let w = builtin_rules(ToolName::AddClips, &op, &[], &tl);
        assert!(w.is_empty());
    }

    #[test]
    fn broll_not_muted_warns_silence() {
        let roles = vec![TrackRoleAssignment {
            track_index: 1,
            role: TrackRole::BRoll,
        }];
        let op = OpContext {
            track_index: Some(1),
            added_track_muted: Some(false),
            ..Default::default()
        };
        let w = builtin_rules(ToolName::AddClips, &op, &roles, &Timeline::new());
        assert!(w.iter().any(|s| s.contains("B-roll 通常无声")));
    }

    #[test]
    fn non_write_tool_no_warnings() {
        let op = OpContext::default();
        assert!(builtin_rules(ToolName::GetTimeline, &op, &[], &Timeline::new()).is_empty());
    }
}
