use opentake_domain::{Clip, ClipType, Timeline, Track};
use opentake_media::analysis::{
    analyze_stabilization, StabilizationConfig, StabilizationMotionSample,
};
use opentake_media::MediaCancelToken;
use opentake_ops::{apply, EditCommand, EditorState, SeqIdGen};
use opentake_render::{build_render_plan, RenderSize, SourceMetrics};

struct FullHdSource;

impl SourceMetrics for FullHdSource {
    fn natural_size(&self, _media_ref: &str) -> Option<(u32, u32)> {
        Some((1920, 1080))
    }
}

fn displacement(path: &[(f64, f64)]) -> f64 {
    path.windows(2)
        .map(|pair| (pair[1].0 - pair[0].0).hypot(pair[1].1 - pair[0].1))
        .sum()
}

#[test]
fn synthetic_shake_produces_editable_undoable_preview_export_solution() {
    let observed = [
        (0.000, 0.000),
        (0.040, -0.018),
        (-0.032, 0.022),
        (0.047, -0.026),
        (-0.038, 0.019),
        (0.034, -0.015),
        (0.000, 0.000),
    ];
    let samples = observed
        .iter()
        .enumerate()
        .map(|(frame, &(x, y))| StabilizationMotionSample {
            frame: frame as i32,
            translation_x: x,
            translation_y: y,
            rotation_degrees: 0.0,
        })
        .collect::<Vec<_>>();
    let cancel = MediaCancelToken::new();
    let solution = analyze_stabilization(
        &samples,
        "asset-shake",
        StabilizationConfig::default(),
        &cancel,
    )
    .expect("synthetic stabilization analysis");

    assert_eq!(solution.model, "opentake.motion-smoothing");
    assert_eq!(solution.model_version, 1);
    assert_eq!(solution.source_identity, "asset-shake");
    let stabilized = samples
        .iter()
        .map(|sample| {
            let correction = solution.sample(sample.frame);
            (
                sample.translation_x + correction.translation_x,
                sample.translation_y + correction.translation_y,
            )
        })
        .collect::<Vec<_>>();
    assert!(displacement(&stabilized) < displacement(&observed));
    assert!(solution.guarantees_coverage(16.0 / 9.0));

    let mut timeline = Timeline::new();
    let mut track = Track::new("video-track", ClipType::Video);
    track.clips.push(Clip::new(
        "clip-shake",
        "asset-shake",
        0,
        samples.len() as i32,
    ));
    timeline.tracks.push(track);
    let mut state = EditorState::from_timeline(timeline);
    let ids = SeqIdGen::new("stabilization");

    apply(
        &mut state,
        EditCommand::ApplyStabilization {
            clip_id: "clip-shake".into(),
            solution: solution.clone(),
        },
        &ids,
    )
    .expect("apply stabilization");
    assert_eq!(state.timeline.tracks[0].clips[0].media_ref, "asset-shake");
    assert_eq!(state.undo_depth(), 1);

    apply(
        &mut state,
        EditCommand::AdjustStabilization {
            clip_id: "clip-shake".into(),
            strength: Some(0.65),
            crop_margin: Some(0.03),
        },
        &ids,
    )
    .expect("adjust stabilization");
    let edited = state.timeline.tracks[0].clips[0]
        .stabilization
        .as_ref()
        .expect("persisted editable stabilization track");
    assert_eq!(edited.strength, 0.65);
    assert_eq!(edited.crop_margin, 0.03);

    let preview = build_render_plan(&state.timeline, RenderSize::new(1920, 1080), &FullHdSource);
    let export = build_render_plan(&state.timeline, RenderSize::new(1920, 1080), &FullHdSource);
    for frame in 0..samples.len() as i32 {
        let preview_draw = &preview.frame(&state.timeline, frame).draws[0];
        let export_draw = &export.frame(&state.timeline, frame).draws[0];
        assert_eq!(preview_draw.affine, export_draw.affine);
        assert_eq!(preview_draw.crop_uv, export_draw.crop_uv);
    }

    apply(&mut state, EditCommand::Undo, &ids).expect("undo adjustment");
    assert_eq!(
        state.timeline.tracks[0].clips[0]
            .stabilization
            .as_ref()
            .expect("analysis remains after undo")
            .strength,
        1.0
    );
    apply(
        &mut state,
        EditCommand::ResetStabilization {
            clip_id: "clip-shake".into(),
        },
        &ids,
    )
    .expect("reset stabilization");
    assert!(state.timeline.tracks[0].clips[0].stabilization.is_none());
    apply(&mut state, EditCommand::Undo, &ids).expect("undo reset");
    assert!(state.timeline.tracks[0].clips[0].stabilization.is_some());

    let cancelled = MediaCancelToken::new();
    cancelled.cancel();
    assert!(analyze_stabilization(
        &samples,
        "asset-shake",
        StabilizationConfig::default(),
        &cancelled,
    )
    .is_err());
}
