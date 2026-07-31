//! Project timeline-settings op: change FPS / resolution. Port of upstream
//! `EditorViewModel.applyTimelineSettings(fps:width:height:)`
//! (Editor/ViewModel/EditorViewModel+ProjectSettings.swift:19-77), stripped of
//! AppKit/undo glue — the command layer snapshots/commits around it.
//!
//! Ported faithfully:
//!  - When FPS changes, every clip's frame-based values are rescaled by
//!    `newFps/oldFps` (start/duration/trim/keyframes/fades), per track in
//!    start-order with upstream's `previousEnd` no-overlap guard, then the
//!    keyframe/fade clamps re-run.
//!  - `fps`, `width`, `height`, `settings_configured = true` are set last.
//!
//! Deliberately NOT ported here (both need state the ops layer doesn't hold):
//!  - `currentFrame` / `sourcePlayheadFrame` rescale — those are UI-owned in
//!    OpenTake (`uiStore`), not on the `Timeline`; the frontend rescales the
//!    playhead itself when it changes FPS.
//!  - The "refit auto-fitted clips to the new canvas aspect" pass
//!    (upstream :55-65) — it compares each clip's transform against
//!    `fitTransform(for: asset, ...)`, which requires per-asset source
//!    dimensions. `EditorState` holds only the `Timeline` (no media manifest),
//!    so this is left to a later pass once asset dims are threaded in. Skipping
//!    it is safe: clips simply keep their explicit transform across a
//!    resolution change (they are not silently re-fitted).

use opentake_domain::Timeline;

/// Apply new project settings to `timeline`. Returns `true` when anything
/// changed (the command layer's snapshot/commit also re-checks, so a no-op call
/// produces no undo entry). Invalid (`<= 0`) `fps`/`width`/`height` are rejected
/// as a no-op to keep the timeline well-formed.
pub fn set_timeline_settings(timeline: &mut Timeline, fps: i32, width: i32, height: i32) -> bool {
    if fps <= 0 || width <= 0 || height <= 0 {
        return false;
    }

    // Nested timelines share one project timebase and output canvas. Keep every
    // stored child synchronized (including frame/keyframe rescaling) so entering
    // a compound never exposes stale settings after the root changes.
    let mut nested_changed = false;
    for sequence in &mut timeline.nested_sequences {
        nested_changed |= set_timeline_settings(&mut sequence.timeline, fps, width, height);
    }

    let prev_fps = timeline.fps;
    let prev_width = timeline.width;
    let prev_height = timeline.height;
    let prev_configured = timeline.settings_configured;

    if fps == prev_fps && width == prev_width && height == prev_height && prev_configured {
        return nested_changed;
    }

    // Rescale all frame-based values when FPS changes (upstream :26-52).
    if fps != prev_fps && prev_fps > 0 {
        let scale = fps as f64 / prev_fps as f64;
        for track in &mut timeline.tracks {
            // Process clips in start order so `previous_end` tracks correctly
            // (upstream sorts indices by startFrame before rescaling).
            let mut order: Vec<usize> = (0..track.clips.len()).collect();
            order.sort_by_key(|&i| track.clips[i].start_frame);
            let mut previous_end: Option<i32> = None;
            for i in order {
                let clip = &mut track.clips[i];
                let scaled_start = round_scale(clip.start_frame, scale);
                let scaled_end = round_scale(clip.end_frame(), scale);
                clip.start_frame = scaled_start.max(previous_end.unwrap_or(scaled_start));
                clip.duration_frames = (scaled_end - clip.start_frame).max(1);
                clip.trim_start_frame = round_scale(clip.trim_start_frame, scale);
                clip.trim_end_frame = round_scale(clip.trim_end_frame, scale);
                clip.rescale_keyframes(scale);
                clip.fade_in_frames = round_scale(clip.fade_in_frames, scale);
                clip.fade_out_frames = round_scale(clip.fade_out_frames, scale);
                if let Some(transition) = &mut clip.transition_out {
                    transition.duration_frames =
                        round_scale(transition.duration_frames, scale).max(1);
                }
                clip.clamp_keyframes_to_duration();
                clip.clamp_fades_to_duration();
                previous_end = Some(clip.end_frame());
            }
        }
    }

    timeline.fps = fps;
    timeline.width = width;
    timeline.height = height;
    timeline.settings_configured = true;
    true
}

