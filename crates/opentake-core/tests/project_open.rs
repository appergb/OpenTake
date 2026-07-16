use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use opentake_core::{EditCommand, EditorSession, SeqIdGen};
use opentake_domain::{ClipType, GenerationInput, MediaManifestEntry, MediaSource, Timeline};
use opentake_project::{GenerationLog, GenerationLogEntry, Project};

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
