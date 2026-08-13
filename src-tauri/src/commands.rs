//! The `#[tauri::command]` surface.
//!
//! Each command is a thin shim over an `opentake_core::dto::handle_*` function
//! (which wraps [`AppCore`]). Project New/Open additionally share one boundary
//! single-flight gate so asynchronous project preparation cannot race another
//! lifecycle transition. Core editing/history/save commands preserve
//! `CmdError` at the IPC boundary; playback-aware lifecycle commands preserve
//! their structured error code so callers never need to parse display text.
//!
//! `EditCommand` itself is not `Deserialize` (it carries engine value types with
//! no serde derives), so the editing entry point takes a local serde-friendly
//! [`EditRequest`] that maps 1:1 onto the variants the front end issues in v1.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use opentake_core::core::PreparedProjectOpen;
use opentake_core::dto::{
    handle_edit_apply_at_project_revision, handle_get_timeline, handle_project_new, EditResultDto,
    TimelineSnapshotDto,
};
use opentake_core::{AppCore, CmdError, EditCommand, ProjectRevision};

use opentake_ops::command::{
    NewTrackClipMode, PasteClipEntry, PlaceMediaTarget, ProjectTimelineSettings, UnplacedClipEntry,
};
use opentake_ops::{
    CaptionEntry, ClipEntry, ClipMove, ClipProperties, FrameRange, KeyframePayload,
    KeyframeProperty, KeyframeValue, RenameEntry, TextAutoTrackEntry, TextEntry,
};

use opentake_domain::{
    AnimPair, AudioDenoise, ChromaKey, Clip, ClipType, ColorGrade, Crop, Effect, Interpolation,
    Keyframe, KeyframeTrack, LoudnessNormalization, LutReference, Mask, StabilizationTrack,
    TextStyle, Transform, TransitionKind,
};

const MAX_CONCURRENT_PROJECT_PREPARES: usize = 4;

#[derive(Clone)]
pub(crate) struct ProjectLifecycleCoordinator {
    gate: std::sync::Arc<tokio::sync::Mutex<()>>,
    prepare_slots: std::sync::Arc<tokio::sync::Semaphore>,
    timed_out_paths:
        std::sync::Arc<std::sync::Mutex<std::collections::HashSet<std::path::PathBuf>>>,
}

#[derive(Clone, Debug)]
struct ProjectLifecycleLease {
    _guard: std::sync::Arc<tokio::sync::OwnedMutexGuard<()>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PrepareOperationStatus {
    Running,
    TimedOut,
    Finished,
}

#[derive(Debug)]
struct PrepareOperationState {
    path: std::path::PathBuf,
    status: std::sync::Mutex<PrepareOperationStatus>,
    timed_out_paths:
        std::sync::Arc<std::sync::Mutex<std::collections::HashSet<std::path::PathBuf>>>,
}

#[derive(Debug)]
struct ProjectPrepareAdmission {
    state: std::sync::Arc<PrepareOperationState>,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

impl Default for ProjectLifecycleCoordinator {
    fn default() -> Self {
        Self {
            gate: std::sync::Arc::new(tokio::sync::Mutex::new(())),
            prepare_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(
                MAX_CONCURRENT_PROJECT_PREPARES,
            )),
            timed_out_paths: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::HashSet::new(),
            )),
        }
    }
}

impl PrepareOperationState {
    fn mark_timed_out(&self) {
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *status != PrepareOperationStatus::Running {
            return;
        }
        self.timed_out_paths
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(self.path.clone());
        *status = PrepareOperationStatus::TimedOut;
    }
}

impl Drop for ProjectPrepareAdmission {
    fn drop(&mut self) {
        let mut status = self
            .state
            .status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *status == PrepareOperationStatus::TimedOut {
            self.state
                .timed_out_paths
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(&self.state.path);
        }
        *status = PrepareOperationStatus::Finished;
    }
}

impl ProjectLifecycleCoordinator {
    fn try_acquire(&self) -> Result<ProjectLifecycleLease, String> {
        self.gate
            .clone()
            .try_lock_owned()
            .map(|guard| ProjectLifecycleLease {
                _guard: std::sync::Arc::new(guard),
            })
            .map_err(|_| "another project lifecycle transition is already in progress".to_string())
    }

    fn try_admit_prepare(&self, path: &std::path::Path) -> Result<ProjectPrepareAdmission, String> {
        if self
            .timed_out_paths
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains(path)
        {
            return Err(format!(
                "a timed-out project prepare is still finishing for {}",
                path.display()
            ));
        }
        let permit = self
            .prepare_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| "too many timed-out project prepares are still finishing".to_string())?;
        Ok(ProjectPrepareAdmission {
            state: std::sync::Arc::new(PrepareOperationState {
                path: path.to_path_buf(),
                status: std::sync::Mutex::new(PrepareOperationStatus::Running),
                timed_out_paths: self.timed_out_paths.clone(),
            }),
            _permit: permit,
        })
    }
}

// MARK: - Read / lifecycle commands (direct DTO passthrough)

/// `get_timeline`: current read-only mirror + version. Infallible.
#[tauri::command]
pub fn get_timeline(core: State<'_, AppCore>) -> TimelineSnapshotDto {
    handle_get_timeline(&core)
}

/// `generation_log`: the current project's append-only AI generation audit log
/// (rows with model / credits / timestamps, persisted as
/// `generation-log.json`). Read-only mirror of `AppCore::generation_log()`;
/// there is deliberately no mutation path — the log is only ever appended by
/// the core's generation lifecycle (upstream `editor.generationLog`, surfaced
/// as Palmier Pro's generation-activity view). Infallible: a session with no
/// project (or a project with no generations) yields the empty log
/// (`version: 1`, no entries).
#[tauri::command]
pub fn generation_log(core: State<'_, AppCore>) -> opentake_project::GenerationLog {
    core.generation_log()
}

/// `undo` / `redo`: global history navigation, bound to the project mirror
/// that enabled the corresponding command in the UI.
#[tauri::command]
pub fn undo(
    core: State<'_, AppCore>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    expected_project_epoch: u64,
    expected_timeline_version: u64,
    expected_project_path: Option<String>,
) -> Result<EditResultDto, CmdError> {
    let _admission = begin_edit_activity(&admission)?;
    handle_edit_apply_at_project_revision(
        &core,
        ProjectRevision {
            project_epoch: expected_project_epoch,
            version: expected_timeline_version,
        },
        expected_project_path.as_deref().map(std::path::Path::new),
        EditCommand::Undo,
    )
}

#[tauri::command]
pub fn redo(
    core: State<'_, AppCore>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    expected_project_epoch: u64,
    expected_timeline_version: u64,
    expected_project_path: Option<String>,
) -> Result<EditResultDto, CmdError> {
    let _admission = begin_edit_activity(&admission)?;
    handle_edit_apply_at_project_revision(
        &core,
        ProjectRevision {
            project_epoch: expected_project_epoch,
            version: expected_timeline_version,
        },
        expected_project_path.as_deref().map(std::path::Path::new),
        EditCommand::Redo,
    )
}

/// `project_new`: replace the session with a fresh project and return its first
/// snapshot. When `path` is supplied, build and persist the new bundle away
/// from the live session, then install it atomically only after preparation
/// succeeds.
#[cfg(feature = "playback-engine")]
#[tauri::command]
pub async fn project_new(
    app: AppHandle,
    path: Option<String>,
) -> Result<TimelineSnapshotDto, crate::playback::session::PlaybackCommandError> {
    let _update_activity = crate::updater::begin_mutating_activity(
        &app.state::<crate::updater::InstallAdmissionGate>(),
    )
    .map_err(crate::playback::session::PlaybackCommandError::busy)?;
    let coordinator = app.state::<ProjectLifecycleCoordinator>();
    let lifecycle = coordinator
        .try_acquire()
        .map_err(crate::playback::session::PlaybackCommandError::busy)?;
    if let Some(path) = path {
        let path = std::path::PathBuf::from(path);
        if !crate::safe_asset_protocol::scope_allows_lexical_path(
            &app.asset_protocol_scope(),
            &path,
        ) {
            return Err(crate::playback::session::PlaybackCommandError::engine(
                "project path has not been approved by a native file dialog",
            ));
        }
        app.state::<crate::playback::PlaybackState>()
            .ensure_project_transition_available()?;
        let admission = coordinator
            .try_admit_prepare(&path)
            .map_err(crate::playback::session::PlaybackCommandError::busy)?;
        let prepared = prepare_saved_project_off_thread(
            path.clone(),
            admission,
            app.state::<crate::updater::InstallAdmissionGate>()
                .inner()
                .clone(),
        )
        .await
        .map_err(crate::playback::session::PlaybackCommandError::engine)?;
        if !prepared.is_current_namespace().map_err(|error| {
            crate::playback::session::PlaybackCommandError::engine(error.to_string())
        })? {
            return Err(crate::playback::session::PlaybackCommandError::engine(
                "project bundle changed while it was being prepared",
            ));
        }
        let result = commit_prepared_project_open_with_playback_and_prewarm(
            &app.state::<AppCore>(),
            prepared,
            &app.state::<crate::playback::PlaybackState>(),
            &app.state::<crate::media::prewarm::PrewarmScheduler>(),
        );
        drop(lifecycle);
        return result;
    }
    let result = project_new_with_playback_and_prewarm(
        &app.state::<AppCore>(),
        &app.state::<crate::playback::PlaybackState>(),
        &app.state::<crate::media::prewarm::PrewarmScheduler>(),
    );
    drop(lifecycle);
    result
}

#[cfg(all(feature = "playback-engine", test))]
pub(crate) fn project_new_with_playback(
    core: &AppCore,
    playback: &crate::playback::PlaybackState,
) -> Result<TimelineSnapshotDto, crate::playback::session::PlaybackCommandError> {
    let prewarm =
        crate::media::prewarm::PrewarmScheduler::new(core.project_revision().project_epoch);
    project_new_with_playback_and_prewarm(core, playback, &prewarm)
}

#[cfg(feature = "playback-engine")]
fn project_new_with_playback_and_prewarm(
    core: &AppCore,
    playback: &crate::playback::PlaybackState,
    prewarm: &crate::media::prewarm::PrewarmScheduler,
) -> Result<TimelineSnapshotDto, crate::playback::session::PlaybackCommandError> {
    let transition = playback.begin_project_transition()?;
    if let Err(error) = prewarm.begin_project_transition() {
        playback.cancel_project_transition(transition);
        return Err(crate::playback::session::PlaybackCommandError::busy(error));
    }
    let snapshot = handle_project_new(core);
    playback.activate_project(transition, snapshot.project_epoch);
    prewarm.activate_project(snapshot.project_epoch);
    Ok(snapshot)
}

#[cfg(not(feature = "playback-engine"))]
#[tauri::command]
pub async fn project_new(
    app: AppHandle,
    path: Option<String>,
) -> Result<TimelineSnapshotDto, String> {
    let _update_activity = crate::updater::begin_mutating_activity(
        &app.state::<crate::updater::InstallAdmissionGate>(),
    )?;
    let coordinator = app.state::<ProjectLifecycleCoordinator>();
    let lifecycle = coordinator.try_acquire()?;
    if let Some(path) = path {
        let path = std::path::PathBuf::from(path);
        if !crate::safe_asset_protocol::scope_allows_lexical_path(
            &app.asset_protocol_scope(),
            &path,
        ) {
            return Err("project path has not been approved by a native file dialog".into());
        }
        let admission = coordinator.try_admit_prepare(&path)?;
        let prepared = prepare_saved_project_off_thread(
            path.clone(),
            admission,
            app.state::<crate::updater::InstallAdmissionGate>()
                .inner()
                .clone(),
        )
        .await?;
        if !prepared
            .is_current_namespace()
            .map_err(|error| error.to_string())?
        {
            return Err("project bundle changed while it was being prepared".into());
        }
        let core = app.state::<AppCore>();
        let prewarm = app.state::<crate::media::prewarm::PrewarmScheduler>();
        prewarm.begin_project_transition()?;
        let snapshot = TimelineSnapshotDto::from(core.commit_project_open(prepared));
        prewarm.activate_project(snapshot.project_epoch);
        drop(lifecycle);
        return Ok(snapshot);
    }
    let core = app.state::<AppCore>();
    let prewarm = app.state::<crate::media::prewarm::PrewarmScheduler>();
    prewarm.begin_project_transition()?;
    let snapshot = handle_project_new(&core);
    prewarm.activate_project(snapshot.project_epoch);
    drop(lifecycle);
    Ok(snapshot)
}

/// `project_open`: open a `.opentake` bundle, returning the first snapshot.
const PROJECT_LIFECYCLE_PREPARE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

async fn run_blocking_with_timeout<T, F>(
    operation: &'static str,
    timeout: std::time::Duration,
    admission: ProjectPrepareAdmission,
    build: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let operation_state = admission.state.clone();
    let task = tokio::task::spawn_blocking(move || {
        let _admission = admission;
        build()
    });
    match tokio::time::timeout(timeout, task).await {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => Err(format!("{operation} task failed: {error}")),
        Err(_) => {
            operation_state.mark_timed_out();
            Err(format!("{operation} timed out after {timeout:?}"))
        }
    }
}

async fn prepare_project_open_off_thread(
    path: std::path::PathBuf,
    admission: ProjectPrepareAdmission,
) -> Result<PreparedProjectOpen, String> {
    run_blocking_with_timeout(
        "project open",
        PROJECT_LIFECYCLE_PREPARE_TIMEOUT,
        admission,
        move || {
            if crate::fs_availability::project_bundle_has_dataless_components(&path) {
                return Err(
                    "项目文件尚未下载到本机，请先在 Finder 中下载后再打开 / Project files are cloud-only; download them in Finder before opening"
                        .to_string(),
                );
            }
            AppCore::prepare_project_open(path).map_err(|error| error.to_string())
        },
    )
    .await
}

async fn prepare_saved_project_off_thread(
    path: std::path::PathBuf,
    admission: ProjectPrepareAdmission,
    update_admission: crate::updater::InstallAdmissionGate,
) -> Result<PreparedProjectOpen, String> {
    let worker_activity = crate::updater::begin_mutating_activity(&update_admission)?;
    run_blocking_with_timeout(
        "project create",
        PROJECT_LIFECYCLE_PREPARE_TIMEOUT,
        admission,
        move || {
            // spawn_blocking cannot be cancelled when the caller's timeout
            // expires. Keep update admission until this worker really stops so
            // its bundle writes cannot cross the install/save barrier.
            let _worker_activity = worker_activity;
            AppCore::new()
                .save_project(Some(path.clone()))
                .map_err(|error| error.to_string())?;
            AppCore::prepare_project_open(path).map_err(|error| error.to_string())
        },
    )
    .await
}

#[cfg(feature = "playback-engine")]
#[tauri::command]
pub async fn project_open(
    app: AppHandle,
    path: String,
) -> Result<TimelineSnapshotDto, crate::playback::session::PlaybackCommandError> {
    let _update_activity = crate::updater::begin_mutating_activity(
        &app.state::<crate::updater::InstallAdmissionGate>(),
    )
    .map_err(crate::playback::session::PlaybackCommandError::busy)?;
    let coordinator = app.state::<ProjectLifecycleCoordinator>();
    let lifecycle = coordinator
        .try_acquire()
        .map_err(crate::playback::session::PlaybackCommandError::busy)?;
    // Fail fast if another project transition is already active. Only the
    // cloneable lifecycle lease crosses into the blocking filesystem prepare.
    app.state::<crate::playback::PlaybackState>()
        .ensure_project_transition_available()?;
    let path = std::path::PathBuf::from(path);
    if !crate::safe_asset_protocol::scope_allows_lexical_path(&app.asset_protocol_scope(), &path) {
        return Err(crate::playback::session::PlaybackCommandError::engine(
            "project path has not been approved by a native file dialog",
        ));
    }
    let admission = coordinator
        .try_admit_prepare(&path)
        .map_err(crate::playback::session::PlaybackCommandError::busy)?;
    let prepared = prepare_project_open_off_thread(path.clone(), admission)
        .await
        .map_err(crate::playback::session::PlaybackCommandError::engine)?;
    if !prepared.is_current_namespace().map_err(|error| {
        crate::playback::session::PlaybackCommandError::engine(error.to_string())
    })? {
        return Err(crate::playback::session::PlaybackCommandError::engine(
            "project bundle changed while it was being prepared",
        ));
    }
    let result = commit_prepared_project_open_with_playback_and_prewarm(
        &app.state::<AppCore>(),
        prepared,
        &app.state::<crate::playback::PlaybackState>(),
        &app.state::<crate::media::prewarm::PrewarmScheduler>(),
    );
    drop(lifecycle);
    result
}

#[cfg(all(feature = "playback-engine", test))]
pub(crate) fn project_open_with_playback(
    core: &AppCore,
    path: String,
    playback: &crate::playback::PlaybackState,
) -> Result<TimelineSnapshotDto, crate::playback::session::PlaybackCommandError> {
    let prewarm =
        crate::media::prewarm::PrewarmScheduler::new(core.project_revision().project_epoch);
    project_open_with_playback_and_prewarm(core, path, playback, &prewarm)
}

#[cfg(all(feature = "playback-engine", test))]
pub(crate) fn project_open_with_playback_and_prewarm(
    core: &AppCore,
    path: String,
    playback: &crate::playback::PlaybackState,
    prewarm: &crate::media::prewarm::PrewarmScheduler,
) -> Result<TimelineSnapshotDto, crate::playback::session::PlaybackCommandError> {
    playback.ensure_project_transition_available()?;
    let prepared =
        AppCore::prepare_project_open(std::path::PathBuf::from(path)).map_err(|error| {
            crate::playback::session::PlaybackCommandError::engine(error.to_string())
        })?;
    commit_prepared_project_open_with_playback_and_prewarm(core, prepared, playback, prewarm)
}

#[cfg(feature = "playback-engine")]
fn commit_prepared_project_open_with_playback_and_prewarm(
    core: &AppCore,
    prepared: PreparedProjectOpen,
    playback: &crate::playback::PlaybackState,
    prewarm: &crate::media::prewarm::PrewarmScheduler,
) -> Result<TimelineSnapshotDto, crate::playback::session::PlaybackCommandError> {
    let transition = playback.begin_project_transition()?;
    if let Err(error) = prewarm.begin_project_transition() {
        playback.cancel_project_transition(transition);
        return Err(crate::playback::session::PlaybackCommandError::busy(error));
    }
    let snapshot = TimelineSnapshotDto::from(core.commit_project_open(prepared));
    playback.activate_project(transition, snapshot.project_epoch);
    prewarm.activate_project(snapshot.project_epoch);
    Ok(snapshot)
}

#[cfg(not(feature = "playback-engine"))]
#[tauri::command]
pub async fn project_open(app: AppHandle, path: String) -> Result<TimelineSnapshotDto, String> {
    let _update_activity = crate::updater::begin_mutating_activity(
        &app.state::<crate::updater::InstallAdmissionGate>(),
    )?;
    let coordinator = app.state::<ProjectLifecycleCoordinator>();
    let lifecycle = coordinator.try_acquire()?;
    let path = std::path::PathBuf::from(path);
    if !crate::safe_asset_protocol::scope_allows_lexical_path(&app.asset_protocol_scope(), &path) {
        return Err("project path has not been approved by a native file dialog".into());
    }
    let admission = coordinator.try_admit_prepare(&path)?;
    let prepared = prepare_project_open_off_thread(path.clone(), admission).await?;
    if !prepared
        .is_current_namespace()
        .map_err(|error| error.to_string())?
    {
        return Err("project bundle changed while it was being prepared".into());
    }
    let core = app.state::<AppCore>();
    let prewarm = app.state::<crate::media::prewarm::PrewarmScheduler>();
    prewarm.begin_project_transition()?;
    let snapshot = TimelineSnapshotDto::from(core.commit_project_open(prepared));
    prewarm.activate_project(snapshot.project_epoch);
    drop(lifecycle);
    Ok(snapshot)
}

