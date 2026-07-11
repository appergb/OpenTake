//! `AppCore` — the concurrent, observable façade over an [`EditorSession`].
//!
//! This is the assembly layer's public handle (`core-SPEC.md` §1.3, §2.5).
//! Upstream's three clients (SwiftUI, in-app agent, MCP server) share one
//! `EditorViewModel` reference inside a single process. OpenTake crosses a
//! logical process boundary, so `AppCore` holds the single authoritative
//! [`EditorSession`] behind an `Arc<Mutex<…>>` and is `Clone` (a clone copies
//! only the `Arc`s). The Tauri command layer, the in-app agent loop, and the MCP
//! server each hold a clone pointing at the *same* session — the cross-thread
//! equivalent of "three clients, one view model".
//!
//! ## What this layer adds on top of `EditorSession`
//!
//! `EditorSession` already delegates editing + the undo/version transaction to
//! `opentake-ops`. `AppCore` adds exactly two things the session can't:
//!
//! 1. **Serialization of all mutations** through one `Mutex`, so `version` is
//!    strictly monotonic even under concurrent clients (`core-SPEC.md` §4.3).
//! 2. **Change broadcasting**: after a committing edit / undo / redo it emits
//!    [`CoreEvent::TimelineChanged`] so observers re-sync their mirror. Events
//!    are emitted **after the lock is released**, so a subscriber callback can
//!    safely call back into the core without deadlocking (`core-SPEC.md` §2.3
//!    step 5).
//!
//! It deliberately does **not** reimplement any editing, transaction, or
//! persistence logic — those live in `opentake-ops` / `opentake-project` and are
//! reached through the session.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use opentake_domain::{MediaManifest, MediaManifestEntry, Timeline};
use opentake_ops::command::{EditCommand, EditResult};
use opentake_ops::IdGen;
use opentake_project::{GenerationLog, ProjectCompatibility};

use crate::deps::CoreDeps;
use crate::error::Result;
use crate::events::{CoreEvent, EventBus, SubscriptionId};
use crate::session::{EditorSession, ProbedMedia};

/// Thread-safe id generator used as the core's default.
///
/// [`opentake_ops::SeqIdGen`] is deliberately `!Sync` (it threads a `Cell`
/// through `&self`), which is fine for single-threaded ops tests but not for the
/// shared, `Send + Sync` [`AppCore`]. This atomic-backed generator mints the
/// same `"{prefix}{n}"` ids while being safe to share across threads, without
/// pulling a `uuid` dependency into the assembly layer. Production wiring
/// (`src-tauri`) can inject a UUID-backed generator via [`AppCore::set_id_gen`].
#[derive(Debug)]
pub struct CoreIdGen {
    prefix: String,
    counter: AtomicU64,
}

impl CoreIdGen {
    /// New generator counting from 1 with the given id prefix.
    pub fn new(prefix: impl Into<String>) -> Self {
        CoreIdGen {
            prefix: prefix.into(),
            counter: AtomicU64::new(0),
        }
    }
}

impl Default for CoreIdGen {
    fn default() -> Self {
        CoreIdGen::new("id-")
    }
}

impl IdGen for CoreIdGen {
    fn next_id(&self) -> String {
        let n = self.counter.fetch_add(1, Ordering::Relaxed) + 1;
        format!("{}{}", self.prefix, n)
    }
}

/// A read-only snapshot of the timeline paired with the version it was taken at.
/// This is the payload `get_timeline` returns; the front end stores it as
/// `{ mirror, mirrorVersion }` and uses `version` for idempotent re-fetching
/// (`core-SPEC.md` §4.1).
#[derive(Clone, Debug)]
pub struct TimelineSnapshot {
    /// The timeline at version [`Self::version`].
    pub timeline: Timeline,
    /// The project session this timeline belongs to.
    pub project_epoch: u64,
    /// The document version this snapshot was taken at.
    pub version: u64,
    /// The current project bundle path, if it has been saved/opened.
    pub project_path: Option<PathBuf>,
    /// Persisted fields this build cannot safely mutate.
    pub compatibility: ProjectCompatibility,
}

/// Identity of the current project session and its document version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectRevision {
    /// Monotonic identity of the current project session.
    pub project_epoch: u64,
    /// Monotonic edit version within the current project session.
    pub version: u64,
}

