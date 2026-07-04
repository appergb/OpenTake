//! Agent context-signal data types (Phase A). New pure-data types defined per
//! `docs/AGENT-CONTEXT-SIGNAL.md` §1.2 — not a port of upstream Swift.
//!
//! These are emitted by the MCP server alongside tool results to steer the agent.
//! This crate only defines the shapes + serde; detection/population lands in the
//! agent layer (Phase B–D). Wire forms follow the doc's examples verbatim:
//! `video_type` is snake_case (`"talking_head"`), `editing_stage` is PascalCase
//! (`"RoughCut"`), and roles keep the doc's spellings (`"MainCamera"`,
//! `"B_Roll"`, `"BGM"`, `"SFX"`).

use serde::{Deserialize, Serialize};

/// Detected video category. Wire form is snake_case (see §1.2 example).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoType {
    TalkingHead,
    Vlog,
    Montage,
    Interview,
    ShortForm,
    LongForm,
}

/// Role a track plays in the edit. Spellings match §1.2 exactly.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum TrackRole {
    MainCamera,
    #[serde(rename = "B_Roll")]
    BRoll,
    Voice,
    #[serde(rename = "BGM")]
    Bgm,
    #[serde(rename = "SFX")]
    Sfx,
    Text,
    Caption,
}

/// Current edit phase. Wire form is PascalCase (see §1.2 example `"RoughCut"`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum EditingStage {
    Importing,
    Classifying,
    RoughCut,
    BRollOverlay,
    AudioPolish,
    ColorGrade,
    ExportReady,
}

/// Assignment of a role to a track index.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct TrackRoleAssignment {
    pub track_index: usize,
    pub role: TrackRole,
}

/// Guidance for the current stage: what to do next and what to avoid.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct StageGuidance {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub next_actions: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// Editing skeleton for the detected video type. `approach` is free text (e.g.
/// `"audio_driven"`, `"montage_beat"`) since the doc lists several values.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct EditingSkeleton {
    #[serde(default)]
    pub approach: String,
    #[serde(default)]
    pub flow: Vec<String>,
    #[serde(default)]
    pub rules: Vec<String>,
}

/// Per-track operating hint surfaced to the agent.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct TrackHint {
    pub track_index: usize,
    pub role: TrackRole,
    pub advice: String,
}

/// The full context signal attached to MCP tool results. 1:1 with §1.2.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ContextSignal {
    pub video_type: VideoType,
    pub confidence: f64,
    #[serde(default)]
    pub track_roles: Vec<TrackRoleAssignment>,
    pub editing_stage: EditingStage,
    #[serde(default)]
    pub stage_guidance: StageGuidance,
    #[serde(default)]
    pub editing_skeleton: EditingSkeleton,
    #[serde(default)]
    pub track_hints: Vec<TrackHint>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_type_snake_case_wire() {
        assert_eq!(
            serde_json::to_string(&VideoType::TalkingHead).unwrap(),
            "\"talking_head\""
        );
        assert_eq!(
            serde_json::to_string(&VideoType::ShortForm).unwrap(),
            "\"short_form\""
        );
    }

    #[test]
    fn track_role_spellings_match_doc() {
        assert_eq!(
            serde_json::to_string(&TrackRole::MainCamera).unwrap(),
            "\"MainCamera\""
        );
        assert_eq!(
            serde_json::to_string(&TrackRole::BRoll).unwrap(),
            "\"B_Roll\""
        );
        assert_eq!(serde_json::to_string(&TrackRole::Bgm).unwrap(), "\"BGM\"");
        assert_eq!(serde_json::to_string(&TrackRole::Sfx).unwrap(), "\"SFX\"");
        assert_eq!(
            serde_json::to_string(&TrackRole::Voice).unwrap(),
            "\"Voice\""
        );
    }

    #[test]
    fn editing_stage_pascal_case_wire() {
        assert_eq!(
            serde_json::to_string(&EditingStage::RoughCut).unwrap(),
            "\"RoughCut\""
        );
        assert_eq!(
            serde_json::to_string(&EditingStage::ExportReady).unwrap(),
            "\"ExportReady\""
        );
    }

    #[test]
    fn roles_roundtrip() {
        for r in [
            TrackRole::MainCamera,
            TrackRole::BRoll,
            TrackRole::Voice,
            TrackRole::Bgm,
            TrackRole::Sfx,
            TrackRole::Text,
            TrackRole::Caption,
        ] {
            let json = serde_json::to_string(&r).unwrap();
            let back: TrackRole = serde_json::from_str(&json).unwrap();
            assert_eq!(r, back);
        }
    }

    #[test]
    fn context_signal_full_roundtrip() {
        let sig = ContextSignal {
            video_type: VideoType::TalkingHead,
            confidence: 0.9,
            track_roles: vec![
                TrackRoleAssignment {
                    track_index: 0,
                    role: TrackRole::MainCamera,
                },
                TrackRoleAssignment {
                    track_index: 1,
                    role: TrackRole::BRoll,
                },
            ],
            editing_stage: EditingStage::RoughCut,
            stage_guidance: StageGuidance {
                description: "rough cut".into(),
                next_actions: vec!["mark breaths".into()],
                warnings: vec!["don't cut mid-word".into()],
            },
            editing_skeleton: EditingSkeleton {
                approach: "audio_driven".into(),
                flow: vec!["extract audio".into(), "transcribe".into()],
                rules: vec![],
            },
            track_hints: vec![TrackHint {
                track_index: 0,
                role: TrackRole::MainCamera,
                advice: "A-roll main line".into(),
            }],
        };
        let json = serde_json::to_string(&sig).unwrap();
        assert!(json.contains("\"video_type\":\"talking_head\""));
        assert!(json.contains("\"editing_stage\":\"RoughCut\""));
        assert!(json.contains("\"approach\":\"audio_driven\""));
        let back: ContextSignal = serde_json::from_str(&json).unwrap();
        assert_eq!(sig, back);
    }

    #[test]
    fn context_signal_minimal_decode_uses_defaults() {
        // Only the required fields; collections/guidance default to empty.
        let json = r#"{"video_type":"montage","confidence":0.85,"editing_stage":"Importing"}"#;
        let sig: ContextSignal = serde_json::from_str(json).unwrap();
        assert_eq!(sig.video_type, VideoType::Montage);
        assert_eq!(sig.editing_stage, EditingStage::Importing);
        assert!(sig.track_roles.is_empty());
        assert!(sig.track_hints.is_empty());
        assert_eq!(sig.stage_guidance, StageGuidance::default());
        assert_eq!(sig.editing_skeleton, EditingSkeleton::default());
    }
}
