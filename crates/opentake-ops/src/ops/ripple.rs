//! Ripple editing: delete-and-close, range delete, and insert-and-push, plus the
//! sync-lock machinery that keeps other tracks aligned. 1:1 port of the mutating
//! ripple methods in `EditorViewModel+Ripple.swift`. The pure math lives in
//! [`crate::engines::RippleEngine`].
//!
//! Refusal semantics are preserved: a sync-locked follower that can't absorb a
//! shift (a clip would move past frame 0, or two clips would collide) aborts the
//! whole edit without mutating anything — upstream beeps + logs; here the call
//! returns `Err(reason)` / `RippleOutcome::Refused`.

use std::collections::{HashMap, HashSet};

use opentake_domain::{Clip, ClipType, Timeline};

use crate::engines::{ClipShift, FrameRange, RippleEngine};
use crate::id::{IdGen, SeqIdGen};
use crate::ops::clear_region::{clear_region, remove_clip};
use crate::ops::linking::linked_partner_ids;
use crate::ops::place::{place_clip, sort_clips, PlaceSpec};
use crate::ops::split::split_clip;
use crate::ops::tracks::prune_empty_tracks;

/// Apply each shift's new `start_frame` to its clip. Returns the count applied.
/// 1:1 port of `applyShifts`.
pub fn apply_shifts(timeline: &mut Timeline, shifts: &[ClipShift]) -> usize {
    if validate_timeline_arithmetic(timeline).is_err()
        || shifts.iter().any(|shift| {
            let Some((track_index, clip_index)) = find(timeline, &shift.clip_id) else {
                return false;
            };
            let clip = &timeline.tracks[track_index].clips[clip_index];
            shift.new_start_frame < 0
                || shift
                    .new_start_frame
                    .checked_add(clip.duration_frames)
                    .is_none()
        })
    {
        return 0;
    }
    let mut applied = 0;
    for shift in shifts {
        if let Some((ti, ci)) = find(timeline, &shift.clip_id) {
            timeline.tracks[ti].clips[ci].start_frame = shift.new_start_frame;
            applied += 1;
        }
    }
    applied
}

/// Dry-run a shift set against a track: returns a blocking reason (collision or
/// negative start) or `None` if safe. 1:1 port of `validateShifts`, using a track
/// index into the live timeline. `label` is the user-facing track name.
pub fn validate_shifts(
    timeline: &Timeline,
    track_index: usize,
    shifts: &[ClipShift],
    label: &str,
) -> Option<String> {
    if shifts.is_empty() || track_index >= timeline.tracks.len() {
        return None;
    }
    let track = &timeline.tracks[track_index];
    let shift_map: HashMap<&str, i32> = shifts
        .iter()
        .map(|s| (s.clip_id.as_str(), s.new_start_frame))
        .collect();

    let mut intervals: Vec<FrameRange> = Vec::with_capacity(track.clips.len());
    for clip in &track.clips {
        let start = shift_map
            .get(clip.id.as_str())
            .copied()
            .unwrap_or(clip.start_frame);
        if start < 0 {
            return Some(format!(
                "Sync-locked track \"{label}\" would move past the timeline start."
            ));
        }
        let Some(end) = start.checked_add(clip.duration_frames) else {
            return Some(format!(
                "Sync-locked track \"{label}\" would overflow the timeline."
            ));
        };
        intervals.push(FrameRange::new(start, end));
    }
    intervals.sort_by_key(|r| r.start);
    for i in 1..intervals.len() {
        if intervals[i].start < intervals[i - 1].end {
            return Some(format!(
                "Sync-locked track \"{label}\" doesn't have room to ripple."
            ));
        }
    }
    None
}

/// Outcome of a ripple-delete-ranges edit.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RippleOutcome {
    Ok(RippleRangesReport),
    Refused(String),
}

/// Report returned by [`ripple_delete_ranges_on_track`] so callers needn't
/// re-read the timeline. 1:1 port of `RippleRangesReport`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RippleRangesReport {
    pub removed_frames: i32,
    pub cleared_tracks: usize,
    pub shifted_clips: usize,
    pub anchor_track_index: usize,
    /// `(clip_id, start_frame, duration_frames)` for the anchor track after the cut.
    pub resulting_fragments: Vec<(String, i32, i32)>,
    pub removed_clip_ids: Vec<String>,
}

