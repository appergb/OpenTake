//! Spawns the loopback MCP server (#36) on the Tauri async runtime, and wires the
//! agent's render + import side-door ([`MediaBridge`]).
//!
//! The server exposes the in-process tool dispatcher over Streamable-HTTP at
//! `http://127.0.0.1:19789/mcp` so external agents (`claude mcp add --transport
//! http opentake http://127.0.0.1:19789/mcp`, Cursor, Codex, …) can drive the
//! same [`AppCore`] the UI edits. The plugin registry seeds the bundled
//! workflows (e.g. the default audio-first Skill) plus any user-authored plugins
//! under `<app_data_dir>/workflows`.
//!
//! `inspect_timeline` and `import_media` need capabilities that live outside
//! `opentake-core` — GPU compositing (`opentake-render`) and the user-facing
//! import path (`crate::media`). The agent crate can't (by design shouldn't) link
//! those, so it takes them through the injected [`MediaBridge`]. This module is
//! where that boundary is implemented ([`TauriMediaBridge`]) and handed to the
//! dispatcher: it owns a session-sharing [`AppCore`] clone plus a [`MediaEngine`]
//! built from the same cache/models dirs the UI uses, so imports produce the exact
//! same posters / manifest entries / `MediaChanged` events as the media panel.

use std::collections::HashMap;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::time::Duration;

#[cfg(test)]
use std::io::Read;

use base64::Engine as _;

use opentake_agent::mcp::core_handle::{AppCoreHandle, CoreHandle};
use opentake_agent::mcp::generation::GenerationBridge;
use opentake_agent::mcp::media_bridge::{
    BridgeError, ImportOutcome, ImportSource, InspectMediaRequest, InspectMediaResult,
    InspectResult, InspectedFrame, InspectedMediaFrame, MediaBridge, SearchCandidate,
    SearchIndexState, SearchMediaResult, SearchSpokenHit, SearchVisualHit, TranscriptSource,
    TranscriptSourceResult, IMPORT_BYTES_DECODED_MAX,
};
use opentake_agent::mcp::motion::MotionBridge;
use opentake_agent::mcp::server;
use opentake_agent::plugin::registry::PluginRegistry;
use opentake_core::{
    importable_clip_type, AppCore, CoreError, DeferredCoreEvents, ProbedMedia,
    ProjectRuntimeSnapshot,
};
use opentake_domain::{ClipType, LutReference, MediaSource, TextStyle};
use opentake_media::{decode_frame_at, decode_frames_at, FrameRequest, MediaEngine, RgbaFrame};
use opentake_project::ProjectRoot;
use opentake_render::gpu::texture::upload_rgba;
use opentake_render::{
    even, try_build_render_plan, Compositor, CosmicTextRasterizer, DecodedFrame, GpuLutTexture,
    GpuTexture, RenderDevice, RenderSize, SourceMetrics, TextRasterRequest, TextRasterizer,
    TextureCache, TextureResolver, TextureSource,
};

use crate::library::ProjectMediaCapability;

/// JPEG quality `inspect_timeline` encodes composited frames at (upstream
/// `inspectTimelineJPEGQuality = 0.7`). `image` takes a 0–100 byte.
const INSPECT_JPEG_QUALITY: u8 = 70;
const INSPECT_MEDIA_FRAME_MAX_DIMENSION: u32 = 512;
const INSPECT_MEDIA_OVERVIEW_TILES: usize = 36;
const INSPECT_MEDIA_OVERVIEW_COLUMNS: u32 = 6;
const INSPECT_MEDIA_OVERVIEW_TILE: (u32, u32) = (192, 108);

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
    client: reqwest::Client,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl ReqwestUrlFetcher {
    fn new() -> Result<Self, BridgeError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(5 * 60))
            .build()
            .map_err(|error| {
                BridgeError::new(format!("Failed to initialize HTTPS client: {error}"))
            })?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map(Arc::new)
            .map_err(|_| BridgeError::new("Failed to initialize HTTPS runtime"))?;
        Ok(Self { client, runtime })
    }
}

