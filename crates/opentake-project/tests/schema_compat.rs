mod common;

use std::path::{Path, PathBuf};

use opentake_project::{Project, ProjectError};
use serde_json::{json, Value};

use common::TempDir;

const COMPONENTS: [&str; 3] = ["project.json", "media.json", "generation-log.json"];

fn write_json(path: &Path, value: &Value) {
    common::write_file(
        path,
        &serde_json::to_vec_pretty(value).expect("encode fixture"),
    );
}

fn write_known_bundle(bundle: &Path) {
    std::fs::create_dir_all(bundle).expect("create bundle");
    write_json(
        &bundle.join("project.json"),
        &json!({
            "fps": 30,
            "width": 1920,
            "height": 1080,
            "settingsConfigured": true,
            "tracks": [{
                "id": "track-1",
                "type": "video",
                "clips": [{
                    "id": "clip-1",
                    "mediaRef": "asset-1",
                    "mediaType": "video",
                    "sourceClipType": "video",
                    "startFrame": 0,
                    "durationFrames": 30,
                    "positionTrack": {
                        "keyframes": [{
                            "frame": 0,
                            "value": {"a": 0.0, "b": 0.0},
                            "interpolationOut": "smooth"
                        }]
                    },
                    "effects": [{"name": "blur", "params": {}, "enabled": true}],
                    "masks": [{
                        "shape": {"kind": "circle", "center": {"x": 0.5, "y": 0.5}, "radius": {"x": 0.5, "y": 0.5}},
                        "feather": 0.0,
                        "invert": false
                    }]
                }]
            }]
        }),
    );
    write_json(
        &bundle.join("media.json"),
        &json!({
            "version": 2,
            "entries": [{
                "id": "asset-1",
                "name": "clip.mov",
                "type": "video",
                "source": {"project": {"relativePath": "media/clip.mov"}},
                "duration": 1.0,
                "generationInput": {
                    "prompt": "fixture",
                    "model": "fixture-model",
                    "duration": 1,
                    "aspectRatio": "16:9"
                }
            }],
            "folders": [{"id": "folder-1", "name": "Fixture"}]
        }),
    );
    write_json(
        &bundle.join("generation-log.json"),
        &json!({
            "version": 1,
            "entries": [{"id": "gen-1", "model": "fixture-model", "costCredits": 1}]
        }),
    );
}

fn mutate_json(bundle: &Path, component: &str, mutate: impl FnOnce(&mut Value)) {
    let path = bundle.join(component);
    let mut value: Value = serde_json::from_slice(&std::fs::read(&path).expect("read fixture"))
        .expect("decode fixture");
    mutate(&mut value);
    write_json(&path, &value);
}

fn component_receipts(bundle: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    COMPONENTS
        .into_iter()
        .map(|file| {
            let path = bundle.join(file);
            let bytes = std::fs::read(&path).expect("record source component bytes");
            (path, bytes)
        })
        .collect()
}

fn assert_compatibility_error(error: ProjectError, expected_blockers: &[&str]) {
    match error {
        ProjectError::CompatibilityReadOnly { blockers } => {
            assert_eq!(
                blockers,
                expected_blockers
                    .iter()
                    .map(|blocker| (*blocker).to_owned())
                    .collect::<Vec<_>>()
            );
        }
        other => panic!("expected compatibility read-only error, got {other:?}"),
    }
}

fn assert_read_only_without_file_changes(bundle: &Path, expected_blockers: &[&str]) -> Project {
    let before = component_receipts(bundle);
    let project = Project::open(bundle).expect("unknown fields remain readable");
    assert!(project.compatibility().is_read_only());
    assert_eq!(
        project.compatibility().blockers(),
        expected_blockers
            .iter()
            .map(|blocker| (*blocker).to_owned())
            .collect::<Vec<_>>()
    );

    assert_compatibility_error(
        project.save().expect_err("same-path save must fail"),
        expected_blockers,
    );
    for (path, bytes) in &before {
        assert_eq!(std::fs::read(path).expect("read unchanged source"), *bytes);
    }

    let destination = bundle
        .parent()
        .expect("fixture has parent")
        .join("Rejected-Save-As.opentake");
    assert!(!destination.exists());
    assert_compatibility_error(
        project
            .save_to(&destination)
            .expect_err("save-as must fail before creating destination"),
        expected_blockers,
    );
    assert!(!destination.exists());
    for (path, bytes) in &before {
        assert_eq!(std::fs::read(path).expect("read unchanged source"), *bytes);
    }

    project
}

#[test]
fn unknown_top_level_timeline_field_blocks_writes_without_changing_bytes() {
    let tmp = TempDir::new("schema-unknown-timeline");
    let bundle = tmp.child("UnknownTimeline.opentake");
    write_known_bundle(&bundle);

    let timeline_path = bundle.join("project.json");
    let mut timeline_json =
        std::fs::read_to_string(&timeline_path).expect("read raw timeline fixture");
    let object_end = timeline_json.rfind('}').expect("timeline is a JSON object");
    timeline_json.insert_str(
        object_end,
        ",\n  \"futureTimeline\": true,\n  \"futureTimeline\": false\n",
    );
    common::write_file(&timeline_path, timeline_json.as_bytes());
    mutate_json(&bundle, "media.json", |manifest| {
        manifest["futureManifest"] = json!(true);
    });
    mutate_json(&bundle, "generation-log.json", |log| {
        log["futureGenerationLog"] = json!(true);
    });

    // Component-local sorting would preserve project/media/log open order, and
    // the repeated raw key proves the final blocker list is also deduplicated.
    assert_read_only_without_file_changes(
        &bundle,
        &[
            "generation-log.json:futureGenerationLog",
            "media.json:futureManifest",
            "project.json:futureTimeline",
        ],
    );
}

