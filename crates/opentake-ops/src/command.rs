//! The single editing entry point: [`EditCommand`] + [`apply`].
//!
//! UI gestures, the in-app agent, and the external MCP server all funnel through
//! one command enum, so undo / validation / versioning are written once
//! (`ARCHITECTURE.md §5`, upstream `ToolExecutor`).
//!
//! `apply` is the `withTimelineSwap` transaction, generalized to the whole
//! document (timeline + manifest):
//!
//! 1. snapshot the document,
//! 2. run the command's mutation (validation errors abort with no change),
//! 3. if `before != after` (`PartialEq` short-circuit) push the snapshot onto the
//!    undo stack and bump the version,
//! 4. return an [`EditResult`].
//!
//! Ripple refusals (a sync-locked follower can't absorb the shift) abort like a
//! validation error: `Err(EditError::Refused)`, document untouched.

use std::collections::HashSet;

use opentake_domain::{ChromaKey, ClipType, ColorGrade, Effect, Mask, Timeline, Transform};

use crate::editor_state::EditorState;
use crate::engines::FrameRange;
use crate::id::IdGen;
use crate::ops;
use crate::ops::move_clips::ClipMove;
use crate::ops::place::PlaceSpec;
use crate::ops::ripple::RippleOutcome;
use crate::ops::trim::TrimEdit;

/// Why a command did not apply.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EditError {
    /// Input failed validation (bad index, missing clip, empty payload, ...).
    Invalid(String),
    /// A ripple edit was refused to preserve sync-lock alignment.
    Refused(String),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditError::Invalid(m) => write!(f, "{m}"),
            EditError::Refused(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for EditError {}

/// Outcome of a successfully-attempted command. 1:1 shape from `ARCHITECTURE.md §5`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EditResult {
    /// Whether the document actually changed (drives undo-stack push + version bump).
    pub changed: bool,
    /// Undo label, e.g. "Add Clips" / "Ripple Delete".
    pub action_name: String,
    /// Clip ids created or directly affected (for selection / response).
    pub affected_clip_ids: Vec<String>,
    /// Document version after applying (unchanged commands report the prior version).
    pub timeline_version: u64,
    /// Human-readable one-line summary.
    pub summary: String,
}

/// One entry for [`EditCommand::AddClips`] / `InsertClips`.
#[derive(Clone, Debug)]
pub struct ClipEntry {
    pub media_ref: String,
    pub media_type: ClipType,
    pub source_clip_type: ClipType,
    pub track_index: usize,
    pub start_frame: i32,
    pub duration_frames: i32,
    pub trim_start_frame: Option<i32>,
    pub trim_end_frame: Option<i32>,
    pub has_audio: bool,
    pub add_linked_audio: bool,
}

impl ClipEntry {
    fn to_spec(&self) -> PlaceSpec {
        PlaceSpec {
            media_ref: self.media_ref.clone(),
            media_type: self.media_type,
            source_clip_type: self.source_clip_type,
            start_frame: self.start_frame,
            duration_frames: self.duration_frames,
            trim_start_frame: self.trim_start_frame,
            trim_end_frame: self.trim_end_frame,
            has_audio: self.has_audio,
            add_linked_audio: self.add_linked_audio,
        }
    }
}

/// One id + new-name pair for [`EditCommand::RenameMedia`] /
/// [`EditCommand::RenameFolder`]. A single rename is a one-element vec, so the
/// batch and single forms apply in the same undo group (1:1 with upstream's
/// `withUndoGroup`).
#[derive(Clone, Debug)]
pub struct RenameEntry {
    pub id: String,
    pub name: String,
}

/// A text overlay entry for [`EditCommand::AddTexts`]. The transform is supplied
/// fully resolved (text measurement is a media/UI concern this leaf doesn't do).
#[derive(Clone, Debug)]
pub struct TextEntry {
    pub track_index: usize,
    pub start_frame: i32,
    pub duration_frames: i32,
    pub content: String,
    pub text_style: opentake_domain::TextStyle,
    pub transform: Transform,
}

/// A single clip property assignment for [`EditCommand::SetClipProperties`].
/// `None` fields are left unchanged; setting a scalar clears the matching
/// keyframe track (mirrors `applyPropertyChanges`).
#[derive(Clone, Debug, Default)]
pub struct ClipProperties {
    pub duration_frames: Option<i32>,
    pub trim_start_frame: Option<i32>,
    pub trim_end_frame: Option<i32>,
    pub speed: Option<f64>,
    pub volume: Option<f64>,
    pub opacity: Option<f64>,
    pub transform: Option<Transform>,
    pub text_content: Option<String>,
}

/// Which keyframe track [`EditCommand::SetKeyframes`] targets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyframeProperty {
    Opacity,
    Volume,
    Rotation,
    Position,
    Scale,
    Crop,
}

/// A keyframe payload for [`EditCommand::SetKeyframes`]. Exactly one variant is
/// used per command, matching `property`.
#[derive(Clone, Debug)]
pub enum KeyframePayload {
    Scalar(opentake_domain::KeyframeTrack<f64>),
    Pair(opentake_domain::KeyframeTrack<opentake_domain::AnimPair>),
    Crop(opentake_domain::KeyframeTrack<opentake_domain::Crop>),
}

