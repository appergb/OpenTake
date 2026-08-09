use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use opentake_core::{AppCore, CoreError, CoreEvent, EditCommand, EditorSession, SeqIdGen};
use opentake_domain::{
    Clip, ClipType, GenerationInput, MediaManifestEntry, MediaSource, Timeline, Track,
};
use opentake_project::{GenerationLog, GenerationLogEntry, Project, ProjectError};

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "opentake-core-project-open-{tag}-{}-{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temporary directory");
        Self(path)
    }

    fn child(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn generation(model: &str, prompt: &str, created_at: f64) -> GenerationInput {
    GenerationInput {
        prompt: prompt.into(),
        model: model.into(),
        duration: 4,
        aspect_ratio: "16:9".into(),
        created_at: Some(created_at),
        ..Default::default()
    }
}

fn manifest_entry(id: &str, generation_input: Option<GenerationInput>) -> MediaManifestEntry {
    MediaManifestEntry {
        id: id.into(),
        name: format!("{id}.mov"),
        kind: ClipType::Video,
        source: MediaSource::Project {
            relative_path: format!("media/{id}.mov"),
        },
        duration: 4.0,
        generation_input,
        source_width: None,
        source_height: None,
        source_fps: None,
        has_audio: Some(true),
        color: None,
        proxy: None,
        folder_id: None,
        cached_remote_url: None,
        cached_remote_url_expires_at: None,
    }
}

fn save_manifest_fixture(
    bundle: &Path,
    entries: Vec<MediaManifestEntry>,
    generation_log: Option<GenerationLog>,
) {
    let mut project = Project::new(bundle);
    project.timeline = Timeline::new();
    project.manifest.entries = entries;
    project.generation_log = generation_log;
    project.save().expect("save fixture");
}

#[derive(Debug, PartialEq, Eq)]
enum ReceiptEntry {
    Directory,
    File(Vec<u8>),
    Symlink(PathBuf),
    Other,
}

fn bundle_receipt(bundle: &Path) -> BTreeMap<String, ReceiptEntry> {
    fn collect(bundle: &Path, path: &Path, receipt: &mut BTreeMap<String, ReceiptEntry>) {
        let mut entries = std::fs::read_dir(path)
            .expect("read bundle directory")
            .map(|entry| entry.expect("read bundle entry"))
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            let path = entry.path();
            let file_type = entry.file_type().expect("read bundle entry type");
            let relative = path
                .strip_prefix(bundle)
                .expect("bundle entry stays under root")
                .to_string_lossy()
                .into_owned();
            if file_type.is_dir() {
                receipt.insert(relative, ReceiptEntry::Directory);
                collect(bundle, &path, receipt);
            } else if file_type.is_symlink() {
                receipt.insert(
                    relative,
                    ReceiptEntry::Symlink(std::fs::read_link(path).expect("read bundle symlink")),
                );
            } else if file_type.is_file() {
                receipt.insert(
                    relative,
                    ReceiptEntry::File(std::fs::read(path).expect("read bundle file")),
                );
            } else {
                receipt.insert(relative, ReceiptEntry::Other);
            }
        }
    }

    let mut receipt = BTreeMap::new();
    receipt.insert(".".into(), ReceiptEntry::Directory);
    collect(bundle, bundle, &mut receipt);
    receipt
}

fn save_composite_fixture(bundle: &Path, label: &str) -> Project {
    let media_id = format!("{label}-media");
    let mut project = Project::new(bundle);
    let mut track = Track::new(format!("{label}-track"), ClipType::Video);
    track
        .clips
        .push(Clip::new(format!("{label}-clip"), &media_id, 12, 48));
    project.timeline.fps = 24;
    project.timeline.tracks.push(track);
    project.manifest.entries.push(manifest_entry(
        &media_id,
        Some(generation(
            &format!("{label}-model"),
            &format!("{label} prompt"),
            700_000_000.0,
        )),
    ));
    project.generation_log = Some(GenerationLog {
        version: 1,
        entries: vec![GenerationLogEntry::new(
            format!("{label}-generation"),
            format!("{label}-model"),
            Some(25),
            Some(700_000_000.0),
        )],
    });
    project.save().expect("save composite fixture");
    let media_dir = bundle.join("media");
    std::fs::create_dir_all(&media_dir).expect("create fixture media directory");
    std::fs::write(
        media_dir.join(format!("{media_id}.mov")),
        format!("{label}-media-bytes"),
    )
    .expect("write fixture media");
    project
}