/// Ripple-delete a set of clips and close the gaps. Sync-locked tracks shift
/// along to preserve cross-track alignment; refuses (returns `Err`, no mutation)
/// if any follower would collide. 1:1 port of `rippleDeleteSelectedClips`.
///
/// `track_label` maps a track index to its display label for refusal messages.
pub fn ripple_delete(
    timeline: &mut Timeline,
    ids: &HashSet<String>,
    track_label: &dyn Fn(&Timeline, usize) -> String,
) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    validate_timeline_arithmetic(timeline)?;

    // Merged ranges used to shift sync-locked tracks with no deletions of their own.
    let global_removed: Vec<FrameRange> = timeline
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .filter(|c| ids.contains(&c.id))
        .map(|clip| {
            clip.start_frame
                .checked_add(clip.duration_frames)
                .map(|end| FrameRange::new(clip.start_frame, end))
                .ok_or_else(|| format!("clip {} endFrame overflows", clip.id))
        })
        .collect::<Result<_, _>>()?;

    // Compute every track's shifts up front; refuse before mutating anything.
    let mut shifts_by_track: HashMap<usize, Vec<ClipShift>> = HashMap::new();
    for ti in 0..timeline.tracks.len() {
        let track = &timeline.tracks[ti];
        let has_own = track.clips.iter().any(|c| ids.contains(&c.id));
        if has_own {
            let shifts = RippleEngine::try_compute_ripple_shifts(&track.clips, ids)
                .ok_or_else(|| "ripple-delete shift arithmetic overflows".to_string())?;
            shifts_by_track.insert(ti, shifts);
        } else if track.sync_locked {
            let s =
                RippleEngine::try_compute_ripple_shifts_for_ranges(&track.clips, &global_removed)
                    .ok_or_else(|| "sync-locked ripple shift arithmetic overflows".to_string())?;
            if let Some(reason) = validate_shifts(timeline, ti, &s, &track_label(timeline, ti)) {
                return Err(reason);
            }
            shifts_by_track.insert(ti, s);
        }
    }

    // Remove, then apply the precomputed shifts.
    for id in ids {
        remove_clip(timeline, id);
    }
    for shifts in shifts_by_track.values() {
        apply_shifts(timeline, shifts);
    }
    prune_empty_tracks(timeline);
    Ok(())
}

/// Ripple-delete project-frame `ranges` from one track (spanning any clips),
/// closing the gaps. Cuts linked A/V partners, shifts sync-locked followers, and
/// refuses if any follower can't absorb the shift. 1:1 port of
/// `rippleDeleteRangesOnTrack`.
pub fn ripple_delete_ranges_on_track(
    timeline: &mut Timeline,
    track_index: usize,
    ranges: &[FrameRange],
    track_label: &dyn Fn(&Timeline, usize) -> String,
    id_gen: &dyn IdGen,
) -> RippleOutcome {
    if let Err(reason) = validate_ripple_delete_ranges(timeline, track_index, ranges) {
        return RippleOutcome::Refused(reason);
    }
    // Exercise the complete clear/split/shift path on a disposable timeline
    // with a private id stream. This establishes that the real mutation cannot
    // encounter a late arithmetic refusal after consuming caller ids.
    let mut preflight = timeline.clone();
    let mut preflight_prefix = "__ripple-preflight-".to_string();
    while timeline
        .tracks
        .iter()
        .flat_map(|track| &track.clips)
        .any(|clip| clip.id.starts_with(&preflight_prefix))
    {
        preflight_prefix.push('_');
    }
    let preflight_ids = SeqIdGen::new(preflight_prefix);
    if let RippleOutcome::Refused(reason) = ripple_delete_ranges_validated(
        &mut preflight,
        track_index,
        ranges,
        track_label,
        &preflight_ids,
    ) {
        return RippleOutcome::Refused(reason);
    }

    let mut candidate = timeline.clone();
    let outcome =
        ripple_delete_ranges_validated(&mut candidate, track_index, ranges, track_label, id_gen);
    if matches!(outcome, RippleOutcome::Ok(_)) {
        *timeline = candidate;
    }
    outcome
}

