//! Track swap op. OpenTake-only extension: exchange two whole tracks without
//! applying clip overwrite semantics.

use opentake_domain::Timeline;

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
}
