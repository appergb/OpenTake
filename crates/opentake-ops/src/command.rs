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

use opentake_domain::{ClipType, Timeline, Transform};

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
    /// Ripple-delete project-frame ranges on a track, closing the gaps.
    RippleDeleteRanges {
        track_index: usize,
        ranges: Vec<FrameRange>,
    },
    /// Add text overlays.
    AddTexts { entries: Vec<TextEntry> },
    /// Link clips into one group.
    Link { clip_ids: Vec<String> },
    /// Unlink clips (and their whole groups).
    Unlink { clip_ids: Vec<String> },
    /// Remove tracks by index.
    RemoveTracks { track_indexes: Vec<usize> },
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
        EditCommand::RippleDeleteRanges {
            track_index,
            ranges,
        } => ripple_delete_ranges(state, track_index, ranges, ids),
        EditCommand::AddTexts { entries } => add_texts(state, entries, ids),
        EditCommand::Link { clip_ids } => link(state, clip_ids, ids),
        EditCommand::Unlink { clip_ids } => unlink(state, clip_ids),
        EditCommand::RemoveTracks { track_indexes } => remove_tracks(state, track_indexes),
        EditCommand::CreateFolder {
            name,
            parent_folder_id,
        } => create_folder(state, name, parent_folder_id, ids),
        EditCommand::MoveToFolder {
            asset_ids,
            folder_id,
        } => move_to_folder(state, asset_ids, folder_id),
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