fn ripple_delete_ranges_validated(
    timeline: &mut Timeline,
    track_index: usize,
    ranges: &[FrameRange],
    track_label: &dyn Fn(&Timeline, usize) -> String,
    id_gen: &dyn IdGen,
) -> RippleOutcome {
    let nonempty: Vec<FrameRange> = ranges
        .iter()
        .copied()
        .filter(|range| range.checked_length().is_some_and(|length| length > 0))
        .collect();
    let merged = RippleEngine::merge_ranges(&nonempty);
    if merged.is_empty() {
        return RippleOutcome::Refused("No non-empty ranges to delete".into());
    }
    let total_removed = merged
        .iter()
        .try_fold(0_i32, |total, range| {
            total.checked_add(range.checked_length()?)
        })
        .expect("merged range arithmetic was prevalidated");

    let anchor_track_id = timeline.tracks[track_index].id.clone();
    let mut clear_track_ids: HashSet<String> = [anchor_track_id.clone()].into_iter().collect();

    // Linked partners of every touched clip, so A/V stays in sync across multi-clip ranges.
    let touched_partner_ids: Vec<String> = timeline.tracks[track_index]
        .clips
        .iter()
        .filter(|c| {
            c.link_group_id.is_some()
                && merged
                    .iter()
                    .any(|r| r.start < c.end_frame() && r.end > c.start_frame)
        })
        .map(|c| c.id.clone())
        .collect();
    for cid in touched_partner_ids {
        for pid in linked_partner_ids(timeline, &cid) {
            if let Some((ti, _)) = find(timeline, &pid) {
                clear_track_ids.insert(timeline.tracks[ti].id.clone());
            }
        }
    }

    // Refuse up front if a sync-locked follower can't absorb the shift. These
    // tracks aren't cleared, so their clips are unchanged when the shift applies.
    for ti in 0..timeline.tracks.len() {
        let track = &timeline.tracks[ti];
        if clear_track_ids.contains(&track.id) || !track.sync_locked {
            continue;
        }
        let s = RippleEngine::try_compute_ripple_shifts_for_ranges(&track.clips, &merged)
            .expect("ripple shift arithmetic was prevalidated");
        if let Some(reason) = validate_shifts(timeline, ti, &s, &track_label(timeline, ti)) {
            return RippleOutcome::Refused(reason);
        }
    }

    let anchor_before_ids: HashSet<String> = timeline.tracks[track_index]
        .clips
        .iter()
        .map(|c| c.id.clone())
        .collect();

    // Clear each touched track over each range.
    let clear_ids_snapshot: Vec<String> = clear_track_ids.iter().cloned().collect();
    for tid in &clear_ids_snapshot {
        if let Some(ti) = timeline.tracks.iter().position(|t| &t.id == tid) {
            for r in &merged {
                clear_region(timeline, ti, r.start, r.end, false, id_gen);
            }
        }
    }

    // Shift the cleared tracks ∪ sync-locked followers left to close the gaps.
    let mut shifted_clips = 0;
    for ti in 0..timeline.tracks.len() {
        let track = &timeline.tracks[ti];
        if !(clear_track_ids.contains(&track.id) || track.sync_locked) {
            continue;
        }
        let Some(s) = RippleEngine::try_compute_ripple_shifts_for_ranges(&track.clips, &merged)
        else {
            return RippleOutcome::Refused("post-clear ripple shift arithmetic overflows".into());
        };
        shifted_clips += apply_shifts(timeline, &s);
        sort_clips(&mut timeline.tracks[ti]);
    }

    // Anchor track's post-cut layout.
    let anchor_ti = timeline
        .tracks
        .iter()
        .position(|t| t.id == anchor_track_id)
        .unwrap_or(track_index);
    let after_clips = &timeline.tracks[anchor_ti].clips;
    let after_ids: HashSet<String> = after_clips.iter().map(|c| c.id.clone()).collect();
    let mut fragments: Vec<(String, i32, i32)> = after_clips
        .iter()
        .map(|c| (c.id.clone(), c.start_frame, c.duration_frames))
        .collect();
    fragments.sort_by_key(|f| f.1);
    let removed_clip_ids: Vec<String> = anchor_before_ids.difference(&after_ids).cloned().collect();

    if let Err(reason) = validate_timeline_arithmetic(timeline) {
        return RippleOutcome::Refused(format!(
            "ripple-delete produced invalid frame arithmetic: {reason}"
        ));
    }

    RippleOutcome::Ok(RippleRangesReport {
        removed_frames: total_removed,
        cleared_tracks: clear_track_ids.len(),
        shifted_clips,
        anchor_track_index: anchor_ti,
        resulting_fragments: fragments,
        removed_clip_ids,
    })
}

