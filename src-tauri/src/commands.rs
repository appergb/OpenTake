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
    handle_edit_apply, handle_get_timeline, handle_project_new, handle_redo, handle_undo,
    EditResultDto, TimelineSnapshotDto,
};
use opentake_core::{AppCore, CmdError, EditCommand};

use opentake_ops::{
    CaptionEntry, ClipEntry, ClipMove, ClipProperties, FrameRange, KeyframePayload,
    KeyframeProperty, KeyframeValue, RenameEntry, TextAutoTrackEntry, TextEntry,
};

use opentake_domain::{
    AnimPair, ChromaKey, ClipType, ColorGrade, Crop, Effect, Interpolation, Keyframe,
    KeyframeTrack, LutReference, Mask, StabilizationTrack, TextStyle, Transform, TransitionKind,
};

#[derive(Clone, Default)]
pub(crate) struct ProjectLifecycleCoordinator {
    gate: std::sync::Arc<tokio::sync::Mutex<()>>,
}

impl ProjectLifecycleCoordinator {
    fn try_acquire(&self) -> Result<tokio::sync::OwnedMutexGuard<()>, String> {
        self.gate
            .clone()
            .try_lock_owned()
            .map_err(|_| "another project lifecycle transition is already in progress".to_string())
    }
}

// MARK: - Read / lifecycle commands (direct DTO passthrough)

/// `get_timeline`: current read-only mirror + version. Infallible.
#[tauri::command]
pub fn get_timeline(core: State<'_, AppCore>) -> TimelineSnapshotDto {
    handle_get_timeline(&core)
}

/// `undo` / `redo`: global history navigation.
#[tauri::command]
pub fn undo(core: State<'_, AppCore>) -> Result<EditResultDto, CmdError> {
    handle_undo(&core)
}

#[tauri::command]
pub fn redo(core: State<'_, AppCore>) -> Result<EditResultDto, CmdError> {
    handle_redo(&core)
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
    let _lifecycle = app
        .state::<ProjectLifecycleCoordinator>()
        .try_acquire()
        .map_err(crate::playback::session::PlaybackCommandError::busy)?;
    if let Some(path) = path {
        app.state::<crate::playback::PlaybackState>()
            .ensure_project_transition_available()?;
        let prepared = prepare_saved_project_off_thread(std::path::PathBuf::from(path))
            .await
            .map_err(crate::playback::session::PlaybackCommandError::engine)?;
        return commit_prepared_project_open_with_playback_and_prewarm(
            &app.state::<AppCore>(),
            prepared,
            &app.state::<crate::playback::PlaybackState>(),
            &app.state::<crate::media::prewarm::PrewarmScheduler>(),
        );
    }
    project_new_with_playback_and_prewarm(
        &app.state::<AppCore>(),
        &app.state::<crate::playback::PlaybackState>(),
        &app.state::<crate::media::prewarm::PrewarmScheduler>(),
    )
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
    let _lifecycle = app.state::<ProjectLifecycleCoordinator>().try_acquire()?;
    if let Some(path) = path {
        let prepared = prepare_saved_project_off_thread(std::path::PathBuf::from(path)).await?;
        let core = app.state::<AppCore>();
        let prewarm = app.state::<crate::media::prewarm::PrewarmScheduler>();
        prewarm.begin_project_transition()?;
        let snapshot = TimelineSnapshotDto::from(core.commit_project_open(prepared));
        prewarm.activate_project(snapshot.project_epoch);
        return Ok(snapshot);
    }
    let core = app.state::<AppCore>();
    let prewarm = app.state::<crate::media::prewarm::PrewarmScheduler>();
    prewarm.begin_project_transition()?;
    let snapshot = handle_project_new(&core);
    prewarm.activate_project(snapshot.project_epoch);
    Ok(snapshot)
}

/// `project_open`: open a `.opentake` bundle, returning the first snapshot.
const PROJECT_LIFECYCLE_PREPARE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

async fn run_blocking_with_timeout<T, F>(
    operation: &'static str,
    timeout: std::time::Duration,
    build: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let task = tokio::task::spawn_blocking(build);
    match tokio::time::timeout(timeout, task).await {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => Err(format!("{operation} task failed: {error}")),
        Err(_) => Err(format!("{operation} timed out after {timeout:?}")),
    }
}

async fn prepare_project_open_off_thread(
    path: std::path::PathBuf,
) -> Result<PreparedProjectOpen, String> {
    run_blocking_with_timeout(
        "project open",
        PROJECT_LIFECYCLE_PREPARE_TIMEOUT,
        move || AppCore::prepare_project_open(path).map_err(|error| error.to_string()),
    )
    .await
}