/// The unified editing command. Every editing surface routes through this.
#[derive(Clone, Debug)]
pub enum EditCommand {
    /// Overwrite-place clips (clears each destination range first).
    AddClips { entries: Vec<ClipEntry> },
    /// Ripple-insert clips at `at_frame`, pushing later clips right.
    InsertClips {
        track_index: usize,
        at_frame: i32,
        entries: Vec<ClipEntry>,
    },
    /// Move clips (expanded to linked partners by the caller) to new tracks/frames.
    MoveClips { moves: Vec<ClipMove> },
    /// Remove clips (expanded to linked partners), pruning emptied tracks.
    RemoveClips { clip_ids: Vec<String> },
    /// Split a clip at a frame (splits linked partners too).
    SplitClip { clip_id: String, at_frame: i32 },
    /// Overwrite-style trim: resize clips in place from new source-frame trims.
    TrimClips { edits: Vec<TrimEdit> },
    /// Assign clip properties (timing changes propagate to linked partners).
    SetClipProperties {
        clip_ids: Vec<String>,
        properties: ClipProperties,
    },
    /// Replace (or clear) a clip's keyframe track for one property.
    SetKeyframes {
        clip_id: String,
        property: KeyframeProperty,
        payload: KeyframePayload,
    },
    /// Set (or clear with `None`) the color grade on one or more clips.
    SetColorGrade {
        clip_ids: Vec<String>,
        grade: Option<ColorGrade>,
    },
    /// Set (or clear with `None`) the chroma key on one or more clips.
    SetChromaKey {
        clip_ids: Vec<String>,
        chroma_key: Option<ChromaKey>,
    },
    /// Replace the mask list on one or more clips (empty clears all masks).
    SetMasks {
        clip_ids: Vec<String>,
        masks: Vec<Mask>,
    },
    /// Replace the effect chain on one or more clips (empty clears all effects).
    SetEffects {
        clip_ids: Vec<String>,
        effects: Vec<Effect>,
    },
    /// Ripple-delete project-frame ranges on a track, closing the gaps.
    RippleDeleteRanges {
        track_index: usize,
        ranges: Vec<FrameRange>,
    },
    /// Ripple-delete a set of selected clips, closing the gaps and shifting
    /// sync-locked followers (refuses on a follower collision).
    RippleDeleteClips { clip_ids: Vec<String> },
    /// Add text overlays.
    AddTexts { entries: Vec<TextEntry> },
    /// Link clips into one group.
    Link { clip_ids: Vec<String> },
    /// Unlink clips (and their whole groups).
    Unlink { clip_ids: Vec<String> },
    /// Remove tracks by index.
    RemoveTracks { track_indexes: Vec<usize> },
    /// Insert a new empty track of `kind` (clamped into its zone). Lets the drop
    /// flow create a track on demand when the timeline has no compatible one
    /// (upstream `placeClip` / `add_clips` with omitted `trackIndex` →
    /// `insertTrack`), so dragging media onto an empty timeline produces a clip.
    InsertTrack { kind: ClipType },
    /// Toggle track-head properties (mute / hide / sync-lock). `None` leaves a
    /// field unchanged. 1:1 with the upstream track-header toggles.
    SetTrackProps {
        track_index: usize,
        muted: Option<bool>,
        hidden: Option<bool>,
        sync_locked: Option<bool>,
    },
    /// Create a media-library folder.
    CreateFolder {
        name: String,
        parent_folder_id: Option<String>,
    },
    /// Move media assets into a folder (or to root with `None`).
    MoveToFolder {
        asset_ids: Vec<String>,
        folder_id: Option<String>,
    },
    /// Rename media assets (single = one-element vec). Library-only; clip
    /// references are unaffected.
    RenameMedia { entries: Vec<RenameEntry> },
    /// Rename folders (single = one-element vec).
    RenameFolder { entries: Vec<RenameEntry> },
    /// Delete media assets and cascade-remove any clips referencing them.
    DeleteMedia { asset_ids: Vec<String> },
    /// Delete folders recursively (subfolders + their assets) and cascade-remove
    /// clips referencing any deleted asset.
    DeleteFolder { folder_ids: Vec<String> },
    /// Undo the last committed command.
    Undo,
    /// Redo the last undone command.
    Redo,
}

/// Apply `command` to `state`, minting any new ids from `ids`. See the module
/// docs for the transaction model.
pub fn apply(
    state: &mut EditorState,
    command: EditCommand,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    match command {
        EditCommand::Undo => {
            let changed = state.undo();
            Ok(result(
                state,
                changed,
                "Undo",
                Vec::new(),
                if changed {
                    "Undid last edit"
                } else {
                    "Nothing to undo"
                },
            ))
        }
        EditCommand::Redo => {
            let changed = state.redo();
            Ok(result(
                state,
                changed,
                "Redo",
                Vec::new(),
                if changed {
                    "Redid last edit"
                } else {
                    "Nothing to redo"
                },
            ))
        }

        EditCommand::AddClips { entries } => add_clips(state, entries, ids),
        EditCommand::InsertClips {
            track_index,
            at_frame,
            entries,
        } => insert_clips(state, track_index, at_frame, entries, ids),
        EditCommand::MoveClips { moves } => move_clips(state, moves, ids),
        EditCommand::RemoveClips { clip_ids } => remove_clips(state, clip_ids),
        EditCommand::SplitClip { clip_id, at_frame } => split(state, clip_id, at_frame, ids),
        EditCommand::TrimClips { edits } => trim(state, edits),
        EditCommand::SetClipProperties {
            clip_ids,
            properties,
        } => set_clip_properties(state, clip_ids, properties),
        EditCommand::SetKeyframes {
            clip_id,
            property,
            payload,
        } => set_keyframes(state, clip_id, property, payload),
        EditCommand::SetColorGrade { clip_ids, grade } => set_color_grade(state, clip_ids, grade),
        EditCommand::SetChromaKey {
            clip_ids,
            chroma_key,
        } => set_chroma_key(state, clip_ids, chroma_key),
        EditCommand::SetMasks { clip_ids, masks } => set_masks(state, clip_ids, masks),
        EditCommand::SetEffects { clip_ids, effects } => set_effects(state, clip_ids, effects),
        EditCommand::RippleDeleteRanges {
            track_index,
            ranges,
        } => ripple_delete_ranges(state, track_index, ranges, ids),
        EditCommand::RippleDeleteClips { clip_ids } => ripple_delete_clips(state, clip_ids),
        EditCommand::AddTexts { entries } => add_texts(state, entries, ids),
        EditCommand::Link { clip_ids } => link(state, clip_ids, ids),
        EditCommand::Unlink { clip_ids } => unlink(state, clip_ids),
        EditCommand::RemoveTracks { track_indexes } => remove_tracks(state, track_indexes),
        EditCommand::InsertTrack { kind } => insert_track_cmd(state, kind, ids),
        EditCommand::SetTrackProps {
            track_index,
            muted,
            hidden,
            sync_locked,
        } => set_track_props(state, track_index, muted, hidden, sync_locked),
        EditCommand::CreateFolder {
            name,
            parent_folder_id,
        } => create_folder(state, name, parent_folder_id, ids),
        EditCommand::MoveToFolder {
            asset_ids,
            folder_id,
        } => move_to_folder(state, asset_ids, folder_id),
        EditCommand::RenameMedia { entries } => rename_media(state, entries),
        EditCommand::RenameFolder { entries } => rename_folder(state, entries),
        EditCommand::DeleteMedia { asset_ids } => delete_media(state, asset_ids),
        EditCommand::DeleteFolder { folder_ids } => delete_folder(state, folder_ids),
    }
}

