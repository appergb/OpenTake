use std::sync::{Arc, Mutex};

use opentake_agent::mcp::motion::{
    AddMotionRequest, EditMotionRequest, MotionBridge, MotionBridgeErrorKind, MotionSourceRequest,
};
use opentake_core::{AppCore, EditCommand};
use opentake_domain::{GenerationJobStatus, MediaResolver};
use opentake_tauri_lib::motion::{MotionProgress, TauriMotionBridge};

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
            MotionProgress::Rendering,
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
