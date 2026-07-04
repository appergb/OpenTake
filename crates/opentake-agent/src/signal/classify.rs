//! Video-type auto-detection (`agent-SPEC.md` §6.3, `AGENT-CONTEXT-SIGNAL.md`
//! §2.1). Structural inference from the `Timeline`; semantically-heavy features
//! (continuous human voice, first-person metadata, chapter markers) need
//! `opentake-media` and are approximated structurally for the MVP.
//!
//! Priority (caller-applied, `agent-SPEC.md` §6.3): plugin video_type > manual
//! project setting > this auto-classify > default.

use opentake_domain::{ClipType, Timeline, VideoType};

/// Auto-classify a timeline into a `(VideoType, confidence)` from structural
/// features. Confidence values match the doc's table.
pub fn classify(timeline: &Timeline) -> (VideoType, f64) {
    let fps = timeline.fps.max(1) as f64;
    let video_tracks = timeline
        .tracks
        .iter()
        .filter(|t| t.kind == ClipType::Video)
        .count();
    let audio_tracks = timeline
        .tracks
        .iter()
        .filter(|t| t.kind == ClipType::Audio)
        .count();
    let portrait = timeline.width < timeline.height;

    let text_clip_count: usize = timeline
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .filter(|c| c.media_type == ClipType::Text)
        .count();

    let video_clips: Vec<&opentake_domain::Clip> = timeline
        .tracks
        .iter()
        .filter(|t| t.kind == ClipType::Video)
        .flat_map(|t| t.clips.iter())
        .filter(|c| c.media_type != ClipType::Text)
        .collect();
    let total_video_clips = video_clips.len();
    let short_clips = video_clips
        .iter()
        .filter(|c| (c.duration_frames as f64 / fps) < 3.0)
        .count();
    let all_short = total_video_clips > 0 && short_clips == total_video_clips;

    // Same-timestamp multi-cam: two+ video tracks with clips that share a
    // start frame (a structural proxy for synced cameras).
    let multicam = video_tracks >= 2 && has_simultaneous_starts(timeline);

    let total_seconds = timeline.total_frames() as f64 / fps;

    // Rules in the doc's order (most specific first).
    if multicam {
        return (VideoType::Interview, 0.9);
    }
    if (1..=2).contains(&video_tracks) && audio_tracks >= 1 && has_long_audio(timeline, fps) {
        return (VideoType::TalkingHead, 0.9);
    }
    if portrait && text_clip_count >= 3 {
        return (VideoType::ShortForm, 0.85);
    }
    if video_tracks >= 2 && all_short && audio_tracks >= 1 {
        return (VideoType::Montage, 0.85);
    }
    if total_video_clips >= 8 && all_short {
        return (VideoType::Vlog, 0.8);
    }
    if total_seconds > 600.0 {
        return (VideoType::LongForm, 0.8);
    }
    // Default fallback: treat single-track-with-audio as talking head at low
    // confidence (the most common editing shape).
    (VideoType::TalkingHead, 0.5)
}

/// Whether any audio track has a long continuous clip (≥ 10s) — a structural
/// proxy for "long stretch of human voice".
fn has_long_audio(timeline: &Timeline, fps: f64) -> bool {
    timeline
        .tracks
        .iter()
        .filter(|t| t.kind == ClipType::Audio)
        .flat_map(|t| t.clips.iter())
        .any(|c| (c.duration_frames as f64 / fps) >= 10.0)
}

/// Whether clips on two different video tracks share a start frame.
fn has_simultaneous_starts(timeline: &Timeline) -> bool {
    let video: Vec<&opentake_domain::Track> = timeline
        .tracks
        .iter()
        .filter(|t| t.kind == ClipType::Video)
        .collect();
    for i in 0..video.len() {
        for j in (i + 1)..video.len() {
            for a in &video[i].clips {
                if video[j]
                    .clips
                    .iter()
                    .any(|b| b.start_frame == a.start_frame)
                {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{Clip, ClipType, Track};

    fn clip(id: &str, start: i32, dur: i32) -> Clip {
        Clip::new(id, "asset", start, dur)
    }

    #[test]
    fn talking_head_one_video_long_audio() {
        let mut tl = Timeline::new(); // fps 30, landscape
        let mut v = Track::new("v1", ClipType::Video);
        v.clips.push(clip("a", 0, 30 * 30));
        let mut a = Track::new("a1", ClipType::Audio);
        a.clips.push(clip("au", 0, 30 * 30)); // 30s voice
        tl.tracks.push(v);
        tl.tracks.push(a);
        let (vt, conf) = classify(&tl);
        assert_eq!(vt, VideoType::TalkingHead);
        assert_eq!(conf, 0.9);
    }

    #[test]
    fn montage_many_short_clips_multi_track_with_music() {
        let mut tl = Timeline::new();
        let mut v1 = Track::new("v1", ClipType::Video);
        let mut v2 = Track::new("v2", ClipType::Video);
        for i in 0..4 {
            v1.clips.push(clip(&format!("a{i}"), i * 60, 60)); // 2s each
            v2.clips.push(clip(&format!("b{i}"), 1000 + i * 60, 60));
        }
        let mut music = Track::new("a1", ClipType::Audio);
        music.clips.push(clip("m", 0, 30 * 5)); // 5s (not long-voice)
        tl.tracks.push(v1);
        tl.tracks.push(v2);
        tl.tracks.push(music);
        let (vt, _) = classify(&tl);
        assert_eq!(vt, VideoType::Montage);
    }

    #[test]
    fn short_form_portrait_with_text() {
        let mut tl = Timeline::new();
        tl.width = 1080;
        tl.height = 1920; // portrait
        let mut v = Track::new("v1", ClipType::Video);
        v.clips.push(clip("a", 0, 90));
        for i in 0..3 {
            let mut t = clip(&format!("t{i}"), i * 30, 30);
            t.media_type = ClipType::Text;
            v.clips.push(t);
        }
        tl.tracks.push(v);
        let (vt, _) = classify(&tl);
        assert_eq!(vt, VideoType::ShortForm);
    }

    #[test]
    fn interview_simultaneous_multicam() {
        let mut tl = Timeline::new();
        let mut v1 = Track::new("v1", ClipType::Video);
        let mut v2 = Track::new("v2", ClipType::Video);
        v1.clips.push(clip("a", 0, 30 * 20));
        v2.clips.push(clip("b", 0, 30 * 20)); // same start frame -> multicam
        tl.tracks.push(v1);
        tl.tracks.push(v2);
        let (vt, conf) = classify(&tl);
        assert_eq!(vt, VideoType::Interview);
        assert_eq!(conf, 0.9);
    }

    #[test]
    fn long_form_over_ten_minutes() {
        let mut tl = Timeline::new();
        let mut v = Track::new("v1", ClipType::Video);
        // 11 minutes of one clip, no separate audio track -> LongForm.
        v.clips.push(clip("a", 0, 30 * 60 * 11));
        tl.tracks.push(v);
        let (vt, _) = classify(&tl);
        assert_eq!(vt, VideoType::LongForm);
    }

    #[test]
    fn empty_timeline_defaults_low_confidence() {
        let tl = Timeline::new();
        let (vt, conf) = classify(&tl);
        assert_eq!(vt, VideoType::TalkingHead);
        assert!(conf < 0.9);
    }
}
