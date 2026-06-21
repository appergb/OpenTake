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
use opentake_project::Project;

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

    // New-style row keeps its costCredits; missing id -> empty string.
    let modern = &log.entries[1];
    assert_eq!(modern.id, "");
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
