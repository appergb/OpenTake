//! High-level editing intents normalized into existing [`EditCommand`] values.
//!
//! This layer is deliberately thin: it performs preflight validation and expands
//! convenience intents (for example "add clips, creating a compatible track if
//! needed") into commands. It never mutates [`EditorState`] and never bypasses
//! [`crate::command::apply`].

use opentake_domain::{Clip, ClipType, Crop, Timeline, Transform};

use crate::command::{ClipEntry, ClipProperties, EditCommand, EditError};
use crate::engines::FrameRange;
use crate::ops::{self, TrimEdge};

/// Preflight output for a high-level edit intent.
#[derive(Clone, Debug)]
pub struct EditPlan {
    pub label: String,
    pub commands: Vec<EditCommand>,
    pub warnings: Vec<String>,
}

impl EditPlan {
    fn new(label: impl Into<String>, commands: Vec<EditCommand>) -> Self {
        EditPlan {
            label: label.into(),
            commands,
            warnings: Vec::new(),
        }
    }
}

/// Clip placement intent. `track_index = None` means "pick or create a shared
/// compatible track" during preflight.
#[derive(Clone, Debug)]
pub struct IntentClipEntry {
    pub media_ref: String,
    pub media_type: ClipType,
    pub source_clip_type: ClipType,
    pub track_index: Option<usize>,
    pub start_frame: i32,
    pub duration_frames: i32,
    pub trim_start_frame: Option<i32>,
    pub trim_end_frame: Option<i32>,
    pub has_audio: bool,
    pub add_linked_audio: bool,
    pub transform: Option<Transform>,
}

