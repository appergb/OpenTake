//! Round-trip persistence: a project saved to disk and reopened must come back
//! byte-for-value identical, and the bundle must have the expected layout.

mod common;

use std::path::Path;

use opentake_domain::{
    Clip, ClipType, MediaManifest, MediaManifestEntry, MediaSource, Timeline, Track,
};
use opentake_project::{GenerationLog, GenerationLogEntry, Project};

use common::TempDir;

/// Build a non-trivial project: configured timeline, two tracks (video + audio),
/// a couple of clips, a manifest with both an internal and external source, a
/// folder, and a generation log.
fn sample_project(bundle: &Path) -> Project {
    let mut timeline = Timeline::new();
    timeline.fps = 24;
    timeline.width = 3840;
    timeline.height = 2160;
    timeline.settings_configured = true;

    let mut video = Track::new("track-video", ClipType::Video);
    let mut clip = Clip::new("clip-1", "asset-internal", 0, 120);
    clip.trim_start_frame = 5;
    clip.volume = 0.8;
    clip.fade_in_frames = 6;
    video.clips.push(clip);
    video
        .clips
        .push(Clip::new("clip-2", "asset-external", 120, 90));

    let mut audio = Track::new("track-audio", ClipType::Audio);
    audio
        .clips
        .push(Clip::new("clip-3", "asset-internal", 0, 200));
    audio.muted = true;

    timeline.tracks.push(video);
    timeline.tracks.push(audio);

    let mut manifest = MediaManifest::new();
    manifest.entries.push(MediaManifestEntry {
        id: "asset-internal".into(),
        name: "internal.mov".into(),
        kind: ClipType::Video,
        source: MediaSource::Project {
            relative_path: "media/internal.mov".into(),
        },
        duration: 5.0,
        generation_input: None,
        source_width: Some(3840),
        source_height: Some(2160),
        source_fps: Some(24.0),
        has_audio: Some(true),
        folder_id: Some("folder-1".into()),
        cached_remote_url: None,
        cached_remote_url_expires_at: None,
    });
    manifest.entries.push(MediaManifestEntry {
        id: "asset-external".into(),
        name: "external.mp4".into(),
        kind: ClipType::Video,
        source: MediaSource::External {
            absolute_path: "/somewhere/external.mp4".into(),
        },
        duration: 3.0,
        generation_input: None,
        source_width: None,
        source_height: None,
        source_fps: None,
        has_audio: None,
        folder_id: None,
        cached_remote_url: None,
        cached_remote_url_expires_at: None,
    });
    manifest
        .folders
        .push(opentake_domain::MediaFolder::new("folder-1", "B-Roll"));

    let generation_log = GenerationLog {
        version: 1,
        entries: vec![GenerationLogEntry::new(
            "gen-1",
            "veo-3",
            Some(250),
            Some(700_000_000.0),
        )],
    };

    Project {
        bundle_path: bundle.to_path_buf(),
        timeline,
        manifest,
        generation_log: Some(generation_log),
        thumbnail: Some(b"\xff\xd8\xff\xe0JPEGDATA".to_vec()),
    }
}

#[test]
fn save_then_open_is_lossless() {
    let tmp = TempDir::new("roundtrip");
    let bundle = tmp.child("Demo.opentake");
    let project = sample_project(&bundle);

    project.save().expect("save project");

    // Bundle layout on disk.
    assert!(bundle.join("project.json").is_file());
    assert!(bundle.join("media.json").is_file());
    assert!(bundle.join("generation-log.json").is_file());
    assert!(bundle.join("thumbnail.jpg").is_file());

    let reopened = Project::open(&bundle).expect("open project");

    assert_eq!(reopened.timeline, project.timeline);
    assert_eq!(reopened.manifest, project.manifest);
    assert_eq!(reopened.generation_log, project.generation_log);
    // Thumbnail is not loaded back into memory by `open` (left on disk).
    assert!(reopened.thumbnail.is_none());
    let thumb = std::fs::read(bundle.join("thumbnail.jpg")).unwrap();
    assert_eq!(thumb, b"\xff\xd8\xff\xe0JPEGDATA");
}

#[test]
fn timeline_json_uses_upstream_camel_case_keys() {
    let tmp = TempDir::new("keys");
    let bundle = tmp.child("Keys.opentake");
    sample_project(&bundle).save().unwrap();

    let timeline_json = std::fs::read_to_string(bundle.join("project.json")).unwrap();
    // Keys must match Swift's JSONEncoder output so upstream can read them.
    assert!(timeline_json.contains("\"settingsConfigured\""));
    assert!(timeline_json.contains("\"syncLocked\""));
    assert!(timeline_json.contains("\"mediaRef\""));
    assert!(timeline_json.contains("\"durationFrames\""));
    assert!(
        timeline_json.contains("\"type\": \"video\"")
            || timeline_json.contains("\"type\":\"video\"")
    );

    let manifest_json = std::fs::read_to_string(bundle.join("media.json")).unwrap();
    // MediaSource is a tagged enum; abbreviation casings are preserved.
    assert!(manifest_json.contains("\"project\""));
    assert!(manifest_json.contains("\"relativePath\""));
    assert!(manifest_json.contains("\"external\""));
    assert!(manifest_json.contains("\"absolutePath\""));
    assert!(manifest_json.contains("\"sourceFPS\""));
}