#[test]
fn project_open_composite_acceptance() {
    let tmp = TempDir::new("composite");

    // A prepared project is fully decoded before the live session is replaced.
    let current_bundle = tmp.child("Current.opentake");
    let target_bundle = tmp.child("Target.opentake");
    let bad_media_bundle = tmp.child("BadMedia.opentake");
    save_composite_fixture(&current_bundle, "current");
    let target = save_composite_fixture(&target_bundle, "target");
    save_composite_fixture(&bad_media_bundle, "bad-media");
    std::fs::write(
        bad_media_bundle.join("media.json"),
        b"{ definitely not a media manifest",
    )
    .expect("damage media manifest");

    let core = AppCore::new();
    core.open_project(&current_bundle)
        .expect("open sentinel project");
    let events = Arc::new(Mutex::new(Vec::<CoreEvent>::new()));
    let received = Arc::clone(&events);
    core.subscribe(move |event| {
        received.lock().unwrap().push(event.clone());
    });
    let before = core.bundle_export_snapshot();
    let before_revision = core.project_revision();
    let current_receipt = bundle_receipt(&current_bundle);
    let bad_receipt = bundle_receipt(&bad_media_bundle);

    let error = core
        .open_project(&bad_media_bundle)
        .expect_err("malformed media manifest must fail closed");
    match error {
        CoreError::Project(ProjectError::Json { file, .. }) => {
            assert_eq!(file, "media.json");
        }
        other => panic!("unexpected malformed-media error: {other}"),
    }
    let after_failure = core.bundle_export_snapshot();
    assert_eq!(core.project_revision(), before_revision);
    assert_eq!(after_failure.timeline, before.timeline);
    assert_eq!(after_failure.manifest, before.manifest);
    assert_eq!(after_failure.generation_log, before.generation_log);
    assert_eq!(after_failure.project_path, before.project_path);
    assert_eq!(after_failure.compatibility, before.compatibility);
    assert!(events.lock().unwrap().is_empty());
    assert_eq!(bundle_receipt(&current_bundle), current_receipt);
    assert_eq!(bundle_receipt(&bad_media_bundle), bad_receipt);

    let target_receipt = bundle_receipt(&target_bundle);
    let prepared =
        AppCore::prepare_project_open(target_bundle.clone()).expect("prepare valid target");
    assert_eq!(
        core.project_revision(),
        before_revision,
        "prepare cannot publish the replacement session"
    );
    assert!(events.lock().unwrap().is_empty());
    assert_eq!(bundle_receipt(&target_bundle), target_receipt);

    let opened = core.commit_project_open(prepared);
    assert_eq!(opened.version, 0);
    assert_eq!(opened.project_epoch, before_revision.project_epoch + 1);
    assert_eq!(
        opened.project_path.as_deref(),
        Some(target_bundle.as_path())
    );
    assert_eq!(opened.timeline, target.timeline);
    assert_eq!(core.media(), target.manifest);
    assert_eq!(
        core.generation_log(),
        target.generation_log.clone().expect("fixture log")
    );
    assert_eq!(
        *events.lock().unwrap(),
        vec![CoreEvent::ProjectOpened {
            path: target_bundle.to_string_lossy().into_owned(),
            project_epoch: opened.project_epoch,
            version: 0,
        }]
    );

    core.save_project(None).expect("save opened target");
    assert_eq!(
        bundle_receipt(&target_bundle),
        target_receipt,
        "an unchanged project save is byte-stable, including bundled media"
    );
    let reopened = AppCore::new();
    let reopened_snapshot = reopened
        .open_project(&target_bundle)
        .expect("reopen saved target");
    assert_eq!(reopened_snapshot.timeline, opened.timeline);
    assert_eq!(reopened.media(), core.media());
    assert_eq!(reopened.generation_log(), core.generation_log());

    // Missing optional components use explicit empty state and remain stable.
    let missing_optional_bundle = tmp.child("MissingOptional.opentake");
    std::fs::create_dir_all(&missing_optional_bundle).expect("create legacy bundle");
    std::fs::write(
        missing_optional_bundle.join("project.json"),
        serde_json::to_vec_pretty(&Timeline::new()).expect("encode legacy timeline"),
    )
    .expect("write required timeline");
    let missing_optional = AppCore::new();
    missing_optional
        .open_project(&missing_optional_bundle)
        .expect("open project without optional components");
    assert!(missing_optional.media().entries.is_empty());
    assert!(missing_optional.generation_log().entries.is_empty());
    missing_optional
        .save_project(None)
        .expect("save project with missing optional components");
    assert!(
        !missing_optional_bundle.join("generation-log.json").exists(),
        "empty recovered provenance does not invent an optional component"
    );
    let missing_optional_reopened = AppCore::new();
    missing_optional_reopened
        .open_project(&missing_optional_bundle)
        .expect("reopen missing-optional project");
    assert!(missing_optional_reopened.media().entries.is_empty());
    assert!(missing_optional_reopened
        .generation_log()
        .entries
        .is_empty());

    // A damaged lenient component opens only as an explicit read-only recovery.
    let bad_log_bundle = tmp.child("BadLog.opentake");
    save_manifest_fixture(
        &bad_log_bundle,
        vec![manifest_entry(
            "recovered-media",
            Some(generation("recovered-model", "recover me", 700_000_100.0)),
        )],
        None,
    );
    std::fs::write(
        bad_log_bundle.join("generation-log.json"),
        b"not valid generation log json",
    )
    .expect("damage generation log");
    let bad_log_receipt = bundle_receipt(&bad_log_bundle);
    let recovered = AppCore::new();
    let recovered_snapshot = recovered
        .open_project(&bad_log_bundle)
        .expect("open recoverable generation log");
    assert_eq!(
        recovered_snapshot.compatibility.blockers(),
        ["generation-log.json:invalid-or-unreadable"]
    );
    assert_eq!(recovered.generation_log().entries.len(), 1);
    assert!(matches!(
        recovered.save_project(None),
        Err(CoreError::Project(
            ProjectError::CompatibilityReadOnly { .. }
        ))
    ));
    assert_eq!(
        bundle_receipt(&bad_log_bundle),
        bad_log_receipt,
        "read-only recovery cannot overwrite the damaged component"
    );
}