/// `project_save`: `path = None` saves back to the open bundle; `Some` is save-as.
///
/// Before delegating to the core save, capture one stable representative frame
/// through the authoritative preview/export compositor. Capture and JPEG encode
/// happen before the identity-bound core save acquires its final session lock;
/// `None` therefore leaves the last valid cover untouched without introducing a
/// GPU/session lock cycle.
#[tauri::command]
pub async fn project_save(
    app: AppHandle,
    path: Option<String>,
    expected_project_epoch: u64,
    expected_project_path: Option<String>,
) -> Result<String, CmdError> {
    save_project_with_composite_cover(app, path, expected_project_epoch, expected_project_path)
        .await
}

const PROJECT_COVER_SAVE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

#[derive(Debug, PartialEq, Eq)]
enum ProjectCoverCapture {
    Captured(Vec<u8>),
    NoVisibleContent,
    CaptureFailed,
}

pub(crate) async fn save_project_with_composite_cover<R: tauri::Runtime>(
    app: AppHandle<R>,
    path: Option<String>,
    expected_project_epoch: u64,
    expected_project_path: Option<String>,
) -> Result<String, CmdError> {
    if let Some(target) = path.as_deref().map(std::path::Path::new) {
        if !crate::safe_asset_protocol::scope_allows_lexical_path(
            &app.asset_protocol_scope(),
            target,
        ) {
            return Err(validation_error(
                "project path has not been approved by a native file dialog".to_string(),
            ));
        }
    }
    let cancel = opentake_media::MediaCancelToken::new();
    let worker_cancel = cancel.clone();
    let deadline = std::time::Instant::now() + PROJECT_COVER_SAVE_TIMEOUT;
    let task = tauri::async_runtime::spawn_blocking(move || {
        let admission = app.state::<crate::updater::InstallAdmissionGate>();
        let _activity =
            crate::updater::begin_mutating_activity(&admission).map_err(validation_error)?;
        let core = app.state::<AppCore>();
        let render = app.state::<crate::render::RenderState>();
        save_project_with_composite_cover_blocking(
            &app,
            &core,
            &render,
            path,
            expected_project_epoch,
            expected_project_path,
            &worker_cancel,
            deadline,
        )
    });
    match tokio::time::timeout(PROJECT_COVER_SAVE_TIMEOUT, task).await {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => Err(internal_error(format!("project save task failed: {error}"))),
        Err(_) => {
            cancel.cancel();
            Err(internal_error(format!(
                "project save timed out after {PROJECT_COVER_SAVE_TIMEOUT:?}"
            )))
        }
    }
}

/// CloseRequested parity entry point. It snapshots the current identity once
/// and delegates to exactly the same bounded authoritative-cover save helper as
/// the explicit Save command.
pub(crate) async fn save_current_project_with_composite_cover<R: tauri::Runtime>(
    app: AppHandle<R>,
) -> Result<String, CmdError> {
    let snapshot = app.state::<AppCore>().runtime_snapshot();
    save_project_with_composite_cover(
        app,
        None,
        snapshot.project_epoch,
        snapshot
            .project_dir
            .map(|path| path.to_string_lossy().into_owned()),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
fn save_project_with_composite_cover_blocking<R: tauri::Runtime>(
    app: &AppHandle<R>,
    core: &AppCore,
    render: &crate::render::RenderState,
    path: Option<String>,
    expected_project_epoch: u64,
    expected_project_path: Option<String>,
    cancel: &opentake_media::MediaCancelToken,
    deadline: std::time::Instant,
) -> Result<String, CmdError> {
    project_save_for_project_with_checkpoint(
        core,
        path,
        expected_project_epoch,
        expected_project_path,
        |snapshot| capture_composite_project_thumbnail(app, core, snapshot, render, cancel),
        || !cancel.is_cancelled() && std::time::Instant::now() < deadline,
    )
}

fn capture_composite_project_thumbnail<R: tauri::Runtime>(
    app: &AppHandle<R>,
    core: &AppCore,
    snapshot: &opentake_core::ProjectRuntimeSnapshot,
    render: &crate::render::RenderState,
    cancel: &opentake_media::MediaCancelToken,
) -> ProjectCoverCapture {
    let bounds = opentake_media::PROJECT_COMPOSITE_COVER_BOUNDS;
    let frame_index = match crate::render::representative_timeline_frame(
        &snapshot.timeline,
        &snapshot.media,
        bounds.0.max(bounds.1),
    ) {
        Ok(Some(frame)) => frame,
        Ok(None) => return ProjectCoverCapture::NoVisibleContent,
        Err(_) => return ProjectCoverCapture::CaptureFailed,
    };
    let authority = match authorize_composite_sources(app, core, snapshot) {
        Ok(authority) => authority,
        Err(_) => return ProjectCoverCapture::CaptureFailed,
    };
    let composite = match crate::render::composite_timeline_frame_authorized(
        &snapshot.timeline,
        &snapshot.media,
        &snapshot.project_dir,
        render,
        frame_index,
        bounds.0.max(bounds.1),
        cancel,
        &authority,
    ) {
        Ok(composite) => composite,
        Err(error) => {
            eprintln!("[project-cover] {error}");
            return ProjectCoverCapture::CaptureFailed;
        }
    };
    let frame = opentake_media::RgbaFrame::new(composite.width, composite.height, composite.rgba);
    opentake_media::encode_project_composite_thumbnail(&frame, bounds).map_or(
        ProjectCoverCapture::CaptureFailed,
        ProjectCoverCapture::Captured,
    )
}

fn authorize_composite_sources<R: tauri::Runtime>(
    app: &AppHandle<R>,
    core: &AppCore,
    snapshot: &opentake_core::ProjectRuntimeSnapshot,
) -> Result<crate::render::CompositeSourceAuthority, String> {
    let project_authority = core.project_asset_authority();
    let scope = app.asset_protocol_scope();
    let mut files = std::collections::HashMap::new();
    for entry in &snapshot.media.entries {
        let retained = match &entry.source {
            opentake_domain::MediaSource::External { absolute_path } => {
                let requested = std::path::Path::new(absolute_path);
                if !crate::safe_asset_protocol::scope_allows_lexical_path(&scope, requested) {
                    continue;
                }
                let Ok((file, final_path)) =
                    crate::safe_asset_protocol::open_retained_regular_file(requested)
                else {
                    continue;
                };
                if !crate::safe_asset_protocol::scope_allows_lexical_path(&scope, &final_path) {
                    continue;
                }
                file
            }
            opentake_domain::MediaSource::Project { relative_path } => {
                let Ok(file) = core.open_project_asset(std::path::Path::new(relative_path)) else {
                    continue;
                };
                file
            }
        };
        files.insert(entry.id.clone(), retained);
    }
    if project_authority
        .as_ref()
        .is_some_and(|authority| !core.project_asset_authority_matches(authority))
        || core.project_revision().project_epoch != snapshot.project_epoch
    {
        return Err("project source authority changed during cover capture".to_string());
    }
    Ok(crate::render::CompositeSourceAuthority::new(files))
}

#[cfg(test)]
fn project_save_for_project(
    core: &AppCore,
    path: Option<String>,
    expected_project_epoch: u64,
    expected_project_path: Option<String>,
    capture_thumbnail: impl FnOnce(&opentake_core::ProjectRuntimeSnapshot) -> ProjectCoverCapture,
) -> Result<String, CmdError> {
    project_save_for_project_with_checkpoint(
        core,
        path,
        expected_project_epoch,
        expected_project_path,
        capture_thumbnail,
        || true,
    )
}

fn project_save_for_project_with_checkpoint(
    core: &AppCore,
    path: Option<String>,
    expected_project_epoch: u64,
    expected_project_path: Option<String>,
    capture_thumbnail: impl FnOnce(&opentake_core::ProjectRuntimeSnapshot) -> ProjectCoverCapture,
    can_commit: impl FnOnce() -> bool,
) -> Result<String, CmdError> {
    let snapshot = core.runtime_snapshot();
    if snapshot.project_epoch != expected_project_epoch
        || snapshot.project_dir.as_deref()
            != expected_project_path.as_deref().map(std::path::Path::new)
    {
        return Err(CmdError::from(opentake_core::CoreError::StaleProject));
    }
    let thumbnail = match capture_thumbnail(&snapshot) {
        ProjectCoverCapture::Captured(bytes) => opentake_project::ThumbnailUpdate::Replace(bytes),
        ProjectCoverCapture::NoVisibleContent => opentake_project::ThumbnailUpdate::Remove,
        ProjectCoverCapture::CaptureFailed => opentake_project::ThumbnailUpdate::Preserve,
    };
    let target = path.map(std::path::PathBuf::from);
    core.save_project_with_thumbnail_update_for_project_if(
        expected_project_epoch,
        expected_project_path.as_deref().map(std::path::Path::new),
        target,
        thumbnail,
        can_commit,
    )
    .map(|p| p.to_string_lossy().into_owned())
    .map_err(CmdError::from)
}

/// `get_default_project_dir`: the default folder new projects save into
/// (`~/Documents/OpenTake`, created on first use). Mirrors upstream
/// `Project.storageDirectory` (`~/Documents/Palmier Pro`). The front end uses it
/// as the save dialog's `defaultPath` so the user picks a location + name like
/// upstream `createNewProject` (`NSSavePanel`).
#[tauri::command]
pub fn get_default_project_dir(
    app: AppHandle,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
) -> Result<String, String> {
    let dir = app
        .path()
        .document_dir()
        .map_err(|e| e.to_string())?
        .join("OpenTake");
    ensure_default_project_dir(&dir, &admission)?;
    Ok(dir.to_string_lossy().into_owned())
}

fn ensure_default_project_dir(
    dir: &std::path::Path,
    admission: &crate::updater::InstallAdmissionGate,
) -> Result<(), String> {
    if dir.is_dir() {
        return Ok(());
    }
    let _activity = crate::updater::begin_mutating_activity(admission)?;
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())
}

#[cfg(test)]
mod default_project_dir_tests {
    use super::ensure_default_project_dir;

    #[test]
    fn update_install_rejects_first_default_project_directory_write() {
        let temp = tempfile::tempdir().expect("default directory fixture");
        let directory = temp.path().join("OpenTake");
        let admission = crate::updater::InstallAdmissionGate::default();
        let install = admission.begin_install().expect("install starts");

        assert_eq!(
            ensure_default_project_dir(&directory, &admission)
                .expect_err("directory creation must fail closed"),
            "app update installation is in progress"
        );
        assert!(!directory.exists());

        drop(install);
        ensure_default_project_dir(&directory, &admission)
            .expect("directory creation resumes after install");
        assert!(directory.is_dir());

        let install = admission.begin_install().expect("second install starts");
        ensure_default_project_dir(&directory, &admission)
            .expect("an existing default directory is a read-only cache hit");
        drop(install);
    }
}

/// `export_xmeml`: write the current timeline to `path` as XMEML 4 (Final Cut
/// Pro 7 XML, `.xml`). This is the Premiere / DaVinci / 剪映-importable
/// interchange format — Premiere Pro does NOT read modern FCPXML natively, so
/// upstream (and OpenTake) emit XMEML; DaVinci/FCP still import FCP7 XML. Reads
/// the timeline / media manifest / project dir from the core, builds the XML via
/// the pure `export_xmeml`, and writes the file.
#[tauri::command]
pub fn export_xmeml(
    core: State<'_, AppCore>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    path: String,
) -> Result<(), String> {
    let _activity = crate::updater::begin_mutating_activity(&admission)?;
    let snapshot = core.runtime_snapshot();
    // Resolve each source file's start timecode via ffprobe (upstream reads the
    // QuickTime `tmcd` track; here `opentake_media::read_start_timecode_frame`
    // reads `tags.timecode`). Per-file failures are silently dropped -> 0.
    let start_timecodes = resolve_start_timecodes(
        &snapshot.timeline,
        &snapshot.media,
        snapshot.project_dir.as_deref(),
    );
    let xml = opentake_project::export_xmeml_with_timecodes(
        &snapshot.timeline,
        &snapshot.media,
        snapshot.project_dir.as_deref(),
        &start_timecodes,
    );
    std::fs::write(&path, xml).map_err(|e| e.to_string())
}

/// Build the `media_ref -> start-frame` map for [`export_xmeml`]. Iterates the
/// manifest, resolves each entry to an on-disk file, and reads its start timecode
/// via ffprobe at the **same integer timebase** the XMEML `<file>` node uses for
/// that source (`max(1, round(source_fps ?? timeline.fps))`, the upstream
/// `rateTags` timebase — so the parsed frame count matches the `<rate>` written
/// beside it). A missing manifest entry path, an unreadable file, or an absent
/// timecode tag simply yields no map entry, and the exporter falls back to 0
/// exactly as upstream's `sourceStartFrame(for:) ?? 0` does. Only entries with a
/// nonzero timecode are inserted (zero is already the exporter default).
fn resolve_start_timecodes(
    timeline: &opentake_domain::Timeline,
    manifest: &opentake_domain::MediaManifest,
    project_base: Option<&std::path::Path>,
) -> std::collections::HashMap<String, i32> {
    let resolver = opentake_domain::MediaResolver::new(manifest, project_base);
    let mut map = std::collections::HashMap::new();
    for entry in &manifest.entries {
        // Same per-file timebase the exporter computes (integer FCP7 timebase).
        let raw_fps = entry.source_fps.unwrap_or(timeline.fps as f64);
        let timebase = (raw_fps.round() as i32).max(1);
        let Some(path) = resolver.expected_path(&entry.id) else {
            continue;
        };
        if let Some(frame) = opentake_media::read_start_timecode_frame(&path, timebase) {
            if frame > 0 {
                map.insert(entry.id.clone(), frame);
            }
        }
    }
    map
}

/// `export_fcpxml`: deprecated alias for [`export_xmeml`], kept so any existing
/// front-end caller keeps working. The command name historically said "fcpxml"
/// but always produced XMEML 4 (FCP7 XML); the honest name is `export_xmeml`.
/// New code (and the format picker) should call `export_xmeml`; native FCPXML is
/// `export_fcpxml_modern`.
#[tauri::command]
pub fn export_fcpxml(
    core: State<'_, AppCore>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    path: String,
) -> Result<(), String> {
    export_xmeml(core, admission, path)
}

/// `export_edl`: write the current timeline to `path` as a CMX3600 EDL (`.edl`).
/// A flat, video-track-only edit decision list (the EDL format itself only
/// describes one V track + linked audio channels) that Premiere / DaVinci /
/// Avid / 剪映 import. Effects, transforms, opacity, and multi-track layering are
/// dropped — see `opentake_project::edl` for the documented limitations.
#[tauri::command]
pub fn export_edl(
    core: State<'_, AppCore>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    path: String,
) -> Result<(), String> {
    let _activity = crate::updater::begin_mutating_activity(&admission)?;
    let snapshot = core.runtime_snapshot();
    let edl = opentake_project::export_edl(&snapshot.timeline, &snapshot.media);
    std::fs::write(&path, edl).map_err(|e| e.to_string())
}

/// `export_otio`: write the current timeline to `path` as OpenTimelineIO JSON
/// (`.otio`) — the industry-standard interchange `otioview` / DaVinci / Blender
/// read. Preserves track order/kind, clip placement, source ranges, gaps, and
/// per-clip media references; see `opentake_project::otio` for what is dropped
/// (effects, transforms, keyframes).
#[tauri::command]
pub fn export_otio(
    core: State<'_, AppCore>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    path: String,
) -> Result<(), String> {
    let _activity = crate::updater::begin_mutating_activity(&admission)?;
    let snapshot = core.runtime_snapshot();
    let json = opentake_project::export_otio(
        &snapshot.timeline,
        &snapshot.media,
        snapshot.project_dir.as_deref(),
    );
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

/// `export_fcpxml_modern`: write the current timeline to `path` as native Final
/// Cut Pro X FCPXML 1.10 (`.fcpxml`). Unlike XMEML, this carries text overlays
/// (`<title>`), transforms, opacity, and volume. NOTE: Premiere does NOT import
/// FCPXML — use `export_xmeml` for Premiere / DaVinci / 剪映. See
/// `opentake_project::fcpxml_modern`.
#[tauri::command]
pub fn export_fcpxml_modern(
    core: State<'_, AppCore>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    path: String,
) -> Result<(), String> {
    let _activity = crate::updater::begin_mutating_activity(&admission)?;
    let snapshot = core.runtime_snapshot();
    let xml = opentake_project::export_fcpxml(
        &snapshot.timeline,
        &snapshot.media,
        snapshot.project_dir.as_deref(),
    );
    std::fs::write(&path, xml).map_err(|e| e.to_string())
}

/// Requested subtitle container, projected from the front end. Lower-cased serde
/// tags (`"srt"` / `"vtt"`) match the file extension the user picks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubtitleFormat {
    /// SubRip (`.srt`) — `HH:MM:SS,mmm` timestamps, numbered cues.
    #[default]
    Srt,
    /// WebVTT (`.vtt`) — `HH:MM:SS.mmm` timestamps, `WEBVTT` header.
    Vtt,
}

/// Summary of a completed subtitle export, returned to the front end. `cueCount`
/// lets the UI distinguish "wrote N cues" from "timeline has no captions" (in
/// which case it shows a friendly toast); the file is still written either way —
/// an empty SRT / header-only VTT is the documented contract of the pure layer.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleExportSummary {
    /// Absolute path the subtitle file was written to.
    pub out_path: String,
    /// Number of caption cues emitted.
    pub cue_count: usize,
}

/// `export_subtitles`: write the current timeline's caption clips to `path` as a
/// SubRip (`.srt`) or WebVTT (`.vtt`) document. Caption cues are collected from
/// every track via the pure `opentake_domain::subtitle_export` layer (any clip
/// carrying a `caption_group_id` + non-empty `text_content`), serialized, and
/// written to disk. Returns the cue count so the UI can report an empty result.
#[tauri::command]
pub fn export_subtitles(
    core: State<'_, AppCore>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    path: String,
    format: SubtitleFormat,
) -> Result<SubtitleExportSummary, String> {
    let _activity = crate::updater::begin_mutating_activity(&admission)?;
    let timeline = core.get_timeline().timeline;
    write_subtitles(&timeline, path, format)
}

/// The subtitle export body, decoupled from Tauri/`AppCore` so it can be driven
/// by a unit test with a hand-built timeline + temp path. The command wrapper
/// only snapshots the live session and delegates here.
fn write_subtitles(
    timeline: &opentake_domain::Timeline,
    path: String,
    format: SubtitleFormat,
) -> Result<SubtitleExportSummary, String> {
    let cue_count = opentake_domain::collect_caption_cues(timeline).len();
    let body = match format {
        SubtitleFormat::Srt => opentake_domain::export_srt(timeline),
        SubtitleFormat::Vtt => opentake_domain::export_vtt(timeline),
    };
    std::fs::write(&path, body).map_err(|e| e.to_string())?;
    Ok(SubtitleExportSummary {
        out_path: path,
        cue_count,
    })
}

/// `can_undo` / `can_redo`: enable/disable the toolbar affordances.
#[tauri::command]
pub fn can_undo(core: State<'_, AppCore>) -> bool {
    core.can_undo()
}

#[tauri::command]
pub fn can_redo(core: State<'_, AppCore>) -> bool {
    core.can_redo()
}

// MARK: - The single editing entry point

fn begin_edit_activity(
    admission: &crate::updater::InstallAdmissionGate,
) -> Result<crate::updater::ActivityLease, CmdError> {
    crate::updater::begin_mutating_activity(admission).map_err(validation_error)
}

