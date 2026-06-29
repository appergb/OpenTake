//! Swap ops. OpenTake-only extensions: exchange two whole tracks, or exchange
//! the positions of two individual clips — both without the overwrite (clip
//! "swallowing") semantics of a plain move.

use opentake_domain::Timeline;

use crate::ops::clear_region::remove_clip;
use crate::ops::place::sort_clips;

/// Swap two whole tracks. Returns `true` when the timeline changed.
///
/// The video/audio partition invariant is preserved by allowing only same-kind
/// swaps. Invalid indexes, identical indexes, and cross-kind requests are no-ops.
pub fn swap_tracks(timeline: &mut Timeline, a: usize, b: usize) -> bool {
    if a == b || a >= timeline.tracks.len() || b >= timeline.tracks.len() {
        return false;
    }
    if timeline.tracks[a].kind != timeline.tracks[b].kind {
        return false;
    }
    timeline.tracks.swap(a, b);
    true
}

/// Swap the `(track, start_frame)` of two clips — the clip-level "exchange
/// positions" gesture: drag a clip onto another track's clip so the two trade
/// places instead of one overwriting (swallowing) the other. Returns `true`
/// when the timeline changed.
///
/// Lossless by construction: if either clip, placed at the other's slot, would
/// overlap a *different* clip on its destination track (e.g. the two clips have
/// different durations and a neighbour sits in the way), the swap is refused and
/// the timeline is left untouched. Cross-kind requests (a video clip onto an
/// audio track, or vice versa), a missing clip, or `id_a == id_b` are all no-ops.
pub fn swap_clip_positions(timeline: &mut Timeline, id_a: &str, id_b: &str) -> bool {
    if id_a == id_b {
        return false;
    }
    let Some((ta, ca)) = find(timeline, id_a) else {
        return false;
    };
    let Some((tb, cb)) = find(timeline, id_b) else {
        return false;
    };
    let clip_a = timeline.tracks[ta].clips[ca].clone();
    let clip_b = timeline.tracks[tb].clips[cb].clone();
    // Each clip must be allowed on the other's track (same kind, or both visual).
    if !timeline.tracks[tb].kind.is_compatible(clip_a.media_type)
        || !timeline.tracks[ta].kind.is_compatible(clip_b.media_type)
    {
        return false;
    }
    let a_start = clip_a.start_frame;
    let b_start = clip_b.start_frame;
    // Both clips vacate their slots, so they never block each other; only OTHER
    // clips on each destination track can refuse the swap.
    let exclude = [id_a, id_b];
    if !range_free(
        &timeline.tracks[tb],
        b_start,
        b_start + clip_a.duration_frames,
        &exclude,
    ) || !range_free(
        &timeline.tracks[ta],
        a_start,
        a_start + clip_b.duration_frames,
        &exclude,
    ) {
        return false;
    }
    remove_clip(timeline, id_a);
    remove_clip(timeline, id_b);
    let mut moved_a = clip_a;
    moved_a.start_frame = b_start;
    let mut moved_b = clip_b;
    moved_b.start_frame = a_start;
    timeline.tracks[tb].clips.push(moved_a);
    timeline.tracks[ta].clips.push(moved_b);
    sort_clips(&mut timeline.tracks[ta]);
    if ta != tb {
        sort_clips(&mut timeline.tracks[tb]);
    }
    true
}

fn find(timeline: &Timeline, clip_id: &str) -> Option<(usize, usize)> {
    timeline.tracks.iter().enumerate().find_map(|(ti, t)| {
        t.clips
            .iter()
            .position(|c| c.id == clip_id)
            .map(|ci| (ti, ci))
    })
}