/// Validate ripple-delete range inputs and every pre-mutation shift. This is
/// shared with the command layer so malformed IPC values are rejected before a
/// snapshot, id allocation, or track mutation.
pub fn validate_ripple_delete_ranges(
    timeline: &Timeline,
    track_index: usize,
    ranges: &[FrameRange],
) -> Result<(), String> {
    if track_index >= timeline.tracks.len() {
        return Err(format!("Track index out of range: {track_index}"));
    }
    if ranges.is_empty() {
        return Err("Missing or empty ranges".into());
    }
    validate_timeline_arithmetic(timeline)?;
    for (index, range) in ranges.iter().enumerate() {
        if range.start < 0 || range.end <= range.start || range.checked_length().is_none() {
            return Err(format!(
                "ranges[{index}] must satisfy 0 <= start < end without overflow"
            ));
        }
    }
    let merged = RippleEngine::merge_ranges(ranges);
    merged
        .iter()
        .try_fold(0_i32, |total, range| {
            total.checked_add(range.checked_length()?)
        })
        .ok_or_else(|| "ripple-delete range total overflows".to_string())?;
    for track in &timeline.tracks {
        RippleEngine::try_compute_ripple_shifts_for_ranges(&track.clips, &merged)
            .ok_or_else(|| "ripple-delete shift arithmetic overflows".to_string())?;
    }
    Ok(())
}

fn validate_timeline_arithmetic(timeline: &Timeline) -> Result<(), String> {
    for track in &timeline.tracks {
        for clip in &track.clips {
            validate_clip_arithmetic(clip)?;
        }
    }
    Ok(())
}

/// Ripple-insert clips at `at_frame`, pushing everything past it right by the
/// total inserted duration on the target track, every sync-locked track, and the
/// audio track any linked audio lands on. Straddling clips on pushed tracks are
/// split at `at_frame` first. Returns the created clip ids. 1:1 port of
/// `rippleInsertClips(specs:trackIndex:atFrame:)`.
pub fn ripple_insert(
    timeline: &mut Timeline,
    specs: &[PlaceSpec],
    track_index: usize,
    at_frame: i32,
    ids: &dyn IdGen,
) -> Vec<String> {
    let Ok(total_push) = validate_ripple_insert(timeline, specs, track_index, at_frame) else {
        return Vec::new();
    };

    // Pin the linked-audio destination before pushing so it ripples too.
    let target_is_video = timeline.tracks[track_index].kind == opentake_domain::ClipType::Video;
    let needs_linked_audio = target_is_video
        && specs
            .iter()
            .any(|s| s.source_clip_type == opentake_domain::ClipType::Video && s.has_audio);
    let linked_audio_track_index: Option<usize> = if needs_linked_audio {
        match timeline
            .tracks
            .iter()
            .position(|t| t.kind == opentake_domain::ClipType::Audio)
        {
            Some(i) => Some(i),
            None => Some(crate::ops::tracks::insert_track(
                timeline,
                timeline.tracks.len(),
                opentake_domain::ClipType::Audio,
                ids,
            )),
        }
    } else {
        None
    };

    // Tracks the gap opens on. Splitting below doesn't add tracks, so these stay valid.
    let push_tracks: Vec<usize> = (0..timeline.tracks.len())
        .filter(|&i| {
            i == track_index
                || Some(i) == linked_audio_track_index
                || timeline.tracks[i].sync_locked
        })
        .collect();

    // Insert-edit: split any clip straddling at_frame on each pushed track so its
    // right half rides the ripple instead of being overlapped.
    for &ti in &push_tracks {
        if let Some(straddler) = timeline.tracks[ti]
            .clips
            .iter()
            .find(|c| c.start_frame < at_frame && at_frame < c.end_frame())
            .map(|c| c.id.clone())
        {
            split_clip(timeline, &straddler, at_frame, ids);
        }
    }

    // Push everything at/after at_frame.
    for &ti in &push_tracks {
        let shifts = RippleEngine::compute_ripple_push(
            &timeline.tracks[ti].clips,
            at_frame,
            total_push,
            &HashSet::new(),
        );
        apply_shifts(timeline, &shifts);
    }

    // Place the clips sequentially into the freed gap.
    let mut created = Vec::new();
    let mut cursor = at_frame;
    for spec in specs {
        let mut s = spec.clone();
        s.start_frame = cursor;
        created.extend(place_clip(
            timeline,
            &s,
            track_index,
            linked_audio_track_index,
            ids,
        ));
        cursor = cursor
            .checked_add(spec.duration_frames)
            .expect("ripple insertion cursor was prevalidated");
    }
    created
}

