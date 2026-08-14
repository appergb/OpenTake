//! Tool-name enum. The 31 upstream tools (`ToolDefinitions.swift:4-36`) plus
//! OpenTake workflow-plugin, analysis, effect, and motion-graphics additions.
//! String values are 1:1 with upstream where applicable; ordering matches
//! `ToolName`.

use std::str::FromStr;

/// Every tool the agent layer exposes. The `UPSTREAM` const pins the 31-tool
/// upstream-compatible set; `ALL` also includes OpenTake additions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolName {
    // --- Read / introspect (7) ---
    GetTimeline,
    GetMedia,
    InspectMedia,
    GetTranscript,
    InspectTimeline,
    SearchMedia,
    ListModels,
    // --- Timeline editing (11) ---
    AddClips,
    InsertClips,
    RemoveClips,
    RemoveTracks,
    MoveClips,
    SetClipProperties,
    SetKeyframes,
    SplitClip,
    RippleDeleteRanges,
    Undo,
    AddTexts,
    AddCaptions,
    DetectBeats,
    AutoCutToBeats,
    SmartReframe,
    TightenSilences,
    RemoveFillerWords,
    // --- Media generation / import (5) ---
    GenerateVideo,
    GenerateImage,
    GenerateAudio,
    UpscaleMedia,
    ImportMedia,
    // --- Media library organization (7) ---
    ListFolders,
    CreateFolder,
    MoveToFolder,
    RenameMedia,
    RenameFolder,
    DeleteMedia,
    DeleteFolder,
    // --- OpenTake workflow plugin (3, agent-SPEC §7.4) ---
    ActivateWorkflow,
    ListWorkflows,
    DeactivateWorkflow,
    // --- OpenTake A-tier shader effects (docs/ADVANCED-FEATURES.md A-layer) ---
    SetColorGrade,
    ChromaKey,
    SetMask,
    ApplyEffect,
    // --- OpenTake Motion Canvas graphics (docs/MOTION-GRAPHICS-PLUGIN.md, Issue #34) ---
    AddMotionGraphic,
    EditMotionGraphic,
    // --- Project-confined Motion Studio authoring ---
    ListMotionDocuments,
    ReadMotionDocument,
    CreateMotionDocument,
    PatchMotionDocument,
    PreviewMotionDocument,
    PublishMotionDocument,
    // --- Advanced AI workflows (capability-gated by the desktop host) ---
    TrackMotion,
    GenerateMatte,
    RemoveObject,
    MatchColor,
    SeparateStems,
    TranslateCaptions,
    ScriptToVideo,
    GenerateAvatar,
    CloneVoice,
}

impl ToolName {
    /// Whether discovery of this tool requires a live host media bridge.
    /// Keeping this predicate next to the catalog prevents MCP and in-app Chat
    /// from drifting into different fail-closed capability sets.
    pub const fn requires_media_bridge(self) -> bool {
        matches!(
            self,
            ToolName::InspectMedia
                | ToolName::GetTranscript
                | ToolName::InspectTimeline
                | ToolName::SearchMedia
                | ToolName::AddCaptions
                | ToolName::RemoveFillerWords
                | ToolName::ImportMedia
        )
    }