/// One-lock snapshot of the state consumed by runtime media operations.
#[derive(Clone, Debug)]
pub struct ProjectRuntimeSnapshot {
    /// The authoritative timeline.
    pub timeline: Timeline,
    /// The media catalog paired with [`Self::timeline`].
    pub media: MediaManifest,
    /// The bundle directory paired with [`Self::timeline`].
    pub project_dir: Option<PathBuf>,
    /// The project session identity paired with [`Self::timeline`].
    pub project_epoch: u64,
    /// The document version paired with [`Self::timeline`].
    pub version: u64,
}

/// One-lock snapshot consumed by self-contained project export.
#[derive(Clone, Debug)]
pub struct BundleExportSnapshot {
    pub timeline: Timeline,
    pub manifest: MediaManifest,
    pub generation_log: GenerationLog,
    pub project_path: Option<PathBuf>,
    pub project_epoch: u64,
    pub compatibility: ProjectCompatibility,
}

struct CoreSessionSlot {
    project_epoch: u64,
    editor: EditorSession,
}

/// Fully loaded project replacement awaiting an atomic session commit.
///
/// Fields stay private so callers can only pass the exact prepared value back
/// to [`AppCore::commit_project_open`].
pub struct PreparedProjectOpen {
    path: PathBuf,
    editor: EditorSession,
}

impl CoreSessionSlot {
    fn timeline_snapshot(&self) -> TimelineSnapshot {
        TimelineSnapshot {
            timeline: self.editor.timeline(),
            project_epoch: self.project_epoch,
            version: self.editor.version(),
            project_path: self.editor.project_dir().map(PathBuf::from),
            compatibility: self.editor.compatibility().clone(),
        }
    }

    fn replace_editor(&mut self, editor: EditorSession) -> TimelineSnapshot {
        self.editor = editor;
        self.project_epoch += 1;
        self.timeline_snapshot()
    }
}

/// The cloneable handle to the one authoritative editing session.
#[derive(Clone)]
pub struct AppCore {
    session: Arc<Mutex<CoreSessionSlot>>,
    events: EventBus,
    deps: Arc<CoreDeps>,
    // `Send + Sync` so `AppCore` stays shareable across threads (Tauri State,
    // MCP handlers). The default ([`CoreIdGen`]) is atomic-backed.
    ids: Arc<dyn IdGen + Send + Sync>,
}

impl Default for AppCore {
    fn default() -> Self {
        AppCore::new()
    }
}

impl AppCore {
    /// A core wrapping a fresh, unsaved project with placeholder capability
    /// backends ([`CoreDeps::default`]) and a default sequential id generator.
    pub fn new() -> Self {
        AppCore::with_deps(CoreDeps::default())
    }

    /// A core with explicit capability backends (the production wiring path).
    pub fn with_deps(deps: CoreDeps) -> Self {
        AppCore {
            session: Arc::new(Mutex::new(CoreSessionSlot {
                project_epoch: 0,
                editor: EditorSession::new_project(),
            })),
            events: EventBus::new(),
            deps: Arc::new(deps),
            ids: Arc::new(CoreIdGen::new("id-")),
        }
    }

    /// Swap the id generator (e.g. a UUID-backed one in production). The
    /// generator must be `Send + Sync` since [`AppCore`] is shared across
    /// threads. Affects ids minted by subsequent commands.
    pub fn set_id_gen(&mut self, ids: Arc<dyn IdGen + Send + Sync>) {
        self.ids = ids;
    }

    /// The event bus, for registering observers (the Tauri bridge subscribes
    /// here to forward [`CoreEvent`]s to the front end).
    pub fn events(&self) -> &EventBus {
        &self.events
    }

    /// Subscribe to [`CoreEvent`]s. Convenience for `self.events().subscribe`.
    pub fn subscribe(&self, listener: impl Fn(&CoreEvent) + Send + 'static) -> SubscriptionId {
        self.events.subscribe(listener)
    }

    /// The injected capability backends (preview/export/media/gen).
    pub fn deps(&self) -> &CoreDeps {
        &self.deps
    }

    // MARK: - Reads

    /// A snapshot of the current timeline + its version (`get_timeline`).
    pub fn get_timeline(&self) -> TimelineSnapshot {
        self.lock().timeline_snapshot()
    }

    /// The identity and document version of the current project session.
    pub fn project_revision(&self) -> ProjectRevision {
        let session = self.lock();
        ProjectRevision {
            project_epoch: session.project_epoch,
            version: session.editor.version(),
        }
    }

