//! Clip-level split — the model invariant lifted from upstream
//! `EditorViewModel.splitSingleClip`. Splitting at a timeline frame strictly
//! inside a clip folds the source frames each half consumes
//! (`round(offset * speed)`) into the surviving trim, so the two halves
//! butt-joined still reference the same source span as the original. Keyframe
//! continuity across the cut is preserved by [`split_keyframe_track`].
//!
//! This lives in the domain crate (not the editor layer) because it operates
//! purely on [`Clip`] fields and `split_keyframe_track` already lives here.
//! Because the domain crate has no `uuid` dependency, the right half's id is
//! caller-supplied — mirroring how `Clip`/`Track` ids are backfilled by the
//! project layer after load.

use crate::clip::Clip;
use crate::keyframe::{split_keyframe_track, AnimPair};

/// Split `clip` at the timeline frame `at_frame`, returning `(left, right)`.
///
/// Returns `None` unless `at_frame` is strictly inside the clip
/// (`start_frame < at_frame < end_frame`); the endpoints do not split.
///
/// The left half keeps the original id; `right_id` becomes the right half's id
/// (upstream stamps a fresh `UUID` there). `round(offset * speed)` source frames
/// are folded into each surviving trim so that, butt-joined, the two halves
/// reference the same source material as the original. All six animatable tracks
/// are cut at the offset with a boundary keyframe inserted so each curve stays
/// continuous across the seam.
pub fn split_clip(clip: &Clip, at_frame: i32, right_id: impl Into<String>) -> Option<(Clip, Clip)> {
    // Half-open guard: endpoints do not split (matches upstream `splitSingleClip`).
    if at_frame <= clip.start_frame || at_frame >= clip.end_frame() {
        return None;
    }

    let split_offset = at_frame - clip.start_frame;
    let left_source = (split_offset as f64 * clip.speed).round() as i32;
    let right_source = ((clip.duration_frames - split_offset) as f64 * clip.speed).round() as i32;

    let mut left = clip.clone();
    left.duration_frames = split_offset;
    left.trim_end_frame = clip.trim_end_frame + right_source;
    left.fade_out_frames = 0;
    left.clamp_fades_to_duration();

    let mut right = clip.clone();
    right.id = right_id.into();
    right.start_frame = at_frame;
    right.duration_frames = clip.duration_frames - split_offset;
    right.trim_start_frame = clip.trim_start_frame + left_source;
    right.fade_in_frames = 0;
    right.clamp_fades_to_duration();

    // Split every animatable track at the cut, inserting a boundary keyframe so
    // each curve stays continuous (rather than copying the whole track to both
    // halves, which would leave out-of-range / unrebased keyframes on each side).
    // Fallbacks mirror upstream exactly.
    (left.opacity_track, right.opacity_track) =
        split_keyframe_track(clip.opacity_track.as_ref(), split_offset, clip.opacity);
    (left.volume_track, right.volume_track) =
        split_keyframe_track(clip.volume_track.as_ref(), split_offset, clip.volume);
    (left.position_track, right.position_track) = split_keyframe_track(
        clip.position_track.as_ref(),
        split_offset,
        AnimPair::new(0.0, 0.0),
    );
    (left.scale_track, right.scale_track) = split_keyframe_track(
        clip.scale_track.as_ref(),
        split_offset,
        AnimPair::new(1.0, 1.0),
    );
    (left.rotation_track, right.rotation_track) =
        split_keyframe_track(clip.rotation_track.as_ref(), split_offset, 0.0);
    (left.crop_track, right.crop_track) =
        split_keyframe_track(clip.crop_track.as_ref(), split_offset, clip.crop);

    Some((left, right))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyframe::{Interpolation, Keyframe, KeyframeTrack};

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    /// A clip on `[100, 130)`, speed 1.0, with both trims set.
    fn base_clip() -> Clip {
        let mut c = Clip::new("orig", "asset1", 100, 30);
        c.trim_start_frame = 5;
        c.trim_end_frame = 7;
        c
    }

    // --- Half-open guard ---

    #[test]
    fn split_at_endpoints_or_outside_returns_none() {
        let c = base_clip(); // [100, 130)
        assert!(split_clip(&c, 100, "r").is_none()); // == start
        assert!(split_clip(&c, 130, "r").is_none()); // == end (exclusive)
        assert!(split_clip(&c, 99, "r").is_none()); // before
        assert!(split_clip(&c, 131, "r").is_none()); // after
        assert!(split_clip(&c, 115, "r").is_some()); // strictly inside
    }

    // --- Trim folding / reconstruction (speed 1.0, no rounding ambiguity) ---

    #[test]
    fn halves_butt_join_on_timeline() {
        let c = base_clip();
        let (left, right) = split_clip(&c, 112, "r").unwrap();
        // Durations partition the original.
        assert_eq!(left.duration_frames, 12);
        assert_eq!(right.duration_frames, 18);
        assert_eq!(
            left.duration_frames + right.duration_frames,
            c.duration_frames
        );
        // Left stays put; right starts at the cut; they meet exactly.
        assert_eq!(left.start_frame, 100);
        assert_eq!(left.end_frame(), 112);
        assert_eq!(right.start_frame, 112);
        assert_eq!(right.end_frame(), 130);
    }

    #[test]
    fn trims_fold_so_source_span_is_preserved() {
        let c = base_clip(); // dur 30, speed 1.0, TS=5, TE=7
                             // offset 12: left_source = round(12*1) = 12, right_source = round(18*1) = 18.
        let (left, right) = split_clip(&c, 112, "r").unwrap();
        // Left keeps original trim_start, absorbs the right half's source into trim_end.
        assert_eq!(left.trim_start_frame, 5);
        assert_eq!(left.trim_end_frame, 7 + 18);
        // Right keeps original trim_end, absorbs the left half's source into trim_start.
        assert_eq!(right.trim_start_frame, 5 + 12);
        assert_eq!(right.trim_end_frame, 7);
        // Both halves reference the same total source span as the original.
        assert_eq!(left.source_duration_frames(), c.source_duration_frames());
        assert_eq!(right.source_duration_frames(), c.source_duration_frames());
        // The cut is seamless in source space: right's visible source begins
        // exactly where left's visible source ends.
        assert_eq!(
            right.trim_start_frame,
            left.trim_start_frame + left.source_frames_consumed()
        );
        assert_eq!(
            left.trim_end_frame,
            right.trim_end_frame + right.source_frames_consumed()
        );
    }

    #[test]
    fn source_folding_uses_round_half_away_from_zero() {
        let mut c = base_clip();
        c.speed = 0.25; // 10 * 0.25 = 2.5 -> rounds to 3 (away from zero)
                        // offset 10, dur 30:
                        //   left_source  = round(10*0.25) = round(2.5) = 3
                        //   right_source = round(20*0.25) = round(5.0) = 5
        let (left, right) = split_clip(&c, 110, "r").unwrap();
        assert_eq!(left.trim_end_frame, c.trim_end_frame + 5);
        assert_eq!(right.trim_start_frame, c.trim_start_frame + 3);
    }

    // --- Fade handling ---

    #[test]
    fn left_keeps_fade_in_right_keeps_fade_out() {
        let mut c = base_clip();
        c.fade_in_frames = 4;
        c.fade_out_frames = 6;
        let (left, right) = split_clip(&c, 115, "r").unwrap();
        // Left keeps the head fade, drops the tail.
        assert_eq!(left.fade_in_frames, 4);
        assert_eq!(left.fade_out_frames, 0);
        // Right keeps the tail fade, drops the head.
        assert_eq!(right.fade_in_frames, 0);
        assert_eq!(right.fade_out_frames, 6);
    }

    #[test]
    fn fades_are_clamped_to_each_half_duration() {
        let mut c = base_clip();
        c.fade_in_frames = 20; // longer than the left half will be
        c.fade_out_frames = 25; // longer than the right half will be
        let (left, right) = split_clip(&c, 110, "r").unwrap(); // left dur 10, right dur 20
        assert_eq!(left.fade_in_frames, 10); // clamped to left duration
        assert_eq!(right.fade_out_frames, 20); // clamped to right duration
    }

    // --- Id assignment ---

    #[test]
    fn left_keeps_id_right_gets_supplied_id() {
        let c = base_clip();
        let (left, right) = split_clip(&c, 115, "right-uuid").unwrap();
        assert_eq!(left.id, "orig");
        assert_eq!(right.id, "right-uuid");
    }

    // --- Keyframe continuity across the cut ---

    #[test]
    fn opacity_track_split_inserts_boundary_and_rebases_right() {
        let mut c = base_clip();
        c.opacity_track = Some(KeyframeTrack::from_keyframes(vec![
            Keyframe::with_interpolation(0, 0.0, Interpolation::Linear),
            Keyframe::with_interpolation(10, 1.0, Interpolation::Linear),
        ]));
        // Cut at offset 5 (at_frame 105). Boundary value = linear sample = 0.5.
        let (left, right) = split_clip(&c, 105, "r").unwrap();
        let lt = left.opacity_track.unwrap();
        let rt = right.opacity_track.unwrap();
        assert_eq!(
            lt.keyframes.iter().map(|k| k.frame).collect::<Vec<_>>(),
            [0, 5]
        );
        approx(lt.keyframes[1].value, 0.5);
        // Right is rebased to 0 with a boundary keyframe at the seam.
        assert_eq!(
            rt.keyframes.iter().map(|k| k.frame).collect::<Vec<_>>(),
            [0, 5]
        );
        approx(rt.keyframes[0].value, 0.5);
        approx(rt.keyframes[1].value, 1.0);
    }

    #[test]
    fn untracked_properties_stay_none_after_split() {
        let c = base_clip(); // no tracks set
        let (left, right) = split_clip(&c, 115, "r").unwrap();
        assert!(left.opacity_track.is_none() && right.opacity_track.is_none());
        assert!(left.position_track.is_none() && right.position_track.is_none());
        assert!(left.scale_track.is_none() && right.scale_track.is_none());
        assert!(left.rotation_track.is_none() && right.rotation_track.is_none());
        assert!(left.crop_track.is_none() && right.crop_track.is_none());
        assert!(left.volume_track.is_none() && right.volume_track.is_none());
    }

    #[test]
    fn position_track_uses_zero_fallback_at_seam() {
        let mut c = base_clip();
        // A single keyframe far from the cut -> the boundary is sampled, and an
        // empty side falls back to AnimPair(0,0) only when no keyframe exists.
        // Here the cut is before the lone keyframe, so the left boundary samples
        // the single value (clamped), and the right rebases it.
        c.position_track = Some(KeyframeTrack::from_keyframes(vec![Keyframe::new(
            20,
            AnimPair::new(0.3, 0.7),
        )]));
        let (left, right) = split_clip(&c, 110, "r").unwrap(); // offset 10
        let lt = left.position_track.unwrap();
        let rt = right.position_track.unwrap();
        // Left: keep frames <= 10 (none) then boundary at 10 sampled from the
        // single clamped keyframe value (0.3, 0.7).
        assert_eq!(
            lt.keyframes.iter().map(|k| k.frame).collect::<Vec<_>>(),
            [10]
        );
        approx(lt.keyframes[0].value.a, 0.3);
        // Right: original frame 20 rebased to 10, plus a boundary at 0.
        assert_eq!(
            rt.keyframes.iter().map(|k| k.frame).collect::<Vec<_>>(),
            [0, 10]
        );
        approx(rt.keyframes[1].value.b, 0.7);
    }
}
