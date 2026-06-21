//! Track-level structural ops: zone partition, insert/remove/prune, audio-track
//! resolution. 1:1 port of the pure parts of `EditorViewModel+Tracks.swift` and
//! the zone helpers from `EditorViewModel+Linking.swift`.
//!
//! These mutate the timeline directly with no undo registration — the
//! [`crate::command`] transaction snapshots/commits around them.

use opentake_domain::{ClipType, Timeline, Track};

use crate::id::IdGen;

/// Video/audio zone partition. Visual (video/image/text/lottie) tracks occupy
/// `[0, first_audio_index)`; audio tracks occupy `[first_audio_index, count)`.
/// 1:1 port of `ZoneLayout` + `zones`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ZoneLayout {
    pub track_count: usize,
    pub first_audio_index: usize,
}

impl ZoneLayout {
    pub fn video_track_count(&self) -> usize {
        self.first_audio_index
    }
    pub fn audio_track_count(&self) -> usize {
        self.track_count - self.first_audio_index
    }
}

/// Compute the video/audio zone partition for a timeline.
pub fn zones(timeline: &Timeline) -> ZoneLayout {
    let count = timeline.tracks.len();
    let first_audio = timeline
        .tracks
        .iter()
        .position(|t| t.kind == ClipType::Audio)
        .unwrap_or(count);
    ZoneLayout {
        track_count: count,
        first_audio_index: first_audio,
    }
}

/// Clamp `requested` so visual tracks always sit above every audio track. 1:1
/// port of `partitionedInsertionIndex(for:requested:)`.
fn partitioned_insertion_index(timeline: &Timeline, kind: ClipType, requested: usize) -> usize {
    let z = zones(timeline);
    let bounded = requested.min(z.track_count);
    match kind {
        ClipType::Audio => bounded.max(z.first_audio_index),
        // Visual kinds must come at or before the first audio track.
        _ => bounded.min(z.first_audio_index),
    }
}

/// Insert a new empty track of `kind` near `index` (clamped into its zone),
/// minting an id. Returns the resolved insertion index. 1:1 port of
/// `insertTrack(at:type:)` (minus undo registration).
pub fn insert_track(
    timeline: &mut Timeline,
    index: usize,
    kind: ClipType,
    ids: &dyn IdGen,
) -> usize {
    let clamped = partitioned_insertion_index(timeline, kind, index);
    let track = Track::new(ids.next_id(), kind);
    timeline.tracks.insert(clamped, track);
    clamped
}

/// Remove every track whose id is in `ids`. Returns the count removed. 1:1 port
/// of `removeTracks(ids:)` (minus undo registration / change guard).
pub fn remove_tracks(timeline: &mut Timeline, ids: &[String]) -> usize {
    let before = timeline.tracks.len();
    timeline.tracks.retain(|t| !ids.contains(&t.id));
    before - timeline.tracks.len()
}

/// Drop all empty tracks. 1:1 port of `pruneEmptyTracks`.
pub fn prune_empty_tracks(timeline: &mut Timeline) {
    timeline.tracks.retain(|t| !t.clips.is_empty());
}

/// First audio track free over `[start_frame, start_frame + duration)`, else
/// `None`. 1:1 port of `availableAudioTrackIndex(startFrame:duration:)`.
pub fn available_audio_track_index(
    timeline: &Timeline,
    start_frame: i32,
    duration: i32,
) -> Option<usize> {
    let z = zones(timeline);
    for i in z.first_audio_index..z.track_count {
        let track = &timeline.tracks[i];
        let conflicts = track
            .clips
            .iter()
            .any(|c| !(c.end_frame() <= start_frame || c.start_frame >= start_frame + duration));
        if !conflicts {
            return Some(i);
        }
    }
    None
}

/// A free audio track for `[start_frame, start_frame + duration)`, creating one
/// at the bottom if none is free. 1:1 port of
/// `resolveOrCreateAudioTrack(startFrame:duration:)`.
pub fn resolve_or_create_audio_track(
    timeline: &mut Timeline,
    start_frame: i32,
    duration: i32,
    ids: &dyn IdGen,
) -> usize {
    if let Some(i) = available_audio_track_index(timeline, start_frame, duration) {
        return i;
    }
    insert_track(timeline, timeline.tracks.len(), ClipType::Audio, ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::SeqIdGen;
    use opentake_domain::Clip;

    fn tl_v_a() -> Timeline {
        let mut tl = Timeline::new();
        tl.tracks.push(Track::new("v1", ClipType::Video));
        tl.tracks.push(Track::new("a1", ClipType::Audio));
        tl
    }

    #[test]
    fn zones_partition() {
        let tl = tl_v_a();
        let z = zones(&tl);
        assert_eq!(z.track_count, 2);
        assert_eq!(z.first_audio_index, 1);
        assert_eq!(z.video_track_count(), 1);
        assert_eq!(z.audio_track_count(), 1);
    }

    #[test]
    fn insert_video_clamps_above_audio() {
        let mut tl = tl_v_a();
        let g = SeqIdGen::default();
        // request index 5 for a video track -> clamps to first_audio_index (1).
        let idx = insert_track(&mut tl, 5, ClipType::Video, &g);
        assert_eq!(idx, 1);
        assert_eq!(tl.tracks[1].kind, ClipType::Video);
    }

    #[test]
    fn insert_audio_clamps_below_visual() {
        let mut tl = tl_v_a();
        let g = SeqIdGen::default();
        // request index 0 for an audio track -> clamps to first_audio_index (1).
        let idx = insert_track(&mut tl, 0, ClipType::Audio, &g);
        assert_eq!(idx, 1);
        assert_eq!(tl.tracks[1].kind, ClipType::Audio);
    }

    #[test]
    fn prune_removes_empty_tracks() {
        let mut tl = tl_v_a();
        tl.tracks[0].clips.push(Clip::new("c", "a", 0, 10));
        prune_empty_tracks(&mut tl);
        assert_eq!(tl.tracks.len(), 1);
        assert_eq!(tl.tracks[0].id, "v1");
    }

    #[test]
    fn available_audio_skips_conflicting() {
        let mut tl = tl_v_a();
        tl.tracks[1].clips.push(Clip::new("c", "a", 0, 100)); // a1 busy [0,100)
        assert_eq!(available_audio_track_index(&tl, 50, 10), None);
        assert_eq!(available_audio_track_index(&tl, 200, 10), Some(1)); // free after
    }

    #[test]
    fn resolve_creates_audio_when_none_free() {
        let mut tl = tl_v_a();
        tl.tracks[1].clips.push(Clip::new("c", "a", 0, 100));
        let g = SeqIdGen::default();
        let idx = resolve_or_create_audio_track(&mut tl, 50, 10, &g);
        assert_eq!(idx, 2); // appended a new audio track at the bottom
        assert_eq!(tl.tracks[2].kind, ClipType::Audio);
    }

    #[test]
    fn remove_tracks_by_id() {
        let mut tl = tl_v_a();
        let n = remove_tracks(&mut tl, &["a1".to_string()]);
        assert_eq!(n, 1);
        assert_eq!(tl.tracks.len(), 1);
    }
}
