const EVIDENCE: &str = include_str!(
    "../../../docs/audit/2026-07-14/runtime-artifacts/automated/hdr-proxy-account-real-device-2026-08-01.md"
);
const GPU_CHILDREN: &str = include_str!("gpu_effects.rs");
const COMPOSITOR: &str = include_str!("../src/gpu/compositor.rs");
const RENDER_OVERVIEW: &str = include_str!("../../../docs/modules/opentake-render/OVERVIEW.md");
const MEDIA_PRINCIPLES: &str = include_str!("../../../docs/specs/media/0-principles.md");
const MEDIA_DOMAIN_CONTRACT: &str = include_str!("../../../docs/specs/media/9-domain-contract.md");
const MEDIA_FFMPEG_CHILDREN: &str =
    include_str!("../../opentake-media/tests/ffmpeg_integration.rs");
const MEDIA_ENGINE_SOURCE: &str = include_str!("../../opentake-media/src/lib.rs");
const MEDIA_INDEX_SOURCE: &str = include_str!("../../opentake-media/src/index_coordinator.rs");

use opentake_domain::{
    Clip, ClipType, Effect, Mask, MaskShape, Point2, Timeline, Track, MAX_MASKS_PER_CLIP,
    MAX_POLYGON_MASK_POINTS,
};
use opentake_ops::{apply, EditCommand, EditError, EditorState, SeqIdGen};

#[test]
fn hdr_proxy_account_children_close_one_composite_acceptance() {
    for child in [
        "HDR child result: **PASS**",
        "Proxy child result: **PASS**",
        "Account child result: **PASS**",
    ] {
        assert!(EVIDENCE.contains(child), "missing child evidence: {child}");
    }
    assert!(EVIDENCE.contains("`HDR child PASS + proxy child PASS + account child PASS`"));
    assert!(EVIDENCE.contains("closes one composite\nacceptance"));
    assert!(EVIDENCE.contains("codesign --verify --deep --strict"));
    assert!(EVIDENCE.contains("This is not\nan HDR-passthrough claim."));
    assert!(EVIDENCE.contains("Export therefore used the original source, not the enabled proxy."));
    assert!(EVIDENCE.contains("Local editing remains the default"));
}

fn state_with_visual_clip() -> EditorState {
    let mut timeline = Timeline::new();
    let mut track = Track::new("video", ClipType::Video);
    track.clips.push(Clip::new("clip", "asset", 0, 30));
    timeline.tracks.push(track);
    EditorState::from_timeline(timeline)
}

#[test]
fn mask_and_effect_records_have_separate_child_owners() {
    // The mixed audit record is closed by two executable pixel owners, rather
    // than by duplicating either renderer in this aggregation test.
    for owner in [
        "linear_circle_and_polygon_masks_match_cpu_reference_in_preview_and_export",
        "advertised_effect_registry_has_preview_export_golden_fixtures",
    ] {
        assert!(
            GPU_CHILDREN.contains(owner),
            "missing executable child owner: {owner}"
        );
    }
    for boundary in [
        "fn pack_masks",
        "MAX_POLYGON_MASK_POINTS",
        "fn pack_effects",
        "validate_effect_chain(draw.effects)",
    ] {
        assert!(
            COMPOSITOR.contains(boundary),
            "missing compositor boundary: {boundary}"
        );
    }

    // Authored overflow and unknown registry entries fail before mutation.
    let ids = SeqIdGen::default();
    let mut state = state_with_visual_clip();
    let original = state.timeline.clone();
    let too_many = vec![Mask::default(); MAX_MASKS_PER_CLIP + 1];
    let error = apply(
        &mut state,
        EditCommand::SetMasks {
            clip_ids: vec!["clip".into()],
            masks: too_many,
        },
        &ids,
    )
    .unwrap_err();
    assert!(matches!(error, EditError::Invalid(message) if message.contains("at most")));
    assert_eq!(state.timeline, original);
    assert_eq!(state.undo_depth(), 0);

    let polygon = Mask {
        shape: MaskShape::Poly {
            points: vec![Point2::new(0.5, 0.5); MAX_POLYGON_MASK_POINTS + 1],
        },
        ..Mask::default()
    };
    let error = apply(
        &mut state,
        EditCommand::SetMasks {
            clip_ids: vec!["clip".into()],
            masks: vec![polygon],
        },
        &ids,
    )
    .unwrap_err();
    assert!(matches!(error, EditError::Invalid(message) if message.contains("polygon")));
    assert_eq!(state.timeline, original);

    let error = apply(
        &mut state,
        EditCommand::SetEffects {
            clip_ids: vec!["clip".into()],
            effects: vec![Effect::new("unadvertised")],
        },
        &ids,
    )
    .unwrap_err();
    assert!(matches!(error, EditError::Invalid(message) if message.contains("unknown effect")));
    assert_eq!(state.timeline, original);

    assert!(RENDER_OVERVIEW.contains("多边形（钢笔）蒙版与通用 Effect 链已落地"));
    assert!(!RENDER_OVERVIEW.contains("编码为全画布 no-op"));
}

#[test]
fn media_principles_headings_reference_exact_child_capabilities() {
    assert!(MEDIA_PRINCIPLES.starts_with("# 设计原则与移植铁律(本 crate 必须遵守)"));
    assert!(MEDIA_DOMAIN_CONTRACT.starts_with("# 跨平台与合规要点"));
    for document in [MEDIA_PRINCIPLES, MEDIA_DOMAIN_CONTRACT] {
        assert!(document.contains("可执行子能力集合"));
        for child in [
            "probe_reports_dimensions_fps_and_audio",
            "decode_frame_returns_rgba_of_expected_size",
            "extract_pcm_yields_16k_mono",
            "waveform_has_expected_bucket_count",
            "encode_roundtrip_produces_playable_video",
            "export_pause_ref_counts",
        ] {
            assert!(document.contains(child), "missing child reference: {child}");
        }
    }

    for child in [
        "fn probe_reports_dimensions_fps_and_audio",
        "fn decode_frame_returns_rgba_of_expected_size",
        "fn extract_pcm_yields_16k_mono",
        "fn waveform_has_expected_bucket_count",
        "fn encode_roundtrip_produces_playable_video",
    ] {
        assert!(
            MEDIA_FFMPEG_CHILDREN.contains(child),
            "missing executable media child: {child}"
        );
    }
    assert!(MEDIA_ENGINE_SOURCE.contains("seconds (f64) at every IO"));
    assert!(MEDIA_ENGINE_SOURCE.contains("pub struct MediaEngine"));
    assert!(MEDIA_INDEX_SOURCE.contains("fn export_pause_ref_counts"));

    // The compliance collection must describe the actual subprocess-sidecar
    // architecture and preserve the known release blocker instead of claiming
    // dynamic linking or a completed public distribution review.
    assert!(MEDIA_DOMAIN_CONTRACT.contains("FFmpeg 子进程 sidecar"));
    assert!(MEDIA_DOMAIN_CONTRACT.contains("Beta 发布阻塞"));
    assert!(!MEDIA_DOMAIN_CONTRACT.contains("动态链接 + NOTICE"));
}
