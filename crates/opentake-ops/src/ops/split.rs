//! Clip splitting. 1:1 port of `splitClip` / `splitSingleClip` from
//! `EditorViewModel+ClipMutations.swift`, minus the AppKit/undo glue.
//!
//! Source consumption is reallocated across the two halves by speed
//! (`round(offset * speed)`) so the spliced pair stays equivalent to the
//! original. All six animatable tracks are split at the cut with a boundary
//! keyframe (via [`opentake_domain::split_keyframe_track`]) so each curve stays
//! continuous. Linked clips split together and each side becomes its own pair.

use std::collections::HashSet;

use opentake_domain::Timeline;

use crate::id::IdGen;
use crate::ops::linking::linked_partner_ids;
use crate::ops::place::sort_clips;

/// Split `clip_id` at `at_frame`, also splitting linked partners. Returns the ids
/// of the newly created right-half clips. No-op (empty) if the clip isn't found.
/// 1:1 port of `splitClip(clipId:atFrame:)`.
pub fn split_clip(
    timeline: &mut Timeline,
    clip_id: &str,
    at_frame: i32,
    ids: &dyn IdGen,
) -> Vec<String> {
    let Some(loc) = find(timeline, clip_id) else {
        return Vec::new();
    };
    let clip = &timeline.tracks[loc.0].clips[loc.1];
    let group_ids: Vec<String> = if clip.link_group_id.is_some() {
        let mut v = vec![clip_id.to_string()];
        v.extend(linked_partner_ids(timeline, clip_id));
        v
    } else {
        vec![clip_id.to_string()]
    };

    let mut right_ids = Vec::new();
    for id in &group_ids {
        if let Some(right_id) = split_single_clip(timeline, id, at_frame, ids) {
            right_ids.push(right_id);
        }
    }

    // Regroup the right halves so each side is its own linked pair.
    if group_ids.len() > 1 && !right_ids.is_empty() {
        let new_group = ids.next_id();
        let right_set: HashSet<&String> = right_ids.iter().collect();
        for t in &mut timeline.tracks {
            for c in &mut t.clips {
                if right_set.contains(&c.id) {
                    c.link_group_id = Some(new_group.clone());
                }
            }
        }
    }
    right_ids
}

/// Split a single clip at `at_frame`. Returns the new right-half id, or `None`
/// when `at_frame` is not strictly inside the clip. 1:1 port of
/// `splitSingleClip(clipId:atFrame:)`.
///
/// The half-open guard, trim folding, and per-track keyframe boundary insertion
/// all live in [`opentake_domain::split_clip`] (the model invariant); this
/// wrapper just locates the clip, mints the right-half id, and writes both halves
/// back into the track.
pub fn split_single_clip(
    timeline: &mut Timeline,
    clip_id: &str,
    at_frame: i32,
    ids: &dyn IdGen,
) -> Option<String> {
    let (ti, ci) = find(timeline, clip_id)?;
    let (left, right) =
        opentake_domain::split_clip(&timeline.tracks[ti].clips[ci], at_frame, ids.next_id())?;
    let right_id = right.id.clone();
    timeline.tracks[ti].clips[ci] = left;
    timeline.tracks[ti].clips.push(right);
    sort_clips(&mut timeline.tracks[ti]);
    Some(right_id)
}

fn find(timeline: &Timeline, clip_id: &str) -> Option<(usize, usize)> {
    for (ti, t) in timeline.tracks.iter().enumerate() {
        if let Some(ci) = t.clips.iter().position(|c| c.id == clip_id) {
            return Some((ti, ci));
        }
    }
    None
}

