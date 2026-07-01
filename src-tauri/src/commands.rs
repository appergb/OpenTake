//! The `#[tauri::command]` surface.
//!
//! Each command is a thin shim: it locks nothing of its own, delegates to an
//! `opentake_core::dto::handle_*` function (which wraps [`AppCore`]), and maps
//! the boundary `CmdError` to a `String` so the front end gets a plain rejected
//! Promise (`AGENTS.md`: "边界层转 Tauri 的 `Err(String)`").
//!
//! `EditCommand` itself is not `Deserialize` (it carries engine value types with
//! no serde derives), so the editing entry point takes a local serde-friendly
//! [`EditRequest`] that maps 1:1 onto the variants the front end issues in v1.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use opentake_core::dto::{
    handle_edit_apply, handle_get_timeline, handle_project_new, handle_project_open,
    handle_project_save, handle_redo, handle_undo, EditResultDto, TimelineSnapshotDto,
};
use opentake_core::{AppCore, CmdError, EditCommand};

use opentake_ops::{
    ClipEntry, ClipMove, ClipProperties, FrameRange, KeyframePayload, KeyframeProperty,
    KeyframeValue, RenameEntry, TextEntry,
};

use opentake_domain::{
    AnimPair, ChromaKey, ClipType, ColorGrade, Crop, Effect, Interpolation, Keyframe,
    KeyframeTrack, Mask, TextStyle, Transform,
};

// MARK: - Read / lifecycle commands (direct DTO passthrough)

/// `get_timeline`: current read-only mirror + version. Infallible.
#[tauri::command]
pub fn get_timeline(core: State<'_, AppCore>) -> TimelineSnapshotDto {
    handle_get_timeline(&core)
}

/// `undo` / `redo`: global history navigation.
#[tauri::command]
pub fn undo(core: State<'_, AppCore>) -> Result<EditResultDto, String> {
    handle_undo(&core).map_err(msg)
}

#[tauri::command]
pub fn redo(core: State<'_, AppCore>) -> Result<EditResultDto, String> {
    handle_redo(&core).map_err(msg)
}

/// `project_new`: replace the session with a fresh, unsaved project.
#[tauri::command]
pub fn project_new(core: State<'_, AppCore>) {
    handle_project_new(&core);
}

/// `project_open`: open a `.opentake` bundle, returning the first snapshot.
#[tauri::command]
pub fn project_open(core: State<'_, AppCore>, path: String) -> Result<TimelineSnapshotDto, String> {
    handle_project_open(&core, path).map_err(msg)
}

/// `project_save`: `path = None` saves back to the open bundle; `Some` is save-as.
#[tauri::command]
pub fn project_save(core: State<'_, AppCore>, path: Option<String>) -> Result<String, String> {
    handle_project_save(&core, path).map_err(msg)
}

/// `get_default_project_dir`: the default folder new projects save into
/// (`~/Documents/OpenTake`, created on first use). Mirrors upstream
/// `Project.storageDirectory` (`~/Documents/Palmier Pro`). The front end uses it
/// as the save dialog's `defaultPath` so the user picks a location + name like
/// upstream `createNewProject` (`NSSavePanel`).
#[tauri::command]
pub fn get_default_project_dir(app: AppHandle) -> Result<String, String> {
    let dir = app
        .path()
        .document_dir()
        .map_err(|e| e.to_string())?
        .join("OpenTake");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().into_owned())
}

/// `export_xmeml`: write the current timeline to `path` as XMEML 4 (Final Cut
/// Pro 7 XML, `.xml`). This is the Premiere / DaVinci / 剪映-importable
/// interchange format — Premiere Pro does NOT read modern FCPXML natively, so
/// upstream (and OpenTake) emit XMEML; DaVinci/FCP still import FCP7 XML. Reads
/// the timeline / media manifest / project dir from the core, builds the XML via
/// the pure `export_xmeml`, and writes the file.
#[tauri::command]
pub fn export_xmeml(core: State<'_, AppCore>, path: String) -> Result<(), String> {
    let timeline = core.get_timeline().timeline;
    let manifest = core.media();
    let project_dir = core.project_dir();
    let xml = opentake_project::export_xmeml(&timeline, &manifest, project_dir.as_deref());
    std::fs::write(&path, xml).map_err(|e| e.to_string())
}

/// `export_fcpxml`: deprecated alias for [`export_xmeml`], kept so any existing
/// front-end caller keeps working. The command name historically said "fcpxml"
/// but always produced XMEML 4 (FCP7 XML); the honest name is `export_xmeml`.
/// New code (and the format picker) should call `export_xmeml`; native FCPXML is
/// `export_fcpxml_modern`.
#[tauri::command]
pub fn export_fcpxml(core: State<'_, AppCore>, path: String) -> Result<(), String> {
    export_xmeml(core, path)
}

/// `export_edl`: write the current timeline to `path` as a CMX3600 EDL (`.edl`).
/// A flat, video-track-only edit decision list (the EDL format itself only
/// describes one V track + linked audio channels) that Premiere / DaVinci /
/// Avid / 剪映 import. Effects, transforms, opacity, and multi-track layering are
/// dropped — see `opentake_project::edl` for the documented limitations.
#[tauri::command]
pub fn export_edl(core: State<'_, AppCore>, path: String) -> Result<(), String> {
    let timeline = core.get_timeline().timeline;
    let manifest = core.media();
    let edl = opentake_project::export_edl(&timeline, &manifest);
    std::fs::write(&path, edl).map_err(|e| e.to_string())
}

