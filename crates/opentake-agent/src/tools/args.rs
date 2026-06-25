//! Strongly-typed tool argument structs + their `ALLOWED_KEYS` (`agent-SPEC.md`
//! §4). Each maps a tool's JSON args to a Rust struct; `Option<T>` = optional,
//! `Vec<T>` = array. The `ALLOWED_KEYS` lists drive the unknown-field guard
//! (`tools::errors`), which also runs per-entry for nested arrays — mirroring
//! upstream `DecodableToolArgs.allowedKeys` (JSON Schema can't reach nested
//! entry keys).

use serde::Deserialize;

use crate::tools::errors::ToolArgs;

// --- Shared sub-structs ---

/// A partial transform (all fields optional; partial-merge semantics live in
/// `opentake-ops` `apply_property_changes`). Mirrors the `transform` object on
/// `set_clip_properties` / `add_texts`.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TransformArg {
    pub center_x: Option<f64>,
    pub center_y: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub flip_horizontal: Option<bool>,
    pub flip_vertical: Option<bool>,
}

// --- get_timeline ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GetTimelineArgs {
    pub start_frame: Option<i32>,
    pub end_frame: Option<i32>,
}
impl ToolArgs for GetTimelineArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["startFrame", "endFrame"];
}

// --- add_clips ---
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AddClipEntry {
    pub media_ref: String,
    pub track_index: Option<usize>,
    pub start_frame: i32,
    pub duration_frames: i32,
    pub trim_start_frame: Option<i32>,
    pub trim_end_frame: Option<i32>,
}
impl ToolArgs for AddClipEntry {
    const ALLOWED_KEYS: &'static [&'static str] = &[
        "mediaRef",
        "trackIndex",
        "startFrame",
        "durationFrames",
        "trimStartFrame",
        "trimEndFrame",
    ];
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AddClipsArgs {
    pub entries: Vec<serde_json::Value>,
}
impl ToolArgs for AddClipsArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["entries"];
}

// --- insert_clips ---
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InsertClipEntry {
    pub media_ref: String,
    pub duration_frames: Option<i32>,
    pub trim_start_frame: Option<i32>,
    pub trim_end_frame: Option<i32>,
}
impl ToolArgs for InsertClipEntry {
    const ALLOWED_KEYS: &'static [&'static str] = &[
        "mediaRef",
        "durationFrames",
        "trimStartFrame",
        "trimEndFrame",
    ];
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InsertClipsArgs {
    pub track_index: usize,
    pub at_frame: i32,
    pub entries: Vec<serde_json::Value>,
}
impl ToolArgs for InsertClipsArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["trackIndex", "atFrame", "entries"];
}

// --- remove_clips ---
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoveClipsArgs {
    pub clip_ids: Vec<String>,
}
impl ToolArgs for RemoveClipsArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["clipIds"];
}

// --- remove_tracks ---
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoveTracksArgs {
    pub track_indexes: Vec<usize>,
}
impl ToolArgs for RemoveTracksArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["trackIndexes"];
}

// --- move_clips ---
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MoveEntry {
    pub clip_id: String,
    pub to_track: Option<usize>,
    pub to_frame: Option<i32>,
}
impl ToolArgs for MoveEntry {
    const ALLOWED_KEYS: &'static [&'static str] = &["clipId", "toTrack", "toFrame"];
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct MoveClipsArgs {
    pub moves: Vec<serde_json::Value>,
}
impl ToolArgs for MoveClipsArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["moves"];
}

// --- set_clip_properties ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SetClipPropertiesArgs {
    pub clip_ids: Vec<String>,
    pub duration_frames: Option<i32>,
    pub trim_start_frame: Option<i32>,
    pub trim_end_frame: Option<i32>,
    pub speed: Option<f64>,
    pub volume: Option<f64>,
    pub opacity: Option<f64>,
    pub transform: Option<TransformArg>,
    pub content: Option<String>,
    pub font_name: Option<String>,
    pub font_size: Option<f64>,
    pub color: Option<String>,
    pub alignment: Option<String>,
}
impl ToolArgs for SetClipPropertiesArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &[
        "clipIds",
        "durationFrames",
        "trimStartFrame",
        "trimEndFrame",
        "speed",
        "volume",
        "opacity",
        "transform",
        "content",
        "fontName",
        "fontSize",
        "color",
        "alignment",
    ];
}

