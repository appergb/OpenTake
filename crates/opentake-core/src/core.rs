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

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard};

use opentake_domain::{
    ClipType, GenerationInput, MediaAsset, MediaManifest, MediaManifestEntry, MediaProxy, Timeline,
};
use opentake_ops::command::{ClipEntry, EditCommand, EditResult};
use opentake_ops::IdGen;
use opentake_project::{GenerationLog, ProjectCompatibility, ProjectRootIdentity};
use same_file::Handle;

use crate::deps::CoreDeps;
use crate::error::{CoreError, Result};
use crate::events::{CoreEvent, EventBus, SubscriptionId};
use crate::session::{
    DerivedStemProvenance, EditorSession, GenerationJobCommit, GenerationStateUpdate,
    PreparedGenerationJob, PreparedGenerationOutput, ProbedMedia,
};

type ProjectIdentityTransitionListener = Arc<dyn Fn(bool) + Send + Sync + 'static>;

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

/// Retained bundle authority for project-local asset reads. Consumers must
/// compare all fields again after any isolated I/O before exposing bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectAssetAuthority {
    pub project_epoch: u64,
    pub project_path: PathBuf,
    pub root_identity: ProjectRootIdentity,
}

/// Outcome of an assistant-owned conditional undo. The project identity,
/// document version, history transaction id, action label, and Undo application
/// are checked under one session lock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnedUndoResult {
    Undone(EditResult),
    NoHistory,
    Conflict {
        actual_action_name: Option<String>,
        actual_transaction_version: Option<u64>,
    },
}

/// One-lock snapshot of project revision plus the exact top undo transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectUndoSnapshot {
    pub revision: ProjectRevision,
    pub project_path: Option<PathBuf>,
    pub action_name: String,
    pub transaction_version: u64,
}

#[derive(Clone, Copy)]
struct EditExpectation<'a> {
    revision: ProjectRevision,
    project_path: Option<&'a Path>,
    check_project_path: bool,
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

/// Placement half of a project-managed motion render commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MotionPlacement {
    Add {
        start_frame: i32,
        duration_frames: i32,
        track_index: Option<usize>,
    },
    Replace {
        clip_id: String,
    },
    /// Replace a clip with a derivative that already contains the result of
    /// its masks, then clear those editable masks in the same undo snapshot.
    ReplaceAndClearMasks {
        clip_id: String,
    },
}

/// Result of atomically registering a rendered video and placing/replacing it.
#[derive(Clone, Debug)]
pub struct MotionMediaCommit {
    pub media: MediaManifestEntry,
    pub edit: EditResult,
}

/// A folder target in a prepared media-import plan. Planned folders are keyed
/// by the scanner without consuming application ids; existing folders retain
/// their authoritative project id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreparedMediaFolderRef {
    Planned(u64),
    Existing(String),
}

/// One filesystem-free operation in a prepared media-import plan. Directory
/// walking and probing happen before these values reach the session commit.
#[derive(Clone, Debug)]
pub enum PreparedMediaImportOp {
    CreateFolder {
        key: u64,
        name: String,
        parent: Option<PreparedMediaFolderRef>,
    },
    ImportFile {
        path: PathBuf,
        name: String,
        probe: ProbedMedia,
        folder: Option<PreparedMediaFolderRef>,
    },
    ImportDerivedStem {
        path: PathBuf,
        name: String,
        probe: ProbedMedia,
        provenance: DerivedStemProvenance,
    },
}

/// One file admitted by a successful durable batch import.
#[derive(Clone, Debug)]
pub struct CommittedMediaImport {
    pub path: PathBuf,
    pub entry: MediaManifestEntry,
}

/// Result of a capability-bound library import whose first manifest commit is
/// durable. `warning` is populated only when a failed postcondition could not
/// be rolled back; the entry remains authoritative and must be preserved.
#[derive(Clone, Debug)]
pub struct CapabilityImportCommit {
    pub entry: MediaManifestEntry,
    pub warning: Option<ImportCommitWarning>,
}

/// A committed import whose postcondition and exact rollback both failed.
/// Keeping the causes separate prevents UI layers from parsing an opaque
/// string while still reporting that the candidate remains authoritative.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportCommitWarning {
    PostconditionRollbackFailed {
        postcondition: String,
        rollback: String,
    },
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

impl PreparedProjectOpen {
    /// Confirm that the ambient bundle name still denotes the retained root
    /// that was prepared. This closes the prepare→scope/commit replacement
    /// window; asset reads additionally compare the retained root identity.
    pub fn is_current_namespace(&self) -> Result<bool> {
        self.editor.project_root_is_current_namespace()
    }

    pub fn project_asset_authority(&self) -> Option<(PathBuf, ProjectRootIdentity)> {
        Some((
            self.editor.project_dir()?.to_path_buf(),
            self.editor.project_root_identity()?,
        ))
    }
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

fn ensure_project_identity(
    session: &CoreSessionSlot,
    expected_project_epoch: u64,
    expected_project_dir: &Path,
) -> Result<()> {
    if session.project_epoch == expected_project_epoch
        && session.editor.project_dir() == Some(expected_project_dir)
    {
        Ok(())
    } else {
        Err(crate::CoreError::Media(
            "project changed during global library workflow".to_string(),
        ))
    }
}

fn resolve_prepared_folder(
    planned: &BTreeMap<u64, String>,
    folder: Option<PreparedMediaFolderRef>,
) -> Result<Option<String>> {
    match folder {
        None => Ok(None),
        Some(PreparedMediaFolderRef::Existing(id)) => Ok(Some(id)),
        Some(PreparedMediaFolderRef::Planned(key)) => planned
            .get(&key)
            .cloned()
            .map(Some)
            .ok_or_else(|| CoreError::Media(format!("prepared folder key not found: {key}"))),
    }
}

/// Events produced by an identity-bound external workflow. The workflow queues
/// them while its project lease is held, then emits only after releasing that
/// lease so synchronous subscribers may safely re-enter project lifecycle APIs.
#[derive(Default)]
pub struct DeferredCoreEvents {
    events: Vec<CoreEvent>,
}

impl DeferredCoreEvents {
    pub fn clear(&mut self) {
        self.events.clear();
    }

    fn push(&mut self, event: CoreEvent) {
        self.events.push(event);
    }
}

/// The cloneable handle to the one authoritative editing session.
#[derive(Clone)]
pub struct AppCore {
    session: Arc<Mutex<CoreSessionSlot>>,
    project_identity_workflow: Arc<RwLock<()>>,
    project_identity_transition: Arc<Mutex<Vec<ProjectIdentityTransitionListener>>>,
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
            project_identity_workflow: Arc::new(RwLock::new(())),
            project_identity_transition: Arc::new(Mutex::new(Vec::new())),
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
    pub fn subscribe(
        &self,
        listener: impl Fn(&CoreEvent) + Send + Sync + 'static,
    ) -> SubscriptionId {
        self.events.subscribe(listener)
    }

