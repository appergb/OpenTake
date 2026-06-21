//! Clip placement and the small track helpers it leans on. Ports `placeClip` /
//! `createClips` / `sortClips` from `EditorViewModel.swift`, frame-based.
//!
//! Difference from upstream: upstream `placeClip` derives the visual transform
//! from the asset's source dimensions (`fitTransform`). That needs media
//! metadata, which is a media-layer concern — this leaf crate never resolves
//! media. So callers pass a fully-formed [`PlaceSpec`] (the agent `add_clips`
//! tool already supplies explicit `durationFrames` / `trim*`), and the placed
//! clip's transform stays `Transform::default()`; a higher layer with asset
//! metadata can fit it. The link / audio-routing / sort behavior is preserved.

use opentake_domain::{Clip, ClipType, Timeline, Transform};

use crate::id::IdGen;
use crate::ops::tracks::resolve_or_create_audio_track;

/// Everything needed to place one clip. `media_type` is the type on the placed
/// (visual) clip; a linked audio partner is created when
/// `add_linked_audio && target is a video track && source_clip_type == video &&
/// has_audio` (mirrors `placeClip`'s `shouldLink`).
#[derive(Clone, Debug)]
pub struct PlaceSpec {
    pub media_ref: String,
    pub media_type: ClipType,
    pub source_clip_type: ClipType,
    pub start_frame: i32,
    pub duration_frames: i32,
    pub trim_start_frame: Option<i32>,
    pub trim_end_frame: Option<i32>,
    /// Source asset carries an audio stream (drives the linked-audio partner).
    pub has_audio: bool,
    /// Whether to auto-create a linked audio partner for video-with-audio.
    pub add_linked_audio: bool,
}

impl PlaceSpec {
    /// Minimal spec: a clip of `media_type` at `start_frame` for `duration_frames`
    /// with no trims and no linked audio.
    pub fn new(
        media_ref: impl Into<String>,
        media_type: ClipType,
        start_frame: i32,
        duration_frames: i32,
    ) -> Self {
        PlaceSpec {
            media_ref: media_ref.into(),
            media_type,
            source_clip_type: media_type,
            start_frame,
            duration_frames,
            trim_start_frame: None,
            trim_end_frame: None,
            has_audio: false,
            add_linked_audio: false,
        }
    }
}

/// Sort a track's clips ascending by `start_frame`. 1:1 port of `sortClips`.
pub fn sort_clips(track: &mut opentake_domain::Track) {
    track.clips.sort_by_key(|c| c.start_frame);
}