// --- set_keyframes ---
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SetKeyframesArgs {
    pub clip_id: String,
    pub property: String,
    /// Rows are `[frame, ...values, interp?]`; kept as raw JSON so the dispatch
    /// layer can build the right `KeyframePayload` per property.
    pub keyframes: Vec<serde_json::Value>,
}
impl ToolArgs for SetKeyframesArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["clipId", "property", "keyframes"];
}

// --- split_clip ---
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitClipArgs {
    pub clip_id: String,
    pub at_frame: i32,
}
impl ToolArgs for SplitClipArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["clipId", "atFrame"];
}

// --- ripple_delete_ranges ---
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RippleDeleteRangesArgs {
    pub track_index: Option<usize>,
    pub clip_id: Option<String>,
    pub ranges: Vec<Vec<f64>>,
    pub units: Option<String>,
}
impl ToolArgs for RippleDeleteRangesArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["trackIndex", "clipId", "ranges", "units"];
}

// --- add_texts ---
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AddTextEntry {
    pub track_index: Option<usize>,
    pub start_frame: i32,
    pub duration_frames: i32,
    pub content: String,
    pub transform: Option<TransformArg>,
    pub font_name: Option<String>,
    pub font_size: Option<f64>,
    pub color: Option<String>,
    pub alignment: Option<String>,
}
impl ToolArgs for AddTextEntry {
    const ALLOWED_KEYS: &'static [&'static str] = &[
        "trackIndex",
        "startFrame",
        "durationFrames",
        "content",
        "transform",
        "fontName",
        "fontSize",
        "color",
        "alignment",
    ];
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AddTextsArgs {
    pub entries: Vec<serde_json::Value>,
}
impl ToolArgs for AddTextsArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["entries"];
}

// --- create_folder ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreateFolderEntry {
    pub name: String,
    pub parent_folder_id: Option<String>,
}
impl ToolArgs for CreateFolderEntry {
    const ALLOWED_KEYS: &'static [&'static str] = &["name", "parentFolderId"];
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreateFolderArgs {
    pub name: Option<String>,
    pub parent_folder_id: Option<String>,
    pub entries: Option<Vec<serde_json::Value>>,
}
impl ToolArgs for CreateFolderArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["name", "parentFolderId", "entries"];
}

// --- move_to_folder ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MoveToFolderEntry {
    pub asset_ids: Vec<String>,
    pub folder_id: Option<String>,
}
impl ToolArgs for MoveToFolderEntry {
    const ALLOWED_KEYS: &'static [&'static str] = &["assetIds", "folderId"];
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MoveToFolderArgs {
    pub asset_ids: Option<Vec<String>>,
    pub folder_id: Option<String>,
    pub entries: Option<Vec<serde_json::Value>>,
}
impl ToolArgs for MoveToFolderArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["assetIds", "folderId", "entries"];
}

// --- activate_workflow ---
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActivateWorkflowArgs {
    pub workflow_id: String,
}
impl ToolArgs for ActivateWorkflowArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["workflowId"];
}

// --- A-tier shader effects ---

/// A `{r, g, b}` color-wheel triple used by `set_color_grade`'s lift/gamma/gain.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RgbArg {
    pub r: Option<f64>,
    pub g: Option<f64>,
    pub b: Option<f64>,
}
impl ToolArgs for RgbArg {
    const ALLOWED_KEYS: &'static [&'static str] = &["r", "g", "b"];
}

// --- set_color_grade ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SetColorGradeArgs {
    pub clip_ids: Vec<String>,
    pub exposure: Option<f64>,
    pub temperature: Option<f64>,
    pub tint: Option<f64>,
    pub lift: Option<RgbArg>,
    pub gamma: Option<RgbArg>,
    pub gain: Option<RgbArg>,
    pub contrast: Option<f64>,
    pub saturation: Option<f64>,
    pub clear: Option<bool>,
}
impl ToolArgs for SetColorGradeArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &[
        "clipIds",
        "exposure",
        "temperature",
        "tint",
        "lift",
        "gamma",
        "gain",
        "contrast",
        "saturation",
        "clear",
    ];
}

// --- chroma_key ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChromaKeyArgs {
    pub clip_ids: Vec<String>,
    pub key_color: Option<String>,
    pub similarity: Option<f64>,
    pub smoothness: Option<f64>,
    pub spill: Option<f64>,
    pub clear: Option<bool>,
}
impl ToolArgs for ChromaKeyArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &[
        "clipIds",
        "keyColor",
        "similarity",
        "smoothness",
        "spill",
        "clear",
    ];
}