// MARK: - Transaction helper

/// Run `work` inside a transaction: snapshot, mutate, commit-if-changed. `work`
/// returns the affected clip ids on success. Validation/refusal errors propagate
/// without committing.
fn transact(
    state: &mut EditorState,
    action_name: &str,
    summarize: impl FnOnce(&[String]) -> String,
    work: impl FnOnce(&mut EditorState) -> Result<Vec<String>, EditError>,
) -> Result<EditResult, EditError> {
    let before = state.snapshot();
    let affected = work(state)?;
    let after = state.snapshot();
    let changed = before != after;
    if changed {
        state.commit(before);
    }
    let summary = summarize(&affected);
    Ok(result(state, changed, action_name, affected, &summary))
}

fn result(
    state: &EditorState,
    changed: bool,
    action_name: &str,
    affected: Vec<String>,
    summary: &str,
) -> EditResult {
    EditResult {
        changed,
        action_name: action_name.to_string(),
        affected_clip_ids: affected,
        timeline_version: state.version(),
        summary: summary.to_string(),
    }
}

// MARK: - Command implementations

fn add_clips(
    state: &mut EditorState,
    entries: Vec<ClipEntry>,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    if entries.is_empty() {
        return Err(EditError::Invalid(
            "Missing or empty 'entries' array".into(),
        ));
    }
    for (i, e) in entries.iter().enumerate() {
        validate_entry(state, e, i)?;
    }
    let action_name = if entries.len() == 1 {
        "Add Clip"
    } else {
        "Add Clips"
    };
    transact(
        state,
        action_name,
        |added| format!("Added {} clip(s): {}", added.len(), added.join(", ")),
        |st| {
            let mut added = Vec::new();
            for e in &entries {
                let track_id = st.timeline.tracks[e.track_index].id.clone();
                // Pin by id: clearRegion may prune/shift indices.
                if let Some(ti) = st.track_index(&track_id) {
                    ops::clear_region(
                        &mut st.timeline,
                        ti,
                        e.start_frame,
                        e.start_frame + e.duration_frames,
                        false,
                        ids,
                    );
                }
                if let Some(ti) = st.track_index(&track_id) {
                    let placed = ops::place_clip(&mut st.timeline, &e.to_spec(), ti, None, ids);
                    added.extend(placed);
                }
            }
            ops::prune_empty_tracks(&mut st.timeline);
            Ok(added)
        },
    )
}

fn insert_track_cmd(
    state: &mut EditorState,
    kind: ClipType,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    transact(
        state,
        "Insert Track",
        |added| format!("Inserted track: {}", added.join(", ")),
        |st| {
            // Append at the end; `insert_track` clamps into the kind's zone
            // (visual above audio).
            let at = st.timeline.tracks.len();
            let idx = ops::insert_track(&mut st.timeline, at, kind, ids);
            Ok(vec![st.timeline.tracks[idx].id.clone()])
        },
    )
}

fn set_track_props(
    state: &mut EditorState,
    track_index: usize,
    muted: Option<bool>,
    hidden: Option<bool>,
    sync_locked: Option<bool>,
) -> Result<EditResult, EditError> {
    if track_index >= state.timeline.tracks.len() {
        return Err(EditError::Invalid(format!(
            "trackIndex {track_index} out of range"
        )));
    }
    transact(
        state,
        "Set Track Properties",
        |_| "Updated track properties".to_string(),
        |st| {
            let track = &mut st.timeline.tracks[track_index];
            if let Some(m) = muted {
                track.muted = m;
            }
            if let Some(h) = hidden {
                track.hidden = h;
            }
            if let Some(s) = sync_locked {
                track.sync_locked = s;
            }
            Ok(Vec::new())
        },
    )
}

fn insert_clips(
    state: &mut EditorState,
    track_index: usize,
    at_frame: i32,
    entries: Vec<ClipEntry>,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    if entries.is_empty() {
        return Err(EditError::Invalid(
            "Missing or empty 'entries' array".into(),
        ));
    }
    if track_index >= state.timeline.tracks.len() {
        return Err(EditError::Invalid(format!(
            "trackIndex {track_index} out of range"
        )));
    }
    if at_frame < 0 {
        return Err(EditError::Invalid(format!(
            "atFrame must be >= 0 (got {at_frame})"
        )));
    }
    let target_type = state.timeline.tracks[track_index].kind;
    for (i, e) in entries.iter().enumerate() {
        if !e.source_clip_type.is_compatible(target_type) {
            return Err(EditError::Invalid(format!(
                "entries[{i}]: asset type is not compatible with the target track"
            )));
        }
        if e.duration_frames < 1 {
            return Err(EditError::Invalid(format!(
                "entries[{i}]: durationFrames must be >= 1 (got {})",
                e.duration_frames
            )));
        }
    }
    let specs: Vec<PlaceSpec> = entries.iter().map(|e| e.to_spec()).collect();
    let action_name = if entries.len() == 1 {
        "Ripple Insert Clip"
    } else {
        "Ripple Insert Clips"
    };
    transact(
        state,
        action_name,
        |c| format!("Inserted {} clip(s): {}", c.len(), c.join(", ")),
        |st| {
            Ok(ops::ripple::ripple_insert(
                &mut st.timeline,
                &specs,
                track_index,
                at_frame,
                ids,
            ))
        },
    )
}

fn move_clips(
    state: &mut EditorState,
    moves: Vec<ClipMove>,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    if moves.is_empty() {
        return Err(EditError::Invalid("Missing or empty 'moves' array".into()));
    }
    let action_name = if moves.len() == 1 {
        "Move Clip"
    } else {
        "Move Clips"
    };
    let moved_ids: Vec<String> = moves.iter().map(|m| m.clip_id.clone()).collect();
    transact(
        state,
        action_name,
        move |_| format!("Moved {} clip(s)", moved_ids.len()),
        |st| {
            ops::move_clips(&mut st.timeline, &moves, ids);
            Ok(moves.iter().map(|m| m.clip_id.clone()).collect())
        },
    )
}