impl IntentClipEntry {
    fn into_entry(self, track_index: usize) -> ClipEntry {
        ClipEntry {
            media_ref: self.media_ref,
            media_type: self.media_type,
            source_clip_type: self.source_clip_type,
            track_index,
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

/// Add clips to explicitly provided tracks, or pick/create shared compatible
/// tracks when every entry omits `track_index`.
pub fn plan_auto_track_add(
    timeline: &Timeline,
    entries: Vec<IntentClipEntry>,
) -> Result<EditPlan, EditError> {
    if entries.is_empty() {
        return Err(EditError::Invalid(
            "Missing or empty intent entries".to_string(),
        ));
    }

    let provided = entries.iter().filter(|e| e.track_index.is_some()).count();
    if provided != 0 && provided != entries.len() {
        return Err(EditError::Invalid(
            "Either provide trackIndex for every entry or omit it for every entry".to_string(),
        ));
    }

    for (i, entry) in entries.iter().enumerate() {
        validate_intent_entry(timeline, entry, i)?;
    }

    if provided == entries.len() {
        let add_entries = entries
            .into_iter()
            .map(|entry| {
                let track_index = entry.track_index.expect("validated above");
                entry.into_entry(track_index)
            })
            .collect();
        return Ok(EditPlan::new(
            "auto_track_add",
            vec![EditCommand::AddClips {
                entries: add_entries,
            }],
        ));
    }

    Ok(EditPlan::new(
        "auto_track_add",
        vec![EditCommand::AddClipsAutoTrack {
            entries: entries
                .into_iter()
                .map(|entry| entry.into_entry(0))
                .collect(),
        }],
    ))
}

/// Plan a CapCut-style trim to playhead for specific clips.
pub fn plan_trim_to_playhead(
    timeline: &Timeline,
    clip_ids: &[String],
    frame: i32,
    edge: TrimEdge,
) -> Result<EditPlan, EditError> {
    if clip_ids.is_empty() {
        return Err(EditError::Invalid(
            "Missing or empty clipIds array".to_string(),
        ));
    }

    let mut edits = Vec::new();
    for id in clip_ids {
        let clip = find_clip(timeline, id)
            .ok_or_else(|| EditError::Invalid(format!("Clip not found: {id}")))?;
        if frame <= clip.start_frame || frame >= clip.end_frame() {
            continue;
        }
        let raw_delta = match edge {
            TrimEdge::Left => frame - clip.start_frame,
            TrimEdge::Right => frame - clip.end_frame(),
        };
        let delta = clamp_trim_delta(clip, edge, raw_delta);
        if delta == 0 {
            continue;
        }
        let speed = if clip.speed > 0.0 { clip.speed } else { 1.0 };
        let (trim_start, trim_end) = ops::trim_values(
            clip.media_type,
            speed,
            clip.trim_start_frame,
            clip.trim_end_frame,
            edge,
            delta,
        );
        edits.push((id.clone(), trim_start, trim_end));
    }

    if edits.is_empty() {
        let mut plan = EditPlan::new("trim_to_playhead", Vec::new());
        plan.warnings
            .push("No clips intersect the playhead frame".to_string());
        return Ok(plan);
    }

    Ok(EditPlan::new(
        "trim_to_playhead",
        vec![EditCommand::TrimClips { edits }],
    ))
}

/// Plan a single half-open project-frame ripple range delete on one track.
pub fn plan_ripple_delete_range(
    track_index: usize,
    start_frame: i32,
    end_frame: i32,
) -> Result<EditPlan, EditError> {
    if end_frame <= start_frame {
        return Err(EditError::Invalid(format!(
            "range end must be greater than start ({start_frame}..{end_frame})"
        )));
    }
    Ok(EditPlan::new(
        "ripple_delete_range",
        vec![EditCommand::RippleDeleteRanges {
            track_index,
            ranges: vec![FrameRange::new(start_frame, end_frame)],
        }],
    ))
}

/// Place clips at beat frames, then use the same auto-track planning as manual
/// placement.
pub fn plan_beat_sync_placement(
    timeline: &Timeline,
    entries: Vec<IntentClipEntry>,
    beat_frames: &[i32],
) -> Result<EditPlan, EditError> {
    if beat_frames.len() < entries.len() {
        return Err(EditError::Invalid(format!(
            "Need at least {} beat frame(s), got {}",
            entries.len(),
            beat_frames.len()
        )));
    }
    let beat_entries = entries
        .into_iter()
        .zip(beat_frames.iter().copied())
        .map(|(mut entry, beat)| {
            entry.start_frame = beat;
            entry
        })
        .collect();
    let mut plan = plan_auto_track_add(timeline, beat_entries)?;
    plan.label = "beat_sync_placement".to_string();
    Ok(plan)
}

/// Apply a smart-reframe crop/transform proposal to clips through
/// `SetClipProperties`.
pub fn plan_smart_reframe(
    clip_ids: &[String],
    crop: Crop,
    transform: Option<Transform>,
) -> Result<EditPlan, EditError> {
    if clip_ids.is_empty() {
        return Err(EditError::Invalid(
            "Missing or empty clipIds array".to_string(),
        ));
    }
    Ok(EditPlan::new(
        "smart_reframe",
        vec![EditCommand::SetClipProperties {
            clip_ids: clip_ids.to_vec(),
            properties: Box::new(ClipProperties {
                crop: Some(crop),
                transform,
                ..Default::default()
            }),
        }],
    ))
}

fn validate_intent_entry(
    timeline: &Timeline,
    entry: &IntentClipEntry,
    index: usize,
) -> Result<(), EditError> {
    if entry.duration_frames < 1 {
        return Err(EditError::Invalid(format!(
            "entries[{index}]: durationFrames must be >= 1 (got {})",
            entry.duration_frames
        )));
    }
    if entry.start_frame < 0 {
        return Err(EditError::Invalid(format!(
            "entries[{index}]: startFrame must be >= 0 (got {})",
            entry.start_frame
        )));
    }
    if let Some(trim) = entry.trim_start_frame {
        if trim < 0 {
            return Err(EditError::Invalid(format!(
                "entries[{index}]: trimStartFrame must be >= 0 (got {trim})"
            )));
        }
    }
    if let Some(trim) = entry.trim_end_frame {
        if trim < 0 {
            return Err(EditError::Invalid(format!(
                "entries[{index}]: trimEndFrame must be >= 0 (got {trim})"
            )));
        }
    }
    if let Some(track_index) = entry.track_index {
        let Some(track) = timeline.tracks.get(track_index) else {
            return Err(EditError::Invalid(format!(
                "entries[{index}]: track index {track_index} out of range"
            )));
        };
        if !entry.source_clip_type.is_compatible(track.kind) {
            return Err(EditError::Invalid(format!(
                "entries[{index}]: asset type is not compatible with the destination track"
            )));
        }
    }
    Ok(())
}

fn find_clip<'a>(timeline: &'a Timeline, clip_id: &str) -> Option<&'a Clip> {
    timeline
        .tracks
        .iter()
        .flat_map(|track| track.clips.iter())
        .find(|clip| clip.id == clip_id)
}

fn clamp_trim_delta(clip: &Clip, edge: TrimEdge, raw_delta: i32) -> i32 {
    match edge {
        TrimEdge::Left => raw_delta.clamp(0, clip.duration_frames - 1),
        TrimEdge::Right => raw_delta.clamp(-(clip.duration_frames - 1), 0),
    }
}
