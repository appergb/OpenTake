mod common;

use common::{write_file, TempDir};
use opentake_domain::{MediaManifest, Timeline};
use opentake_project::{archive, GenerationLog, ProjectError};

#[test]
fn archive_rejects_source_destination_without_mutation() {
    let tmp = TempDir::new("archive-source-destination");
    let source = tmp.child("Source.opentake");
    write_file(&source.join("sentinel"), b"source-bytes");

    let error = archive(
        &Timeline::new(),
        &MediaManifest::new(),
        &GenerationLog::new(),
        Some(&source),
        &source,
    )
    .unwrap_err();

    assert!(matches!(error, ProjectError::DestinationExists { ref path } if path == &source));
    assert_eq!(
        std::fs::read(source.join("sentinel")).unwrap(),
        b"source-bytes"
    );
}

#[test]
fn archive_rejects_existing_destination_without_mutation() {
    let tmp = TempDir::new("archive-existing-destination");
    let destination = tmp.child("Existing.opentake");
    write_file(&destination.join("sentinel"), b"destination-bytes");

    let error = archive(
        &Timeline::new(),
        &MediaManifest::new(),
        &GenerationLog::new(),
        None,
        &destination,
    )
    .unwrap_err();

    assert!(matches!(error, ProjectError::DestinationExists { ref path } if path == &destination));
    assert_eq!(
        std::fs::read(destination.join("sentinel")).unwrap(),
        b"destination-bytes"
    );
}

#[test]
fn archive_rejects_existing_regular_file_without_mutation() {
    let tmp = TempDir::new("archive-existing-file");
    let destination = tmp.child("Existing.opentake");
    write_file(&destination, b"existing-file-bytes");

    let error = archive(
        &Timeline::new(),
        &MediaManifest::new(),
        &GenerationLog::new(),
        None,
        &destination,
    )
    .unwrap_err();

    assert!(matches!(error, ProjectError::DestinationExists { ref path } if path == &destination));
    assert_eq!(std::fs::read(&destination).unwrap(), b"existing-file-bytes");
}

#[cfg(unix)]
#[test]
fn archive_rejects_dangling_destination_symlink_without_mutation() {
    use std::os::unix::fs::symlink;

    let tmp = TempDir::new("archive-dangling-symlink");
    let destination = tmp.child("Existing.opentake");
    let missing_target = tmp.child("missing-target");
    symlink(&missing_target, &destination).unwrap();

    let error = archive(
        &Timeline::new(),
        &MediaManifest::new(),
        &GenerationLog::new(),
        None,
        &destination,
    )
    .unwrap_err();

    assert!(matches!(error, ProjectError::DestinationExists { ref path } if path == &destination));
    assert_eq!(std::fs::read_link(&destination).unwrap(), missing_target);
    assert!(std::fs::symlink_metadata(&destination)
        .unwrap()
        .file_type()
        .is_symlink());
}
