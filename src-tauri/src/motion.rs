//! Deterministic desktop motion-graphic renderer and atomic project commit.
//!
//! The first production path materializes a constrained HTML/template scene
//! through the `opentake-motion` headless-Chromium renderer, encodes its exact
//! PNG frame sequence with the bundled FFmpeg, then asks `AppCore` to register
//! and place/replace the video in one durable undo transaction.

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use opentake_agent::mcp::motion::{
    AddMotionRequest, EditMotionRequest, MotionBridge, MotionBridgeError, MotionBridgeErrorKind,
    MotionCommit, MotionOutputMetadata, MotionSourceRequest,
};
use opentake_core::{AppCore, MotionPlacement, ProbedMedia};
use opentake_domain::{GenerationInput, GenerationJobStatus};
use opentake_motion::{
    HeadlessChromiumRenderer, MotionCache, MotionCancellationToken, MotionError,
    MotionRenderRequest, MotionRenderer, MotionSource, RenderedClip, SandboxPolicy,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

const MOTION_PROVIDER: &str = "opentake-motion";
const MOTION_MODEL: &str = "opentake.motion-v1";
const LEGACY_MOTION_MODEL: &str = "opentake.motion-html-v1";

#[derive(Clone)]
pub struct TauriMotionBridge {
    core: AppCore,
    cache_root: std::path::PathBuf,
    progress: Arc<dyn Fn(MotionProgress) + Send + Sync>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MotionProgress {
    Validating,
    Rendering,
    Encoding,
    Committing,
    Complete,
}

#[derive(Clone)]
pub struct MotionCommandState {
    bridge: Arc<TauriMotionBridge>,
    active: Arc<Mutex<Option<opentake_media::MediaCancelToken>>>,
}

impl MotionCommandState {
    pub fn new(bridge: Arc<TauriMotionBridge>) -> Self {
        Self {
            bridge,
            active: Arc::new(Mutex::new(None)),
        }
    }

    fn begin(&self) -> Result<opentake_media::MediaCancelToken, String> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| "motion command state is unavailable".to_string())?;
        if active.is_some() {
            return Err("another motion render is already running".into());
        }
        let cancel = opentake_media::MediaCancelToken::new();
        *active = Some(cancel.clone());
        Ok(cancel)
    }

    fn finish(&self) {
        if let Ok(mut active) = self.active.lock() {
            *active = None;
        }
    }

    fn cancel(&self) -> bool {
        self.active
            .lock()
            .ok()
            .and_then(|active| active.clone())
            .map(|cancel| {
                cancel.cancel();
                true
            })
            .unwrap_or(false)
    }

    pub fn cancel_active(&self) {
        let _ = self.cancel();
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionAddCommand {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub template_id: Option<String>,
    #[serde(default)]
    pub params: Map<String, Value>,
    pub start_frame: i32,
    pub duration_frames: i32,
    #[serde(default)]
    pub transparent: bool,
    #[serde(default)]
    pub track_index: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionEditCommand {
    pub clip_id: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub params: Option<Map<String, Value>>,
}

#[tauri::command]
pub fn motion_capability(state: State<'_, MotionCommandState>) -> bool {
    state.bridge.can_render_motion()
}

#[tauri::command]
pub async fn motion_add(
    app: AppHandle,
    state: State<'_, MotionCommandState>,
    request: MotionAddCommand,
) -> Result<MotionCommit, String> {
    let source = match (request.code, request.template_id) {
        (Some(code), None) => MotionSourceRequest::Code(code),
        (None, Some(template_id)) => MotionSourceRequest::Template {
            template_id,
            params: request.params,
        },
        _ => return Err("provide exactly one of code or templateId".into()),
    };
    let cancel = state.begin()?;
    let bridge = state
        .bridge
        .as_ref()
        .clone()
        .with_progress_callback(Arc::new(move |phase| {
            let _ = app.emit("motion_progress", phase);
        }));
    let worker = tauri::async_runtime::spawn_blocking(move || {
        bridge.add(
            AddMotionRequest {
                source,
                start_frame: request.start_frame,
                duration_frames: request.duration_frames,
                transparent: request.transparent,
                track_index: request.track_index,
            },
            &cancel,
        )
    })
    .await;
    state.finish();
    let result = worker.map_err(|_| "motion render worker failed".to_string())?;
    result.map_err(|error| error.message)
}

#[tauri::command]
pub async fn motion_edit(
    app: AppHandle,
    state: State<'_, MotionCommandState>,
    request: MotionEditCommand,
) -> Result<MotionCommit, String> {
    let cancel = state.begin()?;
    let bridge = state
        .bridge
        .as_ref()
        .clone()
        .with_progress_callback(Arc::new(move |phase| {
            let _ = app.emit("motion_progress", phase);
        }));
    let worker = tauri::async_runtime::spawn_blocking(move || {
        bridge.edit(
            EditMotionRequest {
                clip_id: request.clip_id,
                code: request.code,
                params: request.params,
            },
            &cancel,
        )
    })
    .await;
    state.finish();
    let result = worker.map_err(|_| "motion render worker failed".to_string())?;
    result.map_err(|error| error.message)
}

#[tauri::command]
pub fn motion_cancel(state: State<'_, MotionCommandState>) -> bool {
    state.cancel()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum StoredMotionSource {
    Code {
        code: String,
    },
    Template {
        template_id: String,
        #[serde(default)]
        params: Map<String, Value>,
    },
}

impl TauriMotionBridge {
    pub fn new(core: AppCore, cache_root: impl Into<std::path::PathBuf>) -> Self {
        Self {
            core,
            cache_root: cache_root.into(),
            progress: Arc::new(|_| {}),
        }
    }

    pub fn with_progress_callback(
        mut self,
        progress: Arc<dyn Fn(MotionProgress) + Send + Sync>,
    ) -> Self {
        self.progress = progress;
        self
    }

    fn render_and_encode(
        &self,
        stored_source: &StoredMotionSource,
        duration_frames: i32,
        transparent: bool,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<(tempfile::TempDir, std::path::PathBuf, ProbedMedia, String), MotionBridgeError>
    {
        (self.progress)(MotionProgress::Validating);
        if transparent {
            return Err(MotionBridgeError::new(
                MotionBridgeErrorKind::CapabilityUnavailable,
                "The current MP4 motion path is opaque; transparent motion output is not supported yet.",
            ));
        }
        if cancel.is_cancelled() {
            return Err(MotionBridgeError::new(
                MotionBridgeErrorKind::Cancelled,
                "motion render cancelled",
            ));
        }
        let snapshot = self.core.runtime_snapshot();
        snapshot.project_dir.as_ref().ok_or_else(|| {
            MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                "Save the project before rendering a motion graphic.",
            )
        })?;
        if duration_frames < 1 {
            return Err(MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                "durationFrames must be at least 1",
            ));
        }
        let fps = u32::try_from(snapshot.timeline.fps.max(1)).unwrap_or(30);
        let width = u32::try_from(snapshot.timeline.width.max(2)).map_err(|_| {
            MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                "timeline width is invalid",
            )
        })?;
        let height = u32::try_from(snapshot.timeline.height.max(2)).map_err(|_| {
            MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                "timeline height is invalid",
            )
        })?;
        let frames = u32::try_from(duration_frames).map_err(|_| {
            MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                "durationFrames is invalid",
            )
        })?;
        let html = source_document(stored_source, fps, width, height, frames)?;
        let request =
            MotionRenderRequest::new(MotionSource::code(html), fps, frames, width, height)
                .with_transparent(false);
        request.validate().map_err(map_motion_error)?;

        let render_cancel = MotionCancellationToken::new();
        let monitor_cancel = render_cancel.clone();
        let media_cancel = cancel.clone();
        let done = Arc::new(AtomicBool::new(false));
        let monitor_done = Arc::clone(&done);
        let monitor = std::thread::spawn(move || {
            while !monitor_done.load(Ordering::Acquire) {
                if media_cancel.is_cancelled() {
                    monitor_cancel.cancel();
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        let renderer = HeadlessChromiumRenderer::new(
            MotionCache::new(self.cache_root.join("motion-frames")),
            SandboxPolicy::offline_with_timeout(Duration::from_secs(90)),
        )
        .with_cancellation_token(render_cancel);
        (self.progress)(MotionProgress::Rendering);
        let rendered = renderer.render(&request).map_err(map_motion_error);
        done.store(true, Ordering::Release);
        let _ = monitor.join();
        let rendered = rendered?;

        let output_dir = tempfile::Builder::new()
            .prefix("opentake-motion-")
            .tempdir()
            .map_err(io_motion_error)?;
        let output = output_dir.path().join("output.mp4");
        (self.progress)(MotionProgress::Encoding);
        if let Err(error) = encode_frames(&rendered, &output, cancel) {
            let _ = std::fs::remove_file(&output);
            return Err(error);
        }
        let probe = opentake_media::probe(&output).map_err(|_| {
            MotionBridgeError::new(
                MotionBridgeErrorKind::RenderFailed,
                "rendered motion video failed validation",
            )
        })?;
        if !probe.has_video
            || probe.width != Some(width)
            || probe.height != Some(height)
            || (probe.duration_secs - f64::from(frames) / f64::from(fps)).abs()
                > (1.5 / f64::from(fps))
        {
            let _ = std::fs::remove_file(&output);
            return Err(MotionBridgeError::new(
                MotionBridgeErrorKind::RenderFailed,
                "rendered motion video metadata did not match the request",
            ));
        }
        Ok((
            output_dir,
            output,
            ProbedMedia {
                duration_secs: probe.duration_secs,
                width: probe.width.and_then(|value| i32::try_from(value).ok()),
                height: probe.height.and_then(|value| i32::try_from(value).ok()),
                fps: probe.fps,
                has_audio: probe.has_audio,
                color: probe.color,
            },
            rendered.content_hash,
        ))
    }

    fn commit(
        &self,
        stored_source: StoredMotionSource,
        duration_frames: i32,
        transparent: bool,
        placement: MotionPlacement,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<MotionCommit, MotionBridgeError> {
        let snapshot = self.core.runtime_snapshot();
        let project_dir = snapshot.project_dir.clone().ok_or_else(|| {
            MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                "Save the project before rendering a motion graphic.",
            )
        })?;
        let (_temporary_output, output, probe, content_hash) =
            self.render_and_encode(&stored_source, duration_frames, transparent, cancel)?;
        let motion_canvas = matches!(
            &stored_source,
            StoredMotionSource::Template { template_id, .. } if template_id == "title-card"
        );
        let output_metadata = MotionOutputMetadata {
            renderer: if motion_canvas {
                "motion-canvas".into()
            } else {
                "opentake-html-fallback".into()
            },
            renderer_version: if motion_canvas {
                "3.17.2".into()
            } else {
                env!("CARGO_PKG_VERSION").into()
            },
            output_file: "output.mp4".into(),
            fps: probe
                .fps
                .unwrap_or_else(|| f64::from(snapshot.timeline.fps.max(1))),
            width: u32::try_from(probe.width.unwrap_or(snapshot.timeline.width)).unwrap_or(0),
            height: u32::try_from(probe.height.unwrap_or(snapshot.timeline.height)).unwrap_or(0),
            duration_frames,
            duration_seconds: probe.duration_secs,
            content_hash: content_hash.clone(),
        };
        let result_path = output
            .parent()
            .ok_or_else(|| {
                MotionBridgeError::new(
                    MotionBridgeErrorKind::RenderFailed,
                    "motion result directory is missing",
                )
            })?
            .join("motion-result.json");
        let result_bytes = serde_json::to_vec_pretty(&output_metadata).map_err(|_| {
            MotionBridgeError::new(
                MotionBridgeErrorKind::RenderFailed,
                "motion result metadata could not be encoded",
            )
        })?;
        std::fs::write(&result_path, result_bytes).map_err(io_motion_error)?;
        validate_motion_result(
            &std::fs::read(&result_path).map_err(io_motion_error)?,
            &output_metadata,
        )?;
        let project_media = crate::library::ProjectMediaCapability::open_verified(
            &self.core,
            snapshot.project_epoch,
            &project_dir,
            true,
        )
        .map_err(|error| MotionBridgeError::new(MotionBridgeErrorKind::RenderFailed, error))?;
        let leaf_name = format!("motion-{}.mp4", uuid::Uuid::new_v4());
        let mut published = project_media
            .create_import(std::path::Path::new(&leaf_name))
            .map_err(|error| MotionBridgeError::new(MotionBridgeErrorKind::RenderFailed, error))?;
        let mut encoded = std::fs::File::open(&output).map_err(io_motion_error)?;
        std::io::copy(&mut encoded, published.file_mut()).map_err(io_motion_error)?;
        published.file_mut().flush().map_err(io_motion_error)?;
        published.file().sync_all().map_err(io_motion_error)?;
        if !project_media
            .matches_leaf(&published)
            .map_err(|error| MotionBridgeError::new(MotionBridgeErrorKind::RenderFailed, error))?
        {
            return Err(MotionBridgeError::new(
                MotionBridgeErrorKind::RenderFailed,
                "motion output identity changed before project commit",
            ));
        }
        let source_json = serde_json::to_string(&stored_source).map_err(|_| {
            MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                "motion source could not be persisted",
            )
        })?;
        let provenance = GenerationInput {
            prompt: source_json,
            model: MOTION_MODEL.into(),
            duration: duration_frames,
            aspect_ratio: format!("{}:{}", snapshot.timeline.width, snapshot.timeline.height),
            provider: Some(MOTION_PROVIDER.into()),
            status: Some(GenerationJobStatus::Ready),
            ..GenerationInput::default()
        };
        (self.progress)(MotionProgress::Committing);
        let committed = self.core.commit_motion_media_for_project(
            snapshot.project_epoch,
            snapshot.version,
            &project_dir,
            published.path(),
            "Motion Graphic",
            &probe,
            provenance,
            placement,
        );
        let committed = match committed {
            Ok(committed) => committed,
            Err(error) => {
                return Err(MotionBridgeError::new(
                    MotionBridgeErrorKind::RenderFailed,
                    format!("motion project commit failed: {error}"),
                ));
            }
        };
        published.commit();
        let clip_id = committed
            .edit
            .affected_clip_ids
            .first()
            .cloned()
            .ok_or_else(|| {
                MotionBridgeError::new(
                    MotionBridgeErrorKind::RenderFailed,
                    "motion commit returned no clip id",
                )
            })?;
        (self.progress)(MotionProgress::Complete);
        Ok(MotionCommit {
            clip_id,
            asset_id: committed.media.id,
            content_hash,
            action_name: committed.edit.action_name,
            output: output_metadata,
        })
    }
}

impl MotionBridge for TauriMotionBridge {
    fn can_render_motion(&self) -> bool {
        HeadlessChromiumRenderer::find_browser().is_some()
            && opentake_media::ffmpeg_status::ffmpeg_available()
            && opentake_media::ffmpeg_status::ffprobe_available()
    }

    fn add(
        &self,
        request: AddMotionRequest,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<MotionCommit, MotionBridgeError> {
        if request.start_frame < 0 {
            return Err(MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                "startFrame must be non-negative",
            ));
        }
        let snapshot = self.core.runtime_snapshot();
        if let Some(track_index) = request.track_index {
            let Some(track) = snapshot.timeline.tracks.get(track_index) else {
                return Err(MotionBridgeError::new(
                    MotionBridgeErrorKind::InvalidArguments,
                    "trackIndex is out of range",
                ));
            };
            if track.kind == opentake_domain::ClipType::Audio {
                return Err(MotionBridgeError::new(
                    MotionBridgeErrorKind::InvalidArguments,
                    "motion graphics require a visual track",
                ));
            }
        }
        let stored_source = match request.source {
            MotionSourceRequest::Code(code) => StoredMotionSource::Code { code },
            MotionSourceRequest::Template {
                template_id,
                params,
            } => StoredMotionSource::Template {
                template_id,
                params,
            },
        };
        self.commit(
            stored_source,
            request.duration_frames,
            request.transparent,
            MotionPlacement::Add {
                start_frame: request.start_frame,
                duration_frames: request.duration_frames,
                track_index: request.track_index,
            },
            cancel,
        )
    }

    fn edit(
        &self,
        request: EditMotionRequest,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<MotionCommit, MotionBridgeError> {
        let snapshot = self.core.runtime_snapshot();
        let clip = snapshot
            .timeline
            .tracks
            .iter()
            .flat_map(|track| &track.clips)
            .find(|clip| clip.id == request.clip_id)
            .ok_or_else(|| {
                MotionBridgeError::new(
                    MotionBridgeErrorKind::ResourceNotFound,
                    "motion clip was not found",
                )
            })?;
        let entry = snapshot
            .media
            .entries
            .iter()
            .find(|entry| entry.id == clip.media_ref)
            .ok_or_else(|| {
                MotionBridgeError::new(
                    MotionBridgeErrorKind::ResourceNotFound,
                    "motion media was not found",
                )
            })?;
        let provenance = entry.generation_input.as_ref().filter(|input| {
            input.provider.as_deref() == Some(MOTION_PROVIDER)
                && matches!(input.model.as_str(), MOTION_MODEL | LEGACY_MOTION_MODEL)
        });
        let provenance = provenance.ok_or_else(|| {
            MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                "the selected clip is not an OpenTake motion graphic",
            )
        })?;
        let mut source: StoredMotionSource =
            serde_json::from_str(&provenance.prompt).map_err(|_| {
                MotionBridgeError::new(
                    MotionBridgeErrorKind::RenderFailed,
                    "stored motion source is invalid",
                )
            })?;
        match (&mut source, request.code, request.params) {
            (StoredMotionSource::Code { code }, Some(updated), None) => *code = updated,
            (StoredMotionSource::Template { params, .. }, None, Some(updated)) => {
                params.extend(updated)
            }
            (StoredMotionSource::Code { .. }, None, Some(_)) => {
                return Err(MotionBridgeError::new(
                    MotionBridgeErrorKind::InvalidArguments,
                    "params can only edit a template-authored motion graphic",
                ));
            }
            (StoredMotionSource::Template { .. }, Some(_), _) => {
                return Err(MotionBridgeError::new(
                    MotionBridgeErrorKind::InvalidArguments,
                    "code cannot replace a template-authored motion graphic",
                ));
            }
            (_, None, None) => {
                return Err(MotionBridgeError::new(
                    MotionBridgeErrorKind::InvalidArguments,
                    "edit requires code or params",
                ));
            }
            (StoredMotionSource::Code { .. }, Some(_), Some(_)) => {
                return Err(MotionBridgeError::new(
                    MotionBridgeErrorKind::InvalidArguments,
                    "code-authored motion edits do not accept template params",
                ));
            }
        }
        self.commit(
            source,
            clip.duration_frames,
            false,
            MotionPlacement::Replace {
                clip_id: request.clip_id,
            },
            cancel,
        )
    }
}

fn source_document(
    source: &StoredMotionSource,
    fps: u32,
    width: u32,
    height: u32,
    frames: u32,
) -> Result<String, MotionBridgeError> {
    match source {
        StoredMotionSource::Code { code } => {
            let trimmed = code.trim();
            if trimmed.is_empty() {
                return Err(MotionBridgeError::new(
                    MotionBridgeErrorKind::InvalidArguments,
                    "motion code must not be empty",
                ));
            }
            if !trimmed.contains('<') {
                return Err(MotionBridgeError::new(
                    MotionBridgeErrorKind::CapabilityUnavailable,
                    "This Beta accepts self-contained HTML/CSS/JS motion code; TS/TSX compilation is not available in the packaged renderer.",
                ));
            }
            Ok(trimmed.to_owned())
        }
        StoredMotionSource::Template {
            template_id,
            params,
        } => template_document(template_id, params, fps, width, height, frames),
    }
}

fn template_document(
    template_id: &str,
    params: &Map<String, Value>,
    fps: u32,
    width: u32,
    height: u32,
    frames: u32,
) -> Result<String, MotionBridgeError> {
    if !matches!(template_id, "title-card" | "lower-third.glass") {
        return Err(MotionBridgeError::new(
            MotionBridgeErrorKind::InvalidArguments,
            format!("unknown motion template: {template_id}"),
        ));
    }
    let string_param = |name: &str, default: &str| -> Result<String, MotionBridgeError> {
        match params.get(name) {
            None => Ok(default.to_owned()),
            Some(Value::String(value)) => Ok(value.clone()),
            Some(_) => Err(MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                format!("template parameter {name} must be a string"),
            )),
        }
    };
    for key in params.keys() {
        if !matches!(key.as_str(), "title" | "subtitle" | "accent" | "background") {
            return Err(MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                format!("unknown template parameter: {key}"),
            ));
        }
    }
    let title = js_string(&string_param("title", "OpenTake")?);
    let subtitle = js_string(&string_param("subtitle", "Motion Graphic")?);
    let accent = js_string(&string_param("accent", "#7C5CFF")?);
    let background = js_string(&string_param("background", "#11131A")?);
    if template_id == "title-card" {
        let config = serde_json::json!({
            "templateId": "title-card",
            "params": {
                "title": string_param("title", "OpenTake")?,
                "subtitle": string_param("subtitle", "Motion Canvas")?,
                "accent": string_param("accent", "#7C5CFF")?,
                "background": string_param("background", "#11131A")?,
            },
            "durationSeconds": f64::from(frames) / f64::from(fps),
            "durationFrames": frames,
            "fps": fps,
            "width": width,
            "height": height,
        });
        let config = safe_inline_json(&config)?;
        let runner = include_str!("../../plugins/motion-canvas-studio/bundle/runner.html");
        if runner.matches("__OPENTAKE_MOTION_CONFIG_JSON__").count() != 1 {
            return Err(MotionBridgeError::new(
                MotionBridgeErrorKind::RenderFailed,
                "embedded Motion Canvas runner has an invalid configuration boundary",
            ));
        }
        return Ok(runner.replacen("__OPENTAKE_MOTION_CONFIG_JSON__", &config, 1));
    }
    let lower_third = template_id == "lower-third.glass";
    Ok(format!(
        r#"<!doctype html><html><head><style>
html,body{{margin:0;width:100%;height:100%;overflow:hidden;background:transparent;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif}}
#stage{{position:absolute;inset:0;background:var(--bg);display:flex;align-items:{align};justify-content:center}}
#card{{position:relative;min-width:42%;max-width:78%;padding:3.5% 4.5%;border-radius:28px;background:rgba(255,255,255,.12);border:1px solid rgba(255,255,255,.28);box-shadow:0 20px 80px rgba(0,0,0,.35);color:white;transform-origin:center}}
#bar{{height:8px;width:28%;background:var(--accent);border-radius:999px;margin-bottom:24px}}
#title{{font-size:clamp(38px,6vw,96px);font-weight:760;letter-spacing:-.035em;line-height:1.02}}
#subtitle{{font-size:clamp(20px,2.4vw,42px);opacity:.76;margin-top:18px;line-height:1.2}}
</style></head><body><div id="stage"><div id="card"><div id="bar"></div><div id="title"></div><div id="subtitle"></div></div></div>
<script>const title={title},subtitle={subtitle},accent={accent},background={background};
document.documentElement.style.setProperty('--accent',accent);document.documentElement.style.setProperty('--bg',background);
document.getElementById('title').textContent=title;document.getElementById('subtitle').textContent=subtitle;
OpenTake.onSeek((t)=>{{const p=Math.max(0,Math.min(1,t*2.4));const eased=1-Math.pow(1-p,3);card.style.opacity=String(p);card.style.transform=`translateY(${{(1-eased)*64}}px) scale(${{.96+.04*eased}})`;bar.style.width=`${{28+Math.sin(t*2.2)*3}}%`;}});</script></body></html>"#,
        align = if lower_third { "flex-end" } else { "center" },
    ))
}

fn safe_inline_json(value: &Value) -> Result<String, MotionBridgeError> {
    serde_json::to_string(value)
        .map(|json| {
            json.replace('<', "\\u003c")
                .replace('>', "\\u003e")
                .replace('&', "\\u0026")
                .replace('\u{2028}', "\\u2028")
                .replace('\u{2029}', "\\u2029")
        })
        .map_err(|_| {
            MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                "motion template parameters could not be encoded",
            )
        })
}

