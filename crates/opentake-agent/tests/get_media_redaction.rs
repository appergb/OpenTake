use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use opentake_agent::mcp::core_handle::CoreHandle;
use opentake_agent::mcp::dispatch::Dispatcher;
use opentake_agent::plugin::registry::PluginRegistry;
use opentake_agent::tools::result::Block;
use opentake_domain::{
    ClipType, GenerationInput, GenerationJobStatus, MediaColorMetadata, MediaFolder, MediaManifest,
    MediaManifestEntry, MediaProxy, MediaSource, Timeline,
};
use opentake_ops::{EditCommand, EditResult};
use serde_json::{json, Value};

struct MediaHandle {
    manifest: MediaManifest,
}

impl CoreHandle for MediaHandle {
    fn timeline(&self) -> Timeline {
        Timeline::new()
    }

    fn media(&self) -> MediaManifest {
        self.manifest.clone()
    }

    fn apply(&self, _command: EditCommand) -> anyhow::Result<EditResult> {
        anyhow::bail!("read-only fixture")
    }

    fn project_dir(&self) -> Option<PathBuf> {
        None
    }
}

fn get_media(manifest: MediaManifest) -> (opentake_agent::tools::result::ToolResult, Value) {
    let dispatcher = Dispatcher::new(
        Arc::new(MediaHandle { manifest }),
        Arc::new(RwLock::new(PluginRegistry::new())),
    );
    let result = dispatcher.dispatch("get_media", json!({}));
    assert!(!result.is_error, "{}", result.text_joined());
    let text = match result.content.first() {
        Some(Block::Text { text }) => text,
        other => panic!("expected JSON text block, got {other:?}"),
    };
    let payload = serde_json::from_str(text).expect("get_media JSON");
    (result, payload)
}