fn remove_clips(state: &mut EditorState, clip_ids: Vec<String>) -> Result<EditResult, EditError> {
    if clip_ids.is_empty() {
        return Err(EditError::Invalid(
            "Missing or empty 'clipIds' array".into(),
        ));
    }
    for id in &clip_ids {
        if state.find_clip(id).is_none() {
            return Err(EditError::Invalid(format!("Clip not found: {id}")));
        }
    }
    let expanded = ops::expand_to_link_group(&state.timeline, &clip_ids.iter().cloned().collect());
    let count = expanded.len();
    transact(
        state,
        "Remove Clip",
        move |_| format!("Removed {count} clip(s)"),
        |st| {
            for id in &expanded {
                ops::clear_region::remove_clip(&mut st.timeline, id);
            }
            ops::prune_empty_tracks(&mut st.timeline);
            Ok(expanded.iter().cloned().collect())
        },
    )
    .map(|mut r| {
        // "Remove Clip"/"Remove Clips" matches upstream pluralization on the expanded set.
        r.action_name = if count == 1 {
            "Remove Clip"
        } else {
            "Remove Clips"
        }
        .to_string();
        r
    })
}

fn split(
    state: &mut EditorState,
    clip_id: String,
    at_frame: i32,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    let Some(loc) = state.find_clip(&clip_id) else {
        return Err(EditError::Invalid(format!("Clip not found: {clip_id}")));
    };
    let clip = &state.timeline.tracks[loc.track_index].clips[loc.clip_index];
    if !(at_frame > clip.start_frame && at_frame < clip.end_frame()) {
        return Err(EditError::Invalid(format!(
            "Frame {at_frame} is outside clip range ({}..{})",
            clip.start_frame,
            clip.end_frame()
        )));
    }
    let linked = clip.link_group_id.is_some();
    transact(
        state,
        if linked { "Split Clips" } else { "Split Clip" },
        |rights| {
            if rights.is_empty() {
                "Split (no-op)".to_string()
            } else {
                format!("Split at {at_frame} -> {}", rights.join(", "))
            }
        },
        |st| Ok(ops::split_clip(&mut st.timeline, &clip_id, at_frame, ids)),
    )
}

fn trim(state: &mut EditorState, edits: Vec<TrimEdit>) -> Result<EditResult, EditError> {
    if edits.is_empty() {
        return Err(EditError::Invalid("Missing or empty trim edits".into()));
    }
    for (id, _, _) in &edits {
        if state.find_clip(id).is_none() {
            return Err(EditError::Invalid(format!("Clip not found: {id}")));
        }
    }
    let n = edits.len();
    transact(
        state,
        if n == 1 { "Trim Clip" } else { "Trim Clips" },
        move |_| format!("Trimmed {n} clip(s)"),
        |st| {
            ops::trim_clips(&mut st.timeline, &edits);
            Ok(edits.iter().map(|(id, _, _)| id.clone()).collect())
        },
    )
}

fn set_clip_properties(
    state: &mut EditorState,
    clip_ids: Vec<String>,
    props: ClipProperties,
) -> Result<EditResult, EditError> {
    if clip_ids.is_empty() {
        return Err(EditError::Invalid(
            "Missing or empty 'clipIds' array".into(),
        ));
    }
    for id in &clip_ids {
        if state.find_clip(id).is_none() {
            return Err(EditError::Invalid(format!("Clip not found: {id}")));
        }
    }
    if let Some(df) = props.duration_frames {
        if df < 1 {
            return Err(EditError::Invalid(format!(
                "durationFrames must be >= 1 (got {df})"
            )));
        }
    }
    // Timing changes propagate to linked partners (trim/speed dropped for text).
    let propagates_timing = props.duration_frames.is_some()
        || props.trim_start_frame.is_some()
        || props.trim_end_frame.is_some()
        || props.speed.is_some();
    let partners: HashSet<String> = if propagates_timing {
        ops::timing_propagation_partners(&state.timeline, &clip_ids.iter().cloned().collect())
    } else {
        HashSet::new()
    };
    let n = clip_ids.len();
    transact(
        state,
        if n == 1 {
            "Set Clip Property"
        } else {
            "Set Clip Properties"
        },
        move |_| format!("Updated {n} clip(s)"),
        |st| {
            for id in &clip_ids {
                apply_property_changes(&mut st.timeline, id, &props, false);
            }
            for pid in &partners {
                let is_text = st
                    .find_clip(pid)
                    .map(|l| st.timeline.tracks[l.track_index].clips[l.clip_index].media_type)
                    == Some(ClipType::Text);
                // Partners receive only timing (and drop it when text).
                let partner_props = ClipProperties {
                    duration_frames: if is_text { None } else { props.duration_frames },
                    trim_start_frame: if is_text {
                        None
                    } else {
                        props.trim_start_frame
                    },
                    trim_end_frame: if is_text { None } else { props.trim_end_frame },
                    speed: if is_text { None } else { props.speed },
                    ..Default::default()
                };
                apply_property_changes(&mut st.timeline, pid, &partner_props, true);
            }
            Ok(clip_ids.clone())
        },
    )
}

