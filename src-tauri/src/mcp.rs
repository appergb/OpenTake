//! Tauri host adapters for the agent's render + import side-door
//! ([`MediaBridge`]).
//!
//! The old fixed-port external MCP listener is disabled for Beta because it has
//! no authenticated pairing UX. Official Codex turns instead create an
//! authenticated, loopback-only endpoint with a fresh bearer token for that
//! single turn. The plugin registry still seeds the bundled workflows (e.g. the
//! default audio-first Skill) plus user-authored plugins under
//! `<app_data_dir>/workflows`.
//!
//! `inspect_timeline` and `import_media` need capabilities that live outside
//! `opentake-core` — GPU compositing (`opentake-render`) and the user-facing
//! import path (`crate::media`). The agent crate can't (by design shouldn't) link
//! those, so it takes them through the injected [`MediaBridge`]. This module is
//! where that boundary is implemented ([`TauriMediaBridge`]) and handed to the
//! dispatcher: it owns a session-sharing [`AppCore`] clone plus a [`MediaEngine`]
//! built from the same cache/models dirs the UI uses, so imports produce the exact
//! same posters / manifest entries / `MediaChanged` events as the media panel.

use std::collections::{HashMap, HashSet};
use std::io::{Seek, SeekFrom, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::sync::{Arc, RwLock};
use std::time::Duration;

#[cfg(test)]
use std::io::Read;

use base64::Engine as _;
use cap_fs_ext::{ambient_authority, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use same_file::Handle;

use opentake_agent::chat::ChatTurnGate;
use opentake_agent::mcp::dispatch::Dispatcher;
use opentake_agent::mcp::media_bridge::{
    BridgeError, ImportOutcome, ImportSource, InspectMediaRequest, InspectMediaResult,
    InspectResult, InspectedFrame, InspectedMediaFrame, MediaBridge, SearchCandidate,
    SearchIndexState, SearchMediaResult, SearchSpokenHit, SearchVisualHit,
    TimelineResultCaptureRequest, TranscriptSource, TranscriptSourceResult,
    IMPORT_BYTES_DECODED_MAX, TIMELINE_RESULT_IMAGE_BASE64_MAX,
};
use opentake_agent::mcp::motion::MotionBridge;
use opentake_agent::mcp::motion_documents::{
    AdmittedMotionDocumentOperation, MotionDocument as AgentMotionDocument, MotionDocumentBridge,
    MotionDocumentBridgeError, MotionDocumentBridgeErrorKind,
    MotionDocumentPreview as AgentMotionDocumentPreview,
    MotionDocumentPublish as AgentMotionDocumentPublish,
    MotionDocumentReference as AgentMotionDocumentReference, MotionDocumentRequest,
    MotionDocumentResponse, MotionDocumentSummary as AgentMotionDocumentSummary,
    MotionPreviewDiagnostic as AgentMotionPreviewDiagnostic,
};
use opentake_agent::mcp::server::{bind_ephemeral_gated, EphemeralMcpEndpoint, EphemeralMcpError};
use opentake_agent::plugin::registry::PluginRegistry;
use opentake_agent::tools::result::{Block, ToolResult};
use opentake_core::{
    importable_clip_type, AppCore, CoreError, DeferredCoreEvents, ProbedMedia,
    ProjectAssetAuthority, ProjectRuntimeSnapshot,
};
use opentake_domain::{ClipType, LutReference, MediaSource, TextStyle};
use opentake_media::{decode_frame_at, decode_frames_at, FrameRequest, MediaEngine, RgbaFrame};
use opentake_project::ProjectRoot;
use opentake_render::gpu::texture::upload_rgba;
use opentake_render::{
    even, try_build_render_plan, Compositor, CosmicTextRasterizer, DecodedFrame, FramePlan,
    GpuLutTexture, GpuTexture, LayerDraw, RenderDevice, RenderSize, SourceMetrics,
    TextRasterRequest, TextRasterizer, TextureCache, TextureResolver, TextureSource,
};

use crate::library::ProjectMediaCapability;

/// JPEG quality `inspect_timeline` encodes composited frames at (upstream
/// `inspectTimelineJPEGQuality = 0.7`). `image` takes a 0–100 byte.
const INSPECT_JPEG_QUALITY: u8 = 70;
const INSPECT_MEDIA_FRAME_MAX_DIMENSION: u32 = 512;
const INSPECT_MEDIA_OVERVIEW_TILES: usize = 36;
const INSPECT_MEDIA_OVERVIEW_COLUMNS: u32 = 6;
const INSPECT_MEDIA_OVERVIEW_TILE: (u32, u32) = (192, 108);
const MCP_MEDIA_PROBE_TIMEOUT: Duration = Duration::from_secs(30);

/// Per-frame texture cache size — bounds VRAM during a multi-frame inspect.
const TEXTURE_CACHE_CAP: usize = 64;

/// Hard decoded-byte ceiling for a URL response. `Content-Length` is only an
/// early rejection hint; the streaming counter below is authoritative.
const URL_IMPORT_DECODED_MAX: u64 = 1024 * 1024 * 1024;
const URL_IMPORT_REDIRECT_MAX: usize = 5;
#[cfg(test)]
const URL_IMPORT_READ_CHUNK: usize = 64 * 1024;

struct UrlFetchResponse {
    status: reqwest::StatusCode,
    location: Option<String>,
    content_type: Option<String>,
    content_length: Option<u64>,
    body: Box<dyn UrlResponseBody>,
}

trait UrlFetcher {
    fn fetch(
        &self,
        url: &reqwest::Url,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<UrlFetchResponse, BridgeError>;
}

struct ReqwestUrlFetcher {
    runtime: Arc<tokio::runtime::Runtime>,
}

impl ReqwestUrlFetcher {
    fn new() -> Result<Self, BridgeError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map(Arc::new)
            .map_err(|_| BridgeError::new("Failed to initialize HTTPS runtime"))?;
        Ok(Self { runtime })
    }
}

impl UrlFetcher for ReqwestUrlFetcher {
    fn fetch(
        &self,
        url: &reqwest::Url,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<UrlFetchResponse, BridgeError> {
        let url = url.clone();
        let (host, pinned) = self.runtime.block_on(resolve_public_target(&url, cancel))?;
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            // A configured proxy could resolve the hostname again or reach an
            // internal target on our behalf, bypassing local address checks.
            .no_proxy()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(5 * 60));
        if let Some(pinned) = pinned {
            builder = builder.resolve(&host, pinned);
        }
        let client = builder
            .build()
            .map_err(|_| BridgeError::new("Failed to initialize the secure HTTPS client"))?;
        let response = self.runtime.block_on(async {
            tokio::select! {
                result = client.get(url).send() => result.map_err(safe_reqwest_error),
                () = wait_for_media_cancel(cancel) => {
                    Err(BridgeError::new("source.url import was cancelled"))
                }
            }
        })?;
        let status = response.status();
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let content_length = response.content_length();
        Ok(UrlFetchResponse {
            status,
            location,
            content_type,
            content_length,
            body: Box::new(ReqwestUrlBody {
                runtime: self.runtime.clone(),
                response,
            }),
        })
    }
}

trait UrlResponseBody: Send {
    fn next_chunk(
        &mut self,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<Option<Vec<u8>>, BridgeError>;
}

#[cfg(test)]
struct ReaderUrlBody {
    reader: Box<dyn Read + Send>,
}

#[cfg(test)]
impl UrlResponseBody for ReaderUrlBody {
    fn next_chunk(
        &mut self,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<Option<Vec<u8>>, BridgeError> {
        cancelled_checkpoint(cancel)?;
        let mut buffer = vec![0_u8; URL_IMPORT_READ_CHUNK];
        match self.reader.read(&mut buffer) {
            Ok(0) => Ok(None),
            Ok(count) => {
                buffer.truncate(count);
                Ok(Some(buffer))
            }
            Err(error) => {
                cancelled_checkpoint(cancel)?;
                Err(BridgeError::new(format!(
                    "Failed while streaming source.url: {error}"
                )))
            }
        }
    }
}

struct ReqwestUrlBody {
    runtime: Arc<tokio::runtime::Runtime>,
    response: reqwest::Response,
}

impl UrlResponseBody for ReqwestUrlBody {
    fn next_chunk(
        &mut self,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<Option<Vec<u8>>, BridgeError> {
        let runtime = self.runtime.clone();
        let response = &mut self.response;
        runtime.block_on(async {
            tokio::select! {
                result = response.chunk() => result
                    .map(|chunk| chunk.map(|bytes| bytes.to_vec()))
                    .map_err(safe_reqwest_error),
                () = wait_for_media_cancel(cancel) => {
                    Err(BridgeError::new("source.url import was cancelled"))
                }
            }
        })
    }
}

async fn wait_for_media_cancel(cancel: &opentake_media::MediaCancelToken) {
    while !cancel.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn resolve_public_target(
    url: &reqwest::Url,
    cancel: &opentake_media::MediaCancelToken,
) -> Result<(String, Option<SocketAddr>), BridgeError> {
    let host = url
        .host_str()
        .ok_or_else(|| BridgeError::new("source.url must include a host"))?
        .to_string();
    let port = url.port_or_known_default().unwrap_or(443);
    if let Some(ip) = literal_host_ip(&host) {
        ensure_public_ip(ip)?;
        return Ok((host, None));
    }

    let addresses = {
        let lookup = tokio::net::lookup_host((host.as_str(), port));
        tokio::pin!(lookup);
        let timeout = tokio::time::sleep(Duration::from_secs(10));
        tokio::pin!(timeout);
        tokio::select! {
            result = &mut lookup => result.map_err(|_| BridgeError::new("source.url DNS resolution failed"))?,
            () = wait_for_media_cancel(cancel) => return Err(BridgeError::new("source.url import was cancelled")),
            () = &mut timeout => return Err(BridgeError::new("source.url DNS resolution timed out")),
        }
    };
    let addresses = addresses.collect::<Vec<_>>();
    Ok((host, Some(pin_public_address(addresses)?)))
}

fn pin_public_address(mut addresses: Vec<SocketAddr>) -> Result<SocketAddr, BridgeError> {
    addresses.sort_unstable();
    addresses.dedup();
    if addresses.is_empty() {
        return Err(BridgeError::new("source.url DNS returned no addresses"));
    }
    // Reject mixed public/private answers rather than choosing the public one:
    // this makes split-horizon and rebinding responses fail closed.
    for address in &addresses {
        ensure_public_ip(address.ip())?;
    }
    Ok(addresses[0])
}

fn ensure_public_ip(ip: IpAddr) -> Result<(), BridgeError> {
    if public_ip(ip) {
        Ok(())
    } else {
        Err(BridgeError::new(
            "source.url resolved to a non-public network address",
        ))
    }
}

fn literal_host_ip(host: &str) -> Option<IpAddr> {
    host.strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host)
        .parse()
        .ok()
}

fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => public_ipv4(ip),
        IpAddr::V6(ip) => public_ipv6(ip),
    }
}

fn public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !matches!(
        (a, b, c),
        (0, _, _)
            | (10, _, _)
            | (100, 64..=127, _)
            | (127, _, _)
            | (169, 254, _)
            | (172, 16..=31, _)
            | (192, 0, 0)
            | (192, 0, 2)
            | (192, 88, 99)
            | (192, 168, _)
            | (198, 18..=19, _)
            | (198, 51, 100)
            | (203, 0, 113)
            | (224..=255, _, _)
    )
}

fn public_ipv6(ip: Ipv6Addr) -> bool {
    if let Some(ipv4) = ip.to_ipv4() {
        return public_ipv4(ipv4);
    }
    let segments = ip.segments();
    let first = segments[0];
    if ip.is_unspecified()
        || ip.is_loopback()
        || (first & 0xfe00) == 0xfc00 // unique-local
        || (first & 0xfe00) == 0xfe00 // link/site-local and reserved
        || (first & 0xff00) == 0xff00 // multicast
        || (first & 0xe000) != 0x2000
    // fail closed outside global unicast 2000::/3
    {
        return false;
    }
    let is_special_purpose = matches!(
        (segments[0], segments[1]),
        (0x2001, 0x0000) // Teredo
            | (0x2001, 0x0002) // benchmarking
            | (0x2001, 0x0db8) // documentation
            | (0x2002, _) // 6to4 transition
    ) || (segments[0] == 0x2001
        && matches!(segments[1] & 0xfff0, 0x0010 | 0x0020));
    !is_special_purpose
}

fn safe_reqwest_error(error: reqwest::Error) -> BridgeError {
    if error.is_timeout() {
        BridgeError::new("source.url download timed out")
    } else if error.is_connect() {
        BridgeError::new("source.url connection failed")
    } else {
        // reqwest::Error Display can contain the complete signed URL.
        BridgeError::new("source.url download failed")
    }
}

/// Built-in workflows + any user-authored plugins under `workflows_dir`
/// (user plugins override a built-in with the same id, since `register` replaces
/// by id and runs after the built-ins).
pub(crate) fn build_registry(workflows_dir: &Path) -> PluginRegistry {
    let mut registry = PluginRegistry::with_builtins();
    if workflows_dir.is_dir() {
        let (user, errors) = PluginRegistry::scan(workflows_dir);
        for e in &errors {
            eprintln!("[mcp] workflow plugin load error: {e}");
        }
        for plugin in user.installed() {
            registry.register(plugin.clone());
        }
    }
    registry
}

/// Spawn the authenticated, project-gated MCP endpoint used by one official
/// Codex turn. The legacy fixed-port listener deliberately remains disabled;
/// every call gets a fresh loopback port and bearer credential whose lifetime
/// is owned by the returned endpoint.
pub(crate) async fn spawn(
    dispatcher: Arc<Dispatcher>,
    registry: Arc<RwLock<PluginRegistry>>,
    gate: Arc<dyn ChatTurnGate>,
) -> Result<EphemeralMcpEndpoint, EphemeralMcpError> {
    bind_ephemeral_gated(dispatcher, registry, gate).await
}

pub(crate) struct LiveProjectMcpGate {
    core: AppCore,
    transition_depth: Arc<AtomicUsize>,
    identity_generation: Arc<AtomicU64>,
    next_dispatch_id: AtomicU64,
    active_dispatches: Arc<Mutex<HashMap<u64, opentake_media::MediaCancelToken>>>,
}

struct LiveDispatchPermit {
    id: u64,
    active_dispatches: Arc<Mutex<HashMap<u64, opentake_media::MediaCancelToken>>>,
}

impl Drop for LiveDispatchPermit {
    fn drop(&mut self) {
        self.active_dispatches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.id);
    }
}

impl LiveProjectMcpGate {
    pub(crate) fn new(core: AppCore) -> Arc<Self> {
        let transition_depth = Arc::new(AtomicUsize::new(0));
        let identity_generation = Arc::new(AtomicU64::new(0));
        let active_dispatches = Arc::new(Mutex::new(HashMap::<
            u64,
            opentake_media::MediaCancelToken,
        >::new()));
        let transition_depth_for_hook = transition_depth.clone();
        let identity_generation_for_hook = identity_generation.clone();
        let active_dispatches_for_hook = active_dispatches.clone();
        core.subscribe_project_identity_transition(move |pending| {
            if pending {
                identity_generation_for_hook.fetch_add(1, Ordering::AcqRel);
                transition_depth_for_hook.fetch_add(1, Ordering::AcqRel);
                for cancel in active_dispatches_for_hook
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .values()
                {
                    cancel.cancel();
                }
            } else {
                let _ = transition_depth_for_hook.fetch_update(
                    Ordering::AcqRel,
                    Ordering::Acquire,
                    |depth| Some(depth.saturating_sub(1)),
                );
            }
        });
        Arc::new(Self {
            core,
            transition_depth,
            identity_generation,
            next_dispatch_id: AtomicU64::new(1),
            active_dispatches,
        })
    }

    fn transition_pending(&self) -> bool {
        self.transition_depth.load(Ordering::Acquire) > 0
    }

    fn register_dispatch(
        &self,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Option<LiveDispatchPermit> {
        if self.transition_pending() {
            return None;
        }
        let id = self.next_dispatch_id.fetch_add(1, Ordering::Relaxed);
        self.active_dispatches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id, cancel.clone());
        let permit = LiveDispatchPermit {
            id,
            active_dispatches: self.active_dispatches.clone(),
        };
        if self.transition_pending() {
            cancel.cancel();
            return None;
        }
        Some(permit)
    }

    fn with_live_project<T>(&self, operation: impl FnOnce() -> T) -> Option<T> {
        let _identity = self.core.lock_project_identity_workflow();
        if self.transition_pending() || self.core.runtime_snapshot().project_dir.is_none() {
            return None;
        }
        Some(operation())
    }

    fn with_live_dispatch<T>(
        &self,
        cancel: &opentake_media::MediaCancelToken,
        operation: impl FnOnce() -> T,
    ) -> Option<T> {
        self.with_live_dispatch_inner(cancel, || {}, operation)
    }

    #[cfg(test)]
    fn with_live_dispatch_after_admission<T>(
        &self,
        cancel: &opentake_media::MediaCancelToken,
        after_admission: impl FnOnce(),
        operation: impl FnOnce() -> T,
    ) -> Option<T> {
        self.with_live_dispatch_inner(cancel, after_admission, operation)
    }

    fn with_live_dispatch_inner<T>(
        &self,
        cancel: &opentake_media::MediaCancelToken,
        after_admission: impl FnOnce(),
        operation: impl FnOnce() -> T,
    ) -> Option<T> {
        let admitted_generation = self.identity_generation.load(Ordering::Acquire);
        if self.transition_pending() || cancel.is_cancelled() {
            return None;
        }
        after_admission();
        // Acquire the identity lease before admission. A transition announces
        // `pending=true` before waiting for this reader, so registration and the
        // second pending check cannot fall through from project A into B.
        let _identity = self.core.lock_project_identity_workflow();
        if self.transition_pending()
            || cancel.is_cancelled()
            || self.identity_generation.load(Ordering::Acquire) != admitted_generation
        {
            return None;
        }
        let expected = self.core.runtime_snapshot();
        expected.project_dir.as_ref()?;
        let _permit = self.register_dispatch(cancel)?;
        if self.transition_pending() || cancel.is_cancelled() {
            return None;
        }
        let result = operation();
        let current = self.core.runtime_snapshot();
        if current.project_epoch != expected.project_epoch
            || current.project_dir != expected.project_dir
        {
            cancel.cancel();
            return None;
        }
        Some(result)
    }
}

impl ChatTurnGate for LiveProjectMcpGate {
    fn timeline(&self, dispatcher: &Dispatcher) -> Option<opentake_domain::Timeline> {
        self.with_live_project(|| dispatcher.timeline())
    }

    fn dispatch(
        &self,
        dispatcher: &Dispatcher,
        name: &str,
        args: serde_json::Value,
    ) -> Option<ToolResult> {
        let cancel = opentake_media::MediaCancelToken::new();
        self.dispatch_cancellable(dispatcher, name, args, &cancel)
    }

    fn dispatch_cancellable(
        &self,
        dispatcher: &Dispatcher,
        name: &str,
        args: serde_json::Value,
        request_cancel: &opentake_media::MediaCancelToken,
    ) -> Option<ToolResult> {
        let (expected_epoch, expected_dir, receipt) =
            self.with_live_dispatch(request_cancel, || {
                let snapshot = self.core.runtime_snapshot();
                (
                    snapshot.project_epoch,
                    snapshot.project_dir,
                    dispatcher.dispatch_cancellable_deferred(name, args, request_cancel),
                )
            })?;
        // GPU work happens after `with_live_dispatch` releases the project
        // identity workflow read lease.
        let result = dispatcher.finish_dispatch(receipt, request_cancel);
        let still_current = self.with_live_project(|| {
            let snapshot = self.core.runtime_snapshot();
            snapshot.project_epoch == expected_epoch && snapshot.project_dir == expected_dir
        })?;
        if !still_current || request_cancel.is_cancelled() {
            request_cancel.cancel();
            return None;
        }
        Some(result)
    }

    fn dispatch_cancellable_scoped(
        &self,
        dispatcher: &Dispatcher,
        name: &str,
        args: serde_json::Value,
        undo_scope: &str,
        request_cancel: &opentake_media::MediaCancelToken,
    ) -> Option<ToolResult> {
        let (expected_epoch, expected_dir, receipt) =
            self.with_live_dispatch(request_cancel, || {
                let snapshot = self.core.runtime_snapshot();
                (
                    snapshot.project_epoch,
                    snapshot.project_dir,
                    dispatcher.dispatch_cancellable_scoped_deferred(
                        undo_scope,
                        name,
                        args,
                        request_cancel,
                    ),
                )
            })?;
        let result = dispatcher.finish_dispatch(receipt, request_cancel);
        let still_current = self.with_live_project(|| {
            let snapshot = self.core.runtime_snapshot();
            snapshot.project_epoch == expected_epoch && snapshot.project_dir == expected_dir
        })?;
        if !still_current || request_cancel.is_cancelled() {
            request_cancel.cancel();
            return None;
        }
        Some(result)
    }
}

pub(crate) fn build_media_bridge(
    core: AppCore,
    cache_root: PathBuf,
    models_dir: PathBuf,
) -> Arc<dyn MediaBridge> {
    Arc::new(TauriMediaBridge::new(core, cache_root, models_dir))
}

pub(crate) fn build_motion_document_bridge(
    core: AppCore,
    cache_root: PathBuf,
    notify: Option<MotionDocumentNotifier>,
) -> Arc<dyn MotionDocumentBridge> {
    let documents = Arc::new(crate::motion_documents::MotionDocumentStore::new(
        core.clone(),
    ));
    let motion = Arc::new(crate::motion::TauriMotionBridge::new(
        core.clone(),
        cache_root,
    ));
    let active = Arc::new(Mutex::new(
        HashMap::<u64, opentake_media::MediaCancelToken>::new(),
    ));
    let transition_active = active.clone();
    core.subscribe_project_identity_transition(move |pending| {
        if pending {
            for cancel in transition_active
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .values()
            {
                cancel.cancel();
            }
        }
    });
    Arc::new(TauriMotionDocumentBridge {
        documents,
        motion,
        active,
        next_operation: Arc::new(AtomicU64::new(1)),
        notify,
    })
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MotionDocumentChange {
    pub project_epoch: u64,
    pub project_path: String,
    pub summary: MotionDocumentChangeSummary,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MotionDocumentChangeSummary {
    pub id: String,
    pub title: String,
    pub revision_hash: String,
    pub updated_at: u64,
}

pub(crate) type MotionDocumentNotifier = Arc<dyn Fn(&MotionDocumentChange) + Send + Sync>;

struct TauriMotionDocumentBridge {
    documents: Arc<crate::motion_documents::MotionDocumentStore>,
    motion: Arc<crate::motion::TauriMotionBridge>,
    active: Arc<Mutex<HashMap<u64, opentake_media::MediaCancelToken>>>,
    next_operation: Arc<AtomicU64>,
    notify: Option<MotionDocumentNotifier>,
}

struct TauriMotionDocumentOperation {
    authority: ProjectAssetAuthority,
    request: MotionDocumentRequest,
    documents: Arc<crate::motion_documents::MotionDocumentStore>,
    motion: Arc<crate::motion::TauriMotionBridge>,
    active: Arc<Mutex<HashMap<u64, opentake_media::MediaCancelToken>>>,
    next_operation: Arc<AtomicU64>,
    notify: Option<MotionDocumentNotifier>,
}

struct ActiveMotionDocumentPermit {
    id: u64,
    active: Arc<Mutex<HashMap<u64, opentake_media::MediaCancelToken>>>,
}

impl Drop for ActiveMotionDocumentPermit {
    fn drop(&mut self) {
        self.active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.id);
    }
}

impl MotionDocumentBridge for TauriMotionDocumentBridge {
    fn can_edit_motion_documents(&self) -> bool {
        self.motion.can_render_motion()
    }

    fn admit(
        &self,
        request: MotionDocumentRequest,
    ) -> Result<Box<dyn AdmittedMotionDocumentOperation>, MotionDocumentBridgeError> {
        let authority = self
            .documents
            .capture_authority()
            .map_err(map_motion_document_store_error)?;
        Ok(Box::new(TauriMotionDocumentOperation {
            authority,
            request,
            documents: self.documents.clone(),
            motion: self.motion.clone(),
            active: self.active.clone(),
            next_operation: self.next_operation.clone(),
            notify: self.notify.clone(),
        }))
    }
}

impl AdmittedMotionDocumentOperation for TauriMotionDocumentOperation {
    fn execute(
        self: Box<Self>,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<MotionDocumentResponse, MotionDocumentBridgeError> {
        if cancel.is_cancelled() {
            return Err(MotionDocumentBridgeError::new(
                MotionDocumentBridgeErrorKind::Cancelled,
                "Motion Studio operation was cancelled",
            ));
        }
        let id = self.next_operation.fetch_add(1, Ordering::Relaxed);
        self.active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id, cancel.clone());
        let _permit = ActiveMotionDocumentPermit {
            id,
            active: self.active.clone(),
        };
        self.execute_inner(cancel)
    }
}

impl TauriMotionDocumentOperation {
    fn execute_inner(
        &self,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<MotionDocumentResponse, MotionDocumentBridgeError> {
        match &self.request {
            MotionDocumentRequest::List => self
                .documents
                .list_for_authority(self.authority.clone())
                .map(|items| {
                    MotionDocumentResponse::Documents(
                        items.into_iter().map(agent_motion_summary).collect(),
                    )
                })
                .map_err(map_motion_document_store_error),
            MotionDocumentRequest::Read { document_id } => self
                .documents
                .read_for_authority(self.authority.clone(), document_id)
                .map(agent_motion_document)
                .map(MotionDocumentResponse::Document)
                .map_err(map_motion_document_store_error),
            MotionDocumentRequest::Create { title } => self
                .documents
                .create_for_authority_cancellable(
                    self.authority.clone(),
                    crate::motion_documents::MotionDocumentCreateRequest {
                        title: title.clone(),
                    },
                    cancel,
                )
                .map(agent_motion_document)
                .map(|document| self.document_changed(document))
                .map(MotionDocumentResponse::Document)
                .map_err(map_motion_document_store_error),
            MotionDocumentRequest::Patch(request) => self.patch(request, cancel),
            MotionDocumentRequest::Preview(request) => self.preview(request, cancel),
            MotionDocumentRequest::Publish(request) => self.publish(request, cancel),
        }
    }

    fn patch(
        &self,
        request: &opentake_agent::mcp::motion_documents::MotionDocumentPatchRequest,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<MotionDocumentResponse, MotionDocumentBridgeError> {
        let edits = request
            .edits
            .iter()
            .map(|edit| crate::motion_documents::MotionTextReplacement {
                start: edit.start,
                end: edit.end,
                replacement: edit.replacement.clone(),
            })
            .collect::<Vec<_>>();
        let expected = match self.documents.hash_patch_for_authority(
            self.authority.clone(),
            crate::motion_documents::MotionDocumentHashRequest {
                document_id: request.document_id.clone(),
                file: request.file.clone(),
                baseline_hash: request.baseline_hash.clone(),
                edits: edits.clone(),
            },
        ) {
            Ok(hash) => hash,
            Err(error) if error.contains("revision conflict") => {
                return Err(self.conflict(&request.document_id))
            }
            Err(error) => return Err(map_motion_document_store_error(error)),
        };
        if cancel.is_cancelled() {
            return Err(MotionDocumentBridgeError::new(
                MotionDocumentBridgeErrorKind::Cancelled,
                "Motion Studio patch was cancelled",
            ));
        }
        match self.documents.save_patch_for_authority_cancellable(
            self.authority.clone(),
            crate::motion_documents::MotionDocumentPatchRequest {
                document_id: request.document_id.clone(),
                file: request.file.clone(),
                baseline_hash: request.baseline_hash.clone(),
                edits,
                expected_result_hash: expected,
            },
            cancel,
        ) {
            Ok(document) => Ok(MotionDocumentResponse::Document(
                self.document_changed(agent_motion_document(document)),
            )),
            Err(error) if error.contains("revision conflict") => {
                Err(self.conflict(&request.document_id))
            }
            Err(error) => Err(map_motion_document_store_error(error)),
        }
    }

    fn document_changed(&self, document: AgentMotionDocument) -> AgentMotionDocument {
        if let Some(notify) = &self.notify {
            notify(&MotionDocumentChange {
                project_epoch: self.authority.project_epoch,
                project_path: self.authority.project_path.to_string_lossy().into_owned(),
                summary: MotionDocumentChangeSummary {
                    id: document.summary.document_id.clone(),
                    title: document.summary.title.clone(),
                    revision_hash: document.summary.revision_hash.clone(),
                    updated_at: document.summary.updated_at,
                },
            });
        }
        document
    }

    fn conflict(&self, document_id: &str) -> MotionDocumentBridgeError {
        let current = self
            .documents
            .read_for_authority(self.authority.clone(), document_id)
            .ok()
            .map(|document| document.summary.revision_hash);
        MotionDocumentBridgeError::conflict(current)
    }

    fn preview(
        &self,
        request: &opentake_agent::mcp::motion_documents::MotionDocumentPreviewRequest,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<MotionDocumentResponse, MotionDocumentBridgeError> {
        let response = crate::motion::render_document_preview_for_agent(
            &self.motion,
            &self.documents,
            self.authority.clone(),
            crate::motion::MotionPreviewRequest {
                document_id: request.document_id.clone(),
                revision_hash: request.revision_hash.clone(),
                width: request.width,
                height: request.height,
                fps: request.fps,
                duration_frames: request.duration_frames,
                frame: request.frame,
            },
            cancel,
        )
        .map_err(map_motion_preview_error)?;
        let png_base64 = response
            .png_data_url
            .strip_prefix("data:image/png;base64,")
            .ok_or_else(|| {
                MotionDocumentBridgeError::new(
                    MotionDocumentBridgeErrorKind::RenderFailed,
                    "Motion Studio preview returned an invalid image",
                )
            })?
            .to_string();
        Ok(MotionDocumentResponse::Preview(
            AgentMotionDocumentPreview {
                revision_hash: response.revision_hash,
                frame: response.frame,
                png_base64,
                diagnostics: response
                    .diagnostics
                    .into_iter()
                    .map(|diagnostic| AgentMotionPreviewDiagnostic {
                        severity: diagnostic.severity.to_string(),
                        message: diagnostic.message,
                        line: diagnostic.line,
                        column: diagnostic.column,
                    })
                    .collect(),
            },
        ))
    }

    fn publish(
        &self,
        request: &opentake_agent::mcp::motion_documents::MotionDocumentPublishRequest,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<MotionDocumentResponse, MotionDocumentBridgeError> {
        let source = crate::motion::resolve_document_motion_source(
            &self.documents,
            self.authority.clone(),
            &request.document_id,
            &request.revision_hash,
        )
        .map_err(map_motion_bridge_error)?;
        let commit = if let Some(clip_id) = &request.clip_id {
            self.motion.edit_document(
                crate::motion::DocumentMotionEditRequest {
                    clip_id: clip_id.clone(),
                    source,
                    project_authority: self.authority.clone(),
                    width: request.width,
                    height: request.height,
                    fps: request.fps,
                    duration_frames: request.duration_frames,
                },
                cancel,
            )
        } else {
            self.motion.add_document(
                crate::motion::DocumentMotionAddRequest {
                    source,
                    project_authority: self.authority.clone(),
                    width: request.width,
                    height: request.height,
                    fps: request.fps,
                    start_frame: request.start_frame.expect("validated at Agent boundary"),
                    duration_frames: request.duration_frames,
                    track_index: request.track_index,
                    transparent: request.transparent,
                },
                cancel,
            )
        }
        .map_err(map_motion_bridge_error)?;
        let source_document = commit.source_document.ok_or_else(|| {
            MotionDocumentBridgeError::new(
                MotionDocumentBridgeErrorKind::RenderFailed,
                "Motion Studio publish lost its source revision",
            )
        })?;
        Ok(MotionDocumentResponse::Published(
            AgentMotionDocumentPublish {
                clip_id: commit.clip_id,
                asset_id: commit.asset_id,
                duration_frames: commit.output.duration_frames,
                duration_seconds: commit.output.duration_seconds,
                fps: commit.output.fps,
                width: commit.output.width,
                height: commit.output.height,
                source_document: AgentMotionDocumentReference {
                    document_id: source_document.document_id,
                    revision_hash: source_document.revision_hash,
                },
            },
        ))
    }
}

fn agent_motion_summary(
    summary: crate::motion_documents::MotionDocumentSummary,
) -> AgentMotionDocumentSummary {
    AgentMotionDocumentSummary {
        document_id: summary.id,
        title: summary.title,
        revision_hash: summary.revision_hash,
        updated_at: summary.updated_at,
    }
}

fn agent_motion_document(document: crate::motion_documents::MotionDocument) -> AgentMotionDocument {
    AgentMotionDocument {
        summary: agent_motion_summary(document.summary),
        html: document.html,
        css: document.css,
        parameters: document.parameters,
    }
}

fn map_motion_document_store_error(error: String) -> MotionDocumentBridgeError {
    let kind = if error.contains("revision conflict") {
        MotionDocumentBridgeErrorKind::Conflict
    } else if error.contains("not found") {
        MotionDocumentBridgeErrorKind::ResourceNotFound
    } else if error.contains("changed") || error.contains("cancel") {
        MotionDocumentBridgeErrorKind::Cancelled
    } else if error.contains("invalid")
        || error.contains("must")
        || error.contains("requires")
        || error.contains("limit")
    {
        MotionDocumentBridgeErrorKind::InvalidArguments
    } else {
        MotionDocumentBridgeErrorKind::CapabilityUnavailable
    };
    MotionDocumentBridgeError::new(kind, error)
}

fn map_motion_preview_error(error: crate::motion::MotionPreviewError) -> MotionDocumentBridgeError {
    let kind = if error.message.contains("cancel") || error.message.contains("project changed") {
        MotionDocumentBridgeErrorKind::Cancelled
    } else if error.message.contains("changed; reload") {
        MotionDocumentBridgeErrorKind::Conflict
    } else if error.message.contains("invalid") || error.message.contains("inside") {
        MotionDocumentBridgeErrorKind::InvalidArguments
    } else {
        MotionDocumentBridgeErrorKind::RenderFailed
    };
    MotionDocumentBridgeError::new(kind, error.message)
}

fn map_motion_bridge_error(
    error: opentake_agent::mcp::motion::MotionBridgeError,
) -> MotionDocumentBridgeError {
    let kind = match error.kind {
        opentake_agent::mcp::motion::MotionBridgeErrorKind::InvalidArguments => {
            MotionDocumentBridgeErrorKind::InvalidArguments
        }
        opentake_agent::mcp::motion::MotionBridgeErrorKind::ResourceNotFound => {
            MotionDocumentBridgeErrorKind::ResourceNotFound
        }
        opentake_agent::mcp::motion::MotionBridgeErrorKind::CapabilityUnavailable => {
            MotionDocumentBridgeErrorKind::CapabilityUnavailable
        }
        opentake_agent::mcp::motion::MotionBridgeErrorKind::Cancelled => {
            MotionDocumentBridgeErrorKind::Cancelled
        }
        opentake_agent::mcp::motion::MotionBridgeErrorKind::RenderFailed => {
            MotionDocumentBridgeErrorKind::RenderFailed
        }
    };
    MotionDocumentBridgeError::new(kind, error.message)
}

/// The production [`MediaBridge`]: composites timeline frames on the GPU and
/// imports media through the same path as the media panel.
struct TauriMediaBridge {
    /// A session-sharing clone of the authoritative core (import + snapshot).
    core: AppCore,
    /// Media engine over the UI's cache/models dirs — probing + poster warming on
    /// import go through this, so imported assets are cached exactly like the
    /// panel's. Built here (the engine is not `Clone`) from the same paths.
    engine: MediaEngine,
    /// Dedicated compositor state for post-commit agent images. It is isolated
    /// from UI preview scheduling while still reusing its GPU context per turn.
    render: crate::render::RenderState,
}

struct RetainedExternalSource {
    path: PathBuf,
    parent: Dir,
    name: std::ffi::OsString,
    handle: Handle,
}

fn retained_external_open_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use cap_std::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
    }
    options
}

impl RetainedExternalSource {
    fn open(path: &Path) -> Result<Self, BridgeError> {
        let name = path
            .file_name()
            .ok_or_else(|| BridgeError::new("source.path must name one regular file"))?
            .to_owned();
        let parent_path = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty());
        let parent = Dir::open_ambient_dir(
            parent_path.unwrap_or_else(|| Path::new(".")),
            ambient_authority(),
        )
        .map_err(|_| {
            BridgeError::new("MCP_SOURCE_PATH_UNREADABLE: source.path parent is unavailable")
        })?;
        let options = retained_external_open_options();
        let file = parent.open_with(&name, &options).map_err(|_| {
            BridgeError::new(
                "MCP_SOURCE_PATH_UNREADABLE: source.path is not a readable no-follow file",
            )
        })?;
        let metadata = file
            .metadata()
            .map_err(|_| BridgeError::new("MCP_SOURCE_PATH_UNREADABLE: metadata failed"))?;
        if crate::media::capability_metadata_is_symlink_or_reparse(&metadata) || !metadata.is_file()
        {
            return Err(BridgeError::new(
                "MCP_SOURCE_PATH_UNREADABLE: source.path must be a regular file",
            ));
        }
        let handle = Handle::from_file(file.into_std()).map_err(|_| {
            BridgeError::new("MCP_SOURCE_PATH_UNREADABLE: source.path identity unavailable")
        })?;
        Ok(Self {
            path: path.to_owned(),
            parent,
            name,
            handle,
        })
    }

    fn file(&self) -> &std::fs::File {
        self.handle.as_file()
    }

    fn matches_path(&self) -> Result<bool, String> {
        let options = retained_external_open_options();
        let current = self
            .parent
            .open_with(&self.name, &options)
            .map_err(|error| error.to_string())?;
        let metadata = current.metadata().map_err(|error| error.to_string())?;
        if crate::media::capability_metadata_is_symlink_or_reparse(&metadata) || !metadata.is_file()
        {
            return Ok(false);
        }
        let current = Handle::from_file(current.into_std()).map_err(|error| error.to_string())?;
        Ok(current == self.handle)
    }
}

impl TauriMediaBridge {
    fn new(core: AppCore, cache_root: PathBuf, models_dir: PathBuf) -> Self {
        TauriMediaBridge {
            core,
            engine: MediaEngine::new(cache_root, models_dir),
            render: crate::render::RenderState::new(),
        }
    }
}

struct ResolvedTranscriptSource {
    source: TranscriptSource,
    resolved: Result<(PathBuf, bool), String>,
}

fn resolve_transcript_batch(
    snapshot: &ProjectRuntimeSnapshot,
    sources: &[TranscriptSource],
) -> Vec<ResolvedTranscriptSource> {
    sources
        .iter()
        .cloned()
        .map(|source| {
            let resolved =
                crate::transcribe::resolve_asset_from_snapshot(snapshot, &source.media_ref);
            ResolvedTranscriptSource { source, resolved }
        })
        .collect()
}

impl MediaBridge for TauriMediaBridge {
    fn visible_timeline_clip_count(
        &self,
        timeline: &opentake_domain::Timeline,
    ) -> Result<usize, BridgeError> {
        crate::render::authoritative_visible_clip_count(timeline, &self.core.media())
            .map_err(BridgeError::new)
    }

    fn capture_timeline_result(
        &self,
        request: &TimelineResultCaptureRequest,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<Block, BridgeError> {
        let expected = request
            .mutation
            .committed_revision
            .as_ref()
            .ok_or_else(|| BridgeError::new("timeline result capture lacks a project revision"))?;
        if request.mutation.visible_clip_count_before == 0
            || request.mutation.visible_clip_count_after != 0
        {
            return Err(BridgeError::new(
                "timeline result capture receipt is not a visible-to-empty transition",
            ));
        }

        // Snapshot under the core's internal lock, then release it before GPU
        // work. The complete identity and timeline are rechecked before bytes
        // cross the bridge boundary.
        let snapshot = self.core.runtime_snapshot();
        if snapshot.project_epoch != expected.project_epoch
            || snapshot.version != expected.timeline_version
            || snapshot.project_dir != expected.project_dir
            || snapshot.timeline != request.timeline
        {
            return Err(BridgeError::new(
                "timeline result capture project revision was superseded",
            ));
        }
        if crate::render::authoritative_visible_clip_count(&snapshot.timeline, &snapshot.media)
            .map_err(BridgeError::new)?
            != 0
        {
            return Err(BridgeError::new(
                "timeline result capture snapshot is not empty",
            ));
        }

        let input = crate::render::EmptyTimelineCanvasInput {
            project_width: snapshot.timeline.width,
            project_height: snapshot.timeline.height,
            fps: snapshot.timeline.fps,
            playhead_frame: crate::render::root_timeline_playhead(snapshot.project_epoch),
        };
        let authority = crate::render::CompositeSourceAuthority::new(HashMap::new());
        let rendered = crate::render::render_timeline_result_png(
            &snapshot.timeline,
            &snapshot.media,
            &snapshot.project_dir,
            &self.render,
            input,
            cancel,
            &authority,
        )
        .map_err(BridgeError::new)?;

        let current = self.core.runtime_snapshot();
        if current.project_epoch != expected.project_epoch
            || current.version != expected.timeline_version
            || current.project_dir != expected.project_dir
            || current.timeline != request.timeline
        {
            return Err(BridgeError::new(
                "timeline result capture project changed during rendering",
            ));
        }
        let base64 = base64::engine::general_purpose::STANDARD.encode(rendered.bytes);
        if base64.is_empty() || base64.len() > TIMELINE_RESULT_IMAGE_BASE64_MAX {
            return Err(BridgeError::new(
                "timeline result capture exceeded the response limit",
            ));
        }
        Ok(Block::image(base64, rendered.media_type))
    }

    fn inspect_media(
        &self,
        request: &InspectMediaRequest,
    ) -> Result<InspectMediaResult, BridgeError> {
        inspect_source_media(&self.core, &self.engine, request)
    }

    fn inspect_timeline(
        &self,
        frames: &[i32],
        max_longest_edge: u32,
    ) -> Result<InspectResult, BridgeError> {
        // Snapshot the live session, then composite off the session lock (the
        // preview path's discipline; a local GPU context per call keeps this off
        // the preview's cached `RenderState` mutex, matching export.rs).
        let snapshot = self.core.runtime_snapshot();
        let timeline = snapshot.timeline;
        let manifest = snapshot.media;
        let project_dir = snapshot.project_dir;
        composite_frames_jpeg(&timeline, &manifest, &project_dir, frames, max_longest_edge)
    }

    fn transcribe_sources(
        &self,
        sources: &[TranscriptSource],
    ) -> Result<Vec<TranscriptSourceResult>, BridgeError> {
        // Per-source, skip-don't-fail (mirrors upstream's per-URL `catch { skipped
        // … }` loop): a missing file, an un-installed model, or a decode error
        // skips just that source with a reason — cached sources still return their
        // transcript, so a mostly-cached timeline never loses results to one bad
        // (or not-yet-transcribable) clip. The whisper backend loads lazily on the
        // first cache miss and is shared across the batch; a model-not-installed
        // failure is memoized so we don't retry the load per source.
        enum Backend {
            /// Not attempted yet.
            Unloaded,
            /// Loaded and ready.
            Ready(opentake_media::WhisperTranscriber),
            /// Load failed (e.g. model not installed); reason skipped per source.
            Failed(String),
        }
        let mut backend = Backend::Unloaded;
        let mut out = Vec::with_capacity(sources.len());
        let snapshot = self.core.runtime_snapshot();
        for resolved_source in resolve_transcript_batch(&snapshot, sources) {
            let src = resolved_source.source;
            let skip = |reason: String| TranscriptSourceResult {
                media_ref: src.media_ref.clone(),
                transcript: None,
                error: Some(reason),
            };
            // Resolve the asset path; a missing/offline source is skipped.
            let (path, is_video) = match resolved_source.resolved {
                Ok(resolved) => resolved,
                Err(reason) => {
                    out.push(skip(reason));
                    continue;
                }
            };
            // Cached full transcript short-circuits before the backend loads —
            // but only for the auto-detect (no language hint) case. A language
            // hint produces a different transcript than the cached auto one, so
            // it bypasses the cache (upstream `EditorViewModel+Captions.swift:127`).
            if src.language.is_none() {
                if let Some(cached) = opentake_media::transcribe::cache::cached_on_disk(
                    self.engine.cache_root(),
                    &path,
                ) {
                    out.push(TranscriptSourceResult {
                        media_ref: src.media_ref.clone(),
                        transcript: Some(cached),
                        error: None,
                    });
                    continue;
                }
            }
            // Lazily load the backend on the first cache miss; memoize failure.
            if let Backend::Unloaded = backend {
                backend = match crate::transcribe::load_backend(&self.engine) {
                    Ok(b) => Backend::Ready(b),
                    Err(e) => Backend::Failed(e),
                };
            }
            let b = match &backend {
                Backend::Ready(b) => b,
                Backend::Failed(reason) => {
                    out.push(skip(reason.clone()));
                    continue;
                }
                Backend::Unloaded => unreachable!("backend was just loaded above"),
            };
            // With a language hint, transcribe directly with the hint threaded to
            // the backend (the cache convenience uses auto-detect defaults). The
            // auto path keeps using the caching convenience so repeats are instant.
            let result = match &src.language {
                Some(lang) => {
                    let opts = opentake_media::TranscribeOptions {
                        preferred_language: Some(lang.clone()),
                        ..Default::default()
                    };
                    opentake_media::transcribe::transcribe_file(&path, b, &opts)
                        .map_err(|e| e.to_string())
                }
                None => {
                    let cache = opentake_media::TranscriptCache::new(self.engine.cache_root());
                    cache
                        .transcript(&path, is_video, None, b)
                        .map_err(|e| e.to_string())
                }
            };
            match result {
                Ok(t) => out.push(TranscriptSourceResult {
                    media_ref: src.media_ref.clone(),
                    transcript: Some(t),
                    error: None,
                }),
                Err(e) => out.push(skip(e)),
            }
        }
        Ok(out)
    }

    fn import_media(
        &self,
        source: ImportSource,
        name: Option<String>,
        folder_id: Option<String>,
    ) -> Result<ImportOutcome, BridgeError> {
        self.import_media_cancellable(
            source,
            name,
            folder_id,
            &opentake_media::MediaCancelToken::new(),
        )
    }

    fn import_media_cancellable(
        &self,
        source: ImportSource,
        name: Option<String>,
        folder_id: Option<String>,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<ImportOutcome, BridgeError> {
        match source {
            ImportSource::Path(path) => self.import_from_path_cancellable(
                &path,
                name.as_deref(),
                folder_id.as_deref(),
                cancel,
            ),
            ImportSource::Bytes { base64, mime_type } => self.import_from_bytes_cancellable(
                &base64,
                &mime_type,
                name.as_deref(),
                folder_id.as_deref(),
                cancel,
            ),
            ImportSource::Url { url, mime_type } => {
                let fetcher = ReqwestUrlFetcher::new()?;
                self.import_from_url_with(
                    &fetcher,
                    &url,
                    mime_type.as_deref(),
                    name.as_deref(),
                    folder_id.as_deref(),
                    URL_IMPORT_DECODED_MAX,
                    cancel,
                    |file, extension, kind| self.probe_required(file, extension, kind),
                    |project_media, manifest| {
                        project_media
                            .write_manifest(manifest)
                            .map_err(CoreError::Media)
                    },
                )
            }
        }
    }

    fn search_media(
        &self,
        candidates: &[SearchCandidate],
        query: &str,
        scope: &str,
        limit: usize,
    ) -> Result<SearchMediaResult, BridgeError> {
        // Resolve every candidate id to its source path from the live manifest.
        // Missing (offline) files are kept — their index/transcript reads simply
        // yield nothing, matching upstream (a missing file has no results, not an
        // error). Unresolvable ids are dropped.
        let snapshot = self.core.runtime_snapshot();
        let manifest = snapshot.media;
        let resolver =
            opentake_domain::MediaResolver::new(&manifest, snapshot.project_dir.as_deref());
        let mut visual_paths: Vec<(String, PathBuf)> = Vec::new();
        let mut spoken_paths: Vec<(String, PathBuf)> = Vec::new();
        for c in candidates {
            let Some(path) = resolver.expected_path(&c.media_ref) else {
                continue;
            };
            if c.is_visual {
                visual_paths.push((c.media_ref.clone(), path.clone()));
            }
            if c.is_spoken {
                spoken_paths.push((c.media_ref.clone(), path));
            }
        }

        let fps = snapshot.timeline.fps;
        let installed = crate::search::model_installed(&self.engine);

        // Visual group (skipped for scope == "spoken").
        let (status, indexable_assets, indexed_assets, moments) = if scope == "spoken" {
            (SearchIndexState::Disabled, 0, None, Vec::new())
        } else {
            let (indexable, indexed) = crate::search::visual_coverage(&self.engine, &visual_paths);
            // Status mirrors upstream `visualStatus`: without the model it's
            // `modelNotInstalled`; with it, `indexing` while any indexable asset
            // is still un-indexed, else `ready`. (Download/preparing/failed are
            // transient front-end states the panel owns; the tool reports the
            // stable installed/ready/indexing view.)
            let status = if !installed {
                SearchIndexState::ModelNotInstalled
            } else if indexable > 0 && indexed < indexable {
                SearchIndexState::Indexing
            } else {
                SearchIndexState::Ready
            };
            let moments: Vec<SearchVisualHit> =
                crate::search::visual_hits_by_id(&self.engine, &visual_paths, query, fps, limit)
                    .into_iter()
                    .map(|h| SearchVisualHit {
                        media_ref: h.media_id,
                        start_seconds: h.start_sec,
                        end_seconds: h.end_sec,
                        score: h.score,
                        is_image: h.is_image,
                    })
                    .collect();
            // `indexedAssets` is only meaningful when the model is loaded
            // (upstream sets it only when an embedder spec exists).
            let indexed_opt = if installed { Some(indexed) } else { None };
            (status, indexable, indexed_opt, moments)
        };

        // Spoken group (skipped for scope == "visual"). Works regardless of the
        // visual index — keyword search over cached transcripts.
        let spoken: Vec<SearchSpokenHit> = if scope == "visual" {
            Vec::new()
        } else {
            self.engine
                .search_spoken(query, &spoken_paths, limit)
                .into_iter()
                .map(|h| SearchSpokenHit {
                    media_ref: h.asset_id,
                    start_seconds: h.start,
                    end_seconds: h.end,
                    text: h.text,
                })
                .collect()
        };

        Ok(SearchMediaResult {
            status,
            indexable_assets,
            indexed_assets,
            moments,
            spoken,
        })
    }
}

impl TauriMediaBridge {
    fn probe_required(
        &self,
        file: &std::fs::File,
        expected_extension: &str,
        expected_kind: &str,
    ) -> Result<ProbedMedia, BridgeError> {
        let probe = self.engine.probe_file(file).map_err(|_| {
            BridgeError::new(
                "MCP_MEDIA_PROBE_FAILED: Downloaded media could not be validated; verify the source type and retry",
            )
        })?;
        let actual_kind = if probe.has_video {
            if probe.duration_secs > 0.0 {
                "video"
            } else {
                "image"
            }
        } else if probe.has_audio {
            "audio"
        } else {
            return Err(BridgeError::new(
                "Downloaded bytes contain no supported audio, video, or image stream",
            ));
        };
        if actual_kind != expected_kind {
            return Err(BridgeError::new(format!(
                "Downloaded media type '{actual_kind}' conflicts with declared '{expected_kind}'"
            )));
        }
        let format_name = probe.format_name.as_deref().ok_or_else(|| {
            BridgeError::new("Downloaded media probe did not identify a container format")
        })?;
        if !container_matches_extension(format_name, expected_extension) {
            return Err(BridgeError::new(format!(
                "Downloaded container '{format_name}' conflicts with '.{expected_extension}'"
            )));
        }
        Ok(ProbedMedia {
            duration_secs: probe.duration_secs,
            width: probe.width.map(|value| value as i32),
            height: probe.height.map(|value| value as i32),
            fps: probe.fps,
            has_audio: probe.has_audio,
            color: probe.color,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn import_from_url_with<F, P, W>(
        &self,
        fetcher: &F,
        raw_url: &str,
        requested_mime: Option<&str>,
        requested_name: Option<&str>,
        folder_id: Option<&str>,
        decoded_limit: u64,
        cancel: &opentake_media::MediaCancelToken,
        probe: P,
        mut write_manifest: W,
    ) -> Result<ImportOutcome, BridgeError>
    where
        F: UrlFetcher,
        P: FnOnce(&std::fs::File, &str, &str) -> Result<ProbedMedia, BridgeError>,
        W: FnMut(&ProjectMediaCapability, &opentake_domain::MediaManifest) -> Result<(), CoreError>,
    {
        let mut current = validate_https_url(raw_url)?;
        let mut redirects = 0_usize;
        let (final_url, mut response) = loop {
            cancelled_checkpoint(cancel)?;
            let response = match fetcher.fetch(&current, cancel) {
                Ok(response) => response,
                Err(error) => {
                    cancelled_checkpoint(cancel)?;
                    return Err(error);
                }
            };
            if is_followed_redirect(response.status) {
                let location = response.location.as_deref().ok_or_else(|| {
                    BridgeError::new(format!(
                        "source.url redirect {} is missing Location",
                        response.status
                    ))
                })?;
                let next = current.join(location).map_err(|error| {
                    BridgeError::new(format!("source.url redirect is invalid: {error}"))
                })?;
                validate_parsed_https_url(&next)?;
                if redirects >= URL_IMPORT_REDIRECT_MAX {
                    return Err(BridgeError::new(format!(
                        "source.url exceeded {URL_IMPORT_REDIRECT_MAX} redirects"
                    )));
                }
                redirects += 1;
                current = next;
                continue;
            }
            if response.status.is_redirection() {
                return Err(BridgeError::new(format!(
                    "source.url returned unsupported redirect status {}",
                    response.status
                )));
            }
            if !response.status.is_success() {
                return Err(BridgeError::new(format!(
                    "source.url returned HTTP {}",
                    response.status
                )));
            }
            break (current, response);
        };

        let (extension, _response_mime, expected_kind) =
            resolve_url_media_type(&final_url, requested_mime, response.content_type.as_deref())?;
        if let Some(length) = response.content_length {
            if length > decoded_limit {
                return Err(BridgeError::new(format!(
                    "source.url Content-Length is too large: {length} bytes, max {decoded_limit}"
                )));
            }
        }

        self.core
            .ensure_project_mutable()
            .map_err(|error| BridgeError::new(error.to_string()))?;
        let project = self.core.runtime_snapshot();
        let project_dir = project
            .project_dir
            .clone()
            .ok_or_else(|| BridgeError::new("No project is open; cannot import source.url"))?;
        let project_media = ProjectMediaCapability::open_verified(
            &self.core,
            project.project_epoch,
            &project_dir,
            true,
        )
        .map_err(BridgeError::new)?;
        let leaf_name = format!("imported-url-{}.{extension}", uuid::Uuid::new_v4());
        let mut staged = project_media
            .create_import(Path::new(&leaf_name))
            .map_err(BridgeError::new)?;

        let mut total = 0_u64;
        loop {
            cancelled_checkpoint(cancel)?;
            let Some(chunk) = response.body.next_chunk(cancel)? else {
                break;
            };
            total = total
                .checked_add(chunk.len() as u64)
                .ok_or_else(|| BridgeError::new("source.url decoded byte count overflowed"))?;
            if total > decoded_limit {
                return Err(BridgeError::new(format!(
                    "source.url decoded payload is too large: {total} bytes, max {decoded_limit}"
                )));
            }
            staged.file_mut().write_all(&chunk).map_err(|error| {
                BridgeError::new(format!("Failed to stage source.url: {error}"))
            })?;
        }
        if total == 0 {
            return Err(BridgeError::new("source.url returned an empty body"));
        }
        staged
            .file_mut()
            .flush()
            .map_err(|error| BridgeError::new(format!("Failed to flush source.url: {error}")))?;
        staged
            .file()
            .sync_all()
            .map_err(|error| BridgeError::new(format!("Failed to sync source.url: {error}")))?;
        staged
            .file_mut()
            .seek(SeekFrom::Start(0))
            .map_err(|error| BridgeError::new(format!("Failed to rewind source.url: {error}")))?;
        cancelled_checkpoint(cancel)?;
        if !project_media
            .matches_leaf(&staged)
            .map_err(BridgeError::new)?
        {
            return Err(BridgeError::new(
                "source.url staging identity changed before probe",
            ));
        }
        let probed = probe(staged.file(), &extension, expected_kind)?;
        cancelled_checkpoint(cancel)?;
        if !project_media
            .matches_leaf(&staged)
            .map_err(BridgeError::new)?
        {
            return Err(BridgeError::new(
                "source.url staging identity changed during probe",
            ));
        }

        let display_name = requested_name
            .map(str::to_owned)
            .or_else(|| url_display_name(&final_url))
            .unwrap_or_else(|| "Imported Media".to_string());
        let mut events = DeferredCoreEvents::default();
        let commit = self
            .core
            .import_retained_media_for_project_deferred_with_manifest_writer(
                project.project_epoch,
                &project_dir,
                staged.path(),
                display_name,
                &probed,
                folder_id,
                &mut events,
                |manifest| write_manifest(&project_media, manifest),
                || {
                    if cancel.checkpoint() {
                        return Err(CoreError::Media(
                            "source.url import was cancelled before publication".to_string(),
                        ));
                    }
                    match project_media.matches_leaf(&staged) {
                        Ok(true) => Ok(()),
                        Ok(false) => Err(CoreError::Media(
                            "source.url staging identity changed during publication".to_string(),
                        )),
                        Err(error) => Err(CoreError::Media(format!(
                            "source.url staging identity check failed during publication: {error}"
                        ))),
                    }
                },
            )
            .map_err(|error| BridgeError::new(error.to_string()))?;
        staged.commit();
        self.core.emit_deferred(events);
        let recovery_required = commit.warning.is_some();
        Ok(ImportOutcome {
            asset_count: 1,
            folder_count: 0,
            recovery_required,
        })
    }

    /// `path` import: in place, mirroring directories recursively — the exact
    /// `crate::media` path the media panel uses (`import_one` / `mirror_dir`), so
    /// posters/manifest/events stay consistent. 1:1 with upstream
    /// `ToolExecutor+Import.importFromPath`.
    #[cfg(test)]
    fn import_from_path(
        &self,
        path: &str,
        name: Option<&str>,
        folder_id: Option<&str>,
    ) -> Result<ImportOutcome, BridgeError> {
        self.import_from_path_cancellable(
            path,
            name,
            folder_id,
            &opentake_media::MediaCancelToken::new(),
        )
    }

    fn import_from_path_cancellable(
        &self,
        path: &str,
        name: Option<&str>,
        folder_id: Option<&str>,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<ImportOutcome, BridgeError> {
        self.import_from_path_cancellable_with_hook(path, name, folder_id, cancel, || {})
    }

    fn import_from_path_cancellable_with_hook(
        &self,
        path: &str,
        name: Option<&str>,
        folder_id: Option<&str>,
        cancel: &opentake_media::MediaCancelToken,
        before_commit: impl FnOnce(),
    ) -> Result<ImportOutcome, BridgeError> {
        cancelled_checkpoint(cancel)?;
        self.core
            .ensure_project_mutable()
            .map_err(|error| BridgeError::new(error.to_string()))?;
        let project = self.core.runtime_snapshot();
        let project_dir = project
            .project_dir
            .clone()
            .ok_or_else(|| BridgeError::new("No project is open; cannot import source.path"))?;
        let file_url = PathBuf::from(path);
        let meta = std::fs::symlink_metadata(&file_url).map_err(|_| {
            BridgeError::new(
                "MCP_SOURCE_PATH_UNREADABLE: source.path does not exist or is not readable",
            )
        })?;

        if meta.file_type().is_symlink() {
            return Err(BridgeError::new(
                "MCP_SOURCE_PATH_UNREADABLE: source.path symbolic links are not allowed",
            ));
        }

        if meta.is_dir() {
            // Recursive directory import (剪注-style folder mirroring). Reuse the
            // media panel's `mirror_dir`; count what actually landed.
            let before_entries = self.core.media().entries.len();
            let before_folders = self.core.media().folders.len();
            let mut skipped = Vec::new();
            let parent = folder_id.map(|s| s.to_string());
            before_commit();
            crate::media::mirror_dir_cancellable(
                &self.core,
                &self.engine,
                &file_url,
                parent,
                &mut skipped,
                cancel,
            )
            .map_err(|error| BridgeError::new(error.to_string()))?;
            let after = self.core.media();
            let asset_count = after.entries.len().saturating_sub(before_entries);
            let folder_count = after.folders.len().saturating_sub(before_folders);
            if asset_count == 0 {
                return Err(BridgeError::new(format!(
                    "No supported media found in folder: {path}"
                )));
            }
            return Ok(ImportOutcome {
                asset_count,
                folder_count,
                recovery_required: false,
            });
        }

        if !meta.is_file() {
            return Err(BridgeError::new(
                "MCP_SOURCE_PATH_UNREADABLE: source.path must be a regular file or directory",
            ));
        }

        // Single file. Validate the extension up front for upstream's precise
        // error (`import_one` would just skip an unsupported file).
        let ext = file_url
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();
        if importable_clip_type(&file_url).is_none() {
            return Err(BridgeError::new(format!(
                "Unsupported file extension '.{ext}'. Supported: mov/mp4/m4v, mp3/wav/aac/m4a, png/jpg/jpeg/tiff/heic."
            )));
        }
        let source = RetainedExternalSource::open(&file_url)?;
        cancelled_checkpoint(cancel)?;
        let probe =
            match self
                .engine
                .probe_file_cancellable(source.file(), cancel, MCP_MEDIA_PROBE_TIMEOUT)
            {
                Ok(probe) => ProbedMedia {
                    duration_secs: probe.duration_secs,
                    width: probe.width.map(|value| value as i32),
                    height: probe.height.map(|value| value as i32),
                    fps: probe.fps,
                    has_audio: probe.has_audio,
                    color: probe.color,
                },
                Err(opentake_media::MediaError::Cancelled) => {
                    return Err(BridgeError::new("source.path import was cancelled"));
                }
                Err(_) => ProbedMedia::default(),
            };
        cancelled_checkpoint(cancel)?;
        let display_name = name
            .map(str::to_owned)
            .unwrap_or_else(|| crate::media::display_name(&file_url));
        before_commit();
        let project_media = ProjectMediaCapability::open_verified(
            &self.core,
            project.project_epoch,
            &project_dir,
            true,
        )
        .map_err(BridgeError::new)?;
        let mut events = DeferredCoreEvents::default();
        let commit = self
            .core
            .import_retained_media_for_project_deferred_with_manifest_writer(
                project.project_epoch,
                &project_dir,
                &source.path,
                display_name,
                &probe,
                folder_id,
                &mut events,
                |manifest| {
                    project_media
                        .write_manifest(manifest)
                        .map_err(CoreError::Media)
                },
                || {
                    if cancel.checkpoint() {
                        return Err(CoreError::Media(
                            "source.path import was cancelled before commit".to_string(),
                        ));
                    }
                    match source.matches_path() {
                        Ok(true) => Ok(()),
                        Ok(false) => Err(CoreError::Media(
                            "source.path identity changed before commit".to_string(),
                        )),
                        Err(error) => Err(CoreError::Media(format!(
                            "source.path identity check failed before commit: {error}"
                        ))),
                    }
                },
            )
            .map_err(|error| {
                if cancel.is_cancelled() {
                    BridgeError::new(error.to_string())
                } else {
                    BridgeError::new(
                        "MCP_SOURCE_IMPORT_FAILED: source.path could not be imported; verify the file type and permissions",
                    )
                }
            })?;
        self.core.emit_deferred(events);
        let recovery_required = commit.warning.is_some();
        Ok(ImportOutcome {
            asset_count: 1,
            folder_count: 0,
            recovery_required,
        })
    }

    /// `bytes` import: write the base64 payload into the project bundle's `media/`,
    /// then register it through the same import path. 1:1 with upstream
    /// `ToolExecutor+Import.importFromBytes`.
    #[cfg(test)]
    fn import_from_bytes(
        &self,
        base64: &str,
        mime_type: &str,
        name: Option<&str>,
        folder_id: Option<&str>,
    ) -> Result<ImportOutcome, BridgeError> {
        self.import_from_bytes_cancellable(
            base64,
            mime_type,
            name,
            folder_id,
            &opentake_media::MediaCancelToken::new(),
        )
    }

    fn import_from_bytes_cancellable(
        &self,
        base64: &str,
        mime_type: &str,
        name: Option<&str>,
        folder_id: Option<&str>,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<ImportOutcome, BridgeError> {
        self.import_from_bytes_cancellable_with_hook(
            base64,
            mime_type,
            name,
            folder_id,
            cancel,
            || {},
        )
    }

    fn import_from_bytes_cancellable_with_hook(
        &self,
        base64: &str,
        mime_type: &str,
        name: Option<&str>,
        folder_id: Option<&str>,
        cancel: &opentake_media::MediaCancelToken,
        before_commit: impl FnOnce(),
    ) -> Result<ImportOutcome, BridgeError> {
        cancelled_checkpoint(cancel)?;
        self.core
            .ensure_project_mutable()
            .map_err(|error| BridgeError::new(error.to_string()))?;
        let Some(file_ext) = crate::media::file_extension_for_mime(mime_type) else {
            return Err(BridgeError::new(format!(
                "Unsupported mimeType '{mime_type}'. {}",
                crate::media::IMPORT_ACCEPTED_MIMES
            )));
        };
        let data = base64::engine::general_purpose::STANDARD
            .decode(base64.trim())
            .ok()
            .filter(|d| !d.is_empty())
            .ok_or_else(|| BridgeError::new("source.bytes is not valid non-empty base64"))?;
        if data.len() > IMPORT_BYTES_DECODED_MAX {
            return Err(BridgeError::new(format!(
                "source.bytes decoded payload is too large: {} bytes, max {}; use source.path for larger files",
                data.len(),
                IMPORT_BYTES_DECODED_MAX
            )));
        }
        cancelled_checkpoint(cancel)?;

        let project = self.core.runtime_snapshot();
        let project_dir = project
            .project_dir
            .ok_or_else(|| BridgeError::new("No project is open; cannot import bytes"))?;
        let project_media = ProjectMediaCapability::open_verified(
            &self.core,
            project.project_epoch,
            &project_dir,
            true,
        )
        .map_err(BridgeError::new)?;
        let filename = format!("imported-{}.{file_ext}", short_uuid());
        let mut staged = project_media
            .create_import(Path::new(&filename))
            .map_err(BridgeError::new)?;
        staged
            .file_mut()
            .write_all(&data)
            .map_err(|error| BridgeError::new(format!("Failed to stage source.bytes: {error}")))?;
        staged
            .file_mut()
            .flush()
            .map_err(|error| BridgeError::new(format!("Failed to flush source.bytes: {error}")))?;
        staged
            .file()
            .sync_all()
            .map_err(|error| BridgeError::new(format!("Failed to sync source.bytes: {error}")))?;
        staged
            .file_mut()
            .seek(SeekFrom::Start(0))
            .map_err(|error| BridgeError::new(format!("Failed to rewind source.bytes: {error}")))?;
        cancelled_checkpoint(cancel)?;
        if !project_media
            .matches_leaf(&staged)
            .map_err(BridgeError::new)?
        {
            return Err(BridgeError::new(
                "source.bytes staging identity changed before probe",
            ));
        }
        let probe =
            match self
                .engine
                .probe_file_cancellable(staged.file(), cancel, MCP_MEDIA_PROBE_TIMEOUT)
            {
                Ok(probe) => ProbedMedia {
                    duration_secs: probe.duration_secs,
                    width: probe.width.map(|value| value as i32),
                    height: probe.height.map(|value| value as i32),
                    fps: probe.fps,
                    has_audio: probe.has_audio,
                    color: probe.color,
                },
                Err(opentake_media::MediaError::Cancelled) => {
                    return Err(BridgeError::new("source.bytes import was cancelled"));
                }
                Err(_) => ProbedMedia::default(),
            };
        cancelled_checkpoint(cancel)?;
        if !project_media
            .matches_leaf(&staged)
            .map_err(BridgeError::new)?
        {
            return Err(BridgeError::new(
                "source.bytes staging identity changed during probe",
            ));
        }
        let display_name = name
            .map(str::to_owned)
            .unwrap_or_else(|| crate::media::display_name(staged.path()));
        before_commit();
        let mut events = DeferredCoreEvents::default();
        let commit = self
            .core
            .import_retained_media_for_project_deferred_with_manifest_writer(
                project.project_epoch,
                &project_dir,
                staged.path(),
                display_name,
                &probe,
                folder_id,
                &mut events,
                |manifest| {
                    project_media
                        .write_manifest(manifest)
                        .map_err(CoreError::Media)
                },
                || {
                    if cancel.checkpoint() {
                        return Err(CoreError::Media(
                            "source.bytes import was cancelled before publication".to_string(),
                        ));
                    }
                    match project_media.matches_leaf(&staged) {
                        Ok(true) => Ok(()),
                        Ok(false) => Err(CoreError::Media(
                            "source.bytes staging identity changed during publication".to_string(),
                        )),
                        Err(error) => Err(CoreError::Media(format!(
                        "source.bytes staging identity check failed during publication: {error}"
                    ))),
                    }
                },
            )
            .map_err(|error| BridgeError::new(error.to_string()))?;
        staged.commit();
        self.core.emit_deferred(events);
        let recovery_required = commit.warning.is_some();
        Ok(ImportOutcome {
            asset_count: 1,
            folder_count: 0,
            recovery_required,
        })
    }
}

fn cancelled_checkpoint(cancel: &opentake_media::MediaCancelToken) -> Result<(), BridgeError> {
    if cancel.checkpoint() {
        Err(BridgeError::new("source.url import was cancelled"))
    } else {
        Ok(())
    }
}

fn validate_https_url(raw: &str) -> Result<reqwest::Url, BridgeError> {
    let url = reqwest::Url::parse(raw)
        .map_err(|error| BridgeError::new(format!("source.url is invalid: {error}")))?;
    validate_parsed_https_url(&url)?;
    Ok(url)
}

fn validate_parsed_https_url(url: &reqwest::Url) -> Result<(), BridgeError> {
    if url.scheme() != "https" {
        return Err(BridgeError::new("source.url must use HTTPS"));
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(BridgeError::new("source.url must include a host"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(BridgeError::new("source.url must not include userinfo"));
    }
    if let Some(ip) = url.host_str().and_then(literal_host_ip) {
        ensure_public_ip(ip)?;
    }
    Ok(())
}

fn is_followed_redirect(status: reqwest::StatusCode) -> bool {
    matches!(
        status,
        reqwest::StatusCode::MOVED_PERMANENTLY
            | reqwest::StatusCode::FOUND
            | reqwest::StatusCode::SEE_OTHER
            | reqwest::StatusCode::TEMPORARY_REDIRECT
            | reqwest::StatusCode::PERMANENT_REDIRECT
    )
}

fn allowed_url_extension(extension: &str) -> Option<(&'static str, &'static str)> {
    match extension.to_ascii_lowercase().as_str() {
        "mov" => Some(("mov", "video")),
        "mp4" => Some(("mp4", "video")),
        "m4v" => Some(("m4v", "video")),
        "mp3" => Some(("mp3", "audio")),
        "wav" => Some(("wav", "audio")),
        "aac" => Some(("aac", "audio")),
        "m4a" => Some(("m4a", "audio")),
        "png" => Some(("png", "image")),
        "jpg" | "jpeg" => Some(("jpg", "image")),
        "tiff" => Some(("tiff", "image")),
        "heic" => Some(("heic", "image")),
        _ => None,
    }
}

fn container_matches_extension(format_name: &str, extension: &str) -> bool {
    let formats = format_name.split(',').collect::<Vec<_>>();
    let any = |accepted: &[&str]| formats.iter().any(|format| accepted.contains(format));
    match extension {
        "mov" | "mp4" | "m4v" | "m4a" => any(&["mov", "mp4", "m4a", "3gp", "3g2", "mj2"]),
        "mp3" => any(&["mp3"]),
        "wav" => any(&["wav"]),
        "aac" => any(&["aac"]),
        "png" => any(&["png_pipe", "image2"]),
        "jpg" => any(&["jpeg_pipe", "image2"]),
        "tiff" => any(&["tiff_pipe", "image2"]),
        "heic" => any(&["heic", "heif", "image2"]),
        _ => false,
    }
}

fn normalized_mime(raw: &str) -> String {
    raw.split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

fn mime_extension_and_kind(raw: &str) -> Result<(&'static str, &'static str), BridgeError> {
    let mime = normalized_mime(raw);
    let extension = crate::media::file_extension_for_mime(&mime).ok_or_else(|| {
        BridgeError::new(format!(
            "Unsupported source.url MIME type '{mime}'. {}",
            crate::media::IMPORT_ACCEPTED_MIMES
        ))
    })?;
    let (_, kind) =
        allowed_url_extension(extension).expect("MIME table only returns allowed URL extensions");
    Ok((extension, kind))
}

fn resolve_url_media_type(
    url: &reqwest::Url,
    requested_mime: Option<&str>,
    response_content_type: Option<&str>,
) -> Result<(String, Option<String>, &'static str), BridgeError> {
    let requested = requested_mime.map(mime_extension_and_kind).transpose()?;
    // An explicit source.mimeType is the caller's type-inference override for
    // signed or opaque URL paths, so an absent/unsupported path extension must
    // not reject the request first. The response MIME and production probe
    // still independently validate the downloaded bytes.
    let url_extension = if requested.is_some() {
        None
    } else {
        Path::new(url.path())
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| {
                allowed_url_extension(value).ok_or_else(|| {
                    BridgeError::new(format!(
                        "Unsupported source.url extension '.{value}'. Supported: mov/mp4/m4v, mp3/wav/aac/m4a, png/jpg/jpeg/tiff/heic."
                    ))
                })
            })
            .transpose()?
    };
    let response = response_content_type
        .map(mime_extension_and_kind)
        .transpose()?;

    if let (Some((_, requested_kind)), Some((_, response_kind))) = (requested, response) {
        if requested_kind != response_kind {
            return Err(BridgeError::new(
                "source.mimeType conflicts with the HTTPS response Content-Type",
            ));
        }
    }
    let mime_choice = requested.or(response);
    if let (Some((_, url_kind)), Some((_, mime_kind))) = (url_extension, mime_choice) {
        if url_kind != mime_kind {
            return Err(BridgeError::new(
                "source.url extension conflicts with its declared MIME type",
            ));
        }
    }
    let (extension, expected_kind) = mime_choice
        .or(url_extension)
        .map(|(extension, kind)| (extension.to_string(), kind))
        .ok_or_else(|| {
            BridgeError::new("source.url needs an allowed extension or an allowed MIME type")
        })?;
    let selected_mime = requested_mime
        .map(normalized_mime)
        .or_else(|| response_content_type.map(normalized_mime));
    Ok((extension, selected_mime, expected_kind))
}

fn url_display_name(url: &reqwest::Url) -> Option<String> {
    Path::new(url.path())
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// Lowercase `ClipType` name for the import confirmation (`video`/`audio`/…),
/// matching upstream `asset.type.rawValue`.
#[cfg(test)]
fn clip_type_name(kind: ClipType) -> &'static str {
    match kind {
        ClipType::Video => "video",
        ClipType::Audio => "audio",
        ClipType::Image => "image",
        ClipType::Text => "text",
        ClipType::Lottie => "lottie",
    }
}

/// An 8-hex-char pseudo-unique token for a written-bytes filename (upstream uses
/// `UUID().uuidString.prefix(8)`). Derived from the system clock — a filename
/// disambiguator only, never a security or collision-critical id.
fn short_uuid() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:08x}", (nanos as u64) & 0xffff_ffff)
}

// MARK: - Raw-source inspection for inspect_media

/// Read a project-local image through the retained no-follow bundle authority
/// when the entry is bundle-relative, falling back to the resolved pathname
/// for materialized external/generated assets. Returning the authority-opened
/// file prevents an ambient rebind of the bundle pathname from redirecting the
/// inspection to a rebound bundle (same-ID/different-path case).
fn inspect_image_thumbnail(
    core: &AppCore,
    entry: &opentake_domain::MediaManifestEntry,
    resolved_path: &Path,
) -> opentake_media::Result<RgbaFrame> {
    if let MediaSource::Project { relative_path } = &entry.source {
        if let Ok(file) = core.open_project_asset(Path::new(relative_path)) {
            return opentake_media::thumbnail::image_thumbnail_reader(
                std::io::BufReader::new(file),
                INSPECT_MEDIA_FRAME_MAX_DIMENSION,
            );
        }
    }
    opentake_media::thumbnail::image_thumbnail(resolved_path, INSPECT_MEDIA_FRAME_MAX_DIMENSION)
}

fn inspect_source_media(
    core: &AppCore,
    engine: &MediaEngine,
    request: &InspectMediaRequest,
) -> Result<InspectMediaResult, BridgeError> {
    let snapshot = core.runtime_snapshot();
    let entry = snapshot
        .media
        .entries
        .iter()
        .find(|entry| entry.id == request.media_ref)
        .ok_or_else(|| {
            BridgeError::not_found("inspect_media: media is not in the active project")
        })?;
    if entry.kind != request.kind {
        return Err(BridgeError::unavailable(
            "inspect_media: media type changed before inspection",
        ));
    }
    if entry.kind == ClipType::Text {
        return Err(BridgeError::unavailable(
            "inspect_media: text clips are not source media",
        ));
    }
    let (path, _) =
        crate::transcribe::resolve_asset_from_snapshot(&snapshot, &request.media_ref)
            .map_err(|_| BridgeError::unavailable("inspect_media: source media is offline"))?;
    let metadata = std::fs::symlink_metadata(&path)
        .map_err(|_| BridgeError::unavailable("inspect_media: source media is unavailable"))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(BridgeError::unavailable(
            "inspect_media: source media is not a regular file",
        ));
    }
    let byte_size = metadata.len();

    if entry.kind == ClipType::Lottie {
        return inspect_lottie_frames(&path, request, byte_size);
    }

    if entry.kind == ClipType::Image {
        let frame = inspect_image_thumbnail(core, entry, &path)
            .map_err(|_| BridgeError::new("inspect_media: failed to decode image"))?;
        let width = frame.width;
        let height = frame.height;
        let bytes = encode_rgba_jpeg(&frame)
            .ok_or_else(|| BridgeError::new("inspect_media: failed to encode image"))?;
        return Ok(InspectMediaResult {
            frames: vec![InspectedMediaFrame {
                timestamp_seconds: 0.0,
                bytes,
                media_type: "image/jpeg".into(),
            }],
            overview_timestamps: Vec::new(),
            duration_seconds: entry.duration.max(0.0),
            width: Some(width),
            height: Some(height),
            fps: None,
            has_audio: false,
            byte_size,
            transcript: None,
            transcription_unavailable: false,
        });
    }

    let probe = engine
        .probe(&path)
        .map_err(|_| BridgeError::new("inspect_media: failed to probe source media"))?;
    let duration = if probe.duration_secs.is_finite() && probe.duration_secs > 0.0 {
        probe.duration_secs
    } else {
        entry.duration.max(0.0)
    };
    let start = request.start_seconds.unwrap_or(0.0).clamp(0.0, duration);
    let end = request.end_seconds.unwrap_or(duration).clamp(0.0, duration);
    if start >= end {
        return Err(BridgeError::new(
            "inspect_media: requested time range is outside the source",
        ));
    }

    let (frames, overview_timestamps) = if entry.kind == ClipType::Video {
        inspect_video_frames(&path, start, end, request.max_frames, request.overview)?
    } else {
        (Vec::new(), Vec::new())
    };

    let (transcript, transcription_unavailable) = if probe.has_audio {
        match inspect_media_transcript(engine, &path, entry.kind == ClipType::Video, (start, end)) {
            Ok(transcript) => (Some(transcript), false),
            Err(error) => {
                eprintln!("[mcp] inspect_media transcription unavailable: {error}");
                (None, true)
            }
        }
    } else {
        (None, false)
    };

    Ok(InspectMediaResult {
        frames,
        overview_timestamps,
        duration_seconds: duration,
        width: probe.width,
        height: probe.height,
        fps: probe.fps,
        has_audio: probe.has_audio,
        byte_size,
        transcript,
        transcription_unavailable,
    })
}

fn inspect_lottie_frames(
    path: &Path,
    request: &InspectMediaRequest,
    byte_size: u64,
) -> Result<InspectMediaResult, BridgeError> {
    struct FixedResolver(Rc<GpuTexture>);

    impl TextureResolver for FixedResolver {
        fn resolve(
            &mut self,
            _source: &TextureSource,
            _source_frame: i64,
        ) -> Option<Rc<GpuTexture>> {
            Some(self.0.clone())
        }
    }

    let dev = RenderDevice::try_new()
        .map_err(|_| BridgeError::unavailable("inspect_media: Lottie GPU rendering unavailable"))?;
    let mut materializer = crate::render::LottieMaterializer::new();
    let metadata = materializer
        .metadata(path)
        .map_err(|_| BridgeError::unavailable("inspect_media: invalid Lottie document"))?;
    let start = request
        .start_seconds
        .unwrap_or(0.0)
        .clamp(0.0, metadata.duration_seconds);
    let end = request
        .end_seconds
        .unwrap_or(metadata.duration_seconds)
        .clamp(0.0, metadata.duration_seconds);
    if start >= end {
        return Err(BridgeError::new(
            "inspect_media: requested time range is outside the Lottie animation",
        ));
    }

    let count = if request.overview {
        INSPECT_MEDIA_OVERVIEW_TILES
    } else {
        request.max_frames.max(1)
    };
    let timestamps = (0..count)
        .map(|index| start + (end - start) * (index as f64 + 0.5) / count as f64)
        .collect::<Vec<_>>();
    let render_size = fit_render_size(
        metadata.width as i32,
        metadata.height as i32,
        INSPECT_MEDIA_FRAME_MAX_DIMENSION,
    );
    let source = TextureSource::Lottie {
        media_ref: request.media_ref.clone(),
    };
    let compositor = Compositor::new(&dev.device);
    let mut cache = TextureCache::new(TEXTURE_CACHE_CAP);
    let mut rendered = Vec::with_capacity(timestamps.len());
    for &timestamp in &timestamps {
        let source_frame = (timestamp * metadata.frame_rate)
            .floor()
            .clamp(0.0, (metadata.frame_count - 1) as f64) as i64;
        let texture = materializer
            .resolve(
                &dev.device,
                &dev.queue,
                &mut cache,
                path,
                source_frame,
                (render_size.width, render_size.height),
                "inspect-media-lottie",
            )
            .map_err(|_| BridgeError::new("inspect_media: failed to render Lottie frame"))?;
        let draw = LayerDraw {
            source: &source,
            source_frame,
            affine: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            nat_size: (render_size.width as f64, render_size.height as f64),
            crop_uv: (0.0, 0.0, 1.0, 1.0),
            opacity: 1.0,
            needs_premultiply: false,
            clip_id: "inspect-media-lottie",
            color_grade: None,
            lut: None,
            chroma_key: None,
            masks: &[],
            effects: &[],
        };
        let plan = FramePlan {
            // Lottie inspection deliberately exposes transparency over neutral
            // gray, matching the public tool description and upstream behavior.
            clear_rgba: [0.5, 0.5, 0.5, 1.0],
            draws: vec![draw],
        };
        let frame = compositor
            .render_to_rgba(
                &dev.device,
                &dev.queue,
                render_size,
                &plan,
                &mut FixedResolver(texture),
            )
            .map_err(|_| BridgeError::new("inspect_media: failed to composite Lottie frame"))?;
        rendered.push((
            timestamp,
            RgbaFrame::new(frame.width, frame.height, frame.rgba),
        ));
    }

    let (frames, overview_timestamps) = if request.overview {
        let (bytes, _, _) = encode_storyboard_jpeg(&rendered)
            .ok_or_else(|| BridgeError::new("inspect_media: failed to encode Lottie overview"))?;
        (
            vec![InspectedMediaFrame {
                timestamp_seconds: start,
                bytes,
                media_type: "image/jpeg".into(),
            }],
            timestamps,
        )
    } else {
        let frames = rendered
            .iter()
            .map(|(timestamp_seconds, frame)| {
                encode_rgba_jpeg(frame)
                    .map(|bytes| InspectedMediaFrame {
                        timestamp_seconds: *timestamp_seconds,
                        bytes,
                        media_type: "image/jpeg".into(),
                    })
                    .ok_or_else(|| BridgeError::new("inspect_media: failed to encode Lottie frame"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        (frames, Vec::new())
    };

    Ok(InspectMediaResult {
        frames,
        overview_timestamps,
        duration_seconds: metadata.duration_seconds,
        width: Some(metadata.width),
        height: Some(metadata.height),
        fps: Some(metadata.frame_rate),
        has_audio: false,
        byte_size,
        transcript: None,
        transcription_unavailable: false,
    })
}

fn inspect_video_frames(
    path: &Path,
    start: f64,
    end: f64,
    requested_frames: usize,
    overview: bool,
) -> Result<(Vec<InspectedMediaFrame>, Vec<f64>), BridgeError> {
    let count = if overview {
        INSPECT_MEDIA_OVERVIEW_TILES
    } else {
        requested_frames.max(1)
    };
    let times = (0..count)
        .map(|index| start + (end - start) * (index as f64 + 0.5) / count as f64)
        .collect::<Vec<_>>();
    let decode = FrameRequest {
        time_secs: 0.0,
        max_size: (
            INSPECT_MEDIA_FRAME_MAX_DIMENSION,
            INSPECT_MEDIA_FRAME_MAX_DIMENSION,
        ),
        tolerance_secs: 0.25,
        apply_rotation: true,
    };
    let decoded = decode_frames_at(path, &times, &decode)
        .into_iter()
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    if decoded.is_empty() {
        return Err(BridgeError::new(
            "inspect_media: failed to decode video frames",
        ));
    }

    if overview {
        let timestamps = decoded.iter().map(|(time, _)| *time).collect::<Vec<_>>();
        let (bytes, _width, _height) = encode_storyboard_jpeg(&decoded)
            .ok_or_else(|| BridgeError::new("inspect_media: failed to encode overview"))?;
        return Ok((
            vec![InspectedMediaFrame {
                timestamp_seconds: start,
                bytes,
                media_type: "image/jpeg".into(),
            }],
            timestamps,
        ));
    }

    let frames = decoded
        .into_iter()
        .filter_map(|(timestamp_seconds, frame)| {
            encode_rgba_jpeg(&frame).map(|bytes| InspectedMediaFrame {
                timestamp_seconds,
                bytes,
                media_type: "image/jpeg".into(),
            })
        })
        .collect::<Vec<_>>();
    if frames.is_empty() {
        return Err(BridgeError::new(
            "inspect_media: failed to encode video frames",
        ));
    }
    Ok((frames, Vec::new()))
}

fn inspect_media_transcript(
    engine: &MediaEngine,
    path: &Path,
    is_video: bool,
    range: (f64, f64),
) -> Result<opentake_media::TranscriptionResult, String> {
    if let Some(full) = opentake_media::transcribe::cache::cached_on_disk(engine.cache_root(), path)
    {
        return Ok(opentake_media::transcribe::cache::filter(&full, range));
    }
    let backend = crate::transcribe::load_backend(engine)?;
    let cache = opentake_media::TranscriptCache::new(engine.cache_root());
    cache
        .transcript(path, is_video, Some(range), &backend)
        .map_err(|error| error.to_string())
}

fn encode_rgba_jpeg(frame: &RgbaFrame) -> Option<Vec<u8>> {
    encode_jpeg(&DecodedFrame::new(
        frame.width,
        frame.height,
        frame.rgba.clone(),
        false,
    ))
}

fn encode_storyboard_jpeg(frames: &[(f64, RgbaFrame)]) -> Option<(Vec<u8>, u32, u32)> {
    let count = u32::try_from(frames.len()).ok()?;
    let columns = count.clamp(1, INSPECT_MEDIA_OVERVIEW_COLUMNS);
    let rows = count.div_ceil(columns);
    let width = columns * INSPECT_MEDIA_OVERVIEW_TILE.0;
    let height = rows * INSPECT_MEDIA_OVERVIEW_TILE.1;
    let mut canvas = image::RgbaImage::from_pixel(width, height, image::Rgba([128, 128, 128, 255]));

    for (index, (_, frame)) in frames.iter().enumerate() {
        let image = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba.clone())?;
        let tile = image::imageops::thumbnail(
            &image,
            INSPECT_MEDIA_OVERVIEW_TILE.0,
            INSPECT_MEDIA_OVERVIEW_TILE.1,
        );
        let index = u32::try_from(index).ok()?;
        let cell_x = (index % columns) * INSPECT_MEDIA_OVERVIEW_TILE.0;
        let cell_y = (index / columns) * INSPECT_MEDIA_OVERVIEW_TILE.1;
        let x = cell_x + (INSPECT_MEDIA_OVERVIEW_TILE.0 - tile.width()) / 2;
        let y = cell_y + (INSPECT_MEDIA_OVERVIEW_TILE.1 - tile.height()) / 2;
        image::imageops::overlay(&mut canvas, &tile, i64::from(x), i64::from(y));
    }
    let bytes = encode_rgba_jpeg(&RgbaFrame::new(width, height, canvas.into_raw()))?;
    Some((bytes, width, height))
}

// MARK: - Timeline compositing for inspect_timeline

/// Aspect-preserving downscale so the longest edge is at most `longest_edge`
/// (never upscales). 1:1 with upstream `inspectTimeline`'s `fit(_:longestEdge:)`,
/// then even-ized for the encoder. `longest_edge == 0` means no cap.
fn fit_render_size(canvas_w: i32, canvas_h: i32, longest_edge: u32) -> RenderSize {
    let cw = canvas_w.max(2) as f64;
    let ch = canvas_h.max(2) as f64;
    if longest_edge == 0 {
        return RenderSize::new(even(cw), even(ch));
    }
    let long = cw.max(ch);
    let scale = if long > longest_edge as f64 {
        longest_edge as f64 / long
    } else {
        1.0
    };
    RenderSize::new(even(cw * scale), even(ch * scale))
}

/// Composite each frame in `frames` at the downscaled render size and JPEG-encode
/// it. A local GPU context is acquired for the batch (export.rs discipline).
/// Frames that fail to render are dropped (upstream `continue`s past a failed
/// `generator.image(at:)`); an all-empty render is an `Err`.
fn composite_frames_jpeg(
    timeline: &opentake_domain::Timeline,
    manifest: &opentake_domain::MediaManifest,
    project_dir: &Option<PathBuf>,
    frames: &[i32],
    max_longest_edge: u32,
) -> Result<InspectResult, BridgeError> {
    let render_size = fit_render_size(timeline.width, timeline.height, max_longest_edge);

    let text = project_text(timeline);
    let (sizes, media) = project_media(manifest, project_dir);
    let straight_alpha = manifest
        .entries
        .iter()
        .filter(|entry| entry.carries_straight_alpha())
        .map(|entry| entry.id.clone())
        .collect();
    let metrics = ManifestMetrics {
        sizes,
        straight_alpha,
    };
    let plan = try_build_render_plan(timeline, render_size, &metrics)
        .map_err(|error| BridgeError::new(format!("invalid timeline graph: {error}")))?;

    let project_root = project_dir
        .as_deref()
        .map(ProjectRoot::open)
        .transpose()
        .map_err(|error| BridgeError::new(format!("open project LUT storage: {error}")))?;

    let dev =
        RenderDevice::try_new().map_err(|e| BridgeError::new(format!("no GPU device: {e}")))?;
    let compositor = Compositor::new(&dev.device);
    let text_rasterizer = CosmicTextRasterizer::new();
    if !text_rasterizer.has_fonts() {
        eprintln!("[render] no system fonts discovered; text clips will render blank");
    }

    let mut out_frames: Vec<InspectedFrame> = Vec::with_capacity(frames.len());
    let mut lut_cache = HashMap::new();
    let mut lottie = crate::render::LottieMaterializer::new();
    for &f in frames {
        let frame_plan = plan.frame(timeline, f);
        let mut resolver = InspectResolver {
            device: &dev.device,
            queue: &dev.queue,
            cache: TextureCache::new(TEXTURE_CACHE_CAP),
            media: &media,
            timeline_fps: plan.fps,
            text: &text,
            text_rasterizer: &text_rasterizer,
            render_box: (render_size.width, render_size.height),
            project_root: project_root.as_ref(),
            lut_cache: &mut lut_cache,
            lottie: &mut lottie,
        };
        let composite = match compositor.render_to_rgba(
            &dev.device,
            &dev.queue,
            render_size,
            &frame_plan,
            &mut resolver,
        ) {
            Ok(c) => c,
            Err(_) => continue, // skip an unrenderable frame (upstream parity)
        };
        let Some(bytes) = encode_jpeg(&composite) else {
            continue;
        };
        out_frames.push(InspectedFrame {
            frame: f,
            bytes,
            media_type: "image/jpeg".into(),
        });
    }

    if out_frames.is_empty() {
        return Err(BridgeError::new("Failed to render timeline frames."));
    }
    Ok(InspectResult {
        frames: out_frames,
        width: render_size.width,
        height: render_size.height,
    })
}

/// JPEG-encode an RGBA composite at [`INSPECT_JPEG_QUALITY`]. `None` on an encode
/// failure so the caller drops the frame (upstream skips a failed encode).
///
/// JPEG carries no alpha channel and `image`'s `JpegEncoder` only accepts `L8` /
/// `Rgb8`, so the RGBA composite is flattened to RGB first. The compositor clears
/// to opaque black and produces a fully-composited frame, so dropping the (opaque)
/// alpha is lossless for the visible pixels — matching upstream, which composites
/// onto an opaque canvas before `encodeJPEG`.
fn encode_jpeg(frame: &DecodedFrame) -> Option<Vec<u8>> {
    let rgb = rgba_to_rgb(&frame.rgba);
    let mut bytes: Vec<u8> = Vec::new();
    let mut encoder =
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, INSPECT_JPEG_QUALITY);
    encoder
        .encode(
            &rgb,
            frame.width,
            frame.height,
            image::ExtendedColorType::Rgb8,
        )
        .ok()?;
    Some(bytes)
}

/// Drop the alpha channel from a tightly-packed RGBA buffer, yielding RGB. Used
/// to feed the alpha-less JPEG encoder.
fn rgba_to_rgb(rgba: &[u8]) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
    for px in rgba.chunks_exact(4) {
        rgb.extend_from_slice(&px[..3]);
    }
    rgb
}

/// Resolvable info for one media asset, projected from the manifest.
struct MediaInfo {
    path: PathBuf,
}

/// A text clip projected from the timeline, keyed by clip id.
struct TextInfo {
    content: String,
    style: TextStyle,
    box_norm: (f64, f64, f64, f64),
}

/// `SourceMetrics` backed by the media manifest (intrinsic size only).
struct ManifestMetrics {
    sizes: HashMap<String, (u32, u32)>,
    straight_alpha: HashSet<String>,
}

impl SourceMetrics for ManifestMetrics {
    fn natural_size(&self, media_ref: &str) -> Option<(u32, u32)> {
        self.sizes.get(media_ref).copied()
    }

    fn needs_premultiply(&self, media_ref: &str) -> bool {
        self.straight_alpha.contains(media_ref)
    }
}

/// `TextureResolver` that materializes a layer's pixels on demand and uploads
/// them to the GPU. Video/image use FFmpeg/image decode, text uses the shared
/// rasterizer, and Lottie uses the same Velato/Vello path as preview/export.
struct InspectResolver<'d> {
    device: &'d opentake_render::wgpu::Device,
    queue: &'d opentake_render::wgpu::Queue,
    cache: TextureCache,
    media: &'d HashMap<String, MediaInfo>,
    timeline_fps: i32,
    text: &'d HashMap<String, TextInfo>,
    text_rasterizer: &'d CosmicTextRasterizer,
    render_box: (u32, u32),
    project_root: Option<&'d ProjectRoot>,
    lut_cache: &'d mut HashMap<String, Rc<GpuLutTexture>>,
    lottie: &'d mut crate::render::LottieMaterializer,
}

impl InspectResolver<'_> {
    fn resolve_text(&mut self, clip_id: &str) -> Option<Rc<GpuTexture>> {
        let key = format!("t:{clip_id}");
        if let Some(tex) = self.cache.get(&key) {
            return Some(tex);
        }
        let info = self.text.get(clip_id)?;
        let req = TextRasterRequest {
            clip_id,
            content: &info.content,
            style: &info.style,
            box_norm: info.box_norm,
            canvas: self.render_box,
        };
        let frame = self.text_rasterizer.rasterize(&req)?;
        let tex = upload_rgba(self.device, self.queue, &frame, false, Some("inspect-text"));
        Some(self.cache.insert(key, tex))
    }

    fn resolve_managed_lut(
        &mut self,
        reference: &LutReference,
    ) -> Result<Option<Rc<GpuLutTexture>>, opentake_render::RenderError> {
        if let Some(cached) = self.lut_cache.get(&reference.id) {
            return Ok(Some(cached.clone()));
        }
        let resolved = crate::lut::resolve_project_lut(
            self.project_root,
            reference,
            self.device,
            self.queue,
            "inspect-lut",
        )?;
        if let Some(texture) = &resolved {
            self.lut_cache.insert(reference.id.clone(), texture.clone());
        }
        Ok(resolved)
    }
}

impl TextureResolver for InspectResolver<'_> {
    fn resolve(&mut self, source: &TextureSource, source_frame: i64) -> Option<Rc<GpuTexture>> {
        if let TextureSource::Lottie { media_ref } = source {
            let info = self.media.get(media_ref)?;
            return self
                .lottie
                .resolve(
                    self.device,
                    self.queue,
                    &mut self.cache,
                    &info.path,
                    source_frame,
                    self.render_box,
                    "inspect-lottie",
                )
                .ok();
        }
        let (media_ref, key, is_image) = match source {
            TextureSource::Decoded { media_ref } => {
                (media_ref, format!("v:{media_ref}:{source_frame}"), false)
            }
            TextureSource::Image { media_ref } => (media_ref, format!("i:{media_ref}"), true),
            TextureSource::Text { clip_id } => return self.resolve_text(clip_id),
            TextureSource::Lottie { .. } => unreachable!("handled above"),
        };

        if let Some(tex) = self.cache.get(&key) {
            return Some(tex);
        }

        let info = self.media.get(media_ref)?;
        let time_secs = if is_image {
            0.0
        } else {
            project_frame_time_secs(source_frame, self.timeline_fps)
        };

        let req = FrameRequest {
            time_secs,
            max_size: self.render_box,
            // Tight tolerance keeps each inspected frame on the exact target time
            // (quality over the scrub-oriented wide tolerance the preview uses).
            tolerance_secs: 0.0,
            apply_rotation: true,
        };
        let (_actual, frame) = decode_frame_at(&info.path, &req).ok()?;
        let decoded = DecodedFrame::new(frame.width, frame.height, frame.rgba, false);
        let tex = upload_rgba(
            self.device,
            self.queue,
            &decoded,
            false,
            Some("inspect-src"),
        );
        Some(self.cache.insert(key, tex))
    }

    fn resolve_lut(
        &mut self,
        reference: &LutReference,
    ) -> Result<Option<Rc<GpuLutTexture>>, opentake_render::RenderError> {
        self.resolve_managed_lut(reference)
    }
}

/// Project the timeline's text clips (content + style + box) into the per-clip
/// lookup the resolver rasterizes from. Keyed by clip id.
fn project_text(timeline: &opentake_domain::Timeline) -> HashMap<String, TextInfo> {
    let mut text: HashMap<String, TextInfo> = HashMap::new();
    for candidate in std::iter::once(timeline).chain(
        timeline
            .nested_sequences
            .iter()
            .map(|sequence| &sequence.timeline),
    ) {
        for track in &candidate.tracks {
            for clip in &track.clips {
                if clip.media_type != ClipType::Text {
                    continue;
                }
                let (Some(content), Some(style)) = (&clip.text_content, &clip.text_style) else {
                    continue;
                };
                let tl = clip.transform.top_left();
                text.insert(
                    clip.id.clone(),
                    TextInfo {
                        content: content.clone(),
                        style: style.clone(),
                        box_norm: (tl.x, tl.y, clip.transform.width, clip.transform.height),
                    },
                );
            }
        }
    }
    text
}

/// Project the media manifest into the render-side `(sizes, media)` lookups,
/// resolving project-relative paths against `project_dir`.
fn project_media(
    manifest: &opentake_domain::MediaManifest,
    project_dir: &Option<PathBuf>,
) -> (HashMap<String, (u32, u32)>, HashMap<String, MediaInfo>) {
    let mut sizes: HashMap<String, (u32, u32)> = HashMap::new();
    let mut media: HashMap<String, MediaInfo> = HashMap::new();
    for entry in &manifest.entries {
        let path = match &entry.source {
            MediaSource::External { absolute_path } => PathBuf::from(absolute_path),
            MediaSource::Project { relative_path } => match project_dir {
                Some(base) => base.join(relative_path),
                None => continue,
            },
        };
        if let (Some(w), Some(h)) = (entry.source_width, entry.source_height) {
            if w > 0 && h > 0 {
                sizes.insert(entry.id.clone(), (w as u32, h as u32));
            }
        }
        media.insert(entry.id.clone(), MediaInfo { path });
    }
    (sizes, media)
}

fn project_frame_time_secs(source_frame: i64, timeline_fps: i32) -> f64 {
    let fps = if timeline_fps > 0 {
        timeline_fps as f64
    } else {
        30.0
    };
    (source_frame.max(0) as f64) / fps
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_agent::mcp::core_handle::{AppCoreHandle, CoreHandle};
    use opentake_agent::mcp::media_bridge::TimelineMutationReceipt;
    use std::sync::Condvar;

    #[test]
    fn documented_mcp_entrypoint_compiles() {
        let _entrypoint = spawn;
    }

    #[test]
    fn motion_document_bridge_is_hash_safe_and_project_bound() {
        let fixture = tempfile::tempdir().expect("motion bridge fixture");
        let core = AppCore::new();
        core.save_project(Some(fixture.path().join("A.opentake")))
            .expect("save project A");
        let notifications = Arc::new(Mutex::new(Vec::new()));
        let captured = notifications.clone();
        let bridge = build_motion_document_bridge(
            core.clone(),
            fixture.path().join("motion-cache"),
            Some(Arc::new(move |summary| {
                captured
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(summary.clone());
            })),
        );
        assert!(bridge.can_edit_motion_documents());

        let created = bridge
            .admit(MotionDocumentRequest::Create {
                title: Some("Agent co-edit".into()),
            })
            .expect("admit create")
            .execute(&opentake_media::MediaCancelToken::new())
            .expect("create document");
        let MotionDocumentResponse::Document(created) = created else {
            panic!("create returned wrong response");
        };
        let start = created.html.len();
        let patched = bridge
            .admit(MotionDocumentRequest::Patch(
                opentake_agent::mcp::motion_documents::MotionDocumentPatchRequest {
                    document_id: created.summary.document_id.clone(),
                    file: "index.html".into(),
                    baseline_hash: created.summary.revision_hash.clone(),
                    edits: vec![
                        opentake_agent::mcp::motion_documents::MotionTextReplacement {
                            start,
                            end: start,
                            replacement: "\n<!-- 真实字符 -->".into(),
                        },
                    ],
                },
            ))
            .expect("admit patch")
            .execute(&opentake_media::MediaCancelToken::new())
            .expect("patch document");
        let MotionDocumentResponse::Document(patched) = patched else {
            panic!("patch returned wrong response");
        };
        assert!(patched.html.contains("真实字符"));
        assert_ne!(patched.summary.revision_hash, created.summary.revision_hash);
        let notifications = notifications
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(notifications.len(), 2);
        assert_eq!(
            notifications
                .last()
                .map(|change| &change.summary.revision_hash),
            Some(&patched.summary.revision_hash)
        );
        let project_a = core.runtime_snapshot();
        assert_eq!(notifications[1].project_epoch, project_a.project_epoch);
        assert_eq!(
            notifications[1].project_path,
            project_a
                .project_dir
                .expect("saved project path")
                .to_string_lossy()
        );
        drop(notifications);

        let stale = bridge
            .admit(MotionDocumentRequest::Patch(
                opentake_agent::mcp::motion_documents::MotionDocumentPatchRequest {
                    document_id: created.summary.document_id.clone(),
                    file: "styles.css".into(),
                    baseline_hash: created.summary.revision_hash,
                    edits: vec![
                        opentake_agent::mcp::motion_documents::MotionTextReplacement {
                            start: 0,
                            end: 0,
                            replacement: "/* stale */".into(),
                        },
                    ],
                },
            ))
            .expect("admit stale patch")
            .execute(&opentake_media::MediaCancelToken::new())
            .expect_err("stale patch conflicts");
        assert_eq!(stale.kind, MotionDocumentBridgeErrorKind::Conflict);
        assert_eq!(
            stale.current_revision_hash.as_deref(),
            Some(patched.summary.revision_hash.as_str())
        );

        let admitted_a = bridge
            .admit(MotionDocumentRequest::Read {
                document_id: created.summary.document_id.clone(),
            })
            .expect("admit project A read");
        core.save_project(Some(fixture.path().join("B.opentake")))
            .expect("switch to project B");
        let switched = admitted_a
            .execute(&opentake_media::MediaCancelToken::new())
            .expect_err("A operation cannot enter B");
        assert_eq!(switched.kind, MotionDocumentBridgeErrorKind::Cancelled);

        let listed_b = bridge
            .admit(MotionDocumentRequest::List)
            .expect("admit B list")
            .execute(&opentake_media::MediaCancelToken::new())
            .expect("list B");
        let MotionDocumentResponse::Documents(listed_b) = listed_b else {
            panic!("list returned wrong response");
        };
        assert_eq!(listed_b.len(), 1, "Save As preserves the document");
        assert_eq!(listed_b[0].document_id, created.summary.document_id);
    }

    struct BlockingImportBridge {
        entered: Mutex<Option<std::sync::mpsc::Sender<()>>>,
        released: Mutex<bool>,
        release_changed: Condvar,
    }

    impl BlockingImportBridge {
        fn new(entered: std::sync::mpsc::Sender<()>) -> Self {
            Self {
                entered: Mutex::new(Some(entered)),
                released: Mutex::new(false),
                release_changed: Condvar::new(),
            }
        }

        fn release(&self) {
            *self
                .released
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
            self.release_changed.notify_all();
        }
    }

    impl MediaBridge for BlockingImportBridge {
        fn import_media_cancellable(
            &self,
            _source: ImportSource,
            _name: Option<String>,
            _folder_id: Option<String>,
            cancel: &opentake_media::MediaCancelToken,
        ) -> Result<ImportOutcome, BridgeError> {
            if let Some(entered) = self
                .entered
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
            {
                let _ = entered.send(());
            }
            let mut released = self
                .released
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            while !*released {
                released = self
                    .release_changed
                    .wait(released)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            if cancel.is_cancelled() {
                Err(BridgeError::new("cancelled blocking import"))
            } else {
                Ok(ImportOutcome {
                    asset_count: 1,
                    folder_count: 0,
                    recovery_required: false,
                })
            }
        }
    }

    #[test]
    fn live_project_gate_requires_a_saved_nontransitioning_project() {
        let fixture = tempfile::tempdir().unwrap();
        let core = AppCore::new();
        let gate = LiveProjectMcpGate::new(core.clone());
        let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
        let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(core.clone()));
        let dispatcher = Dispatcher::new(handle, registry);

        assert!(gate.timeline(&dispatcher).is_none());
        core.save_project(Some(fixture.path().join("A.opentake")))
            .unwrap();
        assert!(gate.timeline(&dispatcher).is_some());

        gate.transition_depth.store(1, Ordering::Release);
        assert!(gate.timeline(&dispatcher).is_none());
        gate.transition_depth.store(0, Ordering::Release);
        assert!(gate.timeline(&dispatcher).is_some());
    }

    #[test]
    fn live_project_gate_refuses_mutating_calls_without_a_saved_project() {
        let core = AppCore::new();
        let gate = LiveProjectMcpGate::new(core.clone());
        let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
        let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(core.clone()));
        let dispatcher = Dispatcher::new(handle, registry);

        assert!(gate
            .dispatch(
                &dispatcher,
                "create_folder",
                serde_json::json!({ "name": "must-not-exist" }),
            )
            .is_none());
        assert!(core.media().folders.is_empty());
    }

    #[test]
    fn tauri_bridge_returns_bounded_real_png_for_current_empty_revision() {
        let fixture = tempfile::tempdir().unwrap();
        let core = AppCore::new();
        core.save_project(Some(fixture.path().join("Empty.opentake")))
            .unwrap();
        let snapshot = core.runtime_snapshot();
        let bridge = TauriMediaBridge::new(
            core,
            fixture.path().join("cache"),
            fixture.path().join("models"),
        );
        let block = bridge
            .capture_timeline_result(
                &TimelineResultCaptureRequest {
                    timeline: snapshot.timeline,
                    mutation: TimelineMutationReceipt {
                        visible_clip_count_before: 1,
                        visible_clip_count_after: 0,
                        committed_revision: Some(opentake_agent::mcp::core_handle::CoreRevision {
                            project_epoch: snapshot.project_epoch,
                            project_dir: snapshot.project_dir,
                            timeline_version: snapshot.version,
                        }),
                    },
                },
                &opentake_media::MediaCancelToken::new(),
            )
            .expect("capture current empty timeline");

        let Block::Image { base64, media_type } = block else {
            panic!("timeline result must be an image block");
        };
        assert_eq!(media_type, "image/png");
        assert!(base64.len() <= TIMELINE_RESULT_IMAGE_BASE64_MAX);
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(base64)
            .expect("decode returned image");
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn tauri_bridge_rejects_stale_project_before_returning_image_bytes() {
        let fixture = tempfile::tempdir().unwrap();
        let core = AppCore::new();
        core.save_project(Some(fixture.path().join("A.opentake")))
            .unwrap();
        let snapshot = core.runtime_snapshot();
        let request = TimelineResultCaptureRequest {
            timeline: snapshot.timeline,
            mutation: TimelineMutationReceipt {
                visible_clip_count_before: 1,
                visible_clip_count_after: 0,
                committed_revision: Some(opentake_agent::mcp::core_handle::CoreRevision {
                    project_epoch: snapshot.project_epoch,
                    project_dir: snapshot.project_dir,
                    timeline_version: snapshot.version,
                }),
            },
        };
        core.save_project(Some(fixture.path().join("B.opentake")))
            .unwrap();
        let bridge = TauriMediaBridge::new(
            core,
            fixture.path().join("cache"),
            fixture.path().join("models"),
        );

        bridge
            .capture_timeline_result(&request, &opentake_media::MediaCancelToken::new())
            .expect_err("stale project capture must fail closed");
    }

    #[test]
    fn tauri_bridge_cancels_real_empty_png_capture_with_the_request_token() {
        let fixture = tempfile::tempdir().unwrap();
        let core = AppCore::new();
        core.save_project(Some(fixture.path().join("Cancelled.opentake")))
            .unwrap();
        let snapshot = core.runtime_snapshot();
        let bridge = TauriMediaBridge::new(
            core,
            fixture.path().join("cache"),
            fixture.path().join("models"),
        );
        let cancel = opentake_media::MediaCancelToken::new();
        cancel.cancel();

        let error = bridge
            .capture_timeline_result(
                &TimelineResultCaptureRequest {
                    timeline: snapshot.timeline,
                    mutation: TimelineMutationReceipt {
                        visible_clip_count_before: 1,
                        visible_clip_count_after: 0,
                        committed_revision: Some(opentake_agent::mcp::core_handle::CoreRevision {
                            project_epoch: snapshot.project_epoch,
                            project_dir: snapshot.project_dir,
                            timeline_version: snapshot.version,
                        }),
                    },
                },
                &cancel,
            )
            .expect_err("the original canceled request token must stop PNG capture");

        assert!(error.message.contains("cancel"), "{}", error.message);
    }

    #[test]
    fn live_project_request_admitted_for_old_project_cannot_write_new_project() {
        let fixture = tempfile::tempdir().unwrap();
        let core = AppCore::new();
        let project_a = fixture.path().join("A.opentake");
        let project_b = fixture.path().join("B.opentake");
        core.save_project(Some(project_a)).unwrap();
        let gate = LiveProjectMcpGate::new(core.clone());
        let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
        let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(core.clone()));
        let dispatcher = Arc::new(Dispatcher::new(handle, registry));
        let cancel = opentake_media::MediaCancelToken::new();
        let (admitted_tx, admitted_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker_gate = gate.clone();
        let worker_dispatcher = dispatcher.clone();
        let worker = std::thread::spawn(move || {
            worker_gate.with_live_dispatch_after_admission(
                &cancel,
                || {
                    admitted_tx
                        .send(())
                        .expect("announce old-project admission");
                    release_rx.recv().expect("release delayed request");
                },
                || {
                    worker_dispatcher.dispatch(
                        "create_folder",
                        serde_json::json!({ "name": "must-not-land-in-B" }),
                    )
                },
            )
        });

        admitted_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("request paused after admission");
        core.save_project(Some(project_b.clone()))
            .expect("switch to project B while old request is delayed");
        release_tx.send(()).expect("release delayed request");

        assert!(
            worker.join().expect("join delayed request").is_none(),
            "old-project request must be rejected after identity generation changes"
        );
        assert_eq!(core.project_dir().as_deref(), Some(project_b.as_path()));
        assert!(
            core.media().folders.is_empty(),
            "late old-project tool call mutated project B"
        );
    }

    #[test]
    fn live_project_transition_cancels_active_request_before_identity_changes() {
        let fixture = tempfile::tempdir().unwrap();
        let core = AppCore::new();
        core.save_project(Some(fixture.path().join("A.opentake")))
            .unwrap();
        let gate = LiveProjectMcpGate::new(core.clone());
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let bridge = Arc::new(BlockingImportBridge::new(entered_tx));
        let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
        let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(core.clone()));
        let dispatcher = Arc::new(Dispatcher::with_bridge(
            handle,
            registry,
            Some(bridge.clone()),
        ));
        let cancel = opentake_media::MediaCancelToken::new();
        let worker_gate = gate.clone();
        let worker_dispatcher = dispatcher.clone();
        let worker_cancel = cancel.clone();
        let dispatch = std::thread::spawn(move || {
            worker_gate.dispatch_cancellable(
                &worker_dispatcher,
                "import_media",
                serde_json::json!({
                    "source": {
                        "bytes": "AA==",
                        "mimeType": "image/png"
                    }
                }),
                &worker_cancel,
            )
        });
        entered_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("persistent dispatch entered while holding identity");

        let (pending_tx, pending_rx) = std::sync::mpsc::channel();
        core.subscribe_project_identity_transition(move |pending| {
            if pending {
                let _ = pending_tx.send(());
            }
        });
        let (saved_tx, saved_rx) = std::sync::mpsc::channel();
        let save_core = core.clone();
        let target = fixture.path().join("B.opentake");
        let save = std::thread::spawn(move || {
            let _ = saved_tx.send(save_core.save_project(Some(target)));
        });
        pending_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("Save As announced its transition");
        assert!(cancel.is_cancelled());
        assert!(matches!(
            saved_rx.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));

        bridge.release();
        dispatch.join().expect("dispatch thread joined");
        saved_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("Save As completed after dispatch released identity")
            .expect("Save As succeeded");
        save.join().expect("Save As thread joined");
    }

    fn unknown_core(root: &Path) -> AppCore {
        let bundle = root.join("Unknown.opentake");
        let project = opentake_project::Project::new(&bundle);
        project.save().expect("save known fixture");
        let path = bundle.join("project.json");
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("read timeline fixture"))
                .expect("decode timeline fixture");
        value["futureTimeline"] = serde_json::json!(true);
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&value).expect("encode unknown fixture"),
        )
        .expect("write unknown fixture");
        let core = AppCore::new();
        core.open_project(bundle).expect("unknown project opens");
        core
    }

    fn recursive_tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        fn walk(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
            if !dir.exists() {
                return;
            }
            let mut paths = std::fs::read_dir(dir)
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
                    out.push((relative, std::fs::read(&path).expect("read tree file")));
                }
            }
        }
        let mut out = Vec::new();
        walk(root, root, &mut out);
        out
    }

    #[test]
    fn transcript_batch_resolution_uses_one_snapshot_and_authoritative_types() {
        let fixture = tempfile::tempdir().expect("fixture tempdir");
        let project_dir = fixture.path().join("Batch.opentake");
        let video_path = project_dir.join("media/video.mov");
        let audio_path = project_dir.join("media/audio.wav");
        std::fs::create_dir_all(video_path.parent().expect("media parent"))
            .expect("create media directory");
        std::fs::write(&video_path, b"video").expect("write video fixture");
        std::fs::write(&audio_path, b"audio").expect("write audio fixture");
        let mut media = opentake_domain::MediaManifest::new();
        for (id, kind, relative_path) in [
            ("video", ClipType::Video, "media/video.mov"),
            ("audio", ClipType::Audio, "media/audio.wav"),
        ] {
            media.entries.push(opentake_domain::MediaManifestEntry {
                id: id.into(),
                name: id.into(),
                kind,
                source: MediaSource::Project {
                    relative_path: relative_path.into(),
                },
                duration: 1.0,
                generation_input: None,
                source_width: None,
                source_height: None,
                source_fps: None,
                has_audio: Some(true),
                color: None,
                proxy: None,
                folder_id: None,
                cached_remote_url: None,
                cached_remote_url_expires_at: None,
            });
        }
        let snapshot = ProjectRuntimeSnapshot {
            timeline: opentake_domain::Timeline::new(),
            media,
            project_dir: Some(project_dir),
            project_epoch: 7,
            version: 3,
        };
        let stale_sources = vec![
            TranscriptSource {
                media_ref: "video".into(),
                is_video: false,
                language: None,
            },
            TranscriptSource {
                media_ref: "audio".into(),
                is_video: true,
                language: None,
            },
        ];

        let resolved = resolve_transcript_batch(&snapshot, &stale_sources);
        assert_eq!(resolved.len(), 2);
        assert_eq!(
            resolved[0].resolved.as_ref().expect("resolve video"),
            &(video_path, true)
        );
        assert_eq!(
            resolved[1].resolved.as_ref().expect("resolve audio"),
            &(audio_path, false)
        );
    }

    #[test]
    fn mcp_path_import_refuses_unknown_project_without_manifest_or_folder_change() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let core = unknown_core(tmp.path());
        let file = tmp.path().join("incoming.mp4");
        std::fs::write(&file, b"fixture").expect("write import fixture");
        let empty = tmp.path().join("empty");
        std::fs::create_dir(&empty).expect("create empty directory fixture");
        let bridge = TauriMediaBridge::new(
            core.clone(),
            tmp.path().join("cache"),
            tmp.path().join("models"),
        );
        let before = core.media();

        bridge
            .import_from_path(&file.to_string_lossy(), None, None)
            .expect_err("MCP file import must be rejected");
        assert_eq!(core.media(), before);
        bridge
            .import_from_path(&empty.to_string_lossy(), None, None)
            .expect_err("MCP empty directory import must be rejected");
        assert_eq!(core.media(), before);
    }

    #[test]
    fn mcp_bytes_import_refuses_before_media_tree_mutation() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let core = unknown_core(tmp.path());
        let media_tree = core.project_dir().expect("opened project").join("media");
        let before_exists = media_tree.exists();
        let before = recursive_tree(&media_tree);
        let bridge =
            TauriMediaBridge::new(core, tmp.path().join("cache"), tmp.path().join("models"));
        let payload = base64::engine::general_purpose::STANDARD.encode(b"png-fixture");

        bridge
            .import_from_bytes(&payload, "image/png", None, None)
            .expect_err("MCP bytes import must be rejected");

        assert_eq!(media_tree.exists(), before_exists);
        assert_eq!(recursive_tree(&media_tree), before);
    }

    #[test]
    fn cancelled_path_import_before_commit_changes_neither_manifest() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let bundle = tmp.path().join("CancelledPath.opentake");
        let source = tmp.path().join("incoming.mp4");
        std::fs::write(&source, b"video fixture").expect("write path fixture");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone()))
            .expect("save path-import fixture");
        let bridge = TauriMediaBridge::new(
            core.clone(),
            tmp.path().join("cache"),
            tmp.path().join("models"),
        );
        let before_live = core.media();
        let manifest_path = bundle.join("media.json");
        let before_disk = std::fs::read(&manifest_path).expect("read media manifest");
        let cancel = opentake_media::MediaCancelToken::new();

        let result = bridge.import_from_path_cancellable_with_hook(
            &source.to_string_lossy(),
            None,
            None,
            &cancel,
            || cancel.cancel(),
        );

        let error = result.expect_err("cancelled path import must fail");
        assert!(error.message.contains("cancel"), "{}", error.message);
        assert_eq!(core.media(), before_live, "live manifest changed");
        assert_eq!(
            std::fs::read(&manifest_path).expect("reread media manifest"),
            before_disk,
            "persisted media.json changed"
        );
    }

    #[test]
    fn cancelled_bytes_import_before_commit_removes_staging_and_manifest_change() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let bundle = tmp.path().join("CancelledBytes.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone()))
            .expect("save bytes-import fixture");
        let bridge = TauriMediaBridge::new(
            core.clone(),
            tmp.path().join("cache"),
            tmp.path().join("models"),
        );
        let before_live = core.media();
        let manifest_path = bundle.join("media.json");
        let before_disk = std::fs::read(&manifest_path).expect("read media manifest");
        let payload = base64::engine::general_purpose::STANDARD.encode(b"png fixture");
        let cancel = opentake_media::MediaCancelToken::new();

        let result = bridge.import_from_bytes_cancellable_with_hook(
            &payload,
            "image/png",
            Some("cancelled"),
            None,
            &cancel,
            || cancel.cancel(),
        );

        let error = result.expect_err("cancelled bytes import must fail");
        assert!(error.message.contains("cancel"), "{}", error.message);
        assert_eq!(core.media(), before_live, "live manifest changed");
        assert_eq!(
            std::fs::read(&manifest_path).expect("reread media manifest"),
            before_disk,
            "persisted media.json changed"
        );
        let media_dir = bundle.join("media");
        assert_eq!(
            std::fs::read_dir(media_dir)
                .expect("bytes staging directory")
                .count(),
            0,
            "cancelled staged file survived"
        );
    }

    #[cfg(unix)]
    #[test]
    fn bytes_import_rejects_project_media_symlink_without_touching_canary() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().expect("create temp root");
        let bundle = tmp.path().join("SymlinkBytes.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone()))
            .expect("save bytes-import fixture");
        let media_dir = bundle.join("media");
        if media_dir.exists() {
            std::fs::remove_dir_all(&media_dir).expect("remove empty media directory");
        }
        let external = tmp.path().join("external");
        std::fs::create_dir(&external).expect("create external canary directory");
        let canary = external.join("canary");
        std::fs::write(&canary, b"untouched").expect("write canary");
        symlink(&external, &media_dir).expect("install malicious media symlink");
        let bridge = TauriMediaBridge::new(
            core.clone(),
            tmp.path().join("cache"),
            tmp.path().join("models"),
        );
        let before_live = core.media();
        let manifest_path = bundle.join("media.json");
        let before_disk = std::fs::read(&manifest_path).expect("read media manifest");
        let payload = base64::engine::general_purpose::STANDARD.encode(b"png fixture");

        bridge
            .import_from_bytes(&payload, "image/png", None, None)
            .expect_err("media symlink must fail closed");

        assert_eq!(std::fs::read(&canary).unwrap(), b"untouched");
        assert_eq!(
            std::fs::read_dir(&external).unwrap().count(),
            1,
            "no staged leaf may escape through the media symlink"
        );
        assert_eq!(core.media(), before_live);
        assert_eq!(std::fs::read(manifest_path).unwrap(), before_disk);
    }

    #[cfg(unix)]
    #[test]
    fn bytes_import_namespace_swap_rolls_back_original_and_preserves_replacement() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let bundle = tmp.path().join("SwapBytes.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone()))
            .expect("save bytes-import fixture");
        let bridge = TauriMediaBridge::new(
            core.clone(),
            tmp.path().join("cache"),
            tmp.path().join("models"),
        );
        let before_live = core.media();
        let manifest_path = bundle.join("media.json");
        let before_disk = std::fs::read(&manifest_path).expect("read media manifest");
        let payload = base64::engine::general_purpose::STANDARD.encode(b"png fixture");
        let media_dir = bundle.join("media");
        let moved_media = bundle.join("media-moved");

        let result = bridge.import_from_bytes_cancellable_with_hook(
            &payload,
            "image/png",
            None,
            None,
            &opentake_media::MediaCancelToken::new(),
            || {
                std::fs::rename(&media_dir, &moved_media).expect("move retained media directory");
                std::fs::create_dir(&media_dir).expect("install replacement media directory");
                let leaf = std::fs::read_dir(&moved_media)
                    .expect("read moved staging directory")
                    .next()
                    .expect("staged leaf exists")
                    .expect("read staged leaf")
                    .file_name();
                std::fs::write(media_dir.join(leaf), b"replacement-canary")
                    .expect("install same-name replacement canary");
            },
        );

        result.expect_err("namespace swap must abort bytes publication");
        assert_eq!(core.media(), before_live, "live manifest changed");
        assert_eq!(
            std::fs::read(&manifest_path).expect("reread media manifest"),
            before_disk,
            "persisted manifest changed"
        );
        let replacement = std::fs::read_dir(&media_dir)
            .expect("read replacement media directory")
            .next()
            .expect("replacement remains")
            .expect("read replacement entry")
            .path();
        assert_eq!(
            std::fs::read(replacement).expect("read replacement canary"),
            b"replacement-canary",
            "rollback touched an attacker-installed replacement"
        );
        assert_eq!(
            std::fs::read_dir(&moved_media).unwrap().count(),
            0,
            "retained uncommitted leaf was not scrubbed and removed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn path_import_rejects_fifo_before_spawning_ffprobe() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let bundle = tmp.path().join("FifoPath.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle)).expect("save path fixture");
        let fifo = tmp.path().join("blocking.mp4");
        let status = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("run mkfifo");
        assert!(status.success());
        let bridge = TauriMediaBridge::new(
            core.clone(),
            tmp.path().join("cache"),
            tmp.path().join("models"),
        );
        let cancel = opentake_media::MediaCancelToken::new();
        let started = std::time::Instant::now();

        bridge
            .import_from_path_cancellable(&fifo.to_string_lossy(), None, None, &cancel)
            .expect_err("FIFO must be rejected as non-regular");

        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(
            cancel.spawned_child_count(),
            0,
            "ffprobe must not receive a blocking FIFO"
        );
        assert!(core.media().entries.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn retained_external_open_rejects_regular_file_swapped_to_fifo_without_blocking() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let source = tmp.path().join("swapped.mp4");
        std::fs::write(&source, b"regular-before-check").expect("write regular source");
        let checked = std::fs::symlink_metadata(&source).expect("initial source metadata");
        assert!(checked.is_file(), "pre-open validation saw a regular file");

        std::fs::remove_file(&source).expect("remove checked source");
        let status = std::process::Command::new("mkfifo")
            .arg(&source)
            .status()
            .expect("replace source with FIFO");
        assert!(status.success());

        let started = std::time::Instant::now();
        let retained = RetainedExternalSource::open(&source);
        assert!(
            retained.is_err(),
            "retained open must reject the swapped FIFO"
        );
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "retained open blocked on a FIFO swapped after metadata validation"
        );
    }

    #[cfg(unix)]
    #[test]
    fn retained_external_revalidation_rejects_fifo_swap_without_blocking() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let source = tmp.path().join("revalidated.mp4");
        std::fs::write(&source, b"retained-regular-source").expect("write regular source");
        let retained = RetainedExternalSource::open(&source).expect("retain regular source");

        std::fs::remove_file(&source).expect("unlink retained source name");
        let status = std::process::Command::new("mkfifo")
            .arg(&source)
            .status()
            .expect("replace retained source name with FIFO");
        assert!(status.success());

        let started = std::time::Instant::now();
        assert!(
            !retained.matches_path().expect("revalidate swapped source"),
            "FIFO replacement must not match the retained regular file"
        );
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "retained identity revalidation blocked on a FIFO replacement"
        );
    }

    #[cfg(windows)]
    #[test]
    fn retained_external_source_rejects_windows_reparse_contract() {
        use std::os::windows::fs::symlink_file;

        assert!(crate::media::windows_file_attributes_are_reparse(0x400));
        assert!(!crate::media::windows_file_attributes_are_reparse(0));

        let tmp = tempfile::tempdir().expect("create Windows reparse fixture");
        let target = tmp.path().join("target.mp4");
        let reparse = tmp.path().join("reparse.mp4");
        std::fs::write(&target, b"target").expect("write reparse target");
        match symlink_file(&target, &reparse) {
            Ok(()) => assert!(
                RetainedExternalSource::open(&reparse).is_err(),
                "a Windows file reparse point must never become a retained source"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                // Hosted Windows configurations without Developer Mode cannot
                // create a file symlink; the attribute predicate above remains
                // an unconditional executable contract.
            }
            Err(error) => panic!("create Windows file reparse point: {error}"),
        }
    }

    #[test]
    fn fit_render_size_downscales_to_longest_edge_keeping_aspect() {
        // 1920x1080, cap 512 → scale 512/1920 → 512x288 (even-ized).
        let rs = fit_render_size(1920, 1080, 512);
        assert_eq!(rs, RenderSize::new(512, 288));
    }

    #[test]
    fn fit_render_size_never_upscales_under_cap() {
        let rs = fit_render_size(320, 240, 512);
        assert_eq!(rs, RenderSize::new(320, 240));
    }

    #[test]
    fn fit_render_size_no_cap_just_evenizes() {
        let rs = fit_render_size(1921, 1081, 0);
        assert_eq!(rs, RenderSize::new(1920, 1080));
    }

    #[test]
    fn clip_type_name_is_lowercase_raw_value() {
        assert_eq!(clip_type_name(ClipType::Video), "video");
        assert_eq!(clip_type_name(ClipType::Audio), "audio");
        assert_eq!(clip_type_name(ClipType::Image), "image");
    }

    #[test]
    fn short_uuid_is_eight_hex_chars() {
        let s = short_uuid();
        assert_eq!(s.len(), 8);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn rgba_to_rgb_drops_alpha_channel() {
        // Two pixels: (1,2,3,255), (4,5,6,128) → RGB only.
        let rgba = vec![1, 2, 3, 255, 4, 5, 6, 128];
        assert_eq!(rgba_to_rgb(&rgba), vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn encode_jpeg_produces_jpeg_soi_marker() {
        // 16x16 opaque RGBA composite → a valid JPEG (alpha flattened to RGB).
        let frame = DecodedFrame::new(16, 16, vec![255u8; 16 * 16 * 4], false);
        let bytes = encode_jpeg(&frame).expect("jpeg encodes");
        // JPEG files start with the SOI marker 0xFFD8.
        assert_eq!(&bytes[..2], &[0xff, 0xd8]);
    }

    #[test]
    fn inspect_media_decodes_an_imported_image_end_to_end() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let source = tmp.path().join("source.png");
        image::RgbaImage::from_pixel(48, 24, image::Rgba([12, 34, 56, 255]))
            .save(&source)
            .expect("write image fixture");

        let core = AppCore::new();
        core.save_project(Some(tmp.path().join("Inspect.opentake")))
            .expect("save image project");
        let entry = core
            .import_media_file(
                &source,
                "source",
                &ProbedMedia {
                    width: Some(48),
                    height: Some(24),
                    ..ProbedMedia::default()
                },
            )
            .expect("import image fixture");
        let engine = MediaEngine::new(tmp.path().join("cache"), tmp.path().join("models"));

        let result = inspect_source_media(
            &core,
            &engine,
            &InspectMediaRequest {
                media_ref: entry.id,
                kind: ClipType::Image,
                start_seconds: None,
                end_seconds: None,
                max_frames: 1,
                overview: false,
            },
        )
        .expect("inspect imported image");

        assert_eq!(result.width, Some(48));
        assert_eq!(result.height, Some(24));
        assert_eq!(result.frames.len(), 1);
        assert_eq!(result.frames[0].media_type, "image/jpeg");
        assert_eq!(result.frames[0].timestamp_seconds, 0.0);
        assert_eq!(
            image::load_from_memory(&result.frames[0].bytes)
                .expect("decode inspected JPEG")
                .into_rgba8()
                .dimensions(),
            (48, 24)
        );
        assert_eq!(
            result.byte_size,
            std::fs::metadata(source).expect("source metadata").len()
        );
    }

    fn write_bundle(path: &Path, color: image::Rgba<u8>) {
        let project = opentake_project::Project::new(path);
        project.save().unwrap();
        std::fs::create_dir_all(path.join("media")).unwrap();
        image::RgbaImage::from_pixel(12, 12, color)
            .save(path.join("media/source.png"))
            .unwrap();
        let mut manifest = opentake_domain::MediaManifest::new();
        manifest.entries.push(opentake_domain::MediaManifestEntry {
            id: "project-image".into(),
            name: "source.png".into(),
            kind: ClipType::Image,
            source: MediaSource::Project {
                relative_path: "media/source.png".into(),
            },
            duration: 0.0,
            generation_input: None,
            source_width: Some(12),
            source_height: Some(12),
            source_fps: None,
            has_audio: Some(false),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        });
        std::fs::write(
            path.join("media.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn inspect_project_media_reads_the_retained_bundle_after_path_rebind() {
        let tmp = tempfile::tempdir().unwrap();
        let selected = tmp.path().join("Selected.opentake");
        let retained = tmp.path().join("Retained.opentake");
        write_bundle(&selected, image::Rgba([240, 10, 10, 255]));
        let core = AppCore::new();
        core.open_project(&selected).unwrap();
        std::fs::rename(&selected, &retained).unwrap();
        write_bundle(&selected, image::Rgba([10, 240, 10, 255]));
        let engine = MediaEngine::new(tmp.path().join("cache"), tmp.path().join("models"));

        let result = inspect_source_media(
            &core,
            &engine,
            &InspectMediaRequest {
                media_ref: "project-image".into(),
                kind: ClipType::Image,
                start_seconds: None,
                end_seconds: None,
                max_frames: 1,
                overview: false,
            },
        )
        .unwrap();
        let pixel = image::load_from_memory(&result.frames[0].bytes)
            .unwrap()
            .into_rgb8()
            .get_pixel(6, 6)
            .0;

        assert!(
            pixel[0] > pixel[1] + 100,
            "inspection reopened the rebound bundle instead of retained bytes: {pixel:?}"
        );
    }

    /// cap-std retains the bundle without FILE_SHARE_DELETE: on Windows the
    /// ambient rename fails closed while the project is open (the
    /// retained-read-after-rebind property is Unix-verified above).
    #[cfg(target_os = "windows")]
    #[test]
    fn inspect_project_media_blocks_bundle_rebind_while_open() {
        let tmp = tempfile::tempdir().unwrap();
        let selected = tmp.path().join("Selected.opentake");
        let retained = tmp.path().join("Retained.opentake");
        write_bundle(&selected, image::Rgba([240, 10, 10, 255]));
        let core = AppCore::new();
        core.open_project(&selected).unwrap();

        assert!(std::fs::rename(&selected, &retained).is_err());

        drop(core);
        std::fs::rename(&selected, &retained).unwrap();
    }

    fn two_frame_lottie_fixture() -> String {
        r##"{
  "v":"5.5.2","fr":2,"ip":0,"op":2,"w":16,"h":16,"ddd":0,"assets":[],
  "layers":[
    {"ddd":0,"ind":1,"ty":4,"nm":"red",
      "ks":{"o":{"a":0,"k":100},"r":{"a":0,"k":0},"p":{"a":0,"k":[8,8,0]},
             "a":{"a":0,"k":[8,8,0]},"s":{"a":0,"k":[100,100,100]}},
      "shapes":[
        {"ty":"rc","d":1,"s":{"a":0,"k":[8,8]},"p":{"a":0,"k":[8,8]},"r":{"a":0,"k":0}},
        {"ty":"fl","c":{"a":0,"k":[1,0,0,1]},"o":{"a":0,"k":100},"r":1}
      ],"ao":0,"ip":0,"op":1,"st":0,"bm":0},
    {"ddd":0,"ind":2,"ty":4,"nm":"green",
      "ks":{"o":{"a":0,"k":100},"r":{"a":0,"k":0},"p":{"a":0,"k":[8,8,0]},
             "a":{"a":0,"k":[8,8,0]},"s":{"a":0,"k":[100,100,100]}},
      "shapes":[
        {"ty":"rc","d":1,"s":{"a":0,"k":[8,8]},"p":{"a":0,"k":[8,8]},"r":{"a":0,"k":0}},
        {"ty":"fl","c":{"a":0,"k":[0,1,0,1]},"o":{"a":0,"k":100},"r":1}
      ],"ao":0,"ip":1,"op":2,"st":0,"bm":0}
  ]
}"##
        .to_string()
    }

    #[test]
    fn inspect_media_renders_lottie_frames_over_gray_end_to_end() {
        let Ok(_) = RenderDevice::try_new() else {
            return;
        };
        let tmp = tempfile::tempdir().expect("create temp root");
        let bundle = tmp.path().join("InspectLottie.opentake");
        let project = opentake_project::Project::new(&bundle);
        project.save().expect("save Lottie project");
        let source = bundle.join("media/animation.json");
        std::fs::create_dir_all(source.parent().expect("media parent"))
            .expect("create media directory");
        std::fs::write(&source, two_frame_lottie_fixture()).expect("write Lottie fixture");
        let mut manifest = opentake_domain::MediaManifest::new();
        manifest.entries.push(opentake_domain::MediaManifestEntry {
            id: "lottie-asset".into(),
            name: "animation".into(),
            kind: ClipType::Lottie,
            source: MediaSource::Project {
                relative_path: "media/animation.json".into(),
            },
            duration: 1.0,
            generation_input: None,
            source_width: Some(16),
            source_height: Some(16),
            source_fps: Some(2.0),
            has_audio: Some(false),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        });
        std::fs::write(
            bundle.join("media.json"),
            serde_json::to_vec_pretty(&manifest).expect("encode Lottie manifest"),
        )
        .expect("write Lottie manifest");
        let core = AppCore::new();
        core.open_project(bundle).expect("open Lottie project");
        let engine = MediaEngine::new(tmp.path().join("cache"), tmp.path().join("models"));

        let result = inspect_source_media(
            &core,
            &engine,
            &InspectMediaRequest {
                media_ref: "lottie-asset".into(),
                kind: ClipType::Lottie,
                start_seconds: None,
                end_seconds: None,
                max_frames: 2,
                overview: false,
            },
        )
        .expect("inspect imported Lottie");

        assert_eq!((result.width, result.height), (Some(16), Some(16)));
        assert_eq!(result.fps, Some(2.0));
        assert_eq!(result.duration_seconds, 1.0);
        assert_eq!(result.frames.len(), 2);
        assert_eq!(result.frames[0].timestamp_seconds, 0.25);
        assert_eq!(result.frames[1].timestamp_seconds, 0.75);
        let decoded = result
            .frames
            .iter()
            .map(|frame| {
                assert_eq!(frame.media_type, "image/jpeg");
                image::load_from_memory(&frame.bytes)
                    .expect("decode inspected Lottie JPEG")
                    .into_rgb8()
            })
            .collect::<Vec<_>>();
        assert_ne!(decoded[0].as_raw(), decoded[1].as_raw());
        let first_center = decoded[0].get_pixel(8, 8).0;
        let second_center = decoded[1].get_pixel(8, 8).0;
        assert!(first_center[0] > first_center[1] + 80, "{first_center:?}");
        assert!(
            second_center[1] > second_center[0] + 80,
            "{second_center:?}"
        );
        for frame in &decoded {
            let corner = frame.get_pixel(0, 0).0;
            assert!(
                (corner[0] as i16 - corner[1] as i16).abs() < 30,
                "{corner:?}"
            );
            assert!(
                (corner[1] as i16 - corner[2] as i16).abs() < 30,
                "{corner:?}"
            );
            assert!(corner[0] > 80 && corner[0] < 180, "{corner:?}");
        }
    }

    #[test]
    fn inspect_media_overview_encodes_the_expected_storyboard_grid() {
        let frames = (0..7)
            .map(|index| {
                (
                    f64::from(index),
                    RgbaFrame::new(16, 9, vec![index as u8; 16 * 9 * 4]),
                )
            })
            .collect::<Vec<_>>();

        let (bytes, width, height) =
            encode_storyboard_jpeg(&frames).expect("encode overview storyboard");

        assert_eq!((width, height), (6 * 192, 2 * 108));
        assert_eq!(
            image::load_from_memory(&bytes)
                .expect("decode overview JPEG")
                .into_rgba8()
                .dimensions(),
            (width, height)
        );
    }

    #[cfg(unix)]
    #[test]
    fn inspect_media_rejects_a_symlink_source() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let target = tmp.path().join("target.png");
        let source = tmp.path().join("source.png");
        image::RgbaImage::from_pixel(8, 8, image::Rgba([255, 255, 255, 255]))
            .save(&target)
            .expect("write image target");
        std::os::unix::fs::symlink(&target, &source).expect("create image symlink");

        let core = AppCore::new();
        core.save_project(Some(tmp.path().join("Symlink.opentake")))
            .expect("save symlink project");
        let entry = core
            .import_media_file(&source, "source", &ProbedMedia::default())
            .expect("import symlink fixture");
        let engine = MediaEngine::new(tmp.path().join("cache"), tmp.path().join("models"));

        let error = inspect_source_media(
            &core,
            &engine,
            &InspectMediaRequest {
                media_ref: entry.id,
                kind: ClipType::Image,
                start_seconds: None,
                end_seconds: None,
                max_frames: 1,
                overview: false,
            },
        )
        .expect_err("symlink source must be rejected");

        assert_eq!(
            error.to_string(),
            "inspect_media: source media is not a regular file"
        );
    }

    #[test]
    fn https_url_import_enforces_scheme_mime_and_decoded_limit() {
        use std::collections::VecDeque;
        use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
        use std::sync::Mutex;

        struct FakeFetcher {
            responses: Mutex<VecDeque<UrlFetchResponse>>,
            seen: Mutex<Vec<String>>,
        }
        impl FakeFetcher {
            fn new(responses: Vec<UrlFetchResponse>) -> Self {
                Self {
                    responses: Mutex::new(responses.into()),
                    seen: Mutex::new(Vec::new()),
                }
            }
        }
        impl UrlFetcher for FakeFetcher {
            fn fetch(
                &self,
                url: &reqwest::Url,
                _cancel: &opentake_media::MediaCancelToken,
            ) -> Result<UrlFetchResponse, BridgeError> {
                self.seen.lock().unwrap().push(url.as_str().to_string());
                self.responses
                    .lock()
                    .unwrap()
                    .pop_front()
                    .ok_or_else(|| BridgeError::new("unexpected fetch"))
            }
        }
        fn response(
            status: reqwest::StatusCode,
            location: Option<&str>,
            mime: Option<&str>,
            length: Option<u64>,
            body: impl Read + Send + 'static,
        ) -> UrlFetchResponse {
            UrlFetchResponse {
                status,
                location: location.map(str::to_owned),
                content_type: mime.map(str::to_owned),
                content_length: length,
                body: Box::new(ReaderUrlBody {
                    reader: Box::new(body),
                }),
            }
        }
        struct CancelAfterFirstRead {
            cursor: std::io::Cursor<Vec<u8>>,
            cancel: opentake_media::MediaCancelToken,
            fired: bool,
        }
        impl Read for CancelAfterFirstRead {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                let count = self.cursor.read(buffer)?;
                if count > 0 && !self.fired {
                    self.fired = true;
                    self.cancel.cancel();
                }
                Ok(count)
            }
        }
        fn saved_bridge(root: &Path) -> (TauriMediaBridge, AppCore, PathBuf) {
            let bundle = root.join("UrlImport.opentake");
            let core = AppCore::new();
            core.save_project(Some(bundle.clone()))
                .expect("save URL import fixture");
            let bridge =
                TauriMediaBridge::new(core.clone(), root.join("cache"), root.join("models"));
            (bridge, core, bundle)
        }
        fn assert_unchanged(
            core: &AppCore,
            bundle: &Path,
            before: &opentake_domain::MediaManifest,
            disk: &[u8],
        ) {
            assert_eq!(&core.media(), before, "live manifest changed after failure");
            assert_eq!(
                std::fs::read(bundle.join("media.json")).expect("read manifest after failure"),
                disk,
                "persistent manifest bytes changed after failure"
            );
            let media_dir = bundle.join("media");
            if media_dir.exists() {
                assert_eq!(
                    std::fs::read_dir(media_dir).unwrap().count(),
                    0,
                    "uncommitted URL staging leaf survived"
                );
            }
        }
        fn persist_manifest(
            capability: &ProjectMediaCapability,
            manifest: &opentake_domain::MediaManifest,
        ) -> Result<(), CoreError> {
            capability
                .write_manifest(manifest)
                .map_err(CoreError::Media)
        }

        // Initial URL validation is strictly pre-fetch.
        let tmp = tempfile::tempdir().unwrap();
        let (bridge, core, bundle) = saved_bridge(tmp.path());
        let manifest_before = core.media();
        let disk_before = std::fs::read(bundle.join("media.json")).unwrap();
        for url in [
            "http://example.com/a.mp4",
            "https://",
            "https://user@example.com/a.mp4",
            "https://127.0.0.1/a.mp4",
            "https://10.0.0.1/a.mp4",
            "https://169.254.169.254/latest/meta-data",
            "https://[::1]/a.mp4",
            "https://[fc00::1]/a.mp4",
            "https://[fe80::1]/a.mp4",
        ] {
            let fetcher = FakeFetcher::new(Vec::new());
            let err = bridge
                .import_from_url_with(
                    &fetcher,
                    url,
                    None,
                    None,
                    None,
                    8,
                    &opentake_media::MediaCancelToken::new(),
                    |_, _, _| Ok(ProbedMedia::default()),
                    persist_manifest,
                )
                .unwrap_err();
            assert!(
                err.message.contains("HTTPS")
                    || err.message.contains("host")
                    || err.message.contains("userinfo")
                    || err.message.contains("invalid")
                    || err.message.contains("non-public"),
                "{url}: {}",
                err.message
            );
            assert!(fetcher.seen.lock().unwrap().is_empty());
            assert_unchanged(&core, &bundle, &manifest_before, &disk_before);
        }

        // Every redirect target is validated before the next request.
        for target in [
            "http://example.com/final.mp4",
            "https://user@example.com/final.mp4",
            "https://127.0.0.1/final.mp4",
            "https://[::1]/final.mp4",
        ] {
            let fetcher = FakeFetcher::new(vec![response(
                reqwest::StatusCode::FOUND,
                Some(target),
                None,
                None,
                std::io::Cursor::new(Vec::new()),
            )]);
            bridge
                .import_from_url_with(
                    &fetcher,
                    "https://example.com/start.mp4",
                    None,
                    None,
                    None,
                    8,
                    &opentake_media::MediaCancelToken::new(),
                    |_, _, _| Ok(ProbedMedia::default()),
                    persist_manifest,
                )
                .expect_err("unsafe redirect target must fail");
            assert_eq!(fetcher.seen.lock().unwrap().len(), 1);
            assert_unchanged(&core, &bundle, &manifest_before, &disk_before);
        }

        // Declared and authoritative streamed sizes are independently capped;
        // unsupported MIME/extension combinations never publish either.
        for (url, mime, length, body) in [
            (
                "https://example.com/a.mp4",
                Some("video/mp4"),
                Some(9),
                vec![1],
            ),
            (
                "https://example.com/a.mp4",
                Some("video/mp4"),
                None,
                vec![1; 9],
            ),
            (
                "https://example.com/a.mp4",
                Some("video/mp4"),
                Some(1),
                vec![1; 9],
            ),
            (
                "https://example.com/a.exe",
                Some("video/mp4"),
                Some(1),
                vec![1],
            ),
            (
                "https://example.com/a.mp4",
                Some("application/zip"),
                Some(1),
                vec![1],
            ),
        ] {
            let fetcher = FakeFetcher::new(vec![response(
                reqwest::StatusCode::OK,
                None,
                mime,
                length,
                std::io::Cursor::new(body),
            )]);
            bridge
                .import_from_url_with(
                    &fetcher,
                    url,
                    None,
                    None,
                    None,
                    8,
                    &opentake_media::MediaCancelToken::new(),
                    |_, _, _| Ok(ProbedMedia::default()),
                    persist_manifest,
                )
                .expect_err("invalid MIME/extension/size must fail");
            assert_unchanged(&core, &bundle, &manifest_before, &disk_before);
        }

        // Explicit source.mimeType overrides an absent or unusable URL path
        // extension. The response MIME and injected probe still validate the
        // selected type before either candidate is published.
        let override_tmp = tempfile::tempdir().unwrap();
        let (override_bridge, override_core, override_bundle) = saved_bridge(override_tmp.path());
        for url in [
            "https://example.com/download",
            "https://example.com/opaque.exe?signature=secret",
        ] {
            let fetcher = FakeFetcher::new(vec![response(
                reqwest::StatusCode::OK,
                None,
                Some("video/mp4"),
                Some(4),
                std::io::Cursor::new(vec![1, 2, 3, 4]),
            )]);
            override_bridge
                .import_from_url_with(
                    &fetcher,
                    url,
                    Some("video/mp4"),
                    None,
                    None,
                    8,
                    &opentake_media::MediaCancelToken::new(),
                    |_, extension, kind| {
                        assert_eq!(extension, "mp4");
                        assert_eq!(kind, "video");
                        Ok(ProbedMedia::default())
                    },
                    persist_manifest,
                )
                .expect("explicit source.mimeType overrides the URL path extension");
        }
        assert_eq!(override_core.media().entries.len(), 2);
        let override_persisted = opentake_project::Project::open(&override_bundle)
            .expect("reopen MIME override fixture");
        assert_eq!(override_persisted.manifest.entries.len(), 2);
        assert!(override_persisted.manifest.entries.iter().all(|entry| {
            matches!(
                &entry.source,
                MediaSource::Project { relative_path } if relative_path.ends_with(".mp4")
            )
        }));

        // The limit itself is accepted (strictly greater is rejected), even
        // when Content-Length understates the streamed body. The injected probe
        // stops before publication so the shared fixture remains unchanged.
        let fetcher = FakeFetcher::new(vec![response(
            reqwest::StatusCode::OK,
            None,
            Some("video/mp4"),
            Some(1),
            std::io::Cursor::new(vec![1; 8]),
        )]);
        let exact_limit = bridge
            .import_from_url_with(
                &fetcher,
                "https://example.com/exact.mp4",
                None,
                None,
                None,
                8,
                &opentake_media::MediaCancelToken::new(),
                |_, _, _| Err(BridgeError::new("exact limit reached probe")),
                persist_manifest,
            )
            .expect_err("probe deliberately stops exact-limit fixture");
        assert!(
            exact_limit.message.contains("exact limit reached probe"),
            "{}",
            exact_limit.message
        );
        assert_unchanged(&core, &bundle, &manifest_before, &disk_before);

        // Cancellation after a partial read drops the retained candidate and
        // leaves both the live and byte-for-byte persistent manifest untouched.
        let cancel = opentake_media::MediaCancelToken::new();
        let body = CancelAfterFirstRead {
            cursor: std::io::Cursor::new(vec![1; 8]),
            cancel: cancel.clone(),
            fired: false,
        };
        let fetcher = FakeFetcher::new(vec![response(
            reqwest::StatusCode::OK,
            None,
            Some("video/mp4"),
            None,
            body,
        )]);
        bridge
            .import_from_url_with(
                &fetcher,
                "https://example.com/cancel.mp4",
                None,
                None,
                None,
                8,
                &cancel,
                |_, _, _| Ok(ProbedMedia::default()),
                persist_manifest,
            )
            .expect_err("cancelled stream must fail");
        assert_unchanged(&core, &bundle, &manifest_before, &disk_before);

        for fail_probe in [true, false] {
            let fetcher = FakeFetcher::new(vec![response(
                reqwest::StatusCode::OK,
                None,
                Some("video/mp4"),
                Some(4),
                std::io::Cursor::new(vec![1, 2, 3, 4]),
            )]);
            let result = bridge.import_from_url_with(
                &fetcher,
                "https://example.com/failure.mp4",
                None,
                None,
                None,
                8,
                &opentake_media::MediaCancelToken::new(),
                move |_, _, _| {
                    if fail_probe {
                        Err(BridgeError::new("injected probe failure"))
                    } else {
                        Ok(ProbedMedia::default())
                    }
                },
                move |capability, manifest| {
                    if fail_probe {
                        persist_manifest(capability, manifest)
                    } else {
                        Err(CoreError::Media("injected writer failure".to_string()))
                    }
                },
            );
            let error = result.expect_err("probe/writer fault must fail closed");
            assert!(
                error.message.contains("probe") || error.message.contains("writer"),
                "{}",
                error.message
            );
            assert_unchanged(&core, &bundle, &manifest_before, &disk_before);
        }

        // A successful HTTPS redirect chain probes the retained bytes before
        // publication, persists the manifest, and survives a project reopen.
        let probed = Arc::new(AtomicBool::new(false));
        let probed_in_closure = probed.clone();
        let fetcher = FakeFetcher::new(vec![
            response(
                reqwest::StatusCode::TEMPORARY_REDIRECT,
                Some("/final.mp4"),
                None,
                None,
                std::io::Cursor::new(Vec::new()),
            ),
            response(
                reqwest::StatusCode::OK,
                None,
                Some("video/mp4; charset=binary"),
                Some(4),
                std::io::Cursor::new(vec![1, 2, 3, 4]),
            ),
        ]);
        bridge
            .import_from_url_with(
                &fetcher,
                "https://example.com/start.mp4",
                None,
                Some("Remote clip"),
                None,
                8,
                &opentake_media::MediaCancelToken::new(),
                move |file, _, _| {
                    let mut bytes = Vec::new();
                    file.try_clone().unwrap().read_to_end(&mut bytes).unwrap();
                    assert_eq!(bytes, vec![1, 2, 3, 4]);
                    probed_in_closure.store(true, AtomicOrdering::Release);
                    Ok(ProbedMedia::default())
                },
                persist_manifest,
            )
            .expect("valid HTTPS import succeeds");
        assert!(probed.load(AtomicOrdering::Acquire));
        assert_eq!(core.media().entries.len(), 1);
        let persisted = opentake_project::Project::open(&bundle).expect("reopen persisted project");
        assert_eq!(persisted.manifest.entries.len(), 1);
        let entry = &persisted.manifest.entries[0];
        assert_eq!(entry.name, "Remote clip");
        let relative = match &entry.source {
            MediaSource::Project { relative_path } => relative_path,
            other => panic!("URL import must be project-retained, got {other:?}"),
        };
        assert_eq!(
            std::fs::read(bundle.join(relative)).unwrap(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(fetcher.seen.lock().unwrap().len(), 2);
    }

    #[test]
    fn public_address_policy_rejects_private_reserved_and_mixed_dns_results() {
        for ip in [
            "0.0.0.0",
            "10.1.2.3",
            "100.64.0.1",
            "127.0.0.1",
            "169.254.169.254",
            "172.16.0.1",
            "192.168.0.1",
            "198.18.0.1",
            "224.0.0.1",
            "::",
            "::1",
            "::ffff:127.0.0.1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
        ] {
            let ip = ip.parse::<IpAddr>().unwrap();
            assert!(!public_ip(ip), "{ip} must not be treated as public");
            assert!(ensure_public_ip(ip).is_err(), "{ip} must fail closed");
        }

        let public_v4 = "93.184.216.34:443".parse::<SocketAddr>().unwrap();
        let public_v6 = "[2606:4700:4700::1111]:443".parse::<SocketAddr>().unwrap();
        assert_eq!(pin_public_address(vec![public_v4]).unwrap(), public_v4);
        assert_eq!(pin_public_address(vec![public_v6]).unwrap(), public_v6);
        assert!(pin_public_address(Vec::new()).is_err());
        assert!(pin_public_address(vec![public_v4, "127.0.0.1:443".parse().unwrap(),]).is_err());
        assert!(pin_public_address(vec![public_v6, "[fc00::1]:443".parse().unwrap(),]).is_err());
    }

    #[test]
    fn reqwest_fetch_rejects_loopback_before_connection_and_redacts_signed_url() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let signed = reqwest::Url::parse(&format!(
            "https://{addr}/media.mp4?token=super-secret-query"
        ))
        .unwrap();

        let started = std::time::Instant::now();
        let error = ReqwestUrlFetcher::new()
            .unwrap()
            .fetch(&signed, &opentake_media::MediaCancelToken::new())
            .err()
            .expect("loopback target must be rejected");

        assert!(error.message.contains("non-public"), "{}", error.message);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(!error.message.contains("super-secret-query"));
        assert!(!error.message.contains(signed.as_str()));
        assert!(matches!(
            listener.accept(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
        ));
    }

    #[test]
    fn bytes_import_without_open_project_errors_after_valid_decode() {
        // A fresh AppCore has no project dir; a valid base64 png payload with a
        // known mime still can't be written (no bundle) — matches upstream's
        // "No project is open" guard, and proves the mime + base64 checks passed.
        let bridge = TauriMediaBridge::new(
            AppCore::new(),
            std::env::temp_dir().join("inspect-cache"),
            std::env::temp_dir().join("inspect-models"),
        );
        let b64 = base64::engine::general_purpose::STANDARD.encode([1u8, 2, 3, 4]);
        let err = bridge
            .import_from_bytes(&b64, "image/png", None, None)
            .unwrap_err();
        assert!(
            err.message.contains("No project is open"),
            "{}",
            err.message
        );
    }

    #[test]
    fn bytes_import_rejects_decoded_payload_over_limit() {
        let bridge = TauriMediaBridge::new(
            AppCore::new(),
            std::env::temp_dir().join("inspect-cache"),
            std::env::temp_dir().join("inspect-models"),
        );
        let oversized = vec![0u8; IMPORT_BYTES_DECODED_MAX + 1];
        let b64 = base64::engine::general_purpose::STANDARD.encode(oversized);
        let err = bridge
            .import_from_bytes(&b64, "image/png", None, None)
            .unwrap_err();
        assert!(
            err.message.contains("decoded payload is too large"),
            "{}",
            err.message
        );
    }

    #[test]
    fn bytes_import_rejects_unknown_mime() {
        let bridge = TauriMediaBridge::new(
            AppCore::new(),
            std::env::temp_dir().join("inspect-cache"),
            std::env::temp_dir().join("inspect-models"),
        );
        let err = bridge
            .import_from_bytes("AAAA", "application/zip", None, None)
            .unwrap_err();
        assert!(
            err.message.contains("Unsupported mimeType"),
            "{}",
            err.message
        );
    }

    // MARK: - ffmpeg + GPU gated end-to-end (mirrors the export integration skip
    // discipline: auto-skip when ffmpeg is off PATH or no GPU adapter is present).

    use std::process::Command;

    /// True when ffmpeg is on PATH (fixture generation).
    fn ffmpeg_ready() -> bool {
        Command::new("ffmpeg")
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Generate an `frames`-frame test video at `path`. Returns false (→ skip).
    fn make_video(path: &Path, w: u32, h: u32, fps: u32, frames: u32) -> bool {
        let dur = frames as f64 / fps as f64;
        Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                &format!("testsrc=duration={dur}:size={w}x{h}:rate={fps}"),
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-y",
            ])
            .arg(path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn make_audio_with_cover(path: &Path) -> bool {
        Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=1",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=32x32:d=1",
                "-map",
                "0:a",
                "-map",
                "1:v",
                "-c:a",
                "libmp3lame",
                "-c:v",
                "mjpeg",
                "-disposition:v",
                "attached_pic",
                "-y",
            ])
            .arg(path)
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[test]
    fn url_probe_validates_real_container_and_audio_cover_art() {
        if !ffmpeg_ready() {
            eprintln!("skip: ffmpeg not available");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let bridge = TauriMediaBridge::new(
            AppCore::new(),
            tmp.path().join("cache"),
            tmp.path().join("models"),
        );

        let video = tmp.path().join("fixture.mp4");
        if !make_video(&video, 32, 18, 1, 1) {
            eprintln!("skip: could not generate video probe fixture");
            return;
        }
        let video_file = std::fs::File::open(&video).unwrap();
        bridge
            .probe_required(&video_file, "mp4", "video")
            .expect("real MP4 passes production URL probe");
        bridge
            .probe_required(&video_file, "mp3", "audio")
            .expect_err("real MP4 cannot masquerade as MP3 audio");

        let audio = tmp.path().join("cover.mp3");
        if !make_audio_with_cover(&audio) {
            eprintln!("skip: could not generate covered MP3 probe fixture");
            return;
        }
        let audio_file = std::fs::File::open(&audio).unwrap();
        let probed = bridge
            .probe_required(&audio_file, "mp3", "audio")
            .expect("MP3 with attached cover remains audio");
        assert!(probed.has_audio);
        assert_eq!(probed.width, None);
        assert_eq!(probed.height, None);
    }

    fn external_entry(
        id: &str,
        path: &Path,
        w: i32,
        h: i32,
    ) -> opentake_domain::MediaManifestEntry {
        opentake_domain::MediaManifestEntry {
            id: id.into(),
            name: id.into(),
            kind: ClipType::Video,
            source: MediaSource::External {
                absolute_path: path.to_string_lossy().into_owned(),
            },
            duration: 2.0,
            generation_input: None,
            source_width: Some(w),
            source_height: Some(h),
            source_fps: Some(30.0),
            has_audio: Some(false),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        }
    }

    #[test]
    fn inspect_timeline_composites_real_frames_when_gpu_available() {
        if !ffmpeg_ready() {
            eprintln!("skip: ffmpeg not available");
            return;
        }
        // A GPU adapter may be unavailable in CI/headless — skip, don't fail
        // (same policy as the export integration test).
        if opentake_render::RenderDevice::try_new().is_err() {
            eprintln!("skip: no GPU adapter available");
            return;
        }

        let tmp = tempfile::tempdir().unwrap();
        let video = tmp.path().join("clip.mp4");
        if !make_video(&video, 320, 240, 30, 30) {
            eprintln!("skip: could not generate fixture media");
            return;
        }

        // A 30-frame timeline over the fixture clip.
        let mut timeline = opentake_domain::Timeline::new();
        timeline.width = 320;
        timeline.height = 240;
        timeline.fps = 30;
        let mut track = opentake_domain::Track::new("track-1", ClipType::Video);
        track
            .clips
            .push(opentake_domain::Clip::new("clip-1", "asset-1", 0, 30));
        timeline.tracks.push(track);
        let mut manifest = opentake_domain::MediaManifest::new();
        manifest
            .entries
            .push(external_entry("asset-1", &video, 320, 240));

        // Sample 3 frames across [0, 30) at the 512px cap.
        let res = composite_frames_jpeg(&timeline, &manifest, &None, &[0, 10, 20], 512)
            .expect("composite should succeed with a GPU + fixture");
        assert_eq!(res.frames.len(), 3);
        // 320x240 is already under 512 → unscaled.
        assert_eq!((res.width, res.height), (320, 240));
        for f in &res.frames {
            assert_eq!(f.media_type, "image/jpeg");
            assert_eq!(&f.bytes[..2], &[0xff, 0xd8], "each frame is a JPEG");
        }
        assert_eq!(
            res.frames.iter().map(|f| f.frame).collect::<Vec<_>>(),
            vec![0, 10, 20]
        );
    }

    #[test]
    fn import_from_path_single_file_registers_asset_end_to_end() {
        if !ffmpeg_ready() {
            eprintln!("skip: ffmpeg not available");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let video = tmp.path().join("My Clip.mp4");
        if !make_video(&video, 160, 120, 30, 15) {
            eprintln!("skip: could not generate fixture media");
            return;
        }
        // Imports are capability-bound to an open project, while the source
        // itself remains referenced in place.
        let core = AppCore::new();
        core.save_project(Some(tmp.path().join("PathImport.opentake")))
            .unwrap();
        let bridge =
            TauriMediaBridge::new(core, tmp.path().join("cache"), tmp.path().join("models"));
        let out = bridge
            .import_from_path(&video.to_string_lossy(), None, None)
            .expect("single-file path import");
        assert_eq!(out.asset_count, 1);
        assert_eq!(out.folder_count, 0);
        assert!(!out.recovery_required);
        // The asset is now in the shared core's manifest, named by its stem.
        let manifest = bridge.core.media();
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(manifest.entries[0].name, "My Clip");
        assert_eq!(manifest.entries[0].kind, ClipType::Video);
    }

    #[test]
    fn import_from_path_missing_file_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let core = AppCore::new();
        core.save_project(Some(tmp.path().join("MissingPath.opentake")))
            .unwrap();
        let bridge =
            TauriMediaBridge::new(core, tmp.path().join("cache"), tmp.path().join("models"));
        let missing = tmp.path().join("missing.mp4");
        let err = bridge
            .import_from_path(&missing.to_string_lossy(), None, None)
            .unwrap_err();
        assert!(
            err.message.contains("MCP_SOURCE_PATH_UNREADABLE"),
            "{}",
            err.message
        );
    }

    #[test]
    fn import_from_path_unsupported_extension_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = tmp.path().join("notes.txt");
        std::fs::write(&doc, b"x").unwrap();
        let core = AppCore::new();
        core.save_project(Some(tmp.path().join("UnsupportedPath.opentake")))
            .unwrap();
        let bridge =
            TauriMediaBridge::new(core, tmp.path().join("cache"), tmp.path().join("models"));
        let err = bridge
            .import_from_path(&doc.to_string_lossy(), None, None)
            .unwrap_err();
        assert!(
            err.message.contains("Unsupported file extension"),
            "{}",
            err.message
        );
    }

    #[cfg(unix)]
    #[test]
    fn path_import_without_project_rejects_before_fifo_source_io() {
        let tmp = tempfile::tempdir().unwrap();
        let fifo = tmp.path().join("untrusted.mp4");
        let status = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("create untrusted FIFO");
        assert!(status.success());
        let bridge = TauriMediaBridge::new(
            AppCore::new(),
            tmp.path().join("cache"),
            tmp.path().join("models"),
        );
        let cancel = opentake_media::MediaCancelToken::new();
        let started = std::time::Instant::now();

        let error = bridge
            .import_from_path_cancellable(&fifo.to_string_lossy(), None, None, &cancel)
            .unwrap_err();

        assert_eq!(
            error.message,
            "No project is open; cannot import source.path"
        );
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(
            cancel.spawned_child_count(),
            0,
            "project authorization must precede source probing"
        );
    }
}