// --- set_mask ---
/// A `{x, y}` point in normalized canvas space, used by mask geometry.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Point2Arg {
    pub x: Option<f64>,
    pub y: Option<f64>,
}
impl ToolArgs for Point2Arg {
    const ALLOWED_KEYS: &'static [&'static str] = &["x", "y"];
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MaskArg {
    pub kind: String,
    pub point: Option<Point2Arg>,
    pub normal: Option<Point2Arg>,
    pub center: Option<Point2Arg>,
    pub radius: Option<Point2Arg>,
    pub points: Option<Vec<Point2Arg>>,
    pub feather: Option<f64>,
    pub invert: Option<bool>,
}
impl ToolArgs for MaskArg {
    const ALLOWED_KEYS: &'static [&'static str] = &[
        "kind", "point", "normal", "center", "radius", "points", "feather", "invert",
    ];
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SetMaskArgs {
    pub clip_ids: Vec<String>,
    /// Kept as raw JSON so the dispatch layer can decode each entry with the
    /// per-entry unknown-key guard (`MaskArg`), mirroring the `entries[]` pattern.
    pub masks: Vec<serde_json::Value>,
}
impl ToolArgs for SetMaskArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["clipIds", "masks"];
}

// --- apply_effect ---
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EffectArg {
    pub name: String,
    pub params: Option<std::collections::BTreeMap<String, f64>>,
    pub enabled: Option<bool>,
}
impl ToolArgs for EffectArg {
    const ALLOWED_KEYS: &'static [&'static str] = &["name", "params", "enabled"];
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApplyEffectArgs {
    pub clip_ids: Vec<String>,
    /// Raw JSON per the `entries[]` pattern (per-entry guard via `EffectArg`).
    pub effects: Vec<serde_json::Value>,
}
impl ToolArgs for ApplyEffectArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["clipIds", "effects"];
}

// --- Web motion graphics (docs/MOTION-GRAPHICS-PLUGIN.md, Issue #14) ---

/// The `source` object on `add_motion_graphic` — exactly one of `code` or
/// `templateId` set (mutual-exclusion is a business-level guard in the motion
/// dispatch path, like `import_media`'s source). `params` only applies with a
/// template; values are kept as raw JSON so the dispatch layer converts them to
/// `opentake_motion::ParamValue` without coupling the tool layer to that crate.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionSourceArg {
    pub code: Option<String>,
    pub template_id: Option<String>,
    pub params: Option<serde_json::Map<String, serde_json::Value>>,
}
impl ToolArgs for MotionSourceArg {
    const ALLOWED_KEYS: &'static [&'static str] = &["code", "templateId", "params"];
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AddMotionGraphicArgs {
    /// Kept as raw JSON so the dispatch layer decodes it with the per-object
    /// unknown-key guard (`MotionSourceArg`), mirroring `import_media`'s `source`.
    pub source: serde_json::Value,
    pub start_frame: i32,
    pub duration_frames: i32,
    pub transparent: Option<bool>,
    pub track_index: Option<usize>,
}
impl ToolArgs for AddMotionGraphicArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &[
        "source",
        "startFrame",
        "durationFrames",
        "transparent",
        "trackIndex",
    ];
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditMotionGraphicArgs {
    pub clip_id: String,
    pub code: Option<String>,
    pub params: Option<serde_json::Map<String, serde_json::Value>>,
}
impl ToolArgs for EditMotionGraphicArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["clipId", "code", "params"];
}

// --- inspect_media ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InspectMediaArgs {
    pub media_ref: String,
    pub clip_id: Option<String>,
    pub max_frames: Option<i32>,
    pub start_seconds: Option<f64>,
    pub end_seconds: Option<f64>,
    pub word_timestamps: Option<bool>,
    pub overview: Option<bool>,
}
impl ToolArgs for InspectMediaArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &[
        "mediaRef",
        "clipId",
        "maxFrames",
        "startSeconds",
        "endSeconds",
        "wordTimestamps",
        "overview",
    ];
}

// --- get_transcript ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GetTranscriptArgs {
    pub start_frame: Option<i32>,
    pub end_frame: Option<i32>,
    pub clip_id: Option<String>,
}
impl ToolArgs for GetTranscriptArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["startFrame", "endFrame", "clipId"];
}

// --- inspect_timeline ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InspectTimelineArgs {
    pub start_frame: Option<i32>,
    pub end_frame: Option<i32>,
    pub max_frames: Option<i32>,
}
impl ToolArgs for InspectTimelineArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["startFrame", "endFrame", "maxFrames"];
}

