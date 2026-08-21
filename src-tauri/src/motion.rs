//! Deterministic desktop motion-graphic renderer and atomic project commit.
//!
//! The first production path materializes a constrained HTML/template scene
//! through the `opentake-motion` headless-Chromium renderer, encodes its exact
//! PNG frame sequence with the bundled FFmpeg, then asks `AppCore` to register
//! and place/replace the video in one durable undo transaction.

use std::collections::BTreeMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use opentake_agent::mcp::motion::{
    AddMotionRequest, EditMotionRequest, MotionBridge, MotionBridgeError, MotionBridgeErrorKind,
    MotionCommit, MotionDocumentReference, MotionOutputMetadata, MotionSourceRequest,
};
use opentake_core::{
    AppCore, DeferredCoreEvents, MotionPlacement, ProbedMedia, ProjectAssetAuthority,
};
use opentake_domain::{GenerationInput, GenerationJobStatus};
use opentake_motion::{
    limits, read_single_preview_png, HeadlessChromiumRenderer, MotionCache,
    MotionCancellationToken, MotionDocumentSource, MotionError, MotionRenderRequest, MotionSource,
    MotionSourceDiagnostic, RenderedClip, SandboxPolicy,
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
    renderer: HeadlessChromiumRenderer,
    progress: Arc<dyn Fn(MotionProgress) + Send + Sync>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum MotionProgress {
    Validating,
    Rendering {
        #[serde(rename = "doneFrames")]
        done_frames: u32,
        #[serde(rename = "totalFrames")]
        total_frames: u32,
    },
    Encoding,
    Committing,
    Complete,
}

#[derive(Clone)]
pub struct MotionCommandState {
    bridge: Arc<TauriMotionBridge>,
    operations: Arc<Mutex<MotionOperations>>,
    admission: crate::updater::InstallAdmissionGate,
}

#[derive(Default)]
struct MotionOperations {
    active_render: Option<ActiveMotionCommand>,
    next_preview_generation: u64,
    active_previews: BTreeMap<u64, ActiveMotionPreview>,
}

struct ActiveMotionCommand {
    cancel: opentake_media::MediaCancelToken,
    _admission: crate::updater::ActivityLease,
}

struct ActiveMotionPreview {
    cancel: MotionCancellationToken,
    _admission: crate::updater::ActivityLease,
}

impl MotionCommandState {
    pub(crate) fn new(
        bridge: Arc<TauriMotionBridge>,
        admission: crate::updater::InstallAdmissionGate,
    ) -> Self {
        Self {
            bridge,
            operations: Arc::new(Mutex::new(MotionOperations::default())),
            admission,
        }
    }

    fn begin(&self) -> Result<opentake_media::MediaCancelToken, String> {
        let admission = self.admission.begin_activity()?;
        let mut operations = self
            .operations
            .lock()
            .map_err(|_| "motion command state is unavailable".to_string())?;
        if operations.active_render.is_some() || !operations.active_previews.is_empty() {
            return Err("another motion render is already running".into());
        }
        let cancel = opentake_media::MediaCancelToken::new();
        operations.active_render = Some(ActiveMotionCommand {
            cancel: cancel.clone(),
            _admission: admission,
        });
        Ok(cancel)
    }

    fn finish(&self) {
        if let Ok(mut operations) = self.operations.lock() {
            operations.active_render = None;
        }
    }

    fn begin_preview(&self) -> Result<(u64, MotionCancellationToken), String> {
        let admission = self.admission.begin_activity()?;
        let mut operations = self
            .operations
            .lock()
            .map_err(|_| "motion command state is unavailable".to_string())?;
        if operations.active_render.is_some() {
            return Err("another motion render is already running".into());
        }
        for active in operations.active_previews.values() {
            active.cancel.cancel();
        }
        operations.next_preview_generation = operations.next_preview_generation.wrapping_add(1);
        if operations.next_preview_generation == 0 {
            operations.next_preview_generation = 1;
        }
        let generation = operations.next_preview_generation;
        let cancel = MotionCancellationToken::new();
        operations.active_previews.insert(
            generation,
            ActiveMotionPreview {
                cancel: cancel.clone(),
                _admission: admission,
            },
        );
        Ok((generation, cancel))
    }

    fn finish_preview(&self, generation: u64) {
        if let Ok(mut operations) = self.operations.lock() {
            operations.active_previews.remove(&generation);
        }
    }

    fn cancel(&self) -> bool {
        let Ok(operations) = self.operations.lock() else {
            return false;
        };
        let mut cancelled = false;
        if let Some(active) = &operations.active_render {
            active.cancel.cancel();
            cancelled = true;
        }
        for active in operations.active_previews.values() {
            active.cancel.cancel();
            cancelled = true;
        }
        cancelled
    }

    fn cancel_previews(&self) -> bool {
        let Ok(operations) = self.operations.lock() else {
            return false;
        };
        let cancelled = !operations.active_previews.is_empty();
        for active in operations.active_previews.values() {
            active.cancel.cancel();
        }
        cancelled
    }

    pub fn has_active(&self) -> bool {
        self.operations
            .lock()
            .map(|operations| {
                operations.active_render.is_some() || !operations.active_previews.is_empty()
            })
            .unwrap_or(true)
    }

    pub fn cancel_active(&self) -> bool {
        self.cancel()
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
    pub document_id: Option<String>,
    #[serde(default)]
    pub revision_hash: Option<String>,
    #[serde(default)]
    pub params: Map<String, Value>,
    pub start_frame: i32,
    pub duration_frames: i32,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub fps: Option<u32>,
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
    #[serde(default)]
    pub document_id: Option<String>,
    #[serde(default)]
    pub revision_hash: Option<String>,
    #[serde(default)]
    pub duration_frames: Option<i32>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub fps: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentMotionSource {
    pub document_id: String,
    pub revision_hash: String,
    pub html: String,
    pub css: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentMotionAddRequest {
    pub source: DocumentMotionSource,
    pub project_authority: ProjectAssetAuthority,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub start_frame: i32,
    pub duration_frames: i32,
    pub track_index: Option<usize>,
    pub transparent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentMotionEditRequest {
    pub clip_id: String,
    pub source: DocumentMotionSource,
    pub project_authority: ProjectAssetAuthority,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration_frames: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MotionPreviewRequest {
    pub document_id: String,
    pub revision_hash: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration_frames: u32,
    pub frame: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionPreviewDiagnostic {
    pub severity: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionPreviewResponse {
    pub revision_hash: String,
    pub frame: u32,
    pub png_data_url: String,
    pub diagnostics: Vec<MotionPreviewDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionPreviewError {
    pub message: String,
    pub diagnostics: Vec<MotionPreviewDiagnostic>,
}

#[tauri::command]
pub fn motion_capability(state: State<'_, MotionCommandState>) -> bool {
    state.bridge.can_render_motion()
}

#[tauri::command]
pub async fn motion_preview(
    state: State<'_, MotionCommandState>,
    documents: State<'_, Arc<crate::motion_documents::MotionDocumentStore>>,
    request: MotionPreviewRequest,
) -> Result<MotionPreviewResponse, MotionPreviewError> {
    let authority = documents.capture_authority().map_err(|_| {
        preview_error("Save the project before previewing a Motion Studio document.")
    })?;
    let (generation, cancellation) = state
        .begin_preview()
        .map_err(|message| preview_error(&message))?;
    let bridge = Arc::clone(&state.bridge);
    let documents = Arc::clone(documents.inner());
    let worker = tauri::async_runtime::spawn_blocking(move || {
        render_document_preview(&bridge, &documents, authority, request, &cancellation)
    })
    .await;
    state.finish_preview(generation);
    worker.map_err(|_| preview_error("Motion preview worker failed."))?
}

#[tauri::command]
pub fn motion_preview_cancel(state: State<'_, MotionCommandState>) -> bool {
    state.cancel_previews()
}

#[tauri::command]
pub async fn motion_add(
    app: AppHandle,
    state: State<'_, MotionCommandState>,
    documents: State<'_, Arc<crate::motion_documents::MotionDocumentStore>>,
    request: MotionAddCommand,
) -> Result<MotionCommit, String> {
    let document_request = match (request.document_id, request.revision_hash) {
        (Some(document_id), Some(revision_hash)) => {
            if request.code.is_some() || request.template_id.is_some() || !request.params.is_empty()
            {
                return Err(
                    "documentId/revisionHash cannot be combined with code, templateId, or params"
                        .into(),
                );
            }
            Some((
                documents.capture_authority()?,
                document_id,
                revision_hash,
                request
                    .width
                    .ok_or_else(|| "document publish requires width".to_string())?,
                request
                    .height
                    .ok_or_else(|| "document publish requires height".to_string())?,
                request
                    .fps
                    .ok_or_else(|| "document publish requires fps".to_string())?,
                request.transparent,
            ))
        }
        (None, None) => None,
        _ => return Err("documentId and revisionHash must be provided together".into()),
    };
    let legacy_source = if document_request.is_none() {
        Some(match (request.code, request.template_id) {
            (Some(code), None) => MotionSourceRequest::Code(code),
            (None, Some(template_id)) => MotionSourceRequest::Template {
                template_id,
                params: request.params,
            },
            _ => {
                return Err(
                    "provide exactly one of code, templateId, or documentId/revisionHash".into(),
                )
            }
        })
    } else {
        None
    };
    let cancel = state.begin()?;
    let bridge = state
        .bridge
        .as_ref()
        .clone()
        .with_progress_callback(Arc::new(move |phase| {
            let _ = app.emit("motion_progress", phase);
        }));
    let documents = Arc::clone(documents.inner());
    let worker = tauri::async_runtime::spawn_blocking(move || {
        if let Some((authority, document_id, revision_hash, width, height, fps, transparent)) =
            document_request
        {
            let source = resolve_document_motion_source(
                &documents,
                authority.clone(),
                &document_id,
                &revision_hash,
            )?;
            bridge.add_document(
                DocumentMotionAddRequest {
                    source,
                    project_authority: authority,
                    width,
                    height,
                    fps,
                    start_frame: request.start_frame,
                    duration_frames: request.duration_frames,
                    track_index: request.track_index,
                    transparent,
                },
                &cancel,
            )
        } else {
            bridge.add(
                AddMotionRequest {
                    source: legacy_source.expect("legacy source validated before worker"),
                    start_frame: request.start_frame,
                    duration_frames: request.duration_frames,
                    transparent: request.transparent,
                    track_index: request.track_index,
                },
                &cancel,
            )
        }
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
    documents: State<'_, Arc<crate::motion_documents::MotionDocumentStore>>,
    request: MotionEditCommand,
) -> Result<MotionCommit, String> {
    let document_request = match (request.document_id, request.revision_hash) {
        (Some(document_id), Some(revision_hash)) => {
            if request.code.is_some() || request.params.is_some() {
                return Err(
                    "documentId/revisionHash cannot be combined with code or params".into(),
                );
            }
            Some((
                documents.capture_authority()?,
                document_id,
                revision_hash,
                request
                    .width
                    .ok_or_else(|| "document edit requires width".to_string())?,
                request
                    .height
                    .ok_or_else(|| "document edit requires height".to_string())?,
                request
                    .fps
                    .ok_or_else(|| "document edit requires fps".to_string())?,
                request
                    .duration_frames
                    .ok_or_else(|| "document edit requires durationFrames".to_string())?,
            ))
        }
        (None, None) => None,
        _ => return Err("documentId and revisionHash must be provided together".into()),
    };
    let cancel = state.begin()?;
    let bridge = state
        .bridge
        .as_ref()
        .clone()
        .with_progress_callback(Arc::new(move |phase| {
            let _ = app.emit("motion_progress", phase);
        }));
    let documents = Arc::clone(documents.inner());
    let worker = tauri::async_runtime::spawn_blocking(move || {
        if let Some((authority, document_id, revision_hash, width, height, fps, duration_frames)) =
            document_request
        {
            let source = resolve_document_motion_source(
                &documents,
                authority.clone(),
                &document_id,
                &revision_hash,
            )?;
            bridge.edit_document(
                DocumentMotionEditRequest {
                    clip_id: request.clip_id,
                    source,
                    project_authority: authority,
                    width,
                    height,
                    fps,
                    duration_frames,
                },
                &cancel,
            )
        } else {
            bridge.edit(
                EditMotionRequest {
                    clip_id: request.clip_id,
                    code: request.code,
                    params: request.params,
                },
                &cancel,
            )
        }
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

fn render_document_preview(
    bridge: &TauriMotionBridge,
    documents: &crate::motion_documents::MotionDocumentStore,
    authority: ProjectAssetAuthority,
    request: MotionPreviewRequest,
    cancellation: &MotionCancellationToken,
) -> Result<MotionPreviewResponse, MotionPreviewError> {
    let (render_request, revision_hash) =
        prepare_document_preview(documents, authority.clone(), &request)?;
    let rendered = bridge
        .renderer
        .render_with_cancellation(&render_request, cancellation)
        .map_err(map_preview_motion_error)?;
    finish_document_preview(
        documents,
        authority,
        request.frame,
        revision_hash,
        rendered,
        cancellation,
    )
}

/// Agent preview adapter. The MCP transport and project lifecycle own a
/// MediaCancelToken, while Chromium uses MotionCancellationToken; this bridge
/// mirrors cancellation for the complete render and joins its short monitor
/// before returning.
pub(crate) fn render_document_preview_for_agent(
    bridge: &TauriMotionBridge,
    documents: &crate::motion_documents::MotionDocumentStore,
    authority: ProjectAssetAuthority,
    request: MotionPreviewRequest,
    cancel: &opentake_media::MediaCancelToken,
) -> Result<MotionPreviewResponse, MotionPreviewError> {
    struct CompletionSignal(Arc<AtomicBool>);
    impl Drop for CompletionSignal {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    let cancellation = MotionCancellationToken::new();
    let complete = Arc::new(AtomicBool::new(false));
    let completion_signal = CompletionSignal(complete.clone());
    let monitor_complete = complete.clone();
    let monitor_cancel = cancel.clone();
    let monitor_motion = cancellation.clone();
    let monitor = std::thread::Builder::new()
        .name("motion-document-preview-cancel".into())
        .spawn(move || {
            while !monitor_complete.load(Ordering::Acquire) {
                if monitor_cancel.is_cancelled() {
                    monitor_motion.cancel();
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        })
        .map_err(|_| preview_error("Motion preview cancellation could not be initialized."))?;
    let result = render_document_preview(bridge, documents, authority, request, &cancellation);
    drop(completion_signal);
    let _ = monitor.join();
    if cancel.is_cancelled() {
        Err(preview_error("Motion preview was cancelled."))
    } else {
        result
    }
}

fn finish_document_preview(
    documents: &crate::motion_documents::MotionDocumentStore,
    authority: ProjectAssetAuthority,
    frame: u32,
    revision_hash: String,
    rendered: RenderedClip,
    cancellation: &MotionCancellationToken,
) -> Result<MotionPreviewResponse, MotionPreviewError> {
    ensure_preview_active(cancellation)?;
    let png = read_single_preview_png(&rendered).map_err(map_preview_motion_error)?;
    ensure_preview_active(cancellation)?;
    documents
        .ensure_authority(&authority)
        .map_err(|_| preview_error("The project changed before the preview completed."))?;
    ensure_preview_active(cancellation)?;
    let png_data_url = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    );
    ensure_preview_active(cancellation)?;
    Ok(MotionPreviewResponse {
        revision_hash,
        frame,
        png_data_url,
        diagnostics: Vec::new(),
    })
}

fn ensure_preview_active(cancellation: &MotionCancellationToken) -> Result<(), MotionPreviewError> {
    if cancellation.is_cancelled() {
        Err(preview_error("Motion preview was cancelled."))
    } else {
        Ok(())
    }
}

fn prepare_document_preview(
    documents: &crate::motion_documents::MotionDocumentStore,
    authority: ProjectAssetAuthority,
    request: &MotionPreviewRequest,
) -> Result<(MotionRenderRequest, String), MotionPreviewError> {
    if request.duration_frames == 0
        || request.duration_frames > limits::MAX_FRAMES
        || request.frame >= request.duration_frames
    {
        return Err(preview_error(
            "Preview frame must be inside the bounded document duration.",
        ));
    }
    let document = documents
        .read_for_authority(authority, &request.document_id)
        .map_err(|_| preview_error("Motion Studio document could not be read."))?;
    if document.summary.revision_hash != request.revision_hash {
        return Err(preview_error(
            "Motion Studio document changed; reload before previewing.",
        ));
    }
    let source = MotionDocumentSource::new(document.html, document.css)
        .inline_document()
        .map_err(preview_source_error)?;
    let render_request = MotionRenderRequest::new(
        MotionSource::code(source),
        request.fps,
        1,
        request.width,
        request.height,
    )
    .with_start_frame(request.frame)
    .with_transparent(true);
    render_request
        .validate()
        .map_err(map_preview_motion_error)?;
    Ok((render_request, document.summary.revision_hash))
}

pub(crate) fn resolve_document_motion_source(
    documents: &crate::motion_documents::MotionDocumentStore,
    authority: ProjectAssetAuthority,
    document_id: &str,
    revision_hash: &str,
) -> Result<DocumentMotionSource, MotionBridgeError> {
    let document = documents
        .read_for_authority(authority, document_id)
        .map_err(|_| {
            MotionBridgeError::new(
                MotionBridgeErrorKind::ResourceNotFound,
                "Motion Studio document could not be read",
            )
        })?;
    if document.summary.revision_hash != revision_hash {
        return Err(MotionBridgeError::new(
            MotionBridgeErrorKind::InvalidArguments,
            "Motion Studio document changed; reload before publishing",
        ));
    }
    MotionDocumentSource::new(document.html.clone(), document.css.clone())
        .inline_document()
        .map_err(|diagnostic| {
            MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                format!(
                    "Motion Studio source is invalid at {}:{}: {}",
                    diagnostic.line, diagnostic.column, diagnostic.message
                ),
            )
        })?;
    Ok(DocumentMotionSource {
        document_id: document.summary.id,
        revision_hash: document.summary.revision_hash,
        html: document.html,
        css: document.css,
    })
}

fn preview_source_error(error: MotionSourceDiagnostic) -> MotionPreviewError {
    MotionPreviewError {
        message: "Motion Studio source contains unsupported active content.".to_string(),
        diagnostics: vec![MotionPreviewDiagnostic {
            severity: "error",
            message: error.message,
            line: Some(error.line),
            column: Some(error.column),
        }],
    }
}

fn map_preview_motion_error(error: MotionError) -> MotionPreviewError {
    match error {
        MotionError::Cancelled => preview_error("Motion preview was cancelled."),
        MotionError::RendererUnavailable(_) => {
            preview_error("Motion preview requires the packaged Chromium renderer.")
        }
        MotionError::InvalidSource(_)
        | MotionError::InvalidRequest(_)
        | MotionError::UnknownTemplate(_)
        | MotionError::Manifest(_)
        | MotionError::Sandbox(_) => preview_error("Motion preview request is invalid."),
        MotionError::Timeout(_) => preview_error("Motion preview exceeded its time budget."),
        MotionError::RenderFailed(_) | MotionError::Io(_) => {
            preview_error("Motion preview could not be rendered.")
        }
    }
}

fn preview_error(message: &str) -> MotionPreviewError {
    MotionPreviewError {
        message: message.to_string(),
        diagnostics: vec![MotionPreviewDiagnostic {
            severity: "error",
            message: message.to_string(),
            line: None,
            column: None,
        }],
    }
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
    Document {
        document_id: String,
        revision_hash: String,
    },
}

struct PreparedMotionCommit {
    stored_source: StoredMotionSource,
    document_source: Option<DocumentMotionSource>,
    expected_authority: Option<ProjectAssetAuthority>,
    duration_frames: i32,
    transparent: bool,
    render_dimensions: Option<(u32, u32, u32)>,
    placement: MotionPlacement,
}

impl TauriMotionBridge {
    pub fn new(core: AppCore, cache_root: impl Into<std::path::PathBuf>) -> Self {
        let cache_root = cache_root.into();
        Self {
            core,
            renderer: HeadlessChromiumRenderer::new(
                MotionCache::new(cache_root.join("motion-frames")),
                // Bounded but generous: a complex motion graphic on a slow or
                // loaded machine can legitimately exceed a minute; 180s still
                // fails closed rather than hanging forever.
                SandboxPolicy::offline_with_timeout(Duration::from_secs(180)),
            ),
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

    pub fn add_document(
        &self,
        request: DocumentMotionAddRequest,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<MotionCommit, MotionBridgeError> {
        validate_document_source_identity(&request.source)?;
        validate_document_render_dimensions(request.width, request.height)?;
        if request.start_frame < 0 {
            return Err(MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                "startFrame must be non-negative",
            ));
        }
        let snapshot = self.core.runtime_snapshot();
        let timeline_duration_frames =
            timeline_duration_frames(request.duration_frames, request.fps, snapshot.timeline.fps)?;
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
        let stored = StoredMotionSource::Document {
            document_id: request.source.document_id.clone(),
            revision_hash: request.source.revision_hash.clone(),
        };
        self.commit(
            PreparedMotionCommit {
                stored_source: stored,
                document_source: Some(request.source),
                expected_authority: Some(request.project_authority),
                duration_frames: request.duration_frames,
                transparent: request.transparent,
                render_dimensions: Some((request.width, request.height, request.fps)),
                placement: MotionPlacement::Add {
                    start_frame: request.start_frame,
                    duration_frames: timeline_duration_frames,
                    track_index: request.track_index,
                },
            },
            cancel,
        )
    }

    pub fn edit_document(
        &self,
        request: DocumentMotionEditRequest,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<MotionCommit, MotionBridgeError> {
        validate_document_source_identity(&request.source)?;
        validate_document_render_dimensions(request.width, request.height)?;
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
        let timeline_duration_frames =
            timeline_duration_frames(request.duration_frames, request.fps, snapshot.timeline.fps)?;
        if timeline_duration_frames != clip.duration_frames {
            return Err(MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                "Motion Studio replacement duration must match the existing clip",
            ));
        }
        if entry
            .generation_input
            .as_ref()
            .filter(|input| input.provider.as_deref() == Some(MOTION_PROVIDER))
            .is_none()
        {
            return Err(MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                "the selected clip is not an OpenTake motion graphic",
            ));
        }
        let stored = StoredMotionSource::Document {
            document_id: request.source.document_id.clone(),
            revision_hash: request.source.revision_hash.clone(),
        };
        self.commit(
            PreparedMotionCommit {
                stored_source: stored,
                document_source: Some(request.source),
                expected_authority: Some(request.project_authority),
                duration_frames: request.duration_frames,
                transparent: entry.carries_straight_alpha(),
                render_dimensions: Some((request.width, request.height, request.fps)),
                placement: MotionPlacement::Replace {
                    clip_id: request.clip_id,
                },
            },
            cancel,
        )
    }

    fn render_and_encode(
        &self,
        stored_source: &StoredMotionSource,
        document_source: Option<&DocumentMotionSource>,
        duration_frames: i32,
        transparent: bool,
        render_dimensions: Option<(u32, u32, u32)>,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<(tempfile::TempDir, std::path::PathBuf, ProbedMedia, String), MotionBridgeError>
    {
        (self.progress)(MotionProgress::Validating);
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
        let (width, height, fps) = render_dimensions.unwrap_or_else(|| {
            (
                u32::try_from(snapshot.timeline.width.max(2)).unwrap_or(2),
                u32::try_from(snapshot.timeline.height.max(2)).unwrap_or(2),
                u32::try_from(snapshot.timeline.fps.max(1)).unwrap_or(30),
            )
        });
        let frames = u32::try_from(duration_frames).map_err(|_| {
            MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                "durationFrames is invalid",
            )
        })?;
        let html = if let Some(document) = document_source {
            MotionDocumentSource::new(document.html.clone(), document.css.clone())
                .inline_document()
                .map_err(|diagnostic| {
                    MotionBridgeError::new(
                        MotionBridgeErrorKind::InvalidArguments,
                        format!(
                            "Motion Studio source is invalid at {}:{}: {}",
                            diagnostic.line, diagnostic.column, diagnostic.message
                        ),
                    )
                })?
        } else {
            source_document(stored_source, fps, width, height, frames, transparent)?
        };
        let request =
            MotionRenderRequest::new(MotionSource::code(html), fps, frames, width, height)
                .with_transparent(transparent);
        request.validate().map_err(map_motion_error)?;
        let render_cancel = MotionCancellationToken::new();
        if cancel.is_cancelled() {
            render_cancel.cancel();
        }
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
        (self.progress)(MotionProgress::Rendering {
            done_frames: 0,
            total_frames: frames,
        });
        let progress = Arc::clone(&self.progress);
        let rendered = self
            .renderer
            .render_with_cancellation_and_progress(
                &request,
                &render_cancel,
                &move |done_frames, total_frames| {
                    progress(MotionProgress::Rendering {
                        done_frames,
                        total_frames,
                    });
                },
            )
            .map_err(map_motion_error);
        done.store(true, Ordering::Release);
        let _ = monitor.join();
        let rendered = rendered?;

        let output_dir = tempfile::Builder::new()
            .prefix("opentake-motion-")
            .tempdir()
            .map_err(io_motion_error)?;
        let output = output_dir.path().join(if transparent {
            "output.mov"
        } else {
            "output.mp4"
        });
        (self.progress)(MotionProgress::Encoding);
        if let Err(error) = encode_frames(&rendered, &output, transparent, cancel) {
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
        request: PreparedMotionCommit,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<MotionCommit, MotionBridgeError> {
        let PreparedMotionCommit {
            stored_source,
            document_source,
            expected_authority,
            duration_frames,
            transparent,
            render_dimensions,
            placement,
        } = request;
        let snapshot = self.core.runtime_snapshot();
        if expected_authority.as_ref().is_some_and(|authority| {
            snapshot.project_epoch != authority.project_epoch
                || snapshot.project_dir.as_ref() != Some(&authority.project_path)
                || !self.core.project_asset_authority_matches(authority)
        }) {
            return Err(MotionBridgeError::new(
                MotionBridgeErrorKind::RenderFailed,
                "project changed before Motion Studio publishing began",
            ));
        }
        let project_dir = snapshot.project_dir.clone().ok_or_else(|| {
            MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                "Save the project before rendering a motion graphic.",
            )
        })?;
        ensure_motion_active(cancel)?;
        let (_temporary_output, output, probe, content_hash) = self.render_and_encode(
            &stored_source,
            document_source.as_ref(),
            duration_frames,
            transparent,
            render_dimensions,
            cancel,
        )?;
        let motion_canvas = matches!(
            &stored_source,
            StoredMotionSource::Template { template_id, .. } if template_id == "title-card"
        );
        let motion_document = matches!(&stored_source, StoredMotionSource::Document { .. });
        let output_metadata = MotionOutputMetadata {
            renderer: if motion_document {
                "opentake-motion-studio".into()
            } else if motion_canvas {
                "motion-canvas".into()
            } else {
                "opentake-html-fallback".into()
            },
            renderer_version: if motion_canvas {
                "3.17.2".into()
            } else {
                env!("CARGO_PKG_VERSION").into()
            },
            output_file: if transparent {
                "output.mov".into()
            } else {
                "output.mp4".into()
            },
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
        ensure_motion_active(cancel)?;
        if expected_authority
            .as_ref()
            .is_some_and(|authority| !self.core.project_asset_authority_matches(authority))
        {
            return Err(MotionBridgeError::new(
                MotionBridgeErrorKind::RenderFailed,
                "project changed before Motion Studio publishing completed",
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
            aspect_ratio: format!(
                "{}:{}",
                probe.width.unwrap_or(snapshot.timeline.width),
                probe.height.unwrap_or(snapshot.timeline.height)
            ),
            provider: Some(MOTION_PROVIDER.into()),
            status: Some(GenerationJobStatus::Ready),
            transparent: Some(transparent),
            ..GenerationInput::default()
        };
        (self.progress)(MotionProgress::Committing);
        let publication = self.core.lock_project_bundle_publication();
        let identity = self.core.lock_project_identity_workflow();
        ensure_motion_active(cancel)?;
        if expected_authority
            .as_ref()
            .is_some_and(|authority| !self.core.project_asset_authority_matches(authority))
        {
            return Err(MotionBridgeError::new(
                MotionBridgeErrorKind::RenderFailed,
                "project changed before Motion Studio publishing committed",
            ));
        }
        let project_media = crate::library::ProjectMediaCapability::open_verified(
            &self.core,
            snapshot.project_epoch,
            &project_dir,
            true,
        )
        .map_err(|error| MotionBridgeError::new(MotionBridgeErrorKind::RenderFailed, error))?;
        let leaf_name = format!(
            "motion-{}.{}",
            uuid::Uuid::new_v4(),
            if transparent { "mov" } else { "mp4" }
        );
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
        project_media
            .sync_media_directory()
            .map_err(|error| MotionBridgeError::new(MotionBridgeErrorKind::RenderFailed, error))?;
        ensure_motion_active(cancel)?;
        if expected_authority
            .as_ref()
            .is_some_and(|authority| !self.core.project_asset_authority_matches(authority))
        {
            return Err(MotionBridgeError::new(
                MotionBridgeErrorKind::RenderFailed,
                "project changed before Motion Studio publishing committed",
            ));
        }
        let mut events = DeferredCoreEvents::default();
        let committed = self.core.commit_motion_media_for_project_deferred(
            &publication,
            snapshot.project_epoch,
            snapshot.version,
            &project_dir,
            published.path(),
            "Motion Graphic",
            &probe,
            provenance,
            placement,
            &mut events,
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
        drop(identity);
        drop(publication);
        self.core.emit_deferred(events);
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
            source_document: match stored_source {
                StoredMotionSource::Document {
                    document_id,
                    revision_hash,
                } => Some(MotionDocumentReference {
                    document_id,
                    revision_hash,
                }),
                _ => None,
            },
        })
    }
}

fn validate_document_source_identity(
    source: &DocumentMotionSource,
) -> Result<(), MotionBridgeError> {
    if source.document_id.is_empty()
        || source.document_id.len() > 128
        || !source
            .document_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(MotionBridgeError::new(
            MotionBridgeErrorKind::InvalidArguments,
            "Motion Studio document id is invalid",
        ));
    }
    if source.revision_hash.len() != 64
        || !source
            .revision_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(MotionBridgeError::new(
            MotionBridgeErrorKind::InvalidArguments,
            "Motion Studio revision hash is invalid",
        ));
    }
    Ok(())
}

fn validate_document_render_dimensions(width: u32, height: u32) -> Result<(), MotionBridgeError> {
    if !width.is_multiple_of(2) || !height.is_multiple_of(2) {
        return Err(MotionBridgeError::new(
            MotionBridgeErrorKind::InvalidArguments,
            "Motion Studio MP4 dimensions must be even numbers",
        ));
    }
    Ok(())
}

fn timeline_duration_frames(
    source_frames: i32,
    source_fps: u32,
    timeline_fps: i32,
) -> Result<i32, MotionBridgeError> {
    if source_frames < 1 || source_fps == 0 || timeline_fps < 1 {
        return Err(MotionBridgeError::new(
            MotionBridgeErrorKind::InvalidArguments,
            "Motion Studio duration and frame rates must be positive",
        ));
    }
    let numerator = u64::try_from(source_frames)
        .ok()
        .and_then(|frames| frames.checked_mul(u64::try_from(timeline_fps).ok()?))
        .ok_or_else(|| {
            MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                "Motion Studio timeline duration is out of range",
            )
        })?;
    let rounded = numerator
        .checked_add(u64::from(source_fps) / 2)
        .map(|value| value / u64::from(source_fps))
        .and_then(|frames| i32::try_from(frames.max(1)).ok())
        .ok_or_else(|| {
            MotionBridgeError::new(
                MotionBridgeErrorKind::InvalidArguments,
                "Motion Studio timeline duration is out of range",
            )
        })?;
    Ok(rounded)
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
            PreparedMotionCommit {
                stored_source,
                document_source: None,
                expected_authority: None,
                duration_frames: request.duration_frames,
                transparent: request.transparent,
                render_dimensions: None,
                placement: MotionPlacement::Add {
                    start_frame: request.start_frame,
                    duration_frames: request.duration_frames,
                    track_index: request.track_index,
                },
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
            (StoredMotionSource::Document { .. }, _, _) => {
                return Err(MotionBridgeError::new(
                    MotionBridgeErrorKind::InvalidArguments,
                    "Motion Studio clips must be edited from an exact document revision",
                ));
            }
        }
        self.commit(
            PreparedMotionCommit {
                stored_source: source,
                document_source: None,
                expected_authority: None,
                duration_frames: clip.duration_frames,
                transparent: false,
                render_dimensions: None,
                placement: MotionPlacement::Replace {
                    clip_id: request.clip_id,
                },
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
    transparent: bool,
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
        } => template_document(template_id, params, fps, width, height, frames, transparent),
        StoredMotionSource::Document { .. } => Err(MotionBridgeError::new(
            MotionBridgeErrorKind::InvalidArguments,
            "Motion Studio document source was not resolved",
        )),
    }
}

fn ensure_motion_active(
    cancel: &opentake_media::MediaCancelToken,
) -> Result<(), MotionBridgeError> {
    if cancel.is_cancelled() {
        Err(MotionBridgeError::new(
            MotionBridgeErrorKind::Cancelled,
            "motion render cancelled",
        ))
    } else {
        Ok(())
    }
}

fn template_document(
    template_id: &str,
    params: &Map<String, Value>,
    fps: u32,
    width: u32,
    height: u32,
    frames: u32,
    transparent: bool,
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
    let background_value = if transparent {
        "transparent".to_owned()
    } else {
        string_param("background", "#11131A")?
    };
    let background = js_string(&background_value);
    if template_id == "title-card" {
        let config = serde_json::json!({
            "templateId": "title-card",
            "params": {
                "title": string_param("title", "OpenTake")?,
                "subtitle": string_param("subtitle", "Motion Canvas")?,
                "accent": string_param("accent", "#7C5CFF")?,
                "background": if transparent {
                    "transparent".to_owned()
                } else {
                    string_param("background", "#11131A")?
                },
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
        let runner = runner.replacen("__OPENTAKE_MOTION_CONFIG_JSON__", &config, 1);
        return Ok(if transparent {
            runner.replace("background:#11131a", "background:transparent")
        } else {
            runner
        });
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
    transparent: bool,
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
    let mut command = Command::new(opentake_media::ffmpeg_status::ffmpeg_path());
    command
        .args(["-v", "error", "-nostdin", "-framerate"])
        .arg(rendered.fps.to_string())
        .arg("-i")
        .arg(pattern)
        .args(["-frames:v", &rendered.frames.len().to_string()])
        .args(["-an", "-c:v"]);
    if transparent {
        command.args(["prores_ks", "-profile:v", "4", "-pix_fmt", "yuva444p10le"]);
    } else {
        command.args(["libx264", "-pix_fmt", "yuv420p", "-movflags", "+faststart"]);
    }
    let mut child = command
        .arg("-y")
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

    fn saved_document() -> (
        tempfile::TempDir,
        AppCore,
        crate::motion_documents::MotionDocumentStore,
        crate::motion_documents::MotionDocument,
    ) {
        let temp = tempfile::tempdir().expect("create preview fixture parent");
        let project = temp.path().join("preview.opentake");
        let core = AppCore::new();
        core.save_project(Some(project))
            .expect("save preview fixture project");
        let store = crate::motion_documents::MotionDocumentStore::new(core.clone());
        let document = store
            .create(crate::motion_documents::MotionDocumentCreateRequest {
                title: Some("片头预览".to_string()),
            })
            .expect("create preview fixture document");
        (temp, core, store, document)
    }

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

    #[test]
    fn updater_gate_observes_and_cancels_an_active_motion_render() {
        let temp = tempfile::tempdir().unwrap();
        let bridge = Arc::new(TauriMotionBridge::new(AppCore::new(), temp.path()));
        let admission = crate::updater::InstallAdmissionGate::default();
        let state = MotionCommandState::new(bridge, admission.clone());
        assert!(!state.has_active());

        let token = state.begin().unwrap();
        assert!(state.has_active());
        assert!(state.cancel_active());
        assert!(token.is_cancelled());

        state.finish();
        assert!(!state.has_active());
        assert!(!state.cancel_active());
    }

    #[test]
    fn motion_cannot_begin_after_update_install_claims_admission() {
        let temp = tempfile::tempdir().unwrap();
        let bridge = Arc::new(TauriMotionBridge::new(AppCore::new(), temp.path()));
        let admission = crate::updater::InstallAdmissionGate::default();
        let state = MotionCommandState::new(bridge, admission.clone());
        let _install = admission.begin_install().unwrap();

        assert_eq!(
            state.begin().err().unwrap(),
            "app update installation is in progress"
        );
    }

    #[test]
    fn preview_preparation_is_bound_to_the_exact_document_revision_and_frame() {
        let (_temp, _core, store, document) = saved_document();
        let authority = store
            .capture_authority()
            .expect("capture project authority");
        let request = MotionPreviewRequest {
            document_id: document.summary.id.clone(),
            revision_hash: document.summary.revision_hash.clone(),
            width: 640,
            height: 360,
            fps: 30,
            duration_frames: 90,
            frame: 42,
        };

        let (render, revision) =
            prepare_document_preview(&store, authority.clone(), &request).unwrap();
        assert_eq!(revision, document.summary.revision_hash);
        assert_eq!(render.start_frame, 42);
        assert_eq!(render.duration_frames, 1);
        assert_eq!((render.width, render.height, render.fps), (640, 360, 30));
        let MotionSource::Code { html_css_js } = render.source else {
            panic!("document preview must compile to a self-contained source");
        };
        assert!(html_css_js.contains("让创意动起来"));
        assert!(html_css_js.contains("@keyframes"));
        assert!(html_css_js.contains("script-src 'none'"));
        assert!(!html_css_js.contains("<script"));

        let mut stale = request;
        stale.revision_hash = "0".repeat(64);
        let error = prepare_document_preview(&store, authority, &stale)
            .expect_err("stale editor content must never be previewed");
        assert!(error.message.contains("changed"));
        assert_eq!(error.diagnostics.len(), 1);
        assert_eq!(error.diagnostics[0].line, None);
        assert_eq!(error.diagnostics[0].column, None);
    }

    #[test]
    fn publish_source_resolution_rejects_a_stale_document_revision() {
        let (_temp, _core, store, document) = saved_document();
        let authority = store.capture_authority().expect("capture authority");
        let resolved = resolve_document_motion_source(
            &store,
            authority.clone(),
            &document.summary.id,
            &document.summary.revision_hash,
        )
        .expect("resolve exact document revision");
        assert_eq!(resolved.document_id, document.summary.id);
        assert_eq!(resolved.revision_hash, document.summary.revision_hash);
        assert!(resolved.html.contains("让创意动起来"));

        let error = resolve_document_motion_source(
            &store,
            authority,
            &document.summary.id,
            &"0".repeat(64),
        )
        .expect_err("stale revision must never be rendered or committed");
        assert_eq!(error.kind, MotionBridgeErrorKind::InvalidArguments);
        assert!(error.message.contains("changed"));
    }

    #[test]
    fn document_publish_validates_dimensions_before_starting_the_renderer() {
        let (temp, core, store, document) = saved_document();
        let authority = store.capture_authority().expect("capture authority");
        let source = resolve_document_motion_source(
            &store,
            authority,
            &document.summary.id,
            &document.summary.revision_hash,
        )
        .unwrap();
        let bridge = TauriMotionBridge::new(core.clone(), temp.path().join("cache"));
        let before = core.runtime_snapshot();
        let error = bridge
            .add_document(
                DocumentMotionAddRequest {
                    source,
                    project_authority: core.project_asset_authority().unwrap(),
                    width: 3,
                    height: 360,
                    fps: 30,
                    start_frame: 0,
                    duration_frames: 30,
                    track_index: None,
                    transparent: false,
                },
                &opentake_media::MediaCancelToken::new(),
            )
            .expect_err("invalid dimensions must fail without renderer availability");
        assert_eq!(error.kind, MotionBridgeErrorKind::InvalidArguments);
        assert_eq!(core.runtime_snapshot().timeline, before.timeline);
        assert_eq!(core.runtime_snapshot().media, before.media);
    }

    #[test]
    fn document_duration_preserves_seconds_across_source_and_timeline_fps() {
        assert_eq!(timeline_duration_frames(90, 30, 60).unwrap(), 180);
        assert_eq!(timeline_duration_frames(24, 24, 30).unwrap(), 30);
        assert_eq!(timeline_duration_frames(1, 24, 30).unwrap(), 1);
        assert!(timeline_duration_frames(0, 30, 60).is_err());
        assert!(timeline_duration_frames(30, 0, 60).is_err());
    }

    #[test]
    fn document_publish_is_bound_to_the_project_captured_at_command_admission() {
        let (temp, core, store, document) = saved_document();
        let authority = store.capture_authority().expect("capture project A");
        let source = resolve_document_motion_source(
            &store,
            authority.clone(),
            &document.summary.id,
            &document.summary.revision_hash,
        )
        .unwrap();
        core.new_project();
        core.save_project(Some(temp.path().join("replacement.opentake")))
            .unwrap();
        let replacement = core.runtime_snapshot();
        let bridge = TauriMotionBridge::new(core.clone(), temp.path().join("cache"));

        let error = bridge
            .add_document(
                DocumentMotionAddRequest {
                    source,
                    project_authority: authority,
                    width: 640,
                    height: 360,
                    fps: 30,
                    start_frame: 0,
                    duration_frames: 30,
                    track_index: None,
                    transparent: false,
                },
                &opentake_media::MediaCancelToken::new(),
            )
            .expect_err("queued publish must not cross into a replacement project");
        assert_eq!(error.kind, MotionBridgeErrorKind::RenderFailed);
        assert!(error.message.contains("project changed"));
        assert_eq!(core.runtime_snapshot().timeline, replacement.timeline);
        assert_eq!(core.runtime_snapshot().media, replacement.media);
    }

    #[test]
    fn ffmpeg_failure_does_not_leave_a_publishable_output() {
        if !opentake_media::ffmpeg_status::ffmpeg_available() {
            eprintln!("SKIP: FFmpeg unavailable");
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let frame = temp.path().join("frame_00000.png");
        std::fs::write(&frame, b"not a PNG").unwrap();
        let rendered = RenderedClip {
            content_hash: "f".repeat(64),
            frames: vec![frame],
            fps: 30,
            width: 64,
            height: 36,
            transparent: false,
        };
        let output = temp.path().join("output.mp4");
        let error = encode_frames(
            &rendered,
            &output,
            false,
            &opentake_media::MediaCancelToken::new(),
        )
        .expect_err("invalid renderer frames must make FFmpeg fail closed");
        assert_eq!(error.kind, MotionBridgeErrorKind::RenderFailed);
        assert!(error.message.contains("FFmpeg failed"));
        assert!(
            !output.exists() || output.metadata().unwrap().len() == 0,
            "failed encoding must not leave a usable output"
        );
    }

    #[test]
    fn transparent_encoding_uses_prores4444_and_preserves_alpha() {
        if !opentake_media::ffmpeg_status::ffmpeg_available() {
            eprintln!("SKIP: FFmpeg unavailable");
            return;
        }
        let temp = tempfile::tempdir().expect("alpha motion tempdir");
        let mut frames = Vec::new();
        for index in 0..2 {
            let path = temp.path().join(format!("frame_{index:05}.png"));
            let image = image::RgbaImage::from_fn(4, 4, |x, _| {
                image::Rgba([255, 20, 10, if x == 0 { 0 } else { 128 }])
            });
            image.save(&path).expect("write alpha PNG");
            frames.push(path);
        }
        let rendered = RenderedClip {
            content_hash: "a".repeat(64),
            frames,
            fps: 30,
            width: 4,
            height: 4,
            transparent: true,
        };
        let output = temp.path().join("output.mov");
        encode_frames(
            &rendered,
            &output,
            true,
            &opentake_media::MediaCancelToken::new(),
        )
        .expect("transparent frames encode");

        let probe = opentake_media::probe(&output).expect("probe ProRes alpha output");
        assert_eq!(probe.video_codec.as_deref(), Some("prores"));
        assert_eq!(probe.width, Some(4));
        assert_eq!(probe.height, Some(4));
        let alpha = Command::new(opentake_media::ffmpeg_status::ffmpeg_path())
            .args([
                "-v",
                "error",
                "-i",
                output.to_str().expect("UTF-8 output path"),
                "-vf",
                "alphaextract",
                "-frames:v",
                "1",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "gray",
                "pipe:1",
            ])
            .output()
            .expect("decode alpha plane");
        assert!(alpha.status.success(), "decode alpha: {:?}", alpha.stderr);
        assert!(alpha.stdout.iter().any(|value| *value == 0));
        assert!(alpha.stdout.iter().any(|value| *value > 0 && *value < 255));
    }

    #[test]
    fn newer_preview_cancels_but_does_not_forget_the_older_worker() {
        let temp = tempfile::tempdir().unwrap();
        let bridge = Arc::new(TauriMotionBridge::new(AppCore::new(), temp.path()));
        let admission = crate::updater::InstallAdmissionGate::default();
        let state = MotionCommandState::new(bridge, admission);

        let (older_generation, older) = state.begin_preview().unwrap();
        let (newer_generation, newer) = state.begin_preview().unwrap();
        assert!(older.is_cancelled());
        assert!(!newer.is_cancelled());
        assert!(state.has_active());
        assert!(matches!(
            state.begin(),
            Err(message) if message == "another motion render is already running"
        ));

        state.finish_preview(newer_generation);
        assert!(
            state.has_active(),
            "the superseded worker must retain its updater lease until it exits"
        );
        state.finish_preview(older_generation);
        assert!(!state.has_active());
    }

    #[test]
    fn preview_cancel_only_cancels_preview_tokens() {
        let temp = tempfile::tempdir().unwrap();
        let bridge = Arc::new(TauriMotionBridge::new(AppCore::new(), temp.path()));
        let admission = crate::updater::InstallAdmissionGate::default();
        let state = MotionCommandState::new(bridge, admission);
        let (generation, preview) = state.begin_preview().unwrap();

        assert!(state.cancel_previews());
        assert!(preview.is_cancelled());
        assert!(
            state.has_active(),
            "worker retains its lease until it exits"
        );

        state.finish_preview(generation);
        assert!(!state.has_active());
        assert!(!state.cancel_previews());
    }

    #[test]
    fn cancelled_preview_cannot_publish_a_completed_png_response() {
        use opentake_motion::{MotionRenderer, StubRenderer};

        let (_temp, _core, store, document) = saved_document();
        let authority = store.capture_authority().expect("capture authority");
        let request = MotionPreviewRequest {
            document_id: document.summary.id.clone(),
            revision_hash: document.summary.revision_hash.clone(),
            width: 64,
            height: 48,
            fps: 30,
            duration_frames: 30,
            frame: 5,
        };
        let (render_request, revision_hash) =
            prepare_document_preview(&store, authority.clone(), &request).unwrap();
        let cache = tempfile::tempdir().unwrap();
        let rendered = StubRenderer::new(MotionCache::new(cache.path()))
            .render(&render_request)
            .unwrap();
        let cancellation = MotionCancellationToken::new();
        cancellation.cancel();

        let error = finish_document_preview(
            &store,
            authority,
            request.frame,
            revision_hash,
            rendered,
            &cancellation,
        )
        .expect_err("a superseded response must fail even after rendering completed");
        assert!(error.message.contains("cancelled"));
    }
}