fn validate_motion_result(
    bytes: &[u8],
    expected: &MotionOutputMetadata,
) -> Result<(), MotionBridgeError> {
    let result: MotionOutputMetadata = serde_json::from_slice(bytes).map_err(|_| {
        MotionBridgeError::new(
            MotionBridgeErrorKind::RenderFailed,
            "motion-result.json is malformed",
        )
    })?;
    if result != *expected {
        return Err(MotionBridgeError::new(
            MotionBridgeErrorKind::RenderFailed,
            "motion-result.json did not match the validated output",
        ));
    }
    Ok(())
}

fn js_string(value: &str) -> String {
    serde_json::to_string(value)
        .unwrap_or_else(|_| "\"\"".into())
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
}

fn encode_frames(
    rendered: &RenderedClip,
    output: &std::path::Path,
    cancel: &opentake_media::MediaCancelToken,
) -> Result<(), MotionBridgeError> {
    let pattern = rendered
        .frames
        .first()
        .and_then(|path| path.parent())
        .ok_or_else(|| {
            MotionBridgeError::new(
                MotionBridgeErrorKind::RenderFailed,
                "motion renderer returned no frames",
            )
        })?
        .join("frame_%05d.png");
    let mut child = Command::new(opentake_media::ffmpeg_status::ffmpeg_path())
        .args(["-v", "error", "-nostdin", "-framerate"])
        .arg(rendered.fps.to_string())
        .arg("-i")
        .arg(pattern)
        .args(["-frames:v", &rendered.frames.len().to_string()])
        .args([
            "-an",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
            "-y",
        ])
        .arg(output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            MotionBridgeError::new(
                MotionBridgeErrorKind::CapabilityUnavailable,
                "FFmpeg could not be started for motion encoding",
            )
        })?;
    loop {
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(MotionBridgeError::new(
                MotionBridgeErrorKind::Cancelled,
                "motion render cancelled",
            ));
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(_)) => {
                return Err(MotionBridgeError::new(
                    MotionBridgeErrorKind::RenderFailed,
                    "FFmpeg failed to encode the motion frame sequence",
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(MotionBridgeError::new(
                    MotionBridgeErrorKind::RenderFailed,
                    "motion encoder status could not be read",
                ));
            }
        }
    }
}