    /// A runtime snapshot of the current project state.
    pub fn runtime_snapshot(&self) -> ProjectRuntimeSnapshot {
        let session = self.lock();
        ProjectRuntimeSnapshot {
            timeline: session.editor.timeline(),
            media: session.editor.media(),
            project_dir: session.editor.project_dir().map(PathBuf::from),
            project_epoch: session.project_epoch,
            version: session.editor.version(),
        }
    }

    /// Snapshot all self-contained bundle inputs under one session lock.
    pub fn bundle_export_snapshot(&self) -> BundleExportSnapshot {
        let session = self.lock();
        BundleExportSnapshot {
            timeline: session.editor.timeline(),
            manifest: session.editor.media(),
            generation_log: session.editor.generation_log().clone(),
            project_path: session.editor.project_dir().map(PathBuf::from),
            project_epoch: session.project_epoch,
            compatibility: session.editor.compatibility().clone(),
        }
    }

    /// Refuse application-layer filesystem work before it can mutate a project.
    pub fn ensure_project_mutable(&self) -> Result<()> {
        self.lock().editor.ensure_mutable()
    }

    /// The current document version.
    pub fn version(&self) -> u64 {
        self.lock().editor.version()
    }

    /// Whether an undo / redo is currently available (for enabling UI affordances).
    pub fn can_undo(&self) -> bool {
        self.lock().editor.can_undo()
    }

    /// Whether a redo is currently available.
    pub fn can_redo(&self) -> bool {
        self.lock().editor.can_redo()
    }

    // MARK: - The single editing entry point

    /// Apply one [`EditCommand`] — the unified entry point shared by UI, in-app
    /// agent, and MCP (`core-SPEC.md` §2.5). Runs the command under the lock
    /// (the ops layer performs the snapshot/commit/version transaction), then,
    /// **after releasing the lock**, emits [`CoreEvent::TimelineChanged`] iff the
    /// command actually changed the document. Unchanged commands (and rejected
    /// ones) emit nothing and do not move the version.
    pub fn apply(&self, command: EditCommand) -> Result<EditResult> {
        let (result, project_epoch) = {
            let mut session = self.lock();
            let result = session.editor.apply(command, self.ids.as_ref())?;
            (result, session.project_epoch)
        };
        if result.changed {
            self.events.emit(&CoreEvent::TimelineChanged {
                project_epoch,
                version: result.timeline_version,
            });
        }
        Ok(result)
    }

    /// Undo the last committed edit (global Cmd+Z). Thin wrapper over
    /// [`EditCommand::Undo`] so the same transaction + event path is reused; the
    /// ops layer bumps the version on a successful undo, which the front-end
    /// mirror needs to re-sync (`core-SPEC.md` §2.4).
    pub fn undo(&self) -> Result<EditResult> {
        self.apply(EditCommand::Undo)
    }

    /// Redo the last undone edit. Symmetric to [`Self::undo`].
    pub fn redo(&self) -> Result<EditResult> {
        self.apply(EditCommand::Redo)
    }

    // MARK: - Project lifecycle

    /// Replace the current session with a fresh, unsaved project, emit
    /// [`CoreEvent::ProjectOpened`] (path empty, version 0), and return its first
    /// snapshot.
    pub fn new_project(&self) -> TimelineSnapshot {
        let snapshot = {
            let mut session = self.lock();
            session.replace_editor(EditorSession::new_project())
        };
        self.events.emit(&CoreEvent::ProjectOpened {
            path: String::new(),
            project_epoch: snapshot.project_epoch,
            version: snapshot.version,
        });
        snapshot
    }

    /// Open the `.opentake` bundle at `path`, replacing the current session.
    /// Emits [`CoreEvent::ProjectOpened`] on success (the front end fetches the
    /// first snapshot itself, so no `TimelineChanged` is emitted —
    /// `core-SPEC.md` §5.4 step 6). Returns the first snapshot for convenience.
    pub fn open_project(&self, path: impl Into<PathBuf>) -> Result<TimelineSnapshot> {
        let prepared = Self::prepare_project_open(path.into())?;
        Ok(self.commit_project_open(prepared))
    }

    pub fn prepare_project_open(path: PathBuf) -> Result<PreparedProjectOpen> {
        let editor = EditorSession::open_project(&path)?;
        Ok(PreparedProjectOpen { path, editor })
    }