    /// Why a schema-known tool is deliberately hidden from discovery, or
    /// `None` when the tool is not capability-gated. The dispatch gate appends
    /// this to its fail-closed "not advertised" result so a model invoking a
    /// gated tool by name learns the missing backend instead of guessing.
    pub const fn hidden_capability_reason(self) -> Option<&'static str> {
        match self {
            ToolName::SmartReframe => Some("vision analysis backend is not available"),
            _ => None,
        }
    }

    /// The wire name (matches upstream / spec exactly).
    pub fn as_str(self) -> &'static str {
        match self {
            ToolName::GetTimeline => "get_timeline",
            ToolName::GetMedia => "get_media",
            ToolName::InspectMedia => "inspect_media",
            ToolName::GetTranscript => "get_transcript",
            ToolName::InspectTimeline => "inspect_timeline",
            ToolName::SearchMedia => "search_media",
            ToolName::ListModels => "list_models",
            ToolName::AddClips => "add_clips",
            ToolName::InsertClips => "insert_clips",
            ToolName::RemoveClips => "remove_clips",
            ToolName::RemoveTracks => "remove_tracks",
            ToolName::MoveClips => "move_clips",
            ToolName::SetClipProperties => "set_clip_properties",
            ToolName::SetKeyframes => "set_keyframes",
            ToolName::SplitClip => "split_clip",
            ToolName::RippleDeleteRanges => "ripple_delete_ranges",
            ToolName::Undo => "undo",
            ToolName::AddTexts => "add_texts",
            ToolName::AddCaptions => "add_captions",
            ToolName::DetectBeats => "detect_beats",
            ToolName::AutoCutToBeats => "auto_cut_to_beats",
            ToolName::SmartReframe => "smart_reframe",
            ToolName::TightenSilences => "tighten_silences",
            ToolName::RemoveFillerWords => "remove_filler_words",
            ToolName::GenerateVideo => "generate_video",
            ToolName::GenerateImage => "generate_image",
            ToolName::GenerateAudio => "generate_audio",
            ToolName::UpscaleMedia => "upscale_media",
            ToolName::ImportMedia => "import_media",
            ToolName::ListFolders => "list_folders",
            ToolName::CreateFolder => "create_folder",
            ToolName::MoveToFolder => "move_to_folder",
            ToolName::RenameMedia => "rename_media",
            ToolName::RenameFolder => "rename_folder",
            ToolName::DeleteMedia => "delete_media",
            ToolName::DeleteFolder => "delete_folder",
            ToolName::ActivateWorkflow => "activate_workflow",
            ToolName::ListWorkflows => "list_workflows",
            ToolName::DeactivateWorkflow => "deactivate_workflow",
            ToolName::SetColorGrade => "set_color_grade",
            ToolName::ChromaKey => "chroma_key",
            ToolName::SetMask => "set_mask",
            ToolName::ApplyEffect => "apply_effect",
            ToolName::AddMotionGraphic => "add_motion_graphic",
            ToolName::EditMotionGraphic => "edit_motion_graphic",
            ToolName::ListMotionDocuments => "list_motion_documents",
            ToolName::ReadMotionDocument => "read_motion_document",
            ToolName::CreateMotionDocument => "create_motion_document",
            ToolName::PatchMotionDocument => "patch_motion_document",
            ToolName::PreviewMotionDocument => "preview_motion_document",
            ToolName::PublishMotionDocument => "publish_motion_document",
            ToolName::TrackMotion => "track_motion",
            ToolName::GenerateMatte => "generate_matte",
            ToolName::RemoveObject => "remove_object",
            ToolName::MatchColor => "match_color",
            ToolName::SeparateStems => "separate_stems",
            ToolName::TranslateCaptions => "translate_captions",
            ToolName::ScriptToVideo => "script_to_video",
            ToolName::GenerateAvatar => "generate_avatar",
            ToolName::CloneVoice => "clone_voice",
        }
    }

    /// Base tools advertised to MCP and in-app Chat in registration order.
    /// Provider-backed generation, Motion, and vision-analysis tools are
    /// appended only when the current host reports their respective live
    /// capabilities.
    pub const ALL: [ToolName; 38] = [
        ToolName::GetTimeline,
        ToolName::GetMedia,
        ToolName::InspectMedia,
        ToolName::GetTranscript,
        ToolName::InspectTimeline,
        ToolName::SearchMedia,
        ToolName::ListModels,
        ToolName::AddClips,
        ToolName::InsertClips,
        ToolName::RemoveClips,
        ToolName::RemoveTracks,
        ToolName::MoveClips,
        ToolName::SetClipProperties,
        ToolName::SetKeyframes,
        ToolName::SplitClip,
        ToolName::RippleDeleteRanges,
        ToolName::Undo,
        ToolName::AddTexts,
        ToolName::AddCaptions,
        ToolName::DetectBeats,
        ToolName::AutoCutToBeats,
        ToolName::TightenSilences,
        ToolName::RemoveFillerWords,
        ToolName::ImportMedia,
        ToolName::ListFolders,
        ToolName::CreateFolder,
        ToolName::MoveToFolder,
        ToolName::RenameMedia,
        ToolName::RenameFolder,
        ToolName::DeleteMedia,
        ToolName::DeleteFolder,
        ToolName::ActivateWorkflow,
        ToolName::ListWorkflows,
        ToolName::DeactivateWorkflow,
        ToolName::SetColorGrade,
        ToolName::ChromaKey,
        ToolName::SetMask,
        ToolName::ApplyEffect,
    ];

    /// Provider-backed tools appended to a host catalog only while its live
    /// generation bridge reports usable authorization.
    pub const GENERATION: [ToolName; 4] = [
        ToolName::GenerateVideo,
        ToolName::GenerateImage,
        ToolName::GenerateAudio,
        ToolName::UpscaleMedia,
    ];

    /// Motion tools appended only by a host with a production render/import/
    /// placement bridge. They remain known for strict compatibility parsing in
    /// all other hosts.
    pub const MOTION: [ToolName; 2] = [ToolName::AddMotionGraphic, ToolName::EditMotionGraphic];

    /// Motion Studio document tools appended only while the host can capture
    /// current-project authority and execute the typed document bridge.
    pub const MOTION_DOCUMENTS: [ToolName; 6] = [
        ToolName::ListMotionDocuments,
        ToolName::ReadMotionDocument,
        ToolName::CreateMotionDocument,
        ToolName::PatchMotionDocument,
        ToolName::PreviewMotionDocument,
        ToolName::PublishMotionDocument,
    ];

    /// Vision-analysis tools appended only by a host with a live frame-sampling
    /// / saliency backend. They remain known for strict compatibility parsing
    /// in all other hosts.
    pub const VISION: [ToolName; 1] = [ToolName::SmartReframe];

    /// Advanced workflows are schema-known but never unconditionally
    /// advertised. The desktop host appends only the exact capabilities backed
    /// by installed local models or a configured provider.
    pub const ADVANCED_AI: [ToolName; 9] = [
        ToolName::TrackMotion,
        ToolName::GenerateMatte,
        ToolName::RemoveObject,
        ToolName::MatchColor,
        ToolName::SeparateStems,
        ToolName::TranslateCaptions,
        ToolName::ScriptToVideo,
        ToolName::GenerateAvatar,
        ToolName::CloneVoice,
    ];

    /// Every recognized schema/wire name, including capabilities deliberately
    /// hidden from discovery until a real backend exists. Keeping this set lets
    /// strict argument validation and compatibility tests cover future tools
    /// without advertising placeholder behavior to models.
    pub const KNOWN: [ToolName; 60] = [
        ToolName::GetTimeline,
        ToolName::GetMedia,
        ToolName::InspectMedia,
        ToolName::GetTranscript,
        ToolName::InspectTimeline,
        ToolName::SearchMedia,
        ToolName::ListModels,
        ToolName::AddClips,
        ToolName::InsertClips,
        ToolName::RemoveClips,
        ToolName::RemoveTracks,
        ToolName::MoveClips,
        ToolName::SetClipProperties,
        ToolName::SetKeyframes,
        ToolName::SplitClip,
        ToolName::RippleDeleteRanges,
        ToolName::Undo,
        ToolName::AddTexts,
        ToolName::AddCaptions,
        ToolName::DetectBeats,
        ToolName::AutoCutToBeats,
        ToolName::SmartReframe,
        ToolName::TightenSilences,
        ToolName::RemoveFillerWords,
        ToolName::GenerateVideo,
        ToolName::GenerateImage,
        ToolName::GenerateAudio,
        ToolName::UpscaleMedia,
        ToolName::ImportMedia,
        ToolName::ListFolders,
        ToolName::CreateFolder,
        ToolName::MoveToFolder,
        ToolName::RenameMedia,
        ToolName::RenameFolder,
        ToolName::DeleteMedia,
        ToolName::DeleteFolder,
        ToolName::ActivateWorkflow,
        ToolName::ListWorkflows,
        ToolName::DeactivateWorkflow,
        ToolName::SetColorGrade,
        ToolName::ChromaKey,
        ToolName::SetMask,
        ToolName::ApplyEffect,
        ToolName::AddMotionGraphic,
        ToolName::EditMotionGraphic,
        ToolName::ListMotionDocuments,
        ToolName::ReadMotionDocument,
        ToolName::CreateMotionDocument,
        ToolName::PatchMotionDocument,
        ToolName::PreviewMotionDocument,
        ToolName::PublishMotionDocument,
        ToolName::TrackMotion,
        ToolName::GenerateMatte,
        ToolName::RemoveObject,
        ToolName::MatchColor,
        ToolName::SeparateStems,
        ToolName::TranslateCaptions,
        ToolName::ScriptToVideo,
        ToolName::GenerateAvatar,
        ToolName::CloneVoice,
    ];

    /// The 31 upstream-equivalent tools (Issue #9's "31 tools").
    pub const UPSTREAM: [ToolName; 31] = [
        ToolName::GetTimeline,
        ToolName::GetMedia,
        ToolName::AddClips,
        ToolName::InsertClips,
        ToolName::RemoveClips,
        ToolName::RemoveTracks,
        ToolName::MoveClips,
        ToolName::SetClipProperties,
        ToolName::SetKeyframes,
        ToolName::SplitClip,
        ToolName::RippleDeleteRanges,
        ToolName::Undo,
        ToolName::AddTexts,
        ToolName::AddCaptions,
        ToolName::GenerateVideo,
        ToolName::GenerateImage,
        ToolName::GenerateAudio,
        ToolName::UpscaleMedia,
        ToolName::ImportMedia,
        ToolName::ListModels,
        ToolName::InspectMedia,
        ToolName::GetTranscript,
        ToolName::InspectTimeline,
        ToolName::SearchMedia,
        ToolName::ListFolders,
        ToolName::CreateFolder,
        ToolName::MoveToFolder,
        ToolName::RenameMedia,
        ToolName::RenameFolder,
        ToolName::DeleteMedia,
        ToolName::DeleteFolder,
    ];
}

