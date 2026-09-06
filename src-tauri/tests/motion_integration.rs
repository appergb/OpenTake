use opentake_agent::mcp::motion::{MotionBridge, MotionBridgeErrorKind};
use opentake_core::{AppCore, EditCommand};
use opentake_domain::MediaResolver;
use opentake_tauri_lib::motion::{
    DocumentMotionAddRequest, DocumentMotionEditRequest, DocumentMotionSource, TauriMotionBridge,
};

fn live_motion_enabled() -> bool {
    std::env::var("OPENTAKE_RUN_FFMPEG_TESTS").as_deref() == Ok("1")
}

fn document_source(id: &str, revision: char, title: &str, accent: &str) -> DocumentMotionSource {
    DocumentMotionSource {
        document_id: id.to_string(),
        revision_hash: revision.to_string().repeat(64),
        html: format!(r#"<main class="stage"><h1>{title}</h1></main>"#),
        css: format!(
            r#"html,body{{background:#08090b;color:white}}.stage{{width:100%;height:100%;display:grid;place-items:center}}h1{{font:700 18px sans-serif;color:{accent};animation:enter .6s both}}@keyframes enter{{from{{opacity:.08;transform:translateX(-20px)}}to{{opacity:1;transform:translateX(20px)}}}}"#
        ),
    }
}

fn decode(path: &std::path::Path, time_secs: f64) -> Vec<u8> {
    opentake_media::decode_frame_at(
        path,
        &opentake_media::FrameRequest {
            time_secs,
            max_size: (96, 54),
            tolerance_secs: 0.0,
            apply_rotation: true,
        },
    )
    .expect("decode published Motion Studio frame")
    .1
    .rgba
}

#[test]
fn studio_document_publish_is_visible_atomic_editable_and_reopenable() {
    if !live_motion_enabled() {
        eprintln!("SKIP: set OPENTAKE_RUN_FFMPEG_TESTS=1 for live Studio publishing");
        return;
    }

    let root = tempfile::tempdir().unwrap();
    let bundle = root.path().join("Studio.opentake");
    let core = AppCore::new();
    core.apply(EditCommand::SetTimelineSettings {
        fps: 20,
        width: 96,
        height: 54,
    })
    .unwrap();
    core.save_project(Some(bundle.clone())).unwrap();
    let bridge = TauriMotionBridge::new(core.clone(), root.path().join("cache"));
    if !bridge.can_render_motion() {
        eprintln!("SKIP: packaged Chromium/FFmpeg motion capability is unavailable");
        return;
    }

    let added = bridge
        .add_document(
            DocumentMotionAddRequest {
                source: document_source("doc-a", 'a', "真实字符 Real text", "#ff3366"),
                project_authority: core.project_asset_authority().unwrap(),
                width: 96,
                height: 54,
                fps: 10,
                start_frame: 2,
                duration_frames: 6,
                transparent: true,
                track_index: None,
            },
            &opentake_media::MediaCancelToken::new(),
        )
        .unwrap();
    assert_eq!(
        added
            .source_document
            .as_ref()
            .map(|source| source.document_id.as_str()),
        Some("doc-a")
    );
    assert_eq!(
        added
            .source_document
            .as_ref()
            .map(|source| source.revision_hash.clone()),
        Some("a".repeat(64))
    );

    let snapshot = core.runtime_snapshot();
    let clip = snapshot
        .timeline
        .tracks
        .iter()
        .flat_map(|track| &track.clips)
        .find(|clip| clip.id == added.clip_id)
        .unwrap();
    assert_eq!((clip.start_frame, clip.duration_frames), (2, 12));
    let entry = snapshot
        .media
        .entries
        .iter()
        .find(|entry| entry.id == added.asset_id)
        .unwrap();
    let published = MediaResolver::new(&snapshot.media, snapshot.project_dir.as_deref())
        .expected_path(&entry.id)
        .unwrap();
    let beginning = decode(&published, 0.0);
    let middle = decode(&published, 0.3);
    let end = decode(&published, 0.5);
    assert_ne!(
        beginning, middle,
        "published CSS animation must move over time"
    );
    assert_ne!(
        middle, end,
        "middle/end frames must remain time-addressable"
    );
    assert!(
        middle
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect::<std::collections::HashSet<_>>()
            .len()
            > 4,
        "published frame must contain real glyph and scene pixels"
    );

    let edited = bridge
        .edit_document(
            DocumentMotionEditRequest {
                clip_id: added.clip_id.clone(),
                source: document_source("doc-a", 'b', "更新字符 Updated", "#33ccff"),
                project_authority: core.project_asset_authority().unwrap(),
                width: 96,
                height: 54,
                fps: 10,
                duration_frames: 6,
            },
            &opentake_media::MediaCancelToken::new(),
        )
        .unwrap();
    assert_eq!(edited.clip_id, added.clip_id);
    assert_ne!(edited.asset_id, added.asset_id);
    assert_eq!(edited.output.output_file, "output.mov");
    assert_eq!(
        core.media().entries.len(),
        2,
        "one edit registers exactly one replacement asset"
    );
    assert_eq!(
        edited
            .source_document
            .as_ref()
            .map(|source| source.revision_hash.clone()),
        Some("b".repeat(64))
    );

    let edited_snapshot = core.runtime_snapshot();
    let edited_entry = edited_snapshot
        .media
        .entries
        .iter()
        .find(|entry| entry.id == edited.asset_id)
        .unwrap();
    let edited_path = MediaResolver::new(
        &edited_snapshot.media,
        edited_snapshot.project_dir.as_deref(),
    )
    .expected_path(&edited_entry.id)
    .unwrap();
    let edited_middle = decode(&edited_path, 0.3);

    core.save_project(None).unwrap();
    let reopened = AppCore::new();
    reopened.open_project(bundle).unwrap();
    let reopened_snapshot = reopened.runtime_snapshot();
    assert_eq!(reopened_snapshot.timeline, core.runtime_snapshot().timeline);
    assert_eq!(reopened_snapshot.media, core.runtime_snapshot().media);
    let reopened_entry = reopened_snapshot
        .media
        .entries
        .iter()
        .find(|entry| entry.id == edited.asset_id)
        .unwrap();
    let reopened_path = MediaResolver::new(
        &reopened_snapshot.media,
        reopened_snapshot.project_dir.as_deref(),
    )
    .expected_path(&reopened_entry.id)
    .unwrap();
    assert_eq!(decode(&reopened_path, 0.3), edited_middle);

    let before_cancel = core.runtime_snapshot();
    let cancelled = opentake_media::MediaCancelToken::new();
    cancelled.cancel();
    let error = bridge
        .add_document(
            DocumentMotionAddRequest {
                source: document_source("doc-cancel", 'c', "cancel", "#ffffff"),
                project_authority: core.project_asset_authority().unwrap(),
                width: 96,
                height: 54,
                fps: 10,
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

    let invalid = bridge
        .add_document(
            DocumentMotionAddRequest {
                source: document_source("doc-invalid", 'd', "invalid", "#ffffff"),
                project_authority: core.project_asset_authority().unwrap(),
                width: 1,
                height: 54,
                fps: 10,
                start_frame: 0,
                duration_frames: 2,
                transparent: false,
                track_index: None,
            },
            &opentake_media::MediaCancelToken::new(),
        )
        .unwrap_err();
    assert_eq!(invalid.kind, MotionBridgeErrorKind::InvalidArguments);
    assert_eq!(core.runtime_snapshot().timeline, before_cancel.timeline);
    assert_eq!(core.runtime_snapshot().media, before_cancel.media);
}