/// `edit_apply`: the unified editing command. The front end constructs an
/// [`EditRequest`] from a UI gesture; this maps it to an [`EditCommand`] and
/// routes it through [`AppCore::apply_at_project_revision`] (which performs the
/// project identity check and snapshot/commit/version transaction under one
/// authoritative lock, then emits `TimelineChanged`).
#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri IPC injects states and request identity separately
pub fn edit_apply(
    core: State<'_, AppCore>,
    render: State<'_, crate::render::RenderState>,
    media: State<'_, crate::media::MediaState>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    command: EditRequest,
    expected_project_epoch: u64,
    expected_timeline_version: u64,
    expected_project_path: Option<String>,
) -> Result<EditResultDto, CmdError> {
    let _admission = begin_edit_activity(&admission)?;
    let mut prepared_freeze_path = None;
    let cmd = match command {
        EditRequest::FreezeFrame {
            clip_id,
            at_frame,
            duration_frames,
        } => {
            // Save As/project replacement takes the write side. Retain the read
            // lease while freeze-frame preparation reads external/render state,
            // then release it before the core emits post-commit events. If a
            // transition wins after capture, the atomic revision/path check
            // below rejects the prepared command.
            let prepared = {
                let _identity = core.lock_project_identity_workflow();
                validate_freeze_frame_request(&core, &clip_id, at_frame, duration_frames)
                    .map_err(validation_error)?;
                crate::render::capture_freeze_frame(&core, &render, &media, &clip_id, at_frame)
                    .map_err(|error| {
                        eprintln!("freeze-frame capture failed: {error}");
                        internal_error("Freeze-frame capture failed")
                    })?
            };
            // The identity lease is deliberately gone before the command can
            // emit TimelineChanged/MediaChanged. The final revision/path check
            // below is still authoritative and shares the edit's core lock.
            prepared_freeze_path = Some(prepared.path);
            EditCommand::RegisterMediaAndFreezeFrame {
                media: prepared.media,
                clip_id,
                at_frame,
                duration_frames,
            }
        }
        other => other.into_command().map_err(validation_error)?,
    };
    let result = handle_edit_apply_at_project_revision(
        &core,
        ProjectRevision {
            project_epoch: expected_project_epoch,
            version: expected_timeline_version,
        },
        expected_project_path.as_deref().map(std::path::Path::new),
        cmd,
    );
    if result.is_err() {
        if let Some(path) = prepared_freeze_path {
            if let Err(error) = std::fs::remove_file(&path) {
                eprintln!(
                    "failed to remove rejected freeze-frame capture {}: {error}",
                    path.display()
                );
            }
        }
    }
    result
}

/// `check_path_exists`: checks if a path (e.g. project bundle folder) exists on disk.
#[tauri::command]
pub fn check_path_exists(path: String) -> bool {
    std::path::Path::new(&path).exists()
}

fn validation_error(message: String) -> CmdError {
    CmdError {
        code: "validation".to_string(),
        message,
    }
}

fn internal_error(message: impl Into<String>) -> CmdError {
    CmdError {
        code: "internal".to_string(),
        message: message.into(),
    }
}

fn validate_freeze_frame_request(
    core: &AppCore,
    clip_id: &str,
    at_frame: i32,
    duration_frames: i32,
) -> Result<(), String> {
    let timeline = core.get_timeline().timeline;
    let clip = timeline
        .tracks
        .iter()
        .flat_map(|track| track.clips.iter())
        .find(|clip| clip.id == clip_id)
        .ok_or_else(|| format!("Clip not found: {clip_id}"))?;
    if !(at_frame > clip.start_frame && at_frame < clip.end_frame()) {
        return Err(format!(
            "Frame {at_frame} must be strictly inside clip range ({}..{})",
            clip.start_frame,
            clip.end_frame()
        ));
    }
    if duration_frames < 1 {
        return Err(format!(
            "durationFrames must be >= 1 (got {duration_frames})"
        ));
    }
    if !matches!(clip.media_type, ClipType::Video | ClipType::Image) {
        return Err(format!(
            "Freeze Frame requires a video or image clip (got {:?})",
            clip.media_type
        ));
    }
    Ok(())
}

// MARK: - EditRequest (serde-friendly mirror of EditCommand)