/// `export_otio`: write the current timeline to `path` as OpenTimelineIO JSON
/// (`.otio`) — the industry-standard interchange `otioview` / DaVinci / Blender
/// read. Preserves track order/kind, clip placement, source ranges, gaps, and
/// per-clip media references; see `opentake_project::otio` for what is dropped
/// (effects, transforms, keyframes).
#[tauri::command]
pub fn export_otio(core: State<'_, AppCore>, path: String) -> Result<(), String> {
    let timeline = core.get_timeline().timeline;
    let manifest = core.media();
    let project_dir = core.project_dir();
    let json = opentake_project::export_otio(&timeline, &manifest, project_dir.as_deref());
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

/// `export_fcpxml_modern`: write the current timeline to `path` as native Final
/// Cut Pro X FCPXML 1.10 (`.fcpxml`). Unlike XMEML, this carries text overlays
/// (`<title>`), transforms, opacity, and volume. NOTE: Premiere does NOT import
/// FCPXML — use `export_xmeml` for Premiere / DaVinci / 剪映. See
/// `opentake_project::fcpxml_modern`.
#[tauri::command]
pub fn export_fcpxml_modern(core: State<'_, AppCore>, path: String) -> Result<(), String> {
    let timeline = core.get_timeline().timeline;
    let manifest = core.media();
    let project_dir = core.project_dir();
    let xml = opentake_project::export_fcpxml(&timeline, &manifest, project_dir.as_deref());
    std::fs::write(&path, xml).map_err(|e| e.to_string())
}

/// Requested subtitle container, projected from the front end. Lower-cased serde
/// tags (`"srt"` / `"vtt"`) match the file extension the user picks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubtitleFormat {
    /// SubRip (`.srt`) — `HH:MM:SS,mmm` timestamps, numbered cues.
    #[default]
    Srt,
    /// WebVTT (`.vtt`) — `HH:MM:SS.mmm` timestamps, `WEBVTT` header.
    Vtt,
}

/// Summary of a completed subtitle export, returned to the front end. `cueCount`
/// lets the UI distinguish "wrote N cues" from "timeline has no captions" (in
/// which case it shows a friendly toast); the file is still written either way —
/// an empty SRT / header-only VTT is the documented contract of the pure layer.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleExportSummary {
    /// Absolute path the subtitle file was written to.
    pub out_path: String,
    /// Number of caption cues emitted.
    pub cue_count: usize,
}

/// `export_subtitles`: write the current timeline's caption clips to `path` as a
/// SubRip (`.srt`) or WebVTT (`.vtt`) document. Caption cues are collected from
/// every track via the pure `opentake_domain::subtitle_export` layer (any clip
/// carrying a `caption_group_id` + non-empty `text_content`), serialized, and
/// written to disk. Returns the cue count so the UI can report an empty result.
#[tauri::command]
pub fn export_subtitles(
    core: State<'_, AppCore>,
    path: String,
    format: SubtitleFormat,
) -> Result<SubtitleExportSummary, String> {
    let timeline = core.get_timeline().timeline;
    write_subtitles(&timeline, path, format)
}

/// The subtitle export body, decoupled from Tauri/`AppCore` so it can be driven
/// by a unit test with a hand-built timeline + temp path. The command wrapper
/// only snapshots the live session and delegates here.
fn write_subtitles(
    timeline: &opentake_domain::Timeline,
    path: String,
    format: SubtitleFormat,
) -> Result<SubtitleExportSummary, String> {
    let cue_count = opentake_domain::collect_caption_cues(timeline).len();
    let body = match format {
        SubtitleFormat::Srt => opentake_domain::export_srt(timeline),
        SubtitleFormat::Vtt => opentake_domain::export_vtt(timeline),
    };
    std::fs::write(&path, body).map_err(|e| e.to_string())?;
    Ok(SubtitleExportSummary {
        out_path: path,
        cue_count,
    })
}

/// `can_undo` / `can_redo`: enable/disable the toolbar affordances.
#[tauri::command]
pub fn can_undo(core: State<'_, AppCore>) -> bool {
    core.can_undo()
}

#[tauri::command]
pub fn can_redo(core: State<'_, AppCore>) -> bool {
    core.can_redo()
}

// MARK: - The single editing entry point

/// `edit_apply`: the unified editing command. The front end constructs an
/// [`EditRequest`] from a UI gesture; this maps it to an [`EditCommand`] and
/// routes it through [`AppCore::apply`] (which performs the snapshot/commit/
/// version transaction and emits `TimelineChanged`).
#[tauri::command]
pub fn edit_apply(core: State<'_, AppCore>, command: EditRequest) -> Result<EditResultDto, String> {
    let cmd = command.into_command()?;
    handle_edit_apply(&core, cmd).map_err(msg)
}

/// `check_path_exists`: checks if a path (e.g. project bundle folder) exists on disk.
#[tauri::command]
pub fn check_path_exists(path: String) -> bool {
    std::path::Path::new(&path).exists()
}

fn msg(e: CmdError) -> String {
    e.message
}

// MARK: - EditRequest (serde-friendly mirror of EditCommand)

