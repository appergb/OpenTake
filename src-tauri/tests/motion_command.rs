use std::sync::{Arc, Mutex};

use opentake_agent::mcp::motion::{
    AddMotionRequest, EditMotionRequest, MotionBridge, MotionBridgeErrorKind, MotionSourceRequest,
};
use opentake_core::{AppCore, EditCommand};
use opentake_domain::{GenerationJobStatus, MediaResolver};
use opentake_tauri_lib::motion::{MotionProgress, TauriMotionBridge};
use opentake_tauri_lib::{
    export::{run_export, ExportQuality, ExportRequest},
    render::{composite_timeline_frame, RenderState},
};

#[test]
fn sandbox_progress_cancel_validated_mp4_result() {
    let root = tempfile::tempdir().unwrap();
    let bundle = root.path().join("Motion.opentake");
    let core = AppCore::new();
    core.apply(EditCommand::SetTimelineSettings {
        fps: 10,
        width: 64,
        height: 36,
    })
    .unwrap();
    core.save_project(Some(bundle.clone())).unwrap();

    let phases = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&phases);
    let bridge = TauriMotionBridge::new(core.clone(), root.path().join("cache"))
        .with_progress_callback(Arc::new(move |phase| recorded.lock().unwrap().push(phase)));
    if !bridge.can_render_motion() {
        eprintln!("SKIP: native Chromium/FFmpeg motion capability is unavailable");
        return;
    }

    let added = bridge
        .add(
            AddMotionRequest {
                source: MotionSourceRequest::Template {
                    template_id: "title-card".into(),
                    params: serde_json::from_value(serde_json::json!({
                        "title": "Deterministic",
                        "subtitle": "OpenTake motion",
                        "accent": "#FF3366"
                    }))
                    .unwrap(),
                },
                start_frame: 2,
                duration_frames: 4,
                transparent: false,
                track_index: None,
            },
            &opentake_media::MediaCancelToken::new(),
        )
        .unwrap();
    assert_eq!(
        *phases.lock().unwrap(),
        vec![
            MotionProgress::Validating,
            MotionProgress::Rendering {
                done_frames: 0,
                total_frames: 4,
            },
            MotionProgress::Rendering {
                done_frames: 1,
                total_frames: 4,
            },
            MotionProgress::Rendering {
                done_frames: 2,
                total_frames: 4,
            },
            MotionProgress::Rendering {
                done_frames: 3,
                total_frames: 4,
            },
            MotionProgress::Rendering {
                done_frames: 4,
                total_frames: 4,
            },
            MotionProgress::Encoding,
            MotionProgress::Committing,
            MotionProgress::Complete,
        ]
    );

    let snapshot = core.runtime_snapshot();
    let clip = snapshot
        .timeline
        .tracks
        .iter()
        .flat_map(|track| &track.clips)
        .find(|clip| clip.id == added.clip_id)
        .unwrap();
    assert_eq!(clip.start_frame, 2);
    assert_eq!(clip.duration_frames, 4);
    assert_eq!(clip.media_ref, added.asset_id);
    let entry = snapshot
        .media
        .entries
        .iter()
        .find(|entry| entry.id == added.asset_id)
        .unwrap();
    let provenance = entry.generation_input.as_ref().unwrap();
    assert_eq!(provenance.provider.as_deref(), Some("opentake-motion"));
    assert_eq!(provenance.status, Some(GenerationJobStatus::Ready));
    let path = MediaResolver::new(&snapshot.media, snapshot.project_dir.as_deref())
        .expected_path(&entry.id)
        .unwrap();
    let probe = opentake_media::probe(&path).unwrap();
    assert!(probe.has_video);
    assert_eq!((probe.width, probe.height), (Some(64), Some(36)));
    assert!((probe.duration_secs - 0.4).abs() <= 0.15);
    let decode = |time_secs| {
        opentake_media::decode_frame_at(
            &path,
            &opentake_media::FrameRequest {
                time_secs,
                max_size: (64, 36),
                tolerance_secs: 0.0,
                apply_rotation: true,
            },
        )
        .unwrap()
        .1
    };
    let first = decode(0.0);
    let animated = decode(0.3);
    assert_ne!(
        first.rgba, animated.rgba,
        "Motion Canvas template must change pixels across its deterministic timeline"
    );
    let distinct_colors = animated
        .rgba
        .chunks_exact(4)
        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect::<std::collections::HashSet<_>>();
    assert!(
        distinct_colors.len() > 4,
        "rendered template must contain real card/text/accent pixels, not a solid placeholder"
    );
    assert_eq!(added.output.renderer, "motion-canvas");
    assert_eq!(added.output.renderer_version, "3.17.2");
    assert_eq!((added.output.width, added.output.height), (64, 36));
    assert_eq!(added.output.duration_frames, 4);
    assert_eq!(added.output.content_hash, added.content_hash);

    let preview_state = RenderState::new();
    let preview_cancel = opentake_media::MediaCancelToken::new();
    let preview_before = composite_timeline_frame(
        &snapshot.timeline,
        &snapshot.media,
        &snapshot.project_dir,
        &preview_state,
        0,
        64,
        &preview_cancel,
    );
    let preview_motion = composite_timeline_frame(
        &snapshot.timeline,
        &snapshot.media,
        &snapshot.project_dir,
        &preview_state,
        3,
        64,
        &preview_cancel,
    );
    match (preview_before, preview_motion) {
        (Ok(before), Ok(motion)) => {
            assert_ne!(
                before.rgba, motion.rgba,
                "preview composite must contain the generated clip"
            );
            assert!(
                motion
                    .rgba
                    .chunks_exact(4)
                    .map(|pixel| [pixel[0], pixel[1], pixel[2]])
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    > 4,
                "preview composite must retain the generated card/text/accent pixels"
            );

            let exported = root.path().join("motion-export.mp4");
            let request = ExportRequest {
                out_path: exported.to_string_lossy().into_owned(),
                codec: Default::default(),
                quality: ExportQuality::P720,
            };
            let summary = run_export(
                &snapshot.timeline,
                &snapshot.media,
                &snapshot.project_dir,
                &request,
            )
            .expect("generated motion timeline must export");
            assert_eq!(summary.frame_count, 6);
            let exported_before = opentake_media::decode_frame_at(
                &exported,
                &opentake_media::FrameRequest {
                    time_secs: 0.0,
                    max_size: (64, 36),
                    tolerance_secs: 0.0,
                    apply_rotation: true,
                },
            )
            .unwrap()
            .1;
            let exported_motion = opentake_media::decode_frame_at(
                &exported,
                &opentake_media::FrameRequest {
                    time_secs: 0.3,
                    max_size: (64, 36),
                    tolerance_secs: 0.0,
                    apply_rotation: true,
                },
            )
            .unwrap()
            .1;
            assert_ne!(
                exported_before.rgba, exported_motion.rgba,
                "export_video output must contain the generated motion clip"
            );
        }
        (Err(error), _) | (_, Err(error)) if error.contains("no GPU device") => {
            eprintln!("SKIP composite/export acceptance: {error}");
        }
        (Err(error), _) | (_, Err(error)) => panic!("motion composite failed: {error}"),
    }

    let duplicate = bridge
        .add(
            AddMotionRequest {
                source: MotionSourceRequest::Template {
                    template_id: "title-card".into(),
                    params: serde_json::from_value(serde_json::json!({
                        "title": "Deterministic",
                        "subtitle": "OpenTake motion",
                        "accent": "#FF3366"
                    }))
                    .unwrap(),
                },
                start_frame: 20,
                duration_frames: 4,
                transparent: false,
                track_index: None,
            },
            &opentake_media::MediaCancelToken::new(),
        )
        .unwrap();
    assert_eq!(duplicate.content_hash, added.content_hash);
    assert_eq!(duplicate.output, added.output);
    let duplicate_snapshot = core.runtime_snapshot();
    let duplicate_entry = duplicate_snapshot
        .media
        .entries
        .iter()
        .find(|entry| entry.id == duplicate.asset_id)
        .unwrap();
    let duplicate_path = MediaResolver::new(
        &duplicate_snapshot.media,
        duplicate_snapshot.project_dir.as_deref(),
    )
    .expected_path(&duplicate_entry.id)
    .unwrap();
    let duplicate_frame = opentake_media::decode_frame_at(
        &duplicate_path,
        &opentake_media::FrameRequest {
            time_secs: 0.3,
            max_size: (64, 36),
            tolerance_secs: 0.0,
            apply_rotation: true,
        },
    )
    .unwrap()
    .1;
    assert_eq!(duplicate_frame.rgba, animated.rgba);
    core.undo().unwrap();

    phases.lock().unwrap().clear();
    let edited = bridge
        .edit(
            EditMotionRequest {
                clip_id: added.clip_id.clone(),
                code: None,
                params: Some(
                    serde_json::from_value(serde_json::json!({"title": "Updated"})).unwrap(),
                ),
            },
            &opentake_media::MediaCancelToken::new(),
        )
        .unwrap();
    assert_eq!(edited.clip_id, added.clip_id);
    assert_ne!(edited.asset_id, added.asset_id);
    assert_eq!(core.media().entries.len(), 2);
    assert_eq!(
        core.get_timeline().timeline.tracks[0].clips[0].media_ref,
        edited.asset_id
    );

    core.undo().unwrap();
    assert_eq!(core.media().entries.len(), 1);
    assert_eq!(
        core.get_timeline().timeline.tracks[0].clips[0].media_ref,
        added.asset_id
    );
    core.save_project(None).unwrap();

    let before_cancel = core.runtime_snapshot();
    let cancelled = opentake_media::MediaCancelToken::new();
    cancelled.cancel();
    let error = bridge
        .add(
            AddMotionRequest {
                source: MotionSourceRequest::Template {
                    template_id: "title-card".into(),
                    params: Default::default(),
                },
                start_frame: 0,
                duration_frames: 2,
                transparent: false,
                track_index: None,
            },
            &cancelled,
        )
        .unwrap_err();
    assert_eq!(error.kind, MotionBridgeErrorKind::Cancelled);
    assert_eq!(core.runtime_snapshot().timeline, before_cancel.timeline);
    assert_eq!(core.runtime_snapshot().media, before_cancel.media);

    let error = bridge
        .add(
            AddMotionRequest {
                source: MotionSourceRequest::Code(
                    r#"<html><body><img src="https://example.com/no.png"></body></html>"#.into(),
                ),
                start_frame: 0,
                duration_frames: 2,
                transparent: false,
                track_index: None,
            },
            &opentake_media::MediaCancelToken::new(),
        )
        .unwrap_err();
    assert_eq!(error.kind, MotionBridgeErrorKind::InvalidArguments);
    assert_eq!(core.runtime_snapshot().timeline, before_cancel.timeline);
    assert_eq!(core.runtime_snapshot().media, before_cancel.media);

    let reopened = AppCore::new();
    reopened.open_project(bundle).unwrap();
    assert_eq!(reopened.media(), core.media());
    assert_eq!(
        reopened.get_timeline().timeline,
        core.get_timeline().timeline
    );
}

