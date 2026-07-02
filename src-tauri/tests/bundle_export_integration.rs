//! Temp-dir integration test for the self-contained `.opentake` bundle export
//! spine (`export::run_bundle_export`).
//!
//! Unlike the video export, bundling is pure file collection — no GPU, no
//! ffmpeg — so this test always runs. It builds a two-clip timeline whose
//! manifest references one on-disk external file and one missing external file,
//! runs the orchestrator, and asserts:
//!  - the destination bundle is created with `project.json` / `media.json`,
//!  - the resolvable media is copied into the bundle's `media/` dir,
//!  - the report lists the resolvable asset as `collected` and the missing one
//!    under `missing` (the exact data the export dialog surfaces).
//!
//! Drives the Tauri-decoupled `run_bundle_export` directly (the same seam the
//! video export's `run_export` uses), so no live `AppCore`/Tauri handle is
//! needed.

use std::fs;

use opentake_domain::{
    Clip, ClipType, MediaManifest, MediaManifestEntry, MediaSource, Timeline, Track,
};
use opentake_project::GenerationLog;
use opentake_tauri_lib::export::run_bundle_export;

/// One external manifest entry pointing at `absolute_path` (which may or may not
/// exist on disk — a non-existent path exercises the missing-media path).
fn external_entry(id: &str, name: &str, absolute_path: &str) -> MediaManifestEntry {
    MediaManifestEntry {
        id: id.into(),
        name: name.into(),
        kind: ClipType::Video,
        source: MediaSource::External {
            absolute_path: absolute_path.into(),
        },
        duration: 1.0,
        generation_input: None,
        source_width: Some(320),
        source_height: Some(240),
        source_fps: Some(30.0),
        has_audio: Some(false),
        folder_id: None,
        cached_remote_url: None,
        cached_remote_url_expires_at: None,
    }
}

#[test]
fn run_bundle_export_collects_present_media_and_reports_missing() {
    let dir = tempfile::tempdir().unwrap();

    // One real external source on disk...
    let present = dir.path().join("present.mp4");
    fs::write(&present, b"fake-mp4-bytes").unwrap();
    // ...and one that does not exist (→ reported missing, kept as dangling ref).
    let absent = dir.path().join("gone.mp4");

    let dest = dir.path().join("Bundle.opentake");

    // Timeline: one video track with a clip per asset.
    let mut tl = Timeline::new();
    let mut track = Track::new("t1", ClipType::Video);
    track
        .clips
        .push(Clip::new("clip-present", "asset-present", 0, 30));
    track
        .clips
        .push(Clip::new("clip-absent", "asset-absent", 30, 30));
    tl.tracks.push(track);

    let mut manifest = MediaManifest::new();
    manifest.entries.push(external_entry(
        "asset-present",
        "present.mp4",
        &present.to_string_lossy(),
    ));
    manifest.entries.push(external_entry(
        "asset-absent",
        "gone.mp4",
        &absent.to_string_lossy(),
    ));

    let log = GenerationLog::new();

    // `None` source bundle → unsaved project; only `.external` media resolves,
    // which is exactly this fixture (matches upstream's optional sourceProjectURL).
    let report = run_bundle_export(
        &tl,
        &manifest,
        &log,
        None,
        dest.to_string_lossy().into_owned(),
    )
    .expect("bundle export should succeed");

    // The bundle exists with its core JSON documents.
    assert!(dest.is_dir(), "destination bundle dir should exist");
    assert!(
        dest.join("project.json").is_file(),
        "project.json should be written"
    );
    assert!(
        dest.join("media.json").is_file(),
        "media.json should be written"
    );

    // The resolvable external asset is collected and physically copied in.
    assert_eq!(report.out_path, dest.to_string_lossy());
    assert_eq!(
        report.collected,
        vec!["asset-present".to_string()],
        "the on-disk external asset should be collected"
    );
    assert!(report.total_bytes > 0, "some bytes should have been copied");
    let media_dir = dest.join("media");
    assert!(media_dir.is_dir(), "media/ dir should exist in the bundle");
    let copied: Vec<_> = fs::read_dir(&media_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(copied.len(), 1, "exactly one media file should be bundled");

    // The missing external asset is reported (not silently dropped), with the id
    // + display name the dialog lists.
    assert_eq!(report.missing.len(), 1, "one asset should be missing");
    assert_eq!(report.missing[0].id, "asset-absent");
    assert_eq!(report.missing[0].name, "gone.mp4");
}
