#![recursion_limit = "256"]

//! 对拍: decode bundles written in upstream PalmierPro's exact JSON shape.
//!
//! The fixtures below are hand-written to mirror what Swift's `JSONEncoder`
//! emits and what upstream's tolerant decoders accept: camelCase keys with
//! abbreviation casing preserved (`mediaRef`, `sourceFPS`), the `MediaSource`
//! tagged enum, omitted optional fields, the legacy `Transform` `x`/`y` keys,
//! a `MediaManifest` with no `version`, and a `GenerationLogEntry` carrying the
//! legacy dollar `cost`. Opening such a bundle must reconstruct the correct
//! `Timeline`, `MediaManifest`, and `GenerationLog`.

mod common;

use opentake_domain::{ClipType, MediaSource};
use opentake_project::{Project, ProjectError};
use serde_json::{json, Value};

use common::{write_file, TempDir};

/// An upstream-style `project.json`: a configured 1080p/30 timeline with one
/// video track (a trimmed clip carrying legacy `x`/`y` transform keys, plus a
/// text clip) and one audio track. Most optional clip fields are omitted to
/// exercise the `#[serde(default)]` fallbacks.
const UPSTREAM_PROJECT_JSON: &str = r#"
{
  "fps": 30,
  "width": 1920,
  "height": 1080,
  "settingsConfigured": true,
  "tracks": [
    {
      "id": "11111111-1111-1111-1111-111111111111",
      "type": "video",
      "syncLocked": true,
      "clips": [
        {
          "id": "clip-a",
          "mediaRef": "media-1",
          "mediaType": "video",
          "startFrame": 0,
          "durationFrames": 90,
          "trimStartFrame": 12,
          "speed": 2.0,
          "volume": 0.5,
          "transform": { "x": 0.1, "y": 0.2, "width": 0.5, "height": 0.5 }
        },
        {
          "id": "clip-text",
          "mediaRef": "",
          "mediaType": "text",
          "startFrame": 90,
          "durationFrames": 60,
          "textContent": "Hello",
          "textStyle": {
            "fontName": "Helvetica",
            "fontSize": 48,
            "alignment": "center"
          }
        }
      ]
    },
    {
      "id": "22222222-2222-2222-2222-222222222222",
      "type": "audio",
      "muted": true,
      "clips": [
        {
          "id": "clip-music",
          "mediaRef": "media-2",
          "mediaType": "audio",
          "startFrame": 0,
          "durationFrames": 300
        }
      ]
    }
  ]
}
"#;

/// An upstream-style `media.json` with **no** `version` key (must fall back to
/// 1), one internal and one external source, and abbreviation casings.
const UPSTREAM_MEDIA_JSON: &str = r#"
{
  "entries": [
    {
      "id": "media-1",
      "name": "shot.mov",
      "type": "video",
      "source": { "project": { "relativePath": "media/shot.mov" } },
      "duration": 3.0,
      "sourceWidth": 1920,
      "sourceHeight": 1080,
      "sourceFPS": 29.97,
      "hasAudio": true
    },
    {
      "id": "media-2",
      "name": "track.mp3",
      "type": "audio",
      "source": { "external": { "absolutePath": "/Music/track.mp3" } },
      "duration": 200.0
    }
  ],
  "folders": []
}
"#;

/// An upstream-style `generation-log.json` with no top-level `version` (→ 1)
/// and a row that uses the legacy dollar `cost` instead of `costCredits`.
const UPSTREAM_GEN_LOG_JSON: &str = r#"
{
  "entries": [
    {
      "id": "gen-legacy",
      "model": "veo-2",
      "cost": 0.42,
      "createdAt": 700000000.0
    },
    {
      "model": "veo-3",
      "costCredits": 300
    }
  ]
}
"#;

fn make_upstream_bundle(tag: &str) -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new(tag);
    let bundle = tmp.child("Upstream.opentake");
    std::fs::create_dir_all(&bundle).unwrap();
    write_file(
        &bundle.join("project.json"),
        UPSTREAM_PROJECT_JSON.as_bytes(),
    );
    write_file(&bundle.join("media.json"), UPSTREAM_MEDIA_JSON.as_bytes());
    write_file(
        &bundle.join("generation-log.json"),
        UPSTREAM_GEN_LOG_JSON.as_bytes(),
    );
    (tmp, bundle)
}

fn make_json_bundle(
    tag: &str,
    timeline: &Value,
    manifest: &Value,
    generation_log: Option<&Value>,
) -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new(tag);
    let bundle = tmp.child("Matrix.opentake");
    std::fs::create_dir_all(&bundle).unwrap();
    write_file(
        &bundle.join("project.json"),
        &serde_json::to_vec_pretty(timeline).unwrap(),
    );
    write_file(
        &bundle.join("media.json"),
        &serde_json::to_vec_pretty(manifest).unwrap(),
    );
    if let Some(log) = generation_log {
        write_file(
            &bundle.join("generation-log.json"),
            &serde_json::to_vec_pretty(log).unwrap(),
        );
    }
    (tmp, bundle)
}

