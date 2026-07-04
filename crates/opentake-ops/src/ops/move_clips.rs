//! Clip moving. 1:1 port of `moveClips(_:)` from
//! `EditorViewModel+ClipMutations.swift`.
//!
//! Moved clips are pulled off their source tracks first so the per-destination
//! `clearRegion` never touches them, then each is dropped at its exact target
//! frame (overwrite-style). Tracks are pinned by id across the clears because
//! pruning could otherwise shift indices.

use opentake_domain::{Clip, Timeline};

use crate::id::IdGen;
use crate::ops::clear_region::clear_region;
use crate::ops::place::sort_clips;
use crate::ops::tracks::prune_empty_tracks;

/// One resolved move: clip `clip_id` to track index `to_track` at frame
/// `to_frame` (already clamped `>= 0` by the caller).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipMove {
    pub clip_id: String,
    pub to_track: usize,
    pub to_frame: i32,
}

/// Move clips to their targets. Incompatible-destination or missing-clip moves
/// are silently dropped (mirrors upstream's `guard ... continue`). Returns the
/// number of clips actually moved.
pub fn move_clips(timeline: &mut Timeline, moves: &[ClipMove], ids: &dyn IdGen) -> usize {
    if moves.is_empty() {
        return 0;
    }

    // Collect current state + validate track-type compatibility.
    struct Info {
        clip: Clip,
        to_track_id: String,
        to_frame: i32,
    }
    let mut infos: Vec<Info> = Vec::new();
    for m in moves {
        let Some((ti, ci)) = find(timeline, &m.clip_id) else {
            continue;
        };
        if m.to_track >= timeline.tracks.len() {
            continue;
        }
        let src_type = timeline.tracks[ti].kind;
        let dest_type = timeline.tracks[m.to_track].kind;
        if !dest_type.is_compatible(src_type) {
            continue;
        }
        infos.push(Info {
            clip: timeline.tracks[ti].clips[ci].clone(),
            to_track_id: timeline.tracks[m.to_track].id.clone(),
            to_frame: m.to_frame.max(0),
        });
    }
    if infos.is_empty() {
        return 0;
    }

    // Pull moved clips off their source tracks first.
    for info in &infos {
        if let Some((ti, ci)) = find(timeline, &info.clip.id) {
            timeline.tracks[ti].clips.remove(ci);
        }
    }

    // Trim / remove non-moved clips blocking each destination range (pin by id).
    for info in &infos {
        if let Some(idx) = timeline
            .tracks
            .iter()
            .position(|t| t.id == info.to_track_id)
        {
            clear_region(
                timeline,
                idx,
                info.to_frame,
                info.to_frame + info.clip.duration_frames,
                false,
                ids,
            );
        }
    }

    // Drop each clip at its exact target frame.
    let mut moved = 0;
    for info in &infos {
        if let Some(idx) = timeline
            .tracks
            .iter()
            .position(|t| t.id == info.to_track_id)
        {
            let mut clip = info.clip.clone();
            clip.start_frame = info.to_frame;
            timeline.tracks[idx].clips.push(clip);
            moved += 1;
        }
    }
    for ti in 0..timeline.tracks.len() {
        sort_clips(&mut timeline.tracks[ti]);
    }
    prune_empty_tracks(timeline);
    moved
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

    fn clip(id: &str, start: i32, dur: i32) -> Clip {
        Clip::new(id, "asset", start, dur)
    }

    fn two_video_tracks() -> Timeline {
        let mut tl = Timeline::new();
        let mut v1 = Track::new("v1", ClipType::Video);
        v1.clips.push(clip("a", 0, 30));
        let mut v2 = Track::new("v2", ClipType::Video);
        v2.clips.push(clip("b", 0, 30));
        tl.tracks.push(v1);
        tl.tracks.push(v2);
        tl
    }

    #[test]
    fn moves_clip_to_new_frame_same_track() {
        let mut tl = two_video_tracks();
        let g = SeqIdGen::default();
        let n = move_clips(
            &mut tl,
            &[ClipMove {
                clip_id: "a".into(),
                to_track: 0,
                to_frame: 100,
            }],
            &g,
        );
        assert_eq!(n, 1);
        assert_eq!(
            tl.tracks[0]
                .clips
                .iter()
                .find(|c| c.id == "a")
                .unwrap()
                .start_frame,
            100
        );
    }

    #[test]
    fn moves_clip_across_tracks() {
        let mut tl = two_video_tracks();
        let g = SeqIdGen::default();
        move_clips(
            &mut tl,
            &[ClipMove {
                clip_id: "a".into(),
                to_track: 1,
                to_frame: 100,
            }],
            &g,
        );
        // a now on v2; v1 emptied -> pruned.
        assert!(tl.tracks.iter().all(|t| t.id != "v1"));
        let v2 = tl.tracks.iter().find(|t| t.id == "v2").unwrap();
        assert!(v2.clips.iter().any(|c| c.id == "a" && c.start_frame == 100));
    }

    #[test]
    fn move_clears_blocking_clip_at_destination() {
        let mut tl = two_video_tracks();
        // put a blocker on v2 at [90,150); moving a to v2@100 should overwrite the overlap.
        tl.tracks[1].clips.push(clip("blocker", 90, 60));
        let g = SeqIdGen::new("r-");
        move_clips(
            &mut tl,
            &[ClipMove {
                clip_id: "a".into(),
                to_track: 1,
                to_frame: 100,
            }],
            &g,
        );
        // a occupies [100,130); blocker [90,150) gets split -> [90,100) + [130,150).
        let v2 = tl.tracks.iter().find(|t| t.id == "v2").unwrap();
        let a = v2.clips.iter().find(|c| c.id == "a").unwrap();
        assert_eq!((a.start_frame, a.end_frame()), (100, 130));
        // overlap region cleared; no clip covers [100,130) except a.
        let covering = v2
            .clips
            .iter()
            .filter(|c| c.id != "a" && c.start_frame < 130 && c.end_frame() > 100)
            .count();
        assert_eq!(covering, 0);
    }

    #[test]
    fn incompatible_destination_is_dropped() {
        let mut tl = Timeline::new();
        let mut v = Track::new("v", ClipType::Video);
        v.clips.push(clip("a", 0, 30));
        let a = Track::new("a", ClipType::Audio);
        tl.tracks.push(v);
        tl.tracks.push(a);
        let g = SeqIdGen::default();
        // moving a video clip to an audio track -> incompatible -> no move.
        let n = move_clips(
            &mut tl,
            &[ClipMove {
                clip_id: "a".into(),
                to_track: 1,
                to_frame: 0,
            }],
            &g,
        );
        assert_eq!(n, 0);
        assert!(tl.tracks[0].clips.iter().any(|c| c.id == "a"));
    }
}
