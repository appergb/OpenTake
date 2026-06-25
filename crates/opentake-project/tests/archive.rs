//! Archiver: collect resolvable media into a self-contained bundle and rewrite
//! manifest sources to bundle-relative paths. Port-equivalence with upstream
//! `PalmierProjectExporter.export`.

mod common;

use opentake_domain::{
    Clip, ClipType, MediaManifest, MediaManifestEntry, MediaSource, Timeline, Track,
};
use opentake_project::{archive, GenerationLog, Project};

use common::{write_file, TempDir};

fn entry(id: &str, name: &str, kind: ClipType, source: MediaSource) -> MediaManifestEntry {
    MediaManifestEntry {
        id: id.into(),
        name: name.into(),
        kind,
        source,
        duration: 1.0,
        generation_input: None,
        source_width: None,
        source_height: None,
        source_fps: None,
        has_audio: None,
        folder_id: None,
        cached_remote_url: None,
        cached_remote_url_expires_at: None,
    }
}

#[test]
fn collects_external_and_internal_media_and_rewrites_sources() {
    let tmp = TempDir::new("archive");

    // Source bundle with one internal media file already in media/.
    let source_bundle = tmp.child("Source.opentake");
    write_file(
        &source_bundle.join("media").join("kept.mov"),
        b"INTERNAL-BYTES",
    );

    // An external file living outside any bundle.
    let external_path = tmp.child("ext").join("outside.mp4");
    write_file(&external_path, b"EXTERNAL-BYTES-LONGER");

    let mut timeline = Timeline::new();
    let mut track = Track::new("t1", ClipType::Video);
    track.clips.push(Clip::new("c1", "internal", 0, 30));
    track.clips.push(Clip::new("c2", "external", 30, 30));
    timeline.tracks.push(track);

    let mut manifest = MediaManifest::new();
    manifest.entries.push(entry(
        "internal",
        "kept.mov",
        ClipType::Video,
        MediaSource::Project {
            relative_path: "media/kept.mov".into(),
        },
    ));
    manifest.entries.push(entry(
        "external",
        "outside.mp4",
        ClipType::Video,
        MediaSource::External {
            absolute_path: external_path.to_string_lossy().into_owned(),
        },
    ));

    let dest_bundle = tmp.child("Dest.opentake");
    let report = archive(
        &timeline,
        &manifest,
        &GenerationLog::new(),
        Some(source_bundle.as_path()),
        &dest_bundle,
    )
    .expect("archive");

    // Report accounting.
    assert_eq!(report.collected, vec!["external".to_string()]);
    assert_eq!(report.copied_internal, 1);
    assert!(report.missing.is_empty());
    assert_eq!(
        report.total_bytes,
        (b"INTERNAL-BYTES".len() + b"EXTERNAL-BYTES-LONGER".len()) as u64
    );

    // Files copied into dest media/.
    assert!(dest_bundle.join("media").join("kept.mov").is_file());
    assert!(dest_bundle
        .join("media")
        .join("import-external.mp4")
        .is_file());
    assert_eq!(
        std::fs::read(dest_bundle.join("media").join("import-external.mp4")).unwrap(),
        b"EXTERNAL-BYTES-LONGER"
    );

    // Reopen the archived bundle: every source is now an internal media/ path.
    let archived = Project::open(&dest_bundle).unwrap();
    for e in &archived.manifest.entries {
        match &e.source {
            MediaSource::Project { relative_path } => {
                assert!(
                    relative_path.starts_with("media/"),
                    "expected media/ relative path, got {relative_path}"
                );
            }
            MediaSource::External { .. } => {
                panic!("source not rewritten to project: {:?}", e.source)
            }
        }
    }
    // Internal kept its name; external got the import- name.
    assert_eq!(
        archived.manifest.entries[0].source,
        MediaSource::Project {
            relative_path: "media/kept.mov".into()
        }
    );
    assert_eq!(
        archived.manifest.entries[1].source,
        MediaSource::Project {
            relative_path: "media/import-external.mp4".into()
        }
    );
}