/// A serde-deserializable mirror of the [`EditCommand`] variants the front end
/// issues. Tagged `{ "type": "addClips", ... }` to match the TS discriminated
/// union. Engine value types (`ClipMove`, `TrimEdit`, `FrameRange`, keyframe
/// tracks) are mirrored as local serde DTOs and converted in [`into_command`].
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum EditRequest {
    #[serde(rename_all = "camelCase")]
    AddClips { entries: Vec<ClipEntryDto> },
    #[serde(rename_all = "camelCase")]
    InsertClips {
        track_index: usize,
        at_frame: i32,
        entries: Vec<ClipEntryDto>,
    },
    #[serde(rename_all = "camelCase")]
    MoveClips { moves: Vec<ClipMoveDto> },
    #[serde(rename_all = "camelCase")]
    DuplicateClips {
        clip_ids: Vec<String>,
        offset_frames: i32,
        target_track_indexes: Vec<usize>,
    },
    #[serde(rename_all = "camelCase")]
    RemoveClips { clip_ids: Vec<String> },
    #[serde(rename_all = "camelCase")]
    SplitClip { clip_id: String, at_frame: i32 },
    #[serde(rename_all = "camelCase")]
    TrimClips { edits: Vec<TrimEditDto> },
    #[serde(rename_all = "camelCase")]
    SetClipProperties {
        clip_ids: Vec<String>,
        // Boxed to keep `EditRequest` small: `ClipPropertiesDto` carries a full
        // `TextStyle`, which would otherwise dominate the enum size.
        properties: Box<ClipPropertiesDto>,
    },
    #[serde(rename_all = "camelCase")]
    SetKeyframes {
        clip_id: String,
        property: KeyframePropertyDto,
        payload: KeyframePayloadDto,
    },
    #[serde(rename_all = "camelCase")]
    StampKeyframe {
        clip_id: String,
        property: KeyframePropertyDto,
        frame: i32,
    },
    #[serde(rename_all = "camelCase")]
    UpsertKeyframe {
        clip_id: String,
        property: KeyframePropertyDto,
        frame: i32,
        value: KeyframeValueDto,
    },
    #[serde(rename_all = "camelCase")]
    RemoveKeyframe {
        clip_id: String,
        property: KeyframePropertyDto,
        frame: i32,
    },
    #[serde(rename_all = "camelCase")]
    MoveKeyframe {
        clip_id: String,
        property: KeyframePropertyDto,
        from_frame: i32,
        to_frame: i32,
    },
    #[serde(rename_all = "camelCase")]
    SetKeyframeInterpolation {
        clip_id: String,
        property: KeyframePropertyDto,
        frame: i32,
        interpolation: Interpolation,
    },
    #[serde(rename_all = "camelCase")]
    SetColorGrade {
        clip_ids: Vec<String>,
        grade: Option<ColorGrade>,
    },
    #[serde(rename_all = "camelCase")]
    SetChromaKey {
        clip_ids: Vec<String>,
        chroma_key: Option<ChromaKey>,
    },
    #[serde(rename_all = "camelCase")]
    SetMasks {
        clip_ids: Vec<String>,
        masks: Vec<Mask>,
    },
    #[serde(rename_all = "camelCase")]
    SetEffects {
        clip_ids: Vec<String>,
        effects: Vec<Effect>,
    },
    #[serde(rename_all = "camelCase")]
    RippleDeleteRanges {
        track_index: usize,
        ranges: Vec<FrameRangeDto>,
    },
    #[serde(rename_all = "camelCase")]
    RippleDeleteClips { clip_ids: Vec<String> },
    #[serde(rename_all = "camelCase")]
    AddTexts { entries: Vec<TextEntryDto> },
    #[serde(rename_all = "camelCase")]
    Link { clip_ids: Vec<String> },
    #[serde(rename_all = "camelCase")]
    Unlink { clip_ids: Vec<String> },
    #[serde(rename_all = "camelCase")]
    RemoveTracks { track_indexes: Vec<usize> },
    #[serde(rename_all = "camelCase")]
    SwapTracks { a: usize, b: usize },
    #[serde(rename_all = "camelCase")]
    SwapClips { clip_a: String, clip_b: String },
    #[serde(rename_all = "camelCase")]
    InsertTrack { kind: ClipType, at: Option<usize> },
    #[serde(rename_all = "camelCase")]
    SetTrackProps {
        track_index: usize,
        muted: Option<bool>,
        hidden: Option<bool>,
        sync_locked: Option<bool>,
    },
    #[serde(rename_all = "camelCase")]
    CreateFolder {
        name: String,
        parent_folder_id: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    MoveToFolder {
        asset_ids: Vec<String>,
        folder_id: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    RenameMedia { entries: Vec<RenameEntryDto> },
    #[serde(rename_all = "camelCase")]
    RenameFolder { entries: Vec<RenameEntryDto> },
    #[serde(rename_all = "camelCase")]
    DeleteMedia { asset_ids: Vec<String> },
    #[serde(rename_all = "camelCase")]
    DeleteFolder { folder_ids: Vec<String> },
    #[serde(rename_all = "camelCase")]
    SwapMedia { clip_id: String, media_ref: String },
}

impl EditRequest {
    fn into_command(self) -> Result<EditCommand, String> {
        Ok(match self {
            EditRequest::AddClips { entries } => EditCommand::AddClips {
                entries: entries.into_iter().map(ClipEntryDto::into_entry).collect(),
            },
            EditRequest::InsertClips {
                track_index,
                at_frame,
                entries,
            } => EditCommand::InsertClips {
                track_index,
                at_frame,
                entries: entries.into_iter().map(ClipEntryDto::into_entry).collect(),
            },
            EditRequest::MoveClips { moves } => EditCommand::MoveClips {
                moves: moves.into_iter().map(ClipMoveDto::into_move).collect(),
            },
            EditRequest::DuplicateClips {
                clip_ids,
                offset_frames,
                target_track_indexes,
            } => EditCommand::DuplicateClips {
                clip_ids,
                offset_frames,
                target_track_indexes,
            },
            EditRequest::RemoveClips { clip_ids } => EditCommand::RemoveClips { clip_ids },
            EditRequest::SplitClip { clip_id, at_frame } => {
                EditCommand::SplitClip { clip_id, at_frame }
            }
            EditRequest::TrimClips { edits } => EditCommand::TrimClips {
                edits: edits.into_iter().map(TrimEditDto::into_edit).collect(),
            },
            EditRequest::SetClipProperties {
                clip_ids,
                properties,
            } => EditCommand::SetClipProperties {
                clip_ids,
                properties: Box::new((*properties).into_properties()),
            },
            EditRequest::SetKeyframes {
                clip_id,
                property,
                payload,
            } => EditCommand::SetKeyframes {
                clip_id,
                property: property.into(),
                payload: payload.into_payload()?,
            },
            EditRequest::StampKeyframe {
                clip_id,
                property,
                frame,
            } => EditCommand::StampKeyframe {
                clip_id,
                property: property.into(),
                frame,
            },
            EditRequest::UpsertKeyframe {
                clip_id,
                property,
                frame,
                value,
            } => EditCommand::UpsertKeyframe {
                clip_id,
                property: property.into(),
                frame,
                value: value.into_value(),
            },
            EditRequest::RemoveKeyframe {
                clip_id,
                property,
                frame,
            } => EditCommand::RemoveKeyframe {
                clip_id,
                property: property.into(),
                frame,
            },
            EditRequest::MoveKeyframe {
                clip_id,
                property,
                from_frame,
                to_frame,
            } => EditCommand::MoveKeyframe {
                clip_id,
                property: property.into(),
                from_frame,
                to_frame,
            },
            EditRequest::SetKeyframeInterpolation {
                clip_id,
                property,
                frame,
                interpolation,
            } => EditCommand::SetKeyframeInterpolation {
                clip_id,
                property: property.into(),
                frame,
                interpolation,
            },
            EditRequest::SetColorGrade { clip_ids, grade } => {
                EditCommand::SetColorGrade { clip_ids, grade }
            }
            EditRequest::SetChromaKey {
                clip_ids,
                chroma_key,
            } => EditCommand::SetChromaKey {
                clip_ids,
                chroma_key,
            },
            EditRequest::SetMasks { clip_ids, masks } => EditCommand::SetMasks { clip_ids, masks },
            EditRequest::SetEffects { clip_ids, effects } => {
                EditCommand::SetEffects { clip_ids, effects }
            }
            EditRequest::RippleDeleteRanges {
                track_index,
                ranges,
            } => EditCommand::RippleDeleteRanges {
                track_index,
                ranges: ranges.into_iter().map(FrameRangeDto::into_range).collect(),
            },
            EditRequest::RippleDeleteClips { clip_ids } => {
                EditCommand::RippleDeleteClips { clip_ids }
            }
            EditRequest::AddTexts { entries } => EditCommand::AddTexts {
                entries: entries.into_iter().map(TextEntryDto::into_entry).collect(),
            },
            EditRequest::Link { clip_ids } => EditCommand::Link { clip_ids },
            EditRequest::Unlink { clip_ids } => EditCommand::Unlink { clip_ids },
            EditRequest::RemoveTracks { track_indexes } => {
                EditCommand::RemoveTracks { track_indexes }
            }
            EditRequest::SwapTracks { a, b } => EditCommand::SwapTracks { a, b },
            EditRequest::SwapClips { clip_a, clip_b } => EditCommand::SwapClips {
                a: clip_a,
                b: clip_b,
            },
            EditRequest::InsertTrack { kind, at } => EditCommand::InsertTrack { kind, at },
            EditRequest::SetTrackProps {
                track_index,
                muted,
                hidden,
                sync_locked,
            } => EditCommand::SetTrackProps {
                track_index,
                muted,
                hidden,
                sync_locked,
            },
            EditRequest::CreateFolder {
                name,
                parent_folder_id,
            } => EditCommand::CreateFolder {
                name,
                parent_folder_id,
            },
            EditRequest::MoveToFolder {
                asset_ids,
                folder_id,
            } => EditCommand::MoveToFolder {
                asset_ids,
                folder_id,
            },
            EditRequest::RenameMedia { entries } => EditCommand::RenameMedia {
                entries: entries
                    .into_iter()
                    .map(RenameEntryDto::into_entry)
                    .collect(),
            },
            EditRequest::RenameFolder { entries } => EditCommand::RenameFolder {
                entries: entries
                    .into_iter()
                    .map(RenameEntryDto::into_entry)
                    .collect(),
            },
            EditRequest::DeleteMedia { asset_ids } => EditCommand::DeleteMedia { asset_ids },
            EditRequest::DeleteFolder { folder_ids } => EditCommand::DeleteFolder { folder_ids },
            EditRequest::SwapMedia { clip_id, media_ref } => {
                EditCommand::SwapMedia { clip_id, media_ref }
            }
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipEntryDto {
    pub media_ref: String,
    pub media_type: ClipType,
    pub source_clip_type: ClipType,
    pub track_index: usize,
    pub start_frame: i32,
    pub duration_frames: i32,
    #[serde(default)]
    pub trim_start_frame: Option<i32>,
    #[serde(default)]
    pub trim_end_frame: Option<i32>,
    #[serde(default)]
    pub has_audio: bool,
    #[serde(default)]
    pub add_linked_audio: bool,
    #[serde(default)]
    pub transform: Option<Transform>,
}

impl ClipEntryDto {
    fn into_entry(self) -> ClipEntry {
        ClipEntry {
            media_ref: self.media_ref,
            media_type: self.media_type,
            source_clip_type: self.source_clip_type,
            track_index: self.track_index,
            start_frame: self.start_frame,
            duration_frames: self.duration_frames,
            trim_start_frame: self.trim_start_frame,
            trim_end_frame: self.trim_end_frame,
            has_audio: self.has_audio,
            add_linked_audio: self.add_linked_audio,
            transform: self.transform,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipMoveDto {
    pub clip_id: String,
    pub to_track: usize,
    pub to_frame: i32,
}

impl ClipMoveDto {
    fn into_move(self) -> ClipMove {
        ClipMove {
            clip_id: self.clip_id,
            to_track: self.to_track,
            to_frame: self.to_frame,
        }
    }
}

/// `[clip_id, trim_start, trim_end]` in source frames (matches `TrimEdit`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrimEditDto {
    pub clip_id: String,
    pub trim_start_frame: i32,
    pub trim_end_frame: i32,
}

impl TrimEditDto {
    fn into_edit(self) -> (String, i32, i32) {
        (self.clip_id, self.trim_start_frame, self.trim_end_frame)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameRangeDto {
    pub start: i32,
    pub end: i32,
}

impl FrameRangeDto {
    fn into_range(self) -> FrameRange {
        FrameRange::new(self.start, self.end)
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipPropertiesDto {
    #[serde(default)]
    pub duration_frames: Option<i32>,
    #[serde(default)]
    pub trim_start_frame: Option<i32>,
    #[serde(default)]
    pub trim_end_frame: Option<i32>,
    #[serde(default)]
    pub speed: Option<f64>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub opacity: Option<f64>,
    #[serde(default)]
    pub transform: Option<Transform>,
    #[serde(default)]
    pub text_content: Option<String>,
    #[serde(default)]
    pub text_style: Option<TextStyle>,
    #[serde(default)]
    pub crop: Option<Crop>,
    #[serde(default)]
    pub fade_in_frames: Option<i32>,
    #[serde(default)]
    pub fade_out_frames: Option<i32>,
    #[serde(default)]
    pub fade_in_interpolation: Option<Interpolation>,
    #[serde(default)]
    pub fade_out_interpolation: Option<Interpolation>,
    #[serde(default)]
    pub flip_horizontal: Option<bool>,
    #[serde(default)]
    pub flip_vertical: Option<bool>,
}

impl ClipPropertiesDto {
    fn into_properties(self) -> ClipProperties {
        ClipProperties {
            duration_frames: self.duration_frames,
            trim_start_frame: self.trim_start_frame,
            trim_end_frame: self.trim_end_frame,
            speed: self.speed,
            volume: self.volume,
            opacity: self.opacity,
            transform: self.transform,
            text_content: self.text_content,
            text_style: self.text_style,
            crop: self.crop,
            fade_in_frames: self.fade_in_frames,
            fade_out_frames: self.fade_out_frames,
            fade_in_interpolation: self.fade_in_interpolation,
            fade_out_interpolation: self.fade_out_interpolation,
            flip_horizontal: self.flip_horizontal,
            flip_vertical: self.flip_vertical,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEntryDto {
    pub track_index: usize,
    pub start_frame: i32,
    pub duration_frames: i32,
    pub content: String,
    pub text_style: TextStyle,
    pub transform: Transform,
}

impl TextEntryDto {
    fn into_entry(self) -> TextEntry {
        TextEntry {
            track_index: self.track_index,
            start_frame: self.start_frame,
            duration_frames: self.duration_frames,
            content: self.content,
            text_style: self.text_style,
            transform: self.transform,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameEntryDto {
    pub id: String,
    pub name: String,
}

impl RenameEntryDto {
    fn into_entry(self) -> RenameEntry {
        RenameEntry {
            id: self.id,
            name: self.name,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyframePropertyDto {
    Opacity,
    Volume,
    Rotation,
    Position,
    Scale,
    Crop,
}

impl From<KeyframePropertyDto> for KeyframeProperty {
    fn from(p: KeyframePropertyDto) -> Self {
        match p {
            KeyframePropertyDto::Opacity => KeyframeProperty::Opacity,
            KeyframePropertyDto::Volume => KeyframeProperty::Volume,
            KeyframePropertyDto::Rotation => KeyframeProperty::Rotation,
            KeyframePropertyDto::Position => KeyframeProperty::Position,
            KeyframePropertyDto::Scale => KeyframeProperty::Scale,
            KeyframePropertyDto::Crop => KeyframeProperty::Crop,
        }
    }
}

/// One keyframe `{ frame, value, interpolationOut }` carrying a JSON value;
/// shaped per the target track in [`KeyframePayloadDto`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScalarKfDto {
    pub frame: i32,
    pub value: f64,
    #[serde(default)]
    pub interpolation_out: Option<Interpolation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairKfDto {
    pub frame: i32,
    pub value: AnimPair,
    #[serde(default)]
    pub interpolation_out: Option<Interpolation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropKfDto {
    pub frame: i32,
    pub value: Crop,
    #[serde(default)]
    pub interpolation_out: Option<Interpolation>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum KeyframePayloadDto {
    Scalar { keyframes: Vec<ScalarKfDto> },
    Pair { keyframes: Vec<PairKfDto> },
    Crop { keyframes: Vec<CropKfDto> },
}

impl KeyframePayloadDto {
    fn into_payload(self) -> Result<KeyframePayload, String> {
        Ok(match self {
            KeyframePayloadDto::Scalar { keyframes } => {
                let kfs = keyframes
                    .into_iter()
                    .map(|k| match k.interpolation_out {
                        Some(i) => Keyframe::with_interpolation(k.frame, k.value, i),
                        None => Keyframe::new(k.frame, k.value),
                    })
                    .collect();
                KeyframePayload::Scalar(KeyframeTrack::from_keyframes(kfs))
            }
            KeyframePayloadDto::Pair { keyframes } => {
                let kfs = keyframes
                    .into_iter()
                    .map(|k| match k.interpolation_out {
                        Some(i) => Keyframe::with_interpolation(k.frame, k.value, i),
                        None => Keyframe::new(k.frame, k.value),
                    })
                    .collect();
                KeyframePayload::Pair(KeyframeTrack::from_keyframes(kfs))
            }
            KeyframePayloadDto::Crop { keyframes } => {
                let kfs = keyframes
                    .into_iter()
                    .map(|k| match k.interpolation_out {
                        Some(i) => Keyframe::with_interpolation(k.frame, k.value, i),
                        None => Keyframe::new(k.frame, k.value),
                    })
                    .collect();
                KeyframePayload::Crop(KeyframeTrack::from_keyframes(kfs))
            }
        })
    }
}

/// An explicit single-value payload for [`EditRequest::UpsertKeyframe`]. Mirrors
/// [`KeyframePayloadDto`]'s `kind`-tagging, but carries one value (not a whole
/// replacement track) to upsert at the command's `frame`.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum KeyframeValueDto {
    Scalar { value: f64 },
    Pair { value: AnimPair },
    Crop { value: Crop },
}

impl KeyframeValueDto {
    fn into_value(self) -> KeyframeValue {
        match self {
            KeyframeValueDto::Scalar { value } => KeyframeValue::Scalar(value),
            KeyframeValueDto::Pair { value } => KeyframeValue::Pair(value),
            KeyframeValueDto::Crop { value } => KeyframeValue::Crop(value),
        }
    }
}

#[cfg(test)]
mod edit_request_serde_tests {
    use super::EditRequest;
    use opentake_core::EditCommand;

    // Regression: the front end sends camelCase keys (clipIds/clipId/atFrame…).
    // serde's enum-level `rename_all` does NOT rename struct-variant fields, so
    // each variant needs its own `rename_all`; without it RemoveClips/SplitClip/
    // … failed to deserialize ("missing field `clip_ids`") and delete/split/etc.
    // silently did nothing.
    #[test]
    fn deserializes_camelcase_multiword_commands() {
        serde_json::from_str::<EditRequest>(r#"{"type":"removeClips","clipIds":["a"]}"#)
            .expect("removeClips camelCase");
        serde_json::from_str::<EditRequest>(r#"{"type":"splitClip","clipId":"a","atFrame":5}"#)
            .expect("splitClip camelCase");
        serde_json::from_str::<EditRequest>(
            r#"{"type":"insertClips","trackIndex":0,"atFrame":0,"entries":[]}"#,
        )
        .expect("insertClips camelCase");
        serde_json::from_str::<EditRequest>(r#"{"type":"rippleDeleteClips","clipIds":["a"]}"#)
            .expect("rippleDeleteClips camelCase");
    }

    #[test]
    fn deserializes_set_clip_properties_with_text_style() {
        // The Inspector sends camelCase `textStyle` with nested camelCase fields
        // (fontName/fontSize/…). It must deserialize and map onto the command's
        // ClipProperties.text_style.
        let request = serde_json::from_str::<EditRequest>(
            r#"{"type":"setClipProperties","clipIds":["c1"],"properties":{"textStyle":{"fontName":"Times-Bold","fontSize":48,"alignment":"left"}}}"#,
        )
        .expect("setClipProperties with textStyle camelCase");

        match request.into_command().expect("setClipProperties command") {
            EditCommand::SetClipProperties {
                clip_ids,
                properties,
            } => {
                assert_eq!(clip_ids, vec!["c1"]);
                let style = properties.text_style.expect("text_style present");
                assert_eq!(style.font_name, "Times-Bold");
                assert_eq!(style.font_size, 48.0);
            }
            other => panic!("expected SetClipProperties, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_swap_media_and_maps_to_command() {
        let request = serde_json::from_str::<EditRequest>(
            r#"{"type":"swapMedia","clipId":"clip-1","mediaRef":"asset-2"}"#,
        )
        .expect("swapMedia camelCase");

        match request.into_command().expect("swapMedia command") {
            EditCommand::SwapMedia { clip_id, media_ref } => {
                assert_eq!(clip_id, "clip-1");
                assert_eq!(media_ref, "asset-2");
            }
            other => panic!("expected SwapMedia, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_swap_tracks_and_maps_to_command() {
        let request = serde_json::from_str::<EditRequest>(r#"{"type":"swapTracks","a":0,"b":2}"#)
            .expect("swapTracks camelCase");

        match request.into_command().expect("swapTracks command") {
            EditCommand::SwapTracks { a, b } => {
                assert_eq!(a, 0);
                assert_eq!(b, 2);
            }
            other => panic!("expected SwapTracks, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_swap_clips_and_maps_to_command() {
        // camelCase clipA/clipB must deserialize, or the cross-track swap gesture
        // silently fails at the IPC boundary (the recurring DTO camelCase trap).
        let request = serde_json::from_str::<EditRequest>(
            r#"{"type":"swapClips","clipA":"clip-1","clipB":"clip-2"}"#,
        )
        .expect("swapClips camelCase");

        match request.into_command().expect("swapClips command") {
            EditCommand::SwapClips { a, b } => {
                assert_eq!(a, "clip-1");
                assert_eq!(b, "clip-2");
            }
            other => panic!("expected SwapClips, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_upsert_keyframe_scalar_and_maps_to_command() {
        // camelCase clipId/frame must deserialize (the recurring DTO camelCase
        // trap), and the "scalar" kind must map onto KeyframeValue::Scalar.
        let request = serde_json::from_str::<EditRequest>(
            r#"{"type":"upsertKeyframe","clipId":"clip-1","property":"opacity","frame":110,"value":{"kind":"scalar","value":0.25}}"#,
        )
        .expect("upsertKeyframe scalar camelCase");

        match request.into_command().expect("upsertKeyframe command") {
            EditCommand::UpsertKeyframe {
                clip_id,
                property,
                frame,
                value,
            } => {
                assert_eq!(clip_id, "clip-1");
                assert_eq!(property, opentake_ops::KeyframeProperty::Opacity);
                assert_eq!(frame, 110);
                assert!(matches!(value, opentake_ops::KeyframeValue::Scalar(v) if v == 0.25));
            }
            other => panic!("expected UpsertKeyframe, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_upsert_keyframe_pair_and_crop_and_maps_to_command() {
        let pair_request = serde_json::from_str::<EditRequest>(
            r#"{"type":"upsertKeyframe","clipId":"clip-1","property":"position","frame":10,"value":{"kind":"pair","value":{"a":0.3,"b":0.7}}}"#,
        )
        .expect("upsertKeyframe pair camelCase");
        match pair_request
            .into_command()
            .expect("upsertKeyframe pair command")
        {
            EditCommand::UpsertKeyframe {
                property, value, ..
            } => {
                assert_eq!(property, opentake_ops::KeyframeProperty::Position);
                match value {
                    opentake_ops::KeyframeValue::Pair(p) => {
                        assert_eq!(p.a, 0.3);
                        assert_eq!(p.b, 0.7);
                    }
                    other => panic!("expected Pair value, got {other:?}"),
                }
            }
            other => panic!("expected UpsertKeyframe, got {other:?}"),
        }

        let crop_request = serde_json::from_str::<EditRequest>(
            r#"{"type":"upsertKeyframe","clipId":"clip-1","property":"crop","frame":10,"value":{"kind":"crop","value":{"left":0.1,"top":0.2,"right":0.3,"bottom":0.4}}}"#,
        )
        .expect("upsertKeyframe crop camelCase");
        match crop_request
            .into_command()
            .expect("upsertKeyframe crop command")
        {
            EditCommand::UpsertKeyframe {
                property, value, ..
            } => {
                assert_eq!(property, opentake_ops::KeyframeProperty::Crop);
                assert!(matches!(value, opentake_ops::KeyframeValue::Crop(_)));
            }
            other => panic!("expected UpsertKeyframe, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_effect_commands_and_maps_to_ops_variants() {
        let grade = serde_json::from_str::<EditRequest>(
            r#"{"type":"setColorGrade","clipIds":["clip-1"],"grade":{"exposure":1.0}}"#,
        )
        .expect("setColorGrade camelCase");
        match grade.into_command().expect("setColorGrade command") {
            EditCommand::SetColorGrade { clip_ids, grade } => {
                assert_eq!(clip_ids, vec!["clip-1"]);
                assert_eq!(grade.expect("grade").exposure, 1.0);
            }
            other => panic!("expected SetColorGrade, got {other:?}"),
        }

        let chroma = serde_json::from_str::<EditRequest>(
            r#"{"type":"setChromaKey","clipIds":["clip-1"],"chromaKey":{"similarity":0.2}}"#,
        )
        .expect("setChromaKey camelCase");
        assert!(matches!(
            chroma.into_command().expect("setChromaKey command"),
            EditCommand::SetChromaKey { .. }
        ));

        let masks = serde_json::from_str::<EditRequest>(
            r#"{"type":"setMasks","clipIds":["clip-1"],"masks":[]}"#,
        )
        .expect("setMasks camelCase");
        assert!(matches!(
            masks.into_command().expect("setMasks command"),
            EditCommand::SetMasks { .. }
        ));

        let effects = serde_json::from_str::<EditRequest>(
            r#"{"type":"setEffects","clipIds":["clip-1"],"effects":[{"name":"gaussianBlur","params":{"radius":4.0}}]}"#,
        )
        .expect("setEffects camelCase");
        match effects.into_command().expect("setEffects command") {
            EditCommand::SetEffects { effects, .. } => {
                assert_eq!(effects[0].name, "gaussianBlur");
                assert_eq!(effects[0].param("radius", 0.0), 4.0);
            }
            other => panic!("expected SetEffects, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_media_library_commands_and_maps_to_ops_variants() {
        let rename_media = serde_json::from_str::<EditRequest>(
            r#"{"type":"renameMedia","entries":[{"id":"asset-1","name":"Hero"}]}"#,
        )
        .expect("renameMedia camelCase");
        assert!(matches!(
            rename_media.into_command().expect("renameMedia command"),
            EditCommand::RenameMedia { .. }
        ));

        let rename_folder = serde_json::from_str::<EditRequest>(
            r#"{"type":"renameFolder","entries":[{"id":"folder-1","name":"B-roll"}]}"#,
        )
        .expect("renameFolder camelCase");
        assert!(matches!(
            rename_folder.into_command().expect("renameFolder command"),
            EditCommand::RenameFolder { .. }
        ));

        let delete_media =
            serde_json::from_str::<EditRequest>(r#"{"type":"deleteMedia","assetIds":["asset-1"]}"#)
                .expect("deleteMedia camelCase");
        assert!(matches!(
            delete_media.into_command().expect("deleteMedia command"),
            EditCommand::DeleteMedia { .. }
        ));

        let delete_folder = serde_json::from_str::<EditRequest>(
            r#"{"type":"deleteFolder","folderIds":["folder-1"]}"#,
        )
        .expect("deleteFolder camelCase");
        assert!(matches!(
            delete_folder.into_command().expect("deleteFolder command"),
            EditCommand::DeleteFolder { .. }
        ));
    }
}

#[cfg(test)]
mod subtitle_export_tests {
    use super::{write_subtitles, SubtitleFormat};
    use opentake_domain::{Clip, ClipType, Timeline, Track};

    /// Build a caption clip: text + caption_group_id set, media_type Text — the
    /// two fields `collect_caption_cues` requires to treat a clip as a caption.
    fn caption(id: &str, group: &str, start: i32, dur: i32, text: &str) -> Clip {
        let mut c = Clip::new(id, "caption", start, dur);
        c.media_type = ClipType::Text;
        c.caption_group_id = Some(group.to_string());
        c.text_content = Some(text.to_string());
        c
    }

    /// A timeline with a single caption track holding `clips`, at the given fps.
    fn timeline_with(fps: i32, clips: Vec<Clip>) -> Timeline {
        let mut tl = Timeline::new();
        tl.fps = fps;
        let mut t = Track::new("t-cap", ClipType::Text);
        t.clips = clips;
        tl.tracks.push(t);
        tl
    }

    /// `SubtitleFormat` must deserialize from the lower-case tags the front end
    /// sends (matching the file extension) and default to SRT for bare payloads.
    #[test]
    fn subtitle_format_deserializes_lowercase_tags() {
        assert_eq!(
            serde_json::from_str::<SubtitleFormat>(r#""srt""#).expect("srt"),
            SubtitleFormat::Srt
        );
        assert_eq!(
            serde_json::from_str::<SubtitleFormat>(r#""vtt""#).expect("vtt"),
            SubtitleFormat::Vtt
        );
        assert_eq!(SubtitleFormat::default(), SubtitleFormat::Srt);
    }

    /// The summary returned to the front end must serialize as camelCase
    /// (`outPath` / `cueCount`) so the TS mirror lines up.
    #[test]
    fn summary_serializes_camel_case() {
        let summary = super::SubtitleExportSummary {
            out_path: "/tmp/x.srt".into(),
            cue_count: 2,
        };
        let json = serde_json::to_string(&summary).expect("serialize");
        assert!(json.contains("\"outPath\""), "got: {json}");
        assert!(json.contains("\"cueCount\":2"), "got: {json}");
    }

    /// A timeline carrying caption clips exports a non-empty SRT body with one
    /// numbered cue per caption, and reports the cue count.
    #[test]
    fn exports_non_empty_srt_with_cue_count() {
        let dir = std::env::temp_dir();
        let path = dir
            .join(format!("opentake-subs-{}.srt", std::process::id()))
            .to_string_lossy()
            .into_owned();
        let tl = timeline_with(
            30,
            vec![
                caption("c1", "g1", 30, 30, "Hello"),
                caption("c2", "g1", 60, 30, "World"),
            ],
        );

        let summary =
            write_subtitles(&tl, path.clone(), SubtitleFormat::Srt).expect("srt export ok");
        assert_eq!(summary.cue_count, 2);
        assert_eq!(summary.out_path, path);

        let written = std::fs::read_to_string(&path).expect("read back srt");
        let _ = std::fs::remove_file(&path);
        assert!(written.contains("Hello"));
        assert!(written.contains("World"));
        // SRT uses comma timestamps and 1-based indices.
        assert!(written.starts_with("1\n"), "got: {written:?}");
        assert!(
            written.contains("00:00:01,000 --> 00:00:02,000"),
            "got: {written:?}"
        );
    }

    /// VTT export always opens with the `WEBVTT` header and uses dot timestamps.
    #[test]
    fn exports_vtt_with_header() {
        let dir = std::env::temp_dir();
        let path = dir
            .join(format!("opentake-subs-{}.vtt", std::process::id()))
            .to_string_lossy()
            .into_owned();
        let tl = timeline_with(30, vec![caption("c1", "g1", 30, 30, "Hello")]);

        let summary =
            write_subtitles(&tl, path.clone(), SubtitleFormat::Vtt).expect("vtt export ok");
        assert_eq!(summary.cue_count, 1);

        let written = std::fs::read_to_string(&path).expect("read back vtt");
        let _ = std::fs::remove_file(&path);
        assert!(written.starts_with("WEBVTT\n\n"), "got: {written:?}");
        assert!(
            written.contains("00:00:01.000 --> 00:00:02.000"),
            "got: {written:?}"
        );
    }

    /// A timeline with no caption clips writes a (header-only / empty) file and
    /// reports `cue_count == 0`, the signal the UI uses for its friendly toast.
    #[test]
    fn empty_timeline_reports_zero_cues() {
        let dir = std::env::temp_dir();
        let path = dir
            .join(format!("opentake-subs-empty-{}.srt", std::process::id()))
            .to_string_lossy()
            .into_owned();
        let tl = Timeline::new();

        let summary =
            write_subtitles(&tl, path.clone(), SubtitleFormat::Srt).expect("empty export ok");
        let _ = std::fs::remove_file(&path);
        assert_eq!(summary.cue_count, 0);
    }
}
