use opentake_domain::{Clip, ClipType, NestedSequence, Timeline, Track};
use opentake_render::{try_build_render_plan, RenderSize, SourceMetrics};

struct Metrics;

impl SourceMetrics for Metrics {
    fn natural_size(&self, _media_ref: &str) -> Option<(u32, u32)> {
        Some((64, 64))
    }
}

fn video_track(id: &str, clips: Vec<Clip>) -> Track {
    let mut track = Track::new(id, ClipType::Video);
    track.clips = clips;
    track
}

#[test]
fn compound_clip_preview_export_frames_match() {
    let mut leaf = Clip::new("leaf", "asset-a", 3, 10);
    leaf.trim_start_frame = 2;
    let mut sequence_b = Timeline::new();
    sequence_b.tracks = vec![video_track("b-track", vec![leaf])];

    let mut nested_b = Clip::new_nested("nested-b", "sequence-b", 5, 8);
    nested_b.trim_start_frame = 3;
    let mut sequence_a = Timeline::new();
    sequence_a.tracks = vec![video_track("a-track", vec![nested_b])];

    let mut root = Timeline::new();
    root.nested_sequences = vec![
        NestedSequence::new("sequence-a", "A", sequence_a),
        NestedSequence::new("sequence-b", "B", sequence_b),
    ];
    let mut compound = Clip::new_nested("compound", "sequence-a", 20, 6);
    compound.trim_start_frame = 5;
    compound.opacity = 0.5;
    compound.transform.width = 0.5;
    compound.transform.height = 0.5;
    root.tracks = vec![video_track("root-track", vec![compound])];

    let preview = try_build_render_plan(&root, RenderSize::new(64, 64), &Metrics).unwrap();
    let export = try_build_render_plan(&root, RenderSize::new(64, 64), &Metrics).unwrap();
    for frame in 19..=27 {
        assert_eq!(preview.frame(&root, frame), export.frame(&root, frame));
    }
    assert!(preview.frame(&root, 19).draws.is_empty());
    assert_eq!(preview.frame(&root, 20).draws[0].source_frame, 2);
    assert_eq!(preview.frame(&root, 20).draws[0].opacity, 0.5);
    assert_eq!(
        preview.frame(&root, 20).draws[0].affine,
        [0.5, 0.0, 0.0, 0.5, 16.0, 16.0]
    );
    assert_eq!(preview.frame(&root, 25).draws[0].source_frame, 7);
    assert!(preview.frame(&root, 26).draws.is_empty());
}

#[test]
fn compound_trim_preserves_inner_fade_sampling_offset() {
    let mut leaf = Clip::new("leaf", "asset-a", 0, 20);
    leaf.fade_in_frames = 20;
    let mut child = Timeline::new();
    child.tracks = vec![video_track("child-track", vec![leaf])];

    let mut root = Timeline::new();
    root.nested_sequences
        .push(NestedSequence::new("sequence", "Sequence", child));
    let mut compound = Clip::new_nested("compound", "sequence", 100, 5);
    compound.trim_start_frame = 10;
    root.tracks = vec![video_track("root-track", vec![compound])];

    let plan = try_build_render_plan(&root, RenderSize::new(64, 64), &Metrics).unwrap();
    let first = &plan.frame(&root, 100).draws[0];
    assert_eq!(first.source_frame, 10);
    assert_eq!(first.opacity, 0.5);
}

#[test]
fn unsupported_compound_retime_fails_before_preview_or_export() {
    let mut child = Timeline::new();
    child.tracks = vec![video_track(
        "child-track",
        vec![Clip::new("leaf", "asset-a", 0, 10)],
    )];
    let mut root = Timeline::new();
    root.nested_sequences
        .push(NestedSequence::new("sequence", "Sequence", child));
    let mut compound = Clip::new_nested("compound", "sequence", 0, 10);
    compound.speed = 2.0;
    root.tracks = vec![video_track("root-track", vec![compound])];

    assert_eq!(
        try_build_render_plan(&root, RenderSize::new(64, 64), &Metrics).unwrap_err(),
        "compound clip compound must use forward 1x playback"
    );
}

#[test]
fn compound_audio_uses_the_same_trimmed_root_span() {
    let mut audio = Clip::new("audio", "asset-a", 4, 10);
    audio.media_type = ClipType::Audio;
    audio.source_clip_type = ClipType::Audio;
    audio.trim_start_frame = 2;
    audio.fade_in_frames = 10;
    let mut child = Timeline::new();
    child.tracks = vec![video_track("unused", vec![])];
    let mut audio_track = Track::new("audio-track", ClipType::Audio);
    audio_track.clips.push(audio);
    child.tracks.push(audio_track);

    let mut root = Timeline::new();
    root.nested_sequences
        .push(NestedSequence::new("sequence", "Sequence", child));
    let mut compound = Clip::new_nested("compound", "sequence", 20, 5);
    compound.trim_start_frame = 6;
    compound.volume = 0.5;
    compound.fade_in_frames = 5;
    root.tracks = vec![video_track("root-track", vec![compound])];

    let plan = try_build_render_plan(&root, RenderSize::new(64, 64), &Metrics).unwrap();
    assert_eq!(plan.audio_clips.len(), 1);
    let flattened = &plan.audio_clips[0];
    assert_eq!(flattened.clip.start_frame, 20);
    assert_eq!(flattened.clip.duration_frames, 5);
    assert_eq!(flattened.clip.trim_start_frame, 4);
    assert_eq!(flattened.volume_at(20), 0.0);
    assert!((flattened.volume_at(22) - 0.08).abs() < 1e-12);
}