/// Apply a property bundle to one clip in place. `partner` marks the call as a
/// linked-partner propagation (only timing fields are set then). 1:1 port of
/// `applyPropertyChanges`.
fn apply_property_changes(
    timeline: &mut Timeline,
    clip_id: &str,
    props: &ClipProperties,
    _partner: bool,
) {
    let Some((ti, ci)) = find(timeline, clip_id) else {
        return;
    };
    let clip = &mut timeline.tracks[ti].clips[ci];

    if let Some(v) = props.duration_frames {
        clip.duration_frames = v;
        clip.clamp_keyframes_to_duration();
        clip.clamp_fades_to_duration();
    }
    if let Some(v) = props.trim_start_frame {
        clip.trim_start_frame = v;
    }
    if let Some(v) = props.trim_end_frame {
        clip.trim_end_frame = v;
    }
    if let Some(v) = props.speed {
        // When no explicit duration is given, recompute duration so the same
        // source span plays at the new speed (mirrors applyPropertyChanges).
        if props.duration_frames.is_none() && v > 0.0 {
            let source_consumed = clip.duration_frames as f64 * clip.speed;
            clip.duration_frames = (1).max((source_consumed / v).round() as i32);
            clip.clamp_keyframes_to_duration();
            clip.clamp_fades_to_duration();
        }
        clip.speed = v;
    }
    // Setting a scalar clears the matching keyframe track.
    if let Some(v) = props.volume {
        clip.volume = v;
        clip.volume_track = None;
    }
    if let Some(v) = props.opacity {
        clip.opacity = v;
        clip.opacity_track = None;
    }
    if let Some(t) = props.transform {
        clip.transform = t;
    }
    if let Some(c) = &props.text_content {
        clip.text_content = Some(c.clone());
    }
}

fn set_keyframes(
    state: &mut EditorState,
    clip_id: String,
    property: KeyframeProperty,
    payload: KeyframePayload,
) -> Result<EditResult, EditError> {
    if state.find_clip(&clip_id).is_none() {
        return Err(EditError::Invalid(format!("Clip not found: {clip_id}")));
    }
    // Type/property agreement check.
    let ok = matches!(
        (property, &payload),
        (KeyframeProperty::Opacity, KeyframePayload::Scalar(_))
            | (KeyframeProperty::Volume, KeyframePayload::Scalar(_))
            | (KeyframeProperty::Rotation, KeyframePayload::Scalar(_))
            | (KeyframeProperty::Position, KeyframePayload::Pair(_))
            | (KeyframeProperty::Scale, KeyframePayload::Pair(_))
            | (KeyframeProperty::Crop, KeyframePayload::Crop(_))
    );
    if !ok {
        return Err(EditError::Invalid(
            "keyframe payload type does not match property".into(),
        ));
    }
    let summary = format!("Set keyframes on {clip_id}");
    transact(
        state,
        "Set Keyframes",
        move |_| summary,
        move |st| {
            let loc = st.find_clip(&clip_id).expect("validated above");
            let clip = &mut st.timeline.tracks[loc.track_index].clips[loc.clip_index];
            match (property, payload) {
                (KeyframeProperty::Opacity, KeyframePayload::Scalar(t)) => {
                    clip.opacity_track = empty_to_none(t)
                }
                (KeyframeProperty::Volume, KeyframePayload::Scalar(t)) => {
                    clip.volume_track = empty_to_none(t)
                }
                (KeyframeProperty::Rotation, KeyframePayload::Scalar(t)) => {
                    clip.rotation_track = empty_to_none(t)
                }
                (KeyframeProperty::Position, KeyframePayload::Pair(t)) => {
                    clip.position_track = empty_to_none(t)
                }
                (KeyframeProperty::Scale, KeyframePayload::Pair(t)) => {
                    clip.scale_track = empty_to_none(t)
                }
                (KeyframeProperty::Crop, KeyframePayload::Crop(t)) => {
                    clip.crop_track = empty_to_none(t)
                }
                _ => unreachable!("validated above"),
            }
            Ok(vec![loc_clip_id(st, loc)])
        },
    )
}

// MARK: - Advanced pixel-effect commands (A-tier)
//
// These set per-clip visual fields (color grade / chroma key / masks / effects).
// Like volume/opacity/transform in `set_clip_properties`, they are per-clip and
// do NOT propagate to linked partners. Each validates its `clip_ids` and runs
// inside the shared `withTimelineSwap` transaction (snapshot -> mutate ->
// commit-if-changed + version bump), so undo/redo and versioning come for free.

/// Validate that `clip_ids` is non-empty and every id resolves, then run `mutate`
/// for each clip inside one transaction. Shared by the four effect setters.
fn set_clip_effect_field(
    state: &mut EditorState,
    clip_ids: Vec<String>,
    action_name: &'static str,
    mutate: impl Fn(&mut opentake_domain::Clip),
) -> Result<EditResult, EditError> {
    if clip_ids.is_empty() {
        return Err(EditError::Invalid(
            "Missing or empty 'clipIds' array".into(),
        ));
    }
    for id in &clip_ids {
        if state.find_clip(id).is_none() {
            return Err(EditError::Invalid(format!("Clip not found: {id}")));
        }
    }
    let n = clip_ids.len();
    transact(
        state,
        action_name,
        move |_| format!("Updated {n} clip(s)"),
        move |st| {
            for id in &clip_ids {
                if let Some((ti, ci)) = find(&st.timeline, id) {
                    mutate(&mut st.timeline.tracks[ti].clips[ci]);
                }
            }
            Ok(clip_ids.clone())
        },
    )
}

fn set_color_grade(
    state: &mut EditorState,
    clip_ids: Vec<String>,
    grade: Option<ColorGrade>,
) -> Result<EditResult, EditError> {
    set_clip_effect_field(state, clip_ids, "Set Color Grade", move |clip| {
        clip.color_grade = grade;
    })
}

fn set_chroma_key(
    state: &mut EditorState,
    clip_ids: Vec<String>,
    chroma_key: Option<ChromaKey>,
) -> Result<EditResult, EditError> {
    set_clip_effect_field(state, clip_ids, "Set Chroma Key", move |clip| {
        clip.chroma_key = chroma_key;
    })
}

fn set_masks(
    state: &mut EditorState,
    clip_ids: Vec<String>,
    masks: Vec<Mask>,
) -> Result<EditResult, EditError> {
    set_clip_effect_field(state, clip_ids, "Set Masks", move |clip| {
        clip.masks = masks.clone();
    })
}

fn set_effects(
    state: &mut EditorState,
    clip_ids: Vec<String>,
    effects: Vec<Effect>,
) -> Result<EditResult, EditError> {
    set_clip_effect_field(state, clip_ids, "Set Effects", move |clip| {
        clip.effects = effects.clone();
    })
}