/// A serde-deserializable mirror of the [`EditCommand`] variants the front end
/// issues. Tagged `{ "type": "addClips", ... }` to match the TS discriminated
/// union. Engine value types (`ClipMove`, `TrimEdit`, `FrameRange`, keyframe
/// tracks) are mirrored as local serde DTOs and converted in [`into_command`].
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum EditRequest {
    #[serde(rename_all = "camelCase")]
    CreateNestedSequence { name: String, clip_ids: Vec<String> },
    #[serde(rename_all = "camelCase")]
    EditNestedSequence {
        sequence_id: String,
        command: Box<EditRequest>,
    },
    #[serde(rename_all = "camelCase")]
    RenameNestedSequence { sequence_id: String, name: String },
    #[serde(rename_all = "camelCase")]
    DissolveNestedSequence { clip_id: String },
    #[serde(rename_all = "camelCase")]
    PlaceMedia {
        sequence_id: Option<String>,
        settings: Option<ProjectTimelineSettingsDto>,
        target: PlaceMediaTargetDto,
        entry: UnplacedClipEntryDto,
    },
    #[serde(rename_all = "camelCase")]
    AddClips { entries: Vec<ClipEntryDto> },
    #[serde(rename_all = "camelCase")]
    InsertClips {
        track_index: usize,
        at_frame: i32,
        entries: Vec<ClipEntryDto>,
    },
    #[serde(rename_all = "camelCase")]
    MoveClips { moves: Vec<ClipMoveDto> },
    #[serde(rename_all = "camelCase")]
    DuplicateClips {
        clip_ids: Vec<String>,
        offset_frames: i32,
        target_track_indexes: Vec<usize>,
    },
    #[serde(rename_all = "camelCase")]
    MoveOrDuplicateClipsToNewTrack {
        clip_ids: Vec<String>,
        lead_clip_id: String,
        requested_frame_delta: i32,
        insert_at: usize,
        mode: NewTrackClipModeDto,
    },
    #[serde(rename_all = "camelCase")]
    PasteClips { entries: Vec<PasteClipEntryDto> },
    #[serde(rename_all = "camelCase")]
    RemoveClips { clip_ids: Vec<String> },
    #[serde(rename_all = "camelCase")]
    SplitClip { clip_id: String, at_frame: i32 },
    #[serde(rename_all = "camelCase")]
    SplitClips {
        clip_ids: Vec<String>,
        at_frame: i32,
    },
    #[serde(rename_all = "camelCase")]
    FreezeFrame {
        clip_id: String,
        at_frame: i32,
        duration_frames: i32,
    },
    #[serde(rename_all = "camelCase")]
    TrimClips { edits: Vec<TrimEditDto> },
    #[serde(rename_all = "camelCase")]
    SetClipProperties {
        clip_ids: Vec<String>,
        // Boxed to keep `EditRequest` small: `ClipPropertiesDto` carries a full
        // `TextStyle`, which would otherwise dominate the enum size.
        properties: Box<ClipPropertiesDto>,
    },
    #[serde(rename_all = "camelCase")]
    SetTransformAtFrame {
        clip_id: String,
        frame: i32,
        transform: Transform,
    },
    #[serde(rename_all = "camelCase")]
    SetKeyframes {
        clip_id: String,
        property: KeyframePropertyDto,
        payload: KeyframePayloadDto,
    },
    #[serde(rename_all = "camelCase")]
    StampKeyframe {
        clip_id: String,
        property: KeyframePropertyDto,
        frame: i32,
    },
    #[serde(rename_all = "camelCase")]
    UpsertKeyframe {
        clip_id: String,
        property: KeyframePropertyDto,
        frame: i32,
        value: KeyframeValueDto,
    },
    #[serde(rename_all = "camelCase")]
    RemoveKeyframe {
        clip_id: String,
        property: KeyframePropertyDto,
        frame: i32,
    },
    #[serde(rename_all = "camelCase")]
    MoveKeyframe {
        clip_id: String,
        property: KeyframePropertyDto,
        from_frame: i32,
        to_frame: i32,
    },
    #[serde(rename_all = "camelCase")]
    SetKeyframeInterpolation {
        clip_id: String,
        property: KeyframePropertyDto,
        frame: i32,
        interpolation: Interpolation,
    },
    #[serde(rename_all = "camelCase")]
    SetColorGrade {
        clip_ids: Vec<String>,
        grade: Option<ColorGrade>,
    },
    #[serde(rename_all = "camelCase")]
    SetLut {
        clip_ids: Vec<String>,
        lut: Option<LutReference>,
    },
    #[serde(rename_all = "camelCase")]
    SetChromaKey {
        clip_ids: Vec<String>,
        chroma_key: Option<ChromaKey>,
    },
    #[serde(rename_all = "camelCase")]
    SetMasks {
        clip_ids: Vec<String>,
        masks: Vec<Mask>,
    },
    #[serde(rename_all = "camelCase")]
    SetEffects {
        clip_ids: Vec<String>,
        effects: Vec<Effect>,
    },
    #[serde(rename_all = "camelCase")]
    SetLoudnessNormalization {
        clip_id: String,
        normalization: Option<LoudnessNormalization>,
    },
    #[serde(rename_all = "camelCase")]
    SetAudioDenoise {
        clip_id: String,
        denoise: Option<AudioDenoise>,
    },
    #[serde(rename_all = "camelCase")]
    ApplyStabilization {
        clip_id: String,
        solution: StabilizationTrack,
    },
    #[serde(rename_all = "camelCase")]
    AdjustStabilization {
        clip_id: String,
        strength: Option<f64>,
        crop_margin: Option<f64>,
    },
    #[serde(rename_all = "camelCase")]
    ResetStabilization { clip_id: String },
    #[serde(rename_all = "camelCase")]
    SetTransition {
        from_clip_id: String,
        to_clip_id: String,
        kind: Option<TransitionKind>,
        duration_frames: i32,
    },
    #[serde(rename_all = "camelCase")]
    RippleDeleteRanges {
        track_index: usize,
        ranges: Vec<FrameRangeDto>,
    },
    #[serde(rename_all = "camelCase")]
    RippleDeleteClips { clip_ids: Vec<String> },
    #[serde(rename_all = "camelCase")]
    AddTexts { entries: Vec<TextEntryDto> },
    #[serde(rename_all = "camelCase")]
    AddTextsAutoTrack { entries: Vec<TextAutoTrackEntryDto> },
    #[serde(rename_all = "camelCase")]
    AddCaptions { entries: Vec<CaptionEntryDto> },
    #[serde(rename_all = "camelCase")]
    Link { clip_ids: Vec<String> },
    #[serde(rename_all = "camelCase")]
    Unlink { clip_ids: Vec<String> },
    #[serde(rename_all = "camelCase")]
    RemoveTracks { track_indexes: Vec<usize> },
    #[serde(rename_all = "camelCase")]
    SwapTracks { a: usize, b: usize },
    #[serde(rename_all = "camelCase")]
    SwapClips { clip_a: String, clip_b: String },
    #[serde(rename_all = "camelCase")]
    InsertTrack { kind: ClipType, at: Option<usize> },
    #[serde(rename_all = "camelCase")]
    SetTrackProps {
        track_index: usize,
        muted: Option<bool>,
        hidden: Option<bool>,
        sync_locked: Option<bool>,
    },
    #[serde(rename_all = "camelCase")]
    CreateFolder {
        name: String,
        parent_folder_id: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    MoveToFolder {
        asset_ids: Vec<String>,
        folder_id: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    RenameMedia { entries: Vec<RenameEntryDto> },
    #[serde(rename_all = "camelCase")]
    RenameFolder { entries: Vec<RenameEntryDto> },
    #[serde(rename_all = "camelCase")]
    DeleteMedia { asset_ids: Vec<String> },
    #[serde(rename_all = "camelCase")]
    DeleteFolder { folder_ids: Vec<String> },
    #[serde(rename_all = "camelCase")]
    SwapMedia { clip_id: String, media_ref: String },
    #[serde(rename_all = "camelCase")]
    ResetTransform { clip_ids: Vec<String> },
    #[serde(rename_all = "camelCase")]
    SetTimelineSettings { fps: i32, width: i32, height: i32 },
}

impl EditRequest {
    fn into_command(self) -> Result<EditCommand, String> {
        Ok(match self {
            EditRequest::CreateNestedSequence { name, clip_ids } => {
                EditCommand::CreateNestedSequenceFromClips { name, clip_ids }
            }
            EditRequest::EditNestedSequence {
                sequence_id,
                command,
            } => EditCommand::EditNestedSequence {
                sequence_id,
                command: Box::new(command.into_command()?),
            },
            EditRequest::RenameNestedSequence { sequence_id, name } => {
                EditCommand::RenameNestedSequence { sequence_id, name }
            }
            EditRequest::DissolveNestedSequence { clip_id } => {
                EditCommand::DissolveNestedSequence { clip_id }
            }
            EditRequest::PlaceMedia {
                sequence_id,
                settings,
                target,
                entry,
            } => EditCommand::PlaceMedia {
                sequence_id,
                settings: settings.map(ProjectTimelineSettingsDto::into_settings),
                target: target.into_target(),
                entry: entry.into_entry(),
            },
            EditRequest::AddClips { entries } => EditCommand::AddClips {
                entries: entries.into_iter().map(ClipEntryDto::into_entry).collect(),
            },
            EditRequest::InsertClips {
                track_index,
                at_frame,
                entries,
            } => EditCommand::InsertClips {
                track_index,
                at_frame,
                entries: entries.into_iter().map(ClipEntryDto::into_entry).collect(),
            },
            EditRequest::MoveClips { moves } => EditCommand::MoveClips {
                moves: moves.into_iter().map(ClipMoveDto::into_move).collect(),
            },
            EditRequest::DuplicateClips {
                clip_ids,
                offset_frames,
                target_track_indexes,
            } => EditCommand::DuplicateClips {
                clip_ids,
                offset_frames,
                target_track_indexes,
            },
            EditRequest::MoveOrDuplicateClipsToNewTrack {
                clip_ids,
                lead_clip_id,
                requested_frame_delta,
                insert_at,
                mode,
            } => EditCommand::MoveOrDuplicateClipsToNewTrack {
                clip_ids,
                lead_clip_id,
                requested_frame_delta,
                insert_at,
                mode: mode.into(),
            },
            EditRequest::PasteClips { entries } => EditCommand::PasteClips {
                entries: entries
                    .into_iter()
                    .map(PasteClipEntryDto::into_entry)
                    .collect(),
            },
            EditRequest::RemoveClips { clip_ids } => EditCommand::RemoveClips { clip_ids },
            EditRequest::SplitClip { clip_id, at_frame } => {
                EditCommand::SplitClip { clip_id, at_frame }
            }
            EditRequest::SplitClips { clip_ids, at_frame } => {
                EditCommand::SplitClips { clip_ids, at_frame }
            }
            EditRequest::FreezeFrame { .. } => {
                return Err("freezeFrame must be handled by edit_apply".into())
            }
            EditRequest::TrimClips { edits } => EditCommand::TrimClips {
                edits: edits.into_iter().map(TrimEditDto::into_edit).collect(),
            },
            EditRequest::SetClipProperties {
                clip_ids,
                properties,
            } => EditCommand::SetClipProperties {
                clip_ids,
                properties: Box::new((*properties).into_properties()),
            },
            EditRequest::SetTransformAtFrame {
                clip_id,
                frame,
                transform,
            } => EditCommand::SetTransformAtFrame {
                clip_id,
                frame,
                transform,
            },
            EditRequest::SetKeyframes {
                clip_id,
                property,
                payload,
            } => EditCommand::SetKeyframes {
                clip_id,
                property: property.into(),
                payload: payload.into_payload()?,
            },
            EditRequest::StampKeyframe {
                clip_id,
                property,
                frame,
            } => EditCommand::StampKeyframe {
                clip_id,
                property: property.into(),
                frame,
            },
            EditRequest::UpsertKeyframe {
                clip_id,
                property,
                frame,
                value,
            } => EditCommand::UpsertKeyframe {
                clip_id,
                property: property.into(),
                frame,
                value: value.into_value(),
            },
            EditRequest::RemoveKeyframe {
                clip_id,
                property,
                frame,
            } => EditCommand::RemoveKeyframe {
                clip_id,
                property: property.into(),
                frame,
            },
            EditRequest::MoveKeyframe {
                clip_id,
                property,
                from_frame,
                to_frame,
            } => EditCommand::MoveKeyframe {
                clip_id,
                property: property.into(),
                from_frame,
                to_frame,
            },
            EditRequest::SetKeyframeInterpolation {
                clip_id,
                property,
                frame,
                interpolation,
            } => EditCommand::SetKeyframeInterpolation {
                clip_id,
                property: property.into(),
                frame,
                interpolation,
            },
            EditRequest::SetColorGrade { clip_ids, grade } => {
                EditCommand::SetColorGrade { clip_ids, grade }
            }
            EditRequest::SetLut { clip_ids, lut } => EditCommand::SetLut { clip_ids, lut },
            EditRequest::SetChromaKey {
                clip_ids,
                chroma_key,
            } => EditCommand::SetChromaKey {
                clip_ids,
                chroma_key,
            },
            EditRequest::SetMasks { clip_ids, masks } => EditCommand::SetMasks { clip_ids, masks },
            EditRequest::SetEffects { clip_ids, effects } => {
                EditCommand::SetEffects { clip_ids, effects }
            }
            EditRequest::SetLoudnessNormalization {
                clip_id,
                normalization,
            } => EditCommand::SetLoudnessNormalization {
                clip_id,
                normalization,
            },
            EditRequest::SetAudioDenoise { clip_id, denoise } => {
                EditCommand::SetAudioDenoise { clip_id, denoise }
            }
            EditRequest::ApplyStabilization { clip_id, solution } => {
                EditCommand::ApplyStabilization { clip_id, solution }
            }
            EditRequest::AdjustStabilization {
                clip_id,
                strength,
                crop_margin,
            } => EditCommand::AdjustStabilization {
                clip_id,
                strength,
                crop_margin,
            },
            EditRequest::ResetStabilization { clip_id } => {
                EditCommand::ResetStabilization { clip_id }
            }
            EditRequest::SetTransition {
                from_clip_id,
                to_clip_id,
                kind,
                duration_frames,
            } => EditCommand::SetTransition {
                from_clip_id,
                to_clip_id,
                kind,
                duration_frames,
            },
            EditRequest::RippleDeleteRanges {
                track_index,
                ranges,
            } => EditCommand::RippleDeleteRanges {
                track_index,
                ranges: ranges.into_iter().map(FrameRangeDto::into_range).collect(),
            },
            EditRequest::RippleDeleteClips { clip_ids } => {
                EditCommand::RippleDeleteClips { clip_ids }
            }
            EditRequest::AddTexts { entries } => EditCommand::AddTexts {
                entries: entries.into_iter().map(TextEntryDto::into_entry).collect(),
            },
            EditRequest::AddTextsAutoTrack { entries } => EditCommand::AddTextsAutoTrack {
                entries: entries
                    .into_iter()
                    .map(TextAutoTrackEntryDto::into_entry)
                    .collect(),
            },
            EditRequest::AddCaptions { entries } => EditCommand::AddCaptions {
                entries: entries
                    .into_iter()
                    .map(CaptionEntryDto::into_entry)
                    .collect(),
            },
            EditRequest::Link { clip_ids } => EditCommand::Link { clip_ids },
            EditRequest::Unlink { clip_ids } => EditCommand::Unlink { clip_ids },
            EditRequest::RemoveTracks { track_indexes } => {
                EditCommand::RemoveTracks { track_indexes }
            }
            EditRequest::SwapTracks { a, b } => EditCommand::SwapTracks { a, b },
            EditRequest::SwapClips { clip_a, clip_b } => EditCommand::SwapClips {
                a: clip_a,
                b: clip_b,
            },
            EditRequest::InsertTrack { kind, at } => EditCommand::InsertTrack { kind, at },
            EditRequest::SetTrackProps {
                track_index,
                muted,
                hidden,
                sync_locked,
            } => EditCommand::SetTrackProps {
                track_index,
                muted,
                hidden,
                sync_locked,
            },
            EditRequest::CreateFolder {
                name,
                parent_folder_id,
            } => EditCommand::CreateFolder {
                name,
                parent_folder_id,
            },
            EditRequest::MoveToFolder {
                asset_ids,
                folder_id,
            } => EditCommand::MoveToFolder {
                asset_ids,
                folder_id,
            },
            EditRequest::RenameMedia { entries } => EditCommand::RenameMedia {
                entries: entries
                    .into_iter()
                    .map(RenameEntryDto::into_entry)
                    .collect(),
            },
            EditRequest::RenameFolder { entries } => EditCommand::RenameFolder {
                entries: entries
                    .into_iter()
                    .map(RenameEntryDto::into_entry)
                    .collect(),
            },
            EditRequest::DeleteMedia { asset_ids } => EditCommand::DeleteMedia { asset_ids },
            EditRequest::DeleteFolder { folder_ids } => EditCommand::DeleteFolder { folder_ids },
            EditRequest::SwapMedia { clip_id, media_ref } => {
                EditCommand::SwapMedia { clip_id, media_ref }
            }
            EditRequest::ResetTransform { clip_ids } => EditCommand::ResetTransform { clip_ids },
            EditRequest::SetTimelineSettings { fps, width, height } => {
                EditCommand::SetTimelineSettings { fps, width, height }
            }
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTimelineSettingsDto {
    pub fps: i32,
    pub width: i32,
    pub height: i32,
}

impl ProjectTimelineSettingsDto {
    fn into_settings(self) -> ProjectTimelineSettings {
        ProjectTimelineSettings {
            fps: self.fps,
            width: self.width,
            height: self.height,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlaceMediaTargetDto {
    #[serde(rename_all = "camelCase")]
    ExistingTrack { track_id: String },
    #[serde(rename_all = "camelCase")]
    NewTrack {
        track_type: ClipType,
        at: Option<usize>,
    },
}

impl PlaceMediaTargetDto {
    fn into_target(self) -> PlaceMediaTarget {
        match self {
            Self::ExistingTrack { track_id } => PlaceMediaTarget::ExistingTrack { track_id },
            Self::NewTrack { track_type, at } => PlaceMediaTarget::NewTrack {
                kind: track_type,
                at,
            },
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnplacedClipEntryDto {
    pub media_ref: String,
    pub media_type: ClipType,
    pub source_clip_type: ClipType,
    pub start_frame: i32,
    pub duration_frames: i32,
    #[serde(default)]
    pub trim_start_frame: Option<i32>,
    #[serde(default)]
    pub trim_end_frame: Option<i32>,
    #[serde(default)]
    pub has_audio: bool,
    #[serde(default)]
    pub add_linked_audio: bool,
    #[serde(default)]
    pub transform: Option<Transform>,
}

impl UnplacedClipEntryDto {
    fn into_entry(self) -> UnplacedClipEntry {
        UnplacedClipEntry {
            media_ref: self.media_ref,
            media_type: self.media_type,
            source_clip_type: self.source_clip_type,
            start_frame: self.start_frame,
            duration_frames: self.duration_frames,
            trim_start_frame: self.trim_start_frame,
            trim_end_frame: self.trim_end_frame,
            has_audio: self.has_audio,
            add_linked_audio: self.add_linked_audio,
            transform: self.transform,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NewTrackClipModeDto {
    Move,
    Duplicate,
}

impl From<NewTrackClipModeDto> for NewTrackClipMode {
    fn from(value: NewTrackClipModeDto) -> Self {
        match value {
            NewTrackClipModeDto::Move => NewTrackClipMode::Move,
            NewTrackClipModeDto::Duplicate => NewTrackClipMode::Duplicate,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteClipEntryDto {
    pub clip: Clip,
    pub target_track_id: String,
    pub start_frame: i32,
}

impl PasteClipEntryDto {
    fn into_entry(self) -> PasteClipEntry {
        PasteClipEntry {
            clip: self.clip,
            target_track_id: self.target_track_id,
            start_frame: self.start_frame,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipEntryDto {
    pub media_ref: String,
    pub media_type: ClipType,
    pub source_clip_type: ClipType,
    pub track_index: usize,
    pub start_frame: i32,
    pub duration_frames: i32,
    #[serde(default)]
    pub trim_start_frame: Option<i32>,
    #[serde(default)]
    pub trim_end_frame: Option<i32>,
    #[serde(default)]
    pub has_audio: bool,
    #[serde(default)]
    pub add_linked_audio: bool,
    #[serde(default)]
    pub transform: Option<Transform>,
}

impl ClipEntryDto {
    fn into_entry(self) -> ClipEntry {
        ClipEntry {
            media_ref: self.media_ref,
            media_type: self.media_type,
            source_clip_type: self.source_clip_type,
            track_index: self.track_index,
            start_frame: self.start_frame,
            duration_frames: self.duration_frames,
            trim_start_frame: self.trim_start_frame,
            trim_end_frame: self.trim_end_frame,
            has_audio: self.has_audio,
            add_linked_audio: self.add_linked_audio,
            transform: self.transform,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipMoveDto {
    pub clip_id: String,
    pub to_track: usize,
    pub to_frame: i32,
}

impl ClipMoveDto {
    fn into_move(self) -> ClipMove {
        ClipMove {
            clip_id: self.clip_id,
            to_track: self.to_track,
            to_frame: self.to_frame,
        }
    }
}

/// `[clip_id, trim_start, trim_end]` in source frames (matches `TrimEdit`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrimEditDto {
    pub clip_id: String,
    pub trim_start_frame: i32,
    pub trim_end_frame: i32,
}

impl TrimEditDto {
    fn into_edit(self) -> (String, i32, i32) {
        (self.clip_id, self.trim_start_frame, self.trim_end_frame)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameRangeDto {
    pub start: i32,
    pub end: i32,
}

impl FrameRangeDto {
    fn into_range(self) -> FrameRange {
        FrameRange::new(self.start, self.end)
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipPropertiesDto {
    #[serde(default)]
    pub duration_frames: Option<i32>,
    #[serde(default)]
    pub trim_start_frame: Option<i32>,
    #[serde(default)]
    pub trim_end_frame: Option<i32>,
    #[serde(default)]
    pub speed: Option<f64>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub opacity: Option<f64>,
    #[serde(default)]
    pub transform: Option<Transform>,
    #[serde(default)]
    pub reversed: Option<bool>,
    #[serde(default)]
    pub text_content: Option<String>,
    #[serde(default)]
    pub text_style: Option<TextStyle>,
    #[serde(default)]
    pub crop: Option<Crop>,
    #[serde(default)]
    pub fade_in_frames: Option<i32>,
    #[serde(default)]
    pub fade_out_frames: Option<i32>,
    #[serde(default)]
    pub fade_in_interpolation: Option<Interpolation>,
    #[serde(default)]
    pub fade_out_interpolation: Option<Interpolation>,
    #[serde(default)]
    pub flip_horizontal: Option<bool>,
    #[serde(default)]
    pub flip_vertical: Option<bool>,
}

impl ClipPropertiesDto {
    fn into_properties(self) -> ClipProperties {
        ClipProperties {
            duration_frames: self.duration_frames,
            trim_start_frame: self.trim_start_frame,
            trim_end_frame: self.trim_end_frame,
            speed: self.speed,
            volume: self.volume,
            opacity: self.opacity,
            transform: self.transform,
            reversed: self.reversed,
            text_content: self.text_content,
            text_style: self.text_style,
            crop: self.crop,
            fade_in_frames: self.fade_in_frames,
            fade_out_frames: self.fade_out_frames,
            fade_in_interpolation: self.fade_in_interpolation,
            fade_out_interpolation: self.fade_out_interpolation,
            flip_horizontal: self.flip_horizontal,
            flip_vertical: self.flip_vertical,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEntryDto {
    pub track_index: usize,
    pub start_frame: i32,
    pub duration_frames: i32,
    pub content: String,
    pub text_style: TextStyle,
    pub transform: Transform,
}

impl TextEntryDto {
    fn into_entry(self) -> TextEntry {
        TextEntry {
            track_index: self.track_index,
            start_frame: self.start_frame,
            duration_frames: self.duration_frames,
            content: self.content,
            text_style: self.text_style,
            transform: self.transform,
        }
    }
}

/// Like [`TextEntryDto`] minus `trackIndex` — every entry in an
/// `addTextsAutoTrack` batch lands on the single fresh track the command
/// creates, so there's nothing to target (#194).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextAutoTrackEntryDto {
    pub start_frame: i32,
    pub duration_frames: i32,
    pub content: String,
    pub text_style: TextStyle,
    pub transform: Transform,
}

impl TextAutoTrackEntryDto {
    fn into_entry(self) -> TextAutoTrackEntry {
        TextAutoTrackEntry {
            start_frame: self.start_frame,
            duration_frames: self.duration_frames,
            content: self.content,
            text_style: self.text_style,
            transform: self.transform,
        }
    }
}

/// One built caption clip on the wire (mirrors [`CaptionEntry`]). Multi-word
/// fields MUST be camelCase (`startFrame`, `durationFrames`, `textStyle`,
/// `captionGroupId`) — the repo's #1 bug class is a DTO field that silently fails
/// to deserialize because it wasn't camelCase. See `commands.rs` module header.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionEntryDto {
    pub start_frame: i32,
    pub duration_frames: i32,
    pub content: String,
    pub text_style: TextStyle,
    pub transform: Transform,
    pub caption_group_id: String,
}

impl CaptionEntryDto {
    fn into_entry(self) -> CaptionEntry {
        CaptionEntry {
            start_frame: self.start_frame,
            duration_frames: self.duration_frames,
            content: self.content,
            text_style: self.text_style,
            transform: self.transform,
            caption_group_id: self.caption_group_id,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameEntryDto {
    pub id: String,
    pub name: String,
}

impl RenameEntryDto {
    fn into_entry(self) -> RenameEntry {
        RenameEntry {
            id: self.id,
            name: self.name,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyframePropertyDto {
    Opacity,
    Volume,
    Rotation,
    Position,
    Scale,
    Crop,
}

impl From<KeyframePropertyDto> for KeyframeProperty {
    fn from(p: KeyframePropertyDto) -> Self {
        match p {
            KeyframePropertyDto::Opacity => KeyframeProperty::Opacity,
            KeyframePropertyDto::Volume => KeyframeProperty::Volume,
            KeyframePropertyDto::Rotation => KeyframeProperty::Rotation,
            KeyframePropertyDto::Position => KeyframeProperty::Position,
            KeyframePropertyDto::Scale => KeyframeProperty::Scale,
            KeyframePropertyDto::Crop => KeyframeProperty::Crop,
        }
    }
}

/// One keyframe `{ frame, value, interpolationOut }` carrying a JSON value;
/// shaped per the target track in [`KeyframePayloadDto`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScalarKfDto {
    pub frame: i32,
    pub value: f64,
    #[serde(default)]
    pub interpolation_out: Option<Interpolation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairKfDto {
    pub frame: i32,
    pub value: AnimPair,
    #[serde(default)]
    pub interpolation_out: Option<Interpolation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropKfDto {
    pub frame: i32,
    pub value: Crop,
    #[serde(default)]
    pub interpolation_out: Option<Interpolation>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum KeyframePayloadDto {
    Scalar { keyframes: Vec<ScalarKfDto> },
    Pair { keyframes: Vec<PairKfDto> },
    Crop { keyframes: Vec<CropKfDto> },
}

impl KeyframePayloadDto {
    fn into_payload(self) -> Result<KeyframePayload, String> {
        Ok(match self {
            KeyframePayloadDto::Scalar { keyframes } => {
                let kfs = keyframes
                    .into_iter()
                    .map(|k| match k.interpolation_out {
                        Some(i) => Keyframe::with_interpolation(k.frame, k.value, i),
                        None => Keyframe::new(k.frame, k.value),
                    })
                    .collect();
                KeyframePayload::Scalar(KeyframeTrack::from_keyframes(kfs))
            }
            KeyframePayloadDto::Pair { keyframes } => {
                let kfs = keyframes
                    .into_iter()
                    .map(|k| match k.interpolation_out {
                        Some(i) => Keyframe::with_interpolation(k.frame, k.value, i),
                        None => Keyframe::new(k.frame, k.value),
                    })
                    .collect();
                KeyframePayload::Pair(KeyframeTrack::from_keyframes(kfs))
            }
            KeyframePayloadDto::Crop { keyframes } => {
                let kfs = keyframes
                    .into_iter()
                    .map(|k| match k.interpolation_out {
                        Some(i) => Keyframe::with_interpolation(k.frame, k.value, i),
                        None => Keyframe::new(k.frame, k.value),
                    })
                    .collect();
                KeyframePayload::Crop(KeyframeTrack::from_keyframes(kfs))
            }
        })
    }
}

/// An explicit single-value payload for [`EditRequest::UpsertKeyframe`]. Mirrors
/// [`KeyframePayloadDto`]'s `kind`-tagging, but carries one value (not a whole
/// replacement track) to upsert at the command's `frame`.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum KeyframeValueDto {
    Scalar { value: f64 },
    Pair { value: AnimPair },
    Crop { value: Crop },
}

impl KeyframeValueDto {
    fn into_value(self) -> KeyframeValue {
        match self {
            KeyframeValueDto::Scalar { value } => KeyframeValue::Scalar(value),
            KeyframeValueDto::Pair { value } => KeyframeValue::Pair(value),
            KeyframeValueDto::Crop { value } => KeyframeValue::Crop(value),
        }
    }
}

#[cfg(test)]
mod project_open_async_tests {
    use super::{
        capture_composite_project_thumbnail, prepare_saved_project_off_thread,
        project_save_for_project, project_save_for_project_with_checkpoint,
        run_blocking_with_timeout, save_current_project_with_composite_cover, ProjectCoverCapture,
        ProjectLifecycleCoordinator,
    };
    use opentake_core::core::PreparedProjectOpen;
    use opentake_core::AppCore;
    use std::time::Duration;
    use tauri::Manager as _;

    fn jpeg_bytes(color: [u8; 3]) -> Vec<u8> {
        let pixels = color.repeat(16 * 9);
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 80)
            .encode(&pixels, 16, 9, image::ExtendedColorType::Rgb8)
            .expect("encode test JPEG");
        bytes
    }
    #[cfg(unix)]
    #[test]
    fn prepared_project_detects_an_ambient_namespace_rebind_before_commit() {
        let fixture = tempfile::tempdir().expect("fixture tempdir");
        let selected = fixture.path().join("Selected.opentake");
        let retained = fixture.path().join("Retained-A.opentake");
        AppCore::new()
            .save_project(Some(selected.clone()))
            .expect("save project A");
        let prepared = AppCore::prepare_project_open(selected.clone()).expect("prepare A");

        std::fs::rename(&selected, &retained).expect("move A out of selected namespace");
        AppCore::new()
            .save_project(Some(selected))
            .expect("replace selected path with B");

        assert!(!prepared
            .is_current_namespace()
            .expect("namespace identity check"));
    }

    /// cap-std retains the bundle without FILE_SHARE_DELETE, so on Windows the
    /// ambient rename fails closed while prepared — the stronger namespace
    /// guard (rebind detection on the open handle is Unix-verified above).
    #[cfg(target_os = "windows")]
    #[test]
    fn prepared_project_blocks_an_ambient_namespace_rebind_while_retained() {
        let fixture = tempfile::tempdir().expect("fixture tempdir");
        let selected = fixture.path().join("Selected.opentake");
        let retained = fixture.path().join("Retained-A.opentake");
        AppCore::new()
            .save_project(Some(selected.clone()))
            .expect("save project A");
        let prepared = AppCore::prepare_project_open(selected.clone()).expect("prepare A");

        assert!(std::fs::rename(&selected, &retained).is_err());
        assert!(prepared
            .is_current_namespace()
            .expect("namespace identity check"));

        drop(prepared);
        std::fs::rename(&selected, &retained).expect("rename succeeds after release");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn blocking_project_prepare_runs_off_the_async_caller_thread() {
        let caller = std::thread::current().id();
        let coordinator = ProjectLifecycleCoordinator::default();
        let lifecycle = coordinator.try_acquire().expect("test transition starts");
        let admission = coordinator
            .try_admit_prepare(std::path::Path::new("off-thread.opentake"))
            .expect("prepare admitted");

        let worker =
            run_blocking_with_timeout("test prepare", Duration::from_secs(1), admission, || {
                Ok(std::thread::current().id())
            })
            .await
            .expect("blocking task completes");

        assert_ne!(worker, caller);
        assert!(
            coordinator.try_acquire().is_err(),
            "caller lease must span successful prepare and commit"
        );
        drop(lifecycle);
        coordinator
            .try_acquire()
            .expect("transition releases after the caller commits");
    }

    #[test]
    fn project_lifecycle_transitions_are_single_flight() {
        let coordinator = ProjectLifecycleCoordinator::default();
        let incumbent = coordinator.try_acquire().expect("first transition starts");

        assert_eq!(
            coordinator
                .try_acquire()
                .expect_err("overlapping transition must be busy"),
            "another project lifecycle transition is already in progress"
        );

        drop(incumbent);
        coordinator
            .try_acquire()
            .expect("transition may retry after incumbent settles");
    }

    #[test]
    fn project_prepare_admission_is_globally_bounded() {
        let coordinator = ProjectLifecycleCoordinator::default();
        let admissions = (0..super::MAX_CONCURRENT_PROJECT_PREPARES)
            .map(|index| {
                coordinator
                    .try_admit_prepare(std::path::Path::new(&format!("blocked-{index}.opentake")))
                    .expect("bounded prepare admitted")
            })
            .collect::<Vec<_>>();

        assert_eq!(
            coordinator
                .try_admit_prepare(std::path::Path::new("overflow.opentake"))
                .expect_err("fifth abandoned prepare must fail fast"),
            "too many timed-out project prepares are still finishing"
        );
        drop(admissions);
        coordinator
            .try_admit_prepare(std::path::Path::new("recovered.opentake"))
            .expect("admission recovers when work actually finishes");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn timed_out_worker_quarantines_only_its_path_and_releases_lifecycle() {
        let coordinator = ProjectLifecycleCoordinator::default();
        let lifecycle = coordinator.try_acquire().expect("first transition starts");
        let (release_worker, wait_for_release) = std::sync::mpsc::channel();
        let slow_path = std::path::Path::new("slow.opentake");
        let admission = coordinator
            .try_admit_prepare(slow_path)
            .expect("slow prepare admitted");

        let result: Result<(), String> = run_blocking_with_timeout(
            "test prepare",
            Duration::from_millis(10),
            admission,
            move || {
                wait_for_release
                    .recv()
                    .map_err(|error| format!("release channel closed: {error}"))
            },
        )
        .await;

        assert_eq!(
            result.expect_err("blocked worker must time out"),
            "test prepare timed out after 10ms"
        );
        drop(lifecycle);
        let retry_lifecycle = coordinator
            .try_acquire()
            .expect("a timed-out worker must not wedge every project transition");
        assert!(coordinator.try_admit_prepare(slow_path).is_err());
        coordinator
            .try_admit_prepare(std::path::Path::new("other.opentake"))
            .expect("an unrelated project path remains usable");
        drop(retry_lifecycle);

        release_worker.send(()).expect("release worker");
        let released_path = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Ok(admission) = coordinator.try_admit_prepare(slow_path) {
                    break admission;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("worker removes path quarantine after finishing");
        drop(released_path);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn timed_out_mutating_worker_keeps_update_admission_until_it_really_finishes() {
        let coordinator = ProjectLifecycleCoordinator::default();
        let admission = coordinator
            .try_admit_prepare(std::path::Path::new("slow-create.opentake"))
            .expect("prepare admitted");
        let update_admission = crate::updater::InstallAdmissionGate::default();
        let worker_activity = crate::updater::begin_mutating_activity(&update_admission).unwrap();
        let (release_worker, wait_for_release) = std::sync::mpsc::channel();

        let result: Result<(), String> = run_blocking_with_timeout(
            "project create",
            Duration::from_millis(10),
            admission,
            move || {
                let _worker_activity = worker_activity;
                wait_for_release
                    .recv()
                    .map_err(|error| format!("release channel closed: {error}"))
            },
        )
        .await;

        assert_eq!(
            result.expect_err("blocked writer must time out at the caller"),
            "project create timed out after 10ms"
        );
        assert!(
            update_admission.begin_install().is_err(),
            "spawn_blocking keeps writing after the caller timeout, so install must still wait"
        );

        release_worker.send(()).expect("release worker");
        let install = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Ok(install) = update_admission.begin_install() {
                    break install;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("worker eventually releases update admission");
        drop(install);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn timed_out_prepare_cannot_commit_a_late_project() {
        let fixture = tempfile::tempdir().expect("fixture tempdir");
        let bundle = fixture.path().join("slow.opentake");
        AppCore::new()
            .save_project(Some(bundle.clone()))
            .expect("save project fixture");
        let core = AppCore::new();
        let before = core.project_revision();
        let coordinator = ProjectLifecycleCoordinator::default();
        let _lifecycle = coordinator.try_acquire().expect("test transition starts");
        let admission = coordinator
            .try_admit_prepare(&bundle)
            .expect("prepare admitted");

        let result: Result<PreparedProjectOpen, String> = run_blocking_with_timeout(
            "project open",
            Duration::from_millis(10),
            admission,
            move || {
                std::thread::sleep(Duration::from_millis(75));
                AppCore::prepare_project_open(bundle).map_err(|error| error.to_string())
            },
        )
        .await;

        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("slow prepare must time out"),
        };
        assert_eq!(error, "project open timed out after 10ms");
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(core.project_revision(), before);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn saved_project_is_prepared_without_mutating_live_core_until_commit() {
        let fixture = tempfile::tempdir().expect("fixture tempdir");
        let bundle = fixture.path().join("Fresh.opentake");
        let core = AppCore::new();
        let before = core.project_revision();
        let coordinator = ProjectLifecycleCoordinator::default();
        let _lifecycle = coordinator.try_acquire().expect("test transition starts");
        let admission = coordinator
            .try_admit_prepare(&bundle)
            .expect("prepare admitted");
        let update_admission = crate::updater::InstallAdmissionGate::default();

        let prepared =
            prepare_saved_project_off_thread(bundle.clone(), admission, update_admission)
                .await
                .expect("new project bundle prepares");

        assert_eq!(core.project_revision(), before);
        assert!(bundle.join("project.json").is_file());
        let snapshot = core.commit_project_open(prepared);
        assert_eq!(snapshot.project_path.as_deref(), Some(bundle.as_path()));
        assert_eq!(snapshot.version, 0);
        assert_ne!(snapshot.project_epoch, before.project_epoch);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn failed_saved_project_prepare_preserves_live_core() {
        let fixture = tempfile::tempdir().expect("fixture tempdir");
        let regular_file = fixture.path().join("not-a-directory");
        std::fs::write(&regular_file, b"occupied").expect("write blocking fixture");
        let bundle = regular_file.join("Fresh.opentake");
        let core = AppCore::new();
        let before = core.project_revision();
        let coordinator = ProjectLifecycleCoordinator::default();
        let _lifecycle = coordinator.try_acquire().expect("test transition starts");
        let admission = coordinator
            .try_admit_prepare(&bundle)
            .expect("prepare admitted");
        let update_admission = crate::updater::InstallAdmissionGate::default();

        let error =
            match prepare_saved_project_off_thread(bundle, admission, update_admission).await {
                Err(error) => error,
                Ok(_) => panic!("invalid destination must fail"),
            };

        assert!(!error.is_empty());
        assert_eq!(core.project_revision(), before);
    }

    #[test]
    fn thumbnail_capture_cannot_save_a_replacement_project() {
        let fixture = tempfile::tempdir().expect("fixture tempdir");
        let first = fixture.path().join("First.opentake");
        let second = fixture.path().join("Second.opentake");
        let destination = fixture.path().join("Stale-Copy.opentake");
        AppCore::new()
            .save_project(Some(first.clone()))
            .expect("save first fixture");
        AppCore::new()
            .save_project(Some(second.clone()))
            .expect("save second fixture");

        let core = AppCore::new();
        core.open_project(&first).expect("open first fixture");
        let expected = core.runtime_snapshot();
        let save_core = core.clone();
        let expected_path = expected
            .project_dir
            .as_ref()
            .expect("opened project has path")
            .to_string_lossy()
            .into_owned();
        let destination_string = destination.to_string_lossy().into_owned();
        let (capture_started, wait_for_capture) = std::sync::mpsc::channel();
        let (release_capture, capture_released) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            project_save_for_project(
                &save_core,
                Some(destination_string),
                expected.project_epoch,
                Some(expected_path),
                |_| {
                    capture_started.send(()).expect("announce capture");
                    capture_released.recv().expect("release capture");
                    ProjectCoverCapture::Captured(b"thumbnail".to_vec())
                },
            )
        });

        wait_for_capture
            .recv_timeout(Duration::from_secs(1))
            .expect("capture starts");
        core.open_project(&second)
            .expect("replace project while thumbnail is captured");
        release_capture.send(()).expect("release capture");
        let error = worker
            .join()
            .expect("save worker joins")
            .expect_err("stale save must be rejected");

        assert_eq!(error.code, "staleProject", "{error:?}");
        assert!(!destination.exists());
        assert_eq!(
            core.runtime_snapshot().project_dir.as_deref(),
            Some(second.as_path())
        );
    }

    #[test]
    fn thumbnail_capture_failure_preserves_the_previous_valid_cover() {
        let fixture = tempfile::tempdir().expect("fixture tempdir");
        let bundle = fixture.path().join("KeepCover.opentake");
        let previous = jpeg_bytes([20, 40, 80]);
        let mut project = opentake_project::Project::new(&bundle);
        project.thumbnail = opentake_project::ThumbnailUpdate::Replace(previous.clone());
        project.save().expect("save prior cover fixture");
        let core = AppCore::new();
        core.open_project(&bundle).expect("open fixture");
        let snapshot = core.runtime_snapshot();

        project_save_for_project(
            &core,
            None,
            snapshot.project_epoch,
            Some(bundle.to_string_lossy().into_owned()),
            |_| ProjectCoverCapture::CaptureFailed,
        )
        .expect("project save remains best effort");

        assert_eq!(
            std::fs::read(bundle.join("thumbnail.jpg")).expect("read retained cover"),
            previous
        );
    }

    #[test]
    fn thumbnail_no_visible_content_removes_even_an_invalid_prior_cover() {
        let fixture = tempfile::tempdir().expect("fixture tempdir");
        let bundle = fixture.path().join("EmptyCover.opentake");
        let mut project = opentake_project::Project::new(&bundle);
        project.thumbnail = opentake_project::ThumbnailUpdate::Replace(b"not a jpeg".to_vec());
        project.save().expect("save invalid prior fixture");
        let core = AppCore::new();
        core.open_project(&bundle).expect("open fixture");
        let snapshot = core.runtime_snapshot();

        project_save_for_project(
            &core,
            None,
            snapshot.project_epoch,
            Some(bundle.to_string_lossy().into_owned()),
            |_| ProjectCoverCapture::NoVisibleContent,
        )
        .expect("empty project save succeeds");

        assert!(!bundle.join("thumbnail.jpg").exists());
    }

    #[test]
    fn cancelled_cover_worker_cannot_commit_a_late_capture() {
        let fixture = tempfile::tempdir().expect("cancel fixture");
        let bundle = fixture.path().join("Cancelled.opentake");
        let previous = jpeg_bytes([11, 22, 33]);
        let mut project = opentake_project::Project::new(&bundle);
        project.thumbnail = opentake_project::ThumbnailUpdate::Replace(previous.clone());
        project.save().expect("save prior cover");
        let core = AppCore::new();
        core.open_project(&bundle).expect("open fixture");
        let snapshot = core.runtime_snapshot();

        let error = project_save_for_project_with_checkpoint(
            &core,
            None,
            snapshot.project_epoch,
            Some(bundle.to_string_lossy().into_owned()),
            |_| ProjectCoverCapture::Captured(jpeg_bytes([200, 100, 50])),
            || false,
        )
        .expect_err("cancelled worker must not enter the commit");

        assert_eq!(error.code, "internal");
        assert_eq!(
            std::fs::read(bundle.join("thumbnail.jpg")).expect("prior cover remains"),
            previous
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn close_requested_uses_the_same_authoritative_project_local_cover_save() {
        use opentake_domain::{Clip, ClipType, MediaManifestEntry, MediaSource, Track};

        if !opentake_media::ffmpeg_status::ffmpeg_available()
            || opentake_render::RenderDevice::try_new().is_err()
        {
            return;
        }
        let fixture = tempfile::tempdir().expect("close fixture");
        let bundle = fixture.path().join("Close.opentake");
        std::fs::create_dir_all(bundle.join("media")).expect("create retained media");
        image::RgbaImage::from_pixel(80, 120, image::Rgba([30, 140, 220, 255]))
            .save(bundle.join("media/inside.png"))
            .expect("write project-local image");
        let mut project = opentake_project::Project::new(&bundle);
        project.timeline.width = 80;
        project.timeline.height = 120;
        let mut clip = Clip::new("clip", "local", 0, 30);
        clip.media_type = ClipType::Image;
        let mut track = Track::new("video", ClipType::Video);
        track.clips.push(clip);
        project.timeline.tracks.push(track);
        project.manifest.entries.push(MediaManifestEntry {
            id: "local".into(),
            name: "local".into(),
            kind: ClipType::Image,
            source: MediaSource::Project {
                relative_path: "media/inside.png".into(),
            },
            duration: 1.0,
            source_width: Some(80),
            source_height: Some(120),
            source_fps: None,
            generation_input: None,
            has_audio: Some(false),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        });
        project.save().expect("save local project");
        let core = AppCore::new();
        core.open_project(&bundle).expect("open local project");
        let app = tauri::test::mock_builder()
            .manage(core)
            .manage(crate::render::RenderState::new())
            .manage(crate::updater::InstallAdmissionGate::default())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build managed mock app");

        save_current_project_with_composite_cover(app.handle().clone())
            .await
            .expect("close parity save");

        let cover = image::open(bundle.join("thumbnail.jpg"))
            .expect("close writes cover")
            .to_rgb8();
        assert_eq!((cover.width(), cover.height()), (640, 360));
        let center = cover.get_pixel(320, 180).0;
        assert!(center[2] > 120 && center[1] > 70, "{center:?}");
        assert!(cover
            .get_pixel(20, 180)
            .0
            .iter()
            .all(|channel| *channel < 20));
    }

    fn capture_corrupt_external(kind: opentake_domain::ClipType) -> ProjectCoverCapture {
        use opentake_domain::{Clip, MediaManifestEntry, MediaSource, Track};
        use tauri::Manager as _;

        let fixture = tempfile::tempdir().expect("corrupt fixture");
        let source = fixture.path().join(match kind {
            opentake_domain::ClipType::Video => "broken.mp4",
            _ => "broken.png",
        });
        std::fs::write(&source, b"corrupt media bytes").expect("write corrupt source");
        let bundle = fixture.path().join("Corrupt.opentake");
        let mut project = opentake_project::Project::new(&bundle);
        let mut clip = Clip::new("clip", "media", 0, 30);
        clip.media_type = kind;
        let mut track = Track::new("video", opentake_domain::ClipType::Video);
        track.clips.push(clip);
        project.timeline.tracks.push(track);
        project.manifest.entries.push(MediaManifestEntry {
            id: "media".into(),
            name: "media".into(),
            kind,
            source: MediaSource::External {
                absolute_path: source.to_string_lossy().into_owned(),
            },
            duration: 1.0,
            source_width: Some(64),
            source_height: Some(64),
            source_fps: (kind == opentake_domain::ClipType::Video).then_some(30.0),
            generation_input: None,
            has_audio: Some(false),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        });
        project.save().expect("save corrupt project");
        let core = AppCore::new();
        core.open_project(&bundle).expect("open corrupt project");
        let app = tauri::test::mock_app();
        app.handle()
            .asset_protocol_scope()
            .allow_file(&source)
            .expect("authorize corrupt source");
        capture_composite_project_thumbnail(
            app.handle(),
            &core,
            &core.runtime_snapshot(),
            &crate::render::RenderState::new(),
            &opentake_media::MediaCancelToken::new(),
        )
    }

    #[test]
    fn corrupt_planned_image_is_capture_failed() {
        if !opentake_media::ffmpeg_status::ffmpeg_available()
            || opentake_render::RenderDevice::try_new().is_err()
        {
            return;
        }
        assert_eq!(
            capture_corrupt_external(opentake_domain::ClipType::Image),
            ProjectCoverCapture::CaptureFailed
        );
    }

    #[test]
    fn corrupt_planned_video_is_capture_failed() {
        if !opentake_media::ffmpeg_status::ffmpeg_available()
            || opentake_render::RenderDevice::try_new().is_err()
        {
            return;
        }
        assert_eq!(
            capture_corrupt_external(opentake_domain::ClipType::Video),
            ProjectCoverCapture::CaptureFailed
        );
    }

    #[test]
    fn corrupt_planned_text_is_capture_failed() {
        use opentake_domain::{Clip, ClipType, TextStyle, Track};
        let fixture = tempfile::tempdir().expect("text fixture");
        let bundle = fixture.path().join("Text.opentake");
        AppCore::new()
            .save_project(Some(bundle.clone()))
            .expect("save project");
        let core = AppCore::new();
        core.open_project(&bundle).expect("open project");
        let mut snapshot = core.runtime_snapshot();
        let mut text = Clip::new("text", "", 0, 30);
        text.media_type = ClipType::Text;
        text.text_content = Some("invalid".into());
        text.text_style = Some(TextStyle {
            font_size: f64::NAN,
            ..TextStyle::default()
        });
        let mut track = Track::new("text", ClipType::Text);
        track.clips.push(text);
        snapshot.timeline.tracks.push(track);
        let app = tauri::test::mock_app();

        let result = capture_composite_project_thumbnail(
            app.handle(),
            &core,
            &snapshot,
            &crate::render::RenderState::new(),
            &opentake_media::MediaCancelToken::new(),
        );

        assert_eq!(result, ProjectCoverCapture::CaptureFailed);
    }

    #[test]
    fn unapproved_external_source_is_capture_failed_without_opening_it() {
        use opentake_domain::{Clip, ClipType, MediaManifestEntry, MediaSource, Track};
        let fixture = tempfile::tempdir().expect("scope fixture");
        let source = fixture.path().join("unapproved.png");
        image::RgbaImage::from_pixel(32, 32, image::Rgba([10, 20, 30, 255]))
            .save(&source)
            .expect("write image");
        let bundle = fixture.path().join("Scoped.opentake");
        let mut project = opentake_project::Project::new(&bundle);
        let mut clip = Clip::new("clip", "media", 0, 30);
        clip.media_type = ClipType::Image;
        let mut track = Track::new("video", ClipType::Video);
        track.clips.push(clip);
        project.timeline.tracks.push(track);
        project.manifest.entries.push(MediaManifestEntry {
            id: "media".into(),
            name: "media".into(),
            kind: ClipType::Image,
            source: MediaSource::External {
                absolute_path: source.to_string_lossy().into_owned(),
            },
            duration: 1.0,
            source_width: Some(32),
            source_height: Some(32),
            source_fps: None,
            generation_input: None,
            has_audio: Some(false),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        });
        project.save().expect("save scope project");
        let core = AppCore::new();
        core.open_project(&bundle).expect("open scope project");
        let app = tauri::test::mock_app();
        app.handle()
            .asset_protocol_scope()
            .forbid_file(&source)
            .expect("forbid unapproved source");

        assert_eq!(
            capture_composite_project_thumbnail(
                app.handle(),
                &core,
                &core.runtime_snapshot(),
                &crate::render::RenderState::new(),
                &opentake_media::MediaCancelToken::new(),
            ),
            ProjectCoverCapture::CaptureFailed
        );
    }

    #[test]
    fn thumbnail_authoritative_composite_contains_transition_overlay_transform_and_text() {
        use opentake_domain::{
            Clip, ClipType, Fill, MediaManifestEntry, MediaSource, Point, Rgba, TextStyle, Track,
            Transform, Transition, TransitionKind,
        };
        use opentake_media::{
            ffmpeg_status::ffmpeg_available, ExportPreset, ExportResolution, RgbaFrame, VideoCodec,
            VideoEncoder,
        };

        if !ffmpeg_available() || opentake_render::RenderDevice::try_new().is_err() {
            eprintln!("skip: authoritative cover fixture needs ffmpeg and a GPU adapter");
            return;
        }
        let fixture = tempfile::tempdir().expect("fixture tempdir");
        let video = fixture.path().join("background.mp4");
        let preset = ExportPreset::new(VideoCodec::H264, ExportResolution::P720);
        let mut encoder =
            VideoEncoder::new(&video, 320, 180, 30, &preset).expect("start background encoder");
        for _ in 0..60 {
            encoder
                .push_frame(&RgbaFrame::new(
                    320,
                    180,
                    [210, 20, 20, 255].repeat(320 * 180),
                ))
                .expect("encode background frame");
        }
        encoder.finish().expect("finish background video");
        let incoming = fixture.path().join("incoming.png");
        let overlay = fixture.path().join("overlay.png");
        image::RgbaImage::from_pixel(320, 180, image::Rgba([20, 180, 20, 255]))
            .save(&incoming)
            .expect("save incoming image");
        image::RgbaImage::from_pixel(320, 180, image::Rgba([20, 40, 220, 255]))
            .save(&overlay)
            .expect("save overlay image");

        let entry = |id: &str, kind: ClipType, path: &std::path::Path| MediaManifestEntry {
            id: id.into(),
            name: id.into(),
            kind,
            source: MediaSource::External {
                absolute_path: path.to_string_lossy().into_owned(),
            },
            duration: 2.0,
            generation_input: None,
            source_width: Some(320),
            source_height: Some(180),
            source_fps: (kind == ClipType::Video).then_some(30.0),
            has_audio: Some(false),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        };
        let mut outgoing = Clip::new("outgoing", "background", 0, 30);
        outgoing.transition_out = Some(Transition {
            from_clip_id: "outgoing".into(),
            to_clip_id: "incoming".into(),
            kind: TransitionKind::CrossDissolve,
            duration_frames: 10,
        });
        let mut incoming_clip = Clip::new("incoming", "incoming", 30, 30);
        incoming_clip.media_type = ClipType::Image;
        let mut background_track = Track::new("background", ClipType::Video);
        background_track.clips = vec![outgoing, incoming_clip];
        let mut overlay_clip = Clip::new("overlay", "overlay", 20, 12);
        overlay_clip.media_type = ClipType::Image;
        overlay_clip.transform = Transform::from_center(Point { x: 0.78, y: 0.5 }, 0.3, 0.6);
        overlay_clip.transform.rotation = 8.0;
        let mut overlay_track = Track::new("overlay", ClipType::Video);
        overlay_track.clips.push(overlay_clip);
        let mut text_clip = Clip::new("text", "", 20, 20);
        text_clip.media_type = ClipType::Text;
        text_clip.text_content = Some("Composite".into());
        let mut text_style = TextStyle::default();
        text_style.background = Fill::new(true, Rgba::new(0.8, 0.1, 0.8, 1.0));
        text_clip.text_style = Some(text_style);
        text_clip.transform = Transform::from_center(Point { x: 0.28, y: 0.5 }, 0.4, 0.2);
        let mut text_track = Track::new("text", ClipType::Text);
        text_track.clips.push(text_clip);
        let mut timeline = opentake_domain::Timeline::new();
        timeline.width = 320;
        timeline.height = 180;
        // Track zero is the topmost visual track, matching the production render
        // plan. Keep the transformed overlay above the transition background.
        timeline.tracks = vec![overlay_track, background_track, text_track];
        let mut project = opentake_project::Project::new(fixture.path().join("Composite.opentake"));
        project.timeline = timeline;
        project.manifest.entries = vec![
            entry("background", ClipType::Video, &video),
            entry("incoming", ClipType::Image, &incoming),
            entry("overlay", ClipType::Image, &overlay),
        ];
        project.save().expect("save composite fixture");
        let core = AppCore::new();
        core.open_project(&project.bundle_path)
            .expect("open composite fixture");
        let app = tauri::test::mock_app();
        use tauri::Manager as _;
        for path in [&video, &incoming, &overlay] {
            app.handle()
                .asset_protocol_scope()
                .allow_file(path)
                .expect("authorize composite fixture");
        }
        let cancel = opentake_media::MediaCancelToken::new();

        let capture = capture_composite_project_thumbnail(
            app.handle(),
            &core,
            &core.runtime_snapshot(),
            &crate::render::RenderState::new(),
            &cancel,
        );
        let ProjectCoverCapture::Captured(bytes) = capture else {
            panic!("capture authoritative cover: {capture:?}");
        };
        let cover = image::load_from_memory(&bytes)
            .expect("decode authoritative cover")
            .to_rgb8();
        assert_eq!((cover.width(), cover.height()), (640, 360));
        let transition = cover.get_pixel(360, 40).0;
        let overlay_pixel = cover.get_pixel(500, 180).0;
        let outside_overlay = cover.get_pixel(620, 180).0;
        let text_background = cover.get_pixel(180, 180).0;
        assert!(transition[0] > 70 && transition[1] > 50, "{transition:?}");
        assert!(
            overlay_pixel[2] > 120 && overlay_pixel[0] < 100,
            "{overlay_pixel:?}"
        );
        assert!(outside_overlay[2] < 100, "{outside_overlay:?}");
        assert!(
            text_background[0] > 120 && text_background[2] > 120,
            "{text_background:?}"
        );
    }
}

#[cfg(all(test, feature = "playback-engine"))]
mod project_prewarm_lifecycle_tests {
    use super::project_open_with_playback_and_prewarm;
    use crate::media::prewarm::PrewarmScheduler;
    use crate::playback::PlaybackState;
    use opentake_agent::mcp::core_handle::{AppCoreHandle, CoreHandle};
    use opentake_core::AppCore;
    use opentake_domain::{Clip, ClipType, MediaManifestEntry, MediaSource, Track};
    use opentake_project::{GenerationLog, GenerationLogEntry, Project};
    use opentake_render::{build_render_plan, RenderSize};

    #[test]
    fn failed_project_prepare_changes_no_playback_or_prewarm_state() {
        let core = AppCore::new();
        let before = core.project_revision();
        let playback = PlaybackState::new();
        let prewarm = PrewarmScheduler::new(before.project_epoch);
        let missing = tempfile::tempdir()
            .expect("tempdir")
            .path()
            .join("missing.opentake")
            .to_string_lossy()
            .into_owned();

        let error = project_open_with_playback_and_prewarm(&core, missing, &playback, &prewarm)
            .expect_err("missing project must fail in prepare");

        assert_eq!(
            error.code,
            crate::playback::session::PlaybackErrorCode::Engine
        );
        assert_eq!(core.project_revision(), before);
        assert_eq!(prewarm.project_state(), (before.project_epoch, false));
        assert!(playback.active_identity().is_none());
    }

    #[test]
    fn project_open_mapped_boundaries_composite_acceptance() {
        let fixture = tempfile::tempdir().expect("fixture tempdir");
        let bundle = fixture.path().join("mapped-boundaries.opentake");
        let media_path = bundle.join("media/source.mov");
        let mut project = Project::new(&bundle);
        let mut track = Track::new("mapped-track", ClipType::Video);
        track
            .clips
            .push(Clip::new("mapped-clip", "mapped-media", 12, 48));
        project.timeline.tracks.push(track);
        project.manifest.entries.push(MediaManifestEntry {
            id: "mapped-media".into(),
            name: "source.mov".into(),
            kind: ClipType::Video,
            source: MediaSource::Project {
                relative_path: "media/source.mov".into(),
            },
            duration: 2.0,
            generation_input: None,
            source_width: Some(1280),
            source_height: Some(720),
            source_fps: Some(24.0),
            has_audio: Some(false),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        });
        project.generation_log = Some(GenerationLog {
            version: 1,
            entries: vec![GenerationLogEntry::new(
                "mapped-generation",
                "mapped-model",
                Some(10),
                Some(700_000_000.0),
            )],
        });
        project.save().expect("save mapped project fixture");
        std::fs::create_dir_all(media_path.parent().expect("media parent"))
            .expect("create media directory");
        std::fs::write(&media_path, b"mapped-media-bytes").expect("write mapped media");

        let core = AppCore::new();
        let before = core.project_revision();
        let playback = PlaybackState::new();
        let prewarm = PrewarmScheduler::new(before.project_epoch);
        let opened = project_open_with_playback_and_prewarm(
            &core,
            bundle.to_string_lossy().into_owned(),
            &playback,
            &prewarm,
        )
        .expect("open through desktop coordinator");
        assert_eq!(opened.version, 0);
        assert_eq!(prewarm.project_state(), (opened.project_epoch, false));

        let catalog = crate::media::MediaListDto::from_core(&core, None);
        assert_eq!(catalog.items.len(), 1);
        assert_eq!(catalog.items[0].id, "mapped-media");
        assert!(!catalog.items[0].missing);
        assert_eq!(catalog.items[0].file_size, Some(18));

        let snapshot = core.runtime_snapshot();
        let (sizes, playback_media) =
            crate::playback::project_media(&snapshot.media, &snapshot.project_dir);
        assert_eq!(
            playback_media
                .get("mapped-media")
                .expect("playback media route")
                .path,
            media_path
        );
        let metrics = crate::playback::ManifestMetrics {
            sizes,
            straight_alpha: std::collections::HashSet::new(),
        };
        let plan = build_render_plan(
            &snapshot.timeline,
            RenderSize::new(
                snapshot.timeline.width as u32,
                snapshot.timeline.height as u32,
            ),
            &metrics,
        );
        assert_eq!(plan.clip_plans.len(), 1);
        assert_eq!(plan.clip_plans[0].clip_id, "mapped-clip");
        assert_eq!(plan.frame(&snapshot.timeline, 12).draws.len(), 1);

        let agent = AppCoreHandle::new(core.clone());
        assert_eq!(agent.timeline(), snapshot.timeline);
        assert_eq!(agent.media(), snapshot.media);
        assert_eq!(agent.media_path("mapped-media"), Some(media_path.clone()));

        core.save_project(None).expect("save mapped project");
        let reopened = AppCore::new();
        reopened
            .open_project(&bundle)
            .expect("reopen mapped project");
        let reopened_agent = AppCoreHandle::new(reopened.clone());
        assert_eq!(reopened_agent.timeline(), agent.timeline());
        assert_eq!(reopened_agent.media(), agent.media());
        assert_eq!(reopened_agent.media_path("mapped-media"), Some(media_path));
        assert_eq!(reopened.generation_log(), core.generation_log());
    }

    #[test]
    fn successful_open_activates_prepared_epoch_in_both_coordinators() {
        let fixture = tempfile::tempdir().expect("fixture tempdir");
        let bundle = fixture.path().join("prepared.opentake");
        let source = AppCore::new();
        source
            .save_project(Some(bundle.clone()))
            .expect("save project fixture");

        let core = AppCore::new();
        let before = core.project_revision();
        let playback = PlaybackState::new();
        let prewarm = PrewarmScheduler::new(before.project_epoch);
        let snapshot = project_open_with_playback_and_prewarm(
            &core,
            bundle.to_string_lossy().into_owned(),
            &playback,
            &prewarm,
        )
        .expect("commit prepared project");

        assert_ne!(snapshot.project_epoch, before.project_epoch);
        assert_eq!(prewarm.project_state(), (snapshot.project_epoch, false));
    }
}

#[cfg(test)]
mod edit_request_serde_tests {
    use super::{begin_edit_activity, validate_freeze_frame_request, EditRequest};
    use opentake_core::{AppCore, EditCommand};
    use opentake_domain::{ClipType, TransitionKind};
    use opentake_ops::command::{NewTrackClipMode, PlaceMediaTarget};
    use opentake_ops::ClipEntry;

    #[test]
    fn deferred_analysis_continuation_cannot_commit_after_install_wins_the_await_boundary() {
        let admission = crate::updater::InstallAdmissionGate::default();
        let analysis = admission.begin_activity().unwrap();
        assert!(admission.begin_install().is_err());
        drop(analysis);

        let install = admission.begin_install().unwrap();
        assert!(begin_edit_activity(&admission).is_err());
        drop(install);
        assert!(begin_edit_activity(&admission).is_ok());
    }

    fn request_route(request: &EditRequest) -> &'static str {
        match request {
            EditRequest::CreateNestedSequence { .. } => "CreateNestedSequence",
            EditRequest::EditNestedSequence { .. } => "EditNestedSequence",
            EditRequest::RenameNestedSequence { .. } => "RenameNestedSequence",
            EditRequest::DissolveNestedSequence { .. } => "DissolveNestedSequence",
            EditRequest::PlaceMedia { .. } => "PlaceMedia",
            EditRequest::AddClips { .. } => "AddClips",
            EditRequest::InsertClips { .. } => "InsertClips",
            EditRequest::MoveClips { .. } => "MoveClips",
            EditRequest::DuplicateClips { .. } => "DuplicateClips",
            EditRequest::MoveOrDuplicateClipsToNewTrack { .. } => "MoveOrDuplicateClipsToNewTrack",
            EditRequest::PasteClips { .. } => "PasteClips",
            EditRequest::RemoveClips { .. } => "RemoveClips",
            EditRequest::SplitClip { .. } => "SplitClip",
            EditRequest::SplitClips { .. } => "SplitClips",
            EditRequest::FreezeFrame { .. } => "FreezeFrame",
            EditRequest::TrimClips { .. } => "TrimClips",
            EditRequest::SetClipProperties { .. } => "SetClipProperties",
            EditRequest::SetTransformAtFrame { .. } => "SetTransformAtFrame",
            EditRequest::SetKeyframes { .. } => "SetKeyframes",
            EditRequest::StampKeyframe { .. } => "StampKeyframe",
            EditRequest::UpsertKeyframe { .. } => "UpsertKeyframe",
            EditRequest::RemoveKeyframe { .. } => "RemoveKeyframe",
            EditRequest::MoveKeyframe { .. } => "MoveKeyframe",
            EditRequest::SetKeyframeInterpolation { .. } => "SetKeyframeInterpolation",
            EditRequest::SetColorGrade { .. } => "SetColorGrade",
            EditRequest::SetLut { .. } => "SetLut",
            EditRequest::SetChromaKey { .. } => "SetChromaKey",
            EditRequest::SetMasks { .. } => "SetMasks",
            EditRequest::SetEffects { .. } => "SetEffects",
            EditRequest::SetLoudnessNormalization { .. } => "SetLoudnessNormalization",
            EditRequest::SetAudioDenoise { .. } => "SetAudioDenoise",
            EditRequest::ApplyStabilization { .. } => "ApplyStabilization",
            EditRequest::AdjustStabilization { .. } => "AdjustStabilization",
            EditRequest::ResetStabilization { .. } => "ResetStabilization",
            EditRequest::SetTransition { .. } => "SetTransition",
            EditRequest::RippleDeleteRanges { .. } => "RippleDeleteRanges",
            EditRequest::RippleDeleteClips { .. } => "RippleDeleteClips",
            EditRequest::AddTexts { .. } => "AddTexts",
            EditRequest::AddTextsAutoTrack { .. } => "AddTextsAutoTrack",
            EditRequest::AddCaptions { .. } => "AddCaptions",
            EditRequest::Link { .. } => "Link",
            EditRequest::Unlink { .. } => "Unlink",
            EditRequest::RemoveTracks { .. } => "RemoveTracks",
            EditRequest::SwapTracks { .. } => "SwapTracks",
            EditRequest::SwapClips { .. } => "SwapClips",
            EditRequest::InsertTrack { .. } => "InsertTrack",
            EditRequest::SetTrackProps { .. } => "SetTrackProps",
            EditRequest::CreateFolder { .. } => "CreateFolder",
            EditRequest::MoveToFolder { .. } => "MoveToFolder",
            EditRequest::RenameMedia { .. } => "RenameMedia",
            EditRequest::RenameFolder { .. } => "RenameFolder",
            EditRequest::DeleteMedia { .. } => "DeleteMedia",
            EditRequest::DeleteFolder { .. } => "DeleteFolder",
            EditRequest::SwapMedia { .. } => "SwapMedia",
            EditRequest::ResetTransform { .. } => "ResetTransform",
            EditRequest::SetTimelineSettings { .. } => "SetTimelineSettings",
        }
    }

    fn command_matches_route(command: &EditCommand, route: &str) -> bool {
        matches!(
            (route, command),
            (
                "CreateNestedSequence",
                EditCommand::CreateNestedSequenceFromClips { .. }
            ) | ("EditNestedSequence", EditCommand::EditNestedSequence { .. })
                | (
                    "RenameNestedSequence",
                    EditCommand::RenameNestedSequence { .. }
                )
                | (
                    "DissolveNestedSequence",
                    EditCommand::DissolveNestedSequence { .. }
                )
                | ("PlaceMedia", EditCommand::PlaceMedia { .. })
                | ("AddClips", EditCommand::AddClips { .. })
                | ("InsertClips", EditCommand::InsertClips { .. })
                | ("MoveClips", EditCommand::MoveClips { .. })
                | ("DuplicateClips", EditCommand::DuplicateClips { .. })
                | (
                    "MoveOrDuplicateClipsToNewTrack",
                    EditCommand::MoveOrDuplicateClipsToNewTrack { .. }
                )
                | ("PasteClips", EditCommand::PasteClips { .. })
                | ("RemoveClips", EditCommand::RemoveClips { .. })
                | ("SplitClip", EditCommand::SplitClip { .. })
                | ("SplitClips", EditCommand::SplitClips { .. })
                | ("TrimClips", EditCommand::TrimClips { .. })
                | ("SetClipProperties", EditCommand::SetClipProperties { .. })
                | (
                    "SetTransformAtFrame",
                    EditCommand::SetTransformAtFrame { .. }
                )
                | ("SetKeyframes", EditCommand::SetKeyframes { .. })
                | ("StampKeyframe", EditCommand::StampKeyframe { .. })
                | ("UpsertKeyframe", EditCommand::UpsertKeyframe { .. })
                | ("RemoveKeyframe", EditCommand::RemoveKeyframe { .. })
                | ("MoveKeyframe", EditCommand::MoveKeyframe { .. })
                | (
                    "SetKeyframeInterpolation",
                    EditCommand::SetKeyframeInterpolation { .. }
                )
                | ("SetColorGrade", EditCommand::SetColorGrade { .. })
                | ("SetLut", EditCommand::SetLut { .. })
                | ("SetChromaKey", EditCommand::SetChromaKey { .. })
                | ("SetMasks", EditCommand::SetMasks { .. })
                | ("SetEffects", EditCommand::SetEffects { .. })
                | (
                    "SetLoudnessNormalization",
                    EditCommand::SetLoudnessNormalization { .. }
                )
                | ("SetAudioDenoise", EditCommand::SetAudioDenoise { .. })
                | ("ApplyStabilization", EditCommand::ApplyStabilization { .. })
                | (
                    "AdjustStabilization",
                    EditCommand::AdjustStabilization { .. }
                )
                | ("ResetStabilization", EditCommand::ResetStabilization { .. })
                | ("SetTransition", EditCommand::SetTransition { .. })
                | ("RippleDeleteRanges", EditCommand::RippleDeleteRanges { .. })
                | ("RippleDeleteClips", EditCommand::RippleDeleteClips { .. })
                | ("AddTexts", EditCommand::AddTexts { .. })
                | ("AddTextsAutoTrack", EditCommand::AddTextsAutoTrack { .. })
                | ("AddCaptions", EditCommand::AddCaptions { .. })
                | ("Link", EditCommand::Link { .. })
                | ("Unlink", EditCommand::Unlink { .. })
                | ("RemoveTracks", EditCommand::RemoveTracks { .. })
                | ("SwapTracks", EditCommand::SwapTracks { .. })
                | ("SwapClips", EditCommand::SwapClips { .. })
                | ("InsertTrack", EditCommand::InsertTrack { .. })
                | ("SetTrackProps", EditCommand::SetTrackProps { .. })
                | ("CreateFolder", EditCommand::CreateFolder { .. })
                | ("MoveToFolder", EditCommand::MoveToFolder { .. })
                | ("RenameMedia", EditCommand::RenameMedia { .. })
                | ("RenameFolder", EditCommand::RenameFolder { .. })
                | ("DeleteMedia", EditCommand::DeleteMedia { .. })
                | ("DeleteFolder", EditCommand::DeleteFolder { .. })
                | ("SwapMedia", EditCommand::SwapMedia { .. })
                | ("ResetTransform", EditCommand::ResetTransform { .. })
                | (
                    "SetTimelineSettings",
                    EditCommand::SetTimelineSettings { .. }
                )
        )
    }

    fn assert_every_edit_request_maps_to_exact_edit_command() {
        let cases = [
            (
                r#"{"type":"createNestedSequence","name":"Scene","clipIds":["c"]}"#,
                "CreateNestedSequence",
            ),
            (
                r#"{"type":"editNestedSequence","sequenceId":"s","command":{"type":"removeClips","clipIds":["c"]}}"#,
                "EditNestedSequence",
            ),
            (
                r#"{"type":"renameNestedSequence","sequenceId":"s","name":"Scene"}"#,
                "RenameNestedSequence",
            ),
            (
                r#"{"type":"dissolveNestedSequence","clipId":"c"}"#,
                "DissolveNestedSequence",
            ),
            (
                r#"{"type":"placeMedia","sequenceId":null,"settings":{"fps":24,"width":1920,"height":1080},"target":{"kind":"newTrack","trackType":"video","at":0},"entry":{"mediaRef":"m","mediaType":"video","sourceClipType":"video","startFrame":0,"durationFrames":24,"hasAudio":false,"addLinkedAudio":false}}"#,
                "PlaceMedia",
            ),
            (r#"{"type":"addClips","entries":[]}"#, "AddClips"),
            (
                r#"{"type":"insertClips","trackIndex":0,"atFrame":0,"entries":[]}"#,
                "InsertClips",
            ),
            (r#"{"type":"moveClips","moves":[]}"#, "MoveClips"),
            (
                r#"{"type":"duplicateClips","clipIds":[],"offsetFrames":0,"targetTrackIndexes":[]}"#,
                "DuplicateClips",
            ),
            (
                r#"{"type":"moveOrDuplicateClipsToNewTrack","clipIds":["c"],"leadClipId":"c","requestedFrameDelta":1,"insertAt":0,"mode":"move"}"#,
                "MoveOrDuplicateClipsToNewTrack",
            ),
            (r#"{"type":"pasteClips","entries":[]}"#, "PasteClips"),
            (r#"{"type":"removeClips","clipIds":[]}"#, "RemoveClips"),
            (
                r#"{"type":"splitClip","clipId":"c","atFrame":1}"#,
                "SplitClip",
            ),
            (
                r#"{"type":"splitClips","clipIds":["a","b"],"atFrame":1}"#,
                "SplitClips",
            ),
            (
                r#"{"type":"freezeFrame","clipId":"c","atFrame":1,"durationFrames":1}"#,
                "FreezeFrame",
            ),
            (r#"{"type":"trimClips","edits":[]}"#, "TrimClips"),
            (
                r#"{"type":"setClipProperties","clipIds":[],"properties":{}}"#,
                "SetClipProperties",
            ),
            (
                r#"{"type":"setTransformAtFrame","clipId":"c","frame":1,"transform":{"centerX":0.5,"centerY":0.5,"width":1.0,"height":1.0,"rotation":0.0,"flipHorizontal":false,"flipVertical":false}}"#,
                "SetTransformAtFrame",
            ),
            (
                r#"{"type":"setKeyframes","clipId":"c","property":"opacity","payload":{"kind":"scalar","keyframes":[]}}"#,
                "SetKeyframes",
            ),
            (
                r#"{"type":"stampKeyframe","clipId":"c","property":"opacity","frame":1}"#,
                "StampKeyframe",
            ),
            (
                r#"{"type":"upsertKeyframe","clipId":"c","property":"opacity","frame":1,"value":{"kind":"scalar","value":0.5}}"#,
                "UpsertKeyframe",
            ),
            (
                r#"{"type":"removeKeyframe","clipId":"c","property":"opacity","frame":1}"#,
                "RemoveKeyframe",
            ),
            (
                r#"{"type":"moveKeyframe","clipId":"c","property":"opacity","fromFrame":1,"toFrame":2}"#,
                "MoveKeyframe",
            ),
            (
                r#"{"type":"setKeyframeInterpolation","clipId":"c","property":"opacity","frame":1,"interpolation":"hold"}"#,
                "SetKeyframeInterpolation",
            ),
            (
                r#"{"type":"setColorGrade","clipIds":[],"grade":null}"#,
                "SetColorGrade",
            ),
            (r#"{"type":"setLut","clipIds":[],"lut":null}"#, "SetLut"),
            (
                r#"{"type":"setChromaKey","clipIds":[],"chromaKey":null}"#,
                "SetChromaKey",
            ),
            (r#"{"type":"setMasks","clipIds":[],"masks":[]}"#, "SetMasks"),
            (
                r#"{"type":"setEffects","clipIds":[],"effects":[]}"#,
                "SetEffects",
            ),
            (
                r#"{"type":"setLoudnessNormalization","clipId":"c","normalization":null}"#,
                "SetLoudnessNormalization",
            ),
            (
                r#"{"type":"setAudioDenoise","clipId":"c","denoise":null}"#,
                "SetAudioDenoise",
            ),
            (
                r#"{"type":"applyStabilization","clipId":"c","solution":{"model":"opentake.motion-smoothing","modelVersion":1,"sourceIdentity":"asset","strength":1.0,"cropMargin":0.0,"keyframes":[{"frame":0,"translationX":0.0,"translationY":0.0,"rotationDegrees":0.0},{"frame":1,"translationX":0.0,"translationY":0.0,"rotationDegrees":0.0}]}}"#,
                "ApplyStabilization",
            ),
            (
                r#"{"type":"adjustStabilization","clipId":"c","strength":0.75,"cropMargin":0.02}"#,
                "AdjustStabilization",
            ),
            (
                r#"{"type":"resetStabilization","clipId":"c"}"#,
                "ResetStabilization",
            ),
            (
                r#"{"type":"setTransition","fromClipId":"a","toClipId":"b","kind":null,"durationFrames":1}"#,
                "SetTransition",
            ),
            (
                r#"{"type":"rippleDeleteRanges","trackIndex":0,"ranges":[]}"#,
                "RippleDeleteRanges",
            ),
            (
                r#"{"type":"rippleDeleteClips","clipIds":[]}"#,
                "RippleDeleteClips",
            ),
            (r#"{"type":"addTexts","entries":[]}"#, "AddTexts"),
            (
                r#"{"type":"addTextsAutoTrack","entries":[]}"#,
                "AddTextsAutoTrack",
            ),
            (r#"{"type":"addCaptions","entries":[]}"#, "AddCaptions"),
            (r#"{"type":"link","clipIds":[]}"#, "Link"),
            (r#"{"type":"unlink","clipIds":[]}"#, "Unlink"),
            (
                r#"{"type":"removeTracks","trackIndexes":[]}"#,
                "RemoveTracks",
            ),
            (r#"{"type":"swapTracks","a":0,"b":1}"#, "SwapTracks"),
            (
                r#"{"type":"swapClips","clipA":"a","clipB":"b"}"#,
                "SwapClips",
            ),
            (
                r#"{"type":"insertTrack","kind":"video","at":0}"#,
                "InsertTrack",
            ),
            (
                r#"{"type":"setTrackProps","trackIndex":0,"muted":true}"#,
                "SetTrackProps",
            ),
            (r#"{"type":"createFolder","name":"f"}"#, "CreateFolder"),
            (
                r#"{"type":"moveToFolder","assetIds":[],"folderId":null}"#,
                "MoveToFolder",
            ),
            (r#"{"type":"renameMedia","entries":[]}"#, "RenameMedia"),
            (r#"{"type":"renameFolder","entries":[]}"#, "RenameFolder"),
            (r#"{"type":"deleteMedia","assetIds":[]}"#, "DeleteMedia"),
            (r#"{"type":"deleteFolder","folderIds":[]}"#, "DeleteFolder"),
            (
                r#"{"type":"swapMedia","clipId":"c","mediaRef":"m"}"#,
                "SwapMedia",
            ),
            (
                r#"{"type":"resetTransform","clipIds":[]}"#,
                "ResetTransform",
            ),
            (
                r#"{"type":"setTimelineSettings","fps":24,"width":1920,"height":1080}"#,
                "SetTimelineSettings",
            ),
        ];

        assert_eq!(cases.len(), 56);
        for (json, expected_route) in cases {
            let mut hostile = serde_json::from_str::<serde_json::Value>(json).unwrap();
            hostile
                .as_object_mut()
                .unwrap()
                .insert("unexpected".to_string(), serde_json::json!(true));
            assert!(
                serde_json::from_value::<EditRequest>(hostile).is_err(),
                "{expected_route} must reject unknown fields"
            );

            let request = serde_json::from_str::<EditRequest>(json)
                .unwrap_or_else(|error| panic!("{expected_route} DTO failed: {error}"));
            assert_eq!(request_route(&request), expected_route);
            if expected_route == "FreezeFrame" {
                assert!(request.into_command().is_err());
            } else {
                let command = request.into_command().expect("request maps to EditCommand");
                assert!(
                    command_matches_route(&command, expected_route),
                    "{expected_route} mapped to {command:?}"
                );
            }
        }
    }

    #[test]
    fn every_frontend_edit_request_deserializes_to_intended_command() {
        assert_every_edit_request_maps_to_exact_edit_command();
    }

    #[test]
    fn every_edit_request_maps_to_exact_edit_command() {
        assert_every_edit_request_maps_to_exact_edit_command();
    }

    #[test]
    fn atomic_timeline_gesture_dtos_preserve_the_authoritative_plan() {
        let place = serde_json::from_str::<EditRequest>(
            r#"{
                "type":"placeMedia",
                "sequenceId":"sequence-a",
                "settings":{"fps":24,"width":3840,"height":2160},
                "target":{"kind":"existingTrack","trackId":"stable-track"},
                "entry":{
                    "mediaRef":"media-a",
                    "mediaType":"video",
                    "sourceClipType":"video",
                    "startFrame":12,
                    "durationFrames":48,
                    "trimStartFrame":3,
                    "trimEndFrame":5,
                    "hasAudio":true,
                    "addLinkedAudio":true
                }
            }"#,
        )
        .unwrap()
        .into_command()
        .unwrap();
        match place {
            EditCommand::PlaceMedia {
                sequence_id,
                settings,
                target,
                entry,
            } => {
                assert_eq!(sequence_id.as_deref(), Some("sequence-a"));
                let settings = settings.expect("settings preserved");
                assert_eq!(
                    (settings.fps, settings.width, settings.height),
                    (24, 3840, 2160)
                );
                assert_eq!(
                    target,
                    PlaceMediaTarget::ExistingTrack {
                        track_id: "stable-track".into()
                    }
                );
                assert_eq!(entry.media_ref, "media-a");
                assert_eq!(entry.media_type, ClipType::Video);
                assert_eq!((entry.start_frame, entry.duration_frames), (12, 48));
                assert_eq!(
                    (entry.trim_start_frame, entry.trim_end_frame),
                    (Some(3), Some(5))
                );
                assert!(entry.has_audio);
                assert!(entry.add_linked_audio);
            }
            command => panic!("unexpected command: {command:?}"),
        }

        let new_track = serde_json::from_str::<EditRequest>(
            r#"{
                "type":"moveOrDuplicateClipsToNewTrack",
                "clipIds":["lead","linked-audio"],
                "leadClipId":"lead",
                "requestedFrameDelta":-17,
                "insertAt":2,
                "mode":"duplicate"
            }"#,
        )
        .unwrap()
        .into_command()
        .unwrap();
        match new_track {
            EditCommand::MoveOrDuplicateClipsToNewTrack {
                clip_ids,
                lead_clip_id,
                requested_frame_delta,
                insert_at,
                mode,
            } => {
                assert_eq!(clip_ids, vec!["lead", "linked-audio"]);
                assert_eq!(lead_clip_id, "lead");
                assert_eq!(requested_frame_delta, -17);
                assert_eq!(insert_at, 2);
                assert_eq!(mode, NewTrackClipMode::Duplicate);
            }
            command => panic!("unexpected command: {command:?}"),
        }

        let paste = serde_json::from_str::<EditRequest>(
            r#"{
                "type":"pasteClips",
                "entries":[{
                    "clip":{
                        "id":"clipboard-a",
                        "mediaRef":"media-a",
                        "mediaType":"video",
                        "sourceClipType":"video",
                        "startFrame":10,
                        "durationFrames":20,
                        "trimStartFrame":2,
                        "trimEndFrame":4,
                        "speed":1.25,
                        "volume":0.75,
                        "fadeInFrames":3,
                        "fadeOutFrames":4,
                        "opacity":0.8,
                        "linkGroupId":"old-link",
                        "captionGroupId":"old-caption",
                        "transitionOut":{
                            "fromClipId":"clipboard-a",
                            "toClipId":"clipboard-b",
                            "kind":"crossDissolve",
                            "durationFrames":5
                        },
                        "reversed":true
                    },
                    "targetTrackId":"stable-track",
                    "startFrame":120
                }]
            }"#,
        )
        .unwrap()
        .into_command()
        .unwrap();
        match paste {
            EditCommand::PasteClips { entries } => {
                assert_eq!(entries.len(), 1);
                let entry = &entries[0];
                assert_eq!(entry.target_track_id, "stable-track");
                assert_eq!(entry.start_frame, 120);
                assert_eq!(entry.clip.id, "clipboard-a");
                assert_eq!(
                    (entry.clip.trim_start_frame, entry.clip.trim_end_frame),
                    (2, 4)
                );
                assert_eq!(
                    (entry.clip.speed, entry.clip.volume, entry.clip.opacity),
                    (1.25, 0.75, 0.8)
                );
                assert_eq!(entry.clip.link_group_id.as_deref(), Some("old-link"));
                assert_eq!(entry.clip.caption_group_id.as_deref(), Some("old-caption"));
                assert!(entry.clip.reversed);
                let transition = entry
                    .clip
                    .transition_out
                    .as_ref()
                    .expect("transition preserved");
                assert_eq!(transition.from_clip_id, "clipboard-a");
                assert_eq!(transition.to_clip_id, "clipboard-b");
                assert_eq!(transition.duration_frames, 5);
            }
            command => panic!("unexpected command: {command:?}"),
        }
    }

    // Regression: the front end sends camelCase keys (clipIds/clipId/atFrame…).
    // serde's enum-level `rename_all` does NOT rename struct-variant fields, so
    // each variant needs its own `rename_all`; without it RemoveClips/SplitClip/
    // … failed to deserialize ("missing field `clip_ids`") and delete/split/etc.
    // silently did nothing.
    #[test]
    fn deserializes_camelcase_multiword_commands() {
        serde_json::from_str::<EditRequest>(r#"{"type":"removeClips","clipIds":["a"]}"#)
            .expect("removeClips camelCase");
        serde_json::from_str::<EditRequest>(r#"{"type":"splitClip","clipId":"a","atFrame":5}"#)
            .expect("splitClip camelCase");
        serde_json::from_str::<EditRequest>(
            r#"{"type":"splitClips","clipIds":["a","b"],"atFrame":5}"#,
        )
        .expect("splitClips camelCase");
        serde_json::from_str::<EditRequest>(
            r#"{"type":"setTransformAtFrame","clipId":"a","frame":5,"transform":{"centerX":0.5,"centerY":0.5,"width":1.0,"height":1.0,"rotation":0.0,"flipHorizontal":false,"flipVertical":false}}"#,
        )
        .expect("setTransformAtFrame camelCase");
        serde_json::from_str::<EditRequest>(
            r#"{"type":"insertClips","trackIndex":0,"atFrame":0,"entries":[]}"#,
        )
        .expect("insertClips camelCase");
        serde_json::from_str::<EditRequest>(r#"{"type":"rippleDeleteClips","clipIds":["a"]}"#)
            .expect("rippleDeleteClips camelCase");
    }

    #[test]
    fn atomic_split_and_transform_requests_preserve_every_field() {
        let split = serde_json::from_str::<EditRequest>(
            r#"{"type":"splitClips","clipIds":["video","audio"],"atFrame":130}"#,
        )
        .expect("splitClips request")
        .into_command()
        .expect("splitClips command");
        match split {
            EditCommand::SplitClips { clip_ids, at_frame } => {
                assert_eq!(clip_ids, vec!["video", "audio"]);
                assert_eq!(at_frame, 130);
            }
            other => panic!("expected SplitClips, got {other:?}"),
        }

        let transform = serde_json::from_str::<EditRequest>(
            r#"{"type":"setTransformAtFrame","clipId":"video","frame":130,"transform":{"centerX":0.7,"centerY":0.6,"width":0.4,"height":0.25,"rotation":33.0,"flipHorizontal":true,"flipVertical":false}}"#,
        )
        .expect("setTransformAtFrame request")
        .into_command()
        .expect("setTransformAtFrame command");
        match transform {
            EditCommand::SetTransformAtFrame {
                clip_id,
                frame,
                transform,
            } => {
                assert_eq!(clip_id, "video");
                assert_eq!(frame, 130);
                assert_eq!(transform.center_x, 0.7);
                assert_eq!(transform.center_y, 0.6);
                assert_eq!(transform.width, 0.4);
                assert_eq!(transform.height, 0.25);
                assert_eq!(transform.rotation, 33.0);
                assert!(transform.flip_horizontal);
                assert!(!transform.flip_vertical);
            }
            other => panic!("expected SetTransformAtFrame, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_set_clip_properties_with_text_style() {
        // The Inspector sends camelCase `textStyle` with nested camelCase fields
        // (fontName/fontSize/…). It must deserialize and map onto the command's
        // ClipProperties.text_style.
        let request = serde_json::from_str::<EditRequest>(
            r#"{"type":"setClipProperties","clipIds":["c1"],"properties":{"textStyle":{"fontName":"Times-Bold","fontSize":48,"alignment":"left"}}}"#,
        )
        .expect("setClipProperties with textStyle camelCase");

        match request.into_command().expect("setClipProperties command") {
            EditCommand::SetClipProperties {
                clip_ids,
                properties,
            } => {
                assert_eq!(clip_ids, vec!["c1"]);
                let style = properties.text_style.expect("text_style present");
                assert_eq!(style.font_name, "Times-Bold");
                assert_eq!(style.font_size, 48.0);
            }
            other => panic!("expected SetClipProperties, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_set_clip_properties_with_reversed() {
        let request: EditRequest = serde_json::from_str(
            r#"{"type":"setClipProperties","clipIds":["c1"],"properties":{"reversed":true}}"#,
        )
        .expect("setClipProperties with reversed camelCase");

        match request.into_command().expect("setClipProperties command") {
            EditCommand::SetClipProperties { properties, .. } => {
                assert_eq!(properties.reversed, Some(true));
            }
            other => panic!("expected SetClipProperties, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_set_transition_pair_and_kind() {
        let request: EditRequest = serde_json::from_str(
            r#"{"type":"setTransition","fromClipId":"a","toClipId":"b","kind":"crossDissolve","durationFrames":15}"#,
        )
        .expect("setTransition camelCase");

        match request.into_command().expect("setTransition command") {
            EditCommand::SetTransition {
                from_clip_id,
                to_clip_id,
                kind,
                duration_frames,
            } => {
                assert_eq!(from_clip_id, "a");
                assert_eq!(to_clip_id, "b");
                assert_eq!(kind, Some(TransitionKind::CrossDissolve));
                assert_eq!(duration_frames, 15);
            }
            other => panic!("expected SetTransition, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_freeze_frame() {
        let request = serde_json::from_str::<EditRequest>(
            r#"{"type":"freezeFrame","clipId":"clip-1","atFrame":120,"durationFrames":30}"#,
        )
        .expect("freezeFrame camelCase");

        match request {
            EditRequest::FreezeFrame {
                clip_id,
                at_frame,
                duration_frames,
            } => {
                assert_eq!(clip_id, "clip-1");
                assert_eq!(at_frame, 120);
                assert_eq!(duration_frames, 30);
            }
            other => panic!("expected FreezeFrame, got {other:?}"),
        }
    }

    #[test]
    fn freeze_frame_preflight_rejects_bad_requests_before_capture() {
        let core = AppCore::new();

        core.apply(EditCommand::InsertTrack {
            kind: ClipType::Video,
            at: None,
        })
        .expect("video track");
        let err = validate_freeze_frame_request(&core, "nope", 10, 1).unwrap_err();
        assert!(err.contains("Clip not found"));

        let added = core
            .apply(EditCommand::AddClips {
                entries: vec![ClipEntry {
                    media_ref: "asset-1".into(),
                    media_type: ClipType::Video,
                    source_clip_type: ClipType::Video,
                    track_index: 0,
                    start_frame: 100,
                    duration_frames: 60,
                    trim_start_frame: None,
                    trim_end_frame: None,
                    has_audio: false,
                    add_linked_audio: false,
                    transform: None,
                }],
            })
            .expect("video clip");
        let clip_id = added.affected_clip_ids[0].clone();
        let err = validate_freeze_frame_request(&core, &clip_id, 100, 30).unwrap_err();
        assert!(err.contains("strictly inside clip range"));

        let err = validate_freeze_frame_request(&core, &clip_id, 120, 0).unwrap_err();
        assert!(err.contains("durationFrames must be >= 1"));

        let audio = AppCore::new();
        audio
            .apply(EditCommand::InsertTrack {
                kind: ClipType::Audio,
                at: None,
            })
            .expect("audio track");
        let added = audio
            .apply(EditCommand::AddClips {
                entries: vec![ClipEntry {
                    media_ref: "asset-a1".into(),
                    media_type: ClipType::Audio,
                    source_clip_type: ClipType::Audio,
                    track_index: 0,
                    start_frame: 100,
                    duration_frames: 60,
                    trim_start_frame: None,
                    trim_end_frame: None,
                    has_audio: true,
                    add_linked_audio: false,
                    transform: None,
                }],
            })
            .expect("audio clip");
        let audio_clip_id = added.affected_clip_ids[0].clone();
        let err = validate_freeze_frame_request(&audio, &audio_clip_id, 120, 30).unwrap_err();
        assert!(err.contains("video or image clip"));
    }

    #[test]
    fn deserializes_add_captions_camelcase_and_maps_to_command() {
        // The Captions tab / add_captions tool send camelCase caption entries.
        // Every multi-word field (startFrame/durationFrames/textStyle/
        // captionGroupId) must deserialize — a non-camelCase key here is the
        // repo's #1 silent-failure bug class, so this guards it explicitly.
        let request = serde_json::from_str::<EditRequest>(
            r#"{"type":"addCaptions","entries":[
                {"startFrame":0,"durationFrames":21,"content":"Hello",
                 "textStyle":{"fontName":"Helvetica-Bold","fontSize":48},
                 "transform":{"centerX":0.5,"centerY":0.9,"width":0.5,"height":0.1,
                              "rotation":0,"flipHorizontal":false,"flipVertical":false},
                 "captionGroupId":"grp-1"}
            ]}"#,
        )
        .expect("addCaptions camelCase");

        match request.into_command().expect("addCaptions command") {
            EditCommand::AddCaptions { entries } => {
                assert_eq!(entries.len(), 1);
                let e = &entries[0];
                assert_eq!(e.start_frame, 0);
                assert_eq!(e.duration_frames, 21);
                assert_eq!(e.content, "Hello");
                assert_eq!(e.caption_group_id, "grp-1");
                assert_eq!(e.text_style.font_size, 48.0);
                assert_eq!(e.transform.center_y, 0.9);
            }
            other => panic!("expected AddCaptions, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_add_texts_auto_track_camelcase_and_maps_to_command() {
        // `addTextClip` (Toolbar "T") and the `add_texts` MCP tool's
        // all-omitted-trackIndex path both send this DTO — no `trackIndex`
        // field at all (#194 fix: writes to a fresh track, never an existing
        // one). Every multi-word field (startFrame/durationFrames/textStyle)
        // must deserialize camelCase, same guard as addCaptions above.
        let request = serde_json::from_str::<EditRequest>(
            r#"{"type":"addTextsAutoTrack","entries":[
                {"startFrame":0,"durationFrames":90,"content":"Hello",
                 "textStyle":{"fontName":"Helvetica-Bold","fontSize":96},
                 "transform":{"centerX":0.5,"centerY":0.5,"width":0.5,"height":0.1,
                              "rotation":0,"flipHorizontal":false,"flipVertical":false}}
            ]}"#,
        )
        .expect("addTextsAutoTrack camelCase");

        match request.into_command().expect("addTextsAutoTrack command") {
            EditCommand::AddTextsAutoTrack { entries } => {
                assert_eq!(entries.len(), 1);
                let e = &entries[0];
                assert_eq!(e.start_frame, 0);
                assert_eq!(e.duration_frames, 90);
                assert_eq!(e.content, "Hello");
                assert_eq!(e.text_style.font_size, 96.0);
                assert_eq!(e.transform.center_x, 0.5);
            }
            other => panic!("expected AddTextsAutoTrack, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_swap_media_and_maps_to_command() {
        let request = serde_json::from_str::<EditRequest>(
            r#"{"type":"swapMedia","clipId":"clip-1","mediaRef":"asset-2"}"#,
        )
        .expect("swapMedia camelCase");

        match request.into_command().expect("swapMedia command") {
            EditCommand::SwapMedia { clip_id, media_ref } => {
                assert_eq!(clip_id, "clip-1");
                assert_eq!(media_ref, "asset-2");
            }
            other => panic!("expected SwapMedia, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_swap_tracks_and_maps_to_command() {
        let request = serde_json::from_str::<EditRequest>(r#"{"type":"swapTracks","a":0,"b":2}"#)
            .expect("swapTracks camelCase");

        match request.into_command().expect("swapTracks command") {
            EditCommand::SwapTracks { a, b } => {
                assert_eq!(a, 0);
                assert_eq!(b, 2);
            }
            other => panic!("expected SwapTracks, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_swap_clips_and_maps_to_command() {
        // camelCase clipA/clipB must deserialize, or the cross-track swap gesture
        // silently fails at the IPC boundary (the recurring DTO camelCase trap).
        let request = serde_json::from_str::<EditRequest>(
            r#"{"type":"swapClips","clipA":"clip-1","clipB":"clip-2"}"#,
        )
        .expect("swapClips camelCase");

        match request.into_command().expect("swapClips command") {
            EditCommand::SwapClips { a, b } => {
                assert_eq!(a, "clip-1");
                assert_eq!(b, "clip-2");
            }
            other => panic!("expected SwapClips, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_upsert_keyframe_scalar_and_maps_to_command() {
        // camelCase clipId/frame must deserialize (the recurring DTO camelCase
        // trap), and the "scalar" kind must map onto KeyframeValue::Scalar.
        let request = serde_json::from_str::<EditRequest>(
            r#"{"type":"upsertKeyframe","clipId":"clip-1","property":"opacity","frame":110,"value":{"kind":"scalar","value":0.25}}"#,
        )
        .expect("upsertKeyframe scalar camelCase");

        match request.into_command().expect("upsertKeyframe command") {
            EditCommand::UpsertKeyframe {
                clip_id,
                property,
                frame,
                value,
            } => {
                assert_eq!(clip_id, "clip-1");
                assert_eq!(property, opentake_ops::KeyframeProperty::Opacity);
                assert_eq!(frame, 110);
                assert!(matches!(value, opentake_ops::KeyframeValue::Scalar(v) if v == 0.25));
            }
            other => panic!("expected UpsertKeyframe, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_upsert_keyframe_pair_and_crop_and_maps_to_command() {
        let pair_request = serde_json::from_str::<EditRequest>(
            r#"{"type":"upsertKeyframe","clipId":"clip-1","property":"position","frame":10,"value":{"kind":"pair","value":{"a":0.3,"b":0.7}}}"#,
        )
        .expect("upsertKeyframe pair camelCase");
        match pair_request
            .into_command()
            .expect("upsertKeyframe pair command")
        {
            EditCommand::UpsertKeyframe {
                property, value, ..
            } => {
                assert_eq!(property, opentake_ops::KeyframeProperty::Position);
                match value {
                    opentake_ops::KeyframeValue::Pair(p) => {
                        assert_eq!(p.a, 0.3);
                        assert_eq!(p.b, 0.7);
                    }
                    other => panic!("expected Pair value, got {other:?}"),
                }
            }
            other => panic!("expected UpsertKeyframe, got {other:?}"),
        }

        let crop_request = serde_json::from_str::<EditRequest>(
            r#"{"type":"upsertKeyframe","clipId":"clip-1","property":"crop","frame":10,"value":{"kind":"crop","value":{"left":0.1,"top":0.2,"right":0.3,"bottom":0.4}}}"#,
        )
        .expect("upsertKeyframe crop camelCase");
        match crop_request
            .into_command()
            .expect("upsertKeyframe crop command")
        {
            EditCommand::UpsertKeyframe {
                property, value, ..
            } => {
                assert_eq!(property, opentake_ops::KeyframeProperty::Crop);
                assert!(matches!(value, opentake_ops::KeyframeValue::Crop(_)));
            }
            other => panic!("expected UpsertKeyframe, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_effect_commands_and_maps_to_ops_variants() {
        let grade = serde_json::from_str::<EditRequest>(
            r#"{"type":"setColorGrade","clipIds":["clip-1"],"grade":{"exposure":1.0}}"#,
        )
        .expect("setColorGrade camelCase");
        match grade.into_command().expect("setColorGrade command") {
            EditCommand::SetColorGrade { clip_ids, grade } => {
                assert_eq!(clip_ids, vec!["clip-1"]);
                assert_eq!(grade.expect("grade").exposure, 1.0);
            }
            other => panic!("expected SetColorGrade, got {other:?}"),
        }

        let chroma = serde_json::from_str::<EditRequest>(
            r#"{"type":"setChromaKey","clipIds":["clip-1"],"chromaKey":{"similarity":0.2}}"#,
        )
        .expect("setChromaKey camelCase");
        assert!(matches!(
            chroma.into_command().expect("setChromaKey command"),
            EditCommand::SetChromaKey { .. }
        ));

        let masks = serde_json::from_str::<EditRequest>(
            r#"{"type":"setMasks","clipIds":["clip-1"],"masks":[]}"#,
        )
        .expect("setMasks camelCase");
        assert!(matches!(
            masks.into_command().expect("setMasks command"),
            EditCommand::SetMasks { .. }
        ));

        let effects = serde_json::from_str::<EditRequest>(
            r#"{"type":"setEffects","clipIds":["clip-1"],"effects":[{"name":"grayscale","params":{"amount":0.4}}]}"#,
        )
        .expect("setEffects camelCase");
        match effects.into_command().expect("setEffects command") {
            EditCommand::SetEffects { effects, .. } => {
                assert_eq!(effects[0].name, "grayscale");
                assert_eq!(effects[0].param("amount", 0.0), 0.4);
            }
            other => panic!("expected SetEffects, got {other:?}"),
        }
    }

    /// Guards the IPC boundary (`AGENTS.md` camelCase discipline): the
    /// multiword `clipIds` field must deserialize on the wire exactly like the
    /// other multi-clip commands (`setColorGrade` et al.).
    #[test]
    fn deserializes_reset_transform_camelcase_and_maps_to_ops_variant() {
        let req = serde_json::from_str::<EditRequest>(
            r#"{"type":"resetTransform","clipIds":["clip-1","clip-2"]}"#,
        )
        .expect("resetTransform camelCase");
        match req.into_command().expect("resetTransform command") {
            EditCommand::ResetTransform { clip_ids } => {
                assert_eq!(clip_ids, vec!["clip-1", "clip-2"]);
            }
            other => panic!("expected ResetTransform, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_media_library_commands_and_maps_to_ops_variants() {
        let rename_media = serde_json::from_str::<EditRequest>(
            r#"{"type":"renameMedia","entries":[{"id":"asset-1","name":"Hero"}]}"#,
        )
        .expect("renameMedia camelCase");
        assert!(matches!(
            rename_media.into_command().expect("renameMedia command"),
            EditCommand::RenameMedia { .. }
        ));

        let rename_folder = serde_json::from_str::<EditRequest>(
            r#"{"type":"renameFolder","entries":[{"id":"folder-1","name":"B-roll"}]}"#,
        )
        .expect("renameFolder camelCase");
        assert!(matches!(
            rename_folder.into_command().expect("renameFolder command"),
            EditCommand::RenameFolder { .. }
        ));

        let delete_media =
            serde_json::from_str::<EditRequest>(r#"{"type":"deleteMedia","assetIds":["asset-1"]}"#)
                .expect("deleteMedia camelCase");
        assert!(matches!(
            delete_media.into_command().expect("deleteMedia command"),
            EditCommand::DeleteMedia { .. }
        ));

        let delete_folder = serde_json::from_str::<EditRequest>(
            r#"{"type":"deleteFolder","folderIds":["folder-1"]}"#,
        )
        .expect("deleteFolder camelCase");
        assert!(matches!(
            delete_folder.into_command().expect("deleteFolder command"),
            EditCommand::DeleteFolder { .. }
        ));
    }
}

#[cfg(test)]
mod generation_log_tests {
    use opentake_core::AppCore;
    use opentake_domain::GenerationJobStatus;
    use opentake_project::{GenerationLog, GenerationLogEntry};
    use serde_json::Value;

    /// The exact wire shape the front end receives: `version` + `entries` on
    /// the log, camelCase on every entry, and `None` fields omitted entirely
    /// (`skip_serializing_if`) so the TS mirror never sees a `null` it did not
    /// declare.
    #[test]
    fn log_serializes_camel_case_entries_and_omits_none_fields() {
        let log = GenerationLog {
            version: 1,
            entries: vec![
                GenerationLogEntry::new("row-1", "veo-3", Some(250), Some(700_000_000.0)),
                GenerationLogEntry::new("row-2", "gpt-4o", None, None),
                GenerationLogEntry::job_event(
                    "row-3",
                    "job-9",
                    "suno-v4",
                    Some(120),
                    "provider-x",
                    Some("provider-job-9".to_string()),
                    "asset-9",
                    GenerationJobStatus::Ready,
                    Some(1.0),
                    None,
                    Some(700_000_100.0),
                    Some("source-asset-8".to_string()),
                    Some("clip-8".to_string()),
                ),
            ],
        };

        let json = serde_json::to_string(&log).expect("serialize");
        let parsed: Value = serde_json::from_str(&json).expect("parse back");
        assert_eq!(parsed["version"], 1, "got: {json}");
        assert_eq!(
            parsed["entries"].as_array().map(Vec::len),
            Some(3),
            "got: {json}"
        );
        let first = &parsed["entries"][0];
        assert_eq!(first["id"], "row-1");
        assert_eq!(first["model"], "veo-3");
        assert_eq!(first["costCredits"], 250);
        assert_eq!(first["createdAt"], 700_000_000.0);
        // `None` fields are absent (not `null`), matching the TS optional keys.
        assert!(first.get("status").is_none(), "got: {json}");
        assert!(
            parsed["entries"][1].get("costCredits").is_none(),
            "got: {json}"
        );
        // A full job row keeps every present field and the lower-case status
        // tag serde derives from the camelCase enum (`Ready` -> `"ready"`).
        let third = &parsed["entries"][2];
        assert_eq!(third["jobId"], "job-9");
        assert_eq!(third["provider"], "provider-x");
        assert_eq!(third["providerJobId"], "provider-job-9");
        assert_eq!(third["status"], "ready");
        assert_eq!(third["sourceAssetId"], "source-asset-8");
        assert_eq!(third["sourceClipId"], "clip-8");
    }

    /// A session with no project must not error: the command is an infallible
    /// read (like `get_timeline`), returning the empty log (`version: 1`, no
    /// entries, zero total credits).
    #[test]
    fn no_project_returns_empty_log() {
        let core = AppCore::new();
        let log = core.generation_log();
        assert_eq!(log.version, 1);
        assert!(log.entries.is_empty());
        assert_eq!(log.total_credits(), 0);

        // The command body is a direct passthrough; a fresh session serializes
        // to the stable empty shape the front end's loading/empty states rely on.
        let json = serde_json::to_string(&log).expect("serialize empty log");
        assert!(json.contains("\"version\":1"), "got: {json}");
        assert!(json.contains("\"entries\":[]"), "got: {json}");
    }

    /// The command is a one-line passthrough to `AppCore::generation_log()`
    /// (mirroring `get_timeline`), so the accessor it exposes is the contract
    /// under test here. Reads are stable snapshots: a fresh session keeps an
    /// empty log across repeated reads, so the UI can poll it freely.
    #[test]
    fn repeated_reads_are_stable_snapshots() {
        let core = AppCore::new();
        let first = core.generation_log();
        let second = core.generation_log();
        assert_eq!(first, second);
        assert!(first.entries.is_empty());
    }
}

#[cfg(test)]
mod subtitle_export_tests {
    use super::{write_subtitles, SubtitleFormat};
    use opentake_domain::{Clip, ClipType, Timeline, Track};

    /// Build a caption clip: text + caption_group_id set, media_type Text — the
    /// two fields `collect_caption_cues` requires to treat a clip as a caption.
    fn caption(id: &str, group: &str, start: i32, dur: i32, text: &str) -> Clip {
        let mut c = Clip::new(id, "caption", start, dur);
        c.media_type = ClipType::Text;
        c.caption_group_id = Some(group.to_string());
        c.text_content = Some(text.to_string());
        c
    }

    /// A timeline with a single caption track holding `clips`, at the given fps.
    fn timeline_with(fps: i32, clips: Vec<Clip>) -> Timeline {
        let mut tl = Timeline::new();
        tl.fps = fps;
        let mut t = Track::new("t-cap", ClipType::Text);
        t.clips = clips;
        tl.tracks.push(t);
        tl
    }

    /// `SubtitleFormat` must deserialize from the lower-case tags the front end
    /// sends (matching the file extension) and default to SRT for bare payloads.
    #[test]
    fn subtitle_format_deserializes_lowercase_tags() {
        assert_eq!(
            serde_json::from_str::<SubtitleFormat>(r#""srt""#).expect("srt"),
            SubtitleFormat::Srt
        );
        assert_eq!(
            serde_json::from_str::<SubtitleFormat>(r#""vtt""#).expect("vtt"),
            SubtitleFormat::Vtt
        );
        assert_eq!(SubtitleFormat::default(), SubtitleFormat::Srt);
    }

    /// The summary returned to the front end must serialize as camelCase
    /// (`outPath` / `cueCount`) so the TS mirror lines up.
    #[test]
    fn summary_serializes_camel_case() {
        let summary = super::SubtitleExportSummary {
            out_path: "/tmp/x.srt".into(),
            cue_count: 2,
        };
        let json = serde_json::to_string(&summary).expect("serialize");
        assert!(json.contains("\"outPath\""), "got: {json}");
        assert!(json.contains("\"cueCount\":2"), "got: {json}");
    }

    /// A timeline carrying caption clips exports a non-empty SRT body with one
    /// numbered cue per caption, and reports the cue count.
    #[test]
    fn exports_non_empty_srt_with_cue_count() {
        let dir = std::env::temp_dir();
        let path = dir
            .join(format!("opentake-subs-{}.srt", std::process::id()))
            .to_string_lossy()
            .into_owned();
        let tl = timeline_with(
            30,
            vec![
                caption("c1", "g1", 30, 30, "Hello"),
                caption("c2", "g1", 60, 30, "World"),
            ],
        );

        let summary =
            write_subtitles(&tl, path.clone(), SubtitleFormat::Srt).expect("srt export ok");
        assert_eq!(summary.cue_count, 2);
        assert_eq!(summary.out_path, path);

        let written = std::fs::read_to_string(&path).expect("read back srt");
        let _ = std::fs::remove_file(&path);
        assert!(written.contains("Hello"));
        assert!(written.contains("World"));
        // SRT uses comma timestamps and 1-based indices.
        assert!(written.starts_with("1\n"), "got: {written:?}");
        assert!(
            written.contains("00:00:01,000 --> 00:00:02,000"),
            "got: {written:?}"
        );
    }

    /// VTT export always opens with the `WEBVTT` header and uses dot timestamps.
    #[test]
    fn exports_vtt_with_header() {
        let dir = std::env::temp_dir();
        let path = dir
            .join(format!("opentake-subs-{}.vtt", std::process::id()))
            .to_string_lossy()
            .into_owned();
        let tl = timeline_with(30, vec![caption("c1", "g1", 30, 30, "Hello")]);

        let summary =
            write_subtitles(&tl, path.clone(), SubtitleFormat::Vtt).expect("vtt export ok");
        assert_eq!(summary.cue_count, 1);

        let written = std::fs::read_to_string(&path).expect("read back vtt");
        let _ = std::fs::remove_file(&path);
        assert!(written.starts_with("WEBVTT\n\n"), "got: {written:?}");
        assert!(
            written.contains("00:00:01.000 --> 00:00:02.000"),
            "got: {written:?}"
        );
    }

    /// A timeline with no caption clips writes a (header-only / empty) file and
    /// reports `cue_count == 0`, the signal the UI uses for its friendly toast.
    #[test]
    fn empty_timeline_reports_zero_cues() {
        let dir = std::env::temp_dir();
        let path = dir
            .join(format!("opentake-subs-empty-{}.srt", std::process::id()))
            .to_string_lossy()
            .into_owned();
        let tl = Timeline::new();

        let summary =
            write_subtitles(&tl, path.clone(), SubtitleFormat::Srt).expect("empty export ok");
        let _ = std::fs::remove_file(&path);
        assert_eq!(summary.cue_count, 0);
    }
}
