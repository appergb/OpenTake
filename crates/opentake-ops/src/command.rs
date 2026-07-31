//! The single editing entry point: [`EditCommand`] + [`apply`].
//!
//! UI gestures, the in-app agent, and the external MCP server all funnel through
//! one command enum, so undo / validation / versioning are written once
//! (`ARCHITECTURE.md 搂5`, upstream `ToolExecutor`).
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

use std::collections::{HashMap, HashSet};

use opentake_domain::{
    ChromaKey, Clip, ClipType, ColorGrade, Crop, Effect, Interpolation, Mask, MaskShape,
    NestedSequence, Timeline, Track, Transform, Transition, TransitionKind, MAX_MASKS_PER_CLIP,
    MAX_POLYGON_MASK_POINTS,
};

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

/// Outcome of a successfully-attempted command. The core result from
/// `ARCHITECTURE.md §5` plus document-domain flags used for precise events.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EditResult {
    /// Whether the document actually changed (drives undo-stack push + version bump).
    pub changed: bool,
    /// Whether the timeline portion of the document changed.
    pub timeline_changed: bool,
    /// Whether the media-manifest portion of the document changed.
    pub manifest_changed: bool,
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
    pub transform: Option<Transform>,
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
            transform: self.transform,
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

/// A text overlay entry for [`EditCommand::AddTextsAutoTrack`]. Identical to
/// [`TextEntry`] minus `track_index` — every entry in the batch lands on the
/// single fresh track the command creates, so there is nothing to target.
#[derive(Clone, Debug)]
pub struct TextAutoTrackEntry {
    pub start_frame: i32,
    pub duration_frames: i32,
    pub content: String,
    pub text_style: opentake_domain::TextStyle,
    pub transform: Transform,
}

/// One built caption clip for [`EditCommand::AddCaptions`]. Like [`TextEntry`]
/// but (a) has no `track_index` — every caption lands on the single fresh track
/// the command creates — and (b) carries the `caption_group_id` all clips from
/// one Generate share, so subtitle export and caption-group style sync recognize
/// them. The pure builder (`opentake_media::caption_specs`) produced the content,
/// frames, style, and transform; this leaf just places them.
#[derive(Clone, Debug)]
pub struct CaptionEntry {
    pub start_frame: i32,
    pub duration_frames: i32,
    pub content: String,
    pub text_style: opentake_domain::TextStyle,
    pub transform: Transform,
    pub caption_group_id: String,
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
    /// Text style for a text clip (font / size / color / alignment / shadow /
    /// background / border). Replaces the clip's whole `text_style`.
    pub text_style: Option<opentake_domain::TextStyle>,
    /// Per-clip crop insets (normalized 0–1). Setting this clears `crop_track`.
    pub crop: Option<Crop>,
    /// Fade-in length in frames. Setting this clamps to the clip duration.
    pub fade_in_frames: Option<i32>,
    /// Fade-out length in frames. Setting this clamps to the clip duration.
    pub fade_out_frames: Option<i32>,
    /// Fade-in interpolation mode.
    pub fade_in_interpolation: Option<Interpolation>,
    /// Fade-out interpolation mode.
    pub fade_out_interpolation: Option<Interpolation>,
    /// Horizontal flip flag (writes to `transform.flip_horizontal`).
    pub flip_horizontal: Option<bool>,
    /// Vertical flip flag (writes to `transform.flip_vertical`).
    pub flip_vertical: Option<bool>,
    /// Reverse playback flag. Per-clip and not propagated to linked audio partners.
    pub reversed: Option<bool>,
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

/// An explicit single-keyframe value for [`EditCommand::UpsertKeyframe`]. Unlike
/// [`KeyframePayload`] (a whole replacement track), this carries just the value
/// to upsert at the command's `frame`. Exactly one variant is used per command,
/// matching `property` — Scalar for Opacity/Volume/Rotation, Pair for
/// Position/Scale, Crop for Crop.
#[derive(Clone, Copy, Debug)]
pub enum KeyframeValue {
    Scalar(f64),
    Pair(opentake_domain::AnimPair),
    Crop(opentake_domain::Crop),
}

/// The unified editing command. Every editing surface routes through this.
#[derive(Clone, Debug)]
pub enum EditCommand {
    /// Register an editable child timeline and place one compound clip that
    /// references it. Sequence + clip creation is one undoable transaction.
    CreateNestedSequence {
        name: String,
        timeline: Timeline,
        track_index: usize,
        start_frame: i32,
        duration_frames: i32,
    },
    /// Turn selected root clips into one editable compound clip. The child
    /// timeline keeps their relative tracks/timing and source edits.
    CreateNestedSequenceFromClips { name: String, clip_ids: Vec<String> },
    /// Apply any ordinary edit command to a child timeline while preserving one
    /// root-level undo snapshot and the shared media manifest.
    EditNestedSequence {
        sequence_id: String,
        command: Box<EditCommand>,
    },
    /// Replace the editable contents of one existing nested sequence.
    SetNestedSequenceTimeline {
        sequence_id: String,
        timeline: Timeline,
    },
    /// Rename a nested sequence without changing references.
    RenameNestedSequence { sequence_id: String, name: String },
    /// Replace one compound clip with clipped copies of its child timeline
    /// tracks, preserving media edits and keeping the operation undoable.
    DissolveNestedSequence { clip_id: String },
    /// Overwrite-place clips (clears each destination range first).
    AddClips { entries: Vec<ClipEntry> },
    /// Overwrite-place clips on fresh shared tracks chosen by media type.
    /// Visual entries share one new visual track; audio entries share one new
    /// audio track. Track insertion and placement commit as one transaction.
    AddClipsAutoTrack { entries: Vec<ClipEntry> },
    /// Ripple-insert clips at `at_frame`, pushing later clips right.
    InsertClips {
        track_index: usize,
        at_frame: i32,
        entries: Vec<ClipEntry>,
    },
    /// Move clips (expanded to linked partners by the caller) to new tracks/frames.
    MoveClips { moves: Vec<ClipMove> },
    /// Deep-copy clips (Option/Alt-drag duplicate) to new positions. Each clip
    /// is cloned with all its fields (keyframe tracks / grade / chroma / masks /
    /// effects / text / transform / crop / fades), gets a fresh id, is shifted
    /// by `offset_frames`, lands on `target_track_indexes[i]`, and has its
    /// `link_group_id` cleared (a copy is not linked to the original's group).
    DuplicateClips {
        clip_ids: Vec<String>,
        offset_frames: i32,
        target_track_indexes: Vec<usize>,
    },
    /// Remove clips (expanded to linked partners), pruning emptied tracks.
    RemoveClips { clip_ids: Vec<String> },
    /// Split a clip at a frame (splits linked partners too).
    SplitClip { clip_id: String, at_frame: i32 },
    /// Freeze Frame: split at `at_frame`, then ripple-insert a still image clip.
    FreezeFrame {
        clip_id: String,
        at_frame: i32,
        duration_frames: i32,
        media_ref: String,
    },
    /// Overwrite-style trim: resize clips in place from new source-frame trims.
    TrimClips { edits: Vec<TrimEdit> },
    /// Assign clip properties (timing changes propagate to linked partners).
    /// `properties` is boxed: it carries a full `TextStyle`, which would
    /// otherwise make this the dominant `EditCommand` variant (the enum is
    /// `Clone`d on every undo snapshot path).
    SetClipProperties {
        clip_ids: Vec<String>,
        properties: Box<ClipProperties>,
    },
    /// Replace (or clear) a clip's keyframe track for one property.
    SetKeyframes {
        clip_id: String,
        property: KeyframeProperty,
        payload: KeyframePayload,
    },
    /// Stamp a keyframe at `frame` (absolute timeline frame) using the clip's
    /// current sampled value for `property`. Creates the track if absent.
    StampKeyframe {
        clip_id: String,
        property: KeyframeProperty,
        frame: i32,
    },
    /// Upsert a keyframe at `frame` (absolute timeline frame) with an EXPLICIT
    /// `value`, instead of the clip's current sampled value (that's
    /// `StampKeyframe`). Creates the track if absent. This is the missing half of
    /// upstream's animation authoring: `write<Property>` does
    /// `if <track>.isActive { clip.upsertKeyframe(in: \.<track>, frame:, value:) }
    /// else { set static }` — the static half already exists via
    /// `SetClipProperties`; this command is the upsert half.
    UpsertKeyframe {
        clip_id: String,
        property: KeyframeProperty,
        frame: i32,
        value: KeyframeValue,
    },
    /// Remove the keyframe at `frame` (absolute timeline frame). Clears the track
    /// to `None` when it becomes empty.
    RemoveKeyframe {
        clip_id: String,
        property: KeyframeProperty,
        frame: i32,
    },
    /// Move a keyframe from `from_frame` to `to_frame` (both absolute timeline
    /// frames). Refuses if `to_frame` is already occupied.
    MoveKeyframe {
        clip_id: String,
        property: KeyframeProperty,
        from_frame: i32,
        to_frame: i32,
    },
    /// Change the interpolation mode of the keyframe at `frame` (absolute timeline
    /// frame).
    SetKeyframeInterpolation {
        clip_id: String,
        property: KeyframeProperty,
        frame: i32,
        interpolation: opentake_domain::Interpolation,
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
    /// Set or clear the visual transition at one exact adjacent clip boundary.
    SetTransition {
        from_clip_id: String,
        to_clip_id: String,
        kind: Option<TransitionKind>,
        duration_frames: i32,
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
    /// Add text overlays onto ONE fresh video track (inserted at index 0), as a
    /// single undoable action. 1:1 port of upstream `addTexts`'s all-omitted
    /// path (`ToolExecutor+Texts.swift:114-121`) and the UI's `addTextClip`
    /// (`EditorViewModel+MediaLibrary.swift:519-558`), both of which always
    /// create a fresh top track rather than writing into whatever the caller
    /// finds at track 0 — the bug this command exists to close (#194): an
    /// existing top track can hold unrelated video/image content, and the
    /// straight `AddTexts` path would `clear_region` over it. Entries within
    /// the batch still overwrite each other on overlap, same as `AddTexts`
    /// (upstream `placeTextClips` groups-by-track + clears each destination
    /// range in `startFrame` order; here the track is single so that reduces
    /// to one ordered pass). Empty `entries` is an error, matching `AddTexts`
    /// (unlike `AddCaptions`, whose empty-is-no-op reflects "no speech
    /// detected" rather than a caller mistake).
    AddTextsAutoTrack { entries: Vec<TextAutoTrackEntry> },
    /// Place a whole batch of generated caption clips on ONE fresh video track
    /// (inserted at index 0), as a single undoable action named "Generate
    /// Captions". 1:1 port of upstream `placeCaptionTrack`
    /// (`EditorViewModel+Captions.swift:226-242`): a new top track holds every
    /// caption, and each clip carries the shared `caption_group_id` so subtitle
    /// export / caption-group style sync recognize it. Atomic on purpose —
    /// composing `InsertTrack` + `AddTexts` would be two undo steps and could not
    /// stamp `caption_group_id`. Empty `entries` is a no-op (no track, no change).
    AddCaptions { entries: Vec<CaptionEntry> },
    /// Link clips into one group.
    Link { clip_ids: Vec<String> },
    /// Unlink clips (and their whole groups).
    Unlink { clip_ids: Vec<String> },
    /// Remove tracks by index.
    RemoveTracks { track_indexes: Vec<usize> },
    /// Swap two same-kind tracks as whole rows. OpenTake-only extension.
    SwapTracks { a: usize, b: usize },
    /// Swap the positions — track + start frame — of two clips, so a cross-track
    /// drag exchanges them instead of overwriting (swallowing) the destination.
    /// Lossless: refused with no change if a clip would overlap a third clip at
    /// its new slot. OpenTake-only extension.
    SwapClips { a: String, b: String },
    /// Insert a new empty track of `kind` (clamped into its zone). Lets the drop
    /// flow create a track on demand when the timeline has no compatible one
    /// (upstream `placeClip` / `add_clips` with omitted `trackIndex` 鈫?
    /// `insertTrack`), so dragging media onto an empty timeline produces a clip.
    InsertTrack { kind: ClipType, at: Option<usize> },
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
    /// Replace a clip's `media_ref` in place, preserving all editing attributes
    /// (transform / crop / keyframe tracks / grade / masks / effects / fade /
    /// trim / speed / start / duration). 1:1 port of upstream
    /// `replaceClipMediaRef(resetTrim: false)`:
    ///
    /// * **Type-must-match**: the candidate asset's `kind` must strictly equal
    ///   the clip's `media_type` (no `isVisual` leniency, no `media_type`
    ///   override). A mismatch is refused without mutating state.
    /// * **Link-group cascade**: clips that share the seed clip's link group
    ///   AND its old `media_ref` are swapped together, so a linked audio/video
    ///   pair pointing at the same file stays in sync.
    /// * **No-op on identical ref**: swapping to the same `media_ref` returns
    ///   `changed = false` (no undo entry, no version bump).
    /// * **No trim/duration rewrites**: trim / speed / start / duration are
    ///   kept verbatim. The render layer is responsible for any overshoot
    ///   sampling when the new media is shorter.
    SwapMedia { clip_id: String, media_ref: String },
    /// Reset the transform section back to defaults. 1:1 port of upstream's
    /// Inspector "Reset transform" button (`InspectorView.transformHeader`):
    /// sets `transform` to identity (`Transform::default()`, full-canvas
    /// centered, no rotation/flip), `opacity` to `1.0`, clears the opacity /
    /// position / scale / rotation keyframe tracks, and zeroes both fades
    /// (frames + interpolation back to `Linear`). Crop and its keyframe track
    /// are untouched (a separate Inspector section upstream).
    ResetTransform { clip_ids: Vec<String> },
    /// Change project timeline settings (FPS / resolution). 1:1 port of upstream
    /// `EditorViewModel.applyTimelineSettings(fps:width:height:)`: when FPS
    /// changes, all clip frame values are rescaled by `new/old`; `width`/`height`
    /// set the canvas. See `ops::set_timeline_settings` for the ported/deferred
    /// details (the playhead rescale + aspect refit are intentionally not here).
    SetTimelineSettings { fps: i32, width: i32, height: i32 },
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
            let before = state.snapshot();
            let changed = state.undo();
            let after = state.snapshot();
            Ok(result(
                state,
                changed && before.timeline != after.timeline,
                changed && before.manifest != after.manifest,
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
            let before = state.snapshot();
            let changed = state.redo();
            let after = state.snapshot();
            Ok(result(
                state,
                changed && before.timeline != after.timeline,
                changed && before.manifest != after.manifest,
                "Redo",
                Vec::new(),
                if changed {
                    "Redid last edit"
                } else {
                    "Nothing to redo"
                },
            ))
        }

        EditCommand::CreateNestedSequence {
            name,
            timeline,
            track_index,
            start_frame,
            duration_frames,
        } => create_nested_sequence(
            state,
            name,
            timeline,
            track_index,
            start_frame,
            duration_frames,
            ids,
        ),
        EditCommand::CreateNestedSequenceFromClips { name, clip_ids } => {
            create_nested_sequence_from_clips(state, name, clip_ids, ids)
        }
        EditCommand::EditNestedSequence {
            sequence_id,
            command,
        } => edit_nested_sequence(state, sequence_id, *command, ids),
        EditCommand::SetNestedSequenceTimeline {
            sequence_id,
            timeline,
        } => set_nested_sequence_timeline(state, sequence_id, timeline),
        EditCommand::RenameNestedSequence { sequence_id, name } => {
            rename_nested_sequence(state, sequence_id, name)
        }
        EditCommand::DissolveNestedSequence { clip_id } => {
            dissolve_nested_sequence(state, clip_id, ids)
        }
        EditCommand::AddClips { entries } => add_clips(state, entries, ids),
        EditCommand::AddClipsAutoTrack { entries } => add_clips_auto_track(state, entries, ids),
        EditCommand::InsertClips {
            track_index,
            at_frame,
            entries,
        } => insert_clips(state, track_index, at_frame, entries, ids),
        EditCommand::MoveClips { moves } => move_clips(state, moves, ids),
        EditCommand::DuplicateClips {
            clip_ids,
            offset_frames,
            target_track_indexes,
        } => duplicate_clips_cmd(state, clip_ids, offset_frames, target_track_indexes, ids),
        EditCommand::RemoveClips { clip_ids } => remove_clips(state, clip_ids),
        EditCommand::SplitClip { clip_id, at_frame } => split(state, clip_id, at_frame, ids),
        EditCommand::FreezeFrame {
            clip_id,
            at_frame,
            duration_frames,
            media_ref,
        } => freeze_frame(state, clip_id, at_frame, duration_frames, media_ref, ids),
        EditCommand::TrimClips { edits } => trim(state, edits),
        EditCommand::SetClipProperties {
            clip_ids,
            properties,
        } => set_clip_properties(state, clip_ids, *properties),
        EditCommand::SetKeyframes {
            clip_id,
            property,
            payload,
        } => set_keyframes(state, clip_id, property, payload),
        EditCommand::StampKeyframe {
            clip_id,
            property,
            frame,
        } => stamp_keyframe(state, clip_id, property, frame),
        EditCommand::UpsertKeyframe {
            clip_id,
            property,
            frame,
            value,
        } => upsert_keyframe(state, clip_id, property, frame, value),
        EditCommand::RemoveKeyframe {
            clip_id,
            property,
            frame,
        } => remove_keyframe(state, clip_id, property, frame),
        EditCommand::MoveKeyframe {
            clip_id,
            property,
            from_frame,
            to_frame,
        } => move_keyframe(state, clip_id, property, from_frame, to_frame),
        EditCommand::SetKeyframeInterpolation {
            clip_id,
            property,
            frame,
            interpolation,
        } => set_keyframe_interpolation(state, clip_id, property, frame, interpolation),
        EditCommand::SetColorGrade { clip_ids, grade } => set_color_grade(state, clip_ids, grade),
        EditCommand::SetChromaKey {
            clip_ids,
            chroma_key,
        } => set_chroma_key(state, clip_ids, chroma_key),
        EditCommand::SetMasks { clip_ids, masks } => set_masks(state, clip_ids, masks),
        EditCommand::SetEffects { clip_ids, effects } => set_effects(state, clip_ids, effects),
        EditCommand::SetTransition {
            from_clip_id,
            to_clip_id,
            kind,
            duration_frames,
        } => set_transition(state, from_clip_id, to_clip_id, kind, duration_frames),
        EditCommand::RippleDeleteRanges {
            track_index,
            ranges,
        } => ripple_delete_ranges(state, track_index, ranges, ids),
        EditCommand::RippleDeleteClips { clip_ids } => ripple_delete_clips(state, clip_ids),
        EditCommand::AddTexts { entries } => add_texts(state, entries, ids),
        EditCommand::AddTextsAutoTrack { entries } => add_texts_auto_track(state, entries, ids),
        EditCommand::AddCaptions { entries } => add_captions(state, entries, ids),
        EditCommand::Link { clip_ids } => link(state, clip_ids, ids),
        EditCommand::Unlink { clip_ids } => unlink(state, clip_ids),
        EditCommand::RemoveTracks { track_indexes } => remove_tracks(state, track_indexes),
        EditCommand::SwapTracks { a, b } => swap_tracks(state, a, b),
        EditCommand::SwapClips { a, b } => swap_clips(state, a, b),
        EditCommand::InsertTrack { kind, at } => insert_track_cmd(state, kind, at, ids),
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
        EditCommand::SwapMedia { clip_id, media_ref } => swap_media(state, clip_id, media_ref),
        EditCommand::ResetTransform { clip_ids } => reset_transform(state, clip_ids),
        EditCommand::SetTimelineSettings { fps, width, height } => {
            set_timeline_settings_cmd(state, fps, width, height)
        }
    }
}

fn create_nested_sequence(
    state: &mut EditorState,
    name: String,
    timeline: Timeline,
    track_index: usize,
    start_frame: i32,
    duration_frames: i32,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(EditError::Invalid(
            "nested sequence name must not be empty".into(),
        ));
    }
    if track_index >= state.timeline.tracks.len() {
        return Err(EditError::Invalid(format!(
            "track index out of range: {track_index}"
        )));
    }
    if state.timeline.tracks[track_index].kind == ClipType::Audio {
        return Err(EditError::Invalid(
            "a compound clip requires a visual track".into(),
        ));
    }
    if start_frame < 0 || duration_frames < 1 {
        return Err(EditError::Invalid(
            "compound timing requires startFrame >= 0 and durationFrames >= 1".into(),
        ));
    }
    if !timeline.nested_sequences.is_empty() {
        return Err(EditError::Invalid(
            "child timelines must reference the root nested sequence registry".into(),
        ));
    }

    transact(
        state,
        "Create Compound Clip",
        |affected| format!("Created compound clip {}", affected.join(", ")),
        |st| {
            let sequence_id = ids.next_id();
            let clip_id = ids.next_id();
            st.timeline.nested_sequences.push(NestedSequence::new(
                sequence_id.clone(),
                name,
                timeline,
            ));
            ops::clear_region(
                &mut st.timeline,
                track_index,
                start_frame,
                start_frame.saturating_add(duration_frames),
                false,
                ids,
            );
            st.timeline.tracks[track_index].clips.push(Clip::new_nested(
                clip_id.clone(),
                sequence_id,
                start_frame,
                duration_frames,
            ));
            ops::sort_clips(&mut st.timeline.tracks[track_index]);
            Ok(vec![clip_id])
        },
    )
}

fn create_nested_sequence_from_clips(
    state: &mut EditorState,
    name: String,
    clip_ids: Vec<String>,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(EditError::Invalid(
            "nested sequence name must not be empty".into(),
        ));
    }
    if clip_ids.is_empty() {
        return Err(EditError::Invalid(
            "at least one clip is required to create a compound".into(),
        ));
    }
    let requested: HashSet<String> = clip_ids.iter().cloned().collect();
    if requested.len() != clip_ids.len() {
        return Err(EditError::Invalid(
            "compound clip selection contains duplicate ids".into(),
        ));
    }
    // Keep linked A/V partners inside the same edit boundary. Leaving one half
    // at root would create a link group spanning independent timelines, which
    // child edit commands cannot preserve safely.
    let selected = ops::expand_to_link_group(&state.timeline, &requested);