#[test]
fn transparent_motion_publishes_prores_alpha_and_marks_manifest() {
    let root = tempfile::tempdir().unwrap();
    let bundle = root.path().join("TransparentMotion.opentake");
    let core = AppCore::new();
    core.apply(EditCommand::SetTimelineSettings {
        fps: 10,
        width: 64,
        height: 36,
    })
    .unwrap();
    core.save_project(Some(bundle)).unwrap();

    let bridge = TauriMotionBridge::new(core.clone(), root.path().join("cache"));
    if !bridge.can_render_motion() {
        eprintln!("SKIP: native Chromium/FFmpeg motion capability is unavailable");
        return;
    }

    let added = bridge
        .add(
            AddMotionRequest {
                source: MotionSourceRequest::Template {
                    template_id: "lower-third.glass".into(),
                    params: serde_json::from_value(serde_json::json!({
                        "title": "Transparent",
                        "subtitle": "OpenTake alpha",
                        "accent": "#FF3366"
                    }))
                    .unwrap(),
                },
                start_frame: 0,
                duration_frames: 4,
                transparent: true,
                track_index: None,
            },
            &opentake_media::MediaCancelToken::new(),
        )
        .expect("transparent motion publish");

    assert_eq!(added.output.output_file, "output.mov");
    let snapshot = core.runtime_snapshot();
    let entry = snapshot
        .media
        .entries
        .iter()
        .find(|entry| entry.id == added.asset_id)
        .expect("transparent motion manifest entry");
    assert_eq!(
        entry
            .generation_input
            .as_ref()
            .and_then(|input| input.transparent),
        Some(true)
    );
    assert!(entry.carries_straight_alpha());
    let path = MediaResolver::new(&snapshot.media, snapshot.project_dir.as_deref())
        .expected_path(&entry.id)
        .unwrap();
    let probe = opentake_media::probe(&path).expect("probe transparent motion output");
    assert_eq!(probe.video_codec.as_deref(), Some("prores"));
    assert_eq!((probe.width, probe.height), (Some(64), Some(36)));

    let alpha = std::process::Command::new(opentake_media::ffmpeg_status::ffmpeg_path())
        .args([
            "-v",
            "error",
            "-i",
            path.to_str().expect("UTF-8 transparent output path"),
            "-vf",
            "alphaextract",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "gray",
            "pipe:1",
        ])
        .output()
        .expect("decode transparent motion alpha");
    assert!(alpha.status.success(), "decode alpha: {:?}", alpha.stderr);
    assert!(alpha.stdout.iter().any(|value| *value == 0));
    assert!(alpha.stdout.iter().any(|value| *value > 0 && *value < 255));
}