/// Validate every frame value that [`ripple_insert`] derives before it can
/// split a clip, create a track, or consume an id. The returned value is the
/// checked aggregate duration used to shift follower clips.
pub fn validate_ripple_insert(
    timeline: &Timeline,
    specs: &[PlaceSpec],
    track_index: usize,
    at_frame: i32,
) -> Result<i32, String> {
    if track_index >= timeline.tracks.len() {
        return Err(format!("Track index out of range: {track_index}"));
    }
    if specs.is_empty() {
        return Err("Missing or empty insertion specs".into());
    }
    if at_frame < 0 {
        return Err(format!("atFrame must be >= 0 (got {at_frame})"));
    }
    for track in &timeline.tracks {
        for clip in &track.clips {
            validate_clip_arithmetic(clip)?;
        }
    }

    let mut cursor = at_frame;
    for (index, spec) in specs.iter().enumerate() {
        validate_spec_arithmetic(spec, cursor)
            .map_err(|reason| format!("entries[{index}]: {reason}"))?;
        cursor = cursor
            .checked_add(spec.duration_frames)
            .ok_or_else(|| "inserted clip durations overflow the timeline".to_string())?;
    }
    let total_push = cursor
        .checked_sub(at_frame)
        .ok_or_else(|| "inserted clip duration total overflows".to_string())?;

    let target_is_video = timeline.tracks[track_index].kind == ClipType::Video;
    let needs_linked_audio = target_is_video
        && specs
            .iter()
            .any(|spec| spec.source_clip_type == ClipType::Video && spec.has_audio);
    let linked_audio_track_index = needs_linked_audio.then(|| {
        timeline
            .tracks
            .iter()
            .position(|track| track.kind == ClipType::Audio)
    });
    let pushed_tracks: Vec<usize> = (0..timeline.tracks.len())
        .filter(|&index| {
            index == track_index
                || linked_audio_track_index.flatten() == Some(index)
                || timeline.tracks[index].sync_locked
        })
        .collect();

    let split_groups: HashSet<String> = pushed_tracks
        .iter()
        .flat_map(|&index| &timeline.tracks[index].clips)
        .filter(|clip| clip.start_frame < at_frame && at_frame < checked_clip_end(clip))
        .filter_map(|clip| clip.link_group_id.clone())
        .collect();
    for clip in timeline.tracks.iter().flat_map(|track| &track.clips) {
        let is_direct_straddler = pushed_tracks.iter().any(|&index| {
            timeline.tracks[index]
                .clips
                .iter()
                .any(|candidate| candidate.id == clip.id)
        }) && clip.start_frame < at_frame
            && at_frame < checked_clip_end(clip);
        let is_linked_straddler = clip
            .link_group_id
            .as_ref()
            .is_some_and(|group| split_groups.contains(group))
            && clip.start_frame < at_frame
            && at_frame < checked_clip_end(clip);
        if is_direct_straddler || is_linked_straddler {
            let (left, mut right) = opentake_domain::split_clip(clip, at_frame, "preflight")
                .ok_or_else(|| format!("clip {} cannot be split at frame {at_frame}", clip.id))?;
            validate_clip_arithmetic(&left)?;
            validate_clip_arithmetic(&right)?;
            if is_direct_straddler {
                right.start_frame = right.start_frame.checked_add(total_push).ok_or_else(|| {
                    format!("ripple shift overflows split clip {} startFrame", clip.id)
                })?;
                validate_clip_arithmetic(&right)?;
            }
        }
    }

    for &index in &pushed_tracks {
        for clip in &timeline.tracks[index].clips {
            if clip.start_frame >= at_frame {
                let shifted_start = clip
                    .start_frame
                    .checked_add(total_push)
                    .ok_or_else(|| format!("ripple shift overflows clip {} startFrame", clip.id))?;
                shifted_start
                    .checked_add(clip.duration_frames)
                    .ok_or_else(|| format!("ripple shift overflows clip {} endFrame", clip.id))?;
            }
        }
    }
    Ok(total_push)
}

fn checked_clip_end(clip: &Clip) -> i32 {
    clip.start_frame
        .checked_add(clip.duration_frames)
        .expect("clip arithmetic was validated")
}

fn validate_spec_arithmetic(spec: &PlaceSpec, start_frame: i32) -> Result<(), String> {
    let trim_start = spec.trim_start_frame.unwrap_or(0);
    let trim_end = spec.trim_end_frame.unwrap_or(0);
    if start_frame < 0 || spec.duration_frames < 1 {
        return Err("startFrame must be >= 0 and durationFrames >= 1".into());
    }
    if !matches!(spec.media_type, ClipType::Image | ClipType::Text)
        && (trim_start < 0 || trim_end < 0)
    {
        return Err("trim frames must be >= 0 for audio/video clips".into());
    }
    start_frame
        .checked_add(spec.duration_frames)
        .ok_or_else(|| "startFrame + durationFrames overflows".to_string())?;
    spec.duration_frames
        .checked_add(trim_start)
        .and_then(|value| value.checked_add(trim_end))
        .ok_or_else(|| "durationFrames + trim frames overflows".to_string())?;
    trim_start
        .checked_add(spec.duration_frames)
        .and_then(|value| value.checked_add(trim_end))
        .ok_or_else(|| "source-frame extent overflows".to_string())?;
    trim_end
        .checked_add(spec.duration_frames)
        .ok_or_else(|| "trimEnd source-frame extent overflows".to_string())?;
    Ok(())
}

