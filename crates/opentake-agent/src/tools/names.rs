//! Tool-name enum. The 31 upstream tools (`ToolDefinitions.swift:4-36`) plus
//! the OpenTake workflow-plugin tools (`agent-SPEC.md` §7.4). String values are
//! 1:1 with upstream; ordering matches `ToolName`.

use std::str::FromStr;

/// Every tool the agent layer exposes. The first 31 are the upstream
/// ToolExecutor set; the last three are OpenTake's workflow-plugin additions.
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
    // --- OpenTake Web motion graphics (docs/MOTION-GRAPHICS-PLUGIN.md, Issue #14) ---
    AddMotionGraphic,
    EditMotionGraphic,
}

impl ToolName {
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
        }
    }

    /// All tools in registration order.
    pub const ALL: [ToolName; 40] = [
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
        ToolName::ALL
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
    fn all_set_is_40() {
        assert_eq!(ToolName::ALL.len(), 40);
    }

    #[test]
    fn motion_graphic_tools_have_expected_wire_names() {
        assert_eq!(ToolName::AddMotionGraphic.as_str(), "add_motion_graphic");
        assert_eq!(ToolName::EditMotionGraphic.as_str(), "edit_motion_graphic");
        // And they round-trip through FromStr.
        for t in [ToolName::AddMotionGraphic, ToolName::EditMotionGraphic] {
            assert_eq!(ToolName::from_str(t.as_str()), Ok(t));
        }
        // They are present in ALL exactly once each.
        assert_eq!(
            ToolName::ALL
                .iter()
                .filter(|t| matches!(t, ToolName::AddMotionGraphic | ToolName::EditMotionGraphic))
                .count(),
            2
        );
        // ...and are NOT part of the 31 upstream tools.
        assert!(!ToolName::UPSTREAM.contains(&ToolName::AddMotionGraphic));
        assert!(!ToolName::UPSTREAM.contains(&ToolName::EditMotionGraphic));
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
