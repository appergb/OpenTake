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
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, RwLock};

use base64::Engine as _;

use opentake_agent::mcp::core_handle::{AppCoreHandle, CoreHandle};
use opentake_agent::mcp::media_bridge::{
    BridgeError, ImportOutcome, ImportSource, InspectResult, InspectedFrame, MediaBridge,
    SearchCandidate, SearchIndexState, SearchMediaResult, SearchSpokenHit, SearchVisualHit,
    TranscriptSource, TranscriptSourceResult, IMPORT_BYTES_DECODED_MAX,
};
use opentake_agent::mcp::server;
use opentake_agent::plugin::registry::PluginRegistry;
use opentake_core::{importable_clip_type, AppCore};
use opentake_domain::{ClipType, MediaSource, TextStyle};
use opentake_media::{decode_frame_at, FrameRequest, MediaEngine};
use opentake_render::gpu::texture::upload_rgba;
use opentake_render::{
    build_render_plan, even, Compositor, CosmicTextRasterizer, DecodedFrame, GpuTexture,
    RenderDevice, RenderSize, SourceMetrics, TextRasterRequest, TextRasterizer, TextureCache,
    TextureResolver, TextureSource,
};

/// JPEG quality `inspect_timeline` encodes composited frames at (upstream
/// `inspectTimelineJPEGQuality = 0.7`). `image` takes a 0–100 byte.
const INSPECT_JPEG_QUALITY: u8 = 70;

/// Per-frame texture cache size — bounds VRAM during a multi-frame inspect.
const TEXTURE_CACHE_CAP: usize = 64;

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
pub fn spawn(core: AppCore, workflows_dir: PathBuf, cache_root: PathBuf, models_dir: PathBuf) {
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
        if let Err(e) = server::serve_with_bridge(addr, handle, registry, Some(bridge)).await {
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

impl MediaBridge for TauriMediaBridge {
    fn inspect_timeline(
        &self,
        frames: &[i32],
        max_longest_edge: u32,
    ) -> Result<InspectResult, BridgeError> {
        // Snapshot the live session, then composite off the session lock (the
        // preview path's discipline; a local GPU context per call keeps this off
        // the preview's cached `RenderState` mutex, matching export.rs).
        let timeline = self.core.get_timeline().timeline;
        let manifest = self.core.media();
        let project_dir = self.core.project_dir();
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
        for src in sources {
            let skip = |reason: String| TranscriptSourceResult {
                media_ref: src.media_ref.clone(),
                transcript: None,
                error: Some(reason),
            };
            // Resolve the asset path; a missing/offline source is skipped.
            let path = match crate::transcribe::resolve_asset(&self.core, &src.media_ref) {
                Ok((path, _is_video)) => path,
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
                        .transcript(&path, src.is_video, None, b)
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
        match source {
            ImportSource::Path(path) => {
                self.import_from_path(&path, name.as_deref(), folder_id.as_deref())
            }
            ImportSource::Bytes { base64, mime_type } => {
                self.import_from_bytes(&base64, &mime_type, name.as_deref(), folder_id.as_deref())
            }
            // HTTPS download is not wired in this build: doing a byte-capped 1 GB
            // streaming download from this synchronous bridge would require adding a
            // full async HTTP stack (reqwest + a tokio runtime) to `src-tauri` — the
            // one client in the workspace (`opentake-gen`) is a typed provider client,
            // not a general file downloader, and is async. Path + bytes are fully
            // functional; url returns a clear, structured error (mirrors upstream's
            // validation-error shape) so the agent knows to fall back to path/bytes.
            ImportSource::Url { .. } => Err(BridgeError::new(
                "source.url import is not yet supported in this build — download the file and pass source.path (a local path), or inline it as source.bytes with source.mimeType.",
            )),
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
        let manifest = self.core.media();
        let project_dir = self.core.project_dir();
        let resolver = opentake_domain::MediaResolver::new(&manifest, project_dir.as_deref());
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

        let fps = self.core.get_timeline().timeline.fps;
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
        let meta = std::fs::metadata(&file_url)
            .map_err(|_| BridgeError::new(format!("File not found: {path}")))?;

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
            .map_err(|error| BridgeError::new(error.to_string()))?
            .ok_or_else(|| BridgeError::new(format!("Failed to import file: {path}")))?;
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
    let plan = build_render_plan(timeline, render_size, &metrics);

    let dev =
        RenderDevice::try_new().map_err(|e| BridgeError::new(format!("no GPU device: {e}")))?;
    let compositor = Compositor::new(&dev.device);
    let text_rasterizer = CosmicTextRasterizer::new();
    if !text_rasterizer.has_fonts() {
        eprintln!("[render] no system fonts discovered; text clips will render blank");
    }

    let mut out_frames: Vec<InspectedFrame> = Vec::with_capacity(frames.len());
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
}

/// Project the timeline's text clips (content + style + box) into the per-clip
/// lookup the resolver rasterizes from. Keyed by clip id.
fn project_text(timeline: &opentake_domain::Timeline) -> HashMap<String, TextInfo> {
    let mut text: HashMap<String, TextInfo> = HashMap::new();
    for track in &timeline.tracks {
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
    fn url_import_returns_structured_not_supported_error() {
        // The bridge fully wires path + bytes; url reports a clear fallback message
        // (path/bytes) rather than silently failing. Constructed without a GPU/core
        // so this stays a pure unit assertion on the url arm.
        let bridge = TauriMediaBridge::new(
            AppCore::new(),
            std::env::temp_dir().join("inspect-cache"),
            std::env::temp_dir().join("inspect-models"),
        );
        let err = bridge
            .import_media(
                ImportSource::Url {
                    url: "https://example.com/a.mp4".into(),
                    mime_type: None,
                },
                None,
                None,
            )
            .unwrap_err();
        assert!(err.message.contains("not yet supported"), "{}", err.message);
        assert!(err.message.contains("source.path"), "{}", err.message);
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
        assert!(err.message.contains("File not found"), "{}", err.message);
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
