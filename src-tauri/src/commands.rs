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

use serde::Deserialize;
use tauri::{AppHandle, Manager, State};

use opentake_core::dto::{
    handle_edit_apply, handle_get_timeline, handle_project_new, handle_project_open,
    handle_project_save, handle_redo, handle_undo, EditResultDto, TimelineSnapshotDto,
};
use opentake_core::{AppCore, CmdError, EditCommand};

use opentake_ops::{
    ClipEntry, ClipMove, ClipProperties, FrameRange, KeyframePayload, KeyframeProperty, TextEntry,
};

use opentake_domain::{
    AnimPair, ClipType, Crop, Interpolation, Keyframe, KeyframeTrack, TextStyle, Transform,
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

/// `export_fcpxml`: write the current timeline to `path` as XMEML 4 (Final Cut
/// Pro 7 XML, `.xml`). Despite the command name, the produced format is XMEML —
/// see `opentake_project::fcpxml` for why (Premiere Pro doesn't read FCPXML
/// natively, so upstream exports XMEML; DaVinci/FCP still import FCP7 XML). Reads
/// the timeline / media manifest / project dir from the core, builds the XML via
/// the pure `export_xmeml`, and writes the file.
#[tauri::command]
pub fn export_fcpxml(core: State<'_, AppCore>, path: String) -> Result<(), String> {
    let timeline = core.get_timeline().timeline;
    let manifest = core.media();
    let project_dir = core.project_dir();
    let xml = opentake_project::export_xmeml(&timeline, &manifest, project_dir.as_deref());
    std::fs::write(&path, xml).map_err(|e| e.to_string())
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
    AddClips {
        entries: Vec<ClipEntryDto>,
    },
    #[serde(rename_all = "camelCase")]
    InsertClips {
        track_index: usize,
        at_frame: i32,
        entries: Vec<ClipEntryDto>,
    },
    #[serde(rename_all = "camelCase")]
    MoveClips {
        moves: Vec<ClipMoveDto>,
    },
    #[serde(rename_all = "camelCase")]
    RemoveClips {
        clip_ids: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    SplitClip {
        clip_id: String,
        at_frame: i32,
    },
    #[serde(rename_all = "camelCase")]
    TrimClips {
        edits: Vec<TrimEditDto>,
    },
    #[serde(rename_all = "camelCase")]
    SetClipProperties {
        clip_ids: Vec<String>,
        properties: ClipPropertiesDto,
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
    RippleDeleteRanges {
        track_index: usize,
        ranges: Vec<FrameRangeDto>,
    },
    #[serde(rename_all = "camelCase")]
    RippleDeleteClips {
        clip_ids: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    AddTexts {
        entries: Vec<TextEntryDto>,
    },
    #[serde(rename_all = "camelCase")]
    Link {
        clip_ids: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    Unlink {
        clip_ids: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    RemoveTracks {
        track_indexes: Vec<usize>,
    },
    #[serde(rename_all = "camelCase")]
    InsertTrack {
        kind: ClipType,
    },
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
    SwapMedia {
        clip_id: String,
        media_ref: String,
    },
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
                properties: properties.into_properties(),
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
            EditRequest::InsertTrack { kind } => EditCommand::InsertTrack { kind },
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

#[cfg(test)]
mod edit_request_serde_tests {
    use super::EditRequest;

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
}