#[test]
fn get_media_tool_result_allowlists_model_safe_metadata() {
    let mut manifest = MediaManifest::new();
    manifest.folders.push(MediaFolder {
        id: "folder-safe".into(),
        name: "References".into(),
        parent_folder_id: None,
    });
    manifest.entries.push(MediaManifestEntry {
        id: "asset-safe-1".into(),
        name: "Hero shot".into(),
        kind: ClipType::Video,
        source: MediaSource::External {
            absolute_path: "/Users/private/secret.mov".into(),
        },
        duration: 12.34567,
        generation_input: Some(GenerationInput {
            prompt: "PROVIDER_INPUT_SECRET".into(),
            model: "private-provider-model".into(),
            duration: 12,
            aspect_ratio: "16:9".into(),
            image_urls: Some(vec![
                "https://reference.invalid/input?token=IMAGE_SECRET".into()
            ]),
            reference_image_urls: Some(vec![
                "https://reference.invalid/image?token=REFERENCE_IMAGE_SECRET".into(),
            ]),
            reference_video_urls: Some(vec![
                "https://reference.invalid/video?token=REFERENCE_VIDEO_SECRET".into(),
            ]),
            reference_audio_urls: Some(vec![
                "https://reference.invalid/audio?token=REFERENCE_AUDIO_SECRET".into(),
            ]),
            provider: Some("private-provider".into()),
            provider_job_id: Some("PROVIDER_JOB_SECRET".into()),
            status: Some(GenerationJobStatus::Generating),
            progress: Some(0.45678),
            error_code: Some("PROVIDER_DIAGNOSTIC_SECRET".into()),
            ..GenerationInput::default()
        }),
        source_width: Some(3840),
        source_height: Some(2160),
        source_fps: Some(23.97654),
        has_audio: Some(true),
        color: Some(MediaColorMetadata {
            primaries: Some("bt2020".into()),
            transfer: Some("smpte2084".into()),
            matrix: Some("bt2020nc".into()),
            range: Some("SOURCE_COLOR_DIAGNOSTIC_SECRET".into()),
        }),
        proxy: Some(MediaProxy {
            relative_path: "proxy/private.mov?token=PROXY_SECRET".into(),
            source_sha256: "SOURCE_DIGEST_SECRET".into(),
            width: 960,
            height: 540,
        }),
        folder_id: Some("folder-safe".into()),
        cached_remote_url: Some("https://host/file?token=SECRET".into()),
        cached_remote_url_expires_at: Some(999_999_999.0),
    });
    manifest.favorite_library_ids.insert(
        "asset-safe-1".into(),
        "GLOBAL_LIBRARY_INTERNAL_SECRET".into(),
    );

    let (result, payload) = get_media(manifest);
    let serialized_result = serde_json::to_string(&result).expect("serialize tool result");

    for secret in [
        "https://host/file?token=SECRET",
        "IMAGE_SECRET",
        "REFERENCE_IMAGE_SECRET",
        "REFERENCE_VIDEO_SECRET",
        "REFERENCE_AUDIO_SECRET",
        "/Users/private/secret.mov",
        "PROVIDER_INPUT_SECRET",
        "PROVIDER_JOB_SECRET",
        "PROVIDER_DIAGNOSTIC_SECRET",
        "SOURCE_COLOR_DIAGNOSTIC_SECRET",
        "PROXY_SECRET",
        "SOURCE_DIGEST_SECRET",
        "GLOBAL_LIBRARY_INTERNAL_SECRET",
    ] {
        assert!(
            !serialized_result.contains(secret),
            "tool result leaked {secret}: {serialized_result}"
        );
    }

    let entry = &payload["entries"][0];
    assert_eq!(entry["id"], json!("asset-safe-1"));
    assert_eq!(entry["name"], json!("Hero shot"));
    assert_eq!(entry["type"], json!("video"));
    assert_eq!(entry["folderId"], json!("folder-safe"));
    assert_eq!(entry["duration"], json!(12.346));
    assert_eq!(entry["sourceWidth"], json!(3840));
    assert_eq!(entry["sourceHeight"], json!(2160));
    assert_eq!(entry["sourceFPS"], json!(23.977));
    assert_eq!(entry["hasAudio"], json!(true));
    assert_eq!(entry["hasProxy"], json!(true));
    assert_eq!(entry["isHdr"], json!(true));
    assert_eq!(entry["generationStatus"], json!("generating"));
    assert_eq!(entry["generationProgress"], json!(0.457));
    assert_eq!(payload["folders"][0]["name"], json!("References"));

    for forbidden_key in [
        "source",
        "generationInput",
        "generationErrorCode",
        "cachedRemoteURL",
        "cachedRemoteURLExpiresAt",
        "color",
        "proxy",
        "favoriteLibraryIds",
    ] {
        assert!(
            !serialized_result.contains(&format!("\"{forbidden_key}\"")),
            "tool result exposed forbidden key {forbidden_key}: {serialized_result}"
        );
    }
}

#[test]
fn get_media_keeps_empty_and_legacy_entries_usable() {
    let (_, empty) = get_media(MediaManifest::new());
    assert_eq!(empty["version"], json!(2));
    assert_eq!(empty["entries"], json!([]));
    assert_eq!(empty["folders"], json!([]));

    let legacy_manifest: MediaManifest = serde_json::from_value(json!({
        "entries": [{
            "id": "legacy-asset",
            "name": "Legacy clip",
            "type": "audio",
            "source": {"project": {"relativePath": "media/legacy.wav"}},
            "duration": 0.0
        }],
        "folders": []
    }))
    .expect("legacy manifest");
    let (legacy_result, legacy) = get_media(legacy_manifest);
    assert!(!legacy_result.text_joined().contains("media/legacy.wav"));
    assert_eq!(legacy["version"], json!(1));
    assert_eq!(legacy["entries"][0]["id"], json!("legacy-asset"));
    assert_eq!(legacy["entries"][0]["name"], json!("Legacy clip"));
    assert_eq!(legacy["entries"][0]["type"], json!("audio"));
    assert_eq!(legacy["entries"][0]["duration"], json!(0.0));
    assert_eq!(legacy["entries"][0]["generationStatus"], json!("none"));
    assert_eq!(legacy["entries"][0]["hasProxy"], json!(false));
    assert_eq!(legacy["entries"][0]["isHdr"], json!(false));
    assert!(legacy["entries"][0].get("source").is_none());
}