impl UrlFetcher for ReqwestUrlFetcher {
    fn fetch(
        &self,
        url: &reqwest::Url,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<UrlFetchResponse, BridgeError> {
        let client = self.client.clone();
        let url = url.clone();
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

/// Spawn the MCP server. `core` is a clone that shares the live session;
/// `workflows_dir` is `<app_data_dir>/workflows`; `cache_root` / `models_dir` are
/// the same paths the UI's [`MediaEngine`] uses, so the bridge's imports land in
/// the same caches. A bind failure (port in use) is logged, not fatal — the app
/// keeps running without the agent network face.
pub fn spawn(
    core: AppCore,
    workflows_dir: PathBuf,
    cache_root: PathBuf,
    models_dir: PathBuf,
    generation_bridge: Arc<dyn GenerationBridge>,
    motion_bridge: Arc<dyn MotionBridge>,
) {
    let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(core.clone()));
    let bridge = build_media_bridge(core, cache_root, models_dir);
    let registry = Arc::new(RwLock::new(build_registry(&workflows_dir)));
    tauri::async_runtime::spawn(async move {
        let addr = match server::DEFAULT_ADDR.parse() {
            Ok(addr) => addr,
            Err(e) => {
                eprintln!("[mcp] invalid bind address {}: {e}", server::DEFAULT_ADDR);
                return;
            }
        };
        if let Err(e) = server::serve_with_capability_bridges(
            addr,
            handle,
            registry,
            Some(bridge),
            Some(generation_bridge),
            Some(motion_bridge),
        )
        .await
        {
            eprintln!("[mcp] server stopped: {e}");
        }
    });
}

pub(crate) fn build_media_bridge(
    core: AppCore,
    cache_root: PathBuf,
    models_dir: PathBuf,
) -> Arc<dyn MediaBridge> {
    Arc::new(TauriMediaBridge::new(core, cache_root, models_dir))
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
}

impl TauriMediaBridge {
    fn new(core: AppCore, cache_root: PathBuf, models_dir: PathBuf) -> Self {
        TauriMediaBridge {
            core,
            engine: MediaEngine::new(cache_root, models_dir),
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
            ImportSource::Path(path) => {
                self.import_from_path(&path, name.as_deref(), folder_id.as_deref())
            }
            ImportSource::Bytes { base64, mime_type } => {
                self.import_from_bytes(&base64, &mime_type, name.as_deref(), folder_id.as_deref())
            }
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

        let (extension, response_mime, expected_kind) =
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
        let warning = commit.warning.map_or(String::new(), |warning| {
            format!(" Warning: retained commit required recovery: {warning:?}.")
        });
        Ok(ImportOutcome {
            message: format!(
                "Imported '{}' (id: {}, type: {}, {} bytes, {}). Available now in get_media.{warning}",
                commit.entry.name,
                commit.entry.id,
                clip_type_name(commit.entry.kind),
                total,
                response_mime.unwrap_or_else(|| format!(".{extension}"))
            ),
        })
    }

    /// `path` import: in place, mirroring directories recursively — the exact
    /// `crate::media` path the media panel uses (`import_one` / `mirror_dir`), so
    /// posters/manifest/events stay consistent. 1:1 with upstream
    /// `ToolExecutor+Import.importFromPath`.
    fn import_from_path(
        &self,
        path: &str,
        name: Option<&str>,
        folder_id: Option<&str>,
    ) -> Result<ImportOutcome, BridgeError> {
        self.core
            .ensure_project_mutable()
            .map_err(|error| BridgeError::new(error.to_string()))?;
        let file_url = PathBuf::from(path);
        let meta = std::fs::metadata(&file_url).map_err(|_| {
            BridgeError::new(
                "MCP_SOURCE_PATH_UNREADABLE: source.path does not exist or is not readable",
            )
        })?;

        if meta.is_dir() {
            // Recursive directory import (剪注-style folder mirroring). Reuse the
            // media panel's `mirror_dir`; count what actually landed.
            let before_entries = self.core.media().entries.len();
            let before_folders = self.core.media().folders.len();
            let mut skipped = Vec::new();
            let parent = folder_id.map(|s| s.to_string());
            crate::media::mirror_dir(&self.core, &self.engine, &file_url, parent, &mut skipped)
                .map_err(|error| BridgeError::new(error.to_string()))?;
            let after = self.core.media();
            let asset_count = after.entries.len().saturating_sub(before_entries);
            let folder_count = after.folders.len().saturating_sub(before_folders);
            if asset_count == 0 {
                return Err(BridgeError::new(format!(
                    "No supported media found in folder: {path}"
                )));
            }
            let dir_name = file_url
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            return Ok(ImportOutcome {
                message: format!(
                    "Imported {asset_count} file(s) into {folder_count} folder(s) from '{dir_name}', mirroring its structure. Available now in get_media / list_folders."
                ),
            });
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
        let entry = crate::media::import_one(&self.core, &self.engine, &file_url)
            .map_err(|_| {
                BridgeError::new(
                    "MCP_SOURCE_IMPORT_FAILED: source.path could not be imported; verify the file type and permissions",
                )
            })?
            .ok_or_else(|| {
                BridgeError::new(
                    "MCP_SOURCE_IMPORT_FAILED: source.path could not be imported; verify the file type and permissions",
                )
            })?;
        let entry = self.apply_import_metadata(entry, name, folder_id)?;
        Ok(ImportOutcome {
            message: format!(
                "Imported '{}' (id: {}, type: {}) from path. Available now in get_media.",
                entry.name,
                entry.id,
                clip_type_name(entry.kind)
            ),
        })
    }

    /// `bytes` import: write the base64 payload into the project bundle's `media/`,
    /// then register it through the same import path. 1:1 with upstream
    /// `ToolExecutor+Import.importFromBytes`.
    fn import_from_bytes(
        &self,
        base64: &str,
        mime_type: &str,
        name: Option<&str>,
        folder_id: Option<&str>,
    ) -> Result<ImportOutcome, BridgeError> {
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

        let project_dir = self
            .core
            .project_dir()
            .ok_or_else(|| BridgeError::new("No project is open; cannot import bytes"))?;
        let media_dir = project_dir.join("media");
        std::fs::create_dir_all(&media_dir)
            .map_err(|e| BridgeError::new(format!("Failed to prepare media directory: {e}")))?;

        let filename = format!("imported-{}.{file_ext}", short_uuid());
        let dest = media_dir.join(filename);
        std::fs::write(&dest, &data)
            .map_err(|e| BridgeError::new(format!("Failed to write bytes to disk: {e}")))?;

        let entry = match crate::media::import_one(&self.core, &self.engine, &dest) {
            Ok(Some(entry)) => entry,
            Ok(None) => {
                let _ = std::fs::remove_file(&dest);
                return Err(BridgeError::new("Failed to register imported asset"));
            }
            Err(error) => {
                let _ = std::fs::remove_file(&dest);
                return Err(BridgeError::new(error.to_string()));
            }
        };
        let entry = self.apply_import_metadata(entry, name, folder_id)?;
        Ok(ImportOutcome {
            message: format!(
                "Imported '{}' (id: {}, type: {}, {} bytes). Available now in get_media.",
                entry.name,
                entry.id,
                clip_type_name(entry.kind),
                data.len()
            ),
        })
    }

    /// Apply the optional display name + folder placement to a freshly imported
    /// asset (upstream `applyImportMetadata`): rename via `RenameMedia`, place via
    /// `MoveToFolder`. Returns the (possibly renamed) entry for the confirmation.
    fn apply_import_metadata(
        &self,
        mut entry: opentake_domain::MediaManifestEntry,
        name: Option<&str>,
        folder_id: Option<&str>,
    ) -> Result<opentake_domain::MediaManifestEntry, BridgeError> {
        if let Some(name) = name {
            self.core
                .apply(opentake_core::EditCommand::RenameMedia {
                    entries: vec![opentake_ops::RenameEntry {
                        id: entry.id.clone(),
                        name: name.to_string(),
                    }],
                })
                .map_err(|error| BridgeError::new(error.to_string()))?;
            entry.name = name.to_string();
        }
        if let Some(folder_id) = folder_id {
            self.core
                .apply(opentake_core::EditCommand::MoveToFolder {
                    asset_ids: vec![entry.id.clone()],
                    folder_id: Some(folder_id.to_string()),
                })
                .map_err(|error| BridgeError::new(error.to_string()))?;
        }
        Ok(entry)
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
    if entry.kind == ClipType::Lottie {
        return Err(BridgeError::unavailable(
            "inspect_media: Lottie rendering is not available in this build",
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

    if entry.kind == ClipType::Image {
        let frame =
            opentake_media::thumbnail::image_thumbnail(&path, INSPECT_MEDIA_FRAME_MAX_DIMENSION)
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
    let metrics = ManifestMetrics { sizes };
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
}

impl SourceMetrics for ManifestMetrics {
    fn natural_size(&self, media_ref: &str) -> Option<(u32, u32)> {
        self.sizes.get(media_ref).copied()
    }
}

/// `TextureResolver` that decodes a layer's pixels on demand via ffmpeg and
/// uploads them to the GPU. Mirrors the preview / export resolvers; the decode box
/// is the downscaled inspect render size. Lottie is skipped (returns `None`).
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
        let (media_ref, key, is_image) = match source {
            TextureSource::Decoded { media_ref } => {
                (media_ref, format!("v:{media_ref}:{source_frame}"), false)
            }
            TextureSource::Image { media_ref } => (media_ref, format!("i:{media_ref}"), true),
            TextureSource::Text { clip_id } => return self.resolve_text(clip_id),
            TextureSource::Lottie { .. } => return None,
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
                    || err.message.contains("invalid"),
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
    fn reqwest_fetch_is_cancellable_and_redacts_signed_url_errors() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            let _ = release_rx.recv_timeout(Duration::from_secs(2));
        });
        let cancel = opentake_media::MediaCancelToken::new();
        let trigger = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            trigger.cancel();
        });
        let fetcher = ReqwestUrlFetcher::new().unwrap();
        let signed =
            reqwest::Url::parse(&format!("http://{addr}/media.mp4?token=super-secret-query"))
                .unwrap();
        let started = std::time::Instant::now();
        let cancelled = fetcher
            .fetch(&signed, &cancel)
            .err()
            .expect("in-flight request must observe cancellation");
        assert!(
            cancelled.message.contains("cancelled"),
            "{}",
            cancelled.message
        );
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "cancellation waited for the network timeout"
        );
        assert!(!cancelled.message.contains("super-secret-query"));
        let _ = release_tx.send(());
        server.join().unwrap();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            drop(stream);
        });
        let signed =
            reqwest::Url::parse(&format!("http://{addr}/media.mp4?token=another-secret")).unwrap();
        let error = fetcher
            .fetch(&signed, &opentake_media::MediaCancelToken::new())
            .err()
            .expect("closed connection fails");
        assert!(
            !error.message.contains("another-secret"),
            "{}",
            error.message
        );
        assert!(
            !error.message.contains(signed.as_str()),
            "{}",
            error.message
        );
        server.join().unwrap();
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
        // A single-file path import references the file in place (no saved bundle
        // needed), through the same `import_one` the media panel uses.
        let bridge = TauriMediaBridge::new(
            AppCore::new(),
            tmp.path().join("cache"),
            tmp.path().join("models"),
        );
        let out = bridge
            .import_from_path(&video.to_string_lossy(), None, None)
            .expect("single-file path import");
        assert!(out.message.contains("from path"), "{}", out.message);
        assert!(out.message.contains("type: video"), "{}", out.message);
        // The asset is now in the shared core's manifest, named by its stem.
        let manifest = bridge.core.media();
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(manifest.entries[0].name, "My Clip");
        assert_eq!(manifest.entries[0].kind, ClipType::Video);
    }

    #[test]
    fn import_from_path_missing_file_errors() {
        let bridge = TauriMediaBridge::new(
            AppCore::new(),
            std::env::temp_dir().join("cache"),
            std::env::temp_dir().join("models"),
        );
        let err = bridge
            .import_from_path("/no/such/file.mp4", None, None)
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
        let bridge = TauriMediaBridge::new(
            AppCore::new(),
            tmp.path().join("cache"),
            tmp.path().join("models"),
        );
        let err = bridge
            .import_from_path(&doc.to_string_lossy(), None, None)
            .unwrap_err();
        assert!(
            err.message.contains("Unsupported file extension"),
            "{}",
            err.message
        );
    }
}
