//! Clip duplication (Option/Alt-drag copy). Deep-clips each source clip —
//! including all keyframe tracks / grade / chroma / masks / effects / text /
//! transform / crop / fades — mints a fresh id, shifts `start_frame` by
//! `offset_frames`, places it on `target_track_indexes[i]`, and remaps the
//! link group (a multi-clip group — e.g. an A/V linked pair — gets a fresh
//! shared id so the copies stay linked; a single-clip group is cleared).
//! The destination range is cleared overwrite-style first (mirrors
//! `move_clips`), so a duplicate landing on an existing clip overwrites it.
//!
//! Companion to [`crate::ops::move_clips`]: same destination-clearing +
//! pin-by-id + sort + prune flow, but the source clip stays put and a deep
//! copy is dropped at the target.

use std::collections::HashMap;

use opentake_domain::{Clip, Timeline};

use crate::id::IdGen;
use crate::ops::clear_region::clear_region;
use crate::ops::place::sort_clips;
use crate::ops::tracks::prune_empty_tracks;

/// Deep-copy each clip in `clip_ids` to a new position: `start_frame` shifted
/// by `offset_frames`, placed on `target_track_indexes[i]` (one target per
/// source, by index). Returns the ids of the newly created clips (in input
/// order). Missing clips or out-of-range / type-incompatible targets are
/// silently skipped (mirrors `move_clips`'s "guard ... continue").
///
/// Each duplicate:
/// - gets a fresh id from `ids`,
/// - keeps every field of the source (keyframe tracks, grade, chroma, masks,
///   effects, text, transform, crop, fades — `Clip: Clone` is a deep copy),
/// - has its `link_group_id` remapped (a group shared by multiple copied
///   clips gets a fresh shared id so the copies stay linked; a single-clip
///   group is cleared to `None`),
/// - has `start_frame = source.start_frame + offset_frames` (clamped `>= 0`).
pub fn duplicate_clips(
    timeline: &mut Timeline,
    clip_ids: &[String],
    offset_frames: i32,
    target_track_indexes: &[usize],
    ids: &dyn IdGen,
) -> Vec<String> {
    if clip_ids.is_empty() {
        return Vec::new();
    }

    // Resolve each source clip + validate its target track. Collect up front so
    // the mutation phase can pin tracks by id (pruning could shift indices).
    struct Plan {
        clone: Clip,
        to_track_id: String,
        to_frame: i32,
    }
    let mut plans: Vec<Plan> = Vec::new();
    for (i, id) in clip_ids.iter().enumerate() {
        let Some((ti, ci)) = find(timeline, id) else {
            continue;
        };
        let Some(&to_track) = target_track_indexes.get(i) else {
            continue;
        };
        if to_track >= timeline.tracks.len() {
            continue;
        }
        let src_type = timeline.tracks[ti].kind;
        let dest_type = timeline.tracks[to_track].kind;
        if !dest_type.is_compatible(src_type) {
            continue;
        }
        let clone = timeline.tracks[ti].clips[ci].clone();
        let to_frame = (clone.start_frame + offset_frames).max(0);
        plans.push(Plan {
            clone,
            to_track_id: timeline.tracks[to_track].id.clone(),
            to_frame,
        });
    }
    if plans.is_empty() {
        return Vec::new();
    }

    // Clear each destination range (pin by track id) so the duplicate overwrites
    // whatever was there, exactly like `move_clips` / `place_clip` do.
    for plan in &plans {
        if let Some(idx) = timeline
            .tracks
            .iter()
            .position(|t| t.id == plan.to_track_id)
        {
            clear_region(
                timeline,
                idx,
                plan.to_frame,
                plan.to_frame + plan.clone.duration_frames,
                false,
                ids,
            );
        }
    }

    // Build the link-group remap table (mirrors upstream's `groupCounts` /
    // `groupRemap` in EditorViewModel+Clipboard.swift): a group shared by
    // multiple copied clips (e.g. an A/V linked pair) maps to a fresh shared id
    // so the copies stay linked to each other; a group with only one clip (or
    // no group) maps to None — that copy stands alone.
    let mut group_counts: HashMap<Option<String>, usize> = HashMap::new();
    for plan in &plans {
        *group_counts
            .entry(plan.clone.link_group_id.clone())
            .or_insert(0) += 1;
    }
    let mut group_remap: HashMap<Option<String>, Option<String>> = HashMap::new();
    for (group_id, &count) in &group_counts {
        let new_id = if count > 1 && group_id.is_some() {
            Some(ids.next_id())
        } else {
            None
        };
        group_remap.insert(group_id.clone(), new_id);
    }

    // Drop each deep copy at its target frame with a fresh id + remapped link.
    let mut created = Vec::new();
    for plan in plans {
        if let Some(idx) = timeline
            .tracks
            .iter()
            .position(|t| t.id == plan.to_track_id)
        {
            let mut clip = plan.clone;
            clip.id = ids.next_id();
            clip.start_frame = plan.to_frame;
            // Remap the link group: multi-clip groups get the fresh shared id,
            // single-clip groups (and None) clear to None.
            let remapped = group_remap.get(&clip.link_group_id).cloned().flatten();
            clip.link_group_id = remapped;
            created.push(clip.id.clone());
            timeline.tracks[idx].clips.push(clip);
            sort_clips(&mut timeline.tracks[idx]);
        }
    }
    prune_empty_tracks(timeline);
    created
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
    use opentake_domain::{
        ChromaKey, ClipType, ColorGrade, Crop, Effect, Interpolation, Keyframe, KeyframeTrack,
        Mask, MaskShape, Point2, Track,
    };

    fn clip(id: &str, start: i32, dur: i32) -> Clip {
        Clip::new(id, "asset", start, dur)
    }

    fn two_video_tracks() -> Timeline {
        let mut tl = Timeline::new();
        let mut v1 = Track::new("v1", ClipType::Video);
        v1.clips.push(clip("a", 0, 30));
        let v2 = Track::new("v2", ClipType::Video);
        tl.tracks.push(v1);
        tl.tracks.push(v2);
        tl
    }

    #[test]
    fn duplicate_keeps_original_and_creates_copy_at_offset() {
        let mut tl = two_video_tracks();
        let g = SeqIdGen::new("d-");
        let created = duplicate_clips(&mut tl, &["a".into()], 100, &[0], &g);
        assert_eq!(created.len(), 1);
        // Original stays put.
        assert!(tl.tracks[0]
            .clips
            .iter()
            .any(|c| c.id == "a" && c.start_frame == 0));
        // Copy lands at frame 100 on the same track with a fresh id.
        let copy = tl.tracks[0].clips.iter().find(|c| c.id == "d-1").unwrap();
        assert_eq!(copy.start_frame, 100);
        assert_eq!(copy.duration_frames, 30);
        assert_eq!(copy.media_ref, "asset");
    }

    #[test]
    fn duplicate_clears_link_group_id() {
        let mut tl = two_video_tracks();
        // Mark the source as linked.
        tl.tracks[0].clips[0].link_group_id = Some("grp".into());
        let g = SeqIdGen::default();
        let created = duplicate_clips(&mut tl, &["a".into()], 50, &[0], &g);
        let copy = tl.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == created[0])
            .unwrap();
        assert!(
            copy.link_group_id.is_none(),
            "duplicate must not inherit link"
        );
        // Original keeps its link group.
        assert_eq!(tl.tracks[0].clips[0].link_group_id.as_deref(), Some("grp"));
    }

    fn find_clip<'a>(tl: &'a Timeline, id: &str) -> &'a Clip {
        tl.tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .find(|c| c.id == id)
            .expect("clip exists")
    }

    #[test]
    fn duplicate_clips_remaps_link_group_for_multi_clip_group() {
        // A/V linked pair (shared group "g1") + a lone clip in its own group
        // ("g2"). Mirrors upstream's groupCounts/groupRemap: the pair's copies
        // must share a fresh group id; the lone clip's copy clears to None.
        let mut tl = Timeline::new();
        let mut v = Track::new("v", ClipType::Video);
        let mut va = clip("va", 0, 30);
        va.link_group_id = Some("g1".into());
        let mut sv = clip("sv", 60, 30);
        sv.link_group_id = Some("g2".into());
        v.clips.push(va);
        v.clips.push(sv);
        let mut a = Track::new("a", ClipType::Audio);
        let mut aa = clip("aa", 0, 30);
        aa.media_type = ClipType::Audio;
        aa.link_group_id = Some("g1".into());
        a.clips.push(aa);
        tl.tracks.push(v);
        tl.tracks.push(a);

        let g = SeqIdGen::default();
        let created = duplicate_clips(
            &mut tl,
            &["va".into(), "aa".into(), "sv".into()],
            200,
            &[0, 1, 0],
            &g,
        );
        assert_eq!(created.len(), 3);

        // The A/V pair copies share a NEW link_group_id (same as each other,
        // different from the source "g1").
        let va_copy = find_clip(&tl, &created[0]);
        let aa_copy = find_clip(&tl, &created[1]);
        assert_eq!(va_copy.link_group_id, aa_copy.link_group_id);
        assert_ne!(va_copy.link_group_id.as_deref(), Some("g1"));
        assert!(
            va_copy.link_group_id.is_some(),
            "multi-clip group copies must stay linked"
        );

        // The lone clip's group ("g2", count == 1) clears to None.
        let sv_copy = find_clip(&tl, &created[2]);
        assert!(
            sv_copy.link_group_id.is_none(),
            "single-clip group must clear to None"
        );

        // Originals keep their original group ids.
        assert_eq!(find_clip(&tl, "va").link_group_id.as_deref(), Some("g1"));
        assert_eq!(find_clip(&tl, "aa").link_group_id.as_deref(), Some("g1"));
    }

    #[test]
    fn duplicate_deep_copies_keyframe_tracks() {
        let mut tl = two_video_tracks();
        // Give the source an opacity track + volume track with keyframes.
        tl.tracks[0].clips[0].opacity_track = Some(KeyframeTrack::from_keyframes(vec![
            Keyframe::new(0, 0.0),
            Keyframe::new(30, 1.0),
        ]));
        tl.tracks[0].clips[0].volume_track = Some(KeyframeTrack::from_keyframes(vec![
            Keyframe::new(0, -6.0),
            Keyframe::new(30, 0.0),
        ]));
        let g = SeqIdGen::default();
        let created = duplicate_clips(&mut tl, &["a".into()], 100, &[0], &g);
        let copy = tl.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == created[0])
            .unwrap();
        // Keyframe offsets are clip-relative, so they're identical to the source
        // (the copy's start_frame moved, but offsets stay).
        let op = copy.opacity_track.as_ref().unwrap();
        assert_eq!(
            op.keyframes.iter().map(|k| k.frame).collect::<Vec<_>>(),
            vec![0, 30]
        );
        let vol = copy.volume_track.as_ref().unwrap();
        assert_eq!(
            vol.keyframes.iter().map(|k| k.value).collect::<Vec<_>>(),
            vec![-6.0, 0.0]
        );
        // Mutating the copy's track must not touch the original (deep copy).
        let copy_op = copy.opacity_track.as_ref().unwrap().clone();
        tl.tracks[0]
            .clips
            .iter_mut()
            .find(|c| c.id == created[0])
            .unwrap()
            .opacity_track = None;
        assert!(tl.tracks[0].clips[0].opacity_track.is_some());
        assert_eq!(
            tl.tracks[0].clips[0].opacity_track.as_ref().unwrap(),
            &copy_op
        );
    }

    #[test]
    fn duplicate_deep_copies_grade_masks_effects() {
        let mut tl = two_video_tracks();
        let src = &mut tl.tracks[0].clips[0];
        src.color_grade = Some(ColorGrade {
            exposure: 0.5,
            ..Default::default()
        });
        src.chroma_key = Some(ChromaKey::default());
        src.masks = vec![Mask {
            shape: MaskShape::Circle {
                center: Point2::new(0.5, 0.5),
                radius: Point2::new(0.3, 0.3),
            },
            feather: 0.05,
            invert: false,
        }];
        src.effects = vec![Effect::new("gaussianBlur").with_param("radius", 4.0)];
        let orig_color_grade = src.color_grade;
        let orig_chroma_key = src.chroma_key;
        let g = SeqIdGen::default();
        let created = duplicate_clips(&mut tl, &["a".into()], 100, &[0], &g);
        let copy = tl.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == created[0])
            .unwrap();
        assert_eq!(copy.color_grade, orig_color_grade);
        assert_eq!(copy.chroma_key, orig_chroma_key);
        assert_eq!(copy.masks.len(), 1);
        assert_eq!(copy.effects.len(), 1);
        // Mutate the copy's masks; the original must be unaffected (no shared ref).
        let copy_masks = copy.masks.clone();
        tl.tracks[0]
            .clips
            .iter_mut()
            .find(|c| c.id == created[0])
            .unwrap()
            .masks
            .clear();
        assert_eq!(tl.tracks[0].clips[0].masks, copy_masks);
    }

    #[test]
    fn duplicate_to_different_track_uses_target_index() {
        let mut tl = two_video_tracks();
        let g = SeqIdGen::default();
        let created = duplicate_clips(&mut tl, &["a".into()], 100, &[1], &g);
        // Copy lands on v2 (index 1).
        let copy = tl.tracks[1]
            .clips
            .iter()
            .find(|c| c.id == created[0])
            .unwrap();
        assert_eq!(copy.start_frame, 100);
        // Original still on v1.
        assert!(tl.tracks[0].clips.iter().any(|c| c.id == "a"));
    }

    #[test]
    fn duplicate_multiple_clips_preserve_relative_spacing() {
        let mut tl = Timeline::new();
        let mut v = Track::new("v", ClipType::Video);
        v.clips.push(clip("a", 0, 30));
        v.clips.push(clip("b", 60, 30)); // 30-frame gap
        tl.tracks.push(v);
        let g = SeqIdGen::default();
        let created = duplicate_clips(&mut tl, &["a".into(), "b".into()], 100, &[0, 0], &g);
        assert_eq!(created.len(), 2);
        let c0 = tl.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == created[0])
            .unwrap();
        let c1 = tl.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == created[1])
            .unwrap();
        // a@0 -> 100, b@60 -> 160; gap of 30 preserved.
        assert_eq!(c0.start_frame, 100);
        assert_eq!(c1.start_frame, 160);
    }

    #[test]
    fn duplicate_overwrites_blocking_clip_at_destination() {
        let mut tl = two_video_tracks();
        // Place a blocker on v2 at [90,150); duplicating a to v2@100 overwrites the overlap.
        tl.tracks[1].clips.push(clip("blocker", 90, 60));
        let g = SeqIdGen::new("r-");
        let created = duplicate_clips(&mut tl, &["a".into()], 100, &[1], &g);
        let v2 = tl.tracks.iter().find(|t| t.id == "v2").unwrap();
        let copy = v2.clips.iter().find(|c| c.id == created[0]).unwrap();
        assert_eq!((copy.start_frame, copy.end_frame()), (100, 130));
        // No clip other than the copy covers [100,130).
        let covering = v2
            .clips
            .iter()
            .filter(|c| c.id != created[0] && c.start_frame < 130 && c.end_frame() > 100)
            .count();
        assert_eq!(covering, 0);
    }

    #[test]
    fn duplicate_clamps_start_frame_to_zero() {
        let mut tl = two_video_tracks();
        let g = SeqIdGen::default();
        // a starts at 0; offset -50 would put it at -50 -> clamped to 0.
        let created = duplicate_clips(&mut tl, &["a".into()], -50, &[0], &g);
        let copy = tl.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == created[0])
            .unwrap();
        assert_eq!(copy.start_frame, 0);
    }

    #[test]
    fn duplicate_skips_missing_clip() {
        let mut tl = two_video_tracks();
        let g = SeqIdGen::default();
        let created = duplicate_clips(&mut tl, &["nope".into()], 100, &[0], &g);
        assert!(created.is_empty());
    }

    #[test]
    fn duplicate_skips_incompatible_target_track() {
        let mut tl = Timeline::new();
        let mut v = Track::new("v", ClipType::Video);
        v.clips.push(clip("a", 0, 30));
        let a = Track::new("a", ClipType::Audio);
        tl.tracks.push(v);
        tl.tracks.push(a);
        let g = SeqIdGen::default();
        // Duplicating a video clip onto an audio track -> incompatible -> skipped.
        let created = duplicate_clips(&mut tl, &["a".into()], 100, &[1], &g);
        assert!(created.is_empty());
        assert!(tl.tracks[1].clips.is_empty());
    }

    #[test]
    fn duplicate_skips_out_of_range_target() {
        let mut tl = two_video_tracks();
        let g = SeqIdGen::default();
        let created = duplicate_clips(&mut tl, &["a".into()], 100, &[99], &g);
        assert!(created.is_empty());
    }

    #[test]
    fn duplicate_copies_text_and_transform_fields() {
        let mut tl = Timeline::new();
        let mut t = Track::new("t", ClipType::Text);
        let mut c = Clip::new("txt", "", 0, 30);
        c.media_type = ClipType::Text;
        c.source_clip_type = ClipType::Text;
        c.text_content = Some("Hello".into());
        c.transform = opentake_domain::Transform::from_center(
            opentake_domain::Point { x: 0.25, y: 0.75 },
            0.5,
            0.5,
        );
        c.crop = Crop {
            left: 0.1,
            top: 0.2,
            right: 0.3,
            bottom: 0.4,
        };
        c.fade_in_frames = 5;
        c.fade_in_interpolation = Interpolation::Smooth;
        c.rotation_track = Some(KeyframeTrack::from_keyframes(vec![
            Keyframe::with_interpolation(0, 0.0, Interpolation::Linear),
            Keyframe::new(10, 0.2),
        ]));
        t.clips.push(c);
        tl.tracks.push(t);
        let g = SeqIdGen::default();
        let created = duplicate_clips(&mut tl, &["txt".into()], 50, &[0], &g);
        let copy = tl.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == created[0])
            .unwrap();
        assert_eq!(copy.text_content.as_deref(), Some("Hello"));
        assert_eq!(copy.transform.center_x, 0.25);
        assert_eq!(copy.crop.left, 0.1);
        assert_eq!(copy.fade_in_frames, 5);
        assert_eq!(copy.fade_in_interpolation, Interpolation::Smooth);
        assert!(copy.rotation_track.is_some());
    }
}