fn validate_clip_arithmetic(clip: &Clip) -> Result<(), String> {
    if !clip.speed.is_finite() || clip.speed <= 0.0 {
        return Err(format!("clip {} speed must be finite and > 0", clip.id));
    }
    let spec = PlaceSpec {
        media_ref: clip.media_ref.clone(),
        media_type: clip.media_type,
        source_clip_type: clip.source_clip_type,
        start_frame: clip.start_frame,
        duration_frames: clip.duration_frames,
        trim_start_frame: Some(clip.trim_start_frame),
        trim_end_frame: Some(clip.trim_end_frame),
        has_audio: false,
        add_linked_audio: false,
        transform: None,
    };
    validate_spec_arithmetic(&spec, clip.start_frame)?;
    let consumed = (clip.duration_frames as f64 * clip.speed).round();
    if !(0.0..=i32::MAX as f64).contains(&consumed) {
        return Err(format!(
            "clip {} source-frame extent is out of range",
            clip.id
        ));
    }
    let consumed = consumed as i32;
    clip.trim_start_frame
        .checked_add(consumed)
        .ok_or_else(|| format!("clip {} trimStart source-frame extent overflows", clip.id))?;
    clip.trim_end_frame
        .checked_add(consumed)
        .ok_or_else(|| format!("clip {} trimEnd source-frame extent overflows", clip.id))?;
    clip.trim_start_frame
        .checked_add(consumed)
        .and_then(|value| value.checked_add(clip.trim_end_frame))
        .ok_or_else(|| format!("clip {} source-frame extent overflows", clip.id))?;
    Ok(())
}