/// True when `[start, end)` is free of any clip on `track` whose id isn't in
/// `exclude` (half-open overlap test, matching the timeline's no-overlap rule).
fn range_free(track: &opentake_domain::Track, start: i32, end: i32, exclude: &[&str]) -> bool {
    !track
        .clips
        .iter()
        .any(|c| !exclude.contains(&c.id.as_str()) && c.start_frame < end && c.end_frame() > start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{Clip, ClipType, Track};

    fn track(id: &str, kind: ClipType, clips: Vec<Clip>) -> Track {
        let mut track = Track::new(id, kind);
        track.clips = clips;
        track
    }

    fn clip(id: &str, media_ref: &str, start: i32, duration: i32) -> Clip {
        Clip::new(id, media_ref, start, duration)
    }

    #[test]
    fn swaps_adjacent_video_tracks_without_touching_clips() {
        let mut tl = Timeline::new();
        tl.tracks.push(track(
            "v-top",
            ClipType::Video,
            vec![clip("overlay", "m-overlay", 0, 30)],
        ));
        tl.tracks.push(track(
            "v-bottom",
            ClipType::Video,
            vec![clip("base", "m-base", 10, 40)],
        ));
        tl.tracks.push(track("a1", ClipType::Audio, vec![]));

        assert!(swap_tracks(&mut tl, 0, 1));

        assert_eq!(
            tl.tracks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            ["v-bottom", "v-top", "a1"]
        );
        assert_eq!(tl.tracks[0].clips[0].id, "base");
        assert_eq!(tl.tracks[0].clips[0].start_frame, 10);
        assert_eq!(tl.tracks[0].clips[0].duration_frames, 40);
        assert_eq!(tl.tracks[1].clips[0].id, "overlay");
    }

    #[test]
    fn swaps_non_adjacent_same_kind_tracks() {
        let mut tl = Timeline::new();
        tl.tracks.push(track("v1", ClipType::Video, vec![]));
        tl.tracks.push(track(
            "a-top",
            ClipType::Audio,
            vec![clip("voice", "m-voice", 20, 50)],
        ));
        tl.tracks.push(track("a-mid", ClipType::Audio, vec![]));
        tl.tracks.push(track(
            "a-bottom",
            ClipType::Audio,
            vec![clip("music", "m-music", 0, 120)],
        ));

        assert!(swap_tracks(&mut tl, 1, 3));

        assert_eq!(
            tl.tracks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            ["v1", "a-bottom", "a-mid", "a-top"]
        );
        assert_eq!(tl.tracks[1].clips[0].id, "music");
        assert_eq!(tl.tracks[3].clips[0].id, "voice");
    }

    #[test]
    fn rejects_cross_kind_swap_without_mutating() {
        let mut tl = Timeline::new();
        tl.tracks.push(track(
            "v1",
            ClipType::Video,
            vec![clip("video", "m-video", 0, 30)],
        ));
        tl.tracks.push(track(
            "a1",
            ClipType::Audio,
            vec![clip("audio", "m-audio", 0, 30)],
        ));
        let before = tl.clone();

        assert!(!swap_tracks(&mut tl, 0, 1));
        assert_eq!(tl, before);
    }

    #[test]
    fn video_swap_changes_track_order_used_for_z_order() {
        let mut tl = Timeline::new();
        tl.tracks.push(track(
            "overlay",
            ClipType::Video,
            vec![clip("overlay-clip", "m-overlay", 0, 30)],
        ));
        tl.tracks.push(track(
            "base",
            ClipType::Video,
            vec![clip("base-clip", "m-base", 0, 30)],
        ));

        assert_eq!(
            tl.tracks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            ["overlay", "base"]
        );
        assert!(swap_tracks(&mut tl, 0, 1));
        assert_eq!(
            tl.tracks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            ["base", "overlay"]
        );
    }

    #[test]
    fn swaps_two_single_clips_across_tracks() {
        let mut tl = Timeline::new();
        tl.tracks.push(track(
            "v-top",
            ClipType::Video,
            vec![clip("x", "m-x", 0, 30)],
        ));
        tl.tracks.push(track(
            "v-bot",
            ClipType::Video,
            vec![clip("y", "m-y", 0, 30)],
        ));

        assert!(swap_clip_positions(&mut tl, "x", "y"));

        // x now sits on the bottom track, y on the top — neither swallowed.
        let vtop = tl.tracks.iter().find(|t| t.id == "v-top").unwrap();
        let vbot = tl.tracks.iter().find(|t| t.id == "v-bot").unwrap();
        assert_eq!(vtop.clips.len(), 1);
        assert_eq!(vbot.clips.len(), 1);
        assert!(vtop.clips.iter().any(|c| c.id == "y" && c.start_frame == 0));
        assert!(vbot.clips.iter().any(|c| c.id == "x" && c.start_frame == 0));
    }

    #[test]
    fn swap_exchanges_start_frames_too() {
        let mut tl = Timeline::new();
        tl.tracks
            .push(track("v1", ClipType::Video, vec![clip("x", "m-x", 10, 40)]));
        tl.tracks.push(track(
            "v2",
            ClipType::Video,
            vec![clip("y", "m-y", 100, 20)],
        ));

        assert!(swap_clip_positions(&mut tl, "x", "y"));

        let v1 = tl.tracks.iter().find(|t| t.id == "v1").unwrap();
        let v2 = tl.tracks.iter().find(|t| t.id == "v2").unwrap();
        assert!(v2
            .clips
            .iter()
            .any(|c| c.id == "x" && c.start_frame == 100 && c.duration_frames == 40));
        assert!(v1
            .clips
            .iter()
            .any(|c| c.id == "y" && c.start_frame == 10 && c.duration_frames == 20));
    }

    #[test]
    fn swap_refused_when_it_would_overlap_a_neighbour() {
        let mut tl = Timeline::new();
        tl.tracks
            .push(track("v1", ClipType::Video, vec![clip("x", "m-x", 0, 100)]));
        tl.tracks.push(track(
            "v2",
            ClipType::Video,
            vec![clip("y", "m-y", 0, 20), clip("z", "m-z", 30, 50)],
        ));
        let before = tl.clone();

        // x (dur 100) at v2@0 would cover [0,100), overlapping z [30,80) -> refuse.
        assert!(!swap_clip_positions(&mut tl, "x", "y"));
        assert_eq!(tl, before);
    }

    #[test]
    fn swap_refused_across_kinds() {
        let mut tl = Timeline::new();
        tl.tracks
            .push(track("v", ClipType::Video, vec![clip("x", "m-x", 0, 30)]));
        tl.tracks
            .push(track("a", ClipType::Audio, vec![clip("y", "m-y", 0, 30)]));
        let before = tl.clone();

        assert!(!swap_clip_positions(&mut tl, "x", "y"));
        assert_eq!(tl, before);
    }

    #[test]
    fn swap_missing_clip_is_noop() {
        let mut tl = Timeline::new();
        tl.tracks
            .push(track("v", ClipType::Video, vec![clip("x", "m-x", 0, 30)]));
        let before = tl.clone();

        assert!(!swap_clip_positions(&mut tl, "x", "nope"));
        assert_eq!(tl, before);
    }
}