    let mut start_frame = i32::MAX;
    let mut end_frame = i32::MIN;
    let mut target_track = None;
    let mut child = Timeline::new();
    child.fps = state.timeline.fps;
    child.width = state.timeline.width;
    child.height = state.timeline.height;
    child.settings_configured = state.timeline.settings_configured;
    for (track_index, track) in state.timeline.tracks.iter().enumerate() {
        let clips: Vec<Clip> = track
            .clips
            .iter()
            .filter(|clip| selected.contains(&clip.id))
            .cloned()
            .collect();
        if clips.is_empty() {
            continue;
        }
        if target_track.is_none() && track.kind != ClipType::Audio {
            target_track = Some(track_index);
        }
        for clip in &clips {
            start_frame = start_frame.min(clip.start_frame);
            end_frame = end_frame.max(clip.end_frame());
        }
        let mut child_track = Track::new(ids.next_id(), track.kind);
        child_track.muted = track.muted;
        child_track.hidden = track.hidden;
        child_track.sync_locked = track.sync_locked;
        child_track.clips = clips;
        child.tracks.push(child_track);
    }
    let found: usize = child.tracks.iter().map(|track| track.clips.len()).sum();
    if found != selected.len() {
        return Err(EditError::Invalid(
            "one or more clips selected for the compound no longer exist".into(),
        ));
    }
    let target_track = target_track.ok_or_else(|| {
        EditError::Invalid("a compound clip requires at least one visual clip".into())
    })?;
    if state.timeline.tracks[target_track]
        .clips
        .iter()
        .any(|clip| {
            !selected.contains(&clip.id)
                && clip.start_frame < end_frame
                && start_frame < clip.end_frame()
        })
    {
        return Err(EditError::Invalid(
            "compound selection span overlaps an unselected clip on its destination track".into(),
        ));
    }
    for track in &mut child.tracks {
        for clip in &mut track.clips {
            clip.start_frame -= start_frame;
        }
    }
    let duration_frames = end_frame - start_frame;

    transact(
        state,
        "Create Compound Clip",
        |affected| format!("Created compound clip {}", affected.join(", ")),
        |st| {
            for track in &mut st.timeline.tracks {
                track.clips.retain(|clip| !selected.contains(&clip.id));
            }
            let sequence_id = ids.next_id();
            let compound_id = ids.next_id();
            st.timeline.nested_sequences.push(NestedSequence::new(
                sequence_id.clone(),
                name,
                child,
            ));
            ops::clear_region(
                &mut st.timeline,
                target_track,
                start_frame,
                end_frame,
                false,
                ids,
            );
            st.timeline.tracks[target_track]
                .clips
                .push(Clip::new_nested(
                    compound_id.clone(),
                    sequence_id,
                    start_frame,
                    duration_frames,
                ));
            ops::sort_clips(&mut st.timeline.tracks[target_track]);
            ops::prune_empty_tracks(&mut st.timeline);
            Ok(vec![compound_id])
        },
    )
}

fn edit_nested_sequence(
    state: &mut EditorState,
    sequence_id: String,
    command: EditCommand,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    if matches!(
        &command,
        EditCommand::Undo
            | EditCommand::Redo
            | EditCommand::CreateNestedSequence { .. }
            | EditCommand::CreateNestedSequenceFromClips { .. }
            | EditCommand::EditNestedSequence { .. }
            | EditCommand::SetNestedSequenceTimeline { .. }
            | EditCommand::RenameNestedSequence { .. }
            | EditCommand::DissolveNestedSequence { .. }
            | EditCommand::CreateFolder { .. }
            | EditCommand::MoveToFolder { .. }
            | EditCommand::RenameMedia { .. }
            | EditCommand::RenameFolder { .. }
            | EditCommand::DeleteMedia { .. }
            | EditCommand::DeleteFolder { .. }
            | EditCommand::SetTimelineSettings { .. }
    ) {
        return Err(EditError::Invalid(
            "nested-sequence, media-library, and project-settings commands must target the root timeline".into(),
        ));
    }
    let child = state
        .timeline
        .nested_sequences
        .iter()
        .find(|sequence| sequence.id == sequence_id)
        .map(|sequence| sequence.timeline.clone())
        .ok_or_else(|| EditError::Invalid(format!("Nested sequence not found: {sequence_id}")))?;

    transact(
        state,
        "Edit Compound Clip",
        |_| format!("Edited nested sequence {sequence_id}"),
        |st| {
            // Supply the root registry during the inner transaction so child
            // references resolve against the same identities as preview/export.
            // The target sequence's stored (pre-edit) contents are blanked in
            // this temporary view: `editable` already represents those clips,
            // and counting both copies would manufacture duplicate clip ids.
            // The enclosing root transaction validates the fully replaced graph
            // (including cycles through this sequence) before it can commit.
            let mut editable = child;
            editable.nested_sequences = st.timeline.nested_sequences.clone();
            editable
                .nested_sequences
                .iter_mut()
                .find(|sequence| sequence.id == sequence_id)
                .expect("sequence was resolved before transaction")
                .timeline = Timeline::new();
            let mut child_state = EditorState::new(editable, st.manifest.clone());
            let inner = apply(&mut child_state, command, ids)?;
            child_state.timeline.nested_sequences.clear();
            let sequence = st
                .timeline
                .nested_sequences
                .iter_mut()
                .find(|sequence| sequence.id == sequence_id)
                .expect("sequence was resolved before transaction");
            sequence.timeline = child_state.timeline;
            st.manifest = child_state.manifest;
            Ok(inner.affected_clip_ids)
        },
    )
}

fn set_nested_sequence_timeline(
    state: &mut EditorState,
    sequence_id: String,
    timeline: Timeline,
) -> Result<EditResult, EditError> {
    if !timeline.nested_sequences.is_empty() {
        return Err(EditError::Invalid(
            "child timelines must reference the root nested sequence registry".into(),
        ));
    }
    transact(
        state,
        "Edit Compound Clip",
        |_| format!("Edited nested sequence {sequence_id}"),
        |st| {
            let sequence = st
                .timeline
                .nested_sequences
                .iter_mut()
                .find(|sequence| sequence.id == sequence_id)
                .ok_or_else(|| {
                    EditError::Invalid(format!("Nested sequence not found: {sequence_id}"))
                })?;
            sequence.timeline = timeline;
            Ok(Vec::new())
        },
    )
}

fn rename_nested_sequence(
    state: &mut EditorState,
    sequence_id: String,
    name: String,
) -> Result<EditResult, EditError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(EditError::Invalid(
            "nested sequence name must not be empty".into(),
        ));
    }
    transact(
        state,
        "Rename Compound Clip",
        |_| format!("Renamed nested sequence {sequence_id}"),
        |st| {
            let sequence = st
                .timeline
                .nested_sequences
                .iter_mut()
                .find(|sequence| sequence.id == sequence_id)
                .ok_or_else(|| {
                    EditError::Invalid(format!("Nested sequence not found: {sequence_id}"))
                })?;
            sequence.name = name;
            Ok(Vec::new())
        },
    )
}

fn dissolve_nested_sequence(
    state: &mut EditorState,
    clip_id: String,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    let location = state
        .find_clip(&clip_id)
        .ok_or_else(|| EditError::Invalid(format!("Clip not found: {clip_id}")))?;
    let compound = state.timeline.tracks[location.track_index].clips[location.clip_index].clone();
    let sequence_id = compound
        .nested_sequence_id
        .as_deref()
        .ok_or_else(|| EditError::Invalid(format!("Clip is not a compound clip: {clip_id}")))?;
    if (compound.speed - 1.0).abs() > f64::EPSILON || compound.reversed {
        return Err(EditError::Invalid(
            "retimed or reversed compound clips must be normalized before dissolve".into(),
        ));
    }
    let has_parent_edits = (compound.volume - 1.0).abs() > f64::EPSILON
        || compound.fade_in_frames != 0
        || compound.fade_out_frames != 0
        || (compound.opacity - 1.0).abs() > f64::EPSILON
        || compound.transform != Transform::default()
        || compound.crop != Crop::default()
        || compound.link_group_id.is_some()
        || compound.caption_group_id.is_some()
        || compound.text_content.is_some()
        || compound.text_style.is_some()
        || compound.opacity_track.is_some()
        || compound.position_track.is_some()
        || compound.scale_track.is_some()
        || compound.rotation_track.is_some()
        || compound.crop_track.is_some()
        || compound.volume_track.is_some()
        || compound.color_grade.is_some()
        || compound.chroma_key.is_some()
        || !compound.masks.is_empty()
        || !compound.effects.is_empty()
        || compound.transition_out.is_some();
    if has_parent_edits {
        return Err(EditError::Invalid(
            "compound clips with parent-level edits must be normalized before dissolve".into(),
        ));
    }
    let child = state
        .timeline
        .nested_sequences
        .iter()
        .find(|sequence| sequence.id == sequence_id)
        .map(|sequence| sequence.timeline.clone())
        .ok_or_else(|| EditError::Invalid(format!("Nested sequence not found: {sequence_id}")))?;

    transact(
        state,
        "Dissolve Compound Clip",
        |affected| format!("Dissolved compound into {} clip(s)", affected.len()),
        |st| {
            let source_start = compound.trim_start_frame;
            let source_end = source_start.saturating_add(compound.duration_frames);
            let mut id_map = HashMap::new();
            let mut link_counts: HashMap<String, usize> = HashMap::new();
            for child_clip in child.tracks.iter().flat_map(|track| &track.clips) {
                let visible_start = child_clip.start_frame.max(source_start);
                let visible_end = child_clip.end_frame().min(source_end);
                if visible_end <= visible_start {
                    continue;
                }
                id_map.insert(child_clip.id.clone(), ids.next_id());
                if let Some(group) = &child_clip.link_group_id {
                    *link_counts.entry(group.clone()).or_default() += 1;
                }
            }
            let mut link_map: HashMap<String, String> = HashMap::new();

            ops::clear_region::remove_clip(&mut st.timeline, &clip_id);
            let mut affected = Vec::new();

            for child_track in child.tracks {
                let requested = st.timeline.tracks.len();
                let target = ops::insert_track(&mut st.timeline, requested, child_track.kind, ids);
                st.timeline.tracks[target].muted = child_track.muted;
                st.timeline.tracks[target].hidden = child_track.hidden;
                st.timeline.tracks[target].sync_locked = child_track.sync_locked;

                for mut child_clip in child_track.clips {
                    let visible_start = child_clip.start_frame.max(source_start);
                    let visible_end = child_clip.end_frame().min(source_end);
                    if visible_end <= visible_start {
                        continue;
                    }
                    let clipped_left = visible_start - child_clip.start_frame;
                    let clipped_right = child_clip.end_frame() - visible_end;
                    let old_id = child_clip.id.clone();
                    child_clip.id = id_map
                        .get(&old_id)
                        .expect("visible child clip received a replacement id")
                        .clone();
                    child_clip.link_group_id = child_clip.link_group_id.take().and_then(|group| {
                        (link_counts.get(&group).copied().unwrap_or(0) > 1).then(|| {
                            link_map
                                .entry(group)
                                .or_insert_with(|| ids.next_id())
                                .clone()
                        })
                    });
                    child_clip.transition_out =
                        child_clip.transition_out.take().and_then(|mut transition| {
                            id_map.get(&transition.to_clip_id).map(|to_id| {
                                transition.to_clip_id = to_id.clone();
                                transition
                            })
                        });
                    child_clip.start_frame = compound
                        .start_frame
                        .saturating_add(visible_start - source_start);
                    child_clip.duration_frames = visible_end - visible_start;
                    child_clip.trim_start_frame = child_clip
                        .trim_start_frame
                        .saturating_add((clipped_left as f64 * child_clip.speed).round() as i32);
                    child_clip.trim_end_frame = child_clip
                        .trim_end_frame
                        .saturating_add((clipped_right as f64 * child_clip.speed).round() as i32);
                    affected.push(child_clip.id.clone());
                    st.timeline.tracks[target].clips.push(child_clip);
                }
                ops::sort_clips(&mut st.timeline.tracks[target]);
            }
            ops::prune_empty_tracks(&mut st.timeline);
            Ok(affected)
        },
    )
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
    let affected = match work(state) {
        Ok(affected) => affected,
        Err(error) => {
            state.restore(before);
            return Err(error);
        }
    };
    prune_invalid_transitions(&mut state.timeline);
    if let Err(reason) = state.timeline.validate_nested_sequences() {
        state.restore(before);
        return Err(EditError::Invalid(reason));
    }
    let after = state.snapshot();
    let timeline_changed = before.timeline != after.timeline;
    let manifest_changed = before.manifest != after.manifest;
    let changed = timeline_changed || manifest_changed;
    if changed {
        state.commit(before);
    }
    let summary = summarize(&affected);
    Ok(result(
        state,
        timeline_changed,
        manifest_changed,
        action_name,
        affected,
        &summary,
    ))
}