fn assert_uuid_v4(id: &str) {
    assert_eq!(id.len(), 36, "UUID length: {id}");
    assert_eq!(&id[8..9], "-");
    assert_eq!(&id[13..14], "-");
    assert_eq!(&id[18..19], "-");
    assert_eq!(&id[23..24], "-");
    assert_eq!(&id[14..15], "4", "UUID version: {id}");
    assert!(
        matches!(&id[19..20], "8" | "9" | "a" | "b"),
        "UUID variant: {id}"
    );
    assert!(
        id.chars()
            .enumerate()
            .all(|(index, ch)| matches!(index, 8 | 13 | 18 | 23) || ch.is_ascii_hexdigit()),
        "UUID characters: {id}"
    );
}

#[test]
fn parses_upstream_timeline_structure() {
    let (_tmp, bundle) = make_upstream_bundle("compat-timeline");
    let project = Project::open(&bundle).expect("open upstream bundle");
    let tl = &project.timeline;

    assert_eq!(tl.fps, 30);
    assert_eq!(tl.width, 1920);
    assert_eq!(tl.height, 1080);
    assert!(tl.settings_configured);
    assert_eq!(tl.tracks.len(), 2);

    let video = &tl.tracks[0];
    assert_eq!(video.kind, ClipType::Video);
    assert_eq!(video.id, "11111111-1111-1111-1111-111111111111");
    assert!(video.sync_locked);
    assert!(!video.muted);
    assert_eq!(video.clips.len(), 2);

    let audio = &tl.tracks[1];
    assert_eq!(audio.kind, ClipType::Audio);
    assert!(audio.muted);
    // sync_locked defaults to true even though the key was omitted.
    assert!(audio.sync_locked);
    assert_eq!(audio.clips.len(), 1);
    assert_eq!(audio.clips[0].duration_frames, 300);
}

#[test]
fn applies_clip_defaults_for_omitted_fields() {
    let (_tmp, bundle) = make_upstream_bundle("compat-defaults");
    let project = Project::open(&bundle).unwrap();
    let clip_a = &project.timeline.tracks[0].clips[0];

    // Present fields decoded as written.
    assert_eq!(clip_a.id, "clip-a");
    assert_eq!(clip_a.media_ref, "media-1");
    assert_eq!(clip_a.media_type, ClipType::Video);
    assert_eq!(clip_a.start_frame, 0);
    assert_eq!(clip_a.duration_frames, 90);
    assert_eq!(clip_a.trim_start_frame, 12);
    assert_eq!(clip_a.speed, 2.0);
    assert_eq!(clip_a.volume, 0.5);

    // Omitted fields fall back to upstream defaults.
    assert_eq!(clip_a.trim_end_frame, 0);
    assert_eq!(clip_a.opacity, 1.0);
    assert_eq!(clip_a.fade_in_frames, 0);
    assert!(clip_a.opacity_track.is_none());
    assert!(clip_a.link_group_id.is_none());

    // end_frame derives correctly from the decoded values.
    assert_eq!(clip_a.end_frame(), 90);
    // source_frames_consumed = round(90 * 2.0) = 180.
    assert_eq!(clip_a.source_frames_consumed(), 180);
}

#[test]
fn migrates_legacy_transform_xy_to_center() {
    let (_tmp, bundle) = make_upstream_bundle("compat-transform");
    let project = Project::open(&bundle).unwrap();
    let clip_a = &project.timeline.tracks[0].clips[0];
    let t = &clip_a.transform;

    // Upstream migration: center = old_xy + size - 0.5.
    // x=0.1, width=0.5 -> center_x = 0.1 + 0.5 - 0.5 = 0.1
    // y=0.2, height=0.5 -> center_y = 0.2 + 0.5 - 0.5 = 0.2
    assert!((t.center_x - 0.1).abs() < 1e-9, "center_x = {}", t.center_x);
    assert!((t.center_y - 0.2).abs() < 1e-9, "center_y = {}", t.center_y);
    assert!((t.width - 0.5).abs() < 1e-9);
    assert!((t.height - 0.5).abs() < 1e-9);
}

#[test]
fn parses_text_clip() {
    let (_tmp, bundle) = make_upstream_bundle("compat-text");
    let project = Project::open(&bundle).unwrap();
    let text_clip = &project.timeline.tracks[0].clips[1];

    assert_eq!(text_clip.media_type, ClipType::Text);
    assert_eq!(text_clip.text_content.as_deref(), Some("Hello"));
    assert!(text_clip.text_style.is_some());
}

