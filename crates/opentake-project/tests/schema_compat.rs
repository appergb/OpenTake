mod common;

use std::path::Path;

use opentake_project::{Project, ProjectError};
use serde_json::{json, Value};

use common::TempDir;

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
                    "loudnessNormalization": {
                        "targetLufs": -16.0,
                        "truePeakCeilingDbtp": -1.0,
                        "inputIntegratedLufs": -24.0,
                        "inputTruePeakDbtp": -12.0,
                        "gainDb": 8.0,
                        "outputIntegratedLufs": -16.0,
                        "outputTruePeakDbtp": -2.0
                    },
                    "audioDenoise": {
                        "mode": "voice",
                        "strength": 0.8,
                        "previewEnabled": true
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
    let before = common::tree_receipt(bundle);
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
    assert_eq!(common::tree_receipt(bundle), before);

    let destination = bundle
        .parent()
        .expect("fixture has parent")
        .join("Rejected-Save-As.opentake");
    assert!(!destination.exists());
    let parent_before = common::tree_receipt(destination.parent().expect("destination has parent"));
    assert_compatibility_error(
        project
            .save_to(&destination)
            .expect_err("save-as must fail before creating destination"),
        expected_blockers,
    );
    assert!(!destination.exists());
    assert_eq!(
        common::tree_receipt(destination.parent().expect("destination has parent")),
        parent_before
    );
    assert_eq!(common::tree_receipt(bundle), before);

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
fn malformed_manifest_contract_matches_authoritative_source() {
    let tmp = TempDir::new("schema-manifest-contract");

    // A complete current bundle remains writable and semantically stable
    // across the full open -> edit -> save -> reopen path.
    let current_manifest = tmp.child("CurrentManifest.opentake");
    write_known_bundle(&current_manifest);
    let mut current = Project::open(&current_manifest).expect("current manifest opens");
    assert_eq!(current.manifest.version, 2);
    assert!(!current.compatibility().is_read_only());
    current.timeline.fps = 48;
    let expected_timeline = current.timeline.clone();
    let expected_manifest = current.manifest.clone();
    let expected_generation_log = current.generation_log.clone();
    current.save().expect("current manifest saves");
    let reopened_current =
        Project::open(&current_manifest).expect("current manifest reopens after edit");
    assert_eq!(reopened_current.timeline, expected_timeline);
    assert_eq!(reopened_current.manifest, expected_manifest);
    assert_eq!(reopened_current.generation_log, expected_generation_log);
    assert!(!reopened_current.compatibility().is_read_only());

    // Decode order is project -> media -> generation log. An earlier strict
    // component wins even when every later component is also malformed.
    let missing_timeline = tmp.child("MissingTimeline.opentake");
    write_known_bundle(&missing_timeline);
    std::fs::remove_file(missing_timeline.join("project.json")).expect("remove timeline");
    common::write_file(&missing_timeline.join("media.json"), b"not-json");
    common::write_file(&missing_timeline.join("generation-log.json"), b"not-json");
    let before = common::tree_receipt(&missing_timeline);
    assert!(matches!(
        Project::open(&missing_timeline),
        Err(ProjectError::MissingTimeline { .. })
    ));
    assert_eq!(common::tree_receipt(&missing_timeline), before);

    let malformed_timeline = tmp.child("MalformedTimeline.opentake");
    write_known_bundle(&malformed_timeline);
    common::write_file(&malformed_timeline.join("project.json"), b"not-json");
    common::write_file(&malformed_timeline.join("media.json"), b"not-json");
    let before = common::tree_receipt(&malformed_timeline);
    assert!(matches!(
        Project::open(&malformed_timeline),
        Err(ProjectError::Json { ref file, .. }) if file == "project.json"
    ));
    assert_eq!(common::tree_receipt(&malformed_timeline), before);

    // A present media.json is strict for both invalid JSON and a valid JSON
    // shape that cannot decode as MediaManifest. Neither branch may recover an
    // empty manifest or alter any component.
    for (label, malformed) in [
        ("syntax", serde_json::Value::String("not-json".into())),
        ("structure", json!({"entries": "not-an-array"})),
    ] {
        let name = format!("MalformedManifest-{label}.opentake");
        let bundle = tmp.child(&name);
        write_known_bundle(&bundle);
        if label == "syntax" {
            common::write_file(&bundle.join("media.json"), b"{ not valid json");
        } else {
            write_json(&bundle.join("media.json"), &malformed);
        }
        common::write_file(&bundle.join("generation-log.json"), b"not-json");
        let before = common::tree_receipt(&bundle);
        assert!(
            matches!(
                Project::open(&bundle),
                Err(ProjectError::Json { ref file, .. }) if file == "media.json"
            ),
            "case {label}"
        );
        assert_eq!(common::tree_receipt(&bundle), before, "case {label}");
    }

    // A missing manifest is an explicit empty current manifest. A safe save
    // creates media.json, keeps the absent optional generation log absent, and
    // reopens with stable semantics.
    let missing_manifest = tmp.child("MissingManifest.opentake");
    write_known_bundle(&missing_manifest);
    std::fs::remove_file(missing_manifest.join("media.json")).expect("remove manifest");
    std::fs::remove_file(missing_manifest.join("generation-log.json"))
        .expect("remove generation log");
    let mut recovered = Project::open(&missing_manifest).expect("missing manifest defaults");
    assert_eq!(recovered.manifest.version, 2);
    assert!(recovered.manifest.entries.is_empty());
    assert!(recovered.manifest.folders.is_empty());
    assert!(recovered.generation_log.is_none());
    assert!(!recovered.compatibility().is_read_only());
    recovered.timeline.fps = 60;
    recovered.save().expect("safe save creates empty manifest");
    assert!(missing_manifest.join("media.json").is_file());
    assert!(!missing_manifest.join("generation-log.json").exists());
    let reopened = Project::open(&missing_manifest).expect("reopen saved missing-manifest project");
    assert_eq!(reopened.timeline.fps, 60);
    assert_eq!(reopened.manifest.version, 2);
    assert!(reopened.manifest.entries.is_empty());
    assert!(reopened.generation_log.is_none());

    // A present legacy `{}` manifest is distinct from a missing file: its
    // custom decoder preserves upstream schema version 1 across save/reopen.
    let legacy_manifest = tmp.child("LegacyManifest.opentake");
    write_known_bundle(&legacy_manifest);
    write_json(&legacy_manifest.join("media.json"), &json!({}));
    let legacy = Project::open(&legacy_manifest).expect("legacy manifest opens");
    assert_eq!(legacy.manifest.version, 1);
    legacy.save().expect("legacy manifest remains writable");
    assert_eq!(
        Project::open(&legacy_manifest)
            .expect("reopen legacy manifest")
            .manifest
            .version,
        1
    );

    // A malformed generation log remains the sole read-only recovery branch.
    // Even with media.json missing, no save may create the default manifest or
    // create a Save As destination over the damaged provenance.
    let missing_manifest_bad_log = tmp.child("MissingManifestBadLog.opentake");
    write_known_bundle(&missing_manifest_bad_log);
    std::fs::remove_file(missing_manifest_bad_log.join("media.json")).expect("remove manifest");
    common::write_file(
        &missing_manifest_bad_log.join("generation-log.json"),
        b"not-json",
    );
    let before = common::tree_receipt(&missing_manifest_bad_log);
    let mut read_only =
        Project::open(&missing_manifest_bad_log).expect("damaged optional log recovers");
    assert_eq!(read_only.manifest.version, 2);
    assert_eq!(
        read_only.compatibility().blockers(),
        ["generation-log.json:invalid-or-unreadable"]
    );
    read_only.timeline.fps = 48;
    assert_compatibility_error(
        read_only.save().expect_err("same-path save must fail"),
        &["generation-log.json:invalid-or-unreadable"],
    );
    assert_eq!(common::tree_receipt(&missing_manifest_bad_log), before);
    let destination = tmp.child("RejectedManifestRecovery.opentake");
    let parent_before = common::tree_receipt(tmp.path());
    assert_compatibility_error(
        read_only
            .save_to(&destination)
            .expect_err("save-as must fail before destination creation"),
        &["generation-log.json:invalid-or-unreadable"],
    );
    assert!(!destination.exists());
    assert_eq!(common::tree_receipt(tmp.path()), parent_before);
    assert_eq!(common::tree_receipt(&missing_manifest_bad_log), before);
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

/// Composite acceptance entry tracked by the data-safety implementation plan.
/// It exercises strict required components, read-only recovery for optional
/// corruption, unknown-field preservation, and the writable save/reopen path.
#[test]
fn cross_cutting_project_safety_acceptance() {
    unknown_top_level_timeline_field_blocks_writes_without_changing_bytes();
    unknown_nested_manifest_entry_and_source_fields_block_writes();
    malformed_optional_generation_log_opens_but_blocks_writes();
    malformed_manifest_contract_matches_authoritative_source();
    trailing_required_json_remains_a_strict_open_error();
    known_schema_remains_writable();
}