// --- search_media ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchMediaArgs {
    pub query: String,
    pub scope: Option<String>,
    pub media_ref: Option<String>,
    pub limit: Option<i32>,
}
impl ToolArgs for SearchMediaArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["query", "scope", "mediaRef", "limit"];
}

// --- list_models ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListModelsArgs {
    #[serde(rename = "type")]
    pub kind: Option<String>,
}
impl ToolArgs for ListModelsArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["type"];
}

// --- add_captions ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AddCaptionsArgs {
    pub clip_ids: Option<Vec<String>>,
    pub language: Option<String>,
    pub font_name: Option<String>,
    pub font_size: Option<f64>,
    pub color: Option<String>,
    pub center_x: Option<f64>,
    pub center_y: Option<f64>,
    pub text_case: Option<String>,
    pub censor_profanity: Option<bool>,
}
impl ToolArgs for AddCaptionsArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &[
        "clipIds",
        "language",
        "fontName",
        "fontSize",
        "color",
        "centerX",
        "centerY",
        "textCase",
        "censorProfanity",
    ];
}

// --- generate_video ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GenerateVideoArgs {
    pub prompt: String,
    pub name: Option<String>,
    pub model: Option<String>,
    pub duration: Option<i32>,
    pub aspect_ratio: Option<String>,
    pub resolution: Option<String>,
    pub start_frame_media_ref: Option<String>,
    pub end_frame_media_ref: Option<String>,
    pub source_video_media_ref: Option<String>,
    pub source_clip_id: Option<String>,
    pub reference_image_media_refs: Option<Vec<String>>,
    pub reference_video_media_refs: Option<Vec<String>>,
    pub reference_audio_media_refs: Option<Vec<String>>,
    pub folder_id: Option<String>,
}
impl ToolArgs for GenerateVideoArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &[
        "prompt",
        "name",
        "model",
        "duration",
        "aspectRatio",
        "resolution",
        "startFrameMediaRef",
        "endFrameMediaRef",
        "sourceVideoMediaRef",
        "sourceClipId",
        "referenceImageMediaRefs",
        "referenceVideoMediaRefs",
        "referenceAudioMediaRefs",
        "folderId",
    ];
}

// --- generate_image ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GenerateImageArgs {
    pub prompt: String,
    pub name: Option<String>,
    pub model: Option<String>,
    pub aspect_ratio: Option<String>,
    pub resolution: Option<String>,
    pub quality: Option<String>,
    pub reference_media_refs: Option<Vec<String>>,
    pub folder_id: Option<String>,
}
impl ToolArgs for GenerateImageArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &[
        "prompt",
        "name",
        "model",
        "aspectRatio",
        "resolution",
        "quality",
        "referenceMediaRefs",
        "folderId",
    ];
}

// --- generate_audio ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GenerateAudioArgs {
    pub prompt: Option<String>,
    pub name: Option<String>,
    pub model: Option<String>,
    pub voice: Option<String>,
    pub lyrics: Option<String>,
    pub style_instructions: Option<String>,
    pub instrumental: Option<bool>,
    pub duration: Option<i32>,
    pub video_source_start_frame: Option<i32>,
    pub video_source_end_frame: Option<i32>,
    pub video_source_media_ref: Option<String>,
    pub folder_id: Option<String>,
}
impl ToolArgs for GenerateAudioArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &[
        "prompt",
        "name",
        "model",
        "voice",
        "lyrics",
        "styleInstructions",
        "instrumental",
        "duration",
        "videoSourceStartFrame",
        "videoSourceEndFrame",
        "videoSourceMediaRef",
        "folderId",
    ];
}

// --- upscale_media ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpscaleMediaArgs {
    pub media_ref: String,
    pub model: Option<String>,
    pub source_clip_id: Option<String>,
}
impl ToolArgs for UpscaleMediaArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["mediaRef", "model", "sourceClipId"];
}

// --- import_media ---
/// The `source` object on `import_media` — exactly one of url/path/bytes set
/// (mutual-exclusion is a business-level guard in the import path, not a decode
/// concern). `mimeType` is required when `bytes` is set.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportSourceArg {
    pub url: Option<String>,
    pub path: Option<String>,
    pub bytes: Option<String>,
    pub mime_type: Option<String>,
}
impl ToolArgs for ImportSourceArg {
    const ALLOWED_KEYS: &'static [&'static str] = &["url", "path", "bytes", "mimeType"];
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportMediaArgs {
    pub source: ImportSourceArg,
    pub name: Option<String>,
    pub folder_id: Option<String>,
}
impl ToolArgs for ImportMediaArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["source", "name", "folderId"];
}