#[test]
fn parses_manifest_with_missing_version_and_tagged_sources() {
    let (_tmp, bundle) = make_upstream_bundle("compat-manifest");
    let project = Project::open(&bundle).unwrap();
    let m = &project.manifest;

    // Missing version falls back to 1 (NOT the struct default of 2).
    assert_eq!(m.version, 1);
    assert_eq!(m.entries.len(), 2);

    let internal = &m.entries[0];
    assert_eq!(internal.id, "media-1");
    assert_eq!(internal.kind, ClipType::Video);
    assert_eq!(internal.source_fps, Some(29.97));
    assert_eq!(internal.has_audio, Some(true));
    assert_eq!(
        internal.source,
        MediaSource::Project {
            relative_path: "media/shot.mov".into()
        }
    );

    let external = &m.entries[1];
    assert_eq!(external.kind, ClipType::Audio);
    assert_eq!(
        external.source,
        MediaSource::External {
            absolute_path: "/Music/track.mp3".into()
        }
    );
    // Omitted optional fields are None.
    assert!(external.source_fps.is_none());
    assert!(external.has_audio.is_none());
}

#[test]
fn migrates_generation_log_legacy_cost_and_version() {
    let (_tmp, bundle) = make_upstream_bundle("compat-genlog");
    let project = Project::open(&bundle).unwrap();
    let log = project.generation_log.expect("generation log present");

    // Missing top-level version -> 1.
    assert_eq!(log.version, 1);
    assert_eq!(log.entries.len(), 2);

    // Legacy dollar cost 0.42 -> ceil(42.0) = 42 credits.
    let legacy = &log.entries[0];
    assert_eq!(legacy.id, "gen-legacy");
    assert_eq!(legacy.model, "veo-2");
    assert_eq!(legacy.cost_credits, Some(42));
    assert_eq!(legacy.created_at, Some(700_000_000.0));

    // New-style row keeps its costCredits; missing id is synthesized once at
    // the bundle boundary, matching upstream's UUID fallback.
    let modern = &log.entries[1];
    assert!(!modern.id.is_empty());
    assert_eq!(modern.cost_credits, Some(300));
    assert!(modern.created_at.is_none());

    assert_eq!(log.total_credits(), 342);
}

#[test]
fn reopen_after_resave_keeps_upstream_values() {
    // Open an upstream bundle, save it back in OpenTake's format, and confirm
    // the values survive the round-trip through our encoder.
    let (_tmp, bundle) = make_upstream_bundle("compat-resave");
    let project = Project::open(&bundle).unwrap();
    project.save().unwrap();

    let reopened = Project::open(&bundle).unwrap();
    assert_eq!(reopened.timeline, project.timeline);
    assert_eq!(reopened.manifest, project.manifest);
    assert_eq!(reopened.generation_log, project.generation_log);
}