#[test]
fn open_missing_timeline_errors() {
    let tmp = TempDir::new("missing");
    let bundle = tmp.child("Empty.opentake");
    std::fs::create_dir_all(&bundle).unwrap();
    // Only media.json present, no project.json.
    common::write_file(&bundle.join("media.json"), b"{}");

    let err = Project::open(&bundle).unwrap_err();
    assert!(
        matches!(err, opentake_project::ProjectError::MissingTimeline { .. }),
        "expected MissingTimeline, got {err:?}"
    );
}

#[test]
fn open_non_directory_errors() {
    let tmp = TempDir::new("notdir");
    let file = tmp.child("not-a-bundle");
    common::write_file(&file, b"hi");
    let err = Project::open(&file).unwrap_err();
    assert!(matches!(err, opentake_project::ProjectError::NotABundle(_)));
}

#[test]
fn missing_manifest_defaults_to_empty() {
    let tmp = TempDir::new("nomanifest");
    let bundle = tmp.child("NoManifest.opentake");
    std::fs::create_dir_all(&bundle).unwrap();
    common::write_file(&bundle.join("project.json"), br#"{"fps":30,"tracks":[]}"#);

    let project = Project::open(&bundle).unwrap();
    assert!(project.manifest.entries.is_empty());
    assert!(project.manifest.folders.is_empty());
    assert!(project.generation_log.is_none());
}

#[test]
fn malformed_generation_log_is_ignored() {
    let tmp = TempDir::new("badlog");
    let bundle = tmp.child("BadLog.opentake");
    std::fs::create_dir_all(&bundle).unwrap();
    common::write_file(&bundle.join("project.json"), br#"{"tracks":[]}"#);
    common::write_file(
        &bundle.join("generation-log.json"),
        b"this is not json at all",
    );

    // Lenient: open succeeds, log degrades to None.
    let project = Project::open(&bundle).unwrap();
    assert!(project.generation_log.is_none());
}

#[test]
fn malformed_manifest_is_an_error() {
    let tmp = TempDir::new("badmanifest");
    let bundle = tmp.child("BadManifest.opentake");
    std::fs::create_dir_all(&bundle).unwrap();
    common::write_file(&bundle.join("project.json"), br#"{"tracks":[]}"#);
    common::write_file(&bundle.join("media.json"), b"{ not valid json ");

    let err = Project::open(&bundle).unwrap_err();
    assert!(
        matches!(err, opentake_project::ProjectError::Json { ref file, .. } if file == "media.json"),
        "expected Json error for media.json, got {err:?}"
    );
}

#[test]
fn save_overwrites_existing_bundle_atomically() {
    let tmp = TempDir::new("overwrite");
    let bundle = tmp.child("Over.opentake");

    let mut first = sample_project(&bundle);
    first.timeline.fps = 24;
    first.save().unwrap();

    // Save again with a changed timeline; must replace cleanly.
    let mut second = Project::open(&bundle).unwrap();
    second.timeline.fps = 60;
    second.timeline.tracks.clear();
    second.save().unwrap();

    let reopened = Project::open(&bundle).unwrap();
    assert_eq!(reopened.timeline.fps, 60);
    assert!(reopened.timeline.tracks.is_empty());
    // No stray temp files left behind.
    let stray: Vec<_> = std::fs::read_dir(&bundle)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
        .collect();
    assert!(stray.is_empty(), "temp files left behind: {stray:?}");
}

#[test]
fn save_preserves_media_and_chat_directories() {
    let tmp = TempDir::new("preserve");
    let bundle = tmp.child("Preserve.opentake");
    let project = sample_project(&bundle);
    project.save().unwrap();

    // Simulate media + chat-sessions the media/agent layers manage.
    common::write_file(&bundle.join("media").join("internal.mov"), b"MEDIA");
    common::write_file(
        &bundle.join("chat-sessions").join("s1.json"),
        br#"{"id":"s1"}"#,
    );

    // Re-saving the JSON components must not wipe those directories.
    let mut again = Project::open(&bundle).unwrap();
    again.timeline.fps = 25;
    again.save().unwrap();

    assert_eq!(
        std::fs::read(bundle.join("media").join("internal.mov")).unwrap(),
        b"MEDIA"
    );
    assert!(bundle.join("chat-sessions").join("s1.json").is_file());
}