fn map_motion_error(error: MotionError) -> MotionBridgeError {
    let kind = match error {
        MotionError::InvalidSource(_)
        | MotionError::InvalidRequest(_)
        | MotionError::UnknownTemplate(_)
        | MotionError::Manifest(_)
        | MotionError::Sandbox(_) => MotionBridgeErrorKind::InvalidArguments,
        MotionError::RendererUnavailable(_) => MotionBridgeErrorKind::CapabilityUnavailable,
        MotionError::Cancelled => MotionBridgeErrorKind::Cancelled,
        MotionError::Timeout(_) | MotionError::RenderFailed(_) | MotionError::Io(_) => {
            MotionBridgeErrorKind::RenderFailed
        }
    };
    MotionBridgeError::new(kind, error.to_string())
}

fn io_motion_error(error: std::io::Error) -> MotionBridgeError {
    MotionBridgeError::new(
        MotionBridgeErrorKind::RenderFailed,
        format!("motion output preparation failed: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output_metadata() -> MotionOutputMetadata {
        MotionOutputMetadata {
            renderer: "motion-canvas".into(),
            renderer_version: "3.17.2".into(),
            output_file: "output.mp4".into(),
            fps: 30.0,
            width: 1920,
            height: 1080,
            duration_frames: 90,
            duration_seconds: 3.0,
            content_hash: "ab".repeat(32),
        }
    }

    #[test]
    fn motion_result_rejects_malformed_or_mismatched_metadata() {
        let expected = output_metadata();
        assert_eq!(
            validate_motion_result(b"not-json", &expected)
                .unwrap_err()
                .message,
            "motion-result.json is malformed"
        );
        let mut mismatched = expected.clone();
        mismatched.output_file = "../output.mp4".into();
        assert_eq!(
            validate_motion_result(&serde_json::to_vec(&mismatched).unwrap(), &expected)
                .unwrap_err()
                .message,
            "motion-result.json did not match the validated output"
        );
        validate_motion_result(&serde_json::to_vec(&expected).unwrap(), &expected).unwrap();
    }
}