/// Locate a clip by id across all tracks (test helper).
#[cfg(test)]
pub(crate) fn clip_by_id<'a>(
    timeline: &'a Timeline,
    id: &str,
) -> Option<&'a opentake_domain::Clip> {
    timeline
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .find(|c| c.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::SeqIdGen;
    use opentake_domain::{Clip, ClipType, Interpolation, Keyframe, KeyframeTrack, Track};

    fn tl_single() -> Timeline {
        let mut tl = Timeline::new();
        let mut t = Track::new("v", ClipType::Video);
        t.clips.push(Clip::new("c", "asset", 100, 60)); // [100,160)
        tl.tracks.push(t);
        tl
    }

    #[test]
    fn split_outside_range_is_noop() {
        let mut tl = tl_single();
        let g = SeqIdGen::default();
        assert!(split_clip(&mut tl, "c", 100, &g).is_empty()); // == start, exclusive
        assert!(split_clip(&mut tl, "c", 160, &g).is_empty()); // == end
        assert_eq!(tl.tracks[0].clips.len(), 1);
    }

    #[test]
    fn split_reallocates_trim_by_speed() {
        let mut tl = tl_single();
        // speed 2.0, trimStart 10, trimEnd 4. split at 130 -> offset 30.
        tl.tracks[0].clips[0].speed = 2.0;
        tl.tracks[0].clips[0].trim_start_frame = 10;
        tl.tracks[0].clips[0].trim_end_frame = 4;
        let g = SeqIdGen::new("r-");
        let right = split_clip(&mut tl, "c", 130, &g);
        assert_eq!(right, vec!["r-1".to_string()]);

        let left = clip_by_id(&tl, "c").unwrap();
        let right = clip_by_id(&tl, "r-1").unwrap();
        // offset 30; leftSource=round(30*2)=60; rightSource=round((60-30)*2)=60.
        assert_eq!(left.duration_frames, 30);
        assert_eq!(left.trim_end_frame, 4 + 60); // 64
        assert_eq!(right.start_frame, 130);
        assert_eq!(right.duration_frames, 30);
        assert_eq!(right.trim_start_frame, 10 + 60); // 70
                                                     // Spliced halves reconstruct the original source span.
        assert_eq!(left.start_frame, 100);
        assert_eq!(left.end_frame(), 130);
        assert_eq!(right.end_frame(), 160);
    }

    #[test]
    fn split_clears_inner_fades() {
        let mut tl = tl_single();
        tl.tracks[0].clips[0].fade_in_frames = 10;
        tl.tracks[0].clips[0].fade_out_frames = 10;
        let g = SeqIdGen::new("r-");
        split_clip(&mut tl, "c", 130, &g);
        let left = clip_by_id(&tl, "c").unwrap();
        let right = clip_by_id(&tl, "r-1").unwrap();
        // left keeps fade-in, drops fade-out; right keeps fade-out, drops fade-in.
        assert_eq!(left.fade_in_frames, 10);
        assert_eq!(left.fade_out_frames, 0);
        assert_eq!(right.fade_in_frames, 0);
        assert_eq!(right.fade_out_frames, 10);
    }

    #[test]
    fn split_inserts_boundary_keyframe_on_each_side() {
        let mut tl = tl_single();
        // opacity track 0->1 over offsets [0,60] (linear). split at offset 30 (frame 130).
        tl.tracks[0].clips[0].opacity_track = Some(KeyframeTrack::from_keyframes(vec![
            Keyframe::with_interpolation(0, 0.0, Interpolation::Linear),
            Keyframe::new(60, 1.0),
        ]));
        let g = SeqIdGen::new("r-");
        split_clip(&mut tl, "c", 130, &g);
        let left = clip_by_id(&tl, "c").unwrap();
        let right = clip_by_id(&tl, "r-1").unwrap();
        let lk = left.opacity_track.as_ref().unwrap();
        let rk = right.opacity_track.as_ref().unwrap();
        // left: [0, 30] with boundary value 0.5 at 30.
        assert_eq!(
            lk.keyframes.iter().map(|k| k.frame).collect::<Vec<_>>(),
            [0, 30]
        );
        assert!((lk.keyframes[1].value - 0.5).abs() < 1e-9);
        // right: rebased [0, 30] with boundary 0.5 at 0.
        assert_eq!(
            rk.keyframes.iter().map(|k| k.frame).collect::<Vec<_>>(),
            [0, 30]
        );
        assert!((rk.keyframes[0].value - 0.5).abs() < 1e-9);
    }

    #[test]
    fn split_linked_pair_regroups_right_halves() {
        let mut tl = Timeline::new();
        let mut v = Track::new("v", ClipType::Video);
        let mut vc = Clip::new("v1", "asset", 100, 60);
        vc.link_group_id = Some("g1".to_string());
        v.clips.push(vc);
        let mut a = Track::new("a", ClipType::Audio);
        let mut ac = Clip::new("a1", "asset", 100, 60);
        ac.link_group_id = Some("g1".to_string());
        a.clips.push(ac);
        tl.tracks.push(v);
        tl.tracks.push(a);

        let g = SeqIdGen::new("n-");
        let rights = split_clip(&mut tl, "v1", 130, &g);
        // both partners split -> two right ids; ids minted before the new group id.
        assert_eq!(rights.len(), 2);
        // left halves keep g1; right halves share a brand-new group.
        let lv = clip_by_id(&tl, "v1").unwrap();
        let la = clip_by_id(&tl, "a1").unwrap();
        assert_eq!(lv.link_group_id.as_deref(), Some("g1"));
        assert_eq!(la.link_group_id.as_deref(), Some("g1"));
        let r0 = clip_by_id(&tl, &rights[0]).unwrap();
        let r1 = clip_by_id(&tl, &rights[1]).unwrap();
        assert_eq!(r0.link_group_id, r1.link_group_id);
        assert!(r0.link_group_id.is_some());
        assert_ne!(r0.link_group_id.as_deref(), Some("g1"));
    }
}