async fn prepare_saved_project_off_thread(
    path: std::path::PathBuf,
) -> Result<PreparedProjectOpen, String> {
    run_blocking_with_timeout(
        "project create",
        PROJECT_LIFECYCLE_PREPARE_TIMEOUT,
        move || {
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
    let _lifecycle = app
        .state::<ProjectLifecycleCoordinator>()
        .try_acquire()
        .map_err(crate::playback::session::PlaybackCommandError::busy)?;
    // Fail fast if another project transition is already active, but never hold
    // a managed-state guard across the blocking filesystem prepare.
    app.state::<crate::playback::PlaybackState>()
        .ensure_project_transition_available()?;
    let prepared = prepare_project_open_off_thread(std::path::PathBuf::from(path))
        .await
        .map_err(crate::playback::session::PlaybackCommandError::engine)?;
    commit_prepared_project_open_with_playback_and_prewarm(
        &app.state::<AppCore>(),
        prepared,
        &app.state::<crate::playback::PlaybackState>(),
        &app.state::<crate::media::prewarm::PrewarmScheduler>(),
    )
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
    let _lifecycle = app.state::<ProjectLifecycleCoordinator>().try_acquire()?;
    let prepared = prepare_project_open_off_thread(std::path::PathBuf::from(path)).await?;
    let core = app.state::<AppCore>();
    let prewarm = app.state::<crate::media::prewarm::PrewarmScheduler>();
    prewarm.begin_project_transition()?;
    let snapshot = TimelineSnapshotDto::from(core.commit_project_open(prepared));
    prewarm.activate_project(snapshot.project_epoch);
    Ok(snapshot)
}

/// `project_save`: `path = None` saves back to the open bundle; `Some` is save-as.
///
/// Before delegating to the core save, capture a cover `thumbnail.jpg` from the
/// timeline's first video/image clip and hand the JPEG bytes in, so the bundle
/// write persists it (upstream `captureThumbnail` → `snapshotThumbnail`,
/// `Project/VideoProject.swift:92,261-300`). We deliberately use the **same
/// source upstream does** — one representative clip frame via ffmpeg
/// (`opentake_media::capture_project_thumbnail`) — rather than a GPU composite:
/// upstream never composites for the cover, and the save runs under the session
/// lock while `composite_frame` takes the GPU lock, so compositing here would
/// risk lock reentrancy for no fidelity gain. Capture is best-effort: a failure
/// yields `None`, leaving any existing cover untouched, and never fails the save.
#[tauri::command]
pub fn project_save(core: State<'_, AppCore>, path: Option<String>) -> Result<String, CmdError> {
    let snapshot = core.runtime_snapshot();
    let thumbnail = opentake_media::capture_project_thumbnail(
        &snapshot.timeline,
        &snapshot.media,
        snapshot.project_dir.as_deref(),
    );

    let target = path.map(std::path::PathBuf::from);
    core.save_project_with_thumbnail(target, thumbnail)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(CmdError::from)
}

/// `get_default_project_dir`: the default folder new projects save into
/// (`~/Documents/OpenTake`, created on first use). Mirrors upstream
/// `Project.storageDirectory` (`~/Documents/Palmier Pro`). The front end uses it
/// as the save dialog's `defaultPath` so the user picks a location + name like
/// upstream `createNewProject` (`NSSavePanel`).
#[tauri::command]
pub fn get_default_project_dir(app: AppHandle) -> Result<String, String> {
    let dir = app
        .path()
        .document_dir()
        .map_err(|e| e.to_string())?
        .join("OpenTake");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().into_owned())
}

/// `export_xmeml`: write the current timeline to `path` as XMEML 4 (Final Cut
/// Pro 7 XML, `.xml`). This is the Premiere / DaVinci / 剪映-importable
/// interchange format — Premiere Pro does NOT read modern FCPXML natively, so
/// upstream (and OpenTake) emit XMEML; DaVinci/FCP still import FCP7 XML. Reads
/// the timeline / media manifest / project dir from the core, builds the XML via
/// the pure `export_xmeml`, and writes the file.
#[tauri::command]
pub fn export_xmeml(core: State<'_, AppCore>, path: String) -> Result<(), String> {
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
pub fn export_fcpxml(core: State<'_, AppCore>, path: String) -> Result<(), String> {
    export_xmeml(core, path)
}

/// `export_edl`: write the current timeline to `path` as a CMX3600 EDL (`.edl`).
/// A flat, video-track-only edit decision list (the EDL format itself only
/// describes one V track + linked audio channels) that Premiere / DaVinci /
/// Avid / 剪映 import. Effects, transforms, opacity, and multi-track layering are
/// dropped — see `opentake_project::edl` for the documented limitations.
#[tauri::command]
pub fn export_edl(core: State<'_, AppCore>, path: String) -> Result<(), String> {
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
pub fn export_otio(core: State<'_, AppCore>, path: String) -> Result<(), String> {
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
pub fn export_fcpxml_modern(core: State<'_, AppCore>, path: String) -> Result<(), String> {
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
    path: String,
    format: SubtitleFormat,
) -> Result<SubtitleExportSummary, String> {
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

/// `edit_apply`: the unified editing command. The front end constructs an
/// [`EditRequest`] from a UI gesture; this maps it to an [`EditCommand`] and
/// routes it through [`AppCore::apply`] (which performs the snapshot/commit/
/// version transaction and emits `TimelineChanged`).
#[tauri::command]
pub fn edit_apply(
    core: State<'_, AppCore>,
    render: State<'_, crate::render::RenderState>,
    media: State<'_, crate::media::MediaState>,
    command: EditRequest,
) -> Result<EditResultDto, CmdError> {
    let cmd = match command {
        EditRequest::FreezeFrame {
            clip_id,
            at_frame,
            duration_frames,
        } => {
            validate_freeze_frame_request(&core, &clip_id, at_frame, duration_frames)
                .map_err(validation_error)?;
            let media_ref =
                crate::render::capture_freeze_frame(&core, &render, &media, &clip_id, at_frame)
                    .map_err(|error| {
                        eprintln!("freeze-frame capture failed: {error}");
                        internal_error("Freeze-frame capture failed")
                    })?;
            EditCommand::FreezeFrame {
                clip_id,
                at_frame,
                duration_frames,
                media_ref,
            }
        }
        other => other.into_command().map_err(validation_error)?,
    };
    handle_edit_apply(&core, cmd)
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
    RemoveClips { clip_ids: Vec<String> },
    #[serde(rename_all = "camelCase")]
    SplitClip { clip_id: String, at_frame: i32 },
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
            EditRequest::RemoveClips { clip_ids } => EditCommand::RemoveClips { clip_ids },
            EditRequest::SplitClip { clip_id, at_frame } => {
                EditCommand::SplitClip { clip_id, at_frame }
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
        prepare_saved_project_off_thread, run_blocking_with_timeout, ProjectLifecycleCoordinator,
    };
    use opentake_core::core::PreparedProjectOpen;
    use opentake_core::AppCore;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn blocking_project_prepare_runs_off_the_async_caller_thread() {
        let caller = std::thread::current().id();

        let worker = run_blocking_with_timeout("test prepare", Duration::from_secs(1), || {
            Ok(std::thread::current().id())
        })
        .await
        .expect("blocking task completes");

        assert_ne!(worker, caller);
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn timed_out_prepare_cannot_commit_a_late_project() {
        let fixture = tempfile::tempdir().expect("fixture tempdir");
        let bundle = fixture.path().join("slow.opentake");
        AppCore::new()
            .save_project(Some(bundle.clone()))
            .expect("save project fixture");
        let core = AppCore::new();
        let before = core.project_revision();

        let result: Result<PreparedProjectOpen, String> =
            run_blocking_with_timeout("project open", Duration::from_millis(10), move || {
                std::thread::sleep(Duration::from_millis(75));
                AppCore::prepare_project_open(bundle).map_err(|error| error.to_string())
            })
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

        let prepared = prepare_saved_project_off_thread(bundle.clone())
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

        let error = match prepare_saved_project_off_thread(bundle).await {
            Err(error) => error,
            Ok(_) => panic!("invalid destination must fail"),
        };

        assert!(!error.is_empty());
        assert_eq!(core.project_revision(), before);
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
        let metrics = crate::playback::ManifestMetrics { sizes };
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
    use super::{validate_freeze_frame_request, EditRequest};
    use opentake_core::{AppCore, EditCommand};
    use opentake_domain::{ClipType, TransitionKind};
    use opentake_ops::ClipEntry;

    fn request_route(request: &EditRequest) -> &'static str {
        match request {
            EditRequest::CreateNestedSequence { .. } => "CreateNestedSequence",
            EditRequest::EditNestedSequence { .. } => "EditNestedSequence",
            EditRequest::RenameNestedSequence { .. } => "RenameNestedSequence",
            EditRequest::DissolveNestedSequence { .. } => "DissolveNestedSequence",
            EditRequest::AddClips { .. } => "AddClips",
            EditRequest::InsertClips { .. } => "InsertClips",
            EditRequest::MoveClips { .. } => "MoveClips",
            EditRequest::DuplicateClips { .. } => "DuplicateClips",
            EditRequest::RemoveClips { .. } => "RemoveClips",
            EditRequest::SplitClip { .. } => "SplitClip",
            EditRequest::FreezeFrame { .. } => "FreezeFrame",
            EditRequest::TrimClips { .. } => "TrimClips",
            EditRequest::SetClipProperties { .. } => "SetClipProperties",
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
                | ("AddClips", EditCommand::AddClips { .. })
                | ("InsertClips", EditCommand::InsertClips { .. })
                | ("MoveClips", EditCommand::MoveClips { .. })
                | ("DuplicateClips", EditCommand::DuplicateClips { .. })
                | ("RemoveClips", EditCommand::RemoveClips { .. })
                | ("SplitClip", EditCommand::SplitClip { .. })
                | ("TrimClips", EditCommand::TrimClips { .. })
                | ("SetClipProperties", EditCommand::SetClipProperties { .. })
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
            (r#"{"type":"removeClips","clipIds":[]}"#, "RemoveClips"),
            (
                r#"{"type":"splitClip","clipId":"c","atFrame":1}"#,
                "SplitClip",
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

        assert_eq!(cases.len(), 49);
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
            r#"{"type":"insertClips","trackIndex":0,"atFrame":0,"entries":[]}"#,
        )
        .expect("insertClips camelCase");
        serde_json::from_str::<EditRequest>(r#"{"type":"rippleDeleteClips","clipIds":["a"]}"#)
            .expect("rippleDeleteClips camelCase");
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
