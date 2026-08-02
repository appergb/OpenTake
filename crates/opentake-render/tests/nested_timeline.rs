use opentake_domain::{Clip, ClipType, NestedSequence, Timeline, Track};
use opentake_render::{build_render_plan, RenderSize, SourceMetrics};

struct Metrics;

impl SourceMetrics for Metrics {
    fn natural_size(&self, _media_ref: &str) -> Option<(u32, u32)> {
        Some((64, 64))
    }
}

fn video_clip(id: &str, media_ref: &str, start: i32, duration: i32) -> Clip {
    let mut clip = Clip::new(id, media_ref, start, duration);
    clip.media_type = ClipType::Video;
    clip.source_clip_type = ClipType::Video;
    clip
}

fn sequence_clip(id: &str, sequence_id: &str, start: i32, duration: i32) -> Clip {
    Clip::new_nested(id, sequence_id, start, duration)
}

#[test]
fn nested_edits_preview_and_export_same_frames() {
    let mut child_timeline = Timeline::new();
    child_timeline.settings_configured = true;
    let mut child_track = Track::new("child-v1", ClipType::Video);
    child_track
        .clips
        .push(video_clip("child-clip", "child-source-a", 2, 8));
    child_timeline.tracks.push(child_track);

    let mut root = Timeline::new();
    root.settings_configured = true;
    root.nested_sequences
        .push(NestedSequence::new("sequence-a", "Scene A", child_timeline));
    let mut root_track = Track::new("root-v1", ClipType::Video);
    root_track
        .clips
        .push(sequence_clip("compound", "sequence-a", 10, 20));
    root.tracks.push(root_track);

    root.validate_nested_sequences()
        .expect("valid nested graph");
    let encoded = serde_json::to_vec(&root).expect("serialize nested project");
    let mut reopened: Timeline = serde_json::from_slice(&encoded).expect("reopen nested project");
    assert_eq!(reopened.nested_sequences[0].name, "Scene A");

    let preview_plan = build_render_plan(&reopened, RenderSize::new(64, 64), &Metrics);
    let preview = preview_plan.frame(&reopened, 12);
    assert_eq!(preview.draws.len(), 1);
    assert_eq!(preview.draws[0].clip_id, "child-clip");
    assert_eq!(preview.draws[0].source_frame, 0);

    reopened.nested_sequences[0].timeline.tracks[0].clips[0].media_ref =
        "child-source-b".to_string();
    reopened.nested_sequences[0].timeline.tracks[0].clips[0].trim_start_frame = 3;

    let preview_after_edit = build_render_plan(&reopened, RenderSize::new(64, 64), &Metrics);
    let export_after_edit = build_render_plan(&reopened, RenderSize::new(64, 64), &Metrics);
    for frame in 12..20 {
        assert_eq!(
            preview_after_edit.frame(&reopened, frame),
            export_after_edit.frame(&reopened, frame),
            "preview/export diverged at frame {frame}",
        );
    }
    assert_eq!(
        preview_after_edit.frame(&reopened, 12).draws[0].source_frame,
        3
    );

    let mut cycle = Timeline::new();
    let mut a = Timeline::new();
    let mut a_track = Track::new("a-track", ClipType::Video);
    a_track.clips.push(sequence_clip("a-to-b", "b", 0, 10));
    a.tracks.push(a_track);
    let mut b = Timeline::new();
    let mut b_track = Track::new("b-track", ClipType::Video);
    b_track.clips.push(sequence_clip("b-to-a", "a", 0, 10));
    b.tracks.push(b_track);
    cycle
        .nested_sequences
        .push(NestedSequence::new("a", "A", a));
    cycle
        .nested_sequences
        .push(NestedSequence::new("b", "B", b));
    let error = cycle
        .validate_nested_sequences()
        .expect_err("cycle must be rejected");
    assert!(error.contains("a -> b -> a"), "unexpected error: {error}");
}
