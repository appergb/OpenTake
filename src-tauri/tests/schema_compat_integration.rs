use std::fs;
use std::path::{Path, PathBuf};

use opentake_project::Project;
use opentake_tauri_lib::export::run_bundle_export;
use serde_json::{json, Value};

fn unknown_project(root: &Path) -> Project {
    let bundle = root.join("Unknown.opentake");
    let project = Project::new(&bundle);
    project.save().expect("save known fixture");
    let timeline_path = bundle.join("project.json");
    let mut timeline: Value =
        serde_json::from_slice(&fs::read(&timeline_path).expect("read timeline"))
            .expect("decode timeline");
    timeline["futureTimeline"] = json!(true);
    fs::write(
        &timeline_path,
        serde_json::to_vec_pretty(&timeline).expect("encode unknown timeline"),
    )
    .expect("write unknown timeline");
    Project::open(bundle).expect("unknown project remains readable")
}

fn recursive_manifest(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut entries = fs::read_dir(dir)
            .expect("read manifest directory")
            .map(|entry| entry.expect("read manifest entry").path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            let relative = path
                .strip_prefix(root)
                .expect("path is under root")
                .to_path_buf();
            if path.is_dir() {
                out.push((relative.clone(), b"<dir>".to_vec()));
                walk(root, &path, out);
            } else {
                out.push((relative, fs::read(&path).expect("read manifest file")));
            }
        }
    }

    if !root.exists() {
        return Vec::new();
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

#[test]
fn bundle_export_refuses_unknown_project_before_new_destination_creation() {
    let temp = tempfile::tempdir().expect("create temp root");
    let project = unknown_project(temp.path());
    let destination = temp.path().join("NewDestination.opentake");
    let log = opentake_project::GenerationLog::new();

    let error = run_bundle_export(
        &project.timeline,
        &project.manifest,
        project.generation_log.as_ref().unwrap_or(&log),
        Some(&project.bundle_path),
        project.compatibility(),
        destination.to_string_lossy().into_owned(),
    )
    .expect_err("compatibility read-only bundle export must fail");

    assert!(error.contains("compatibility read-only"), "{error}");
    assert!(!destination.exists());
}

#[test]
fn bundle_export_refuses_unknown_project_without_replacing_existing_destination() {
    let temp = tempfile::tempdir().expect("create temp root");
    let project = unknown_project(temp.path());
    let destination = temp.path().join("ExistingDestination.opentake");
    fs::create_dir_all(destination.join("nested/deep")).expect("create existing destination");
    fs::write(destination.join("root.txt"), b"root-before").expect("write root fixture");
    fs::write(destination.join("nested/deep/file.bin"), b"deep-before")
        .expect("write nested fixture");
    let before = recursive_manifest(&destination);

    let log = opentake_project::GenerationLog::new();
    let error = run_bundle_export(
        &project.timeline,
        &project.manifest,
        project.generation_log.as_ref().unwrap_or(&log),
        Some(&project.bundle_path),
        project.compatibility(),
        destination.to_string_lossy().into_owned(),
    )
    .expect_err("compatibility read-only bundle export must fail");

    assert!(error.contains("compatibility read-only"), "{error}");
    assert_eq!(recursive_manifest(&destination), before);
}