#[test]
fn exhaustive_legacy_default_matrix() {
    // Current schema: every persisted Timeline/Track/Clip/Manifest/GenerationLog
    // branch survives a real edit, save, and reopen through Project.
    let current_timeline = json!({
        "fps": 24,
        "width": 3840,
        "height": 2160,
        "settingsConfigured": true,
        "tracks": [{
            "id": "track-current",
            "type": "video",
            "muted": true,
            "hidden": true,
            "syncLocked": false,
            "clips": [{
                "id": "clip-current",
                "mediaRef": "asset-current",
                "mediaType": "image",
                "sourceClipType": "video",
                "startFrame": 12,
                "durationFrames": 48,
                "trimStartFrame": 2,
                "trimEndFrame": 3,
                "speed": 1.25,
                "volume": 0.75,
                "fadeInFrames": 4,
                "fadeOutFrames": 5,
                "fadeInInterpolation": "smooth",
                "fadeOutInterpolation": "hold",
                "opacity": 0.8,
                "transform": {
                    "centerX": 0.4,
                    "centerY": 0.6,
                    "width": 0.7,
                    "height": 0.8,
                    "rotation": 12.0,
                    "flipHorizontal": true,
                    "flipVertical": true
                },
                "crop": {"left": 0.1, "top": 0.2, "right": 0.3, "bottom": 0.1},
                "linkGroupId": "link-current",
                "captionGroupId": "caption-current",
                "textContent": "known text",
                "textStyle": {"fontName": "Helvetica", "fontSize": 42.0},
                "opacityTrack": {"keyframes": [{"frame": 0, "value": 0.4, "interpolationOut": "linear"}]},
                "positionTrack": {"keyframes": [{"frame": 1, "value": {"a": 0.2, "b": 0.3}, "interpolationOut": "smooth"}]},
                "scaleTrack": {"keyframes": [{"frame": 2, "value": {"a": 0.8, "b": 0.9}, "interpolationOut": "smooth"}]},
                "rotationTrack": {"keyframes": [{"frame": 3, "value": 22.0, "interpolationOut": "smooth"}]},
                "cropTrack": {"keyframes": [{"frame": 4, "value": {"left": 0.2, "top": 0.1, "right": 0.0, "bottom": 0.3}, "interpolationOut": "smooth"}]},
                "volumeTrack": {"keyframes": [{"frame": 5, "value": -6.0, "interpolationOut": "smooth"}]}
            }]
        }]
    });
    let current_manifest = json!({
        "version": 2,
        "entries": [{
            "id": "asset-current",
            "name": "current.mov",
            "type": "video",
            "source": {"project": {"relativePath": "media/current.mov"}},
            "duration": 2.0,
            "generationInput": {
                "prompt": "current prompt",
                "model": "current-model",
                "duration": 2,
                "aspectRatio": "16:9",
                "resolution": "1080p",
                "quality": "high",
                "imageURLs": ["https://example.invalid/input.png"],
                "numImages": 1,
                "voice": "voice",
                "lyrics": "lyrics",
                "styleInstructions": "style",
                "instrumental": false,
                "generateAudio": true,
                "referenceImageURLs": ["https://example.invalid/ref.png"],
                "referenceVideoURLs": ["https://example.invalid/ref.mov"],
                "referenceAudioURLs": ["https://example.invalid/ref.wav"],
                "imageURLAssetIds": ["asset-image"],
                "referenceImageAssetIds": ["asset-ref-image"],
                "referenceVideoAssetIds": ["asset-ref-video"],
                "referenceAudioAssetIds": ["asset-ref-audio"],
                "createdAt": 700000000.0
            },
            "sourceWidth": 1920,
            "sourceHeight": 1080,
            "sourceFPS": 23.976,
            "hasAudio": true,
            "folderId": "folder-current",
            "cachedRemoteURL": "https://example.invalid/current.mov",
            "cachedRemoteURLExpiresAt": 800000000.0
        }],
        "folders": [{"id": "folder-current", "name": "Current", "parentFolderId": "folder-parent"}]
    });
    let current_log = json!({
        "version": 1,
        "entries": [{
            "id": "generation-current",
            "model": "current-model",
            "costCredits": 321,
            "createdAt": 700000001.0
        }]
    });
    let (_current_tmp, current_bundle) = make_json_bundle(
        "compat-matrix-current",
        &current_timeline,
        &current_manifest,
        Some(&current_log),
    );
    let mut current = Project::open(&current_bundle).expect("open current field matrix");
    assert!(!current.compatibility().is_read_only());
    let current_clip = &current.timeline.tracks[0].clips[0];
    assert!(current_clip.opacity_track.is_some());
    assert!(current_clip.position_track.is_some());
    assert!(current_clip.scale_track.is_some());
    assert!(current_clip.rotation_track.is_some());
    assert!(current_clip.crop_track.is_some());
    assert!(current_clip.volume_track.is_some());
    assert_eq!(current.manifest.entries[0].source_fps, Some(23.976));
    assert_eq!(
        current.generation_log.as_ref().unwrap().entries[0].cost_credits,
        Some(321)
    );
    current.timeline.fps = 48;
    current.timeline.tracks[0].clips[0].volume = 0.625;
    current.save().expect("save edited current matrix");
    let current_reopened = Project::open(&current_bundle).expect("reopen edited current matrix");
    assert_eq!(current_reopened.timeline, current.timeline);
    assert_eq!(current_reopened.manifest, current.manifest);
    assert_eq!(current_reopened.generation_log, current.generation_log);

    // Missing defaultable keys: Timeline, Track, Clip, Manifest, and Log all
    // use the documented legacy fallbacks. Missing Track/Clip/Log ids are
    // synthesized once, then remain stable across save/reopen.
    let missing_timeline = json!({
        "tracks": [{
            "type": "video",
            "clips": [{
                "mediaRef": "asset-minimal",
                "startFrame": 3,
                "durationFrames": 9
            }]
        }]
    });
    let (_missing_tmp, missing_bundle) = make_json_bundle(
        "compat-matrix-missing",
        &missing_timeline,
        &json!({}),
        Some(&json!({"entries": [
            {"model": "legacy-model"},
            {"id": "", "model": "explicit-empty"}
        ]})),
    );
    let missing = Project::open(&missing_bundle).expect("open missing-field matrix");
    assert_eq!(
        (
            missing.timeline.fps,
            missing.timeline.width,
            missing.timeline.height
        ),
        (30, 1920, 1080)
    );
    assert!(!missing.timeline.settings_configured);
    let missing_track = &missing.timeline.tracks[0];
    assert!(!missing_track.id.is_empty());
    assert_uuid_v4(&missing_track.id);
    assert!(!missing_track.muted);
    assert!(!missing_track.hidden);
    assert!(missing_track.sync_locked);
    let missing_clip = &missing_track.clips[0];
    assert!(!missing_clip.id.is_empty());
    assert_uuid_v4(&missing_clip.id);
    assert_eq!(missing_clip.media_type, ClipType::Video);
    assert_eq!(missing_clip.source_clip_type, ClipType::Video);
    assert_eq!(
        (missing_clip.trim_start_frame, missing_clip.trim_end_frame),
        (0, 0)
    );
    assert_eq!(
        (
            missing_clip.speed,
            missing_clip.volume,
            missing_clip.opacity
        ),
        (1.0, 1.0, 1.0)
    );
    assert_eq!(
        (missing_clip.fade_in_frames, missing_clip.fade_out_frames),
        (0, 0)
    );
    assert_eq!(missing_clip.transform, Default::default());
    assert_eq!(missing_clip.crop, Default::default());
    assert!(missing_clip.link_group_id.is_none());
    assert!(missing_clip.caption_group_id.is_none());
    assert!(missing_clip.text_content.is_none());
    assert!(missing_clip.text_style.is_none());
    assert!(missing_clip.opacity_track.is_none());
    assert!(missing_clip.position_track.is_none());
    assert!(missing_clip.scale_track.is_none());
    assert!(missing_clip.rotation_track.is_none());
    assert!(missing_clip.crop_track.is_none());
    assert!(missing_clip.volume_track.is_none());
    assert_eq!(missing.manifest.version, 1);
    assert!(missing.manifest.entries.is_empty());
    assert!(missing.manifest.folders.is_empty());
    let missing_log = missing.generation_log.as_ref().unwrap();
    assert_eq!(missing_log.version, 1);
    assert!(!missing_log.entries[0].id.is_empty());
    assert_uuid_v4(&missing_log.entries[0].id);
    assert_eq!(missing_log.entries[1].id, "");
    missing.save().expect("save defaulted matrix");
    let missing_reopened = Project::open(&missing_bundle).expect("reopen defaulted matrix");
    assert_eq!(missing_reopened.timeline, missing.timeline);
    assert_eq!(missing_reopened.manifest, missing.manifest);
    assert_eq!(missing_reopened.generation_log, missing.generation_log);

    let batch_clips = (0..128)
        .map(|index| {
            json!({
                "mediaRef": format!("batch-{index}"),
                "startFrame": index,
                "durationFrames": 1
            })
        })
        .collect::<Vec<_>>();
    let (_batch_tmp, batch_bundle) = make_json_bundle(
        "compat-matrix-id-batch",
        &json!({"tracks": [{"type": "video", "clips": batch_clips}]}),
        &json!({}),
        None,
    );
    let batch = Project::open(&batch_bundle).expect("synthesize batch UUIDs");
    let ids = batch.timeline.tracks[0]
        .clips
        .iter()
        .map(|clip| clip.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(ids.len(), 128);
    ids.iter().for_each(|id| assert_uuid_v4(id));

    let (_empty_id_tmp, empty_id_bundle) = make_json_bundle(
        "compat-matrix-explicit-empty-id",
        &json!({"tracks": [{
            "id": "",
            "type": "video",
            "clips": [{"id": "", "mediaRef": "empty", "startFrame": 0, "durationFrames": 1}]
        }]}),
        &json!({}),
        None,
    );
    let empty_ids = Project::open(&empty_id_bundle).expect("explicit empty ids are valid strings");
    assert_eq!(empty_ids.timeline.tracks[0].id, "");
    assert_eq!(empty_ids.timeline.tracks[0].clips[0].id, "");
    empty_ids.save().expect("save explicit empty ids");
    let empty_ids_reopened = Project::open(&empty_id_bundle).expect("reopen explicit empty ids");
    assert_eq!(empty_ids_reopened.timeline.tracks[0].id, "");
    assert_eq!(empty_ids_reopened.timeline.tracks[0].clips[0].id, "");

    // Legacy aliases still migrate, then persist in their current form.
    let (_legacy_tmp, legacy_bundle) = make_upstream_bundle("compat-matrix-legacy");
    let legacy = Project::open(&legacy_bundle).expect("open legacy aliases");
    assert!((legacy.timeline.tracks[0].clips[0].transform.center_x - 0.1).abs() < 1e-9);
    assert_eq!(
        legacy.generation_log.as_ref().unwrap().entries[0].cost_credits,
        Some(42)
    );
    legacy.save().expect("save migrated aliases");
    let legacy_reopened = Project::open(&legacy_bundle).expect("reopen migrated aliases");
    assert_eq!(legacy_reopened.timeline, legacy.timeline);
    assert_eq!(legacy_reopened.generation_log, legacy.generation_log);

    // Upstream's tolerant Track/Clip decoders default malformed *optional*
    // fields, while retaining the required mediaRef/start/duration values.
    let malformed_optional_timeline = json!({
        "tracks": [{
            "id": 7,
            "type": "video",
            "muted": "yes",
            "hidden": "bad",
            "syncLocked": null,
            "clips": [{
                "id": false,
                "mediaRef": "asset-malformed-optional",
                "mediaType": "future-kind",
                "sourceClipType": 5,
                "startFrame": 5,
                "durationFrames": 10,
                "trimStartFrame": "bad",
                "trimEndFrame": "bad",
                "speed": "fast",
                "volume": "bad",
                "fadeInFrames": false,
                "fadeOutFrames": null,
                "fadeInInterpolation": "future",
                "fadeOutInterpolation": 3,
                "opacity": "opaque",
                "transform": {"centerX": "bad"},
                "crop": {"left": "bad", "top": 0.0, "right": 0.0, "bottom": 0.0},
                "linkGroupId": 1,
                "captionGroupId": 7,
                "textContent": 7,
                "textStyle": "bad",
                "opacityTrack": false,
                "positionTrack": {"keyframes": [{"frame": 0, "value": {"a": "bad", "b": 2.0}, "interpolationOut": "smooth"}]},
                "scaleTrack": {"keyframes": [{"frame": 0, "value": {"a": 1.0}, "interpolationOut": "smooth"}]},
                "rotationTrack": {"keyframes": [{"frame": "bad", "value": 1.0, "interpolationOut": 3}]},
                "cropTrack": {"keyframes": [{"frame": 0, "value": {"left": "bad", "top": 0.0, "right": 0.0, "bottom": 0.0}, "interpolationOut": "smooth"}]},
                "volumeTrack": false
            }]
        }]
    });
    let (_malformed_optional_tmp, malformed_optional_bundle) = make_json_bundle(
        "compat-matrix-malformed-optional",
        &malformed_optional_timeline,
        &json!({}),
        None,
    );
    let malformed_optional = Project::open(&malformed_optional_bundle)
        .expect("malformed optional Track/Clip fields use upstream defaults");
    let malformed_track = &malformed_optional.timeline.tracks[0];
    assert!(!malformed_track.id.is_empty());
    assert_uuid_v4(&malformed_track.id);
    assert!(!malformed_track.muted && !malformed_track.hidden && malformed_track.sync_locked);
    let malformed_clip = &malformed_track.clips[0];
    assert!(!malformed_clip.id.is_empty());
    assert_uuid_v4(&malformed_clip.id);
    assert_eq!(malformed_clip.media_type, ClipType::Video);
    assert_eq!(malformed_clip.source_clip_type, ClipType::Video);
    assert_eq!(
        (
            malformed_clip.trim_start_frame,
            malformed_clip.trim_end_frame
        ),
        (0, 0)
    );
    assert_eq!(
        (
            malformed_clip.speed,
            malformed_clip.volume,
            malformed_clip.opacity
        ),
        (1.0, 1.0, 1.0)
    );
    assert_eq!(malformed_clip.transform, Default::default());
    assert_eq!(malformed_clip.crop, Default::default());
    assert!(malformed_clip.opacity_track.is_none());
    assert!(malformed_clip.position_track.is_none());
    assert!(malformed_clip.scale_track.is_none());
    assert!(malformed_clip.rotation_track.is_none());
    assert!(malformed_clip.crop_track.is_none());
    assert!(
        !malformed_optional.compatibility().is_read_only(),
        "unexpected blockers: {:?}",
        malformed_optional.compatibility().blockers()
    );
    malformed_optional
        .save()
        .expect("known malformed scalar fields remain writable");
    let malformed_reopened = Project::open(&malformed_optional_bundle).unwrap();
    assert_eq!(malformed_reopened.timeline, malformed_optional.timeline);

    let (_malformed_clips_tmp, malformed_clips_bundle) = make_json_bundle(
        "compat-matrix-malformed-clips",
        &json!({"tracks": [{"type": "video", "clips": "malformed"}]}),
        &json!({}),
        None,
    );
    let malformed_clips =
        Project::open(&malformed_clips_bundle).expect("upstream defaults malformed Track.clips");
    assert!(malformed_clips.timeline.tracks[0].clips.is_empty());
    assert!(!malformed_clips.compatibility().is_read_only());
    malformed_clips
        .save()
        .expect("known malformed scalar clips remains writable");
    assert!(Project::open(&malformed_clips_bundle)
        .unwrap()
        .timeline
        .tracks[0]
        .clips
        .is_empty());

    let (_future_clips_tmp, future_clips_bundle) = make_json_bundle(
        "compat-matrix-future-clips-container",
        &json!({"tracks": [{"type": "video", "clips": {"futureContainer": true}}]}),
        &json!({}),
        None,
    );
    let future_clips = Project::open(&future_clips_bundle)
        .expect("future Track.clips container remains inspectable");
    assert!(future_clips.timeline.tracks[0].clips.is_empty());
    assert_eq!(
        future_clips.compatibility().blockers(),
        ["project.json:tracks.0.clips.futureContainer"]
    );
    assert!(matches!(
        future_clips.save(),
        Err(ProjectError::CompatibilityReadOnly { .. })
    ));

    let (_empty_future_clips_tmp, empty_future_clips_bundle) = make_json_bundle(
        "compat-matrix-empty-future-clips-container",
        &json!({"tracks": [{"type": "video", "clips": {}}]}),
        &json!({}),
        None,
    );
    let empty_future_clips = Project::open(&empty_future_clips_bundle)
        .expect("empty structured Track.clips container remains inspectable");
    assert_eq!(
        empty_future_clips.compatibility().blockers(),
        ["project.json:tracks.0.clips"]
    );

    let (_dropped_future_tmp, dropped_future_bundle) = make_json_bundle(
        "compat-matrix-dropped-clips-future",
        &json!({"tracks": [{"type": "video", "clips": [
            {
                "id": "valid",
                "mediaRef": "asset",
                "startFrame": 0,
                "durationFrames": 1,
                "effects": [{"name": "blur", "params": {}, "enabled": true, "futureEffect": true}]
            },
            {"id": "required-field-malformed", "startFrame": 1, "durationFrames": 1}
        ]}]}),
        &json!({}),
        None,
    );
    let dropped_future =
        Project::open(&dropped_future_bundle).expect("failed clips array remains inspectable");
    assert!(dropped_future.timeline.tracks[0].clips.is_empty());
    assert_eq!(
        dropped_future.compatibility().blockers(),
        [
            "project.json:tracks.0.clips.0.effects.0.futureEffect",
            "project.json:tracks.0.clips:invalid-or-unreadable",
        ]
    );

    // Malformed required project/manifest data remains a strict open error.
    let (_bad_timeline_tmp, bad_timeline_bundle) = make_json_bundle(
        "compat-matrix-bad-timeline",
        &json!({"fps": "thirty", "tracks": []}),
        &json!({}),
        None,
    );
    assert!(matches!(
        Project::open(&bad_timeline_bundle),
        Err(ProjectError::Json { ref file, .. }) if file == "project.json"
    ));
    let (_bad_manifest_tmp, bad_manifest_bundle) = make_json_bundle(
        "compat-matrix-bad-manifest",
        &json!({"tracks": []}),
        &json!({"entries": "not-an-array"}),
        None,
    );
    assert!(matches!(
        Project::open(&bad_manifest_bundle),
        Err(ProjectError::Json { ref file, .. }) if file == "media.json"
    ));

    // generation-log.json is the one lenient component: malformed bytes keep
    // the project readable but make every project write fail closed.
    let (_bad_log_tmp, bad_log_bundle) = make_json_bundle(
        "compat-matrix-bad-log",
        &json!({"tracks": []}),
        &json!({}),
        None,
    );
    write_file(&bad_log_bundle.join("generation-log.json"), b"not-json");
    let bad_log = Project::open(&bad_log_bundle).expect("malformed optional log opens");
    assert!(bad_log.generation_log.is_none());
    assert_eq!(
        bad_log.compatibility().blockers(),
        ["generation-log.json:invalid-or-unreadable"]
    );
    assert!(matches!(
        bad_log.save(),
        Err(ProjectError::CompatibilityReadOnly { .. })
    ));

    for (case, malformed_entry) in [
        ("id", json!({"id": 7, "model": "m"})),
        ("model", json!({"id": "g", "model": 7})),
        (
            "cost-credits",
            json!({"id": "g", "model": "m", "costCredits": "7", "cost": 1.0}),
        ),
        (
            "created-at",
            json!({"id": "g", "model": "m", "createdAt": {"futureDate": true}}),
        ),
        (
            "legacy-cost",
            json!({"id": "g", "model": "m", "cost": "1.0"}),
        ),
    ] {
        let (_tmp, bundle) = make_json_bundle(
            &format!("compat-matrix-bad-log-{case}"),
            &json!({"tracks": []}),
            &json!({}),
            Some(&json!({"entries": [malformed_entry]})),
        );
        let project = Project::open(&bundle).expect("malformed generation row is leniently opened");
        assert!(project.generation_log.is_none(), "case {case}");
        assert_eq!(
            project.compatibility().blockers(),
            ["generation-log.json:invalid-or-unreadable"],
            "case {case}"
        );
    }

    // Unknown future fields are readable for inspection, but an in-memory edit
    // must never authorize an unsafe same-path or Save As write.
    let future_timeline = json!({
        "fps": 30,
        "tracks": [{
            "id": "future-track",
            "type": "video",
            "clips": [{
                "id": "future-clip",
                "mediaRef": "future-asset",
                "startFrame": 0,
                "durationFrames": 30,
                "transform": {"centerX": {"futureCenter": true}, "futureTransform": true},
                "crop": {"left": [0.1], "top": 0.0, "right": 0.0, "bottom": 0.0},
                "positionTrack": {
                    "keyframes": [{"frame": 0, "value": {"a": {"futureA": true}, "b": 0.0, "futureValue": true}, "interpolationOut": "smooth"}],
                    "futureKeyframeTrack": true
                },
                "cropTrack": {
                    "keyframes": [{"frame": 0, "value": {"left": {"futureLeft": true}, "top": 0.0, "right": 0.0, "bottom": 0.0}, "interpolationOut": "smooth"}]
                },
                "scaleTrack": {"keyframes": {}},
                "opacityTrack": {
                    "keyframes": [{"frame": {"futureFrame": true}, "value": {"futureScalar": true}, "interpolationOut": ["smooth"]}]
                },
                "volumeTrack": {
                    "keyframes": [{"frame": 0, "value": [1.0, 2.0], "interpolationOut": {"futureInterpolation": true}}]
                },
                "futureClip": {"retained": true}
            }, {
                "id": "future-crop-object",
                "mediaRef": "future-asset",
                "startFrame": 30,
                "durationFrames": 30,
                "crop": {"left": {"futureObject": true}, "top": 0.0, "right": 0.0, "bottom": 0.0}
            }],
            "futureTrack": {"retained": true}
        }],
        "futureTimeline": {"retained": true}
    });
    let future_manifest = json!({
        "version": 2,
        "entries": [{
            "id": "future-asset",
            "name": "future.mov",
            "type": "video",
            "source": {"project": {"relativePath": "media/future.mov", "futureSource": true}},
            "duration": 1.0,
            "futureEntry": {"retained": true}
        }],
        "folders": [],
        "futureManifest": {"retained": true}
    });
    let future_log = json!({
        "version": 1,
        "entries": [{
            "id": "future-generation",
            "model": "future-model",
            "futureGenerationEntry": {"retained": true}
        }],
        "futureGenerationLog": {"retained": true}
    });
    let (future_tmp, future_bundle) = make_json_bundle(
        "compat-matrix-future",
        &future_timeline,
        &future_manifest,
        Some(&future_log),
    );
    let component_paths =
        ["project.json", "media.json", "generation-log.json"].map(|name| future_bundle.join(name));
    let before = component_paths
        .iter()
        .map(|path| std::fs::read(path).unwrap())
        .collect::<Vec<_>>();
    let mut future = Project::open(&future_bundle).expect("future fields remain inspectable");
    assert_eq!(future.timeline.tracks[0].clips[0].media_ref, "future-asset");
    assert_eq!(
        future.compatibility().blockers(),
        [
            "generation-log.json:entries.0.futureGenerationEntry",
            "generation-log.json:futureGenerationLog",
            "media.json:entries.0.futureEntry",
            "media.json:entries.0.source.project.futureSource",
            "media.json:futureManifest",
            "project.json:futureTimeline",
            "project.json:tracks.0.clips.0.crop.left",
            "project.json:tracks.0.clips.0.cropTrack.keyframes.0.value.left.futureLeft",
            "project.json:tracks.0.clips.0.futureClip",
            "project.json:tracks.0.clips.0.opacityTrack.keyframes.0.frame.futureFrame",
            "project.json:tracks.0.clips.0.opacityTrack.keyframes.0.interpolationOut",
            "project.json:tracks.0.clips.0.opacityTrack.keyframes.0.value.futureScalar",
            "project.json:tracks.0.clips.0.positionTrack.futureKeyframeTrack",
            "project.json:tracks.0.clips.0.positionTrack.keyframes.0.value.a.futureA",
            "project.json:tracks.0.clips.0.positionTrack.keyframes.0.value.futureValue",
            "project.json:tracks.0.clips.0.scaleTrack.keyframes",
            "project.json:tracks.0.clips.0.transform.centerX.futureCenter",
            "project.json:tracks.0.clips.0.transform.futureTransform",
            "project.json:tracks.0.clips.0.volumeTrack.keyframes.0.interpolationOut.futureInterpolation",
            "project.json:tracks.0.clips.0.volumeTrack.keyframes.0.value",
            "project.json:tracks.0.clips.1.crop.left.futureObject",
            "project.json:tracks.0.futureTrack",
        ]
    );
    future.timeline.fps = 60;
    assert!(matches!(
        future.save(),
        Err(ProjectError::CompatibilityReadOnly { .. })
    ));
    let save_as = future_tmp.child("Rejected-Future-Save-As.opentake");
    assert!(matches!(
        future.save_to(&save_as),
        Err(ProjectError::CompatibilityReadOnly { .. })
    ));
    assert!(!save_as.exists());
    for (path, expected) in component_paths.iter().zip(before) {
        assert_eq!(std::fs::read(path).unwrap(), expected);
    }
}