// --- rename_media (single + batch entry) ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenameMediaEntry {
    pub media_ref: String,
    pub name: String,
}
impl ToolArgs for RenameMediaEntry {
    const ALLOWED_KEYS: &'static [&'static str] = &["mediaRef", "name"];
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenameMediaArgs {
    pub media_ref: Option<String>,
    pub name: Option<String>,
    pub entries: Option<Vec<serde_json::Value>>,
}
impl ToolArgs for RenameMediaArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["mediaRef", "name", "entries"];
}

// --- rename_folder (single + batch entry) ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenameFolderEntry {
    pub folder_id: String,
    pub name: String,
}
impl ToolArgs for RenameFolderEntry {
    const ALLOWED_KEYS: &'static [&'static str] = &["folderId", "name"];
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenameFolderArgs {
    pub folder_id: Option<String>,
    pub name: Option<String>,
    pub entries: Option<Vec<serde_json::Value>>,
}
impl ToolArgs for RenameFolderArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["folderId", "name", "entries"];
}

// --- delete_media / delete_folder ---
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteMediaArgs {
    pub asset_ids: Vec<String>,
}
impl ToolArgs for DeleteMediaArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["assetIds"];
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFolderArgs {
    pub folder_ids: Vec<String>,
}
impl ToolArgs for DeleteFolderArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["folderIds"];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::errors::decode_tool_args;

    #[test]
    fn add_clip_entry_decodes_camel_case() {
        let v = serde_json::json!({
            "mediaRef": "m", "trackIndex": 1, "startFrame": 0,
            "durationFrames": 30, "trimStartFrame": 5
        });
        let e: AddClipEntry = decode_tool_args(&v, "entries[0]").unwrap();
        assert_eq!(e.media_ref, "m");
        assert_eq!(e.track_index, Some(1));
        assert_eq!(e.trim_start_frame, Some(5));
        assert_eq!(e.trim_end_frame, None);
    }

    #[test]
    fn add_clip_entry_rejects_unknown_nested_key() {
        let v = serde_json::json!({
            "mediaRef": "m", "startFrame": 0, "durationFrames": 30, "speed": 2.0
        });
        let err = decode_tool_args::<AddClipEntry>(&v, "entries[2]").unwrap_err();
        assert!(
            err.message
                .starts_with("entries[2]: unknown field(s) 'speed'"),
            "{}",
            err.message
        );
    }

    #[test]
    fn set_clip_properties_all_optional_except_clip_ids() {
        let v = serde_json::json!({"clipIds": ["a"], "speed": 1.5});
        let a: SetClipPropertiesArgs = decode_tool_args(&v, "").unwrap();
        assert_eq!(a.clip_ids, vec!["a"]);
        assert_eq!(a.speed, Some(1.5));
        assert_eq!(a.volume, None);
    }

    #[test]
    fn move_entry_requires_clip_id() {
        let v = serde_json::json!({"toFrame": 10});
        let err = decode_tool_args::<MoveEntry>(&v, "moves[0]").unwrap_err();
        assert_eq!(
            err.message,
            "moves[0].clipId: missing required field 'clipId'"
        );
    }

    #[test]
    fn ripple_ranges_parse_pairs() {
        let v = serde_json::json!({"trackIndex": 0, "ranges": [[0.0, 5.0], [10.0, 12.0]]});
        let a: RippleDeleteRangesArgs = decode_tool_args(&v, "").unwrap();
        assert_eq!(a.ranges, vec![vec![0.0, 5.0], vec![10.0, 12.0]]);
        assert_eq!(a.track_index, Some(0));
    }

    #[test]
    fn create_folder_either_form_decodes() {
        let single = serde_json::json!({"name": "B-Roll"});
        let a: CreateFolderArgs = decode_tool_args(&single, "").unwrap();
        assert_eq!(a.name.as_deref(), Some("B-Roll"));

        let batch = serde_json::json!({"entries": [{"name": "A"}, {"name": "B"}]});
        let b: CreateFolderArgs = decode_tool_args(&batch, "").unwrap();
        assert_eq!(b.entries.unwrap().len(), 2);
    }

    #[test]
    fn transform_arg_partial() {
        let v = serde_json::json!({"clipIds": ["a"], "transform": {"centerY": 0.1}});
        let a: SetClipPropertiesArgs = decode_tool_args(&v, "").unwrap();
        let t = a.transform.unwrap();
        assert_eq!(t.center_y, Some(0.1));
        assert_eq!(t.center_x, None);
    }

    #[test]
    fn inspect_media_requires_media_ref_with_path() {
        let v = serde_json::json!({"maxFrames": 6});
        let err = decode_tool_args::<InspectMediaArgs>(&v, "").unwrap_err();
        assert_eq!(err.message, "arguments: missing required field 'mediaRef'");
    }

    #[test]
    fn inspect_media_rejects_unknown_field() {
        let v = serde_json::json!({"mediaRef": "m", "bogus": 1});
        let err = decode_tool_args::<InspectMediaArgs>(&v, "").unwrap_err();
        assert!(
            err.message
                .starts_with("arguments: unknown field(s) 'bogus'"),
            "{}",
            err.message
        );
    }

    #[test]
    fn search_media_decodes_and_requires_query() {
        let ok = serde_json::json!({"query": "harbor at sunset", "scope": "visual", "limit": 5});
        let a: SearchMediaArgs = decode_tool_args(&ok, "").unwrap();
        assert_eq!(a.query, "harbor at sunset");
        assert_eq!(a.scope.as_deref(), Some("visual"));
        assert_eq!(a.limit, Some(5));

        let missing = serde_json::json!({"scope": "both"});
        let err = decode_tool_args::<SearchMediaArgs>(&missing, "").unwrap_err();
        assert_eq!(err.message, "arguments: missing required field 'query'");
    }

    #[test]
    fn list_models_type_key_maps_to_kind() {
        let v = serde_json::json!({"type": "video"});
        let a: ListModelsArgs = decode_tool_args(&v, "").unwrap();
        assert_eq!(a.kind.as_deref(), Some("video"));
        // The wire key is `type`; an unknown sibling is still rejected.
        let bad = serde_json::json!({"type": "video", "kind": "x"});
        let err = decode_tool_args::<ListModelsArgs>(&bad, "").unwrap_err();
        assert!(
            err.message.contains("unknown field(s) 'kind'"),
            "{}",
            err.message
        );
    }

    #[test]
    fn generate_video_requires_prompt_and_rejects_unknown() {
        let ok = serde_json::json!({"prompt": "a sweeping drone shot", "duration": 5});
        let a: GenerateVideoArgs = decode_tool_args(&ok, "").unwrap();
        assert_eq!(a.duration, Some(5));
        assert!(a.reference_image_media_refs.is_none());

        let bad = serde_json::json!({"prompt": "x", "aspectRation": "16:9"}); // typo
        let err = decode_tool_args::<GenerateVideoArgs>(&bad, "").unwrap_err();
        assert!(
            err.message.contains("unknown field(s) 'aspectRation'"),
            "{}",
            err.message
        );
    }

    #[test]
    fn import_media_nested_source_decodes() {
        let v = serde_json::json!({"source": {"url": "https://example.com/a.mp4"}, "name": "Clip"});
        let a: ImportMediaArgs = decode_tool_args(&v, "").unwrap();
        assert_eq!(a.source.url.as_deref(), Some("https://example.com/a.mp4"));
        assert_eq!(a.name.as_deref(), Some("Clip"));
    }

    #[test]
    fn import_source_rejects_unknown_nested_key() {
        // The source object's unknown keys are caught when decoded as its own
        // ToolArgs (the dispatch layer decodes `source` with this path prefix).
        let v = serde_json::json!({"url": "https://x", "bogus": 1});
        let err = decode_tool_args::<ImportSourceArg>(&v, "source").unwrap_err();
        assert!(
            err.message.starts_with("source: unknown field(s) 'bogus'"),
            "{}",
            err.message
        );
    }

    #[test]
    fn rename_media_entry_requires_both_with_path() {
        let v = serde_json::json!({"mediaRef": "m"});
        let err = decode_tool_args::<RenameMediaEntry>(&v, "entries[0]").unwrap_err();
        assert_eq!(
            err.message,
            "entries[0].name: missing required field 'name'"
        );
    }

    #[test]
    fn rename_folder_either_form_decodes() {
        let single = serde_json::json!({"folderId": "f", "name": "New"});
        let a: RenameFolderArgs = decode_tool_args(&single, "").unwrap();
        assert_eq!(a.folder_id.as_deref(), Some("f"));
        assert_eq!(a.name.as_deref(), Some("New"));

        let batch = serde_json::json!({"entries": [{"folderId": "a", "name": "X"}]});
        let b: RenameFolderArgs = decode_tool_args(&batch, "").unwrap();
        assert_eq!(b.entries.unwrap().len(), 1);
    }

    #[test]
    fn delete_media_requires_asset_ids() {
        let v = serde_json::json!({});
        let err = decode_tool_args::<DeleteMediaArgs>(&v, "").unwrap_err();
        assert_eq!(err.message, "arguments: missing required field 'assetIds'");

        let ok = serde_json::json!({"assetIds": ["a", "b"]});
        let a: DeleteMediaArgs = decode_tool_args(&ok, "").unwrap();
        assert_eq!(a.asset_ids, vec!["a", "b"]);
    }

    #[test]
    fn upscale_media_requires_media_ref() {
        let v = serde_json::json!({"model": "x"});
        let err = decode_tool_args::<UpscaleMediaArgs>(&v, "").unwrap_err();
        assert_eq!(err.message, "arguments: missing required field 'mediaRef'");
    }

    // --- A-tier shader-effect args ---

    #[test]
    fn set_color_grade_partial_and_clip_ids() {
        let v = serde_json::json!({
            "clipIds": ["a", "b"],
            "exposure": 0.5,
            "saturation": 1.2,
            "lift": {"r": 0.02}
        });
        let a: SetColorGradeArgs = decode_tool_args(&v, "").unwrap();
        assert_eq!(a.clip_ids, vec!["a", "b"]);
        assert_eq!(a.exposure, Some(0.5));
        assert_eq!(a.saturation, Some(1.2));
        assert_eq!(a.lift.unwrap().r, Some(0.02));
        assert_eq!(a.contrast, None);
    }

    #[test]
    fn set_color_grade_clear_form() {
        let v = serde_json::json!({"clipIds": ["a"], "clear": true});
        let a: SetColorGradeArgs = decode_tool_args(&v, "").unwrap();
        assert_eq!(a.clear, Some(true));
    }

    #[test]
    fn set_color_grade_rejects_unknown_field() {
        let v = serde_json::json!({"clipIds": ["a"], "exposre": 1.0}); // typo
        let err = decode_tool_args::<SetColorGradeArgs>(&v, "").unwrap_err();
        assert!(
            err.message.contains("unknown field(s) 'exposre'"),
            "{}",
            err.message
        );
    }

    #[test]
    fn chroma_key_decodes() {
        let v = serde_json::json!({
            "clipIds": ["a"], "keyColor": "#00FF00", "similarity": 0.3, "spill": 0.6
        });
        let a: ChromaKeyArgs = decode_tool_args(&v, "").unwrap();
        assert_eq!(a.key_color.as_deref(), Some("#00FF00"));
        assert_eq!(a.similarity, Some(0.3));
        assert_eq!(a.spill, Some(0.6));
    }

    #[test]
    fn chroma_key_requires_clip_ids() {
        let v = serde_json::json!({"keyColor": "#00FF00"});
        let err = decode_tool_args::<ChromaKeyArgs>(&v, "").unwrap_err();
        assert_eq!(err.message, "arguments: missing required field 'clipIds'");
    }

    #[test]
    fn set_mask_decodes_and_entry_guard() {
        let v = serde_json::json!({
            "clipIds": ["a"],
            "masks": [{"kind": "circle", "center": {"x": 0.5, "y": 0.5}, "radius": {"x": 0.3, "y": 0.3}}]
        });
        let a: SetMaskArgs = decode_tool_args(&v, "").unwrap();
        assert_eq!(a.masks.len(), 1);
        // Each mask entry decodes with its own per-entry unknown-key guard.
        let m: MaskArg = decode_tool_args(&a.masks[0], "masks[0]").unwrap();
        assert_eq!(m.kind, "circle");
        assert_eq!(m.center.unwrap().x, Some(0.5));
    }

    #[test]
    fn mask_entry_rejects_unknown_key() {
        let v = serde_json::json!({"kind": "circle", "radius": {"x": 0.3}, "bogus": 1});
        let err = decode_tool_args::<MaskArg>(&v, "masks[0]").unwrap_err();
        assert!(
            err.message
                .starts_with("masks[0]: unknown field(s) 'bogus'"),
            "{}",
            err.message
        );
    }

    #[test]
    fn apply_effect_decodes_with_params() {
        let v = serde_json::json!({
            "clipIds": ["a"],
            "effects": [{"name": "gaussianBlur", "params": {"radius": 4.0}}]
        });
        let a: ApplyEffectArgs = decode_tool_args(&v, "").unwrap();
        assert_eq!(a.effects.len(), 1);
        let e: EffectArg = decode_tool_args(&a.effects[0], "effects[0]").unwrap();
        assert_eq!(e.name, "gaussianBlur");
        assert_eq!(e.params.unwrap().get("radius"), Some(&4.0));
    }

    #[test]
    fn apply_effect_entry_requires_name() {
        let v = serde_json::json!({"params": {"radius": 2.0}});
        let err = decode_tool_args::<EffectArg>(&v, "effects[0]").unwrap_err();
        assert_eq!(
            err.message,
            "effects[0].name: missing required field 'name'"
        );
    }

    // --- Web motion-graphic args ---

    #[test]
    fn add_motion_graphic_decodes_code_source() {
        let v = serde_json::json!({
            "source": {"code": "<h1>Title</h1>"},
            "startFrame": 30,
            "durationFrames": 90,
            "transparent": true
        });
        let a: AddMotionGraphicArgs = decode_tool_args(&v, "").unwrap();
        assert_eq!(a.start_frame, 30);
        assert_eq!(a.duration_frames, 90);
        assert_eq!(a.transparent, Some(true));
        assert!(a.track_index.is_none());
        // The nested source decodes with its own per-object unknown-key guard.
        let src: MotionSourceArg = decode_tool_args(&a.source, "source").unwrap();
        assert_eq!(src.code.as_deref(), Some("<h1>Title</h1>"));
        assert!(src.template_id.is_none());
    }

    #[test]
    fn add_motion_graphic_decodes_template_source_with_params() {
        let v = serde_json::json!({
            "source": {"templateId": "lower-third.glass", "params": {"title": "Hi", "accent": "#FF0066"}},
            "startFrame": 0,
            "durationFrames": 120,
            "trackIndex": 2
        });
        let a: AddMotionGraphicArgs = decode_tool_args(&v, "").unwrap();
        assert_eq!(a.track_index, Some(2));
        let src: MotionSourceArg = decode_tool_args(&a.source, "source").unwrap();
        assert_eq!(src.template_id.as_deref(), Some("lower-third.glass"));
        assert_eq!(
            src.params.as_ref().unwrap().get("title").unwrap(),
            &serde_json::json!("Hi")
        );
    }

    #[test]
    fn add_motion_graphic_requires_source_and_frames() {
        let v = serde_json::json!({"startFrame": 0, "durationFrames": 30});
        let err = decode_tool_args::<AddMotionGraphicArgs>(&v, "").unwrap_err();
        assert_eq!(err.message, "arguments: missing required field 'source'");
    }

    #[test]
    fn add_motion_graphic_rejects_unknown_field() {
        let v = serde_json::json!({
            "source": {"code": "x"}, "startFrame": 0, "durationFrames": 30, "loop": true
        });
        let err = decode_tool_args::<AddMotionGraphicArgs>(&v, "").unwrap_err();
        assert!(
            err.message.contains("unknown field(s) 'loop'"),
            "{}",
            err.message
        );
    }

    #[test]
    fn motion_source_arg_rejects_unknown_nested_key() {
        let v = serde_json::json!({"code": "x", "bogus": 1});
        let err = decode_tool_args::<MotionSourceArg>(&v, "source").unwrap_err();
        assert!(
            err.message.starts_with("source: unknown field(s) 'bogus'"),
            "{}",
            err.message
        );
    }

    #[test]
    fn edit_motion_graphic_decodes_and_requires_clip_id() {
        let ok = serde_json::json!({"clipId": "c1", "code": "<b>new</b>"});
        let a: EditMotionGraphicArgs = decode_tool_args(&ok, "").unwrap();
        assert_eq!(a.clip_id, "c1");
        assert_eq!(a.code.as_deref(), Some("<b>new</b>"));
        assert!(a.params.is_none());

        let missing = serde_json::json!({"code": "x"});
        let err = decode_tool_args::<EditMotionGraphicArgs>(&missing, "").unwrap_err();
        assert_eq!(err.message, "arguments: missing required field 'clipId'");
    }

    #[test]
    fn edit_motion_graphic_decodes_params_override() {
        let v = serde_json::json!({"clipId": "c1", "params": {"title": "Updated"}});
        let a: EditMotionGraphicArgs = decode_tool_args(&v, "").unwrap();
        assert_eq!(
            a.params.unwrap().get("title").unwrap(),
            &serde_json::json!("Updated")
        );
    }
}