#[test]
fn deduplicates_shared_source_files() {
    let tmp = TempDir::new("archive-dedup");
    let shared = tmp.child("shared.mp4");
    write_file(&shared, b"SHARED");

    let mut manifest = MediaManifest::new();
    // Two entries pointing at the same external file.
    manifest.entries.push(entry(
        "a",
        "one.mp4",
        ClipType::Video,
        MediaSource::External {
            absolute_path: shared.to_string_lossy().into_owned(),
        },
    ));
    manifest.entries.push(entry(
        "b",
        "two.mp4",
        ClipType::Video,
        MediaSource::External {
            absolute_path: shared.to_string_lossy().into_owned(),
        },
    ));

    let dest = tmp.child("Dedup.opentake");
    let report = archive(
        &Timeline::new(),
        &manifest,
        &GenerationLog::new(),
        None,
        &dest,
    )
    .unwrap();

    // Copied once; both entries collected; bytes counted once.
    assert_eq!(report.total_bytes, b"SHARED".len() as u64);
    assert_eq!(report.collected.len(), 2);
    let media_files: Vec<_> = std::fs::read_dir(dest.join("media"))
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(media_files.len(), 1, "expected a single deduped file");

    // Both manifest entries point at the same archived file.
    let archived = Project::open(&dest).unwrap();
    assert_eq!(
        archived.manifest.entries[0].source,
        archived.manifest.entries[1].source
    );
}

#[test]
fn missing_source_is_reported_and_kept_dangling() {
    let tmp = TempDir::new("archive-missing");

    let mut manifest = MediaManifest::new();
    manifest.entries.push(entry(
        "gone",
        "gone.mp4",
        ClipType::Video,
        MediaSource::External {
            absolute_path: "/does/not/exist.mp4".into(),
        },
    ));

    let dest = tmp.child("Missing.opentake");
    let report = archive(
        &Timeline::new(),
        &manifest,
        &GenerationLog::new(),
        None,
        &dest,
    )
    .unwrap();

    assert_eq!(report.missing.len(), 1);
    assert_eq!(report.missing[0].id, "gone");
    assert_eq!(report.missing[0].name, "gone.mp4");
    assert_eq!(report.total_bytes, 0);

    // The dangling reference is preserved unchanged (still external).
    let archived = Project::open(&dest).unwrap();
    assert_eq!(
        archived.manifest.entries[0].source,
        MediaSource::External {
            absolute_path: "/does/not/exist.mp4".into()
        }
    );
}

#[test]
fn carries_thumbnail_and_chat_sessions() {
    let tmp = TempDir::new("archive-extras");
    let source_bundle = tmp.child("Src.opentake");
    write_file(&source_bundle.join("thumbnail.jpg"), b"JPEGDATA");
    write_file(
        &source_bundle.join("chat-sessions").join("s1.json"),
        br#"{"id":"s1"}"#,
    );
    write_file(
        &source_bundle.join("chat-sessions").join("s2.json"),
        br#"{"id":"s2"}"#,
    );

    let dest = tmp.child("Extras.opentake");
    archive(
        &Timeline::new(),
        &MediaManifest::new(),
        &GenerationLog::new(),
        Some(source_bundle.as_path()),
        &dest,
    )
    .unwrap();

    assert_eq!(
        std::fs::read(dest.join("thumbnail.jpg")).unwrap(),
        b"JPEGDATA"
    );
    assert!(dest.join("chat-sessions").join("s1.json").is_file());
    assert!(dest.join("chat-sessions").join("s2.json").is_file());
}

#[test]
fn name_collision_gets_suffixed() {
    let tmp = TempDir::new("archive-collide");

    // Two distinct external files that share a base name "import-<id8>" only if
    // ids collide in their first 8 chars — force that to test uniquing.
    let f1 = tmp.child("a").join("v.mp4");
    let f2 = tmp.child("b").join("v.mp4");
    write_file(&f1, b"FIRST");
    write_file(&f2, b"SECOND");

    let mut manifest = MediaManifest::new();
    manifest.entries.push(entry(
        "abcd1234XXXX",
        "v.mp4",
        ClipType::Video,
        MediaSource::External {
            absolute_path: f1.to_string_lossy().into_owned(),
        },
    ));
    manifest.entries.push(entry(
        "abcd1234YYYY", // same first 8 chars -> same preferred name
        "v.mp4",
        ClipType::Video,
        MediaSource::External {
            absolute_path: f2.to_string_lossy().into_owned(),
        },
    ));

    let dest = tmp.child("Collide.opentake");
    archive(
        &Timeline::new(),
        &manifest,
        &GenerationLog::new(),
        None,
        &dest,
    )
    .unwrap();

    // First keeps import-abcd1234.mp4; second gets -1 suffix.
    assert!(dest.join("media").join("import-abcd1234.mp4").is_file());
    assert!(dest.join("media").join("import-abcd1234-1.mp4").is_file());
    let archived = Project::open(&dest).unwrap();
    assert_eq!(
        archived.manifest.entries[0].source,
        MediaSource::Project {
            relative_path: "media/import-abcd1234.mp4".into()
        }
    );
    assert_eq!(
        archived.manifest.entries[1].source,
        MediaSource::Project {
            relative_path: "media/import-abcd1234-1.mp4".into()
        }
    );
}