impl FromStr for ToolName {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        ToolName::KNOWN
            .iter()
            .copied()
            .find(|t| t.as_str() == s)
            .ok_or(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_set_is_31() {
        assert_eq!(ToolName::UPSTREAM.len(), 31);
    }

    #[test]
    fn advertised_set_is_38_and_known_set_is_60() {
        assert_eq!(ToolName::ALL.len(), 38);
        assert_eq!(ToolName::KNOWN.len(), 60);
        assert!(ToolName::ALL
            .iter()
            .all(|tool| ToolName::KNOWN.contains(tool)));
    }

    #[test]
    fn advanced_ai_tools_are_known_but_capability_gated() {
        for tool in ToolName::ADVANCED_AI {
            assert_eq!(ToolName::from_str(tool.as_str()), Ok(tool));
            assert!(ToolName::KNOWN.contains(&tool));
            assert!(!ToolName::ALL.contains(&tool));
            assert!(!ToolName::UPSTREAM.contains(&tool));
        }
    }

    #[test]
    fn vision_tools_are_known_but_capability_gated() {
        assert_eq!(ToolName::VISION, [ToolName::SmartReframe]);
        for tool in ToolName::VISION {
            assert_eq!(ToolName::from_str(tool.as_str()), Ok(tool));
            assert!(ToolName::KNOWN.contains(&tool));
            assert!(!ToolName::ALL.contains(&tool));
            assert!(!ToolName::UPSTREAM.contains(&tool));
            assert_eq!(
                tool.hidden_capability_reason(),
                Some("vision analysis backend is not available")
            );
        }
    }

    #[test]
    fn ungated_tools_have_no_hidden_capability_reason() {
        for tool in ToolName::ALL {
            assert_eq!(
                tool.hidden_capability_reason(),
                None,
                "{} is advertised but reports a hidden capability",
                tool.as_str()
            );
        }
    }

    #[test]
    fn analysis_tools_have_expected_wire_names() {
        assert_eq!(ToolName::DetectBeats.as_str(), "detect_beats");
        assert_eq!(ToolName::AutoCutToBeats.as_str(), "auto_cut_to_beats");
        assert_eq!(ToolName::SmartReframe.as_str(), "smart_reframe");
        assert_eq!(ToolName::TightenSilences.as_str(), "tighten_silences");
        assert_eq!(ToolName::RemoveFillerWords.as_str(), "remove_filler_words");
        for t in [
            ToolName::DetectBeats,
            ToolName::AutoCutToBeats,
            ToolName::SmartReframe,
            ToolName::TightenSilences,
            ToolName::RemoveFillerWords,
        ] {
            assert_eq!(ToolName::from_str(t.as_str()), Ok(t));
            assert!(!ToolName::UPSTREAM.contains(&t));
        }
    }

    #[test]
    fn motion_graphic_tools_have_expected_wire_names() {
        assert_eq!(ToolName::AddMotionGraphic.as_str(), "add_motion_graphic");
        assert_eq!(ToolName::EditMotionGraphic.as_str(), "edit_motion_graphic");
        // And they round-trip through FromStr.
        for t in [ToolName::AddMotionGraphic, ToolName::EditMotionGraphic] {
            assert_eq!(ToolName::from_str(t.as_str()), Ok(t));
        }
        // They stay out of the unconditional base catalog: a capable desktop
        // host appends MOTION, while non-rendering hosts remain fail-closed.
        assert_eq!(
            ToolName::KNOWN
                .iter()
                .filter(|t| matches!(t, ToolName::AddMotionGraphic | ToolName::EditMotionGraphic))
                .count(),
            2
        );
        assert!(!ToolName::ALL.contains(&ToolName::AddMotionGraphic));
        assert!(!ToolName::ALL.contains(&ToolName::EditMotionGraphic));
        // ...and are NOT part of the 31 upstream tools.
        assert!(!ToolName::UPSTREAM.contains(&ToolName::AddMotionGraphic));
        assert!(!ToolName::UPSTREAM.contains(&ToolName::EditMotionGraphic));
    }

    #[test]
    fn motion_document_tools_have_expected_wire_names() {
        let expected = [
            (ToolName::ListMotionDocuments, "list_motion_documents"),
            (ToolName::ReadMotionDocument, "read_motion_document"),
            (ToolName::CreateMotionDocument, "create_motion_document"),
            (ToolName::PatchMotionDocument, "patch_motion_document"),
            (ToolName::PreviewMotionDocument, "preview_motion_document"),
            (ToolName::PublishMotionDocument, "publish_motion_document"),
        ];
        for (tool, wire) in expected {
            assert_eq!(tool.as_str(), wire);
            assert_eq!(ToolName::from_str(wire), Ok(tool));
            assert!(!ToolName::ALL.contains(&tool));
            assert!(!ToolName::UPSTREAM.contains(&tool));
        }
    }

    #[test]
    fn a_tier_effect_tools_have_expected_wire_names() {
        assert_eq!(ToolName::SetColorGrade.as_str(), "set_color_grade");
        assert_eq!(ToolName::ChromaKey.as_str(), "chroma_key");
        assert_eq!(ToolName::SetMask.as_str(), "set_mask");
        assert_eq!(ToolName::ApplyEffect.as_str(), "apply_effect");
        // And they round-trip through FromStr.
        for t in [
            ToolName::SetColorGrade,
            ToolName::ChromaKey,
            ToolName::SetMask,
            ToolName::ApplyEffect,
        ] {
            assert_eq!(ToolName::from_str(t.as_str()), Ok(t));
        }
    }

    #[test]
    fn roundtrip_str() {
        for t in ToolName::ALL {
            assert_eq!(ToolName::from_str(t.as_str()), Ok(t));
        }
    }

    #[test]
    fn unknown_tool_errors() {
        assert_eq!(ToolName::from_str("not_a_tool"), Err(()));
    }

    #[test]
    fn names_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for t in ToolName::ALL {
            assert!(seen.insert(t.as_str()), "duplicate {}", t.as_str());
        }
    }
}
