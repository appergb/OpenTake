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
    /// The document version this snapshot was taken at.
    pub version: u64,
}

/// The cloneable handle to the one authoritative editing session.
#[derive(Clone)]
pub struct AppCore {
    session: Arc<Mutex<EditorSession>>,
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
            session: Arc::new(Mutex::new(EditorSession::new_project())),
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
        let session = self.lock();
        TimelineSnapshot {
            timeline: session.timeline(),
            version: session.version(),
        }
    }

    /// The current document version.
    pub fn version(&self) -> u64 {
        self.lock().version()
    }

    /// Whether an undo / redo is currently available (for enabling UI affordances).
    pub fn can_undo(&self) -> bool {
        self.lock().can_undo()
    }

    /// Whether a redo is currently available.
    pub fn can_redo(&self) -> bool {
        self.lock().can_redo()
    }

    // MARK: - The single editing entry point

    /// Apply one [`EditCommand`] — the unified entry point shared by UI, in-app
    /// agent, and MCP (`core-SPEC.md` §2.5). Runs the command under the lock
    /// (the ops layer performs the snapshot/commit/version transaction), then,
    /// **after releasing the lock**, emits [`CoreEvent::TimelineChanged`] iff the
    /// command actually changed the document. Unchanged commands (and rejected
    /// ones) emit nothing and do not move the version.
    pub fn apply(&self, command: EditCommand) -> Result<EditResult> {
        let result = {
            let mut session = self.lock();
            session.apply(command, self.ids.as_ref())?
        };
        if result.changed {
            self.events.emit(&CoreEvent::TimelineChanged {
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

    /// Replace the current session with a fresh, unsaved project and emit
    /// [`CoreEvent::ProjectOpened`] (path empty, version 0).
    pub fn new_project(&self) {
        {
            let mut session = self.lock();
            *session = EditorSession::new_project();
        }
        self.events.emit(&CoreEvent::ProjectOpened {
            path: String::new(),
            version: 0,
        });
    }

    /// Open the `.opentake` bundle at `path`, replacing the current session.
    /// Emits [`CoreEvent::ProjectOpened`] on success (the front end fetches the
    /// first snapshot itself, so no `TimelineChanged` is emitted —
    /// `core-SPEC.md` §5.4 step 6). Returns the first snapshot for convenience.
    pub fn open_project(&self, path: impl Into<PathBuf>) -> Result<TimelineSnapshot> {
        let path = path.into();
        let opened = EditorSession::open_project(&path)?;
        let snapshot = {
            let mut session = self.lock();
            *session = opened;
            TimelineSnapshot {
                timeline: session.timeline(),
                version: session.version(),
            }
        };
        self.events.emit(&CoreEvent::ProjectOpened {
            path: path.to_string_lossy().into_owned(),
            version: snapshot.version,
        });
        Ok(snapshot)
    }

    /// Save the current project. `path = None` saves back to the open bundle
    /// (autosave); `Some(path)` is a save-as. Emits [`CoreEvent::ProjectSaved`]
    /// with the written path on success.
    pub fn save_project(&self, path: Option<PathBuf>) -> Result<PathBuf> {
        let written = {
            let mut session = self.lock();
            session.save_project(path)?
        };
        self.events.emit(&CoreEvent::ProjectSaved {
            path: written.to_string_lossy().into_owned(),
        });
        Ok(written)
    }

    // MARK: - Media import

    /// A snapshot of the current media manifest (`get_media`). The catalog the
    /// media panel renders; reads are infallible.
    pub fn media(&self) -> MediaManifest {
        self.lock().media()
    }

    /// The open project's `.opentake` bundle directory, or `None` for an unsaved
    /// project. Needed to resolve [`MediaSource::Project`](opentake_domain::MediaSource)
    /// relative paths to on-disk files (preview/composite read the original media).
    pub fn project_dir(&self) -> Option<PathBuf> {
        self.lock().project_dir().map(|p| p.to_path_buf())
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
        let (entry, count) = {
            let mut session = self.lock();
            let entry = session.import_media_file(path, id, name, probe)?;
            let count = session.media().entries.len();
            (entry, count)
        };
        self.events.emit(&CoreEvent::MediaChanged { count });
        Ok(entry)
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
        let (entry, count) = {
            let mut session = self.lock();
            let entry = session.relink_media_file(asset_id, path, probe)?;
            let count = session.media().entries.len();
            (entry, count)
        };
        self.events.emit(&CoreEvent::MediaChanged { count });
        Ok(entry)
    }

    // MARK: - Internal

    /// Lock the session, recovering from a poisoned mutex by taking the inner
    /// guard. Command bodies are panic-free value-type ops, so poisoning is not
    /// expected; recovering keeps a stray panic in one observer from wedging the
    /// whole core.
    fn lock(&self) -> std::sync::MutexGuard<'_, EditorSession> {
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
            session.seed_from_timeline(tl);
        }
        core
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
        assert_eq!(events, vec![CoreEvent::TimelineChanged { version: 1 }]);
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
                CoreEvent::TimelineChanged { version } => *version,
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
    fn new_project_resets_and_emits_project_opened() {
        let core = core_with_track();
        core.apply(add_one_clip()).unwrap();
        assert_eq!(core.version(), 1);

        let seen: Arc<Mutex<Vec<CoreEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        core.subscribe(move |ev| sink.lock().unwrap().push(ev.clone()));

        core.new_project();
        assert_eq!(core.version(), 0);
        assert!(core.get_timeline().timeline.tracks.is_empty());
        assert_eq!(
            seen.lock().unwrap().clone(),
            vec![CoreEvent::ProjectOpened {
                path: String::new(),
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
        assert_eq!(snap.version, 0);

        // First core saw a ProjectSaved event with the dir path.
        let path_str = dir.to_string_lossy().into_owned();
        assert_eq!(
            seen.lock().unwrap().clone(),
            vec![CoreEvent::ProjectSaved { path: path_str }]
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
            vec![CoreEvent::MediaChanged { count: 1 }]
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