/// Keep transition pair identity aligned with the actual cut graph after every
/// transactional edit. A move/delete/trim must never leave a dormant transition
/// that could later bind to a different neighbor.
fn prune_invalid_transitions(timeline: &mut Timeline) {
    for sequence in &mut timeline.nested_sequences {
        prune_invalid_transitions(&mut sequence.timeline);
    }
    for track in &mut timeline.tracks {
        if track.kind == ClipType::Audio {
            for clip in &mut track.clips {
                clip.transition_out = None;
            }
            continue;
        }
        let mut order: Vec<usize> = (0..track.clips.len()).collect();
        order.sort_by_key(|&index| {
            (
                track.clips[index].start_frame,
                track.clips[index].id.clone(),
            )
        });
        let mut valid: HashMap<String, (String, i32)> = HashMap::new();
        for pair in order.windows(2) {
            let from = &track.clips[pair[0]];
            let to = &track.clips[pair[1]];
            if from.end_frame() != to.start_frame
                || matches!(from.media_type, ClipType::Audio | ClipType::Text)
                || matches!(to.media_type, ClipType::Audio | ClipType::Text)
            {
                continue;
            }
            valid.insert(
                from.id.clone(),
                (
                    to.id.clone(),
                    (from.duration_frames.min(to.duration_frames) / 2).max(1),
                ),
            );
        }
        for clip in &mut track.clips {
            let Some(transition) = &mut clip.transition_out else {
                continue;
            };
            let Some((to_id, maximum)) = valid.get(&clip.id) else {
                clip.transition_out = None;
                continue;
            };
            if transition.to_clip_id != *to_id {
                clip.transition_out = None;
                continue;
            }
            transition.duration_frames = transition.duration_frames.clamp(1, *maximum);
        }
    }
}

fn result(
    state: &EditorState,
    timeline_changed: bool,
    manifest_changed: bool,
    action_name: &str,
    affected: Vec<String>,
    summary: &str,
) -> EditResult {
    EditResult {
        changed: timeline_changed || manifest_changed,
        timeline_changed,
        manifest_changed,
        action_name: action_name.to_string(),
        affected_clip_ids: affected,
        timeline_version: state.version(),
        summary: summary.to_string(),
    }
}

#[cfg(test)]
mod transaction_tests {
    use super::*;

    #[test]
    fn failed_transaction_restores_document_and_history() {
        let mut state = EditorState::default();
        let before = state.snapshot();

        let error = transact(
            &mut state,
            "Injected Failure",
            |_| String::new(),
            |state| {
                state.timeline.fps = 60;
                state.manifest.folders.push(opentake_domain::MediaFolder {
                    id: "partial-folder".into(),
                    name: "Partial".into(),
                    parent_folder_id: None,
                });
                Err(EditError::Invalid("injected after mutation".into()))
            },
        )
        .unwrap_err();

        assert_eq!(error, EditError::Invalid("injected after mutation".into()));
        assert_eq!(state.snapshot(), before);
        assert_eq!(state.version(), 0);
        assert!(!state.can_undo());
        assert!(!state.can_redo());
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

fn add_clips_auto_track(
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
        validate_auto_track_entry(e, i)?;
    }
    let has_visual = entries
        .iter()
        .any(|entry| entry.source_clip_type != ClipType::Audio);
    let has_audio = entries
        .iter()
        .any(|entry| entry.source_clip_type == ClipType::Audio);
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
            let visual_track_index = has_visual.then(|| {
                let at = st.timeline.tracks.len();
                ops::insert_track(&mut st.timeline, at, ClipType::Video, ids)
            });
            let audio_track_index = has_audio.then(|| {
                let at = st.timeline.tracks.len();
                ops::insert_track(&mut st.timeline, at, ClipType::Audio, ids)
            });
            let mut placed = Vec::new();
            for entry in &entries {
                let track_index = if entry.source_clip_type == ClipType::Audio {
                    audio_track_index
                } else {
                    visual_track_index
                }
                .expect("validated required track kind above");
                let mut entry = entry.clone();
                entry.track_index = track_index;
                let track_id = st.timeline.tracks[track_index].id.clone();
                if let Some(ti) = st.track_index(&track_id) {
                    ops::clear_region(
                        &mut st.timeline,
                        ti,
                        entry.start_frame,
                        entry.start_frame + entry.duration_frames,
                        false,
                        ids,
                    );
                }
                if let Some(ti) = st.track_index(&track_id) {
                    placed.extend(ops::place_clip(
                        &mut st.timeline,
                        &entry.to_spec(),
                        ti,
                        None,
                        ids,
                    ));
                }
            }
            Ok(placed)
        },
    )
}

