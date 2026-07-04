//! Overwrite-style region clearing. 1:1 port of `clearRegion(trackIndex:start:
//! end:prune:)` from `EditorViewModel+ClipMutations.swift`.
//!
//! Computes overwrite actions with [`OverwriteEngine`] and lands them: removing,
//! trimming, or splitting the clips overlapping `[start, end)` so a new clip can
//! be placed there. The split branch re-runs the real split path (so its new id
//! and keyframe boundaries are identical to a manual split), then removes the
//! piece sitting inside the region — splitting once more if that piece overruns
//! `end`. This is the shared "make room" primitive behind add / move / paste.

use opentake_domain::Timeline;

use crate::engines::{OverwriteAction, OverwriteEngine};
use crate::id::IdGen;
use crate::ops::split::split_clip;

/// Clear `[start, end)` on the track at `track_index` by removing / trimming /
/// splitting overlapping clips. `prune` controls whether emptied tracks are
/// dropped afterward (passed through; the transaction layer usually prunes once
/// at the end, so callers commonly pass `false`).
pub fn clear_region(
    timeline: &mut Timeline,
    track_index: usize,
    start: i32,
    end: i32,
    prune: bool,
    ids: &dyn IdGen,
) {
    if track_index >= timeline.tracks.len() {
        return;
    }
    let actions =
        OverwriteEngine::compute_overwrite(&timeline.tracks[track_index].clips, start, end);

    for action in actions {
        match action {
            OverwriteAction::Remove { clip_id } => {
                remove_clip(timeline, &clip_id);
            }

            OverwriteAction::TrimEnd {
                clip_id,
                new_duration,
            } => {
                if let Some((ti, ci)) = find(timeline, &clip_id) {
                    let clip = &timeline.tracks[ti].clips[ci];
                    let source_delta =
                        ((clip.duration_frames - new_duration) as f64 * clip.speed).round() as i32;
                    let new_trim_end = clip.trim_end_frame + source_delta;
                    let c = &mut timeline.tracks[ti].clips[ci];
                    c.trim_end_frame = new_trim_end;
                    c.set_duration(new_duration);
                }
            }

            OverwriteAction::TrimStart {
                clip_id,
                new_start_frame,
                new_trim_start,
                new_duration,
            } => {
                if let Some((ti, ci)) = find(timeline, &clip_id) {
                    let c = &mut timeline.tracks[ti].clips[ci];
                    c.start_frame = new_start_frame;
                    c.trim_start_frame = new_trim_start;
                    c.set_duration(new_duration);
                }
            }

            OverwriteAction::Split { clip_id, .. } => {
                if find(timeline, &clip_id).is_some() {
                    // Split at `start`; the right half is what now covers the region.
                    split_clip(timeline, &clip_id, start, ids);
                    // Locate the freshly created right half (starts at `start`, not the original id).
                    let right = timeline
                        .tracks
                        .iter()
                        .flat_map(|t| &t.clips)
                        .find(|c| c.start_frame == start && c.id != clip_id)
                        .map(|c| (c.id.clone(), c.end_frame()));
                    if let Some((right_id, right_end)) = right {
                        if right_end > end {
                            // Right half overruns the region — split again at `end`,
                            // then drop the [start, end) middle piece.
                            split_clip(timeline, &right_id, end, ids);
                            remove_clip(timeline, &right_id);
                        } else {
                            remove_clip(timeline, &right_id);
                        }
                    }
                }
            }
        }
    }

    if prune {
        crate::ops::tracks::prune_empty_tracks(timeline);
    }
}

/// Remove a single clip by id from whatever track holds it.
pub(crate) fn remove_clip(timeline: &mut Timeline, clip_id: &str) {
    for t in &mut timeline.tracks {
        t.clips.retain(|c| c.id != clip_id);
    }
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

    fn track(clips: Vec<Clip>) -> Timeline {
        let mut tl = Timeline::new();
        let mut t = Track::new("v", ClipType::Video);
        t.clips = clips;
        tl.tracks.push(t);
        tl
    }

    fn clip(id: &str, start: i32, dur: i32) -> Clip {
        Clip::new(id, "asset", start, dur)
    }

    #[test]
    fn removes_fully_covered_clip() {
        let mut tl = track(vec![clip("inner", 110, 30)]);
        let g = SeqIdGen::default();
        clear_region(&mut tl, 0, 100, 200, false, &g);
        assert!(tl.tracks[0].clips.is_empty());
    }

    #[test]
    fn trims_left_overlap_end_with_speed() {
        // clip [50,150) speed 2.0 trimEnd 0. region [100,200): keep [50,100) dur 50.
        let mut c = clip("c", 50, 100);
        c.speed = 2.0;
        let mut tl = track(vec![c]);
        let g = SeqIdGen::default();
        clear_region(&mut tl, 0, 100, 200, false, &g);
        let c = &tl.tracks[0].clips[0];
        assert_eq!(c.duration_frames, 50);
        // source_delta = round((100-50)*2) = 100 -> trimEnd 0 + 100.
        assert_eq!(c.trim_end_frame, 100);
        assert_eq!(c.start_frame, 50);
    }

    #[test]
    fn trims_right_overlap_start() {
        // clip [150,250), region [100,200): keep [200,250).
        let mut tl = track(vec![clip("c", 150, 100)]);
        let g = SeqIdGen::default();
        clear_region(&mut tl, 0, 100, 200, false, &g);
        let c = &tl.tracks[0].clips[0];
        assert_eq!(c.start_frame, 200);
        assert_eq!(c.duration_frames, 50);
    }

    #[test]
    fn splits_spanning_clip_and_removes_middle() {
        // clip [0,300), region [100,200): leaves [0,100) and [200,300).
        let mut tl = track(vec![clip("c", 0, 300)]);
        let g = SeqIdGen::new("r-");
        clear_region(&mut tl, 0, 100, 200, false, &g);
        let mut spans: Vec<(i32, i32)> = tl.tracks[0]
            .clips
            .iter()
            .map(|c| (c.start_frame, c.end_frame()))
            .collect();
        spans.sort();
        assert_eq!(spans, vec![(0, 100), (200, 300)]);
    }

    #[test]
    fn spanning_clip_when_right_half_exactly_meets_end() {
        // region end == clip end after first split: right half doesn't overrun.
        // clip [0,200), region [100,200): leaves only [0,100).
        let mut tl = track(vec![clip("c", 0, 200)]);
        let g = SeqIdGen::new("r-");
        clear_region(&mut tl, 0, 100, 200, false, &g);
        // [100,200) overlaps right edge only -> trimStart path actually (cs<start false? cs=0<100 true, ce=200>200 false) -> trimEnd.
        // So this is the left-overlap branch: keep [0,100).
        assert_eq!(tl.tracks[0].clips.len(), 1);
        let c = &tl.tracks[0].clips[0];
        assert_eq!((c.start_frame, c.end_frame()), (0, 100));
    }

    #[test]
    fn prune_drops_emptied_track() {
        let mut tl = track(vec![clip("inner", 0, 100)]);
        let g = SeqIdGen::default();
        clear_region(&mut tl, 0, 0, 100, true, &g);
        assert!(tl.tracks.is_empty());
    }
}