/// `round(value * scale)` with Swift `.rounded()` semantics (half away from
/// zero == Rust `f64::round`), matching the domain's port convention.
fn round_scale(value: i32, scale: f64) -> i32 {
    (value as f64 * scale).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{
        Clip, ClipType, Keyframe, KeyframeTrack, Track, Transition, TransitionKind,
    };

    fn track(id: &str, kind: ClipType, clips: Vec<Clip>) -> Track {
        let mut track = Track::new(id, kind);
        track.clips = clips;
        track
    }

    fn clip(id: &str, start: i32, duration: i32) -> Clip {
        Clip::new(id, "m", start, duration)
    }

    #[test]
    fn sets_dims_and_marks_configured() {
        let mut tl = Timeline::new(); // 30fps 1920x1080, settings_configured=false
        assert!(set_timeline_settings(&mut tl, 30, 1080, 1920));
        assert_eq!((tl.width, tl.height), (1080, 1920));
        assert_eq!(tl.fps, 30);
        assert!(tl.settings_configured);
    }

    #[test]
    fn resolution_change_leaves_clip_frames_untouched() {
        let mut tl = Timeline::new();
        tl.tracks
            .push(track("v", ClipType::Video, vec![clip("x", 10, 40)]));
        // Same fps, only resolution changes → no frame rescale.
        assert!(set_timeline_settings(&mut tl, 30, 3840, 2160));
        let c = &tl.tracks[0].clips[0];
        assert_eq!((c.start_frame, c.duration_frames), (10, 40));
    }

    #[test]
    fn settings_change_rescales_registered_nested_timelines() {
        use opentake_domain::NestedSequence;

        let mut child = Timeline::new();
        child.tracks.push(track(
            "child",
            ClipType::Video,
            vec![clip("nested", 15, 30)],
        ));
        let mut root = Timeline::new();
        root.nested_sequences
            .push(NestedSequence::new("sequence", "Scene", child));

        assert!(set_timeline_settings(&mut root, 60, 1280, 720));

        let child = &root.nested_sequences[0].timeline;
        assert_eq!((child.fps, child.width, child.height), (60, 1280, 720));
        assert!(child.settings_configured);
        assert_eq!(child.tracks[0].clips[0].start_frame, 30);
        assert_eq!(child.tracks[0].clips[0].duration_frames, 60);
    }

    #[test]
    fn fps_doubling_scales_clip_start_and_duration() {
        let mut tl = Timeline::new();
        tl.tracks
            .push(track("v", ClipType::Video, vec![clip("x", 10, 40)]));
        // 30 → 60 fps, scale 2: start 10→20, end 50→100, duration 100-20=80.
        assert!(set_timeline_settings(&mut tl, 60, 1920, 1080));
        let c = &tl.tracks[0].clips[0];
        assert_eq!(c.start_frame, 20);
        assert_eq!(c.duration_frames, 80);
        assert_eq!(tl.fps, 60);
    }

    #[test]
    fn fps_change_scales_trim_and_fades() {
        let mut tl = Timeline::new();
        let mut c = clip("x", 0, 100);
        c.trim_start_frame = 10;
        c.trim_end_frame = 20;
        c.fade_in_frames = 8;
        c.fade_out_frames = 12;
        tl.tracks.push(track("v", ClipType::Video, vec![c]));
        assert!(set_timeline_settings(&mut tl, 60, 1920, 1080)); // scale 2
        let c = &tl.tracks[0].clips[0];
        assert_eq!(c.trim_start_frame, 20);
        assert_eq!(c.trim_end_frame, 40);
        assert_eq!(c.fade_in_frames, 16);
        assert_eq!(c.fade_out_frames, 24);
    }

    #[test]
    fn fps_change_scales_transition_duration() {
        let mut tl = Timeline::new();
        let mut a = clip("a", 0, 60);
        a.transition_out = Some(Transition {
            to_clip_id: "b".into(),
            kind: TransitionKind::CrossDissolve,
            duration_frames: 15,
        });
        tl.tracks
            .push(track("v", ClipType::Video, vec![a, clip("b", 60, 60)]));
        assert!(set_timeline_settings(&mut tl, 60, 1920, 1080));
        assert_eq!(
            tl.tracks[0].clips[0]
                .transition_out
                .as_ref()
                .unwrap()
                .duration_frames,
            30
        );
    }

    #[test]
    fn fps_change_rescales_keyframe_offsets() {
        let mut tl = Timeline::new();
        let mut c = clip("x", 0, 100);
        c.opacity_track = Some(KeyframeTrack {
            keyframes: vec![Keyframe::new(0, 0.0), Keyframe::new(20, 1.0)],
        });
        tl.tracks.push(track("v", ClipType::Video, vec![c]));
        assert!(set_timeline_settings(&mut tl, 60, 1920, 1080)); // scale 2
        let kfs = &tl.tracks[0].clips[0]
            .opacity_track
            .as_ref()
            .unwrap()
            .keyframes;
        assert_eq!(kfs[0].frame, 0);
        assert_eq!(kfs[1].frame, 40);
    }

    #[test]
    fn fps_halving_preserves_no_overlap_between_adjacent_clips() {
        // Two back-to-back clips at 60fps; halving to 30 rounds both starts, and
        // the previous_end guard keeps the second from landing before the first ends.
        let mut tl = Timeline::new();
        tl.fps = 60;
        tl.settings_configured = true;
        tl.tracks.push(track(
            "v",
            ClipType::Video,
            vec![clip("a", 0, 3), clip("b", 3, 3)],
        ));
        assert!(set_timeline_settings(&mut tl, 30, 1920, 1080)); // scale 0.5
        let a = &tl.tracks[0].clips[0];
        let b = &tl.tracks[0].clips[1];
        // a: start 0, end round(3*0.5)=2 → duration max(2,1)=2.
        assert_eq!((a.start_frame, a.duration_frames), (0, 2));
        // b: scaled_start round(3*0.5)=2 == a.end → no overlap; end round(6*0.5)=3
        // → duration max(3-2,1)=1.
        assert_eq!(b.start_frame, 2);
        assert!(b.start_frame >= a.end_frame());
    }

    #[test]
    fn rejects_nonpositive_settings_as_noop() {
        let mut tl = Timeline::new();
        let before = tl.clone();
        assert!(!set_timeline_settings(&mut tl, 0, 1920, 1080));
        assert!(!set_timeline_settings(&mut tl, 30, 0, 1080));
        assert!(!set_timeline_settings(&mut tl, 30, 1920, -1));
        assert_eq!(tl, before);
    }

    #[test]
    fn identical_configured_settings_report_unchanged() {
        let mut tl = Timeline::new();
        tl.settings_configured = true; // 30/1920/1080 already
        assert!(!set_timeline_settings(&mut tl, 30, 1920, 1080));
    }

    #[test]
    fn first_configure_with_default_dims_still_changes() {
        // Fresh timeline is 30/1920/1080 but settings_configured=false; applying
        // the same dims must still "change" so it flips the configured flag.
        let mut tl = Timeline::new();
        assert!(!tl.settings_configured);
        assert!(set_timeline_settings(&mut tl, 30, 1920, 1080));
        assert!(tl.settings_configured);
    }
}