fn ripple_delete_ranges(
    state: &mut EditorState,
    track_index: usize,
    ranges: Vec<FrameRange>,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    if ranges.is_empty() {
        return Err(EditError::Invalid("Missing or empty 'ranges' array".into()));
    }
    if track_index >= state.timeline.tracks.len() {
        return Err(EditError::Invalid(format!(
            "Track index out of range: {track_index}"
        )));
    }
    // Run the op outside transact so a refusal aborts before any snapshot/commit.
    let before = state.snapshot();
    let outcome = ops::ripple::ripple_delete_ranges_on_track(
        &mut state.timeline,
        track_index,
        &ranges,
        &track_display_label,
        ids,
    );
    match outcome {
        RippleOutcome::Refused(reason) => {
            // Restore in case clear_region partially mutated before a later refusal
            // (it can't here — refusal is dry-run first — but keep it airtight).
            state.restore(before);
            Err(EditError::Refused(reason))
        }
        RippleOutcome::Ok(report) => {
            let after = state.snapshot();
            let changed = before != after;
            if changed {
                state.commit(before);
            }
            let summary = format!(
                "Removed {} frame(s) across {} track(s), shifted {} clip(s)",
                report.removed_frames, report.cleared_tracks, report.shifted_clips
            );
            let affected: Vec<String> = report
                .resulting_fragments
                .iter()
                .map(|f| f.0.clone())
                .collect();
            Ok(result(state, changed, "Ripple Delete", affected, &summary))
        }
    }
}

/// Ripple-delete the selected clips (and their link groups), closing the gaps
/// and shifting sync-locked followers. Refuses (no mutation) if a follower would
/// collide. 1:1 with upstream `rippleDeleteSelectedClips`.
fn ripple_delete_clips(
    state: &mut EditorState,
    clip_ids: Vec<String>,
) -> Result<EditResult, EditError> {
    if clip_ids.is_empty() {
        return Err(EditError::Invalid(
            "Missing or empty 'clipIds' array".into(),
        ));
    }
    for id in &clip_ids {
        if state.find_clip(id).is_none() {
            return Err(EditError::Invalid(format!("Clip not found: {id}")));
        }
    }
    // Selecting one of a linked pair deletes the whole group (upstream selection).
    let id_set = ops::expand_to_link_group(&state.timeline, &clip_ids.into_iter().collect());
    let before = state.snapshot();
    match ops::ripple::ripple_delete(&mut state.timeline, &id_set, &track_display_label) {
        Err(reason) => {
            state.restore(before);
            Err(EditError::Refused(reason))
        }
        Ok(()) => {
            let after = state.snapshot();
            let changed = before != after;
            if changed {
                state.commit(before);
            }
            let affected: Vec<String> = id_set.iter().cloned().collect();
            let n = affected.len();
            Ok(result(
                state,
                changed,
                "Ripple Delete",
                affected,
                &format!("Ripple-deleted {n} clip(s)"),
            ))
        }
    }
}

fn add_texts(
    state: &mut EditorState,
    entries: Vec<TextEntry>,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    if entries.is_empty() {
        return Err(EditError::Invalid(
            "Missing or empty 'entries' array".into(),
        ));
    }
    for (i, e) in entries.iter().enumerate() {
        if e.track_index >= state.timeline.tracks.len() {
            return Err(EditError::Invalid(format!(
                "entries[{i}]: track index {} out of range",
                e.track_index
            )));
        }
        if !ClipType::Text.is_compatible(state.timeline.tracks[e.track_index].kind) {
            return Err(EditError::Invalid(format!(
                "entries[{i}]: track {} is an audio track; text requires a visual track",
                e.track_index
            )));
        }
        if e.duration_frames < 1 {
            return Err(EditError::Invalid(format!(
                "entries[{i}]: durationFrames must be >= 1 (got {})",
                e.duration_frames
            )));
        }
        if e.start_frame < 0 {
            return Err(EditError::Invalid(format!(
                "entries[{i}]: startFrame must be >= 0 (got {})",
                e.start_frame
            )));
        }
    }
    let action_name = if entries.len() == 1 {
        "Add Text"
    } else {
        "Add Texts"
    };
    transact(
        state,
        action_name,
        |c| format!("Added {} text clip(s): {}", c.len(), c.join(", ")),
        |st| {
            let mut added = Vec::new();
            for e in &entries {
                let track_id = st.timeline.tracks[e.track_index].id.clone();
                if let Some(ti) = st.track_index(&track_id) {
                    ops::clear_region(
                        &mut st.timeline,
                        ti,
                        e.start_frame,
                        e.start_frame + e.duration_frames,
                        false,
                        ids,
                    );
                }
                if let Some(ti) = st.track_index(&track_id) {
                    let mut clip = opentake_domain::Clip::new(
                        ids.next_id(),
                        "",
                        e.start_frame,
                        e.duration_frames,
                    );
                    clip.media_type = ClipType::Text;
                    clip.source_clip_type = ClipType::Text;
                    clip.transform = e.transform;
                    clip.text_content = Some(e.content.clone());
                    clip.text_style = Some(e.text_style.clone());
                    added.push(clip.id.clone());
                    st.timeline.tracks[ti].clips.push(clip);
                    ops::sort_clips(&mut st.timeline.tracks[ti]);
                }
            }
            Ok(added)
        },
    )
}

fn link(
    state: &mut EditorState,
    clip_ids: Vec<String>,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    if clip_ids.len() < 2 {
        return Err(EditError::Invalid("Link requires at least 2 clips".into()));
    }
    for id in &clip_ids {
        if state.find_clip(id).is_none() {
            return Err(EditError::Invalid(format!("Clip not found: {id}")));
        }
    }
    let set: HashSet<String> = clip_ids.iter().cloned().collect();
    transact(
        state,
        "Link",
        |_| "Linked clips".to_string(),
        |st| {
            let new_group = ids.next_id();
            for t in &mut st.timeline.tracks {
                for c in &mut t.clips {
                    if set.contains(&c.id) {
                        c.link_group_id = Some(new_group.clone());
                    }
                }
            }
            Ok(set.iter().cloned().collect())
        },
    )
}