    pub fn commit_project_open(&self, prepared: PreparedProjectOpen) -> TimelineSnapshot {
        let snapshot = {
            let mut session = self.lock();
            session.replace_editor(prepared.editor)
        };
        self.events.emit(&CoreEvent::ProjectOpened {
            path: prepared.path.to_string_lossy().into_owned(),
            project_epoch: snapshot.project_epoch,
            version: snapshot.version,
        });
        snapshot
    }

    /// Save the current project. `path = None` saves back to the open bundle
    /// (autosave); `Some(path)` is a save-as. Emits [`CoreEvent::ProjectSaved`]
    /// with the written path on success.
    pub fn save_project(&self, path: Option<PathBuf>) -> Result<PathBuf> {
        self.save_project_with_thumbnail(path, None)
    }

    /// Like [`Self::save_project`] but also writes a cover `thumbnail.jpg` from
    /// the supplied JPEG bytes (`None` leaves any existing cover in place). The
    /// caller — which owns the media engine / GPU — captures the representative
    /// frame (upstream `captureThumbnail`, via
    /// [`opentake_media::capture_project_thumbnail`]) so this assembly layer stays
    /// free of the ffmpeg/GPU stack. Emits [`CoreEvent::ProjectSaved`] on success.
    pub fn save_project_with_thumbnail(
        &self,
        path: Option<PathBuf>,
        thumbnail: Option<Vec<u8>>,
    ) -> Result<PathBuf> {
        let (written, project_epoch) = {
            let mut session = self.lock();
            let written = session
                .editor
                .save_project_with_thumbnail(path, thumbnail)?;
            (written, session.project_epoch)
        };
        self.events.emit(&CoreEvent::ProjectSaved {
            path: written.to_string_lossy().into_owned(),
            project_epoch,
        });
        Ok(written)
    }

    // MARK: - Media import

    /// A snapshot of the current media manifest (`get_media`). The catalog the
    /// media panel renders; reads are infallible.
    pub fn media(&self) -> MediaManifest {
        self.lock().editor.media()
    }

    /// A snapshot of the current AI generation log. Cloned out from under the
    /// session lock so a caller (the `.opentake` bundle exporter) can write it
    /// into a self-contained bundle alongside the timeline + manifest, exactly as
    /// upstream carries `editor.generationLog` into `PalmierProjectExporter`
    /// (`Export/ExportService.swift:186-197`). Reads are infallible.
    pub fn generation_log(&self) -> GenerationLog {
        self.lock().editor.generation_log().clone()
    }

    /// The open project's `.opentake` bundle directory, or `None` for an unsaved
    /// project. Needed to resolve [`MediaSource::Project`](opentake_domain::MediaSource)
    /// relative paths to on-disk files (preview/composite read the original media).
    pub fn project_dir(&self) -> Option<PathBuf> {
        self.lock().editor.project_dir().map(|p| p.to_path_buf())
    }

    /// Import a local media file as an external reference, minting the asset id
    /// from the core's id generator. Returns the new [`MediaManifestEntry`] and,
    /// **after releasing the lock**, emits [`CoreEvent::MediaChanged`] so
    /// observers refresh their media mirror.
    ///
    /// The caller (which owns the media engine) supplies the probed metadata; see
    /// [`ProbedMedia`] and [`EditorSession::import_media_file`]. Errors with
    /// [`crate::CoreError::Unsupported`]`("media")` for files whose extension is
    /// not on the import white-list.
    pub fn import_media_file(
        &self,
        path: impl AsRef<std::path::Path>,
        name: impl Into<String>,
        probe: &ProbedMedia,
    ) -> Result<MediaManifestEntry> {
        let id = self.ids.next_id();
        let (entry, count, project_epoch) = {
            let mut session = self.lock();
            let entry = session.editor.import_media_file(path, id, name, probe)?;
            let count = session.editor.media().entries.len();
            (entry, count, session.project_epoch)
        };
        self.events.emit(&CoreEvent::MediaChanged {
            project_epoch,
            count,
        });
        Ok(entry)
    }