fn insert_track_cmd(
    state: &mut EditorState,
    kind: ClipType,
    at: Option<usize>,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    transact(
        state,
        "Insert Track",
        |added| format!("Inserted track: {}", added.join(", ")),
        |st| {
            let requested = at.unwrap_or(st.timeline.tracks.len());
            let idx = ops::insert_track(&mut st.timeline, requested, kind, ids);
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

fn swap_tracks(state: &mut EditorState, a: usize, b: usize) -> Result<EditResult, EditError> {
    let track_count = state.timeline.tracks.len();
    if a >= track_count || b >= track_count {
        return Err(EditError::Invalid(format!(
            "track index out of range: a={a}, b={b}, timeline has {track_count} track(s)"
        )));
    }
    transact(
        state,
        "Swap Tracks",
        move |_| format!("Swapped tracks {a} and {b}"),
        |st| {
            ops::swap_tracks(&mut st.timeline, a, b);
            Ok(Vec::new())
        },
    )
}

/// Swap the positions of two clips. The op refuses (leaves the timeline
/// untouched) when the swap would overlap a third clip; `transact` then reports
/// `changed = false`, so a refused swap is a clean no-op with no undo entry.
fn swap_clips(state: &mut EditorState, a: String, b: String) -> Result<EditResult, EditError> {
    if state.find_clip(&a).is_none() {
        return Err(EditError::Invalid(format!("Clip not found: {a}")));
    }
    if state.find_clip(&b).is_none() {
        return Err(EditError::Invalid(format!("Clip not found: {b}")));
    }
    transact(
        state,
        "Swap Clips",
        |_| "Swapped clip positions".to_string(),
        move |st| {
            ops::swap_clip_positions(&mut st.timeline, &a, &b);
            Ok(vec![a, b])
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
        if !e.media_type.is_compatible(target_type) {
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

/// Option/Alt-drag duplicate: deep-copy each clip to a new position. See
/// [`EditCommand::DuplicateClips`].
fn duplicate_clips_cmd(
    state: &mut EditorState,
    clip_ids: Vec<String>,
    offset_frames: i32,
    target_track_indexes: Vec<usize>,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    if clip_ids.is_empty() {
        return Err(EditError::Invalid(
            "Missing or empty 'clipIds' array".into(),
        ));
    }
    if target_track_indexes.len() != clip_ids.len() {
        return Err(EditError::Invalid(format!(
            "targetTrackIndexes length ({}) must match clipIds length ({})",
            target_track_indexes.len(),
            clip_ids.len()
        )));
    }
    for id in &clip_ids {
        if state.find_clip(id).is_none() {
            return Err(EditError::Invalid(format!("Clip not found: {id}")));
        }
    }
    let action_name = if clip_ids.len() == 1 {
        "Duplicate Clip"
    } else {
        "Duplicate Clips"
    };
    let n = clip_ids.len();
    transact(
        state,
        action_name,
        move |_| format!("Duplicated {n} clip(s)"),
        |st| {
            Ok(ops::duplicate_clips(
                &mut st.timeline,
                &clip_ids,
                offset_frames,
                &target_track_indexes,
                ids,
            ))
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

fn freeze_frame(
    state: &mut EditorState,
    clip_id: String,
    at_frame: i32,
    duration_frames: i32,
    media_ref: String,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    let loc = state
        .find_clip(&clip_id)
        .ok_or_else(|| EditError::Invalid(format!("Clip not found: {clip_id}")))?;
    let clip = &state.timeline.tracks[loc.track_index].clips[loc.clip_index];
    if !(at_frame > clip.start_frame && at_frame < clip.end_frame()) {
        return Err(EditError::Invalid(format!(
            "Frame {at_frame} must be strictly inside clip range ({}..{})",
            clip.start_frame,
            clip.end_frame()
        )));
    }
    if duration_frames < 1 {
        return Err(EditError::Invalid(format!(
            "durationFrames must be >= 1 (got {duration_frames})"
        )));
    }
    if !matches!(clip.media_type, ClipType::Video | ClipType::Image) {
        return Err(EditError::Invalid(format!(
            "Freeze Frame requires a video or image clip (got {:?})",
            clip.media_type
        )));
    }
    let track_id = state.timeline.tracks[loc.track_index].id.clone();
    transact(
        state,
        "Freeze Frame",
        |_| format!("Froze {clip_id} at frame {at_frame} for {duration_frames} frame(s)"),
        |st| {
            let Some(ti) = st.track_index(&track_id) else {
                return Err(EditError::Invalid("Track vanished".into()));
            };
            let mut affected = ops::split_clip(&mut st.timeline, &clip_id, at_frame, ids);
            let spec = PlaceSpec::new(media_ref, ClipType::Image, at_frame, duration_frames);
            affected.extend(ops::ripple::ripple_insert(
                &mut st.timeline,
                std::slice::from_ref(&spec),
                ti,
                at_frame,
                ids,
            ));
            Ok(affected)
        },
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
        let location = state
            .find_clip(id)
            .ok_or_else(|| EditError::Invalid(format!("Clip not found: {id}")))?;
        let clip = &state.timeline.tracks[location.track_index].clips[location.clip_index];
        if clip.nested_sequence_id.is_some()
            && (props
                .speed
                .is_some_and(|speed| (speed - 1.0).abs() > f64::EPSILON)
                || props.reversed == Some(true)
                || props.crop.is_some_and(|crop| crop != Crop::default())
                || props.text_content.is_some()
                || props.text_style.is_some())
        {
            return Err(EditError::Invalid(format!(
                "compound clip {id} does not support retime, reverse, crop, or text properties"
            )));
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
    if let Some(c) = props.crop {
        clip.crop = c;
        clip.crop_track = None;
    }
    if let Some(v) = props.fade_in_frames {
        clip.fade_in_frames = v.max(0);
        clip.clamp_fades_to_duration();
    }
    if let Some(v) = props.fade_out_frames {
        clip.fade_out_frames = v.max(0);
        clip.clamp_fades_to_duration();
    }
    if let Some(i) = props.fade_in_interpolation {
        clip.fade_in_interpolation = i;
    }
    if let Some(i) = props.fade_out_interpolation {
        clip.fade_out_interpolation = i;
    }
    if let Some(f) = props.flip_horizontal {
        clip.transform.flip_horizontal = f;
    }
    if let Some(f) = props.flip_vertical {
        clip.transform.flip_vertical = f;
    }
    if let Some(reversed) = props.reversed {
        clip.reversed = reversed;
    }
    if let Some(c) = &props.text_content {
        clip.text_content = Some(c.clone());
    }
    if let Some(s) = &props.text_style {
        clip.text_style = Some(s.clone());
    }
}

fn set_keyframes(
    state: &mut EditorState,
    clip_id: String,
    property: KeyframeProperty,
    payload: KeyframePayload,
) -> Result<EditResult, EditError> {
    let location = state
        .find_clip(&clip_id)
        .ok_or_else(|| EditError::Invalid(format!("Clip not found: {clip_id}")))?;
    if property == KeyframeProperty::Crop
        && state.timeline.tracks[location.track_index].clips[location.clip_index]
            .nested_sequence_id
            .is_some()
    {
        return Err(EditError::Invalid(
            "compound clips do not support crop keyframes".into(),
        ));
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

fn stamp_keyframe(
    state: &mut EditorState,
    clip_id: String,
    property: KeyframeProperty,
    frame: i32,
) -> Result<EditResult, EditError> {
    let loc = state
        .find_clip(&clip_id)
        .ok_or_else(|| EditError::Invalid(format!("Clip not found: {clip_id}")))?;
    let clip = &state.timeline.tracks[loc.track_index].clips[loc.clip_index];
    if property == KeyframeProperty::Crop && clip.nested_sequence_id.is_some() {
        return Err(EditError::Invalid(
            "compound clips do not support crop keyframes".into(),
        ));
    }
    if !clip.contains(frame) {
        return Err(EditError::Invalid(format!(
            "Frame {frame} is outside clip range ({}..{})",
            clip.start_frame,
            clip.end_frame()
        )));
    }
    let summary = format!("Stamp keyframe on {clip_id}");
    transact(
        state,
        "Stamp Keyframe",
        move |_| summary,
        move |st| {
            let loc = st.find_clip(&clip_id).expect("validated above");
            let clip = &mut st.timeline.tracks[loc.track_index].clips[loc.clip_index];
            let rel = frame - clip.start_frame;
            match property {
                KeyframeProperty::Opacity => {
                    let v = clip.raw_opacity_at(frame);
                    let mut track = clip.opacity_track.take().unwrap_or_default();
                    track.upsert(opentake_domain::Keyframe::new(rel, v));
                    clip.opacity_track = empty_to_none(track);
                }
                KeyframeProperty::Volume => {
                    let v = clip
                        .volume_track
                        .as_ref()
                        .map(|t| t.sample(rel, 0.0))
                        .unwrap_or(0.0);
                    let mut track = clip.volume_track.take().unwrap_or_default();
                    track.upsert(opentake_domain::Keyframe::new(rel, v));
                    clip.volume_track = empty_to_none(track);
                }
                KeyframeProperty::Rotation => {
                    let v = clip.rotation_at(frame);
                    let mut track = clip.rotation_track.take().unwrap_or_default();
                    track.upsert(opentake_domain::Keyframe::new(rel, v));
                    clip.rotation_track = empty_to_none(track);
                }
                KeyframeProperty::Position => {
                    let tl = clip.top_left_at(frame);
                    let v = opentake_domain::AnimPair::new(tl.x, tl.y);
                    let mut track = clip.position_track.take().unwrap_or_default();
                    track.upsert(opentake_domain::Keyframe::new(rel, v));
                    clip.position_track = empty_to_none(track);
                }
                KeyframeProperty::Scale => {
                    let sz = clip.size_at(frame);
                    let v = opentake_domain::AnimPair::new(sz.0, sz.1);
                    let mut track = clip.scale_track.take().unwrap_or_default();
                    track.upsert(opentake_domain::Keyframe::new(rel, v));
                    clip.scale_track = empty_to_none(track);
                }
                KeyframeProperty::Crop => {
                    let v = clip.crop_at(frame);
                    let mut track = clip.crop_track.take().unwrap_or_default();
                    track.upsert(opentake_domain::Keyframe::new(rel, v));
                    clip.crop_track = empty_to_none(track);
                }
            }
            Ok(vec![clip_id])
        },
    )
}

/// Write an explicit `value` into `property`'s keyframe track at `frame`
/// (absolute timeline frame), upserting in place (creating the track if
/// absent). Unlike [`stamp_keyframe`], the value is supplied by the caller
/// rather than sampled from the clip's current state — this is the missing
/// half of upstream's animation authoring (`write<Property>`'s
/// `clip.upsertKeyframe(...)` branch; the static-value branch already exists
/// via `SetClipProperties`). Does NOT touch the clip's static field.
fn upsert_keyframe(
    state: &mut EditorState,
    clip_id: String,
    property: KeyframeProperty,
    frame: i32,
    value: KeyframeValue,
) -> Result<EditResult, EditError> {
    let loc = state
        .find_clip(&clip_id)
        .ok_or_else(|| EditError::Invalid(format!("Clip not found: {clip_id}")))?;
    let clip = &state.timeline.tracks[loc.track_index].clips[loc.clip_index];
    if property == KeyframeProperty::Crop && clip.nested_sequence_id.is_some() {
        return Err(EditError::Invalid(
            "compound clips do not support crop keyframes".into(),
        ));
    }
    if !clip.contains(frame) {
        return Err(EditError::Invalid(format!(
            "Frame {frame} is outside clip range ({}..{})",
            clip.start_frame,
            clip.end_frame()
        )));
    }
    // Type/property agreement check (mirrors `set_keyframes`).
    let ok = matches!(
        (property, value),
        (KeyframeProperty::Opacity, KeyframeValue::Scalar(_))
            | (KeyframeProperty::Volume, KeyframeValue::Scalar(_))
            | (KeyframeProperty::Rotation, KeyframeValue::Scalar(_))
            | (KeyframeProperty::Position, KeyframeValue::Pair(_))
            | (KeyframeProperty::Scale, KeyframeValue::Pair(_))
            | (KeyframeProperty::Crop, KeyframeValue::Crop(_))
    );
    if !ok {
        return Err(EditError::Invalid(
            "keyframe value type does not match property".into(),
        ));
    }
    let summary = format!("Set keyframe on {clip_id}");
    transact(
        state,
        "Set Keyframe",
        move |_| summary,
        move |st| {
            let loc = st.find_clip(&clip_id).expect("validated above");
            let clip = &mut st.timeline.tracks[loc.track_index].clips[loc.clip_index];
            let rel = frame - clip.start_frame;
            match (property, value) {
                (KeyframeProperty::Opacity, KeyframeValue::Scalar(v)) => {
                    let mut track = clip.opacity_track.take().unwrap_or_default();
                    track.upsert(opentake_domain::Keyframe::new(rel, v));
                    clip.opacity_track = empty_to_none(track);
                }
                (KeyframeProperty::Volume, KeyframeValue::Scalar(v)) => {
                    let mut track = clip.volume_track.take().unwrap_or_default();
                    track.upsert(opentake_domain::Keyframe::new(rel, v));
                    clip.volume_track = empty_to_none(track);
                }
                (KeyframeProperty::Rotation, KeyframeValue::Scalar(v)) => {
                    let mut track = clip.rotation_track.take().unwrap_or_default();
                    track.upsert(opentake_domain::Keyframe::new(rel, v));
                    clip.rotation_track = empty_to_none(track);
                }
                (KeyframeProperty::Position, KeyframeValue::Pair(v)) => {
                    let mut track = clip.position_track.take().unwrap_or_default();
                    track.upsert(opentake_domain::Keyframe::new(rel, v));
                    clip.position_track = empty_to_none(track);
                }
                (KeyframeProperty::Scale, KeyframeValue::Pair(v)) => {
                    let mut track = clip.scale_track.take().unwrap_or_default();
                    track.upsert(opentake_domain::Keyframe::new(rel, v));
                    clip.scale_track = empty_to_none(track);
                }
                (KeyframeProperty::Crop, KeyframeValue::Crop(v)) => {
                    let mut track = clip.crop_track.take().unwrap_or_default();
                    track.upsert(opentake_domain::Keyframe::new(rel, v));
                    clip.crop_track = empty_to_none(track);
                }
                _ => unreachable!("validated above"),
            }
            Ok(vec![clip_id])
        },
    )
}

fn remove_keyframe(
    state: &mut EditorState,
    clip_id: String,
    property: KeyframeProperty,
    frame: i32,
) -> Result<EditResult, EditError> {
    let loc = state
        .find_clip(&clip_id)
        .ok_or_else(|| EditError::Invalid(format!("Clip not found: {clip_id}")))?;
    let clip = &state.timeline.tracks[loc.track_index].clips[loc.clip_index];
    let rel = frame - clip.start_frame;
    let has_kf = match property {
        KeyframeProperty::Opacity => has_keyframe_at(&clip.opacity_track, rel),
        KeyframeProperty::Volume => has_keyframe_at(&clip.volume_track, rel),
        KeyframeProperty::Rotation => has_keyframe_at(&clip.rotation_track, rel),
        KeyframeProperty::Position => has_keyframe_at(&clip.position_track, rel),
        KeyframeProperty::Scale => has_keyframe_at(&clip.scale_track, rel),
        KeyframeProperty::Crop => has_keyframe_at(&clip.crop_track, rel),
    };
    if !has_kf {
        return Err(EditError::Invalid(format!(
            "Keyframe not found at frame {frame}"
        )));
    }
    let summary = format!("Remove keyframe on {clip_id}");
    transact(
        state,
        "Remove Keyframe",
        move |_| summary,
        move |st| {
            let loc = st.find_clip(&clip_id).expect("validated above");
            let clip = &mut st.timeline.tracks[loc.track_index].clips[loc.clip_index];
            let rel = frame - clip.start_frame;
            match property {
                KeyframeProperty::Opacity => {
                    if let Some(mut t) = clip.opacity_track.take() {
                        t.remove(rel);
                        clip.opacity_track = empty_to_none(t);
                    }
                }
                KeyframeProperty::Volume => {
                    if let Some(mut t) = clip.volume_track.take() {
                        t.remove(rel);
                        clip.volume_track = empty_to_none(t);
                    }
                }
                KeyframeProperty::Rotation => {
                    if let Some(mut t) = clip.rotation_track.take() {
                        t.remove(rel);
                        clip.rotation_track = empty_to_none(t);
                    }
                }
                KeyframeProperty::Position => {
                    if let Some(mut t) = clip.position_track.take() {
                        t.remove(rel);
                        clip.position_track = empty_to_none(t);
                    }
                }
                KeyframeProperty::Scale => {
                    if let Some(mut t) = clip.scale_track.take() {
                        t.remove(rel);
                        clip.scale_track = empty_to_none(t);
                    }
                }
                KeyframeProperty::Crop => {
                    if let Some(mut t) = clip.crop_track.take() {
                        t.remove(rel);
                        clip.crop_track = empty_to_none(t);
                    }
                }
            }
            Ok(vec![clip_id])
        },
    )
}

fn move_keyframe(
    state: &mut EditorState,
    clip_id: String,
    property: KeyframeProperty,
    from_frame: i32,
    to_frame: i32,
) -> Result<EditResult, EditError> {
    let loc = state
        .find_clip(&clip_id)
        .ok_or_else(|| EditError::Invalid(format!("Clip not found: {clip_id}")))?;
    let clip = &state.timeline.tracks[loc.track_index].clips[loc.clip_index];
    let from_rel = from_frame - clip.start_frame;
    let to_rel = to_frame - clip.start_frame;
    let has_source = match property {
        KeyframeProperty::Opacity => has_keyframe_at(&clip.opacity_track, from_rel),
        KeyframeProperty::Volume => has_keyframe_at(&clip.volume_track, from_rel),
        KeyframeProperty::Rotation => has_keyframe_at(&clip.rotation_track, from_rel),
        KeyframeProperty::Position => has_keyframe_at(&clip.position_track, from_rel),
        KeyframeProperty::Scale => has_keyframe_at(&clip.scale_track, from_rel),
        KeyframeProperty::Crop => has_keyframe_at(&clip.crop_track, from_rel),
    };
    if !has_source {
        return Err(EditError::Invalid(format!(
            "Keyframe not found at frame {from_frame}"
        )));
    }
    // Validate target frame is within clip range (half-open [start, end)).
    if !clip.contains(to_frame) {
        return Err(EditError::Invalid(format!(
            "Target frame {to_frame} is outside clip range ({}..{})",
            clip.start_frame,
            clip.end_frame()
        )));
    }
    if from_rel != to_rel {
        let target_occupied = match property {
            KeyframeProperty::Opacity => has_keyframe_at(&clip.opacity_track, to_rel),
            KeyframeProperty::Volume => has_keyframe_at(&clip.volume_track, to_rel),
            KeyframeProperty::Rotation => has_keyframe_at(&clip.rotation_track, to_rel),
            KeyframeProperty::Position => has_keyframe_at(&clip.position_track, to_rel),
            KeyframeProperty::Scale => has_keyframe_at(&clip.scale_track, to_rel),
            KeyframeProperty::Crop => has_keyframe_at(&clip.crop_track, to_rel),
        };
        if target_occupied {
            return Err(EditError::Invalid(format!(
                "Target frame {to_frame} already occupied"
            )));
        }
    }
    let summary = format!("Move keyframe on {clip_id}");
    transact(
        state,
        "Move Keyframe",
        move |_| summary,
        move |st| {
            let loc = st.find_clip(&clip_id).expect("validated above");
            let clip = &mut st.timeline.tracks[loc.track_index].clips[loc.clip_index];
            let from_rel = from_frame - clip.start_frame;
            let to_rel = to_frame - clip.start_frame;
            match property {
                KeyframeProperty::Opacity => {
                    if let Some(mut t) = clip.opacity_track.take() {
                        t.move_keyframe(from_rel, to_rel);
                        clip.opacity_track = empty_to_none(t);
                    }
                }
                KeyframeProperty::Volume => {
                    if let Some(mut t) = clip.volume_track.take() {
                        t.move_keyframe(from_rel, to_rel);
                        clip.volume_track = empty_to_none(t);
                    }
                }
                KeyframeProperty::Rotation => {
                    if let Some(mut t) = clip.rotation_track.take() {
                        t.move_keyframe(from_rel, to_rel);
                        clip.rotation_track = empty_to_none(t);
                    }
                }
                KeyframeProperty::Position => {
                    if let Some(mut t) = clip.position_track.take() {
                        t.move_keyframe(from_rel, to_rel);
                        clip.position_track = empty_to_none(t);
                    }
                }
                KeyframeProperty::Scale => {
                    if let Some(mut t) = clip.scale_track.take() {
                        t.move_keyframe(from_rel, to_rel);
                        clip.scale_track = empty_to_none(t);
                    }
                }
                KeyframeProperty::Crop => {
                    if let Some(mut t) = clip.crop_track.take() {
                        t.move_keyframe(from_rel, to_rel);
                        clip.crop_track = empty_to_none(t);
                    }
                }
            }
            Ok(vec![clip_id])
        },
    )
}

fn set_keyframe_interpolation(
    state: &mut EditorState,
    clip_id: String,
    property: KeyframeProperty,
    frame: i32,
    interpolation: opentake_domain::Interpolation,
) -> Result<EditResult, EditError> {
    let loc = state
        .find_clip(&clip_id)
        .ok_or_else(|| EditError::Invalid(format!("Clip not found: {clip_id}")))?;
    let clip = &state.timeline.tracks[loc.track_index].clips[loc.clip_index];
    let rel = frame - clip.start_frame;
    let has_kf = match property {
        KeyframeProperty::Opacity => has_keyframe_at(&clip.opacity_track, rel),
        KeyframeProperty::Volume => has_keyframe_at(&clip.volume_track, rel),
        KeyframeProperty::Rotation => has_keyframe_at(&clip.rotation_track, rel),
        KeyframeProperty::Position => has_keyframe_at(&clip.position_track, rel),
        KeyframeProperty::Scale => has_keyframe_at(&clip.scale_track, rel),
        KeyframeProperty::Crop => has_keyframe_at(&clip.crop_track, rel),
    };
    if !has_kf {
        return Err(EditError::Invalid(format!(
            "Keyframe not found at frame {frame}"
        )));
    }
    let summary = format!("Set keyframe interpolation on {clip_id}");
    transact(
        state,
        "Set Keyframe Interpolation",
        move |_| summary,
        move |st| {
            let loc = st.find_clip(&clip_id).expect("validated above");
            let clip = &mut st.timeline.tracks[loc.track_index].clips[loc.clip_index];
            let rel = frame - clip.start_frame;
            match property {
                KeyframeProperty::Opacity => {
                    set_kf_interp(&mut clip.opacity_track, rel, interpolation)
                }
                KeyframeProperty::Volume => {
                    set_kf_interp(&mut clip.volume_track, rel, interpolation)
                }
                KeyframeProperty::Rotation => {
                    set_kf_interp(&mut clip.rotation_track, rel, interpolation)
                }
                KeyframeProperty::Position => {
                    set_kf_interp(&mut clip.position_track, rel, interpolation)
                }
                KeyframeProperty::Scale => set_kf_interp(&mut clip.scale_track, rel, interpolation),
                KeyframeProperty::Crop => set_kf_interp(&mut clip.crop_track, rel, interpolation),
            }
            Ok(vec![clip_id])
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

fn reject_compound_effect_targets(
    state: &EditorState,
    clip_ids: &[String],
    adding_effect: bool,
) -> Result<(), EditError> {
    if !adding_effect {
        return Ok(());
    }
    for id in clip_ids {
        let location = state
            .find_clip(id)
            .ok_or_else(|| EditError::Invalid(format!("Clip not found: {id}")))?;
        if state.timeline.tracks[location.track_index].clips[location.clip_index]
            .nested_sequence_id
            .is_some()
        {
            return Err(EditError::Invalid(format!(
                "compound clip {id} does not support direct pixel effects"
            )));
        }
    }
    Ok(())
}

fn set_color_grade(
    state: &mut EditorState,
    clip_ids: Vec<String>,
    grade: Option<ColorGrade>,
) -> Result<EditResult, EditError> {
    reject_compound_effect_targets(state, &clip_ids, grade.is_some())?;
    set_clip_effect_field(state, clip_ids, "Set Color Grade", move |clip| {
        clip.color_grade = grade;
    })
}

fn set_chroma_key(
    state: &mut EditorState,
    clip_ids: Vec<String>,
    chroma_key: Option<ChromaKey>,
) -> Result<EditResult, EditError> {
    reject_compound_effect_targets(state, &clip_ids, chroma_key.is_some())?;
    set_clip_effect_field(state, clip_ids, "Set Chroma Key", move |clip| {
        clip.chroma_key = chroma_key;
    })
}

fn set_masks(
    state: &mut EditorState,
    clip_ids: Vec<String>,
    masks: Vec<Mask>,
) -> Result<EditResult, EditError> {
    if masks.len() > MAX_MASKS_PER_CLIP {
        return Err(EditError::Invalid(format!(
            "a clip supports at most {MAX_MASKS_PER_CLIP} masks"
        )));
    }
    for (index, mask) in masks.iter().enumerate() {
        if let MaskShape::Poly { points } = &mask.shape {
            if points.len() < 3 || points.len() > MAX_POLYGON_MASK_POINTS {
                return Err(EditError::Invalid(format!(
                    "mask {index} polygon must contain 3..={MAX_POLYGON_MASK_POINTS} points"
                )));
            }
        }
    }
    reject_compound_effect_targets(state, &clip_ids, !masks.is_empty())?;
    set_clip_effect_field(state, clip_ids, "Set Masks", move |clip| {
        clip.masks = masks.clone();
    })
}

fn set_effects(
    state: &mut EditorState,
    clip_ids: Vec<String>,
    effects: Vec<Effect>,
) -> Result<EditResult, EditError> {
    reject_compound_effect_targets(state, &clip_ids, !effects.is_empty())?;
    set_clip_effect_field(state, clip_ids, "Set Effects", move |clip| {
        clip.effects = effects.clone();
    })
}

fn set_transition(
    state: &mut EditorState,
    from_clip_id: String,
    to_clip_id: String,
    kind: Option<TransitionKind>,
    duration_frames: i32,
) -> Result<EditResult, EditError> {
    let from_location = state
        .find_clip(&from_clip_id)
        .ok_or_else(|| EditError::Invalid(format!("Clip not found: {from_clip_id}")))?;

    if kind.is_some()
        && state.timeline.tracks[from_location.track_index].clips[from_location.clip_index]
            .nested_sequence_id
            .is_some()
    {
        return Err(EditError::Invalid(
            "compound clips do not support direct transitions".into(),
        ));
    }

    // Clearing is allowed even after the pair stopped being adjacent so stale
    // metadata can always be removed safely. Pair identity must still match.
    if kind.is_none() {
        return transact(
            state,
            "Remove Transition",
            |_| "Removed transition".to_string(),
            |st| {
                let clip = &mut st.timeline.tracks[from_location.track_index].clips
                    [from_location.clip_index];
                if clip
                    .transition_out
                    .as_ref()
                    .is_some_and(|transition| transition.to_clip_id == to_clip_id)
                {
                    clip.transition_out = None;
                }
                Ok(vec![from_clip_id.clone(), to_clip_id.clone()])
            },
        );
    }

    if duration_frames < 1 {
        return Err(EditError::Invalid(format!(
            "durationFrames must be >= 1 (got {duration_frames})"
        )));
    }
    let to_location = state
        .find_clip(&to_clip_id)
        .ok_or_else(|| EditError::Invalid(format!("Clip not found: {to_clip_id}")))?;
    if state.timeline.tracks[to_location.track_index].clips[to_location.clip_index]
        .nested_sequence_id
        .is_some()
    {
        return Err(EditError::Invalid(
            "compound clips do not support direct transitions".into(),
        ));
    }
    if from_location.track_index != to_location.track_index {
        return Err(EditError::Invalid(
            "A transition requires clips on the same track".into(),
        ));
    }
    let track = &state.timeline.tracks[from_location.track_index];
    if track.kind == ClipType::Audio {
        return Err(EditError::Invalid(
            "Visual transitions are unavailable on audio tracks".into(),
        ));
    }
    let from = &track.clips[from_location.clip_index];
    let to = &track.clips[to_location.clip_index];
    if matches!(from.media_type, ClipType::Audio | ClipType::Text)
        || matches!(to.media_type, ClipType::Audio | ClipType::Text)
    {
        return Err(EditError::Invalid(
            "A transition requires two visual source clips".into(),
        ));
    }
    if from.end_frame() != to.start_frame {
        return Err(EditError::Invalid(
            "A transition requires an exact adjacent clip boundary".into(),
        ));
    }
    let mut ordered: Vec<&opentake_domain::Clip> = track.clips.iter().collect();
    ordered.sort_by_key(|clip| (clip.start_frame, clip.id.as_str()));
    let successor = ordered
        .iter()
        .position(|clip| clip.id == from_clip_id)
        .and_then(|index| ordered.get(index + 1));
    if successor.map(|clip| clip.id.as_str()) != Some(to_clip_id.as_str()) {
        return Err(EditError::Invalid(
            "A transition requires the immediate next clip".into(),
        ));
    }

    let maximum = (from.duration_frames.min(to.duration_frames) / 2).max(1);
    let clamped = duration_frames.min(maximum);
    let kind = kind.expect("kind checked above");
    transact(
        state,
        "Set Transition",
        |_| format!("Set transition from {from_clip_id} to {to_clip_id}"),
        |st| {
            st.timeline.tracks[from_location.track_index].clips[from_location.clip_index]
                .transition_out = Some(Transition {
                to_clip_id: to_clip_id.clone(),
                kind,
                duration_frames: clamped,
            });
            Ok(vec![from_clip_id.clone(), to_clip_id.clone()])
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
            // (it can't here 鈥?refusal is dry-run first 鈥?but keep it airtight).
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
            Ok(result(
                state,
                changed,
                false,
                "Ripple Delete",
                affected,
                &summary,
            ))
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
                false,
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

/// Add text overlays onto one fresh video track inserted at index 0, as a
/// single "Add Text(s)" transaction. 1:1 port of upstream `addTexts`'s
/// all-omitted-trackIndex path (`ToolExecutor+Texts.swift:102-121`): a new top
/// track is created and every entry lands there, so an existing track's
/// content is never touched. Unlike [`add_captions`] (whose fresh track is
/// exclusively new caption content and never overlaps itself), entries here
/// still `clear_region` each other in order — matching `add_texts` and
/// upstream `placeTextClips`'s per-track `clearRegion` pass — so a caller
/// batching overlapping entries gets the same "later entry wins" overwrite
/// semantics as targeting an existing track explicitly.
fn add_texts_auto_track(
    state: &mut EditorState,
    entries: Vec<TextAutoTrackEntry>,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    if entries.is_empty() {
        return Err(EditError::Invalid(
            "Missing or empty 'entries' array".into(),
        ));
    }
    for (i, e) in entries.iter().enumerate() {
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
            // Fresh video track at the very top, same slot upstream's
            // `addTexts`/`addTextClip` always insert into.
            st.timeline.tracks.insert(
                0,
                opentake_domain::Track::new(ids.next_id(), ClipType::Video),
            );
            let mut added = Vec::with_capacity(entries.len());
            for e in &entries {
                ops::clear_region(
                    &mut st.timeline,
                    0,
                    e.start_frame,
                    e.start_frame + e.duration_frames,
                    false,
                    ids,
                );
                let mut clip =
                    opentake_domain::Clip::new(ids.next_id(), "", e.start_frame, e.duration_frames);
                clip.media_type = ClipType::Text;
                clip.source_clip_type = ClipType::Text;
                clip.transform = e.transform;
                clip.text_content = Some(e.content.clone());
                clip.text_style = Some(e.text_style.clone());
                added.push(clip.id.clone());
                st.timeline.tracks[0].clips.push(clip);
                ops::sort_clips(&mut st.timeline.tracks[0]);
            }
            Ok(added)
        },
    )
}

/// Place a batch of built caption clips on one fresh video track at index 0, as a
/// single "Generate Captions" transaction. 1:1 port of upstream `placeCaptionTrack`
/// (`EditorViewModel+Captions.swift:226-242`): insert `Track(type: .video)` at 0,
/// place every caption clip there (each carrying its `caption_group_id`), and
/// commit once. Empty input is a no-op. Unlike `add_texts` this never clears a
/// region — the track is brand new and exclusively the caption track, so clips
/// are appended directly and sorted (upstream `placeTextClips` onto an empty
/// track reduces to the same).
fn add_captions(
    state: &mut EditorState,
    entries: Vec<CaptionEntry>,
    ids: &dyn IdGen,
) -> Result<EditResult, EditError> {
    if entries.is_empty() {
        // No captions built (e.g. no speech detected): no track, no change.
        // Matches upstream returning `[]` and restoring `timeline` before commit.
        return Ok(result(
            state,
            false,
            false,
            "Generate Captions",
            Vec::new(),
            "",
        ));
    }
    for (i, e) in entries.iter().enumerate() {
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
    transact(
        state,
        "Generate Captions",
        |c| format!("Added {} caption(s): {}", c.len(), c.join(", ")),
        |st| {
            // Fresh video track at the very top (upstream inserts at index 0).
            st.timeline.tracks.insert(
                0,
                opentake_domain::Track::new(ids.next_id(), ClipType::Video),
            );
            let mut added = Vec::with_capacity(entries.len());
            for e in &entries {
                let mut clip =
                    opentake_domain::Clip::new(ids.next_id(), "", e.start_frame, e.duration_frames);
                clip.media_type = ClipType::Text;
                clip.source_clip_type = ClipType::Text;
                clip.transform = e.transform;
                clip.text_content = Some(e.content.clone());
                clip.text_style = Some(e.text_style.clone());
                clip.caption_group_id = Some(e.caption_group_id.clone());
                added.push(clip.id.clone());
                st.timeline.tracks[0].clips.push(clip);
            }
            ops::sort_clips(&mut st.timeline.tracks[0]);
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

/// Replace a clip's `media_ref` in place, preserving every editing attribute
/// (transform / crop / keyframe tracks / grade / masks / effects / fade / text
/// / trim / speed / start / duration). 1:1 port of upstream
/// `replaceClipMediaRef(resetTrim: false)`:
///
/// 1. Validate the seed clip exists and the candidate asset exists in the
///    manifest, then refuse unless `clip.media_type == asset.kind` (strict
///    equality — no `isVisual` leniency). A video clip can only be swapped to
///    a video asset, an audio clip only to an audio asset, etc.
/// 2. Walk the seed clip's link group, picking every clip that shares the
///    same `media_ref`. Each one is updated to the new ref in the same
///    transaction, so a linked audio/video pair pointing at the same file
///    stays in sync (and `Undo` restores every old ref atomically).
/// 3. **No** trim / duration / start rewrites — `resetTrim: false`. The render
///    layer is responsible for any overshoot sampling when the new media is
///    shorter.
/// 4. Same `media_ref` is a no-op (`changed = false`, no undo entry, no
///    version bump).
fn swap_media(
    state: &mut EditorState,
    clip_id: String,
    media_ref: String,
) -> Result<EditResult, EditError> {
    // 1. Seed clip must exist.
    let seed_loc = state
        .find_clip(&clip_id)
        .ok_or_else(|| EditError::Invalid(format!("Clip not found: {clip_id}")))?;

    // 2. Candidate asset must exist in the manifest.
    let new_asset = state
        .manifest
        .entries
        .iter()
        .find(|e| e.id == media_ref)
        .ok_or_else(|| EditError::Invalid(format!("Media not found: {media_ref}")))?;

    // 3. Strict type-match: clip.media_type == asset.kind. No isVisual leniency,
    //    no media_type override. A video clip can only swap to a video asset,
    //    an audio clip only to an audio asset.
    let seed_media_type =
        state.timeline.tracks[seed_loc.track_index].clips[seed_loc.clip_index].media_type;
    if seed_media_type != new_asset.kind {
        return Err(EditError::Refused(format!(
            "Type mismatch: clip is {:?}, asset is {:?}",
            seed_media_type, new_asset.kind
        )));
    }

    // 4. No-op when the seed already references the new media.
    let seed_old_ref = state.timeline.tracks[seed_loc.track_index].clips[seed_loc.clip_index]
        .media_ref
        .clone();
    if seed_old_ref == media_ref {
        let version = state.version();
        return Ok(EditResult {
            changed: false,
            timeline_changed: false,
            manifest_changed: false,
            action_name: "Swap Media".to_string(),
            affected_clip_ids: vec![clip_id.clone()],
            timeline_version: version,
            summary: format!("No-op: {clip_id} already references {media_ref}"),
        });
    }

    // 5. Collect every link-group partner that also references the old ref.
    //    `expand_to_link_group` returns the whole group; we then keep only
    //    the members whose `media_ref` matches the seed's old ref.
    let link_group = ops::expand_to_link_group(&state.timeline, &{
        let mut s = HashSet::new();
        s.insert(clip_id.clone());
        s
    });
    let mut targets: Vec<String> = Vec::new();
    for member_id in &link_group {
        if let Some(loc) = state.find_clip(member_id) {
            let c = &state.timeline.tracks[loc.track_index].clips[loc.clip_index];
            if c.media_ref == seed_old_ref {
                targets.push(member_id.clone());
            }
        }
    }
    if !targets.iter().any(|id| id == &clip_id) {
        // Defensive: the seed itself must always be in the target set.
        targets.push(clip_id.clone());
    }

    let summary_old = seed_old_ref;
    let summary_new = media_ref.clone();
    let target_count = targets.len();
    transact(
        state,
        "Swap Media",
        move |affected| {
            if affected.len() <= 1 {
                format!("Swapped {clip_id}: {summary_old} -> {summary_new}")
            } else {
                format!(
                    "Swapped {n} linked clips: {summary_old} -> {summary_new}",
                    n = affected.len()
                )
            }
        },
        move |st| {
            let mut affected = Vec::with_capacity(target_count);
            for tid in &targets {
                if let Some(loc) = st.find_clip(tid) {
                    st.timeline.tracks[loc.track_index].clips[loc.clip_index].media_ref =
                        media_ref.clone();
                    affected.push(tid.clone());
                }
            }
            Ok(affected)
        },
    )
}

/// 1:1 port of upstream's Inspector "Reset transform" button
/// (`InspectorView.transformHeader`'s `onReset` closure): resets `transform`
/// to identity, `opacity` to `1.0`, clears the opacity / position / scale /
/// rotation keyframe tracks, and zeroes both fades back to `Linear`
/// interpolation. Crop is a separate section upstream and is left untouched.
fn reset_transform(
    state: &mut EditorState,
    clip_ids: Vec<String>,
) -> Result<EditResult, EditError> {
    set_clip_effect_field(state, clip_ids, "Reset Transform", |clip| {
        clip.transform = Transform::default();
        clip.opacity = 1.0;
        clip.opacity_track = None;
        clip.position_track = None;
        clip.scale_track = None;
        clip.rotation_track = None;
        clip.fade_in_frames = 0;
        clip.fade_out_frames = 0;
        clip.fade_in_interpolation = Interpolation::Linear;
        clip.fade_out_interpolation = Interpolation::Linear;
    })
}

/// Change project timeline settings (FPS / resolution). Validates positivity up
/// front (a bad request is a hard error, not a silent no-op), then delegates to
/// `ops::set_timeline_settings` inside a transaction; an unchanged request
/// (identical already-configured settings) commits nothing.
fn set_timeline_settings_cmd(
    state: &mut EditorState,
    fps: i32,
    width: i32,
    height: i32,
) -> Result<EditResult, EditError> {
    if fps <= 0 || width <= 0 || height <= 0 {
        return Err(EditError::Invalid(format!(
            "timeline settings must be positive (got fps={fps}, width={width}, height={height})"
        )));
    }
    transact(
        state,
        "Change Project Settings",
        move |_| format!("Set timeline to {width}×{height} @ {fps} fps"),
        |st| {
            ops::set_timeline_settings(&mut st.timeline, fps, width, height);
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
    // Destination compatibility is determined by the placed lane type. A
    // linked audio clip can legitimately retain `source_clip_type = Video`
    // because it still resolves audio from the original video asset.
    if !e.media_type.is_compatible(target) {
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

fn validate_auto_track_entry(e: &ClipEntry, i: usize) -> Result<(), EditError> {
    let target = if e.media_type == ClipType::Audio {
        ClipType::Audio
    } else {
        ClipType::Video
    };
    if !e.media_type.is_compatible(target) {
        return Err(EditError::Invalid(format!(
            "entries[{i}]: asset type is not compatible with an auto-created track"
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

fn has_keyframe_at<V>(t_opt: &Option<opentake_domain::KeyframeTrack<V>>, rel: i32) -> bool {
    t_opt
        .as_ref()
        .map(|t| t.keyframes.iter().any(|k| k.frame == rel))
        .unwrap_or(false)
}

fn set_kf_interp<V>(
    t_opt: &mut Option<opentake_domain::KeyframeTrack<V>>,
    rel: i32,
    interpolation: opentake_domain::Interpolation,
) {
    if let Some(t) = t_opt {
        for kf in &mut t.keyframes {
            if kf.frame == rel {
                kf.interpolation_out = interpolation;
            }
        }
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
                at: None,
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
                at: None,
            },
            &ids,
        )
        .unwrap();
        assert_eq!(state.timeline.tracks.len(), 2);
        assert_eq!(state.timeline.tracks[1].kind, ClipType::Audio);
    }

    #[test]
    fn insert_track_honors_requested_index() {
        let mut state = EditorState::default();
        let ids = SeqIdGen::default();

        apply(
            &mut state,
            EditCommand::InsertTrack {
                kind: ClipType::Video,
                at: None,
            },
            &ids,
        )
        .unwrap();
        let first_id = state.timeline.tracks[0].id.clone();
        apply(
            &mut state,
            EditCommand::InsertTrack {
                kind: ClipType::Audio,
                at: None,
            },
            &ids,
        )
        .unwrap();

        apply(
            &mut state,
            EditCommand::InsertTrack {
                kind: ClipType::Video,
                at: Some(0),
            },
            &ids,
        )
        .unwrap();

        assert_eq!(state.timeline.tracks[1].id, first_id);
        assert_eq!(state.timeline.tracks[0].kind, ClipType::Video);
        assert_eq!(state.timeline.tracks[2].kind, ClipType::Audio);
    }

    #[test]
    fn set_track_props_toggles_only_given_fields() {
        let mut state = EditorState::default();
        let ids = SeqIdGen::default();
        apply(
            &mut state,
            EditCommand::InsertTrack {
                kind: ClipType::Audio,
                at: None,
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

#[cfg(test)]
mod keyframe_edit_tests {
    use super::*;
    use crate::id::SeqIdGen;
    use opentake_domain::{ClipType, Interpolation, Keyframe, KeyframeTrack};

    /// Build a state with one video track and one clip at [100, 130).
    fn make_state_with_clip() -> (EditorState, SeqIdGen, String) {
        let mut state = EditorState::default();
        let ids = SeqIdGen::default();
        apply(
            &mut state,
            EditCommand::InsertTrack {
                kind: ClipType::Video,
                at: None,
            },
            &ids,
        )
        .unwrap();
        let clip_id = ids.next_id();
        let clip = opentake_domain::Clip::new(clip_id.clone(), "asset1", 100, 30);
        state.timeline.tracks[0].clips.push(clip);
        (state, ids, clip_id)
    }

    fn set_opacity_track(state: &mut EditorState, clip_id: &str, kfs: Vec<Keyframe<f64>>) {
        let loc = state.find_clip(clip_id).unwrap();
        state.timeline.tracks[loc.track_index].clips[loc.clip_index].opacity_track =
            Some(KeyframeTrack::from_keyframes(kfs));
    }

    fn opacity_track_kfs(state: &EditorState, clip_id: &str) -> Vec<(i32, f64, Interpolation)> {
        let loc = state.find_clip(clip_id).unwrap();
        let clip = &state.timeline.tracks[loc.track_index].clips[loc.clip_index];
        clip.opacity_track
            .as_ref()
            .map(|t| {
                t.keyframes
                    .iter()
                    .map(|k| (k.frame, k.value, k.interpolation_out))
                    .collect()
            })
            .unwrap_or_default()
    }

    // --- StampKeyframe ---

    #[test]
    fn stamp_keyframe_creates_track_when_absent() {
        let (mut state, ids, clip_id) = make_state_with_clip();
        // No opacity track initially.
        let loc = state.find_clip(&clip_id).unwrap();
        assert!(state.timeline.tracks[loc.track_index].clips[loc.clip_index]
            .opacity_track
            .is_none());

        let res = apply(
            &mut state,
            EditCommand::StampKeyframe {
                clip_id: clip_id.clone(),
                property: KeyframeProperty::Opacity,
                frame: 110, // rel 10
            },
            &ids,
        )
        .unwrap();
        assert!(res.changed);
        assert_eq!(res.affected_clip_ids, vec![clip_id.clone()]);

        let kfs = opacity_track_kfs(&state, &clip_id);
        assert_eq!(kfs.len(), 1);
        assert_eq!(kfs[0].0, 10); // rel frame
                                  // Default opacity is 1.0, so stamped value is 1.0.
        approx(kfs[0].1, 1.0);
    }

    #[test]
    fn stamp_keyframe_upserts_existing() {
        let (mut state, ids, clip_id) = make_state_with_clip();
        // Pre-existing track with a kf at rel 10.
        set_opacity_track(&mut state, &clip_id, vec![Keyframe::new(10, 0.5)]);

        apply(
            &mut state,
            EditCommand::StampKeyframe {
                clip_id: clip_id.clone(),
                property: KeyframeProperty::Opacity,
                frame: 110, // rel 10 閳?same as existing kf
            },
            &ids,
        )
        .unwrap();

        // Upsert should not duplicate.
        let kfs = opacity_track_kfs(&state, &clip_id);
        assert_eq!(kfs.len(), 1);
        assert_eq!(kfs[0].0, 10);
    }

    #[test]
    fn stamp_keyframe_clip_not_found() {
        let (mut state, ids, _clip_id) = make_state_with_clip();
        let err = apply(
            &mut state,
            EditCommand::StampKeyframe {
                clip_id: "nonexistent".into(),
                property: KeyframeProperty::Opacity,
                frame: 110,
            },
            &ids,
        );
        assert!(err.is_err());
    }

    #[test]
    fn stamp_keyframe_frame_outside_clip() {
        let (mut state, ids, clip_id) = make_state_with_clip();
        // Clip spans [100, 130). Frame 200 is outside.
        let err = apply(
            &mut state,
            EditCommand::StampKeyframe {
                clip_id,
                property: KeyframeProperty::Opacity,
                frame: 200,
            },
            &ids,
        );
        assert!(err.is_err());
    }

    // --- RemoveKeyframe ---

    #[test]
    fn remove_keyframe_deletes_and_clears_empty_track() {
        let (mut state, ids, clip_id) = make_state_with_clip();
        set_opacity_track(&mut state, &clip_id, vec![Keyframe::new(10, 0.5)]);

        apply(
            &mut state,
            EditCommand::RemoveKeyframe {
                clip_id: clip_id.clone(),
                property: KeyframeProperty::Opacity,
                frame: 110, // rel 10
            },
            &ids,
        )
        .unwrap();

        // Track should be cleared to None when empty.
        let loc = state.find_clip(&clip_id).unwrap();
        assert!(state.timeline.tracks[loc.track_index].clips[loc.clip_index]
            .opacity_track
            .is_none());
    }

    #[test]
    fn remove_keyframe_not_found() {
        let (mut state, ids, clip_id) = make_state_with_clip();
        set_opacity_track(&mut state, &clip_id, vec![Keyframe::new(0, 0.5)]);

        let err = apply(
            &mut state,
            EditCommand::RemoveKeyframe {
                clip_id,
                property: KeyframeProperty::Opacity,
                frame: 110, // rel 10 閳?no kf here
            },
            &ids,
        );
        assert!(err.is_err());
    }

    // --- MoveKeyframe ---

    #[test]
    fn move_keyframe_to_empty() {
        let (mut state, ids, clip_id) = make_state_with_clip();
        set_opacity_track(&mut state, &clip_id, vec![Keyframe::new(0, 0.5)]);

        apply(
            &mut state,
            EditCommand::MoveKeyframe {
                clip_id: clip_id.clone(),
                property: KeyframeProperty::Opacity,
                from_frame: 100, // rel 0
                to_frame: 110,   // rel 10
            },
            &ids,
        )
        .unwrap();

        let kfs = opacity_track_kfs(&state, &clip_id);
        assert_eq!(kfs.len(), 1);
        assert_eq!(kfs[0].0, 10); // moved to rel 10
    }

    #[test]
    fn move_keyframe_target_occupied() {
        let (mut state, ids, clip_id) = make_state_with_clip();
        set_opacity_track(
            &mut state,
            &clip_id,
            vec![Keyframe::new(0, 0.0), Keyframe::new(10, 1.0)],
        );

        let err = apply(
            &mut state,
            EditCommand::MoveKeyframe {
                clip_id,
                property: KeyframeProperty::Opacity,
                from_frame: 100, // rel 0
                to_frame: 110,   // rel 10 閳?occupied
            },
            &ids,
        );
        assert!(err.is_err());
    }

    #[test]
    fn move_keyframe_source_missing() {
        let (mut state, ids, clip_id) = make_state_with_clip();
        set_opacity_track(&mut state, &clip_id, vec![Keyframe::new(0, 0.5)]);

        let err = apply(
            &mut state,
            EditCommand::MoveKeyframe {
                clip_id,
                property: KeyframeProperty::Opacity,
                from_frame: 115, // rel 15 閳?no kf
                to_frame: 120,   // rel 20
            },
            &ids,
        );
        assert!(err.is_err());
    }

    #[test]
    fn move_keyframe_target_outside_clip() {
        let (mut state, ids, clip_id) = make_state_with_clip();
        // Clip spans [100, 130). Frame 200 is outside.
        set_opacity_track(&mut state, &clip_id, vec![Keyframe::new(0, 0.5)]);
        let err = apply(
            &mut state,
            EditCommand::MoveKeyframe {
                clip_id,
                property: KeyframeProperty::Opacity,
                from_frame: 100, // rel 0
                to_frame: 200,   // outside clip range
            },
            &ids,
        );
        assert!(err.is_err());
    }

    // --- SetKeyframeInterpolation ---

    #[test]
    fn set_keyframe_interpolation_changes_mode() {
        let (mut state, ids, clip_id) = make_state_with_clip();
        // Keyframe::new defaults to Smooth.
        set_opacity_track(&mut state, &clip_id, vec![Keyframe::new(0, 0.5)]);
        assert_eq!(
            opacity_track_kfs(&state, &clip_id)[0].2,
            Interpolation::Smooth
        );

        apply(
            &mut state,
            EditCommand::SetKeyframeInterpolation {
                clip_id: clip_id.clone(),
                property: KeyframeProperty::Opacity,
                frame: 100, // rel 0
                interpolation: Interpolation::Linear,
            },
            &ids,
        )
        .unwrap();

        let kfs = opacity_track_kfs(&state, &clip_id);
        assert_eq!(kfs[0].2, Interpolation::Linear);
    }

    #[test]
    fn set_keyframe_interpolation_kf_not_found() {
        let (mut state, ids, clip_id) = make_state_with_clip();
        set_opacity_track(&mut state, &clip_id, vec![Keyframe::new(0, 0.5)]);

        let err = apply(
            &mut state,
            EditCommand::SetKeyframeInterpolation {
                clip_id,
                property: KeyframeProperty::Opacity,
                frame: 115, // rel 15 閳?no kf
                interpolation: Interpolation::Linear,
            },
            &ids,
        );
        assert!(err.is_err());
    }

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }
}

#[cfg(test)]
mod upsert_keyframe_tests {
    use super::*;
    use crate::id::SeqIdGen;
    use opentake_domain::{ClipType, Crop, Keyframe, KeyframeTrack};

    /// Build a state with one video track and one clip at [100, 130) (start
    /// frame != 0, so `rel = frame - start` is exercised, not just identity).
    fn make_state_with_clip() -> (EditorState, SeqIdGen, String) {
        let mut state = EditorState::default();
        let ids = SeqIdGen::default();
        apply(
            &mut state,
            EditCommand::InsertTrack {
                kind: ClipType::Video,
                at: None,
            },
            &ids,
        )
        .unwrap();
        let clip_id = ids.next_id();
        let clip = opentake_domain::Clip::new(clip_id.clone(), "asset1", 100, 30);
        state.timeline.tracks[0].clips.push(clip);
        (state, ids, clip_id)
    }

    fn opacity_track_kfs(state: &EditorState, clip_id: &str) -> Vec<(i32, f64)> {
        let loc = state.find_clip(clip_id).unwrap();
        let clip = &state.timeline.tracks[loc.track_index].clips[loc.clip_index];
        clip.opacity_track
            .as_ref()
            .map(|t| t.keyframes.iter().map(|k| (k.frame, k.value)).collect())
            .unwrap_or_default()
    }

    #[test]
    fn creates_track_on_clean_clip() {
        let (mut state, ids, clip_id) = make_state_with_clip();
        let loc = state.find_clip(&clip_id).unwrap();
        assert!(state.timeline.tracks[loc.track_index].clips[loc.clip_index]
            .opacity_track
            .is_none());

        let res = apply(
            &mut state,
            EditCommand::UpsertKeyframe {
                clip_id: clip_id.clone(),
                property: KeyframeProperty::Opacity,
                frame: 110, // rel 10 (start_frame = 100)
                value: KeyframeValue::Scalar(0.25),
            },
            &ids,
        )
        .unwrap();
        assert!(res.changed);
        assert_eq!(res.affected_clip_ids, vec![clip_id.clone()]);

        let kfs = opacity_track_kfs(&state, &clip_id);
        assert_eq!(kfs, vec![(10, 0.25)]);
    }

    #[test]
    fn upsert_at_existing_rel_overwrites_value() {
        let (mut state, ids, clip_id) = make_state_with_clip();
        let loc = state.find_clip(&clip_id).unwrap();
        state.timeline.tracks[loc.track_index].clips[loc.clip_index].opacity_track =
            Some(KeyframeTrack::from_keyframes(vec![Keyframe::new(10, 0.5)]));

        apply(
            &mut state,
            EditCommand::UpsertKeyframe {
                clip_id: clip_id.clone(),
                property: KeyframeProperty::Opacity,
                frame: 110, // rel 10 — same as existing kf
                value: KeyframeValue::Scalar(0.9),
            },
            &ids,
        )
        .unwrap();

        // Upsert overwrites in place — no duplicate keyframe.
        let kfs = opacity_track_kfs(&state, &clip_id);
        assert_eq!(kfs, vec![(10, 0.9)]);
    }

    #[test]
    fn wrong_value_variant_for_property_is_invalid() {
        let (mut state, ids, clip_id) = make_state_with_clip();

        // Opacity requires Scalar, not Pair.
        let err = apply(
            &mut state,
            EditCommand::UpsertKeyframe {
                clip_id: clip_id.clone(),
                property: KeyframeProperty::Opacity,
                frame: 110,
                value: KeyframeValue::Pair(opentake_domain::AnimPair::new(0.0, 0.0)),
            },
            &ids,
        );
        assert!(matches!(err, Err(EditError::Invalid(_))));

        // Position requires Pair, not Scalar.
        let err = apply(
            &mut state,
            EditCommand::UpsertKeyframe {
                clip_id: clip_id.clone(),
                property: KeyframeProperty::Position,
                frame: 110,
                value: KeyframeValue::Scalar(1.0),
            },
            &ids,
        );
        assert!(matches!(err, Err(EditError::Invalid(_))));

        // Crop requires Crop, not Scalar.
        let err = apply(
            &mut state,
            EditCommand::UpsertKeyframe {
                clip_id,
                property: KeyframeProperty::Crop,
                frame: 110,
                value: KeyframeValue::Scalar(1.0),
            },
            &ids,
        );
        assert!(matches!(err, Err(EditError::Invalid(_))));
    }

    #[test]
    fn frame_outside_clip_range_is_invalid() {
        let (mut state, ids, clip_id) = make_state_with_clip();
        // Clip spans [100, 130). Frame 200 is outside.
        let err = apply(
            &mut state,
            EditCommand::UpsertKeyframe {
                clip_id,
                property: KeyframeProperty::Opacity,
                frame: 200,
                value: KeyframeValue::Scalar(0.5),
            },
            &ids,
        );
        assert!(matches!(err, Err(EditError::Invalid(_))));
    }

    #[test]
    fn clip_not_found_is_invalid() {
        let (mut state, ids, _clip_id) = make_state_with_clip();
        let err = apply(
            &mut state,
            EditCommand::UpsertKeyframe {
                clip_id: "nonexistent".into(),
                property: KeyframeProperty::Opacity,
                frame: 110,
                value: KeyframeValue::Scalar(0.5),
            },
            &ids,
        );
        assert!(matches!(err, Err(EditError::Invalid(_))));
    }

    /// `rel` is computed as `frame - start_frame`; verify with a clip whose
    /// `start_frame != 0` (the fixture uses 100) and confirm the round-trip
    /// value samples back correctly for each payload shape.
    #[test]
    fn rel_offset_computed_from_nonzero_start_frame_and_value_round_trips() {
        let (mut state, ids, clip_id) = make_state_with_clip();

        // Pair (Position): value round-trips via `sample`.
        apply(
            &mut state,
            EditCommand::UpsertKeyframe {
                clip_id: clip_id.clone(),
                property: KeyframeProperty::Position,
                frame: 115, // rel 15
                value: KeyframeValue::Pair(opentake_domain::AnimPair::new(0.3, 0.7)),
            },
            &ids,
        )
        .unwrap();
        let loc = state.find_clip(&clip_id).unwrap();
        let clip = &state.timeline.tracks[loc.track_index].clips[loc.clip_index];
        let position_track = clip.position_track.as_ref().expect("position track");
        assert_eq!(position_track.keyframes.len(), 1);
        assert_eq!(position_track.keyframes[0].frame, 15);
        let sampled = position_track.sample(15, opentake_domain::AnimPair::new(0.0, 0.0));
        approx(sampled.a, 0.3);
        approx(sampled.b, 0.7);

        // Crop: value round-trips via `sample`.
        let crop = Crop {
            left: 0.1,
            top: 0.2,
            right: 0.3,
            bottom: 0.4,
        };
        apply(
            &mut state,
            EditCommand::UpsertKeyframe {
                clip_id: clip_id.clone(),
                property: KeyframeProperty::Crop,
                frame: 120, // rel 20
                value: KeyframeValue::Crop(crop),
            },
            &ids,
        )
        .unwrap();
        let loc = state.find_clip(&clip_id).unwrap();
        let clip = &state.timeline.tracks[loc.track_index].clips[loc.clip_index];
        let crop_track = clip.crop_track.as_ref().expect("crop track");
        assert_eq!(crop_track.keyframes[0].frame, 20);
        let sampled_crop = crop_track.sample(20, Crop::default());
        assert_eq!(sampled_crop, crop);

        // Volume (dB): value round-trips via `sample` — the track stores raw
        // dB, unconverted (see `clip.volume_at` which does
        // `VolumeScale::linear_from_db(t.sample(...))` on top of this stored
        // value).
        apply(
            &mut state,
            EditCommand::UpsertKeyframe {
                clip_id: clip_id.clone(),
                property: KeyframeProperty::Volume,
                frame: 125, // rel 25
                value: KeyframeValue::Scalar(-6.0),
            },
            &ids,
        )
        .unwrap();
        let loc = state.find_clip(&clip_id).unwrap();
        let clip = &state.timeline.tracks[loc.track_index].clips[loc.clip_index];
        let volume_track = clip.volume_track.as_ref().expect("volume track");
        assert_eq!(volume_track.keyframes[0].frame, 25);
        approx(volume_track.sample(25, 0.0), -6.0);
    }

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }
}

#[cfg(test)]
mod duplicate_clips_tests {
    use super::*;
    use crate::id::SeqIdGen;
    use opentake_domain::{Clip, ClipType, Keyframe, KeyframeTrack, Track};

    fn state_with_clip() -> EditorState {
        let mut tl = Timeline::new();
        let mut t = Track::new("v1", ClipType::Video);
        t.clips.push(Clip::new("c1", "asset", 0, 30));
        tl.tracks.push(t);
        EditorState::from_timeline(tl)
    }

    #[test]
    fn duplicate_clips_creates_copy_with_new_id() {
        let mut state = state_with_clip();
        let ids = SeqIdGen::new("d-");
        let res = apply(
            &mut state,
            EditCommand::DuplicateClips {
                clip_ids: vec!["c1".into()],
                offset_frames: 100,
                target_track_indexes: vec![0],
            },
            &ids,
        )
        .unwrap();
        assert!(res.changed);
        assert_eq!(res.action_name, "Duplicate Clip");
        assert_eq!(res.affected_clip_ids.len(), 1);
        // Original retained, copy present at frame 100.
        assert!(state.timeline.tracks[0].clips.iter().any(|c| c.id == "c1"));
        let copy = state.timeline.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == res.affected_clip_ids[0])
            .unwrap();
        assert_eq!(copy.start_frame, 100);
        assert_ne!(copy.id, "c1");
    }

    #[test]
    fn duplicate_clips_deep_copies_keyframe_tracks() {
        let mut state = state_with_clip();
        state.timeline.tracks[0].clips[0].opacity_track =
            Some(KeyframeTrack::from_keyframes(vec![
                Keyframe::new(0, 0.0),
                Keyframe::new(30, 1.0),
            ]));
        let ids = SeqIdGen::default();
        let res = apply(
            &mut state,
            EditCommand::DuplicateClips {
                clip_ids: vec!["c1".into()],
                offset_frames: 100,
                target_track_indexes: vec![0],
            },
            &ids,
        )
        .unwrap();
        let copy = state.timeline.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == res.affected_clip_ids[0])
            .unwrap();
        let op = copy.opacity_track.as_ref().unwrap();
        assert_eq!(
            op.keyframes.iter().map(|k| k.frame).collect::<Vec<_>>(),
            vec![0, 30]
        );
    }

    #[test]
    fn duplicate_clips_clears_link_group_id() {
        let mut state = state_with_clip();
        state.timeline.tracks[0].clips[0].link_group_id = Some("grp".into());
        let ids = SeqIdGen::default();
        let res = apply(
            &mut state,
            EditCommand::DuplicateClips {
                clip_ids: vec!["c1".into()],
                offset_frames: 50,
                target_track_indexes: vec![0],
            },
            &ids,
        )
        .unwrap();
        let copy = state.timeline.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == res.affected_clip_ids[0])
            .unwrap();
        assert!(copy.link_group_id.is_none());
    }

    #[test]
    fn duplicate_clips_missing_clip_errors() {
        let mut state = state_with_clip();
        let ids = SeqIdGen::default();
        let err = apply(
            &mut state,
            EditCommand::DuplicateClips {
                clip_ids: vec!["nope".into()],
                offset_frames: 100,
                target_track_indexes: vec![0],
            },
            &ids,
        );
        assert!(matches!(err, Err(EditError::Invalid(_))));
    }

    #[test]
    fn duplicate_clips_length_mismatch_errors() {
        let mut state = state_with_clip();
        let ids = SeqIdGen::default();
        let err = apply(
            &mut state,
            EditCommand::DuplicateClips {
                clip_ids: vec!["c1".into()],
                offset_frames: 100,
                target_track_indexes: vec![0, 1], // wrong length
            },
            &ids,
        );
        assert!(matches!(err, Err(EditError::Invalid(_))));
    }

    #[test]
    fn duplicate_clips_empty_ids_errors() {
        let mut state = state_with_clip();
        let ids = SeqIdGen::default();
        let err = apply(
            &mut state,
            EditCommand::DuplicateClips {
                clip_ids: vec![],
                offset_frames: 100,
                target_track_indexes: vec![],
            },
            &ids,
        );
        assert!(matches!(err, Err(EditError::Invalid(_))));
    }

    #[test]
    fn duplicate_clips_is_undoable() {
        let mut state = state_with_clip();
        let ids = SeqIdGen::default();
        let version_before = state.version();
        apply(
            &mut state,
            EditCommand::DuplicateClips {
                clip_ids: vec!["c1".into()],
                offset_frames: 100,
                target_track_indexes: vec![0],
            },
            &ids,
        )
        .unwrap();
        assert_eq!(state.timeline.tracks[0].clips.len(), 2);
        assert!(state.can_undo());
        apply(&mut state, EditCommand::Undo, &ids).unwrap();
        assert_eq!(state.timeline.tracks[0].clips.len(), 1);
        assert_eq!(state.timeline.tracks[0].clips[0].id, "c1");
        assert_eq!(state.version(), version_before + 2); // commit + undo
    }
}

#[cfg(test)]
mod text_style_property_tests {
    use super::*;
    use crate::id::SeqIdGen;
    use opentake_domain::{Clip, ClipType, TextAlignment, TextStyle, Track};

    fn state_with_text_clip() -> EditorState {
        let mut tl = Timeline::new();
        let mut t = Track::new("v1", ClipType::Video);
        let mut clip = Clip::new("c1", "", 0, 30);
        clip.media_type = ClipType::Text;
        clip.source_clip_type = ClipType::Text;
        clip.text_content = Some("Hi".into());
        clip.text_style = Some(TextStyle::default());
        t.clips.push(clip);
        tl.tracks.push(t);
        EditorState::from_timeline(tl)
    }

    #[test]
    fn set_text_style_replaces_clip_style_and_is_undoable() {
        let mut state = state_with_text_clip();
        let ids = SeqIdGen::default();
        let version_before = state.version();

        let style = TextStyle {
            font_name: "Times-Bold".into(),
            font_size: 48.0,
            alignment: TextAlignment::Left,
            ..Default::default()
        };
        let res = apply(
            &mut state,
            EditCommand::SetClipProperties {
                clip_ids: vec!["c1".into()],
                properties: Box::new(ClipProperties {
                    text_style: Some(style.clone()),
                    ..Default::default()
                }),
            },
            &ids,
        )
        .unwrap();

        assert!(res.changed);
        let applied = state.timeline.tracks[0].clips[0]
            .text_style
            .as_ref()
            .expect("text_style present");
        assert_eq!(applied.font_name, "Times-Bold");
        assert_eq!(applied.font_size, 48.0);
        assert_eq!(applied.alignment, TextAlignment::Left);

        // Undo restores the original default style.
        apply(&mut state, EditCommand::Undo, &ids).unwrap();
        let restored = state.timeline.tracks[0].clips[0]
            .text_style
            .as_ref()
            .expect("text_style present");
        assert_eq!(restored.font_name, "Helvetica-Bold");
        assert_eq!(state.version(), version_before + 2); // commit + undo
    }

    #[test]
    fn set_text_style_alongside_text_content() {
        let mut state = state_with_text_clip();
        let ids = SeqIdGen::default();

        let style = TextStyle {
            font_size: 120.0,
            ..Default::default()
        };
        apply(
            &mut state,
            EditCommand::SetClipProperties {
                clip_ids: vec!["c1".into()],
                properties: Box::new(ClipProperties {
                    text_content: Some("Updated".into()),
                    text_style: Some(style),
                    ..Default::default()
                }),
            },
            &ids,
        )
        .unwrap();

        let clip = &state.timeline.tracks[0].clips[0];
        assert_eq!(clip.text_content.as_deref(), Some("Updated"));
        assert_eq!(clip.text_style.as_ref().unwrap().font_size, 120.0);
    }
}

#[cfg(test)]
mod reversed_property_tests {
    use super::*;
    use crate::id::SeqIdGen;
    use opentake_domain::{Clip, ClipType, Track};

    fn state_with_video_clips() -> EditorState {
        let mut tl = Timeline::new();
        let mut t = Track::new("v1", ClipType::Video);
        t.clips.push(Clip::new("c1", "asset", 0, 30));
        t.clips.push(Clip::new("c2", "asset", 40, 30));
        tl.tracks.push(t);
        EditorState::from_timeline(tl)
    }

    #[test]
    fn set_clip_properties_reversed_sets_only_requested_clip() {
        let mut state = state_with_video_clips();
        let ids = SeqIdGen::default();
        let video_id = state.timeline.tracks[0].clips[0].id.clone();
        let untouched_id = state.timeline.tracks[0].clips[1].id.clone();
        let before = state.timeline.clone();

        let result = apply(
            &mut state,
            EditCommand::SetClipProperties {
                clip_ids: vec![video_id.clone()],
                properties: Box::new(ClipProperties {
                    reversed: Some(true),
                    ..Default::default()
                }),
            },
            &ids,
        )
        .unwrap();

        assert!(result.changed);
        let clip = state
            .timeline
            .tracks
            .iter()
            .flat_map(|track| &track.clips)
            .find(|clip| clip.id == video_id)
            .unwrap();
        assert!(clip.reversed);
        let untouched = state
            .timeline
            .tracks
            .iter()
            .flat_map(|track| &track.clips)
            .find(|clip| clip.id == untouched_id)
            .unwrap();
        assert!(!untouched.reversed);
        assert_ne!(state.timeline, before);
    }
}

#[cfg(test)]
mod reset_transform_tests {
    use super::*;
    use crate::id::SeqIdGen;
    use opentake_domain::{AnimPair, ClipType, Interpolation, Keyframe, KeyframeTrack};

    /// Build a state with one video track and two clips: `clip_id` is fully
    /// animated (transform / opacity / fades / all four tracks), `other_id` is
    /// left untouched to prove the reset is scoped to the requested clip.
    fn make_state_with_animated_clip() -> (EditorState, SeqIdGen, String, String) {
        let mut state = EditorState::default();
        let ids = SeqIdGen::default();
        apply(
            &mut state,
            EditCommand::InsertTrack {
                kind: ClipType::Video,
                at: None,
            },
            &ids,
        )
        .unwrap();
        let clip_id = ids.next_id();
        let mut clip = opentake_domain::Clip::new(clip_id.clone(), "asset1", 0, 30);
        clip.transform = Transform {
            center_x: 0.25,
            center_y: 0.75,
            width: 2.0,
            height: 3.0,
            rotation: 45.0,
            flip_horizontal: true,
            flip_vertical: true,
        };
        clip.opacity = 0.5;
        clip.opacity_track = Some(KeyframeTrack::from_keyframes(vec![Keyframe::new(0, 0.2)]));
        clip.position_track = Some(KeyframeTrack::from_keyframes(vec![Keyframe::new(
            0,
            AnimPair::new(0.1, 0.1),
        )]));
        clip.scale_track = Some(KeyframeTrack::from_keyframes(vec![Keyframe::new(
            0,
            AnimPair::new(1.5, 1.5),
        )]));
        clip.rotation_track = Some(KeyframeTrack::from_keyframes(vec![Keyframe::new(0, 90.0)]));
        clip.crop = Crop {
            left: 0.1,
            top: 0.1,
            right: 0.1,
            bottom: 0.1,
        };
        clip.crop_track = Some(KeyframeTrack::from_keyframes(vec![Keyframe::new(
            0,
            Crop {
                left: 0.2,
                top: 0.2,
                right: 0.2,
                bottom: 0.2,
            },
        )]));
        clip.fade_in_frames = 10;
        clip.fade_out_frames = 10;
        clip.fade_in_interpolation = Interpolation::Smooth;
        clip.fade_out_interpolation = Interpolation::Smooth;
        state.timeline.tracks[0].clips.push(clip);

        let other_id = ids.next_id();
        let mut other = opentake_domain::Clip::new(other_id.clone(), "asset2", 40, 30);
        other.transform.rotation = 30.0;
        other.opacity = 0.7;
        state.timeline.tracks[0].clips.push(other);

        (state, ids, clip_id, other_id)
    }

    #[test]
    fn reset_transform_restores_defaults_and_clears_exactly_the_upstream_tracks() {
        let (mut state, ids, clip_id, _other_id) = make_state_with_animated_clip();
        let res = apply(
            &mut state,
            EditCommand::ResetTransform {
                clip_ids: vec![clip_id.clone()],
            },
            &ids,
        )
        .unwrap();
        assert!(res.changed);
        assert_eq!(res.affected_clip_ids, vec![clip_id.clone()]);

        let loc = state.find_clip(&clip_id).unwrap();
        let clip = &state.timeline.tracks[loc.track_index].clips[loc.clip_index];

        // Transform back to identity (full-canvas centered, no rotate/flip).
        assert_eq!(clip.transform, Transform::default());
        assert_eq!(clip.transform.center_x, 0.5);
        assert_eq!(clip.transform.center_y, 0.5);
        assert_eq!(clip.transform.width, 1.0);
        assert_eq!(clip.transform.height, 1.0);
        assert_eq!(clip.transform.rotation, 0.0);
        assert!(!clip.transform.flip_horizontal);
        assert!(!clip.transform.flip_vertical);

        // Opacity back to fully opaque.
        assert_eq!(clip.opacity, 1.0);

        // Exactly the four animation tracks upstream clears.
        assert!(clip.opacity_track.is_none());
        assert!(clip.position_track.is_none());
        assert!(clip.scale_track.is_none());
        assert!(clip.rotation_track.is_none());

        // Fades zeroed and interpolation reset to Linear.
        assert_eq!(clip.fade_in_frames, 0);
        assert_eq!(clip.fade_out_frames, 0);
        assert_eq!(clip.fade_in_interpolation, Interpolation::Linear);
        assert_eq!(clip.fade_out_interpolation, Interpolation::Linear);

        // Crop and its keyframe track are a separate Inspector section
        // upstream and must survive the reset untouched.
        assert_eq!(
            clip.crop,
            Crop {
                left: 0.1,
                top: 0.1,
                right: 0.1,
                bottom: 0.1,
            }
        );
        assert!(clip.crop_track.is_some());
    }

    #[test]
    fn reset_transform_leaves_other_clips_untouched() {
        let (mut state, ids, clip_id, other_id) = make_state_with_animated_clip();
        apply(
            &mut state,
            EditCommand::ResetTransform {
                clip_ids: vec![clip_id],
            },
            &ids,
        )
        .unwrap();

        let loc = state.find_clip(&other_id).unwrap();
        let other = &state.timeline.tracks[loc.track_index].clips[loc.clip_index];
        assert_eq!(other.transform.rotation, 30.0);
        assert_eq!(other.opacity, 0.7);
    }

    #[test]
    fn reset_transform_is_undoable() {
        let (mut state, ids, clip_id, _other_id) = make_state_with_animated_clip();
        let version_before = state.version();
        apply(
            &mut state,
            EditCommand::ResetTransform {
                clip_ids: vec![clip_id.clone()],
            },
            &ids,
        )
        .unwrap();

        apply(&mut state, EditCommand::Undo, &ids).unwrap();
        let loc = state.find_clip(&clip_id).unwrap();
        let clip = &state.timeline.tracks[loc.track_index].clips[loc.clip_index];
        assert_eq!(clip.transform.rotation, 45.0);
        assert_eq!(clip.opacity, 0.5);
        assert!(clip.opacity_track.is_some());
        assert_eq!(state.version(), version_before + 2); // reset commit + undo
    }

    #[test]
    fn reset_transform_missing_clip_errors_with_no_change() {
        let mut state = EditorState::default();
        let ids = SeqIdGen::default();
        let version_before = state.version();

        let err = apply(
            &mut state,
            EditCommand::ResetTransform {
                clip_ids: vec!["does-not-exist".into()],
            },
            &ids,
        );
        assert!(err.is_err());
        assert_eq!(state.version(), version_before);
    }

    #[test]
    fn reset_transform_empty_clip_ids_errors() {
        let mut state = EditorState::default();
        let ids = SeqIdGen::default();
        let err = apply(
            &mut state,
            EditCommand::ResetTransform { clip_ids: vec![] },
            &ids,
        );
        assert!(err.is_err());
    }

    #[test]
    fn reset_transform_noop_when_already_default_reports_unchanged() {
        let mut state = EditorState::default();
        let ids = SeqIdGen::default();
        apply(
            &mut state,
            EditCommand::InsertTrack {
                kind: ClipType::Video,
                at: None,
            },
            &ids,
        )
        .unwrap();
        let clip_id = ids.next_id();
        let clip = opentake_domain::Clip::new(clip_id.clone(), "asset1", 0, 30);
        state.timeline.tracks[0].clips.push(clip);
        let version_before = state.version();

        let res = apply(
            &mut state,
            EditCommand::ResetTransform {
                clip_ids: vec![clip_id],
            },
            &ids,
        )
        .unwrap();
        assert!(!res.changed);
        assert_eq!(state.version(), version_before);
    }

    #[test]
    fn set_timeline_settings_changes_dims_and_bumps_version() {
        let mut state = EditorState::default();
        let ids = SeqIdGen::default();
        let version_before = state.version();
        let res = apply(
            &mut state,
            EditCommand::SetTimelineSettings {
                fps: 30,
                width: 1080,
                height: 1920,
            },
            &ids,
        )
        .unwrap();
        assert!(res.changed);
        assert_eq!((state.timeline.width, state.timeline.height), (1080, 1920));
        assert!(state.timeline.settings_configured);
        assert_eq!(state.version(), version_before + 1);
    }

    #[test]
    fn set_timeline_settings_is_undoable() {
        let mut state = EditorState::default();
        let ids = SeqIdGen::default();
        let (w0, h0, fps0) = (
            state.timeline.width,
            state.timeline.height,
            state.timeline.fps,
        );
        apply(
            &mut state,
            EditCommand::SetTimelineSettings {
                fps: 60,
                width: 2560,
                height: 1080,
            },
            &ids,
        )
        .unwrap();
        assert_eq!(state.timeline.width, 2560);

        let undo = apply(&mut state, EditCommand::Undo, &ids).unwrap();
        assert!(undo.changed);
        assert_eq!(
            (
                state.timeline.width,
                state.timeline.height,
                state.timeline.fps
            ),
            (w0, h0, fps0)
        );
    }

    #[test]
    fn set_timeline_settings_rejects_nonpositive() {
        let mut state = EditorState::default();
        let ids = SeqIdGen::default();
        assert!(apply(
            &mut state,
            EditCommand::SetTimelineSettings {
                fps: 0,
                width: 1920,
                height: 1080,
            },
            &ids,
        )
        .is_err());
    }

    #[test]
    fn set_timeline_settings_noop_when_identical_and_configured() {
        let mut state = EditorState::default();
        let ids = SeqIdGen::default();
        // First call configures 1920x1080@30.
        apply(
            &mut state,
            EditCommand::SetTimelineSettings {
                fps: 30,
                width: 1920,
                height: 1080,
            },
            &ids,
        )
        .unwrap();
        let version_before = state.version();
        // Re-applying the same, now-configured settings is a clean no-op.
        let res = apply(
            &mut state,
            EditCommand::SetTimelineSettings {
                fps: 30,
                width: 1920,
                height: 1080,
            },
            &ids,
        )
        .unwrap();
        assert!(!res.changed);
        assert_eq!(state.version(), version_before);
    }
}

#[cfg(test)]
mod add_captions_tests {
    use super::*;
    use crate::id::SeqIdGen;
    use opentake_domain::{Clip, ClipType, TextStyle, Track, Transform};

    fn state_with_video_and_audio() -> EditorState {
        let mut tl = Timeline::new();
        let mut v = Track::new("v1", ClipType::Video);
        v.clips.push(Clip::new("c1", "asset", 0, 300));
        tl.tracks.push(v);
        let mut a = Track::new("a1", ClipType::Audio);
        a.clips.push({
            let mut c = Clip::new("a-clip", "audio-asset", 0, 300);
            c.media_type = ClipType::Audio;
            c.source_clip_type = ClipType::Audio;
            c
        });
        tl.tracks.push(a);
        EditorState::from_timeline(tl)
    }

    fn caption(content: &str, start: i32, dur: i32, group: &str) -> CaptionEntry {
        CaptionEntry {
            start_frame: start,
            duration_frames: dur,
            content: content.into(),
            text_style: TextStyle::default(),
            transform: Transform::default(),
            caption_group_id: group.into(),
        }
    }

    #[test]
    fn add_captions_inserts_top_video_track_with_group_ids() {
        let mut state = state_with_video_and_audio();
        let ids = SeqIdGen::new("cap-");
        let res = apply(
            &mut state,
            EditCommand::AddCaptions {
                entries: vec![
                    caption("hello", 0, 21, "g1"),
                    caption("world", 21, 21, "g1"),
                ],
            },
            &ids,
        )
        .unwrap();
        assert!(res.changed);
        assert_eq!(res.action_name, "Generate Captions");
        assert_eq!(res.affected_clip_ids.len(), 2);
        // A new track was inserted at index 0 (above the pre-existing video track).
        assert_eq!(state.timeline.tracks.len(), 3);
        let cap_track = &state.timeline.tracks[0];
        assert_eq!(cap_track.kind, ClipType::Video);
        assert_eq!(cap_track.clips.len(), 2);
        // Every caption clip is a text clip carrying the caption group id + content.
        for clip in &cap_track.clips {
            assert_eq!(clip.media_type, ClipType::Text);
            assert_eq!(clip.caption_group_id.as_deref(), Some("g1"));
            assert!(clip.text_content.is_some());
            assert!(clip.text_style.is_some());
        }
        // The original tracks are pushed down, untouched.
        assert_eq!(state.timeline.tracks[1].id, "v1");
        assert_eq!(state.timeline.tracks[2].id, "a1");
    }

    #[test]
    fn add_captions_is_one_undo_step() {
        let mut state = state_with_video_and_audio();
        let ids = SeqIdGen::new("cap-");
        let tracks_before = state.timeline.tracks.len();
        apply(
            &mut state,
            EditCommand::AddCaptions {
                entries: vec![caption("a", 0, 30, "g")],
            },
            &ids,
        )
        .unwrap();
        assert_eq!(state.timeline.tracks.len(), tracks_before + 1);
        // A single Undo reverts the entire caption placement (track + all clips).
        let undo = apply(&mut state, EditCommand::Undo, &ids).unwrap();
        assert!(undo.changed);
        assert_eq!(state.timeline.tracks.len(), tracks_before);
    }

    #[test]
    fn add_captions_empty_is_noop() {
        let mut state = state_with_video_and_audio();
        let ids = SeqIdGen::new("cap-");
        let version_before = state.version();
        let res = apply(
            &mut state,
            EditCommand::AddCaptions { entries: vec![] },
            &ids,
        )
        .unwrap();
        assert!(!res.changed);
        assert_eq!(res.action_name, "Generate Captions");
        assert_eq!(state.version(), version_before);
        // No track was created.
        assert_eq!(state.timeline.tracks.len(), 2);
    }

    #[test]
    fn add_captions_rejects_bad_duration() {
        let mut state = state_with_video_and_audio();
        let ids = SeqIdGen::new("cap-");
        let err = apply(
            &mut state,
            EditCommand::AddCaptions {
                entries: vec![caption("x", 0, 0, "g")],
            },
            &ids,
        )
        .unwrap_err();
        assert!(matches!(err, EditError::Invalid(_)));
        // State untouched by the refusal.
        assert_eq!(state.timeline.tracks.len(), 2);
    }
}

/// Tests for [`EditCommand::AddTextsAutoTrack`] (#194): the all-omitted-
/// trackIndex path must always create a fresh top track rather than writing
/// into whatever the caller finds at track 0, so pre-existing content on an
/// existing top track is never clobbered.
#[cfg(test)]
mod add_texts_auto_track_tests {
    use super::*;
    use crate::id::SeqIdGen;
    use opentake_domain::{Clip, ClipType, TextStyle, Track, Transform};

    /// A pre-existing top video track already holding unrelated content,
    /// mirroring the exact scenario #194 reported: calling `add_texts` with
    /// every `trackIndex` omitted used to `clear_region` over this clip.
    fn state_with_existing_top_track() -> EditorState {
        let mut tl = Timeline::new();
        let mut v = Track::new("existing-video", ClipType::Video);
        v.clips.push(Clip::new("existing-clip", "asset", 0, 300));
        tl.tracks.push(v);
        EditorState::from_timeline(tl)
    }

    fn text(content: &str, start: i32, dur: i32) -> TextAutoTrackEntry {
        TextAutoTrackEntry {
            start_frame: start,
            duration_frames: dur,
            content: content.into(),
            text_style: TextStyle::default(),
            transform: Transform::default(),
        }
    }

    #[test]
    fn creates_new_top_track_and_leaves_existing_track_untouched() {
        let mut state = state_with_existing_top_track();
        let ids = SeqIdGen::new("txt-");
        let res = apply(
            &mut state,
            EditCommand::AddTextsAutoTrack {
                entries: vec![text("hello", 0, 30)],
            },
            &ids,
        )
        .unwrap();
        assert!(res.changed);
        assert_eq!(res.affected_clip_ids.len(), 1);

        // A new track was inserted at index 0 — the existing track is pushed
        // down, not written into.
        assert_eq!(state.timeline.tracks.len(), 2);
        let new_track = &state.timeline.tracks[0];
        assert_eq!(new_track.kind, ClipType::Video);
        assert_eq!(new_track.clips.len(), 1);
        assert_eq!(new_track.clips[0].media_type, ClipType::Text);
        assert_eq!(new_track.clips[0].text_content.as_deref(), Some("hello"));

        // The pre-existing track and its clip are completely unchanged — this
        // is the exact regression #194 reported (an omitted trackIndex used
        // to `clear_region` straight over track 0's content).
        let existing = &state.timeline.tracks[1];
        assert_eq!(existing.id, "existing-video");
        assert_eq!(existing.clips.len(), 1);
        assert_eq!(existing.clips[0].id, "existing-clip");
        assert_eq!(existing.clips[0].start_frame, 0);
        assert_eq!(existing.clips[0].duration_frames, 300);
    }

    #[test]
    fn is_one_undo_step_including_the_new_track() {
        let mut state = state_with_existing_top_track();
        let ids = SeqIdGen::new("txt-");
        let tracks_before = state.timeline.tracks.len();
        apply(
            &mut state,
            EditCommand::AddTextsAutoTrack {
                entries: vec![text("a", 0, 30), text("b", 30, 30)],
            },
            &ids,
        )
        .unwrap();
        assert_eq!(state.timeline.tracks.len(), tracks_before + 1);

        // A single Undo reverts the entire transaction: both text clips AND
        // the track they were created on.
        let undo = apply(&mut state, EditCommand::Undo, &ids).unwrap();
        assert!(undo.changed);
        assert_eq!(state.timeline.tracks.len(), tracks_before);
        assert_eq!(state.timeline.tracks[0].id, "existing-video");
        assert_eq!(state.timeline.tracks[0].clips.len(), 1);
    }

    #[test]
    fn empty_entries_is_an_error_not_a_noop() {
        // Unlike AddCaptions (whose empty-is-no-op reflects "no speech
        // detected"), an empty entries array here is a caller mistake — same
        // as the straight AddTexts path.
        let mut state = state_with_existing_top_track();
        let ids = SeqIdGen::new("txt-");
        let version_before = state.version();
        let err = apply(
            &mut state,
            EditCommand::AddTextsAutoTrack { entries: vec![] },
            &ids,
        )
        .unwrap_err();
        assert!(matches!(err, EditError::Invalid(_)));
        assert_eq!(state.version(), version_before);
        assert_eq!(state.timeline.tracks.len(), 1);
    }

    #[test]
    fn rejects_bad_duration_and_leaves_state_untouched() {
        let mut state = state_with_existing_top_track();
        let ids = SeqIdGen::new("txt-");
        let err = apply(
            &mut state,
            EditCommand::AddTextsAutoTrack {
                entries: vec![text("x", 0, 0)],
            },
            &ids,
        )
        .unwrap_err();
        assert!(matches!(err, EditError::Invalid(_)));
        assert_eq!(state.timeline.tracks.len(), 1);
    }

    #[test]
    fn rejects_negative_start_frame() {
        let mut state = state_with_existing_top_track();
        let ids = SeqIdGen::new("txt-");
        let err = apply(
            &mut state,
            EditCommand::AddTextsAutoTrack {
                entries: vec![text("x", -1, 30)],
            },
            &ids,
        )
        .unwrap_err();
        assert!(matches!(err, EditError::Invalid(_)));
        assert_eq!(state.timeline.tracks.len(), 1);
    }

    #[test]
    fn overlapping_entries_in_the_same_batch_overwrite_in_order() {
        // Same "later entry wins" overwrite semantics as the explicit-track
        // AddTexts path and upstream `placeTextClips`'s per-track clearRegion
        // pass — even though the destination track is brand new here.
        let mut state = state_with_existing_top_track();
        let ids = SeqIdGen::new("txt-");
        let res = apply(
            &mut state,
            EditCommand::AddTextsAutoTrack {
                entries: vec![text("first", 0, 60), text("second", 20, 60)],
            },
            &ids,
        )
        .unwrap();
        assert!(res.changed);

        let new_track = &state.timeline.tracks[0];
        // "first" (0..60) is trimmed/removed to make room for "second"
        // (20..80): only one clip's worth of "first" content can survive the
        // overlap, and "second" is placed in full.
        let second = new_track
            .clips
            .iter()
            .find(|c| c.text_content.as_deref() == Some("second"))
            .expect("second clip survives at its full requested range");
        assert_eq!(second.start_frame, 20);
        assert_eq!(second.duration_frames, 60);
        // "first" no longer occupies the region "second" claimed.
        for clip in &new_track.clips {
            if clip.text_content.as_deref() == Some("first") {
                assert!(clip.start_frame + clip.duration_frames <= 20);
            }
        }
    }
}

#[cfg(test)]
mod freeze_frame_tests {
    use super::*;
    use crate::id::SeqIdGen;
    use opentake_domain::{Clip, Track};

    fn state_with_video_clip() -> (EditorState, SeqIdGen) {
        let mut state = EditorState::default();
        let ids = SeqIdGen::default();
        let mut t = Track::new("v1", ClipType::Video);
        t.clips.push(Clip::new("c1", "asset1", 100, 60));
        state.timeline.tracks.push(t);
        (state, ids)
    }

    fn freeze(clip_id: &str, at_frame: i32, duration_frames: i32) -> EditCommand {
        EditCommand::FreezeFrame {
            clip_id: clip_id.to_string(),
            at_frame,
            duration_frames,
            media_ref: format!("freeze:{clip_id}:{at_frame}"),
        }
    }

    #[test]
    fn freeze_frame_splits_and_inserts_image_clip() {
        let (mut state, ids) = state_with_video_clip();
        let res = apply(&mut state, freeze("c1", 130, 30), &ids).unwrap();
        assert!(res.changed);
        assert_eq!(res.action_name, "Freeze Frame");
        let track = &state.timeline.tracks[0];
        assert_eq!(track.clips.len(), 3);
        assert_eq!(track.clips[0].id, "c1");
        assert_eq!(track.clips[0].start_frame, 100);
        assert_eq!(track.clips[0].end_frame(), 130);
        assert_eq!(track.clips[1].start_frame, 130);
        assert_eq!(track.clips[1].duration_frames, 30);
        assert_eq!(track.clips[1].media_type, ClipType::Image);
        assert_eq!(track.clips[1].media_ref, "freeze:c1:130");
        assert_eq!(track.clips[2].start_frame, 160);
        assert_eq!(track.clips[2].end_frame(), 190);
    }

    #[test]
    fn freeze_frame_preserves_real_media_ref() {
        let (mut state, ids) = state_with_video_clip();
        apply(
            &mut state,
            EditCommand::FreezeFrame {
                clip_id: "c1".to_string(),
                at_frame: 130,
                duration_frames: 15,
                media_ref: "asset-xyz".to_string(),
            },
            &ids,
        )
        .unwrap();
        assert_eq!(state.timeline.tracks[0].clips[1].media_ref, "asset-xyz");
    }

    #[test]
    fn freeze_frame_at_start_endpoint_rejected() {
        let (mut state, ids) = state_with_video_clip();
        assert!(apply(&mut state, freeze("c1", 100, 30), &ids).is_err());
        assert_eq!(state.timeline.tracks[0].clips.len(), 1);
    }

    #[test]
    fn freeze_frame_at_end_endpoint_rejected() {
        let (mut state, ids) = state_with_video_clip();
        assert!(apply(&mut state, freeze("c1", 160, 30), &ids).is_err());
        assert_eq!(state.timeline.tracks[0].clips.len(), 1);
    }

    #[test]
    fn freeze_frame_zero_duration_rejected() {
        let (mut state, ids) = state_with_video_clip();
        assert!(apply(&mut state, freeze("c1", 130, 0), &ids).is_err());
        assert_eq!(state.timeline.tracks[0].clips.len(), 1);
    }

    #[test]
    fn freeze_frame_audio_clip_rejected() {
        let mut state = EditorState::default();
        let ids = SeqIdGen::default();
        let mut t = Track::new("a1", ClipType::Audio);
        let mut c = Clip::new("ac", "asset", 100, 60);
        c.media_type = ClipType::Audio;
        c.source_clip_type = ClipType::Audio;
        t.clips.push(c);
        state.timeline.tracks.push(t);
        assert!(apply(&mut state, freeze("ac", 130, 30), &ids).is_err());
    }

    #[test]
    fn freeze_frame_text_clip_rejected() {
        let mut state = EditorState::default();
        let ids = SeqIdGen::default();
        let mut t = Track::new("t1", ClipType::Text);
        let mut c = Clip::new("tc", "asset", 100, 60);
        c.media_type = ClipType::Text;
        t.clips.push(c);
        state.timeline.tracks.push(t);
        assert!(apply(&mut state, freeze("tc", 130, 30), &ids).is_err());
    }

    #[test]
    fn freeze_frame_missing_clip_rejected() {
        let (mut state, ids) = state_with_video_clip();
        assert!(apply(&mut state, freeze("nope", 130, 30), &ids).is_err());
    }

    #[test]
    fn freeze_frame_undo_restores_original_in_one_step() {
        let (mut state, ids) = state_with_video_clip();
        let before = state.snapshot();
        apply(&mut state, freeze("c1", 130, 30), &ids).unwrap();
        assert_eq!(state.undo_depth(), 1);
        apply(&mut state, EditCommand::Undo, &ids).unwrap();
        assert_eq!(state.timeline, before.timeline);
    }

    #[test]
    fn freeze_frame_shifts_a_follower_on_same_track() {
        let (mut state, ids) = state_with_video_clip();
        state.timeline.tracks[0]
            .clips
            .push(Clip::new("c2", "asset2", 160, 60));
        apply(&mut state, freeze("c1", 130, 30), &ids).unwrap();
        let c2 = state.timeline.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == "c2")
            .unwrap();
        assert_eq!(c2.start_frame, 190);
        assert_eq!(c2.end_frame(), 250);
    }
}