fn unlink(state: &mut EditorState, clip_ids: Vec<String>) -> Result<EditResult, EditError> {
    if clip_ids.is_empty() {
        return Err(EditError::Invalid(
            "Missing or empty 'clipIds' array".into(),
        ));
    }
    let expanded = ops::expand_to_link_group(&state.timeline, &clip_ids.iter().cloned().collect());
    transact(
        state,
        "Unlink",
        |_| "Unlinked clips".to_string(),
        |st| {
            for t in &mut st.timeline.tracks {
                for c in &mut t.clips {
                    if expanded.contains(&c.id) {
                        c.link_group_id = None;
                    }
                }
            }
            Ok(expanded.iter().cloned().collect())
        },
    )
}

fn remove_tracks(
    state: &mut EditorState,
    track_indexes: Vec<usize>,
) -> Result<EditResult, EditError> {
    if track_indexes.is_empty() {
        return Err(EditError::Invalid(
            "trackIndexes must be a non-empty array".into(),
        ));
    }
    // Resolve indexes to ids first (indexes shift as we remove).
    let mut seen = HashSet::new();
    let mut ids_to_remove = Vec::new();
    for &i in &track_indexes {
        if !seen.insert(i) {
            continue;
        }
        if i >= state.timeline.tracks.len() {
            return Err(EditError::Invalid(format!(
                "track index {i} out of range (timeline has {} tracks)",
                state.timeline.tracks.len()
            )));
        }
        ids_to_remove.push(state.timeline.tracks[i].id.clone());
    }
    let n = ids_to_remove.len();
    transact(
        state,
        if n == 1 {
            "Remove Track"
        } else {
            "Remove Tracks"
        },
        move |_| format!("Removed {n} track(s)"),
        |st| {
            ops::remove_tracks(&mut st.timeline, &ids_to_remove);
            Ok(Vec::new())
        },
    )
}

fn create_folder(
    state: &mut EditorState,
    name: String,
    parent_folder_id: Option<String>,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    if name.is_empty() {
        return Err(EditError::Invalid("folder name is required".into()));
    }
    transact(
        state,
        "New Folder",
        |c| {
            c.first()
                .map(|id| format!("Created folder {id}"))
                .unwrap_or_else(|| "Created folder".to_string())
        },
        |st| {
            let id = ops::create_folder(
                &mut st.manifest,
                name.clone(),
                parent_folder_id.clone(),
                ids,
            );
            Ok(vec![id])
        },
    )
}

fn move_to_folder(
    state: &mut EditorState,
    asset_ids: Vec<String>,
    folder_id: Option<String>,
) -> Result<EditResult, EditError> {
    if asset_ids.is_empty() {
        return Err(EditError::Invalid("assetIds is required".into()));
    }
    let n = asset_ids.len();
    transact(
        state,
        "Move to Folder",
        move |_| format!("Moved {n} asset(s)"),
        |st| {
            ops::move_to_folder(
                &mut st.manifest,
                &asset_ids.iter().cloned().collect(),
                folder_id.clone(),
            );
            Ok(Vec::new())
        },
    )
}

fn rename_media(
    state: &mut EditorState,
    entries: Vec<RenameEntry>,
) -> Result<EditResult, EditError> {
    if entries.is_empty() {
        return Err(EditError::Invalid(
            "rename_media: no entries to rename".into(),
        ));
    }
    // Atomic: every target must exist before any rename is applied.
    for e in &entries {
        if !state.manifest.entries.iter().any(|m| m.id == e.id) {
            return Err(EditError::Invalid(format!(
                "Media asset not found: {}",
                e.id
            )));
        }
    }
    let single = (entries.len() == 1).then(|| (entries[0].id.clone(), entries[0].name.clone()));
    let n = entries.len();
    let action = if n == 1 {
        "Rename Asset"
    } else {
        "Rename Assets"
    };
    transact(
        state,
        action,
        move |_| match &single {
            Some((id, name)) => format!("Renamed {id} to '{name}'"),
            None => format!("Renamed {n} media asset(s)"),
        },
        |st| {
            for e in &entries {
                ops::rename_media(&mut st.manifest, &e.id, e.name.clone());
            }
            Ok(Vec::new())
        },
    )
}

fn rename_folder(
    state: &mut EditorState,
    entries: Vec<RenameEntry>,
) -> Result<EditResult, EditError> {
    if entries.is_empty() {
        return Err(EditError::Invalid(
            "rename_folder: no entries to rename".into(),
        ));
    }
    for e in &entries {
        if !state.manifest.folders.iter().any(|f| f.id == e.id) {
            return Err(EditError::Invalid(format!("folderId not found: {}", e.id)));
        }
    }
    let single = (entries.len() == 1).then(|| (entries[0].id.clone(), entries[0].name.clone()));
    let n = entries.len();
    let action = if n == 1 {
        "Rename Folder"
    } else {
        "Rename Folders"
    };
    transact(
        state,
        action,
        move |_| match &single {
            Some((id, name)) => format!("Renamed folder {id} to '{name}'"),
            None => format!("Renamed {n} folder(s)"),
        },
        |st| {
            for e in &entries {
                ops::rename_folder(&mut st.manifest, &e.id, e.name.clone());
            }
            Ok(Vec::new())
        },
    )
}

fn delete_media(state: &mut EditorState, asset_ids: Vec<String>) -> Result<EditResult, EditError> {
    if asset_ids.is_empty() {
        return Err(EditError::Invalid("assetIds is required".into()));
    }
    for id in &asset_ids {
        if !state.manifest.entries.iter().any(|m| m.id == *id) {
            return Err(EditError::Invalid(format!("Media asset not found: {id}")));
        }
    }
    let n = asset_ids.len();
    transact(
        state,
        "Delete Media",
        move |_| {
            format!(
                "Deleted {n} asset(s). Any clips referencing them were removed from the timeline."
            )
        },
        |st| {
            let set: HashSet<String> = asset_ids.iter().cloned().collect();
            ops::delete_media(&mut st.timeline, &mut st.manifest, &set);
            Ok(Vec::new())
        },
    )
}