    /// Toggle favorite state for `asset_ids` (#91), emitting
    /// [`CoreEvent::MediaChanged`] after releasing the lock (only when something
    /// changed) so the media mirror refreshes. Favoriting is a manifest mutation
    /// outside undo — see [`EditorSession::set_media_favorite`]. Returns how many
    /// ids changed state.
    pub fn set_media_favorite(&self, asset_ids: &[String], favorite: bool) -> Result<usize> {
        let (changed, count, project_epoch) = {
            let mut session = self.lock();
            let changed = session.editor.set_media_favorite(asset_ids, favorite)?;
            let count = session.editor.media().entries.len();
            (changed, count, session.project_epoch)
        };
        if changed > 0 {
            self.events.emit(&CoreEvent::MediaChanged {
                project_epoch,
                count,
            });
        }
        Ok(changed)
    }

    /// Relink an existing asset (by id) to a new file, keeping the same id, and
    /// emit [`CoreEvent::MediaChanged`] after releasing the lock so observers
    /// refresh. See [`EditorSession::relink_media_file`]: re-importing would mint
    /// a new id and leave clips stranded on the missing entry; relinking heals
    /// them in place. Errors with [`crate::CoreError::Media`] for an unknown id
    /// or a type mismatch.
    pub fn relink_media_file(
        &self,
        asset_id: &str,
        path: impl AsRef<std::path::Path>,
        probe: &ProbedMedia,
    ) -> Result<MediaManifestEntry> {
        let (entry, count, project_epoch) = {
            let mut session = self.lock();
            let entry = session.editor.relink_media_file(asset_id, path, probe)?;
            let count = session.editor.media().entries.len();
            (entry, count, session.project_epoch)
        };
        self.events.emit(&CoreEvent::MediaChanged {
            project_epoch,
            count,
        });
        Ok(entry)
    }

    // MARK: - Internal

    /// Lock the session, recovering from a poisoned mutex by taking the inner
    /// guard. Command bodies are panic-free value-type ops, so poisoning is not
    /// expected; recovering keeps a stray panic in one observer from wedging the
    /// whole core.
    fn lock(&self) -> std::sync::MutexGuard<'_, CoreSessionSlot> {
        self.session
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{ClipType, Timeline, Track};
    use opentake_ops::command::ClipEntry;
    use std::sync::Mutex;

    /// Build a core whose session has one empty video track, ready for AddClips.
    fn core_with_track() -> AppCore {
        let core = AppCore::new();
        {
            let mut session = core.session.lock().unwrap();
            let mut tl = Timeline::new();
            tl.tracks.push(Track::new("t1", ClipType::Video));
            session.editor.seed_from_timeline(tl);
        }
        core
    }

    fn project_bundle(label: &str) -> PathBuf {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "opentake-core-project-epoch-{}-{label}-{sequence}.opentake",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);