fn find(timeline: &Timeline, clip_id: &str) -> Option<(usize, usize)> {
    for (ti, t) in timeline.tracks.iter().enumerate() {
        if let Some(ci) = t.clips.iter().position(|c| c.id == clip_id) {
            return Some((ti, ci));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::SeqIdGen;
    use opentake_domain::{Clip, ClipType, Track};

    fn label(_tl: &Timeline, i: usize) -> String {
        format!("T{i}")
    }

    fn clip(id: &str, start: i32, dur: i32) -> Clip {
        Clip::new(id, "asset", start, dur)
    }

    fn one_track(clips: Vec<Clip>, sync: bool) -> Track {
        let mut t = Track::new("t", ClipType::Video);
        t.clips = clips;
        t.sync_locked = sync;
        t
    }

    #[test]
    fn ripple_delete_closes_gap_on_same_track() {
        let mut tl = Timeline::new();
        tl.tracks.push(one_track(
            vec![clip("a", 0, 30), clip("b", 30, 30), clip("c", 60, 30)],
            true,
        ));
        let ids: HashSet<String> = ["b".to_string()].into_iter().collect();
        ripple_delete(&mut tl, &ids, &label).unwrap();
        // b removed; c shifts left 30 -> [30,60). a stays.
        let spans: Vec<(i32, i32)> = tl.tracks[0]
            .clips
            .iter()
            .map(|c| (c.start_frame, c.end_frame()))
            .collect();
        assert_eq!(spans, vec![(0, 30), (30, 60)]);
    }

    #[test]
    fn ripple_delete_shifts_sync_locked_follower() {
        let mut tl = Timeline::new();
        // track0 (video) has the deletion; track1 (audio, sync-locked) follows.
        tl.tracks
            .push(one_track(vec![clip("a", 0, 30), clip("b", 30, 30)], true));
        let mut a = Track::new("audio", ClipType::Audio);
        a.sync_locked = true;
        a.clips.push(clip("x", 60, 30)); // sits after the deleted range
        tl.tracks.push(a);

        ripple_delete(&mut tl, &["a".to_string()].into_iter().collect(), &label).unwrap();
        // a removed -> b shifts to 0; follower x (60) shifts left by 30 -> 30.
        assert_eq!(tl.tracks[0].clips[0].start_frame, 0); // b
        assert_eq!(tl.tracks[1].clips[0].start_frame, 30); // x
    }

    #[test]
    fn ripple_delete_refuses_when_follower_would_collide() {
        let mut tl = Timeline::new();
        tl.tracks
            .push(one_track(vec![clip("a", 0, 30), clip("b", 30, 30)], true));
        // follower has a clip at 0 and one at 60; shifting the 60-clip left by 30
        // -> [30,60), but the 0-clip is [0,30): no collision actually. Make it collide:
        // clip at 20 (fixed, before removed range end? removed range is [0,30)).
        let mut a = Track::new("audio", ClipType::Audio);
        a.sync_locked = true;
        a.clips.push(clip("fixed", 40, 30)); // [40,70)
        a.clips.push(clip("mover", 60, 30)); // would shift to [30,60) -> overlaps fixed [40,70)
        tl.tracks.push(a);

        let res = ripple_delete(&mut tl, &["a".to_string()].into_iter().collect(), &label);
        assert!(res.is_err());
        // nothing mutated: a still present, follower unchanged.
        assert!(tl.tracks[0].clips.iter().any(|c| c.id == "a"));
        assert_eq!(
            tl.tracks[1]
                .clips
                .iter()
                .find(|c| c.id == "mover")
                .unwrap()
                .start_frame,
            60
        );
    }

    // A sync-locked follower can never refuse through `ripple_delete` /
    // `ripple_delete_ranges_on_track` for moving past frame 0. Shifts are
    // computed by `RippleEngine::try_compute_ripple_shifts_for_ranges`, which
    // only counts removed ranges with `end <= clip.start`. On a well-formed
    // timeline (all starts validated >= 0) those ranges are disjoint, ordered,
    // and lie within `[0, start]`, so the accumulated shift never exceeds
    // `clip.start` and the new start frame stays >= 0. The negative-start
    // refusal in `validate_shifts` is therefore reachable only via a direct
    // call with a hand-crafted negative `ClipShift`, tested below.
    #[test]
    fn validate_shifts_refuses_negative_start() {
        let mut tl = Timeline::new();
        tl.tracks.push(one_track(vec![clip("x", 0, 30)], true));
        let reason = validate_shifts(&tl, 0, &[ClipShift::new("x", -5)], &label(&tl, 0)).unwrap();
        assert!(reason.contains("past the timeline start"));
        // Non-negative shifts on the same track pass.
        assert!(validate_shifts(&tl, 0, &[ClipShift::new("x", 10)], &label(&tl, 0)).is_none());
    }

    #[test]
    fn ripple_delete_ranges_cuts_and_reports() {
        let mut tl = Timeline::new();
        tl.tracks.push(one_track(
            vec![clip("a", 0, 100), clip("b", 100, 100)],
            true,
        ));
        let g = SeqIdGen::new("r-");
        let out = ripple_delete_ranges_on_track(&mut tl, 0, &[FrameRange::new(40, 60)], &label, &g);
        match out {
            RippleOutcome::Ok(report) => {
                assert_eq!(report.removed_frames, 20);
                assert_eq!(report.anchor_track_index, 0);
                // a split into [0,40)+[60,100)->shift; total span shrinks by 20.
                let max_end = tl.tracks[0]
                    .clips
                    .iter()
                    .map(|c| c.end_frame())
                    .max()
                    .unwrap();
                assert_eq!(max_end, 180); // was 200, minus 20 removed
            }
            RippleOutcome::Refused(r) => panic!("unexpected refuse: {r}"),
        }
    }

    #[test]
    fn ripple_delete_ranges_keeps_linked_av_frame_exact() {
        let mut tl = Timeline::new();
        let mut video = Track::new("video", ClipType::Video);
        let mut video_clip = clip("video-clip", 0, 900);
        video_clip.link_group_id = Some("av".into());
        video.clips.push(video_clip);
        let mut audio = Track::new("audio", ClipType::Audio);
        let mut audio_clip = clip("audio-clip", 0, 900);
        audio_clip.media_type = ClipType::Audio;
        audio_clip.link_group_id = Some("av".into());
        audio.clips.push(audio_clip);
        tl.tracks.extend([video, audio]);

        let g = SeqIdGen::new("r-");
        let out = ripple_delete_ranges_on_track(&mut tl, 1, &[FrameRange::new(6, 12)], &label, &g);
        assert!(matches!(out, RippleOutcome::Ok(_)));
        let spans = |track: &Track| {
            track
                .clips
                .iter()
                .map(|clip| (clip.start_frame, clip.end_frame()))
                .collect::<Vec<_>>()
        };
        assert_eq!(spans(&tl.tracks[0]), vec![(0, 6), (6, 894)]);
        assert_eq!(spans(&tl.tracks[1]), vec![(0, 6), (6, 894)]);
    }

    #[test]
    fn ripple_delete_multiple_ranges_with_sync_locked_captions_is_atomic() {
        let mut tl = Timeline::new();
        let mut captions = Track::new("captions", ClipType::Video);
        captions.sync_locked = true;
        captions.clips.extend([
            clip("caption-1", 0, 86),
            clip("caption-2", 146, 116),
            clip("caption-3", 350, 21),
            clip("caption-4", 371, 57),
            clip("caption-5", 428, 82),
        ]);
        let mut video = Track::new("video", ClipType::Video);
        let mut video_clip = clip("video-clip", 0, 900);
        video_clip.link_group_id = Some("av".into());
        video.clips.push(video_clip);
        let mut audio = Track::new("audio", ClipType::Audio);
        let mut audio_clip = clip("audio-clip", 0, 900);
        audio_clip.media_type = ClipType::Audio;
        audio_clip.link_group_id = Some("av".into());
        audio.clips.push(audio_clip);
        tl.tracks.extend([captions, video, audio]);
        let before = tl.clone();

        let g = SeqIdGen::new("r-");
        let out = ripple_delete_ranges_on_track(
            &mut tl,
            2,
            &[FrameRange::new(151, 154), FrameRange::new(358, 365)],
            &label,
            &g,
        );

        assert!(matches!(out, RippleOutcome::Refused(_)));
        assert_eq!(tl, before, "a follower collision must be side-effect free");
    }

    #[test]
    fn ripple_delete_ranges_refuses_on_locked_follower_collision() {
        let mut tl = Timeline::new();
        tl.tracks.push(one_track(vec![clip("a", 0, 200)], true));
        // follower not touched, sync-locked, clips collide after shift.
        let mut f = Track::new("f", ClipType::Audio);
        f.sync_locked = true;
        f.clips.push(clip("fixed", 0, 50)); // [0,50)
        f.clips.push(clip("mover", 100, 50)); // shift left by 60 (range len) -> [40,90) overlaps fixed? no, [0,50) vs [40,90) overlap at [40,50)
        tl.tracks.push(f);
        let g = SeqIdGen::default();
        let out = ripple_delete_ranges_on_track(&mut tl, 0, &[FrameRange::new(0, 60)], &label, &g);
        assert!(matches!(out, RippleOutcome::Refused(_)));
        // unchanged
        assert_eq!(tl.tracks[0].clips[0].duration_frames, 200);
    }

    #[test]
    fn ripple_insert_pushes_and_places() {
        let mut tl = Timeline::new();
        tl.tracks
            .push(one_track(vec![clip("a", 0, 30), clip("b", 30, 30)], true));
        let g = SeqIdGen::new("n-");
        let spec = PlaceSpec::new("m", ClipType::Video, 0, 20);
        let created = ripple_insert(&mut tl, &[spec], 0, 15, &g);
        assert_eq!(created.len(), 1);
        // straddler a [0,30) split at 15 -> a[0,15) + right[15,30). right + b push +20.
        // inserted clip occupies [15,35).
        let inserted = tl.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == created[0])
            .unwrap();
        assert_eq!((inserted.start_frame, inserted.end_frame()), (15, 35));
        // b originally at 30 -> pushed to 50.
        assert_eq!(
            tl.tracks[0]
                .clips
                .iter()
                .find(|c| c.id == "b")
                .unwrap()
                .start_frame,
            50
        );
    }

    #[test]
    fn extreme_range_and_insert_refuse_without_mutation_or_ids() {
        let mut tl = Timeline::new();
        tl.tracks.push(one_track(vec![clip("a", 0, 30)], true));
        let before = tl.clone();
        let ids = SeqIdGen::new("extreme-ripple-");

        let delete_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ripple_delete_ranges_on_track(
                &mut tl,
                0,
                &[FrameRange::new(i32::MIN, i32::MAX)],
                &label,
                &ids,
            )
        }));
        assert!(matches!(delete_result.unwrap(), RippleOutcome::Refused(_)));
        assert_eq!(tl, before);
        assert_eq!(ids.count(), 0);

        let insert_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ripple_insert(
                &mut tl,
                &[PlaceSpec::new("m", ClipType::Video, 0, 1)],
                0,
                i32::MAX,
                &ids,
            )
        }));
        assert!(insert_result.unwrap().is_empty());
        assert_eq!(tl, before);
        assert_eq!(ids.count(), 0);
    }
}
