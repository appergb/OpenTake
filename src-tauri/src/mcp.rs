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
fn build_registry(workflows_dir: &Path) -> PluginRegistry {
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
    let bridge: Arc<dyn MediaBridge> =
        Arc::new(TauriMediaBridge::new(core, cache_root, models_dir));
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
            crate::media::mirror_dir(&self.core, &self.engine, &file_url, parent, &mut skipped);
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
            .ok_or_else(|| BridgeError::new(format!("Failed to import file: {path}")))?;
        let entry = self.apply_import_metadata(entry, name, folder_id);
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

        let Some(entry) = crate::media::import_one(&self.core, &self.engine, &dest) else {
            let _ = std::fs::remove_file(&dest);
            return Err(BridgeError::new("Failed to register imported asset"));
        };
        let entry = self.apply_import_metadata(entry, name, folder_id);
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
    ) -> opentake_domain::MediaManifestEntry {
        if let Some(name) = name {
            if self
                .core
                .apply(opentake_core::EditCommand::RenameMedia {
                    entries: vec![opentake_ops::RenameEntry {
                        id: entry.id.clone(),
                        name: name.to_string(),
                    }],
                })
                .is_ok()
            {
                entry.name = name.to_string();
            }
        }
        if let Some(folder_id) = folder_id {
            let _ = self.core.apply(opentake_core::EditCommand::MoveToFolder {
                asset_ids: vec![entry.id.clone()],
                folder_id: Some(folder_id.to_string()),
            });
        }
        entry
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