        let core = AppCore::new();
        {
            let mut session = core.session.lock().unwrap();
            let mut timeline = Timeline::new();
            timeline
                .tracks
                .push(Track::new(format!("{label}-track"), ClipType::Video));
            session.editor.seed_from_timeline(timeline);
        }
        core.import_media_file(
            std::env::temp_dir().join(format!("{label}.mp4")),
            label,
            &ProbedMedia::default(),
        )
        .unwrap();
        core.save_project(Some(dir.clone())).unwrap();
        dir
    }

    fn assert_runtime_snapshot_matches_project(
        snapshot: &ProjectRuntimeSnapshot,
        first_dir: &std::path::Path,
        second_dir: &std::path::Path,
        initial_epoch: u64,
    ) {
        assert_eq!(snapshot.version, 0);
        let (label, expected_epoch_parity) = match snapshot.project_dir.as_deref() {
            Some(path) if path == first_dir => ("first", 0),
            Some(path) if path == second_dir => ("second", 1),
            other => panic!("runtime snapshot has unexpected project dir: {other:?}"),
        };
        assert_eq!(snapshot.timeline.tracks.len(), 1);
        assert_eq!(snapshot.timeline.tracks[0].id, format!("{label}-track"));
        assert_eq!(snapshot.media.entries.len(), 1);
        assert_eq!(snapshot.media.entries[0].name, label);
        assert!(snapshot.project_epoch >= initial_epoch);
        assert_eq!(
            (snapshot.project_epoch - initial_epoch) % 2,
            expected_epoch_parity
        );
    }

    fn add_one_clip() -> EditCommand {
        EditCommand::AddClips {
            entries: vec![ClipEntry {
                media_ref: "asset-1".into(),
                media_type: ClipType::Video,
                source_clip_type: ClipType::Video,
                track_index: 0,
                start_frame: 0,
                duration_frames: 30,
                trim_start_frame: None,
                trim_end_frame: None,
                has_audio: false,
                add_linked_audio: false,
                transform: None,
            }],
        }
    }

    #[test]
    fn app_core_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        // The cross-process design (§1.3) requires the handle be shareable
        // across threads; this fails to compile if a field breaks that.
        assert_send_sync::<AppCore>();
    }

    #[test]
    fn core_id_gen_is_monotonic_from_one() {
        let g = CoreIdGen::new("c-");
        assert_eq!(g.next_id(), "c-1");
        assert_eq!(g.next_id(), "c-2");
    }

    #[test]
    fn clones_share_one_session() {
        let a = core_with_track();
        let b = a.clone();
        assert_eq!(b.version(), 0);

        let res = a.apply(add_one_clip()).unwrap();
        assert!(res.changed);
        // The clone observes the same authoritative state.
        assert_eq!(b.version(), 1);
        assert_eq!(b.get_timeline().version, 1);
        assert_eq!(b.get_timeline().timeline.tracks[0].clips.len(), 1);
    }

    #[test]
    fn apply_bumps_version_and_emits_once() {
        let core = core_with_track();
        let seen: Arc<Mutex<Vec<CoreEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        core.subscribe(move |ev| sink.lock().unwrap().push(ev.clone()));

        let res = core.apply(add_one_clip()).unwrap();
        assert!(res.changed);
        assert_eq!(res.timeline_version, 1);
        assert_eq!(core.version(), 1);

        let events = seen.lock().unwrap().clone();
        assert_eq!(
            events,
            vec![CoreEvent::TimelineChanged {
                project_epoch: 0,
                version: 1
            }]
        );
    }

    #[test]
    fn unchanged_command_does_not_emit_or_bump() {
        let core = core_with_track();
        let seen: Arc<Mutex<Vec<CoreEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        core.subscribe(move |ev| sink.lock().unwrap().push(ev.clone()));

        // Undo with empty history changes nothing.
        let res = core.undo().unwrap();
        assert!(!res.changed);
        assert_eq!(core.version(), 0);
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn undo_redo_through_core_bumps_version_and_emits() {
        let core = core_with_track();
        let seen: Arc<Mutex<Vec<CoreEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        core.subscribe(move |ev| sink.lock().unwrap().push(ev.clone()));

        core.apply(add_one_clip()).unwrap(); // v1
        core.undo().unwrap(); // v2, clip gone
        assert_eq!(core.get_timeline().timeline.tracks[0].clips.len(), 0);
        core.redo().unwrap(); // v3, clip back
        assert_eq!(core.get_timeline().timeline.tracks[0].clips.len(), 1);

        let versions: Vec<u64> = seen
            .lock()
            .unwrap()
            .iter()
            .map(|e| match e {
                CoreEvent::TimelineChanged { version, .. } => *version,
                _ => 0,
            })
            .collect();
        assert_eq!(versions, vec![1, 2, 3]);
    }

    #[test]
    fn rejected_command_returns_err_without_emitting() {
        let core = core_with_track();
        let seen: Arc<Mutex<Vec<CoreEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        core.subscribe(move |ev| sink.lock().unwrap().push(ev.clone()));

        // Empty entries is a validation error in the ops layer.
        let err = core.apply(EditCommand::AddClips { entries: vec![] });
        assert!(err.is_err());
        assert_eq!(core.version(), 0);
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn opening_two_projects_produces_distinct_epochs_at_version_zero() {
        let first_dir = project_bundle("first");
        let second_dir = project_bundle("second");
        let core = AppCore::new();

        let first = core.open_project(&first_dir).unwrap();
        let second = core.open_project(&second_dir).unwrap();

        assert_eq!(first.version, 0);
        assert_eq!(second.version, 0);
        assert_ne!(first.project_epoch, second.project_epoch);

        let _ = std::fs::remove_dir_all(first_dir);
        let _ = std::fs::remove_dir_all(second_dir);
    }

    #[test]
    fn new_project_advances_epoch_even_when_versions_collide() {
        let core = AppCore::new();
        let before = core.project_revision();

        core.new_project();
        let after = core.project_revision();

        assert_eq!(before.version, 0);
        assert_eq!(after.version, 0);
        assert!(after.project_epoch > before.project_epoch);
    }

    #[test]
    fn runtime_snapshot_never_mixes_timeline_media_and_project_dir() {
        let first_dir = project_bundle("first");
        let second_dir = project_bundle("second");
        let core = AppCore::new();
        core.open_project(&first_dir).unwrap();
        let initial_epoch = core.project_revision().project_epoch;

        assert_runtime_snapshot_matches_project(
            &core.runtime_snapshot(),
            &first_dir,
            &second_dir,
            initial_epoch,
        );

        let mut spare = EditorSession::open_project(&second_dir).unwrap();
        let writer_core = core.clone();
        let writer = std::thread::spawn(move || {
            for _ in 0..20_000 {
                let mut session = writer_core.lock();
                std::mem::swap(&mut session.editor, &mut spare);
                session.project_epoch += 1;
                drop(session);
                std::thread::yield_now();
            }
        });

        for _ in 0..10_000 {
            assert_runtime_snapshot_matches_project(
                &core.runtime_snapshot(),
                &first_dir,
                &second_dir,
                initial_epoch,
            );
        }
        writer.join().unwrap();

        let _ = std::fs::remove_dir_all(first_dir);
        let _ = std::fs::remove_dir_all(second_dir);
    }

    #[test]
    fn new_project_resets_and_emits_project_opened() {
        let core = core_with_track();
        core.apply(add_one_clip()).unwrap();
        assert_eq!(core.version(), 1);

        let seen: Arc<Mutex<Vec<CoreEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        core.subscribe(move |ev| sink.lock().unwrap().push(ev.clone()));

        let snapshot = core.new_project();
        assert_eq!(core.version(), 0);
        assert_eq!(snapshot.project_epoch, 1);
        assert!(core.get_timeline().timeline.tracks.is_empty());
        assert_eq!(
            seen.lock().unwrap().clone(),
            vec![CoreEvent::ProjectOpened {
                path: String::new(),
                project_epoch: 1,
                version: 0
            }]
        );
    }

    #[test]
    fn open_save_roundtrip_through_core_emits_lifecycle_events() {
        let dir = std::env::temp_dir().join(format!(
            "opentake-core-appcore-{}-{}.opentake",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&dir);

        let core = core_with_track();
        core.apply(add_one_clip()).unwrap();
        let before = core.get_timeline().timeline;

        let seen: Arc<Mutex<Vec<CoreEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        core.subscribe(move |ev| sink.lock().unwrap().push(ev.clone()));

        core.save_project(Some(dir.clone())).unwrap();

        // Open into a second core and verify identical timeline.
        let core2 = AppCore::new();
        let snap = core2.open_project(dir.clone()).unwrap();
        assert_eq!(snap.timeline, before);
        assert_eq!(snap.project_epoch, 1);
        assert_eq!(snap.version, 0);

        // First core saw a ProjectSaved event with the dir path.
        let path_str = dir.to_string_lossy().into_owned();
        assert_eq!(
            seen.lock().unwrap().clone(),
            vec![CoreEvent::ProjectSaved {
                path: path_str,
                project_epoch: 0
            }]
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_media_mints_id_appends_and_emits_media_changed() {
        let core = AppCore::new();
        let seen: Arc<Mutex<Vec<CoreEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        core.subscribe(move |ev| sink.lock().unwrap().push(ev.clone()));

        let probe = ProbedMedia {
            duration_secs: 3.0,
            width: Some(640),
            height: Some(480),
            fps: Some(24.0),
            has_audio: false,
        };
        let entry = core.import_media_file("/abs/a.mp4", "a", &probe).unwrap();

        // Id came from the core generator (default "id-" prefix).
        assert_eq!(entry.id, "id-1");
        assert_eq!(core.media().entries.len(), 1);
        // Importing does not move the timeline version.
        assert_eq!(core.version(), 0);
        assert_eq!(
            seen.lock().unwrap().clone(),
            vec![CoreEvent::MediaChanged {
                project_epoch: 0,
                count: 1
            }]
        );
    }

    #[test]
    fn import_media_unsupported_errors_and_emits_nothing() {
        let core = AppCore::new();
        let seen: Arc<Mutex<Vec<CoreEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        core.subscribe(move |ev| sink.lock().unwrap().push(ev.clone()));

        let err = core.import_media_file("/abs/a.txt", "a", &ProbedMedia::default());
        assert!(err.is_err());
        assert!(core.media().entries.is_empty());
        assert!(seen.lock().unwrap().is_empty());
    }
}