#[test]
fn unknown_nested_clip_field_blocks_writes_without_changing_bytes() {
    let tmp = TempDir::new("schema-unknown-clip");
    let bundle = tmp.child("UnknownClip.opentake");
    write_known_bundle(&bundle);
    mutate_json(&bundle, "project.json", |timeline| {
        let track = &mut timeline["tracks"][0];
        track["futureTrack"] = json!(true);
        let clip = &mut track["clips"][0];
        clip["futureClip"] = json!(true);
        clip["positionTrack"]["futureKeyframeTrack"] = json!(true);
        clip["positionTrack"]["keyframes"][0]["value"]["futureKeyframeValue"] = json!(true);
        clip["effects"][0]["futureEffect"] = json!(true);
        clip["masks"][0]["futureMask"] = json!(true);
    });

    assert_read_only_without_file_changes(
        &bundle,
        &[
            "project.json:tracks.0.clips.0.effects.0.futureEffect",
            "project.json:tracks.0.clips.0.futureClip",
            "project.json:tracks.0.clips.0.masks.0.futureMask",
            "project.json:tracks.0.clips.0.positionTrack.futureKeyframeTrack",
            "project.json:tracks.0.clips.0.positionTrack.keyframes.0.value.futureKeyframeValue",
            "project.json:tracks.0.futureTrack",
        ],
    );
}

#[test]
fn unknown_nested_manifest_entry_and_source_fields_block_writes() {
    let tmp = TempDir::new("schema-unknown-manifest");
    let bundle = tmp.child("UnknownManifest.opentake");
    write_known_bundle(&bundle);
    mutate_json(&bundle, "media.json", |manifest| {
        manifest["futureManifest"] = json!(true);
        manifest["folders"][0]["futureFolder"] = json!(true);
        let entry = &mut manifest["entries"][0];
        entry["futureEntry"] = json!(true);
        entry["source"]["project"]["futureSource"] = json!(true);
        entry["generationInput"]["futureGenerationInput"] = json!(true);
    });

    assert_read_only_without_file_changes(
        &bundle,
        &[
            "media.json:entries.0.futureEntry",
            "media.json:entries.0.generationInput.futureGenerationInput",
            "media.json:entries.0.source.project.futureSource",
            "media.json:folders.0.futureFolder",
            "media.json:futureManifest",
        ],
    );
}

#[test]
fn unknown_generation_log_entry_field_blocks_writes() {
    let tmp = TempDir::new("schema-unknown-generation-log");
    let bundle = tmp.child("UnknownGenerationLog.opentake");
    write_known_bundle(&bundle);
    mutate_json(&bundle, "generation-log.json", |log| {
        log["futureGenerationLog"] = json!(true);
        log["entries"][0]["futureGenerationLogEntry"] = json!(true);
    });

    assert_read_only_without_file_changes(
        &bundle,
        &[
            "generation-log.json:entries.0.futureGenerationLogEntry",
            "generation-log.json:futureGenerationLog",
        ],
    );
}

#[test]
fn malformed_optional_generation_log_opens_but_blocks_writes() {
    let tmp = TempDir::new("schema-malformed-generation-log");
    let bundle = tmp.child("MalformedGenerationLog.opentake");
    write_known_bundle(&bundle);
    common::write_file(
        &bundle.join("generation-log.json"),
        b"this is not valid generation log json",
    );

    let project = assert_read_only_without_file_changes(
        &bundle,
        &["generation-log.json:invalid-or-unreadable"],
    );
    assert!(project.generation_log.is_none());
}

#[test]
fn trailing_required_json_remains_a_strict_open_error() {
    let tmp = TempDir::new("schema-trailing-required-json");
    let bundle = tmp.child("TrailingRequiredJson.opentake");
    write_known_bundle(&bundle);
    common::write_file(
        &bundle.join("project.json"),
        br#"{"fps":30,"tracks":[]} trailing"#,
    );

    let error = Project::open(&bundle).expect_err("trailing JSON must remain invalid");
    assert!(
        matches!(error, ProjectError::Json { ref file, .. } if file == "project.json"),
        "expected project.json parse error, got {error:?}"
    );
}

#[test]
fn known_schema_remains_writable() {
    let tmp = TempDir::new("schema-known");
    let bundle = tmp.child("Known.opentake");
    write_known_bundle(&bundle);

    let mut project = Project::open(&bundle).expect("open known schema");
    assert!(!project.compatibility().is_read_only());
    assert!(project.compatibility().blockers().is_empty());
    project.timeline.fps = 60;
    project.save().expect("known schema saves in place");
    assert_eq!(Project::open(&bundle).unwrap().timeline.fps, 60);

    let destination = tmp.child("Known-Save-As.opentake");
    project
        .save_to(&destination)
        .expect("known schema saves to a new bundle");
    assert!(destination.is_dir());
    let saved_as = Project::open(&destination).expect("open saved-as bundle");
    assert_eq!(saved_as.timeline.fps, 60);
    assert!(!saved_as.compatibility().is_read_only());
}