    pub fn emit_deferred(&self, events: DeferredCoreEvents) {
        for event in events.events {
            self.events.emit(&event);
        }
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

    /// Snapshot the exact retained bundle authority under the same session
    /// lock as its epoch/path. Isolated readers must compare this tuple again
    /// before publishing bytes so a project switch or namespace rebind fails
    /// closed.
    pub fn project_asset_authority(&self) -> Option<ProjectAssetAuthority> {
        let session = self.lock();
        Some(ProjectAssetAuthority {
            project_epoch: session.project_epoch,
            project_path: session.editor.project_dir()?.to_path_buf(),
            root_identity: session.editor.project_root_identity()?,
        })
    }

    pub fn project_asset_authority_matches(&self, expected: &ProjectAssetAuthority) -> bool {
        self.project_asset_authority().as_ref() == Some(expected)
    }

    /// Open a project-local asset through the retained no-follow bundle
    /// authority, under the same session lock that snapshots the authority.
    /// Callers that publish derived bytes should compare
    /// [`Self::project_asset_authority`] again before exposing them so a
    /// project switch or namespace rebind fails closed.
    pub fn open_project_asset(&self, relative: &Path) -> Result<fs::File> {
        let session = self.lock();
        session.editor.open_asset_file(relative)
    }

    /// Return a mutable-project runtime snapshot only when the caller's IPC
    /// identity still names the current project. This is the authorization gate
    /// for workflows that perform global I/O before their final project commit.
    pub fn mutable_runtime_snapshot_for_project(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
    ) -> Result<ProjectRuntimeSnapshot> {
        let session = self.lock();
        ensure_project_identity(&session, expected_project_epoch, expected_project_dir)?;
        session.editor.ensure_mutable()?;
        Ok(ProjectRuntimeSnapshot {
            timeline: session.editor.timeline(),
            media: session.editor.media(),
            project_dir: session.editor.project_dir().map(PathBuf::from),
            project_epoch: session.project_epoch,
            version: session.editor.version(),
        })
    }

    /// Require a caller-retained no-follow bundle handle to match the handle
    /// retained when the current project session was opened or saved.
    pub fn ensure_project_root_identity_for_project(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        current_root: &Handle,
    ) -> Result<()> {
        let session = self.lock();
        ensure_project_identity(&session, expected_project_epoch, expected_project_dir)?;
        if session.editor.matches_project_root_identity(current_root)? {
            Ok(())
        } else {
            Err(crate::CoreError::Media(
                "project bundle identity no longer matches the open session".to_string(),
            ))
        }
    }

    /// Hold the current project identity stable across an external workflow.
    /// Project replacement and save-as take the exclusive side of this lock.
    pub fn lock_project_identity_workflow(&self) -> RwLockReadGuard<'_, ()> {
        self.project_identity_workflow
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Register a synchronous project-identity transition hook. `true` runs
    /// immediately before replacement or Save As waits for the exclusive
    /// identity lease; `false` runs after the lease is released, on success or
    /// failure. Long external workflows use the pending interval to reject new
    /// work and request cooperative cancellation while old readers still hold
    /// the shared lease. A post-commit [`CoreEvent`] alone is too late because
    /// the writer cannot acquire the lease until those workflows return.
    pub fn subscribe_project_identity_transition(
        &self,
        listener: impl Fn(bool) + Send + Sync + 'static,
    ) {
        self.project_identity_transition
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(Arc::new(listener));
    }

    fn announce_project_identity_transition(&self, pending: bool) {
        let listeners = self
            .project_identity_transition
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        for listener in listeners {
            listener(pending);
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

    /// Label of the most recent undoable transaction, if any. Callers that act
    /// on this value must still use a revision-bound apply for the final Undo;
    /// the version comparison closes the check/commit race.
    pub fn undo_action_name(&self) -> Option<String> {
        self.lock().editor.undo_action_name().map(str::to_owned)
    }

    /// Stable version identity of the transaction currently at the top of the
    /// undo stack.
    pub fn undo_transaction_version(&self) -> Option<u64> {
        self.lock().editor.undo_transaction_version()
    }

    /// Read the full undo ownership tuple under one session lock.
    pub fn project_undo_snapshot(&self) -> Option<ProjectUndoSnapshot> {
        let session = self.lock();
        Some(ProjectUndoSnapshot {
            revision: ProjectRevision {
                project_epoch: session.project_epoch,
                version: session.editor.version(),
            },
            project_path: session.editor.project_dir().map(PathBuf::from),
            action_name: session.editor.undo_action_name()?.to_owned(),
            transaction_version: session.editor.undo_transaction_version()?,
        })
    }

    /// Undo only when the exact assistant-owned history transaction is still at
    /// the top of the same project revision. Equal action labels are insufficient:
    /// `expected_transaction_version` distinguishes a later user edit with the
    /// same label. All comparisons and the Undo share one session lock.
    pub fn undo_if_owned(
        &self,
        expected: ProjectRevision,
        expected_project_path: Option<&Path>,
        expected_action_name: &str,
        expected_transaction_version: u64,
    ) -> Result<OwnedUndoResult> {
        let outcome = {
            let mut session = self.lock();
            if session.project_epoch != expected.project_epoch
                || session.editor.version() != expected.version
                || session.editor.project_dir() != expected_project_path
            {
                return Err(CoreError::StaleProject);
            }
            if !session.editor.can_undo() {
                return Ok(OwnedUndoResult::NoHistory);
            }
            let actual_action_name = session.editor.undo_action_name().map(str::to_owned);
            let actual_transaction_version = session.editor.undo_transaction_version();
            if actual_action_name.as_deref() != Some(expected_action_name)
                || actual_transaction_version != Some(expected_transaction_version)
            {
                return Ok(OwnedUndoResult::Conflict {
                    actual_action_name,
                    actual_transaction_version,
                });
            }
            let edit = session.editor.apply(EditCommand::Undo, self.ids.as_ref())?;
            let media_count = edit
                .manifest_changed
                .then(|| session.editor.media().entries.len());
            (edit, session.project_epoch, media_count)
        };
        let (edit, project_epoch, media_count) = outcome;
        if edit.changed {
            self.events.emit(&CoreEvent::TimelineChanged {
                project_epoch,
                version: edit.timeline_version,
            });
        }
        if let Some(count) = media_count {
            self.events.emit(&CoreEvent::MediaChanged {
                project_epoch,
                count,
            });
        }
        Ok(OwnedUndoResult::Undone(edit))
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
    /// command actually changed the document. A command that changes the media
    /// manifest also emits [`CoreEvent::MediaChanged`] so catalog observers
    /// refresh; this includes undo/redo restoring a manifest snapshot. Unchanged
    /// commands (and rejected ones) emit nothing and do not move the version.
    pub fn apply(&self, command: EditCommand) -> Result<EditResult> {
        self.apply_with_revision(command, None)
    }

    /// Apply a deferred edit only if the project session and document version
    /// still match the snapshot from which the edit was derived.
    ///
    /// Long-running workflows such as transcription must not commit results
    /// built from project A after another client has opened or edited project B.
    /// The revision check and edit run under the same session lock, so there is
    /// no check-then-apply race.
    pub fn apply_at_revision(
        &self,
        expected: ProjectRevision,
        command: EditCommand,
    ) -> Result<EditResult> {
        self.apply_with_revision(
            command,
            Some(EditExpectation {
                revision: expected,
                project_path: None,
                check_project_path: false,
            }),
        )
    }

    /// Apply an IPC edit only when the project epoch, saved bundle path, and
    /// timeline version all still match the read-only mirror that produced the
    /// gesture. The complete identity check and edit transaction share the
    /// same session lock, so a delayed request can never fall through to a
    /// replacement project that happens to contain the same clip/media ids.
    pub fn apply_at_project_revision(
        &self,
        expected: ProjectRevision,
        expected_project_path: Option<&Path>,
        command: EditCommand,
    ) -> Result<EditResult> {
        self.apply_with_revision(
            command,
            Some(EditExpectation {
                revision: expected,
                project_path: expected_project_path,
                check_project_path: true,
            }),
        )
    }

    /// Apply one revision-bound edit and durably save the project under the
    /// same session lock. Persistence failure restores document, history, and
    /// version exactly before returning.
    pub fn apply_at_revision_persisted(
        &self,
        expected: ProjectRevision,
        command: EditCommand,
    ) -> Result<EditResult> {
        let (result, project_epoch, media_count, written) = {
            let mut session = self.lock();
            if session.project_epoch != expected.project_epoch
                || session.editor.version() != expected.version
            {
                return Err(CoreError::StaleProject);
            }
            let before = session.editor.checkpoint_editor_state();
            let outcome = (|| {
                let result = session.editor.apply(command, self.ids.as_ref())?;
                let media_count = result
                    .manifest_changed
                    .then(|| session.editor.media().entries.len());
                let written = session.editor.save_project(None)?;
                Ok((result, media_count, written))
            })();
            match outcome {
                Ok((result, media_count, written)) => {
                    (result, session.project_epoch, media_count, written)
                }
                Err(error) => {
                    session.editor.restore_editor_state(before);
                    return Err(error);
                }
            }
        };
        if result.changed {
            self.events.emit(&CoreEvent::TimelineChanged {
                project_epoch,
                version: result.timeline_version,
            });
        }
        if let Some(count) = media_count {
            self.events.emit(&CoreEvent::MediaChanged {
                project_epoch,
                count,
            });
        }
        self.events.emit(&CoreEvent::ProjectSaved {
            path: written.to_string_lossy().into_owned(),
            project_epoch,
        });
        Ok(result)
    }

    fn apply_with_revision(
        &self,
        command: EditCommand,
        expected: Option<EditExpectation<'_>>,
    ) -> Result<EditResult> {
        let (result, project_epoch, media_count) = {
            let mut session = self.lock();
            if expected.is_some_and(|expected| {
                session.project_epoch != expected.revision.project_epoch
                    || session.editor.version() != expected.revision.version
                    || (expected.check_project_path
                        && session.editor.project_dir() != expected.project_path)
            }) {
                return Err(CoreError::StaleProject);
            }
            let result = session.editor.apply(command, self.ids.as_ref())?;
            let media_count = result
                .manifest_changed
                .then(|| session.editor.media().entries.len());
            (result, session.project_epoch, media_count)
        };
        if result.changed {
            self.events.emit(&CoreEvent::TimelineChanged {
                project_epoch,
                version: result.timeline_version,
            });
        }
        if let Some(count) = media_count {
            self.events.emit(&CoreEvent::MediaChanged {
                project_epoch,
                count,
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
        self.announce_project_identity_transition(true);
        let _identity = self
            .project_identity_workflow
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let snapshot = {
            let mut session = self.lock();
            session.replace_editor(EditorSession::new_project())
        };
        drop(_identity);
        self.announce_project_identity_transition(false);
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
        self.announce_project_identity_transition(true);
        let _identity = self
            .project_identity_workflow
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let snapshot = {
            let mut session = self.lock();
            session.replace_editor(prepared.editor)
        };
        drop(_identity);
        self.announce_project_identity_transition(false);
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
        self.save_project_with_thumbnail_at_identity(None, None, path, thumbnail)
    }

    /// Save only if the project session still has the caller's exact identity.
    /// This binds deferred thumbnail generation and stale IPC requests to the
    /// project that initiated them; a concurrently opened replacement can never
    /// be written to the old request's Save As destination.
    pub fn save_project_with_thumbnail_for_project(
        &self,
        expected_project_epoch: u64,
        expected_project_path: Option<&Path>,
        path: Option<PathBuf>,
        thumbnail: Option<Vec<u8>>,
    ) -> Result<PathBuf> {
        self.save_project_with_thumbnail_at_identity(
            Some(expected_project_epoch),
            expected_project_path,
            path,
            thumbnail,
        )
    }

    fn save_project_with_thumbnail_at_identity(
        &self,
        expected_project_epoch: Option<u64>,
        expected_project_path: Option<&Path>,
        path: Option<PathBuf>,
        thumbnail: Option<Vec<u8>>,
    ) -> Result<PathBuf> {
        let changes_identity = path.is_some();
        if changes_identity {
            self.announce_project_identity_transition(true);
        }
        let _identity = self
            .project_identity_workflow
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let result = {
            let mut session = self.lock();
            if expected_project_epoch.is_some_and(|epoch| {
                session.project_epoch != epoch
                    || session.editor.project_dir() != expected_project_path
            }) {
                Err(CoreError::StaleProject)
            } else {
                session
                    .editor
                    .save_project_with_thumbnail(path, thumbnail)
                    .map(|written| (written, session.project_epoch))
            }
        };
        drop(_identity);
        if changes_identity {
            self.announce_project_identity_transition(false);
        }
        let (written, project_epoch) = result?;
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

    /// Persist placeholder assets and the queued audit event before a paid
    /// provider request is submitted. No ids are returned unless the project
    /// snapshot is durable.
    pub fn begin_generation_job_for_project(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        plan: PreparedGenerationJob,
    ) -> Result<GenerationJobCommit> {
        self.persist_generation_mutation(
            expected_project_epoch,
            expected_project_dir,
            |editor, ids| editor.begin_generation_job(plan, ids),
        )
    }

    pub fn update_generation_job_for_project(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        job_id: &str,
        update: GenerationStateUpdate,
    ) -> Result<usize> {
        self.persist_generation_mutation(
            expected_project_epoch,
            expected_project_dir,
            |editor, ids| editor.update_generation_job(job_id, update, ids),
        )
    }

    pub fn finalize_generation_output_for_project(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        output: PreparedGenerationOutput,
    ) -> Result<()> {
        self.persist_generation_mutation(
            expected_project_epoch,
            expected_project_dir,
            |editor, ids| editor.finalize_generation_output(output, ids),
        )
    }

    /// Finalize a generated output and stream its media bytes into the same
    /// complete-bundle publication as the manifest and generation log.
    pub fn finalize_generation_output_with_media_for_project(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        output: PreparedGenerationOutput,
        media_leaf: &str,
        media_byte_size: u64,
        media: &mut dyn std::io::Read,
    ) -> Result<()> {
        let expected_relative_path = format!("media/{media_leaf}");
        if output.relative_path != expected_relative_path {
            return Err(CoreError::Media(
                "generation output path does not match its media leaf".to_string(),
            ));
        }
        self.persist_generation_mutation_using(
            expected_project_epoch,
            expected_project_dir,
            |editor, ids| editor.finalize_generation_output(output, ids),
            |editor| editor.save_generation_state_with_media(media_leaf, media_byte_size, media),
        )
    }

    pub fn fail_generation_output_for_project(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        asset_id: &str,
        error_code: &str,
        created_at: Option<f64>,
    ) -> Result<()> {
        self.persist_generation_mutation(
            expected_project_epoch,
            expected_project_dir,
            |editor, ids| editor.fail_generation_output(asset_id, error_code, created_at, ids),
        )
    }

    pub fn cancel_generation_output_for_project(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        asset_id: &str,
        created_at: Option<f64>,
    ) -> Result<()> {
        self.persist_generation_mutation(
            expected_project_epoch,
            expected_project_dir,
            |editor, ids| editor.cancel_generation_output(asset_id, created_at, ids),
        )
    }

    fn persist_generation_mutation<T>(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        mutate: impl FnOnce(&mut EditorSession, &dyn IdGen) -> Result<T>,
    ) -> Result<T> {
        self.persist_generation_mutation_using(
            expected_project_epoch,
            expected_project_dir,
            mutate,
            |editor| editor.save_generation_state(),
        )
    }

    fn persist_generation_mutation_using<T>(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        mutate: impl FnOnce(&mut EditorSession, &dyn IdGen) -> Result<T>,
        persist: impl FnOnce(&mut EditorSession) -> Result<PathBuf>,
    ) -> Result<T> {
        let (value, count, written) = {
            let mut session = self.lock();
            ensure_project_identity(&session, expected_project_epoch, expected_project_dir)?;
            let checkpoint = session.editor.checkpoint_generation_state();
            let result = (|| {
                let value = mutate(&mut session.editor, self.ids.as_ref())?;
                let written = persist(&mut session.editor)?;
                Ok((value, written))
            })();
            match result {
                Ok((value, written)) => (value, session.editor.media().entries.len(), written),
                Err(error) => {
                    session.editor.restore_generation_state(checkpoint);
                    return Err(error);
                }
            }
        };
        self.events.emit(&CoreEvent::MediaChanged {
            project_epoch: expected_project_epoch,
            count,
        });
        self.events.emit(&CoreEvent::ProjectSaved {
            path: written.to_string_lossy().into_owned(),
            project_epoch: expected_project_epoch,
        });
        Ok(value)
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

    /// Prepare a local media manifest entry without registering it or emitting
    /// any event. The returned entry carries a fresh id, but the authoritative
    /// project remains byte-for-byte unchanged until a later edit command
    /// commits it. This is the safe preparation half of deferred render edits.
    pub fn prepare_media_file_entry(
        &self,
        path: impl AsRef<Path>,
        name: impl Into<String>,
        probe: &ProbedMedia,
    ) -> Result<MediaManifestEntry> {
        let id = self.ids.next_id();
        self.lock()
            .editor
            .prepare_media_file_entry(path, id, name, probe)
    }

    /// Import media only if the expected project still owns the session lock.
    ///
    /// Save-as-media renders without holding the core lock. Its final identity
    /// check and manifest mutation must therefore share this one critical
    /// section; a project replacement either happens before it (and the import
    /// is rejected) or after it (and the entry belongs to the expected project).
    pub fn import_media_file_for_project(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        path: impl AsRef<Path>,
        name: impl Into<String>,
        probe: &ProbedMedia,
    ) -> Result<MediaManifestEntry> {
        self.import_media_file_for_project_checked(
            expected_project_epoch,
            expected_project_dir,
            path,
            name,
            probe,
            || Ok(()),
        )
    }

    /// Project-bound import with a postcondition checked while the session lock
    /// is still held. The editor restores its pre-import manifest if the check
    /// fails, so callers can safely validate external filesystem state at the
    /// commit boundary without leaving a live dangling entry.
    pub fn import_media_file_for_project_checked(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        path: impl AsRef<Path>,
        name: impl Into<String>,
        probe: &ProbedMedia,
        postcondition: impl FnOnce() -> Result<()>,
    ) -> Result<MediaManifestEntry> {
        let id = self.ids.next_id();
        let (entry, count) = {
            let mut session = self.lock();
            if session.project_epoch != expected_project_epoch
                || session.editor.project_dir() != Some(expected_project_dir)
            {
                return Err(crate::CoreError::Media(
                    "project changed while saving media".to_string(),
                ));
            }
            let entry =
                session
                    .editor
                    .import_media_file_checked(path, id, name, probe, postcondition)?;
            let count = session.editor.media().entries.len();
            (entry, count)
        };
        self.events.emit(&CoreEvent::MediaChanged {
            project_epoch: expected_project_epoch,
            count,
        });
        Ok(entry)
    }

    /// Atomically register a completed project-managed motion render and place
    /// or replace its timeline clip.
    #[allow(clippy::too_many_arguments)]
    pub fn commit_motion_media_for_project(
        &self,
        expected_project_epoch: u64,
        expected_version: u64,
        expected_project_dir: &Path,
        path: impl AsRef<Path>,
        name: impl Into<String>,
        probe: &ProbedMedia,
        provenance: GenerationInput,
        placement: MotionPlacement,
    ) -> Result<MotionMediaCommit> {
        self.commit_generated_media_for_project(
            expected_project_epoch,
            expected_version,
            expected_project_dir,
            path,
            name,
            ClipType::Video,
            probe,
            provenance,
            placement,
            "Add Motion Graphic",
        )
    }

    /// Atomically register a completed generated audio/video file and place or
    /// replace its timeline clip. The generated file must already be a
    /// regular, non-symlink child of the active bundle's `media/` directory.
    /// The document command and project save share the session lock; any command
    /// or persistence failure restores timeline, manifest, undo/redo, and
    /// version exactly. A stale document version is rejected under that same
    /// lock. Events are emitted only after the durable save succeeds.
    #[allow(clippy::too_many_arguments)]
    pub fn commit_generated_media_for_project(
        &self,
        expected_project_epoch: u64,
        expected_version: u64,
        expected_project_dir: &Path,
        path: impl AsRef<Path>,
        name: impl Into<String>,
        kind: ClipType,
        probe: &ProbedMedia,
        provenance: GenerationInput,
        placement: MotionPlacement,
        action_name: &str,
    ) -> Result<MotionMediaCommit> {
        let path = path.as_ref();
        let media_dir = expected_project_dir.join(opentake_project::layout::MEDIA_DIR);
        if path.parent() != Some(media_dir.as_path())
            || path.file_name().is_none()
            || path.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
        {
            return Err(CoreError::Media(
                "generated output must be one direct child of the active project media directory"
                    .into(),
            ));
        }
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|error| CoreError::Media(format!("motion output metadata failed: {error}")))?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(CoreError::Media(
                "generated output must be a regular non-symlink file".into(),
            ));
        }

        let (commit, count, written) = {
            let mut session = self.lock();
            ensure_project_identity(&session, expected_project_epoch, expected_project_dir)?;
            if session.editor.version() != expected_version {
                return Err(CoreError::Media(
                    "project changed while preparing a generated-media edit".into(),
                ));
            }
            session.editor.ensure_mutable()?;
            let before = session.editor.checkpoint_editor_state();
            let id = loop {
                let candidate = self.ids.next_id();
                if session.editor.media_entry(&candidate).is_none() {
                    break candidate;
                }
            };

            let result = (|| {
                if !matches!(kind, ClipType::Audio | ClipType::Video) {
                    return Err(CoreError::Media(
                        "generated placement supports audio or video".into(),
                    ));
                }
                let mut asset = MediaAsset::new(id, path, kind, name, probe.duration_secs);
                asset.source_width = probe.width;
                asset.source_height = probe.height;
                asset.source_fps = probe.fps;
                asset.color = probe.color.clone();
                asset.has_audio = probe.has_audio;
                asset.generation_input = Some(provenance);
                let media = asset.to_manifest_entry(Some(expected_project_dir), 0.0);

                let command = match placement {
                    MotionPlacement::Add {
                        start_frame,
                        duration_frames,
                        track_index,
                    } => EditCommand::RegisterMediaAndAddClip {
                        entry: ClipEntry {
                            media_ref: media.id.clone(),
                            media_type: kind,
                            source_clip_type: kind,
                            track_index: track_index.unwrap_or(0),
                            start_frame,
                            duration_frames,
                            trim_start_frame: None,
                            trim_end_frame: None,
                            has_audio: probe.has_audio,
                            add_linked_audio: false,
                            transform: None,
                        },
                        media: media.clone(),
                        auto_track: track_index.is_none(),
                    },
                    MotionPlacement::Replace { clip_id } => EditCommand::RegisterMediaAndSwapClip {
                        media: media.clone(),
                        clip_id,
                    },
                    MotionPlacement::ReplaceAndClearMasks { clip_id } => {
                        EditCommand::RegisterMediaAndSwapClipClearingMasks {
                            media: media.clone(),
                            clip_id,
                        }
                    }
                };
                let mut edit = session.editor.apply(command, self.ids.as_ref())?;
                edit.action_name = action_name.to_string();
                edit.summary = format!("{} generated media clip(s)", edit.affected_clip_ids.len());
                let written = session.editor.save_project(None)?;
                Ok((MotionMediaCommit { media, edit }, written))
            })();

            match result {
                Ok((commit, written)) => {
                    let count = session.editor.media().entries.len();
                    (commit, count, written)
                }
                Err(error) => {
                    session.editor.restore_editor_state(before);
                    return Err(error);
                }
            }
        };

        self.events.emit(&CoreEvent::TimelineChanged {
            project_epoch: expected_project_epoch,
            version: commit.edit.timeline_version,
        });
        self.events.emit(&CoreEvent::MediaChanged {
            project_epoch: expected_project_epoch,
            count,
        });
        self.events.emit(&CoreEvent::ProjectSaved {
            path: written.to_string_lossy().into_owned(),
            project_epoch: expected_project_epoch,
        });
        Ok(commit)
    }

    /// Commit a fully probed media-import plan as one project-bound durable
    /// transaction. The session lock covers identity validation, all manifest
    /// edits, and the atomic `media.json` write. Any failure restores the exact
    /// pre-call editor state (manifest, undo/redo, and version); events are
    /// published only after the lock is released and the write succeeds.
    pub fn import_media_batch_for_project_persisted(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        plan: Vec<PreparedMediaImportOp>,
    ) -> Result<Vec<CommittedMediaImport>> {
        self.import_media_batch_for_project_with_writer(
            expected_project_epoch,
            expected_project_dir,
            plan,
            || Ok(()),
            |editor| editor.save_media_manifest(),
        )
    }

    /// Cancellable/project-bound batch import. `precondition` runs under the
    /// session lock immediately before the first manifest/history mutation.
    pub fn import_media_batch_for_project_persisted_checked(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        plan: Vec<PreparedMediaImportOp>,
        precondition: impl FnOnce() -> Result<()>,
    ) -> Result<Vec<CommittedMediaImport>> {
        self.import_media_batch_for_project_with_writer(
            expected_project_epoch,
            expected_project_dir,
            plan,
            precondition,
            |editor| editor.save_media_manifest(),
        )
    }

    fn import_media_batch_for_project_with_writer<P, F>(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        plan: Vec<PreparedMediaImportOp>,
        precondition: P,
        persist: F,
    ) -> Result<Vec<CommittedMediaImport>>
    where
        P: FnOnce() -> Result<()>,
        F: FnOnce(&mut EditorSession) -> Result<PathBuf>,
    {
        let (imports, count, initial_version, final_version, written) = {
            let mut session = self.lock();
            ensure_project_identity(&session, expected_project_epoch, expected_project_dir)?;
            session.editor.ensure_mutable()?;
            precondition()?;
            let before = session.editor.checkpoint_editor_state();
            let initial_version = session.editor.version();
            let result = (|| {
                let mut folder_ids = BTreeMap::<u64, String>::new();
                let mut imports = Vec::new();

                for operation in plan {
                    match operation {
                        PreparedMediaImportOp::CreateFolder { key, name, parent } => {
                            if folder_ids.contains_key(&key) {
                                return Err(CoreError::Media(format!(
                                    "duplicate prepared folder key: {key}"
                                )));
                            }
                            let parent_folder_id = resolve_prepared_folder(&folder_ids, parent)?;
                            let result = session.editor.apply(
                                EditCommand::CreateFolder {
                                    name,
                                    parent_folder_id,
                                },
                                self.ids.as_ref(),
                            )?;
                            let folder_id =
                                result.affected_clip_ids.into_iter().next().ok_or_else(|| {
                                    CoreError::Media(
                                        "folder creation returned no id during batch import"
                                            .to_string(),
                                    )
                                })?;
                            folder_ids.insert(key, folder_id);
                        }
                        PreparedMediaImportOp::ImportFile {
                            path,
                            name,
                            probe,
                            folder,
                        } => {
                            let folder_id = resolve_prepared_folder(&folder_ids, folder)?;
                            let id = self.ids.next_id();
                            let mut entry =
                                session.editor.import_media_file(&path, id, name, &probe)?;
                            if let Some(folder_id) = folder_id {
                                session.editor.apply(
                                    EditCommand::MoveToFolder {
                                        asset_ids: vec![entry.id.clone()],
                                        folder_id: Some(folder_id.clone()),
                                    },
                                    self.ids.as_ref(),
                                )?;
                                entry.folder_id = Some(folder_id);
                            }
                            imports.push(CommittedMediaImport { path, entry });
                        }
                        PreparedMediaImportOp::ImportDerivedStem {
                            path,
                            name,
                            probe,
                            provenance,
                        } => {
                            let id = self.ids.next_id();
                            let entry = session
                                .editor
                                .import_derived_stem_file(&path, id, name, &probe, provenance)?;
                            imports.push(CommittedMediaImport { path, entry });
                        }
                    }
                }

                let written = persist(&mut session.editor)?;
                Ok((imports, written))
            })();

            match result {
                Ok((imports, written)) => (
                    imports,
                    session.editor.media().entries.len(),
                    initial_version,
                    session.editor.version(),
                    written,
                ),
                Err(error) => {
                    session.editor.restore_editor_state(before);
                    return Err(error);
                }
            }
        };

        if final_version != initial_version {
            self.events.emit(&CoreEvent::TimelineChanged {
                project_epoch: expected_project_epoch,
                version: final_version,
            });
        }
        self.events.emit(&CoreEvent::MediaChanged {
            project_epoch: expected_project_epoch,
            count,
        });
        self.events.emit(&CoreEvent::ProjectSaved {
            path: written.to_string_lossy().into_owned(),
            project_epoch: expected_project_epoch,
        });
        Ok(imports)
    }

    /// Import one global-library file, bind its content id, and persist the
    /// project as a single in-memory transaction. Any import, mapping, or save
    /// error restores the exact pre-call manifest before the lock is released.
    pub fn import_library_media_for_project(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        path: impl AsRef<Path>,
        name: impl Into<String>,
        probe: &ProbedMedia,
        library_id: &str,
    ) -> Result<MediaManifestEntry> {
        let mut events = DeferredCoreEvents::default();
        let entry = self.import_library_media_for_project_deferred(
            expected_project_epoch,
            expected_project_dir,
            path,
            name,
            probe,
            library_id,
            &mut events,
        )?;
        self.emit_deferred(events);
        Ok(entry)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn import_library_media_for_project_deferred(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        path: impl AsRef<Path>,
        name: impl Into<String>,
        probe: &ProbedMedia,
        library_id: &str,
        events: &mut DeferredCoreEvents,
    ) -> Result<MediaManifestEntry> {
        let id = self.ids.next_id();
        let (entry, count) = {
            let mut session = self.lock();
            ensure_project_identity(&session, expected_project_epoch, expected_project_dir)?;
            let before = session.editor.media();
            let result = (|| {
                let entry = session.editor.import_media_file(path, id, name, probe)?;
                session
                    .editor
                    .set_media_global_favorite(&entry.id, Some(library_id.to_string()))?;
                session.editor.save_media_manifest()?;
                Ok(entry)
            })();
            match result {
                Ok(entry) => {
                    let count = session.editor.media().entries.len();
                    (entry, count)
                }
                Err(error) => {
                    session.editor.restore_media(before);
                    return Err(error);
                }
            }
        };
        events.push(CoreEvent::MediaChanged {
            project_epoch: expected_project_epoch,
            count,
        });
        events.push(CoreEvent::ProjectSaved {
            path: expected_project_dir.to_string_lossy().into_owned(),
            project_epoch: expected_project_epoch,
        });
        Ok(entry)
    }

    /// Capability-bound variant of the global-library import transaction. The
    /// caller persists the candidate manifest through retained directory
    /// authority; a writer error restores the exact live manifest before the
    /// session lock is released.
    #[allow(clippy::too_many_arguments)]
    pub fn import_library_media_for_project_deferred_with_manifest_writer<F, V>(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        path: impl AsRef<Path>,
        name: impl Into<String>,
        probe: &ProbedMedia,
        library_id: &str,
        events: &mut DeferredCoreEvents,
        mut write_manifest: F,
        validate_postcondition: V,
    ) -> Result<CapabilityImportCommit>
    where
        F: FnMut(&MediaManifest) -> Result<()>,
        V: FnOnce() -> Result<()>,
    {
        let id = self.ids.next_id();
        // Both callbacks execute while the session Mutex is held. They must do
        // only retained filesystem I/O and must never re-enter AppCore, emit an
        // event, acquire the project-identity lock, or acquire LibraryStore's
        // workflow/write locks.
        let (entry, warning, count) = {
            let mut session = self.lock();
            ensure_project_identity(&session, expected_project_epoch, expected_project_dir)?;
            let before = session.editor.media();
            let result = (|| {
                let entry = session.editor.import_media_file(path, id, name, probe)?;
                session
                    .editor
                    .set_media_global_favorite(&entry.id, Some(library_id.to_string()))?;
                write_manifest(&session.editor.media())?;
                let mut warning = None;
                if let Err(postcondition) = validate_postcondition() {
                    match write_manifest(&before) {
                        Ok(()) => return Err(postcondition),
                        Err(rollback) => {
                            warning = Some(ImportCommitWarning::PostconditionRollbackFailed {
                                postcondition: postcondition.to_string(),
                                rollback: rollback.to_string(),
                            });
                        }
                    }
                }
                Ok((entry, warning))
            })();
            match result {
                Ok((entry, warning)) => (entry, warning, session.editor.media().entries.len()),
                Err(error) => {
                    session.editor.restore_media(before);
                    return Err(error);
                }
            }
        };
        events.push(CoreEvent::MediaChanged {
            project_epoch: expected_project_epoch,
            count,
        });
        events.push(CoreEvent::ProjectSaved {
            path: expected_project_dir.to_string_lossy().into_owned(),
            project_epoch: expected_project_epoch,
        });
        Ok(CapabilityImportCommit { entry, warning })
    }

    /// Capability-bound retained-media import transaction used by external
    /// producers such as the MCP URL downloader. The candidate manifest is
    /// persisted through the caller's retained directory capability while the
    /// session lock is held. The retained-file/cancellation postcondition runs
    /// before the first persistent write; the atomic writer is the last fallible
    /// step. Any returned error therefore restores the exact pre-import live
    /// manifest without first publishing candidate manifest bytes.
    #[allow(clippy::too_many_arguments)]
    pub fn import_retained_media_for_project_deferred_with_manifest_writer<F, V>(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        path: impl AsRef<Path>,
        name: impl Into<String>,
        probe: &ProbedMedia,
        folder_id: Option<&str>,
        events: &mut DeferredCoreEvents,
        mut write_manifest: F,
        validate_postcondition: V,
    ) -> Result<CapabilityImportCommit>
    where
        F: FnMut(&MediaManifest) -> Result<()>,
        V: FnOnce() -> Result<()>,
    {
        let id = self.ids.next_id();
        let (entry, warning, count) = {
            let mut session = self.lock();
            ensure_project_identity(&session, expected_project_epoch, expected_project_dir)?;
            let before = session.editor.media();
            let result = (|| {
                let mut entry = session.editor.import_media_file(path, id, name, probe)?;
                if let Some(folder_id) = folder_id {
                    let mut candidate = session.editor.media();
                    if !candidate
                        .folders
                        .iter()
                        .any(|folder| folder.id == folder_id)
                    {
                        return Err(CoreError::Media(format!("folderId not found: {folder_id}")));
                    }
                    let imported = candidate
                        .entries
                        .iter_mut()
                        .find(|candidate| candidate.id == entry.id)
                        .ok_or_else(|| {
                            CoreError::Media("imported media entry disappeared".to_string())
                        })?;
                    imported.folder_id = Some(folder_id.to_string());
                    entry.folder_id = imported.folder_id.clone();
                    session.editor.restore_media(candidate);
                }
                // The retained leaf identity and cancellation checkpoint are
                // validated while the session lock is held, before the first
                // persistent manifest write. The capability writer is atomic,
                // so a returned writer error preserves the previous bytes and
                // there is no validation-failure rollback/warning state.
                validate_postcondition()?;
                write_manifest(&session.editor.media())?;
                Ok((entry, None))
            })();
            match result {
                Ok((entry, warning)) => (entry, warning, session.editor.media().entries.len()),
                Err(error) => {
                    session.editor.restore_media(before);
                    return Err(error);
                }
            }
        };
        events.push(CoreEvent::MediaChanged {
            project_epoch: expected_project_epoch,
            count,
        });
        events.push(CoreEvent::ProjectSaved {
            path: expected_project_dir.to_string_lossy().into_owned(),
            project_epoch: expected_project_epoch,
        });
        Ok(CapabilityImportCommit { entry, warning })
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

    pub fn set_media_favorite_for_project(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        asset_ids: &[String],
        favorite: bool,
    ) -> Result<usize> {
        let mut events = DeferredCoreEvents::default();
        let changed = self.set_media_favorite_for_project_deferred(
            expected_project_epoch,
            expected_project_dir,
            asset_ids,
            favorite,
            &mut events,
        )?;
        self.emit_deferred(events);
        Ok(changed)
    }

    pub fn set_media_favorite_for_project_deferred(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        asset_ids: &[String],
        favorite: bool,
        events: &mut DeferredCoreEvents,
    ) -> Result<usize> {
        let (changed, count) = {
            let mut session = self.lock();
            ensure_project_identity(&session, expected_project_epoch, expected_project_dir)?;
            let changed = session.editor.set_media_favorite(asset_ids, favorite)?;
            (changed, session.editor.media().entries.len())
        };
        if changed > 0 {
            events.push(CoreEvent::MediaChanged {
                project_epoch: expected_project_epoch,
                count,
            });
        }
        Ok(changed)
    }

    /// Persist one asset's playback proxy as an atomic manifest mutation.
    /// Failure to write restores the in-memory manifest before the lock is
    /// released, so UI and disk never disagree about proxy availability.
    pub fn set_media_proxy_for_project(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        asset_id: &str,
        proxy: Option<MediaProxy>,
    ) -> Result<MediaManifestEntry> {
        let (entry, count, written) = {
            let mut session = self.lock();
            ensure_project_identity(&session, expected_project_epoch, expected_project_dir)?;
            let before = session.editor.media();
            let entry = session.editor.set_media_proxy(asset_id, proxy)?;
            let count = session.editor.media().entries.len();
            match session.editor.save_media_manifest() {
                Ok(written) => (entry, count, written),
                Err(error) => {
                    session.editor.restore_media(before);
                    return Err(error);
                }
            }
        };
        self.events.emit(&CoreEvent::MediaChanged {
            project_epoch: expected_project_epoch,
            count,
        });
        self.events.emit(&CoreEvent::ProjectSaved {
            path: written.to_string_lossy().into_owned(),
            project_epoch: expected_project_epoch,
        });
        Ok(entry)
    }

    /// Set or clear one project's global-favorite mapping, emitting the same
    /// media-change signal used by other manifest mutations.
    pub fn set_media_global_favorite(
        &self,
        asset_id: &str,
        library_id: Option<String>,
    ) -> Result<bool> {
        let (changed, count, project_epoch) = {
            let mut session = self.lock();
            let changed = session
                .editor
                .set_media_global_favorite(asset_id, library_id)?;
            let count = session.editor.media().entries.len();
            (changed, count, session.project_epoch)
        };
        if changed {
            self.events.emit(&CoreEvent::MediaChanged {
                project_epoch,
                count,
            });
        }
        Ok(changed)
    }

    /// Project-identity-checked variant for workflows that perform global
    /// library I/O before updating the current project mirror.
    pub fn set_media_global_favorite_for_project(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        asset_id: &str,
        library_id: Option<String>,
    ) -> Result<bool> {
        let mut events = DeferredCoreEvents::default();
        let changed = self.set_media_global_favorite_for_project_deferred(
            expected_project_epoch,
            expected_project_dir,
            asset_id,
            library_id,
            &mut events,
        )?;
        self.emit_deferred(events);
        Ok(changed)
    }

    pub fn set_media_global_favorite_for_project_deferred(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        asset_id: &str,
        library_id: Option<String>,
        events: &mut DeferredCoreEvents,
    ) -> Result<bool> {
        let (changed, count) = {
            let mut session = self.lock();
            ensure_project_identity(&session, expected_project_epoch, expected_project_dir)?;
            let changed = session
                .editor
                .set_media_global_favorite(asset_id, library_id)?;
            (changed, session.editor.media().entries.len())
        };
        if changed {
            events.push(CoreEvent::MediaChanged {
                project_epoch: expected_project_epoch,
                count,
            });
        }
        Ok(changed)
    }

    /// Clear every current-project mapping for a removed global-library id.
    pub fn clear_media_global_favorite_id(&self, library_id: &str) -> Result<usize> {
        let (changed, count, project_epoch) = {
            let mut session = self.lock();
            let changed = session.editor.clear_media_global_favorite_id(library_id)?;
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

    pub fn clear_media_global_favorite_id_for_project(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        library_id: &str,
    ) -> Result<usize> {
        let mut events = DeferredCoreEvents::default();
        let changed = self.clear_media_global_favorite_id_for_project_deferred(
            expected_project_epoch,
            expected_project_dir,
            library_id,
            &mut events,
        )?;
        self.emit_deferred(events);
        Ok(changed)
    }

    pub fn clear_media_global_favorite_id_for_project_deferred(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        library_id: &str,
        events: &mut DeferredCoreEvents,
    ) -> Result<usize> {
        let (changed, count) = {
            let mut session = self.lock();
            ensure_project_identity(&session, expected_project_epoch, expected_project_dir)?;
            let changed = session.editor.clear_media_global_favorite_id(library_id)?;
            (changed, session.editor.media().entries.len())
        };
        if changed > 0 {
            events.push(CoreEvent::MediaChanged {
                project_epoch: expected_project_epoch,
                count,
            });
        }
        Ok(changed)
    }

    /// Atomically save only the media manifest if the originating project still
    /// owns the session lock.
    pub fn save_media_manifest_for_project(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
    ) -> Result<PathBuf> {
        let mut events = DeferredCoreEvents::default();
        let written = self.save_media_manifest_for_project_deferred(
            expected_project_epoch,
            expected_project_dir,
            &mut events,
        )?;
        self.emit_deferred(events);
        Ok(written)
    }

    pub fn save_media_manifest_for_project_deferred(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        events: &mut DeferredCoreEvents,
    ) -> Result<PathBuf> {
        let written = {
            let mut session = self.lock();
            ensure_project_identity(&session, expected_project_epoch, expected_project_dir)?;
            session.editor.save_media_manifest()?
        };
        events.push(CoreEvent::ProjectSaved {
            path: written.to_string_lossy().into_owned(),
            project_epoch: expected_project_epoch,
        });
        Ok(written)
    }

    /// Restore an exact media snapshot and persist it while the expected
    /// project still owns the session. External workflows use this only to
    /// roll back a postcondition failure before deferred events are emitted.
    pub fn restore_media_manifest_for_project_deferred(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        manifest: MediaManifest,
        events: &mut DeferredCoreEvents,
    ) -> Result<()> {
        let mut session = self.lock();
        ensure_project_identity(&session, expected_project_epoch, expected_project_dir)?;
        session.editor.restore_media(manifest);
        session.editor.save_media_manifest()?;
        events.clear();
        Ok(())
    }

    /// Restore and persist a manifest through caller-supplied retained
    /// capability authority. If persistence fails, put the prior live state
    /// back so memory continues to agree with the last successful disk commit.
    pub fn restore_media_manifest_for_project_deferred_with_manifest_writer<F>(
        &self,
        expected_project_epoch: u64,
        expected_project_dir: &Path,
        manifest: MediaManifest,
        events: &mut DeferredCoreEvents,
        write_manifest: F,
    ) -> Result<()>
    where
        F: FnOnce(&MediaManifest) -> Result<()>,
    {
        let mut session = self.lock();
        ensure_project_identity(&session, expected_project_epoch, expected_project_dir)?;
        let before = session.editor.media();
        session.editor.restore_media(manifest);
        if let Err(error) = write_manifest(&session.editor.media()) {
            session.editor.restore_media(before);
            return Err(error);
        }
        events.clear();
        Ok(())
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
        let (entry, count, project_epoch, saved) = {
            let mut session = self.lock();
            let before = session.editor.media();
            let entry = session.editor.relink_media_file(asset_id, path, probe)?;
            let count = session.editor.media().entries.len();
            let saved = if session.editor.project_dir().is_some() {
                match session.editor.save_media_manifest() {
                    Ok(path) => Some(path),
                    Err(error) => {
                        session.editor.restore_media(before);
                        return Err(error);
                    }
                }
            } else {
                None
            };
            (entry, count, session.project_epoch, saved)
        };
        self.events.emit(&CoreEvent::MediaChanged {
            project_epoch,
            count,
        });
        if let Some(path) = saved {
            self.events.emit(&CoreEvent::ProjectSaved {
                path: path.to_string_lossy().into_owned(),
                project_epoch,
            });
        }
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
    use opentake_domain::{Clip, ClipType, MediaColorMetadata, MediaProxy, Timeline, Track};
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

    #[test]
    fn motion_media_commit_is_durable_atomic_and_one_step_undoable() {
        let bundle = project_bundle("motion-commit");
        let core = AppCore::new();
        core.open_project(&bundle).unwrap();
        let media_dir = opentake_project::layout::media_dir(&bundle);
        std::fs::create_dir_all(&media_dir).unwrap();
        let rendered = media_dir.join("motion-a.mp4");
        std::fs::write(&rendered, b"validated-render-fixture").unwrap();
        let snapshot = core.runtime_snapshot();
        let probe = ProbedMedia {
            duration_secs: 1.0,
            width: Some(64),
            height: Some(36),
            fps: Some(30.0),
            has_audio: false,
            color: None,
        };

        let committed = core
            .commit_motion_media_for_project(
                snapshot.project_epoch,
                snapshot.version,
                &bundle,
                &rendered,
                "Motion A",
                &probe,
                GenerationInput {
                    prompt: "{\"templateId\":\"title-card\"}".into(),
                    model: "opentake.motion-canvas".into(),
                    duration: 30,
                    aspect_ratio: "64:36".into(),
                    provider: Some("opentake-motion".into()),
                    status: Some(opentake_domain::GenerationJobStatus::Ready),
                    ..GenerationInput::default()
                },
                MotionPlacement::Add {
                    start_frame: 0,
                    duration_frames: 30,
                    track_index: Some(0),
                },
            )
            .unwrap();
        assert_eq!(committed.edit.action_name, "Add Motion Graphic");
        assert_eq!(core.media().entries.len(), 2);
        assert_eq!(core.get_timeline().timeline.tracks[0].clips.len(), 1);

        let reopened = AppCore::new();
        reopened.open_project(&bundle).unwrap();
        assert_eq!(reopened.media().entries.len(), 2);
        assert_eq!(reopened.get_timeline().timeline.tracks[0].clips.len(), 1);

        core.undo().unwrap();
        assert_eq!(core.media().entries.len(), 1);
        assert!(core.get_timeline().timeline.tracks[0].clips.is_empty());
        let _ = std::fs::remove_dir_all(bundle);
    }

    #[test]
    fn generated_media_commit_refuses_version_drift_without_mutation() {
        let bundle = project_bundle("generated-version-drift");
        let core = AppCore::new();
        core.open_project(&bundle).unwrap();
        let media_dir = opentake_project::layout::media_dir(&bundle);
        std::fs::create_dir_all(&media_dir).unwrap();
        let rendered = media_dir.join("stale-render.mp4");
        std::fs::write(&rendered, b"validated-render-fixture").unwrap();
        let stale = core.runtime_snapshot();
        core.apply(EditCommand::SetTimelineSettings {
            fps: 24,
            width: 1280,
            height: 720,
        })
        .unwrap();
        let before_commit = core.runtime_snapshot();

        let error = core
            .commit_motion_media_for_project(
                stale.project_epoch,
                stale.version,
                &bundle,
                &rendered,
                "Stale Render",
                &ProbedMedia::default(),
                GenerationInput::default(),
                MotionPlacement::Add {
                    start_frame: 0,
                    duration_frames: 1,
                    track_index: Some(0),
                },
            )
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("project changed while preparing a generated-media edit"));
        let after_commit = core.runtime_snapshot();
        assert_eq!(after_commit.timeline, before_commit.timeline);
        assert_eq!(after_commit.media, before_commit.media);
        assert_eq!(after_commit.version, before_commit.version);

        let _ = std::fs::remove_dir_all(bundle);
    }

    #[test]
    fn motion_media_commit_rejects_output_outside_active_bundle_without_mutation() {
        let bundle = project_bundle("motion-outside");
        let core = AppCore::new();
        core.open_project(&bundle).unwrap();
        let outside = std::env::temp_dir().join(format!(
            "opentake-motion-outside-{}-{}.mp4",
            std::process::id(),
            core.runtime_snapshot().project_epoch
        ));
        std::fs::write(&outside, b"outside").unwrap();
        let before = core.runtime_snapshot();

        let error = core
            .commit_motion_media_for_project(
                before.project_epoch,
                before.version,
                &bundle,
                &outside,
                "Outside",
                &ProbedMedia::default(),
                GenerationInput::default(),
                MotionPlacement::Add {
                    start_frame: 0,
                    duration_frames: 1,
                    track_index: Some(0),
                },
            )
            .unwrap_err();
        assert!(error.to_string().contains("active project media"));
        let after = core.runtime_snapshot();
        assert_eq!(after.timeline, before.timeline);
        assert_eq!(after.media, before.media);
        assert_eq!(after.version, before.version);

        let _ = std::fs::remove_file(outside);
        let _ = std::fs::remove_dir_all(bundle);
    }

    #[cfg(unix)]
    #[test]
    fn motion_media_commit_rejects_symlink_without_mutation() {
        use std::os::unix::fs::symlink;

        let bundle = project_bundle("motion-symlink");
        let core = AppCore::new();
        core.open_project(&bundle).unwrap();
        let media_dir = opentake_project::layout::media_dir(&bundle);
        std::fs::create_dir_all(&media_dir).unwrap();
        let target = media_dir.join("motion-target.mp4");
        let linked = media_dir.join("motion-linked.mp4");
        std::fs::write(&target, b"validated-render-fixture").unwrap();
        symlink(&target, &linked).unwrap();
        let before = core.runtime_snapshot();

        let error = core
            .commit_motion_media_for_project(
                before.project_epoch,
                before.version,
                &bundle,
                &linked,
                "Linked",
                &ProbedMedia::default(),
                GenerationInput::default(),
                MotionPlacement::Add {
                    start_frame: 0,
                    duration_frames: 1,
                    track_index: Some(0),
                },
            )
            .unwrap_err();
        assert!(error.to_string().contains("regular non-symlink"));
        let after = core.runtime_snapshot();
        assert_eq!(after.timeline, before.timeline);
        assert_eq!(after.media, before.media);
        assert_eq!(after.version, before.version);

        let _ = std::fs::remove_dir_all(bundle);
    }

    #[test]
    fn project_identity_workflow_blocks_project_replacement_until_release() {
        let core = AppCore::new();
        let workflow = core.lock_project_identity_workflow();
        let replacement = core.clone();
        let (sent, received) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            replacement.new_project();
            sent.send(()).unwrap();
        });

        assert!(received
            .recv_timeout(std::time::Duration::from_millis(50))
            .is_err());
        drop(workflow);
        received
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("replacement proceeds after workflow releases identity");
        worker.join().unwrap();
    }

    #[test]
    fn identity_bound_save_never_writes_a_replacement_project_to_the_old_request_target() {
        let first = project_bundle("save-identity-first");
        let second = project_bundle("save-identity-second");
        let destination = std::env::temp_dir().join(format!(
            "opentake-core-stale-save-{}-{}.opentake",
            std::process::id(),
            first
                .file_stem()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or("fixture")
        ));
        let _ = std::fs::remove_dir_all(&destination);
        let core = AppCore::new();
        core.open_project(&first).unwrap();
        let expected = core.runtime_snapshot();
        core.open_project(&second).unwrap();

        let error = core
            .save_project_with_thumbnail_for_project(
                expected.project_epoch,
                expected.project_dir.as_deref(),
                Some(destination.clone()),
                Some(b"stale thumbnail".to_vec()),
            )
            .expect_err("replacement project must reject the stale save");

        assert!(matches!(error, CoreError::StaleProject));
        assert!(!destination.exists());
        assert_eq!(
            core.runtime_snapshot().project_dir.as_deref(),
            Some(second.as_path())
        );

        let _ = std::fs::remove_dir_all(first);
        let _ = std::fs::remove_dir_all(second);
        let _ = std::fs::remove_dir_all(destination);
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

    fn seed_same_video_clip(core: &AppCore, clip_id: &str) {
        let mut slot = core.session.lock().unwrap();
        let mut timeline = Timeline::new();
        let mut track = Track::new("same-track", ClipType::Video);
        track
            .clips
            .push(Clip::new(clip_id, "source-asset", 100, 60));
        timeline.tracks.push(track);
        slot.editor.seed_from_timeline(timeline);
    }

    fn registered_freeze(media: MediaManifestEntry, clip_id: &str) -> EditCommand {
        EditCommand::RegisterMediaAndFreezeFrame {
            media,
            clip_id: clip_id.to_string(),
            at_frame: 130,
            duration_frames: 30,
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
        assert!(res.timeline_changed);
        assert!(!res.manifest_changed);
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
    fn deferred_apply_rejects_version_and_project_drift_without_mutation() {
        let version_drift = core_with_track();
        let expected = version_drift.project_revision();
        version_drift.apply(add_one_clip()).unwrap();
        let before = version_drift.runtime_snapshot();
        let error = version_drift
            .apply_at_revision(expected, add_one_clip())
            .expect_err("stale document version must reject deferred edit");
        assert_eq!(
            error.to_string(),
            "project changed while preparing a deferred edit"
        );
        let after = version_drift.runtime_snapshot();
        assert_eq!(after.project_epoch, before.project_epoch);
        assert_eq!(after.version, before.version);
        assert_eq!(after.timeline, before.timeline);
        assert_eq!(after.media, before.media);

        let project_drift = core_with_track();
        let expected = project_drift.project_revision();
        project_drift.new_project();
        let before = project_drift.runtime_snapshot();
        let error = project_drift
            .apply_at_revision(expected, add_one_clip())
            .expect_err("stale project epoch must reject deferred edit");
        assert_eq!(
            error.to_string(),
            "project changed while preparing a deferred edit"
        );
        let after = project_drift.runtime_snapshot();
        assert_eq!(after.project_epoch, before.project_epoch);
        assert_eq!(after.version, before.version);
        assert_eq!(after.timeline, before.timeline);
        assert_eq!(after.media, before.media);
    }

    #[test]
    fn ipc_edit_revision_rejects_save_as_path_drift_without_mutation() {
        let temp = tempfile::tempdir().unwrap();
        let first = temp.path().join("first.opentake");
        let second = temp.path().join("second.opentake");
        let core = core_with_track();
        core.save_project(Some(first.clone())).unwrap();
        let expected = core.project_revision();

        core.save_project(Some(second.clone())).unwrap();
        assert_eq!(
            core.project_revision(),
            expected,
            "Save As keeps the session revision"
        );
        let before = core.runtime_snapshot();
        let error = core
            .apply_at_project_revision(expected, Some(&first), add_one_clip())
            .expect_err("an edit captured before Save As must not target the new bundle path");

        assert!(matches!(error, CoreError::StaleProject));
        let after = core.runtime_snapshot();
        assert_eq!(after.project_dir.as_deref(), Some(second.as_path()));
        assert_eq!(after.project_epoch, before.project_epoch);
        assert_eq!(after.version, before.version);
        assert_eq!(after.timeline, before.timeline);
        assert_eq!(after.media, before.media);
    }

    #[test]
    fn registered_freeze_rejects_version_drift_without_manifest_or_events() {
        let core = core_with_track();
        let added = core.apply(add_one_clip()).unwrap();
        let clip_id = added.affected_clip_ids[0].clone();
        let expected = core.project_revision();
        let media = core
            .prepare_media_file_entry(
                std::env::temp_dir().join("freeze-version-drift.png"),
                "Freeze",
                &ProbedMedia::default(),
            )
            .unwrap();
        core.apply(EditCommand::CreateFolder {
            name: "version drift".into(),
            parent_folder_id: None,
        })
        .unwrap();
        let before = core.runtime_snapshot();
        let seen: Arc<Mutex<Vec<CoreEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        core.subscribe(move |event| sink.lock().unwrap().push(event.clone()));

        let error = core
            .apply_at_project_revision(expected, None, registered_freeze(media, &clip_id))
            .expect_err("a capture prepared before another edit must be rejected");

        assert!(matches!(error, CoreError::StaleProject));
        let after = core.runtime_snapshot();
        assert_eq!(after.timeline, before.timeline);
        assert_eq!(after.media, before.media);
        assert_eq!(after.version, before.version);
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn registered_freeze_rejects_project_drift_even_with_same_clip_id() {
        let core = AppCore::new();
        seed_same_video_clip(&core, "same-clip");
        let expected = core.project_revision();
        let media = core
            .prepare_media_file_entry(
                std::env::temp_dir().join("freeze-project-drift.png"),
                "Freeze",
                &ProbedMedia::default(),
            )
            .unwrap();

        core.new_project();
        seed_same_video_clip(&core, "same-clip");
        let before = core.runtime_snapshot();
        let seen: Arc<Mutex<Vec<CoreEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        core.subscribe(move |event| sink.lock().unwrap().push(event.clone()));

        let error = core
            .apply_at_project_revision(expected, None, registered_freeze(media, "same-clip"))
            .expect_err("the replacement project must not accept an old capture");

        assert!(matches!(error, CoreError::StaleProject));
        let after = core.runtime_snapshot();
        assert_eq!(after.timeline, before.timeline);
        assert_eq!(after.media, before.media);
        assert_eq!(after.version, before.version);
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn manifest_edit_and_undo_emit_media_changed() {
        let core = core_with_track();
        let seen: Arc<Mutex<Vec<CoreEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        core.subscribe(move |ev| sink.lock().unwrap().push(ev.clone()));

        let created = core
            .apply(EditCommand::CreateFolder {
                name: "Review".into(),
                parent_folder_id: None,
            })
            .unwrap();
        assert!(created.changed);
        assert!(!created.timeline_changed);
        assert!(created.manifest_changed);
        assert_eq!(core.media().folders.len(), 1);

        let undone = core.undo().unwrap();
        assert!(undone.changed);
        assert!(!undone.timeline_changed);
        assert!(undone.manifest_changed);
        assert!(core.media().folders.is_empty());

        assert_eq!(
            *seen.lock().unwrap(),
            vec![
                CoreEvent::TimelineChanged {
                    project_epoch: 0,
                    version: 1,
                },
                CoreEvent::MediaChanged {
                    project_epoch: 0,
                    count: 0,
                },
                CoreEvent::TimelineChanged {
                    project_epoch: 0,
                    version: 2,
                },
                CoreEvent::MediaChanged {
                    project_epoch: 0,
                    count: 0,
                },
            ]
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
        assert!(!res.timeline_changed);
        assert!(!res.manifest_changed);
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
            color: None,
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
    fn hdr_and_proxy_metadata_persist_across_project_reopen() {
        let temp = tempfile::tempdir().unwrap();
        let bundle = temp.path().join("ColorProxy.opentake");
        let source = temp.path().join("source.mp4");
        std::fs::write(&source, b"source").unwrap();
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let entry = core
            .import_media_file(
                &source,
                "source",
                &ProbedMedia {
                    duration_secs: 1.0,
                    width: Some(1920),
                    height: Some(1080),
                    fps: Some(24.0),
                    has_audio: false,
                    color: Some(MediaColorMetadata {
                        primaries: Some("bt2020".into()),
                        transfer: Some("smpte2084".into()),
                        matrix: Some("bt2020nc".into()),
                        range: Some("tv".into()),
                    }),
                },
            )
            .unwrap();
        core.save_project(None).unwrap();
        let proxy_relative = "media/proxies/source.mp4";
        std::fs::create_dir_all(bundle.join("media/proxies")).unwrap();
        std::fs::write(bundle.join(proxy_relative), b"proxy").unwrap();
        let revision = core.runtime_snapshot();
        core.set_media_proxy_for_project(
            revision.project_epoch,
            &bundle,
            &entry.id,
            Some(MediaProxy {
                relative_path: proxy_relative.into(),
                source_sha256: "a".repeat(64),
                width: 640,
                height: 360,
            }),
        )
        .unwrap();

        let reopened = AppCore::new();
        reopened.open_project(&bundle).unwrap();
        let restored = reopened.media().entries.into_iter().next().unwrap();
        assert!(restored.color.as_ref().is_some_and(|color| color.is_hdr()));
        assert_eq!(restored.proxy.unwrap().relative_path, proxy_relative);
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

    #[test]
    fn import_media_for_project_rejects_replacement_without_mutating_manifest() {
        let sequence = CoreIdGen::default().next_id();
        let root = std::env::temp_dir().join(format!(
            "opentake-core-conditional-import-{}-{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let project_a = root.join("A.opentake");
        let project_b = root.join("B.opentake");
        let core = AppCore::new();
        core.save_project(Some(project_a.clone()))
            .expect("save project A");
        let expected_epoch = core.runtime_snapshot().project_epoch;

        opentake_project::Project::new(&project_b)
            .save()
            .expect("save project B");
        core.open_project(project_b).expect("switch to project B");
        let before = serde_json::to_vec(&core.media()).expect("serialize B manifest");

        let error = core
            .import_media_file_for_project(
                expected_epoch,
                &project_a,
                project_a.join("media/rendered.wav"),
                "rendered.wav",
                &ProbedMedia::default(),
            )
            .expect_err("stale project import must be rejected");

        assert_eq!(error.to_string(), "project changed while saving media");
        assert_eq!(
            serde_json::to_vec(&core.media()).expect("serialize B manifest after rejection"),
            before
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn project_scoped_media_mutations_reject_a_replacement_project() {
        let sequence = CoreIdGen::default().next_id();
        let root = std::env::temp_dir().join(format!(
            "opentake-core-project-scoped-media-{}-{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let project_a = root.join("A.opentake");
        let project_b = root.join("B.opentake");
        let core = AppCore::new();
        core.save_project(Some(project_a.clone())).unwrap();
        let entry = core
            .import_media_file("/abs/a.mp4", "a", &ProbedMedia::default())
            .unwrap();
        core.save_project(None).unwrap();
        let expected_epoch = core.runtime_snapshot().project_epoch;
        opentake_project::Project::new(&project_b).save().unwrap();
        core.open_project(project_b).unwrap();
        let before = core.media();

        assert!(core
            .set_media_global_favorite_for_project(
                expected_epoch,
                &project_a,
                &entry.id,
                Some("content-hash".into()),
            )
            .is_err());
        assert!(core
            .import_media_file_for_project(
                expected_epoch,
                &project_a,
                "/abs/rendered.mp4",
                "rendered",
                &ProbedMedia::default(),
            )
            .is_err());
        assert!(core
            .import_library_media_for_project(
                expected_epoch,
                &project_a,
                "/abs/library.mp4",
                "library",
                &ProbedMedia::default(),
                "content-hash",
            )
            .is_err());
        assert!(core
            .save_media_manifest_for_project(expected_epoch, &project_a)
            .is_err());
        assert_eq!(core.media(), before);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn prepared_media_batch_rejects_project_replacement_without_pollution() {
        let sequence = CoreIdGen::default().next_id();
        let root = std::env::temp_dir().join(format!(
            "opentake-core-stale-media-batch-{}-{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let project_a = root.join("A.opentake");
        let project_b = root.join("B.opentake");
        let core = AppCore::new();
        core.save_project(Some(project_a.clone())).unwrap();
        let expected_epoch = core.runtime_snapshot().project_epoch;
        opentake_project::Project::new(&project_b).save().unwrap();
        core.open_project(project_b.clone()).unwrap();
        let before_b = core.media();

        let error = core
            .import_media_batch_for_project_persisted(
                expected_epoch,
                &project_a,
                vec![
                    PreparedMediaImportOp::CreateFolder {
                        key: 0,
                        name: "stale".to_string(),
                        parent: None,
                    },
                    PreparedMediaImportOp::ImportFile {
                        path: PathBuf::from("/abs/stale.mp4"),
                        name: "stale".to_string(),
                        probe: ProbedMedia::default(),
                        folder: Some(PreparedMediaFolderRef::Planned(0)),
                    },
                ],
            )
            .expect_err("stale batch must be rejected");

        assert!(error.to_string().contains("project changed"));
        assert_eq!(core.media(), before_b);
        let reopened_a = AppCore::new();
        reopened_a.open_project(project_a).unwrap();
        assert!(reopened_a.media().entries.is_empty());
        assert!(reopened_a.media().folders.is_empty());
        let reopened_b = AppCore::new();
        reopened_b.open_project(project_b).unwrap();
        assert_eq!(reopened_b.media(), before_b);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn prepared_media_batch_writer_failure_restores_full_editor_state() {
        let sequence = CoreIdGen::default().next_id();
        let root = std::env::temp_dir().join(format!(
            "opentake-core-media-batch-rollback-{}-{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let project = root.join("Rollback.opentake");
        let core = AppCore::new();
        core.save_project(Some(project.clone())).unwrap();
        core.apply(EditCommand::CreateFolder {
            name: "existing".to_string(),
            parent_folder_id: None,
        })
        .unwrap();
        core.undo().unwrap();
        core.save_media_manifest_for_project(core.runtime_snapshot().project_epoch, &project)
            .unwrap();
        let before_media = core.media();
        let before_version = core.version();
        let before_can_undo = core.can_undo();
        let before_can_redo = core.can_redo();
        let expected_epoch = core.runtime_snapshot().project_epoch;
        let seen: Arc<Mutex<Vec<CoreEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        core.subscribe(move |event| sink.lock().unwrap().push(event.clone()));

        let error = core
            .import_media_batch_for_project_with_writer(
                expected_epoch,
                &project,
                vec![
                    PreparedMediaImportOp::CreateFolder {
                        key: 0,
                        name: "candidate".to_string(),
                        parent: None,
                    },
                    PreparedMediaImportOp::ImportFile {
                        path: PathBuf::from("/abs/candidate.mp4"),
                        name: "candidate".to_string(),
                        probe: ProbedMedia::default(),
                        folder: Some(PreparedMediaFolderRef::Planned(0)),
                    },
                ],
                || Ok(()),
                |_| {
                    Err(CoreError::Media(
                        "injected manifest write failure".to_string(),
                    ))
                },
            )
            .expect_err("writer failure must abort the batch");

        assert_eq!(error.to_string(), "injected manifest write failure");
        assert_eq!(core.media(), before_media);
        assert_eq!(core.version(), before_version);
        assert_eq!(core.can_undo(), before_can_undo);
        assert_eq!(core.can_redo(), before_can_redo);
        assert!(seen.lock().unwrap().is_empty());
        let reopened = AppCore::new();
        reopened.open_project(project).unwrap();
        assert_eq!(reopened.media(), before_media);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn global_favorite_interfaces_emit_media_changed_only_on_change() {
        let core = AppCore::new();
        let entry = core
            .import_media_file("/abs/a.mp4", "a", &ProbedMedia::default())
            .unwrap();
        let seen: Arc<Mutex<Vec<CoreEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        core.subscribe(move |event| sink.lock().unwrap().push(event.clone()));

        assert!(core
            .set_media_global_favorite(&entry.id, Some("content-hash".into()))
            .unwrap());
        assert!(!core
            .set_media_global_favorite(&entry.id, Some("content-hash".into()))
            .unwrap());
        assert_eq!(
            core.clear_media_global_favorite_id("content-hash").unwrap(),
            1
        );

        assert_eq!(
            seen.lock().unwrap().clone(),
            vec![
                CoreEvent::MediaChanged {
                    project_epoch: 0,
                    count: 1,
                },
                CoreEvent::MediaChanged {
                    project_epoch: 0,
                    count: 1,
                },
            ]
        );
    }

    /// Composite acceptance entry tracked by the data-safety implementation
    /// plan. These child slices cover authoritative versions/events, stale edit
    /// refusal, manifest undo, coherent concurrent snapshots, and save/reopen.
    #[test]
    fn cross_cutting_runtime_acceptance() {
        apply_bumps_version_and_emits_once();
        deferred_apply_rejects_version_and_project_drift_without_mutation();
        manifest_edit_and_undo_emit_media_changed();
        undo_redo_through_core_bumps_version_and_emits();
        runtime_snapshot_never_mixes_timeline_media_and_project_dir();
        open_save_roundtrip_through_core_emits_lifecycle_events();
        prepared_media_batch_writer_failure_restores_full_editor_state();
    }
}