#[test]
fn missing_generation_log_seeds_manifest_provenance_once() {
    let tmp = TempDir::new("generation-seed");
    let seeded_bundle = tmp.child("Seeded.opentake");
    let duplicate = generation("legacy-video-model", "same generation", 700_000_000.0);
    let second = generation("legacy-audio-model", "different generation", 700_000_010.0);
    let entries = vec![
        manifest_entry("imported", None),
        manifest_entry("generated-z", Some(duplicate.clone())),
        manifest_entry("generated-a", Some(duplicate)),
        manifest_entry("generated-b", Some(second)),
    ];
    save_manifest_fixture(&seeded_bundle, entries.clone(), None);

    let mut session = EditorSession::open_project(&seeded_bundle).expect("open legacy project");
    let seeded_entries = &session.generation_log().entries;
    assert_eq!(seeded_entries.len(), 2);
    let video = seeded_entries
        .iter()
        .find(|entry| entry.model == "legacy-video-model")
        .expect("video provenance is seeded");
    assert!(video.id.starts_with("legacy-generation:"));
    assert_eq!(video.id.len(), "legacy-generation:".len() + 64);
    assert_eq!(video.cost_credits, None);
    assert_eq!(video.created_at, Some(700_000_000.0));
    let audio = seeded_entries
        .iter()
        .find(|entry| entry.model == "legacy-audio-model")
        .expect("audio provenance is seeded");
    assert!(audio.id.starts_with("legacy-generation:"));
    assert_eq!(audio.id.len(), "legacy-generation:".len() + 64);
    assert_eq!(audio.cost_credits, None);
    assert_eq!(audio.created_at, Some(700_000_010.0));
    assert_ne!(video.id, audio.id);

    let reordered_bundle = tmp.child("Reordered.opentake");
    let mut reordered_entries = entries.clone();
    reordered_entries.reverse();
    save_manifest_fixture(&reordered_bundle, reordered_entries, None);
    assert_eq!(
        EditorSession::open_project(&reordered_bundle)
            .expect("open reordered legacy project")
            .generation_log(),
        session.generation_log(),
        "manifest ordering cannot change the deterministic seed"
    );

    let signed_zero_bundle = tmp.child("SignedZero.opentake");
    let signed_zero_entries = vec![
        manifest_entry(
            "duplicate-id",
            Some(generation("same-model", "same prompt", -0.0)),
        ),
        manifest_entry(
            "duplicate-id",
            Some(generation("same-model", "same prompt", 0.0)),
        ),
    ];
    save_manifest_fixture(&signed_zero_bundle, signed_zero_entries.clone(), None);
    let signed_zero =
        EditorSession::open_project(&signed_zero_bundle).expect("open signed-zero fixture");
    assert_eq!(signed_zero.generation_log().entries.len(), 2);
    assert_ne!(
        signed_zero.generation_log().entries[0].id,
        signed_zero.generation_log().entries[1].id,
        "different provenance remains uniquely identified when asset ids collide"
    );
    let signed_zero_reordered_bundle = tmp.child("SignedZeroReordered.opentake");
    let mut signed_zero_reordered_entries = signed_zero_entries;
    signed_zero_reordered_entries.reverse();
    save_manifest_fixture(
        &signed_zero_reordered_bundle,
        signed_zero_reordered_entries,
        None,
    );
    let signed_zero_reordered = EditorSession::open_project(&signed_zero_reordered_bundle)
        .expect("open reordered signed-zero fixture");
    assert_eq!(
        serde_json::to_vec(signed_zero.generation_log()).unwrap(),
        serde_json::to_vec(signed_zero_reordered.generation_log()).unwrap(),
        "signed zero and duplicate asset ids remain byte-stable after manifest reorder"
    );

    let seed_before_edit = session.generation_log().clone();
    session
        .apply(
            EditCommand::InsertTrack {
                kind: ClipType::Video,
                at: None,
            },
            &SeqIdGen::new("edited-track-"),
        )
        .expect("edit opened legacy project");
    assert_eq!(
        session.generation_log(),
        &seed_before_edit,
        "ordinary edits cannot duplicate or rewrite the recovered audit log"
    );
    session
        .save_project(None)
        .expect("persist seeded generation log");
    let log_path = seeded_bundle.join("generation-log.json");
    let first_bytes = std::fs::read(&log_path).expect("seeded log exists");
    let reopened = EditorSession::open_project(&seeded_bundle).expect("reopen seeded project");
    assert_eq!(reopened.generation_log(), session.generation_log());
    assert_eq!(
        reopened.timeline().tracks.len(),
        1,
        "the representative edit persists through save and reopen"
    );
    let mut reopened = reopened;
    reopened
        .save_project(None)
        .expect("save reopened seeded project");
    assert_eq!(
        std::fs::read(&log_path).expect("read saved log"),
        first_bytes,
        "repeated saves are byte-stable and never duplicate seeded rows"
    );

    let existing = GenerationLog {
        version: 1,
        entries: vec![GenerationLogEntry::new(
            "existing-row",
            "recorded-model",
            Some(42),
            Some(699_999_999.0),
        )],
    };
    let existing_bundle = tmp.child("Existing.opentake");
    save_manifest_fixture(&existing_bundle, entries.clone(), Some(existing.clone()));
    assert_eq!(
        EditorSession::open_project(&existing_bundle)
            .expect("open project with valid log")
            .generation_log(),
        &existing,
        "a valid partial log is authoritative and is never supplemented"
    );

    let empty_log_bundle = tmp.child("EmptyLog.opentake");
    save_manifest_fixture(
        &empty_log_bundle,
        entries.clone(),
        Some(GenerationLog::new()),
    );
    let mut empty_log_session =
        EditorSession::open_project(&empty_log_bundle).expect("open project with valid empty log");
    assert!(
        empty_log_session.generation_log().entries.is_empty(),
        "a valid empty log remains authoritative"
    );
    let empty_log_copy = tmp.child("EmptyLogCopy.opentake");
    empty_log_session
        .save_project(Some(empty_log_copy.clone()))
        .expect("save-as project with valid empty log");
    assert!(empty_log_copy.join("generation-log.json").is_file());
    assert!(
        EditorSession::open_project(&empty_log_copy)
            .expect("reopen save-as with valid empty log")
            .generation_log()
            .entries
            .is_empty(),
        "save-as preserves the valid empty component instead of reseeding"
    );

    let empty_manifest_bundle = tmp.child("EmptyManifest.opentake");
    save_manifest_fixture(&empty_manifest_bundle, Vec::new(), None);
    let mut empty_session =
        EditorSession::open_project(&empty_manifest_bundle).expect("open empty manifest");
    assert!(empty_session.generation_log().entries.is_empty());
    empty_session
        .save_project(None)
        .expect("save empty legacy project");
    assert!(
        !empty_manifest_bundle.join("generation-log.json").exists(),
        "an empty seed does not create an unnecessary optional component"
    );

    let malformed_bundle = tmp.child("Malformed.opentake");
    save_manifest_fixture(&malformed_bundle, entries, None);
    std::fs::write(
        malformed_bundle.join("generation-log.json"),
        b"not valid generation log json",
    )
    .expect("damage optional generation log");
    let mut malformed =
        EditorSession::open_project(&malformed_bundle).expect("malformed optional log opens");
    assert_eq!(
        malformed.generation_log().entries,
        session.generation_log().entries,
        "a damaged log gets the same deterministic in-memory recovery seed"
    );
    assert!(
        malformed.save_project(None).is_err(),
        "compatibility safety blocks overwriting the damaged component"
    );
}