/// Place one clip, optionally with linked audio. Returns the created clip ids
/// (`[clip]` or `[clip, audio]`). Empty if `track_index` is out of range. 1:1
/// port of `placeClip(...)`, frame-based (see module note on transform).
pub fn place_clip(
    timeline: &mut Timeline,
    spec: &PlaceSpec,
    track_index: usize,
    linked_audio_track_index: Option<usize>,
    ids: &dyn IdGen,
) -> Vec<String> {
    if track_index >= timeline.tracks.len() {
        return Vec::new();
    }
    let target_is_video = timeline.tracks[track_index].kind == ClipType::Video;
    let should_link = spec.add_linked_audio
        && target_is_video
        && spec.source_clip_type == ClipType::Video
        && spec.has_audio;
    let link_group_id: Option<String> = if should_link {
        Some(ids.next_id())
    } else {
        None
    };

    let mut clip = Clip::new(
        ids.next_id(),
        spec.media_ref.clone(),
        spec.start_frame,
        spec.duration_frames,
    );
    clip.media_type = spec.media_type;
    clip.source_clip_type = spec.source_clip_type;
    clip.transform = Transform::default();
    clip.link_group_id = link_group_id.clone();
    if let Some(t) = spec.trim_start_frame {
        clip.trim_start_frame = t;
    }
    if let Some(t) = spec.trim_end_frame {
        clip.trim_end_frame = t;
    }
    let clip_id = clip.id.clone();
    timeline.tracks[track_index].clips.push(clip);
    sort_clips(&mut timeline.tracks[track_index]);

    let mut out = vec![clip_id];

    if let Some(gid) = link_group_id {
        let audio_idx = linked_audio_track_index
            .filter(|&i| i < timeline.tracks.len())
            .unwrap_or_else(|| {
                resolve_or_create_audio_track(timeline, spec.start_frame, spec.duration_frames, ids)
            });
        if audio_idx >= timeline.tracks.len() {
            return out;
        }
        let mut audio = Clip::new(
            ids.next_id(),
            spec.media_ref.clone(),
            spec.start_frame,
            spec.duration_frames,
        );
        audio.media_type = ClipType::Audio;
        audio.source_clip_type = spec.source_clip_type;
        audio.link_group_id = Some(gid);
        if let Some(t) = spec.trim_start_frame {
            audio.trim_start_frame = t;
        }
        if let Some(t) = spec.trim_end_frame {
            audio.trim_end_frame = t;
        }
        let audio_id = audio.id.clone();
        timeline.tracks[audio_idx].clips.push(audio);
        sort_clips(&mut timeline.tracks[audio_idx]);
        out.push(audio_id);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::SeqIdGen;
    use opentake_domain::Track;

    fn tl_v_a() -> Timeline {
        let mut tl = Timeline::new();
        tl.tracks.push(Track::new("v", ClipType::Video));
        tl.tracks.push(Track::new("a", ClipType::Audio));
        tl
    }

    #[test]
    fn place_clip_appends_and_sorts() {
        let mut tl = tl_v_a();
        tl.tracks[0].clips.push(Clip::new("existing", "x", 100, 30));
        let g = SeqIdGen::new("n-");
        let spec = PlaceSpec::new("m", ClipType::Video, 0, 30);
        let out = place_clip(&mut tl, &spec, 0, None, &g);
        assert_eq!(out, vec!["n-1".to_string()]);
        // sorted: new clip (start 0) before existing (start 100).
        assert_eq!(tl.tracks[0].clips[0].id, "n-1");
        assert_eq!(tl.tracks[0].clips[1].id, "existing");
    }

    #[test]
    fn place_clip_applies_trims() {
        let mut tl = tl_v_a();
        let g = SeqIdGen::default();
        let mut spec = PlaceSpec::new("m", ClipType::Video, 0, 30);
        spec.trim_start_frame = Some(5);
        spec.trim_end_frame = Some(7);
        place_clip(&mut tl, &spec, 0, None, &g);
        let c = &tl.tracks[0].clips[0];
        assert_eq!(c.trim_start_frame, 5);
        assert_eq!(c.trim_end_frame, 7);
    }

    #[test]
    fn place_video_with_audio_creates_linked_partner() {
        let mut tl = tl_v_a();
        let g = SeqIdGen::new("n-");
        let mut spec = PlaceSpec::new("m", ClipType::Video, 0, 30);
        spec.has_audio = true;
        spec.add_linked_audio = true;
        let out = place_clip(&mut tl, &spec, 0, None, &g);
        assert_eq!(out.len(), 2);
        // ids: group(n-1), video(n-2), audio(n-3).
        let video = tl.tracks[0].clips.iter().find(|c| c.id == out[0]).unwrap();
        let audio = tl.tracks[1].clips.iter().find(|c| c.id == out[1]).unwrap();
        assert_eq!(video.media_type, ClipType::Video);
        assert_eq!(audio.media_type, ClipType::Audio);
        assert_eq!(video.link_group_id, audio.link_group_id);
        assert!(video.link_group_id.is_some());
    }

    #[test]
    fn place_no_link_when_target_is_audio_track() {
        let mut tl = tl_v_a();
        let g = SeqIdGen::default();
        let mut spec = PlaceSpec::new("m", ClipType::Audio, 0, 30);
        spec.has_audio = true;
        spec.add_linked_audio = true;
        // placing onto audio track (index 1): no video target -> no link.
        let out = place_clip(&mut tl, &spec, 1, None, &g);
        assert_eq!(out.len(), 1);
        assert!(tl.tracks[1].clips[0].link_group_id.is_none());
    }

    #[test]
    fn place_out_of_range_track_is_noop() {
        let mut tl = tl_v_a();
        let g = SeqIdGen::default();
        let spec = PlaceSpec::new("m", ClipType::Video, 0, 30);
        assert!(place_clip(&mut tl, &spec, 9, None, &g).is_empty());
    }
}