fn delete_folder(
    state: &mut EditorState,
    folder_ids: Vec<String>,
) -> Result<EditResult, EditError> {
    if folder_ids.is_empty() {
        return Err(EditError::Invalid("folderIds is required".into()));
    }
    for id in &folder_ids {
        if !state.manifest.folders.iter().any(|f| f.id == *id) {
            return Err(EditError::Invalid(format!("folderId not found: {id}")));
        }
    }
    let n = folder_ids.len();
    transact(
        state,
        "Delete Folder",
        move |_| {
            format!(
            "Deleted {n} folder(s) with their contents. Any clips referencing deleted assets were removed from the timeline."
        )
        },
        |st| {
            let set: HashSet<String> = folder_ids.iter().cloned().collect();
            ops::delete_folder(&mut st.timeline, &mut st.manifest, &set);
            Ok(Vec::new())
        },
    )
}

// MARK: - Small local helpers

fn validate_entry(state: &EditorState, e: &ClipEntry, i: usize) -> Result<(), EditError> {
    if e.track_index >= state.timeline.tracks.len() {
        return Err(EditError::Invalid(format!(
            "entries[{i}]: track index {} out of range",
            e.track_index
        )));
    }
    let target = state.timeline.tracks[e.track_index].kind;
    if !e.source_clip_type.is_compatible(target) {
        return Err(EditError::Invalid(format!(
            "entries[{i}]: asset type is not compatible with the destination track"
        )));
    }
    if e.duration_frames < 1 {
        return Err(EditError::Invalid(format!(
            "entries[{i}]: durationFrames must be >= 1 (got {})",
            e.duration_frames
        )));
    }
    if e.start_frame < 0 {
        return Err(EditError::Invalid(format!(
            "entries[{i}]: startFrame must be >= 0 (got {})",
            e.start_frame
        )));
    }
    if let Some(t) = e.trim_start_frame {
        if t < 0 {
            return Err(EditError::Invalid(format!(
                "entries[{i}]: trimStartFrame must be >= 0 (got {t})"
            )));
        }
    }
    if let Some(t) = e.trim_end_frame {
        if t < 0 {
            return Err(EditError::Invalid(format!(
                "entries[{i}]: trimEndFrame must be >= 0 (got {t})"
            )));
        }
    }
    Ok(())
}

fn empty_to_none<V>(
    track: opentake_domain::KeyframeTrack<V>,
) -> Option<opentake_domain::KeyframeTrack<V>> {
    if track.keyframes.is_empty() {
        None
    } else {
        Some(track)
    }
}

fn loc_clip_id(state: &EditorState, loc: opentake_domain::ClipLocation) -> String {
    state.timeline.tracks[loc.track_index].clips[loc.clip_index]
        .id
        .clone()
}

fn find(timeline: &Timeline, clip_id: &str) -> Option<(usize, usize)> {
    for (ti, t) in timeline.tracks.iter().enumerate() {
        if let Some(ci) = t.clips.iter().position(|c| c.id == clip_id) {
            return Some((ti, ci));
        }
    }
    None
}

/// "V1" / "A1" / "I1" style track label. 1:1 port of `timelineTrackDisplayLabel`.
fn track_display_label(timeline: &Timeline, track_index: usize) -> String {
    if track_index >= timeline.tracks.len() {
        return String::new();
    }
    let kind = timeline.tracks[track_index].kind;
    let prefix = match kind {
        ClipType::Video => "V",
        ClipType::Audio => "A",
        ClipType::Image => "I",
        ClipType::Text => "T",
        ClipType::Lottie => "L",
    };
    let first_audio = ops::zones(timeline).first_audio_index;
    let mut n = 0;
    if kind == ClipType::Audio {
        for i in 0..=track_index {
            if timeline.tracks[i].kind == kind {
                n += 1;
            }
        }
    } else {
        for i in track_index..track_index.max(first_audio).max(track_index + 1) {
            if i < timeline.tracks.len() && timeline.tracks[i].kind == kind {
                n += 1;
            }
        }
    }
    format!("{prefix}{n}")
}

#[cfg(test)]
mod insert_track_tests {
    use super::*;
    use crate::id::SeqIdGen;
    use opentake_domain::ClipType;

    #[test]
    fn insert_track_on_empty_timeline_creates_compatible_track() {
        // The drop-onto-empty-timeline path: a brand-new project has no tracks,
        // so `addMediaToTimeline` first issues `InsertTrack` before `AddClips`.
        let mut state = EditorState::default();
        let ids = SeqIdGen::default();
        assert_eq!(state.timeline.tracks.len(), 0);

        let res = apply(
            &mut state,
            EditCommand::InsertTrack {
                kind: ClipType::Video,
            },
            &ids,
        )
        .unwrap();
        assert!(res.changed);
        assert_eq!(state.timeline.tracks.len(), 1);
        assert_eq!(state.timeline.tracks[0].kind, ClipType::Video);

        // A subsequent audio track clamps below the video zone.
        apply(
            &mut state,
            EditCommand::InsertTrack {
                kind: ClipType::Audio,
            },
            &ids,
        )
        .unwrap();
        assert_eq!(state.timeline.tracks.len(), 2);
        assert_eq!(state.timeline.tracks[1].kind, ClipType::Audio);
    }

    #[test]
    fn set_track_props_toggles_only_given_fields() {
        let mut state = EditorState::default();
        let ids = SeqIdGen::default();
        apply(
            &mut state,
            EditCommand::InsertTrack {
                kind: ClipType::Audio,
            },
            &ids,
        )
        .unwrap();
        // Mute + hide track 0; leave sync_locked unchanged.
        let prev_sync = state.timeline.tracks[0].sync_locked;
        let res = apply(
            &mut state,
            EditCommand::SetTrackProps {
                track_index: 0,
                muted: Some(true),
                hidden: Some(true),
                sync_locked: None,
            },
            &ids,
        )
        .unwrap();
        assert!(res.changed);
        assert!(state.timeline.tracks[0].muted);
        assert!(state.timeline.tracks[0].hidden);
        assert_eq!(state.timeline.tracks[0].sync_locked, prev_sync);
    }

    #[test]
    fn set_track_props_out_of_range_errors() {
        let mut state = EditorState::default();
        let ids = SeqIdGen::default();
        let err = apply(
            &mut state,
            EditCommand::SetTrackProps {
                track_index: 5,
                muted: Some(true),
                hidden: None,
                sync_locked: None,
            },
            &ids,
        );
        assert!(err.is_err());
    }
}
