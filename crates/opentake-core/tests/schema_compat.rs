use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use opentake_core::{AppCore, CoreError, EditCommand, ProbedMedia, TimelineSnapshotDto};
use opentake_domain::{ClipType, MediaManifestEntry, MediaSource, Timeline, Track};
use opentake_project::{Project, ProjectError};
use serde_json::{json, Value};

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "opentake-core-schema-compat-{}-{label}-{sequence}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp root");
        Self(path)
    }

    fn child(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn external_entry(id: &str, name: &str, source: &Path) -> MediaManifestEntry {
    MediaManifestEntry {
        id: id.into(),
        name: name.into(),
        kind: ClipType::Video,
        source: MediaSource::External {
            absolute_path: source.to_string_lossy().into_owned(),
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

fn write_unknown_bundle(root: &Path, label: &str) -> PathBuf {
    let bundle = root.join(format!("{label}.opentake"));
    let source = root.join(format!("{label}.mp4"));
    fs::write(&source, b"fixture").expect("write source fixture");

    let mut project = Project::new(&bundle);
    let mut timeline = Timeline::new();
    timeline
        .tracks
        .push(Track::new(format!("{label}-track"), ClipType::Video));
    project.timeline = timeline;
    project
        .manifest
        .entries
        .push(external_entry("asset-1", label, &source));
    project.save().expect("save known fixture");

    let timeline_path = bundle.join("project.json");
    let mut timeline_json: Value =
        serde_json::from_slice(&fs::read(&timeline_path).expect("read timeline"))
            .expect("decode timeline");
    timeline_json[format!("future{label}Timeline")] = json!(true);
    fs::write(
        &timeline_path,
        serde_json::to_vec_pretty(&timeline_json).expect("encode timeline"),
    )
    .expect("write unknown timeline");

    let media_path = bundle.join("media.json");
    let mut media_json: Value =
        serde_json::from_slice(&fs::read(&media_path).expect("read media")).expect("decode media");
    media_json[format!("future{label}Media")] = json!(true);
    fs::write(
        &media_path,
        serde_json::to_vec_pretty(&media_json).expect("encode media"),
    )
    .expect("write unknown media");
    bundle
}

fn recursive_tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut paths = fs::read_dir(dir)
            .expect("read tree")
            .map(|entry| entry.expect("read tree entry").path())
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let relative = path
                .strip_prefix(root)
                .expect("tree path under root")
                .into();
            if path.is_dir() {
                out.push((relative, b"<dir>".to_vec()));
                walk(root, &path, out);
            } else {
                out.push((relative, fs::read(&path).expect("read tree file")));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

fn assert_compatibility_error(error: CoreError) {
    match error {
        CoreError::Project(ProjectError::CompatibilityReadOnly { blockers }) => {
            assert!(!blockers.is_empty());
        }
        other => panic!("expected compatibility read-only error, got {other:?}"),
    }
}

#[test]
fn unknown_project_apply_is_rejected_without_state_or_history_change() {
    let tmp = TempDir::new("apply");
    let bundle = write_unknown_bundle(&tmp.0, "Apply");
    let core = AppCore::new();
    core.open_project(&bundle).expect("unknown project opens");
    let before = core.get_timeline();
    assert!(!core.can_undo());
    assert!(!core.can_redo());

    let error = core
        .apply(EditCommand::InsertTrack {
            kind: ClipType::Audio,
            at: None,
        })
        .expect_err("unknown project edit must be rejected");
    assert_compatibility_error(error);

    let after = core.get_timeline();
    assert_eq!(after.timeline, before.timeline);
    assert_eq!(after.version, before.version);
    assert_eq!(after.project_epoch, before.project_epoch);
    assert!(!core.can_undo());
    assert!(!core.can_redo());
}

#[test]
fn unknown_project_media_mutations_are_rejected_without_manifest_change() {
    let tmp = TempDir::new("media");
    let bundle = write_unknown_bundle(&tmp.0, "Media");
    let import = tmp.child("import.mp4");
    let relink = tmp.child("relink.mp4");
    fs::write(&import, b"import").expect("write import fixture");
    fs::write(&relink, b"relink").expect("write relink fixture");
    let core = AppCore::new();
    core.open_project(&bundle).expect("unknown project opens");
    let before = core.media();

    assert_compatibility_error(
        core.import_media_file(&import, "import", &ProbedMedia::default())
            .expect_err("import must be rejected"),
    );
    assert_eq!(core.media(), before);

    let original_source = match &before.entries[0].source {
        MediaSource::External { absolute_path } => PathBuf::from(absolute_path),
        other => panic!("fixture source must be external: {other:?}"),
    };
    assert_compatibility_error(
        core.relink_media_file("asset-1", &original_source, &ProbedMedia::default())
            .expect_err("identical relink must be rejected"),
    );
    assert_eq!(core.media(), before);
    assert_compatibility_error(
        core.relink_media_file("asset-1", &relink, &ProbedMedia::default())
            .expect_err("different relink must be rejected"),
    );
    assert_eq!(core.media(), before);

    assert_compatibility_error(
        core.set_media_favorite(&["asset-1".into()], true)
            .expect_err("favorite must be rejected"),
    );
    assert_eq!(core.media(), before);
}

#[test]
fn unknown_project_save_and_save_as_keep_source_and_destination_unchanged() {
    let tmp = TempDir::new("save");
    let bundle = write_unknown_bundle(&tmp.0, "Save");
    let destination = tmp.child("SaveAs.opentake");
    let before = recursive_tree(&bundle);
    let core = AppCore::new();
    core.open_project(&bundle).expect("unknown project opens");

    assert_compatibility_error(core.save_project(None).expect_err("save must be rejected"));
    assert_eq!(recursive_tree(&bundle), before);
    assert_compatibility_error(
        core.save_project(Some(destination.clone()))
            .expect_err("save-as must be rejected"),
    );
    assert!(!destination.exists());
    assert_eq!(core.project_dir().as_deref(), Some(bundle.as_path()));
    assert_eq!(recursive_tree(&bundle), before);
}

#[test]
fn bundle_snapshot_is_single_epoch_and_snapshots_carry_sorted_compatibility() {
    let tmp = TempDir::new("snapshot");
    let first = write_unknown_bundle(&tmp.0, "Alpha");
    let second = write_unknown_bundle(&tmp.0, "Zulu");
    let core = AppCore::new();
    core.open_project(&first)
        .expect("first unknown project opens");

    let timeline = core.get_timeline();
    assert_eq!(
        timeline.compatibility.blockers(),
        [
            "media.json:futureAlphaMedia",
            "project.json:futureAlphaTimeline"
        ]
    );
    assert_eq!(timeline.project_path.as_deref(), Some(first.as_path()));
    let dto = TimelineSnapshotDto::from(timeline.clone());
    let json = serde_json::to_value(dto).expect("serialize timeline snapshot dto");
    assert_eq!(json["projectPath"], first.to_string_lossy().as_ref());
    assert_eq!(json["compatibilityReadOnly"], true);
    assert_eq!(
        json["compatibilityBlockers"],
        json!([
            "media.json:futureAlphaMedia",
            "project.json:futureAlphaTimeline"
        ])
    );
    let initial_bundle = core.bundle_export_snapshot();
    assert_eq!(initial_bundle.project_epoch, timeline.project_epoch);
    assert_eq!(initial_bundle.timeline, timeline.timeline);
    assert_eq!(initial_bundle.project_path, timeline.project_path);
    assert_eq!(initial_bundle.compatibility, timeline.compatibility);

    let toggler = {
        let core = core.clone();
        let first = first.clone();
        let second = second.clone();
        std::thread::spawn(move || {
            for index in 0..80 {
                let path = if index % 2 == 0 { &second } else { &first };
                core.open_project(path).expect("toggle project");
            }
        })
    };

    for _ in 0..300 {
        let snapshot = core.bundle_export_snapshot();
        let path = snapshot
            .project_path
            .as_deref()
            .expect("opened project path");
        let (label, blocker) = if path == first {
            ("Alpha", "project.json:futureAlphaTimeline")
        } else if path == second {
            ("Zulu", "project.json:futureZuluTimeline")
        } else {
            panic!("unexpected snapshot project path: {}", path.display());
        };
        assert_eq!(snapshot.timeline.tracks[0].id, format!("{label}-track"));
        assert_eq!(snapshot.manifest.entries[0].name, label);
        assert!(snapshot
            .compatibility
            .blockers()
            .iter()
            .any(|candidate| candidate == blocker));
        assert!(snapshot.project_epoch > 0);
    }
    toggler.join().expect("project toggler completes");
}
