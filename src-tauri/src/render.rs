//! Timeline composite-frame rendering for the preview (#47-A).
//!
//! Wires the ready-made wgpu compositor (`opentake-render`) to the live editing
//! session: build a `RenderPlan` from the current `Timeline`, evaluate one frame
//! into an ordered draw list, resolve each layer's pixels through ffmpeg decode
//! (`opentake-media`), composite on the GPU, read back, and return the frame as a
//! base64 PNG data URL the WebView paints onto a `<canvas>` (replacing the black
//! placeholder shown on the Timeline tab).
//!
//! Scope: **video + image + text + Lottie** layers. Text clips rasterize through
//! `CosmicTextRasterizer` (cosmic-text glyph layout + swash raster) to a
//! premultiplied-RGBA box texture composited last, like upstream's `CATextLayer`
//! (#65). Lottie JSON is parsed by Velato and rasterized by Vello into the same
//! device-local premultiplied-RGBA texture contract used by playback/export.
//!
//! The GPU device + compositor are acquired once and cached in Tauri managed
//! state ([`RenderState`]); only the per-frame texture cache is short-lived. A
//! single `Mutex` serializes composites, which is what we want for the preview
//! (one frame at a time, no GPU contention). The continuous playback engine
//! (#53) will move this onto a dedicated render thread.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use opentake_core::{AppCore, EditCommand, ProjectRevision};
use opentake_domain::{ClipType, LutReference, MediaSource, TextStyle, Timeline};
use opentake_media::{
    decode_frame_at_cancellable, decode_frame_file_at_cancellable, interpolate_frame_pair,
    FrameInterpolationFallback, FrameInterpolationMode, FrameRequest, MediaCancelToken,
};
use opentake_ops::command::RenameEntry;
use opentake_project::ProjectRoot;
use opentake_render::gpu::compositor::{
    TextureInterpolationConfig, TextureInterpolationFallback, TextureInterpolationMode,
    TextureResolveRequest,
};
use opentake_render::gpu::texture::upload_rgba;
use opentake_render::wgpu;
use opentake_render::{
    even, try_build_render_plan, Compositor, CosmicTextRasterizer, DecodedFrame, FramePlan,
    GpuLutTexture, GpuTexture, LayerDraw, RenderDevice, RenderPlan, RenderSize, SourceMetrics,
    TextRasterRequest, TextRasterizer, TextureCache, TextureResolver, TextureSource,
};

/// Cap (longest canvas side, px) for a composite when the caller passes no
/// `max_size`. Keeps the PNG payload small for interactive scrubbing while still
/// looking crisp in the preview pane.
const DEFAULT_PREVIEW_CAP: u32 = 1280;

/// Agent result images stay below both the chat display dimension and its
/// 1 MiB base64 payload ceiling (768 KiB raw expands to exactly 1 MiB).
pub(crate) const AGENT_TIMELINE_RESULT_MAX_DIMENSION: u32 = 640;
pub(crate) const AGENT_TIMELINE_RESULT_PNG_BYTES_MAX: usize = 768 * 1024;
const EMPTY_TIMELINE_BACKGROUND_RGBA: [u8; 4] = [22, 24, 29, 255];
const EMPTY_TIMELINE_MARKER_RGBA: [u8; 4] = [174, 181, 195, 255];

static ROOT_TIMELINE_PLAYHEAD: OnceLock<Mutex<Option<(u64, i32)>>> = OnceLock::new();

fn record_root_timeline_playhead(project_epoch: u64, frame: i32) {
    *ROOT_TIMELINE_PLAYHEAD
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((project_epoch, frame));
}

pub(crate) fn root_timeline_playhead(project_epoch: u64) -> i32 {
    let guard = ROOT_TIMELINE_PLAYHEAD
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard
        .as_ref()
        .filter(|(epoch, _)| *epoch == project_epoch)
        .map_or(0, |(_, frame)| *frame)
}

/// Explicit project-owned inputs for the semantic empty-timeline frame. The
/// renderer rejects values that disagree with the committed timeline snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EmptyTimelineCanvasInput {
    pub project_width: i32,
    pub project_height: i32,
    pub fps: i32,
    pub playhead_frame: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TimelineResultPng {
    pub bytes: Vec<u8>,
    pub media_type: &'static str,
    pub width: u32,
    pub height: u32,
    pub playhead_frame: i32,
    pub timecode: String,
    pub empty_canvas: bool,
}

/// Per-frame texture cache size. Bounds VRAM during scrubbing; video frames are
/// keyed per source-frame so adjacent scrub positions reuse nothing, but a small
/// cache still helps repeated seeks to the same frame.
const TEXTURE_CACHE_CAP: usize = 64;

/// The composited frame handed back to the WebView.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompositeFrameDto {
    /// Composite width in pixels (after preview downscale).
    pub width: u32,
    /// Composite height in pixels.
    pub height: u32,
    /// `data:image/png;base64,...` — assignable directly to an `<img>`/canvas.
    pub data_url: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompositeFrameRequest {
    pub frame: i32,
    pub project_epoch: u64,
    pub timeline_version: u64,
    pub session_id: String,
    pub session_generation: u64,
    pub seek_generation: u64,
    #[serde(default)]
    pub sequence_id: Option<String>,
    #[serde(default)]
    pub source_media_id: Option<String>,
}

/// Lazily-acquired GPU device + compositor, cached across composite calls.
struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    compositor: Compositor,
    /// Text rasterizer (system fonts discovered once on first composite).
    text_rasterizer: CosmicTextRasterizer,
    /// Vector pipelines are discarded when the GPU context is rebuilt, so they
    /// never cross a device-loss boundary. Rc texture caches remain local to a
    /// composite call because Tauri managed state must be Send + Sync.
    lottie: LottieMaterializer,
}

/// Tauri managed state holding the (lazily created) GPU context. `None` until the
/// first composite; an acquisition failure (no adapter / headless) surfaces to
/// the caller as a command error rather than panicking.
pub struct RenderState {
    ctx: Mutex<Option<GpuContext>>,
    preview: PreviewCompositeCoordinator,
}

impl RenderState {
    /// An empty render state (GPU acquired on first `composite_frame`).
    pub fn new() -> Self {
        Self {
            ctx: Mutex::new(None),
            preview: PreviewCompositeCoordinator::default(),
        }
    }
}

impl Default for RenderState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
struct ActivePreviewComposite {
    seek_generation: u64,
    cancel: MediaCancelToken,
}

#[derive(Default)]
struct PreviewCompositeState {
    revision: Option<ProjectRevision>,
    session_id: String,
    session_generation: u64,
    minimum_seek_generation: u64,
    active: Option<ActivePreviewComposite>,
}

#[derive(Default)]
struct PreviewCompositeCoordinator(Mutex<PreviewCompositeState>);

impl PreviewCompositeCoordinator {
    fn select_session(
        state: &mut PreviewCompositeState,
        revision: ProjectRevision,
        session_id: &str,
        session_generation: u64,
    ) -> Result<(), String> {
        let same_revision = state.revision == Some(revision);
        if same_revision && session_generation < state.session_generation {
            return Err("preview composite session was superseded".to_string());
        }
        if same_revision
            && session_generation == state.session_generation
            && !state.session_id.is_empty()
            && state.session_id != session_id
        {
            return Err("preview composite session identity mismatch".to_string());
        }
        if !same_revision || session_generation > state.session_generation {
            if let Some(active) = state.active.take() {
                active.cancel.cancel();
            }
            state.revision = Some(revision);
            state.session_id = session_id.to_string();
            state.session_generation = session_generation;
            state.minimum_seek_generation = 0;
        }
        Ok(())
    }

    fn begin(
        &self,
        revision: ProjectRevision,
        session_id: &str,
        session_generation: u64,
        seek_generation: u64,
    ) -> Result<MediaCancelToken, String> {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Self::select_session(&mut state, revision, session_id, session_generation)?;
        if seek_generation < state.minimum_seek_generation {
            return Err("preview composite seek was superseded".to_string());
        }
        if let Some(active) = state.active.take() {
            active.cancel.cancel();
        }
        let cancel = MediaCancelToken::new();
        state.active = Some(ActivePreviewComposite {
            seek_generation,
            cancel: cancel.clone(),
        });
        Ok(cancel)
    }

    fn cancel_before(
        &self,
        revision: ProjectRevision,
        session_id: &str,
        session_generation: u64,
        minimum_seek_generation: u64,
    ) {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.revision.is_none() {
            state.revision = Some(revision);
            state.session_id = session_id.to_string();
            state.session_generation = session_generation;
        } else if state.revision != Some(revision)
            || state.session_id != session_id
            || state.session_generation != session_generation
        {
            // A late cancellation from an old project/session must never cancel
            // the active successor. Its exact matching work, if any, was
            // already cancelled when the successor session began.
            return;
        }
        state.minimum_seek_generation = state.minimum_seek_generation.max(minimum_seek_generation);
        if state
            .active
            .as_ref()
            .is_some_and(|active| active.seek_generation < state.minimum_seek_generation)
        {
            if let Some(active) = state.active.take() {
                active.cancel.cancel();
            }
        }
    }

    fn is_current(
        &self,
        revision: ProjectRevision,
        session_id: &str,
        session_generation: u64,
        seek_generation: u64,
    ) -> bool {
        let state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.revision == Some(revision)
            && state.session_id == session_id
            && state.session_generation == session_generation
            && state.minimum_seek_generation <= seek_generation
            && state.active.as_ref().is_some_and(|active| {
                active.seek_generation == seek_generation && !active.cancel.is_cancelled()
            })
    }
}

/// Resolvable info for one media asset, projected from the manifest.
struct MediaInfo<'a> {
    path: PathBuf,
    retained: Option<&'a File>,
    source_fps: Option<f64>,
}

/// Retained, pre-authorized media inputs for strict project-cover capture.
/// Missing entries fail closed; the renderer never reopens their manifest paths.
pub(crate) struct CompositeSourceAuthority {
    files: HashMap<String, File>,
}

impl CompositeSourceAuthority {
    pub(crate) fn new(files: HashMap<String, File>) -> Self {
        Self { files }
    }
}

/// A text clip projected from the timeline, keyed by clip id. The box's width /
/// height drive the rasterized texture size; position is carried by the layer
/// affine (so x/y are kept only for completeness).
struct TextInfo {
    content: String,
    style: TextStyle,
    box_norm: (f64, f64, f64, f64),
}

fn text_style_is_finite(style: &TextStyle) -> bool {
    let color_is_finite = |color: opentake_domain::Rgba| {
        [color.r, color.g, color.b, color.a]
            .into_iter()
            .all(f64::is_finite)
    };
    style.font_size.is_finite()
        && style.font_size > 0.0
        && style.font_scale.is_finite()
        && style.font_scale > 0.0
        && color_is_finite(style.color)
        && color_is_finite(style.shadow.color)
        && style.shadow.offset_x.is_finite()
        && style.shadow.offset_y.is_finite()
        && style.shadow.blur.is_finite()
        && style.shadow.blur >= 0.0
        && color_is_finite(style.background.color)
        && color_is_finite(style.border.color)
}

/// `SourceMetrics` backed by the media manifest: only intrinsic size is known
/// here (orientation/alpha use the documented identity/false defaults; ffmpeg
/// auto-rotates on decode in this first cut).
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

const MAX_LOTTIE_BYTES: usize = 8 * 1024 * 1024;
const MAX_LOTTIE_DIMENSION: usize = 4096;

struct CachedLottie {
    content_hash: String,
    composition: velato::Composition,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LottieMetadata {
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    pub frame_count: i64,
    pub duration_seconds: f64,
}

/// Device-lifetime Lottie JSON parser/rasterizer shared by preview, playback,
/// and export. GPU textures live in the caller's [`TextureCache`]; parsed
/// documents live here and are replaced whenever the source content hash
/// changes. Dropping the owning GPU context drops both halves, so a recovered
/// device never receives a texture or Vello pipeline created by the old device.
pub(crate) struct LottieMaterializer {
    documents: HashMap<PathBuf, CachedLottie>,
    scene_renderer: velato::Renderer,
    gpu_renderer: Option<velato::vello::Renderer>,
}

impl LottieMaterializer {
    pub(crate) fn new() -> Self {
        Self {
            documents: HashMap::new(),
            scene_renderer: velato::Renderer::new(),
            gpu_renderer: None,
        }
    }

    fn ensure_document(&mut self, path: &std::path::Path) -> Result<(), String> {
        let bytes = std::fs::read(path)
            .map_err(|error| format!("read Lottie document {}: {error}", path.display()))?;
        self.ensure_document_bytes(path, &bytes)
    }

    fn ensure_document_file(&mut self, path: &std::path::Path, file: &File) -> Result<(), String> {
        let mut input = file
            .try_clone()
            .map_err(|error| format!("clone retained Lottie document: {error}"))?;
        input
            .seek(SeekFrom::Start(0))
            .map_err(|error| format!("rewind retained Lottie document: {error}"))?;
        let mut bytes = Vec::new();
        input
            .take((MAX_LOTTIE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("read retained Lottie document: {error}"))?;
        self.ensure_document_bytes(path, &bytes)
    }

    fn ensure_document_bytes(
        &mut self,
        path: &std::path::Path,
        bytes: &[u8],
    ) -> Result<(), String> {
        if bytes.is_empty() || bytes.len() > MAX_LOTTIE_BYTES {
            return Err(format!(
                "Lottie document {} must be 1..={MAX_LOTTIE_BYTES} bytes (got {})",
                path.display(),
                bytes.len()
            ));
        }
        let content_hash = format!("{:x}", Sha256::digest(bytes));
        let needs_parse = self
            .documents
            .get(path)
            .is_none_or(|cached| cached.content_hash != content_hash);
        if needs_parse {
            let composition = std::panic::catch_unwind(|| velato::Composition::from_slice(bytes))
                .map_err(|_| {
                    format!(
                        "Lottie document {} uses an unsupported or malformed feature",
                        path.display()
                    )
                })?
                .map_err(|error| format!("parse Lottie document {}: {error}", path.display()))?;
            validate_lottie(&composition, path)?;
            self.documents.insert(
                path.to_path_buf(),
                CachedLottie {
                    content_hash,
                    composition,
                },
            );
        }
        Ok(())
    }

    pub(crate) fn metadata(&mut self, path: &std::path::Path) -> Result<LottieMetadata, String> {
        self.ensure_document(path)?;
        let composition = &self
            .documents
            .get(path)
            .expect("document inserted or already cached")
            .composition;
        let frame_count = lottie_frame_count(composition);
        Ok(LottieMetadata {
            width: composition.width as u32,
            height: composition.height as u32,
            frame_rate: composition.frame_rate,
            frame_count,
            duration_seconds: frame_count as f64 / composition.frame_rate,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        textures: &mut TextureCache,
        path: &std::path::Path,
        source_frame: i64,
        render_box: (u32, u32),
        label: &str,
    ) -> Result<Rc<GpuTexture>, String> {
        self.ensure_document(path)?;
        self.resolve_loaded(
            device,
            queue,
            textures,
            path,
            source_frame,
            render_box,
            label,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_file(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        textures: &mut TextureCache,
        path: &std::path::Path,
        file: &File,
        source_frame: i64,
        render_box: (u32, u32),
        label: &str,
    ) -> Result<Rc<GpuTexture>, String> {
        self.ensure_document_file(path, file)?;
        self.resolve_loaded(
            device,
            queue,
            textures,
            path,
            source_frame,
            render_box,
            label,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_loaded(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        textures: &mut TextureCache,
        path: &std::path::Path,
        source_frame: i64,
        render_box: (u32, u32),
        label: &str,
    ) -> Result<Rc<GpuTexture>, String> {
        let cached = self
            .documents
            .get(path)
            .expect("document inserted or already cached");
        let content_hash = cached.content_hash.clone();
        let composition = &cached.composition;
        let frame_count = lottie_frame_count(composition);
        let internal_frame = source_frame.rem_euclid(frame_count);
        let (width, height) = lottie_texture_size(composition, render_box);
        let key = format!("l:{content_hash}:{internal_frame}:{width}x{height}");
        if let Some(texture) = textures.get(&key) {
            return Ok(texture);
        }

        let transform = velato::vello::kurbo::Affine::scale_non_uniform(
            width as f64 / composition.width as f64,
            height as f64 / composition.height as f64,
        );
        let frame = composition.frames.start + internal_frame as f64;
        let scene = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.scene_renderer
                .render(composition, frame, transform, 1.0)
        }))
        .map_err(|_| {
            format!(
                "render Lottie document {} failed on an unsupported feature",
                path.display()
            )
        })?;
        if self.gpu_renderer.is_none() {
            self.gpu_renderer = Some(
                velato::vello::Renderer::new(
                    device,
                    velato::vello::RendererOptions {
                        surface_format: None,
                        use_cpu: false,
                        antialiasing_support: velato::vello::AaSupport::area_only(),
                        num_init_threads: NonZeroUsize::new(1),
                    },
                )
                .map_err(|error| format!("initialize Lottie GPU renderer: {error}"))?,
            );
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.gpu_renderer
            .as_mut()
            .expect("renderer initialized above")
            .render_to_texture(
                device,
                queue,
                &scene,
                &view,
                &velato::vello::RenderParams {
                    base_color: velato::vello::peniko::color::palette::css::TRANSPARENT,
                    width,
                    height,
                    antialiasing_method: velato::vello::AaConfig::Area,
                },
            )
            .map_err(|error| format!("render Lottie frame {internal_frame}: {error}"))?;

        Ok(textures.insert(
            key,
            GpuTexture {
                texture,
                view,
                width,
                height,
            },
        ))
    }
}

impl Default for LottieMaterializer {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_lottie(
    composition: &velato::Composition,
    path: &std::path::Path,
) -> Result<(), String> {
    if composition.width == 0
        || composition.height == 0
        || composition.width > MAX_LOTTIE_DIMENSION
        || composition.height > MAX_LOTTIE_DIMENSION
    {
        return Err(format!(
            "Lottie document {} canvas must be within 1..={MAX_LOTTIE_DIMENSION} (got {}x{})",
            path.display(),
            composition.width,
            composition.height
        ));
    }
    if !composition.frames.start.is_finite()
        || !composition.frames.end.is_finite()
        || composition.frames.end <= composition.frames.start
        || !composition.frame_rate.is_finite()
        || composition.frame_rate <= 0.0
    {
        return Err(format!(
            "Lottie document {} has an invalid frame range or frame rate",
            path.display()
        ));
    }
    Ok(())
}

fn lottie_frame_count(composition: &velato::Composition) -> i64 {
    (composition.frames.end - composition.frames.start)
        .ceil()
        .clamp(1.0, i64::MAX as f64) as i64
}

fn lottie_texture_size(composition: &velato::Composition, render_box: (u32, u32)) -> (u32, u32) {
    let max_width = render_box.0.max(1) as f64;
    let max_height = render_box.1.max(1) as f64;
    let scale = (max_width / composition.width as f64)
        .min(max_height / composition.height as f64)
        .min(1.0);
    (
        (composition.width as f64 * scale).round().max(1.0) as u32,
        (composition.height as f64 * scale).round().max(1.0) as u32,
    )
}

/// `TextureResolver` that decodes a layer's pixels on demand via ffmpeg and
/// uploads them to the GPU (with a small LRU cache). Image and Lottie cache keys
/// include source content hashes, so edited files cannot reuse stale textures.
struct MediaResolver<'d> {
    device: &'d wgpu::Device,
    queue: &'d wgpu::Queue,
    cache: &'d mut TextureCache,
    lottie: &'d mut LottieMaterializer,
    media: &'d HashMap<String, MediaInfo<'d>>,
    timeline_fps: i32,
    /// Text clips by id (content + style + box) for on-demand rasterization.
    text: &'d HashMap<String, TextInfo>,
    /// cosmic-text rasterizer (system fonts) for text layers.
    text_rasterizer: &'d CosmicTextRasterizer,
    /// Downscale box for decoded source frames (matches the preview render size).
    preview_box: (u32, u32),
    cancel: &'d MediaCancelToken,
    project_root: Option<&'d ProjectRoot>,
    lut_cache: &'d mut HashMap<String, Rc<GpuLutTexture>>,
    materialization_error: Option<String>,
    strict_materialization: bool,
}

impl MediaResolver<'_> {
    fn fail_materialization<T>(&mut self, message: impl Into<String>) -> Option<T> {
        // A missing or corrupt source is never a valid preview frame. The
        // strict flag only controls retained-handle authority and the
        // interpolation tail fallback; it must not turn an unavailable layer
        // into a successful black composite.
        if self.materialization_error.is_none() {
            self.materialization_error = Some(message.into());
        }
        None
    }

    /// Rasterize a text clip's box to a premultiplied-RGBA texture (composited
    /// last, like upstream's `CATextLayer`). The box texture is uploaded with
    /// `srgb = false` so it blends in the same encoded space as video/image, and
    /// the plan marks text `needs_premultiply = false` so the shader treats it as
    /// already premultiplied (which it is).
    fn resolve_text(&mut self, clip_id: &str) -> Option<Rc<GpuTexture>> {
        let key = format!("t:{clip_id}");
        if let Some(tex) = self.cache.get(&key) {
            return Some(tex);
        }
        let Some(info) = self.text.get(clip_id) else {
            return self.fail_materialization(format!("text clip {clip_id} has no raster input"));
        };
        if !text_style_is_finite(&info.style) {
            return self.fail_materialization(format!("text clip {clip_id} has invalid style"));
        }
        let req = TextRasterRequest {
            clip_id,
            content: &info.content,
            style: &info.style,
            box_norm: info.box_norm,
            canvas: self.preview_box,
        };
        let frame = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.text_rasterizer.rasterize(&req)
        })) {
            Ok(Some(frame)) => frame,
            Ok(None) => {
                return self
                    .fail_materialization(format!("text clip {clip_id} rasterization failed"));
            }
            Err(_) => {
                return self
                    .fail_materialization(format!("text clip {clip_id} rasterization panicked"));
            }
        };
        let tex = upload_rgba(self.device, self.queue, &frame, false, Some("preview-text"));
        Some(self.cache.insert(key, tex))
    }

    fn resolve_interpolated_video(
        &mut self,
        media_ref: &str,
        source_frame: i64,
        interpolation: TextureInterpolationConfig,
    ) -> Option<Rc<GpuTexture>> {
        let key = format!(
            "vf:{media_ref}:{source_frame}:{:?}:{:.6}:{:.6}",
            interpolation.mode, interpolation.source_fps, interpolation.target_fps
        );
        if let Some(tex) = self.cache.get(&key) {
            return Some(tex);
        }
        let Some(info) = self.media.get(media_ref) else {
            return self.fail_materialization(format!("video source {media_ref} is unauthorized"));
        };
        let source_fps = info.source_fps.unwrap_or(interpolation.source_fps);
        if !source_fps.is_finite() || source_fps <= 0.0 {
            return self
                .fail_materialization(format!("video source {media_ref} has invalid frame rate"));
        }
        let timestamp = source_frame.max(0) as f64 / interpolation.target_fps;
        let source_position = timestamp * source_fps;
        let first_index = source_position.floor().max(0.0) as i64;
        let next_index = source_position.ceil().max(0.0) as i64;
        let alpha = source_position - first_index as f64;
        let decode = |index: i64| {
            let request = FrameRequest {
                time_secs: index as f64 / source_fps,
                max_size: self.preview_box,
                tolerance_secs: 0.0,
                apply_rotation: true,
            };
            match info.retained {
                Some(file) => decode_frame_file_at_cancellable(file, &request, self.cancel),
                None => decode_frame_at_cancellable(&info.path, &request, self.cancel),
            }
            .ok()
            .map(|(_, frame)| frame)
        };
        let Some(first) = decode(first_index) else {
            return self.fail_materialization(format!("video source {media_ref} decode failed"));
        };
        let last = if next_index == first_index {
            first.clone()
        } else {
            // A half-open media duration may not expose the mathematical next
            // frame at the tail. Hold the last decodable endpoint instead of
            // dropping the whole layer to black.
            match decode(next_index) {
                Some(frame) => frame,
                None if !self.strict_materialization => first.clone(),
                None => {
                    return self.fail_materialization(format!(
                        "video source {media_ref} interpolation endpoint decode failed"
                    ));
                }
            }
        };
        let requested = match interpolation.mode {
            TextureInterpolationMode::Nearest => FrameInterpolationMode::Nearest,
            TextureInterpolationMode::Blend => FrameInterpolationMode::Blend,
            TextureInterpolationMode::OpticalFlow => FrameInterpolationMode::OpticalFlow,
        };
        let fallback = match interpolation.fallback {
            TextureInterpolationFallback::Nearest => FrameInterpolationFallback::Nearest,
            TextureInterpolationFallback::Blend => FrameInterpolationFallback::Blend,
            TextureInterpolationFallback::Error => FrameInterpolationFallback::Error,
        };
        let frame = match interpolate_frame_pair(&first, &last, alpha, requested, fallback, true) {
            Ok(result) => result.frame,
            Err(_) => {
                return self.fail_materialization(format!(
                    "video source {media_ref} interpolation failed"
                ));
            }
        };
        let decoded = DecodedFrame::new(frame.width, frame.height, frame.rgba, false);
        let tex = upload_rgba(
            self.device,
            self.queue,
            &decoded,
            false,
            Some("preview-optical-flow"),
        );
        Some(self.cache.insert(key, tex))
    }
}

impl TextureResolver for MediaResolver<'_> {
    fn resolve(&mut self, source: &TextureSource, source_frame: i64) -> Option<Rc<GpuTexture>> {
        let (media_ref, is_image) = match source {
            TextureSource::Decoded { media_ref } => (media_ref, false),
            TextureSource::Image { media_ref } => (media_ref, true),
            TextureSource::Text { clip_id } => return self.resolve_text(clip_id),
            TextureSource::Lottie { media_ref } => {
                let Some(info) = self.media.get(media_ref) else {
                    return self.fail_materialization(format!(
                        "Lottie source {media_ref} is unauthorized"
                    ));
                };
                let result = match info.retained {
                    Some(file) => self.lottie.resolve_file(
                        self.device,
                        self.queue,
                        self.cache,
                        &info.path,
                        file,
                        source_frame,
                        self.preview_box,
                        "preview-lottie",
                    ),
                    None => self.lottie.resolve(
                        self.device,
                        self.queue,
                        self.cache,
                        &info.path,
                        source_frame,
                        self.preview_box,
                        "preview-lottie",
                    ),
                };
                return match result {
                    Ok(texture) => Some(texture),
                    Err(error) => {
                        eprintln!("[render] {error}");
                        self.materialization_error = Some(error);
                        None
                    }
                };
            }
        };

        let Some(info) = self.media.get(media_ref) else {
            return self.fail_materialization(format!("media source {media_ref} is unauthorized"));
        };
        let key = if is_image {
            let content_hash = match info.retained {
                Some(file) => opentake_media::file_sha256_file_cancellable(file, self.cancel),
                None => opentake_media::file_sha256(&info.path),
            };
            let Ok(content_hash) = content_hash else {
                return self
                    .fail_materialization(format!("image source {media_ref} hashing failed"));
            };
            format!("i:{content_hash}")
        } else {
            format!("v:{media_ref}:{source_frame}")
        };

        if let Some(tex) = self.cache.get(&key) {
            return Some(tex);
        }

        let time_secs = if is_image {
            0.0
        } else {
            project_frame_time_secs(source_frame, self.timeline_fps)
        };

        let req = FrameRequest {
            time_secs,
            // A wide seek tolerance makes ffmpeg decode far more than the target
            // frame per call (the dominant per-frame CPU/RSS cost during scrub).
            // 0.1s lands on a nearby keyframe with ~10x less waste; the streaming
            // playback engine (#53) replaces this seek-per-frame path entirely.
            max_size: self.preview_box,
            tolerance_secs: 0.1,
            apply_rotation: true,
        };
        let decoded = match info.retained {
            Some(file) => decode_frame_file_at_cancellable(file, &req, self.cancel),
            None => decode_frame_at_cancellable(&info.path, &req, self.cancel),
        };
        let Ok((_actual, frame)) = decoded else {
            return self.fail_materialization(format!("media source {media_ref} decode failed"));
        };
        // ffmpeg emits straight RGBA; the plan's `needs_premultiply` flag (false
        // for image/video here) drives the shader, so the `premultiplied` marker
        // on the upload is informational only.
        let decoded = DecodedFrame::new(frame.width, frame.height, frame.rgba, false);
        let tex = upload_rgba(
            self.device,
            self.queue,
            &decoded,
            false,
            Some("preview-src"),
        );
        Some(self.cache.insert(key, tex))
    }

    fn resolve_with_interpolation(
        &mut self,
        request: TextureResolveRequest<'_>,
    ) -> Option<Rc<GpuTexture>> {
        match request.source {
            TextureSource::Decoded { media_ref }
                if request.interpolation.mode != TextureInterpolationMode::Nearest =>
            {
                self.resolve_interpolated_video(
                    media_ref,
                    request.source_frame,
                    request.interpolation,
                )
            }
            _ => self.resolve(request.source, request.source_frame),
        }
    }

    fn resolve_lut(
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
            "preview-lut",
        )?;
        if let Some(texture) = &resolved {
            self.lut_cache.insert(reference.id.clone(), texture.clone());
        }
        Ok(resolved)
    }
}

/// Preview render size: even-ized canvas, optionally downscaled so the longest
/// side fits `cap` (0 = no cap). Uniform scale preserves the plan's affine math.
fn preview_render_size(canvas_w: i32, canvas_h: i32, cap: u32) -> RenderSize {
    let cw = (canvas_w.max(2)) as f64;
    let ch = (canvas_h.max(2)) as f64;
    if cap == 0 {
        return RenderSize::new(even(cw), even(ch));
    }
    let long = cw.max(ch);
    let scale = if long > cap as f64 {
        cap as f64 / long
    } else {
        1.0
    };
    RenderSize::new(even(cw * scale), even(ch * scale))
}

/// Derive cover candidate ordering from the same authoritative render plan used
/// by preview/export. Source materialization is deliberately deferred so an
/// offline or corrupt planned layer becomes `CaptureFailed`, not false
/// `NoVisibleContent`.
pub(crate) fn representative_timeline_frame(
    timeline: &Timeline,
    manifest: &opentake_domain::MediaManifest,
    max_size: u32,
) -> Result<Option<i32>, String> {
    Ok(authoritative_render_plan(timeline, manifest, max_size)?.representative_frame(timeline))
}

fn authoritative_render_plan(
    timeline: &Timeline,
    manifest: &opentake_domain::MediaManifest,
    max_size: u32,
) -> Result<RenderPlan, String> {
    let mut sizes = HashMap::new();
    let mut straight_alpha = HashSet::new();
    for entry in &manifest.entries {
        if entry.carries_straight_alpha() {
            straight_alpha.insert(entry.id.clone());
        }
        if let (Some(width), Some(height)) = (entry.source_width, entry.source_height) {
            if width > 0 && height > 0 {
                sizes.insert(entry.id.clone(), (width as u32, height as u32));
            }
        }
    }
    let render_size = preview_render_size(timeline.width, timeline.height, max_size);
    let plan = try_build_render_plan(
        timeline,
        render_size,
        &ManifestMetrics {
            sizes,
            straight_alpha,
        },
    )
    .map_err(|error| format!("invalid timeline graph: {error}"))?;
    Ok(plan)
}

/// Count meaningful visual clips through the authoritative flattened render
/// plan. Each plan entry is evaluated in isolation so a transparent or
/// degenerate clip is not made visible merely by a neighboring transition.
pub(crate) fn authoritative_visible_clip_count(
    timeline: &Timeline,
    manifest: &opentake_domain::MediaManifest,
) -> Result<usize, String> {
    let plan = authoritative_render_plan(timeline, manifest, AGENT_TIMELINE_RESULT_MAX_DIMENSION)?;
    let clip_count = plan
        .clip_plans
        .iter()
        .filter(|clip| {
            RenderPlan {
                fps: plan.fps,
                render_size: plan.render_size,
                total_frames: plan.total_frames,
                clip_plans: vec![(*clip).clone()],
                text_plans: Vec::new(),
                audio_clips: Vec::new(),
            }
            .representative_frame(timeline)
            .is_some()
        })
        .count();
    let text_count = plan
        .text_plans
        .iter()
        .filter(|clip| {
            RenderPlan {
                fps: plan.fps,
                render_size: plan.render_size,
                total_frames: plan.total_frames,
                clip_plans: Vec::new(),
                text_plans: vec![(*clip).clone()],
                audio_clips: Vec::new(),
            }
            .representative_frame(timeline)
            .is_some()
        })
        .count();
    Ok(clip_count + text_count)
}

/// Encode an RGBA composite as PNG bytes. Shared by the preview data-URL path
/// and the capture-to-media on-disk path.
fn encode_png_bytes(frame: &DecodedFrame) -> Result<Vec<u8>, String> {
    use image::ImageEncoder;
    let mut bytes: Vec<u8> = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            &frame.rgba,
            frame.width,
            frame.height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("png encode: {e}"))?;
    Ok(bytes)
}

/// Encode an RGBA composite as a base64 PNG `data:` URL.
fn encode_png_data_url(frame: &DecodedFrame) -> Result<String, String> {
    let bytes = encode_png_bytes(frame)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:image/png;base64,{b64}"))
}

fn timeline_timecode(frame: i32, fps: i32) -> String {
    let fps = fps.max(1);
    let frame = frame.max(0);
    let frames = frame % fps;
    let total_seconds = frame / fps;
    let seconds = total_seconds % 60;
    let total_minutes = total_seconds / 60;
    let minutes = total_minutes % 60;
    let hours = total_minutes / 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}:{frames:02}")
}

fn paint_empty_timeline_overlay(size: RenderSize, timecode: &str) -> DecodedFrame {
    let width = size.width as usize;
    let height = size.height as usize;
    let mut rgba = vec![0_u8; width.saturating_mul(height).saturating_mul(4)];
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.copy_from_slice(&EMPTY_TIMELINE_BACKGROUND_RGBA);
    }

    let mut paint = |x: i32, y: i32| {
        if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
            return;
        }
        let offset = (y as usize * width + x as usize) * 4;
        rgba[offset..offset + 4].copy_from_slice(&EMPTY_TIMELINE_MARKER_RGBA);
    };

    // A language-neutral empty-set marker: outlined circle plus diagonal slash.
    let center_x = width as i32 / 2;
    let center_y = height as i32 * 2 / 5;
    let radius = (width.min(height) as i32 / 9).max(3);
    let thickness = (radius / 7).max(1);
    for y in center_y - radius - thickness..=center_y + radius + thickness {
        for x in center_x - radius - thickness..=center_x + radius + thickness {
            let dx = x - center_x;
            let dy = y - center_y;
            let distance_squared = dx * dx + dy * dy;
            let outer = radius + thickness;
            let inner = (radius - thickness).max(0);
            let on_ring = distance_squared <= outer * outer && distance_squared >= inner * inner;
            let on_slash = (dx + dy).abs() <= thickness && dx.abs().max(dy.abs()) <= radius;
            if on_ring || on_slash {
                paint(x, y);
            }
        }
    }

    // Render the clamped playhead timecode with a deterministic 3x5 bitmap,
    // avoiding locale/system-font dependencies in agent results.
    fn glyph(character: char) -> [u8; 5] {
        match character {
            '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
            '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
            '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
            '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
            '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
            '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
            '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
            '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
            '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
            '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
            ':' => [0, 0b010, 0, 0b010, 0],
            _ => [0; 5],
        }
    }
    let scale = ((width / (timecode.len() * 4)).min(height / 24)).clamp(1, 4) as i32;
    let advance = 4 * scale;
    let text_width = advance * timecode.chars().count() as i32 - scale;
    let origin_x = (width as i32 - text_width) / 2;
    let origin_y = height as i32 * 7 / 10;
    for (index, character) in timecode.chars().enumerate() {
        for (row, bits) in glyph(character).into_iter().enumerate() {
            for column in 0..3 {
                if bits & (1 << (2 - column)) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        paint(
                            origin_x + index as i32 * advance + column * scale + dx,
                            origin_y + row as i32 * scale + dy,
                        );
                    }
                }
            }
        }
    }

    DecodedFrame::new(size.width, size.height, rgba, true)
}

struct TimelineResultTextureResolver {
    texture: Rc<GpuTexture>,
}

impl TextureResolver for TimelineResultTextureResolver {
    fn resolve(&mut self, _source: &TextureSource, _source_frame: i64) -> Option<Rc<GpuTexture>> {
        Some(self.texture.clone())
    }
}

fn composite_empty_timeline_canvas(
    render: &RenderState,
    size: RenderSize,
    timecode: &str,
) -> Result<DecodedFrame, String> {
    let canvas = paint_empty_timeline_overlay(size, timecode);
    let mut guard = render
        .ctx
        .lock()
        .map_err(|_| "render state lock poisoned".to_string())?;
    if guard.is_none() {
        let dev = RenderDevice::try_new().map_err(|error| format!("no GPU device: {error}"))?;
        *guard = Some(GpuContext {
            compositor: Compositor::new(&dev.device),
            text_rasterizer: CosmicTextRasterizer::new(),
            lottie: LottieMaterializer::new(),
            device: dev.device,
            queue: dev.queue,
        });
    }
    let ctx = guard.as_ref().expect("GPU context initialized above");
    let texture = Rc::new(upload_rgba(
        &ctx.device,
        &ctx.queue,
        &canvas,
        false,
        Some("agent-empty-timeline"),
    ));
    let source = TextureSource::Image {
        media_ref: "agent-empty-timeline".to_string(),
    };
    let frame_plan = FramePlan {
        clear_rgba: [
            EMPTY_TIMELINE_BACKGROUND_RGBA[0] as f64 / 255.0,
            EMPTY_TIMELINE_BACKGROUND_RGBA[1] as f64 / 255.0,
            EMPTY_TIMELINE_BACKGROUND_RGBA[2] as f64 / 255.0,
            1.0,
        ],
        draws: vec![LayerDraw {
            source: &source,
            source_frame: 0,
            affine: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            nat_size: (size.width as f64, size.height as f64),
            crop_uv: (0.0, 0.0, 1.0, 1.0),
            opacity: 1.0,
            needs_premultiply: false,
            clip_id: "agent-empty-timeline",
            color_grade: None,
            lut: None,
            chroma_key: None,
            masks: &[],
            effects: &[],
        }],
    };
    let mut resolver = TimelineResultTextureResolver { texture };
    ctx.compositor
        .render_to_rgba(&ctx.device, &ctx.queue, size, &frame_plan, &mut resolver)
        .map_err(|error| format!("compose empty timeline: {error}"))
}

/// Render the post-commit agent result through the Rust compositor. A
/// non-empty timeline delegates to the same strict authoritative compositor as
/// project capture; a genuinely empty render plan gets the explicit semantic
/// project canvas.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_timeline_result_png(
    timeline: &Timeline,
    manifest: &opentake_domain::MediaManifest,
    project_dir: &Option<PathBuf>,
    render: &RenderState,
    input: EmptyTimelineCanvasInput,
    cancel: &MediaCancelToken,
    authority: &CompositeSourceAuthority,
) -> Result<TimelineResultPng, String> {
    if input.project_width != timeline.width
        || input.project_height != timeline.height
        || input.fps != timeline.fps
        || input.project_width <= 0
        || input.project_height <= 0
        || input.fps <= 0
    {
        return Err("empty timeline canvas does not match project snapshot".to_string());
    }
    if cancel.is_cancelled() {
        return Err("timeline result capture cancelled".to_string());
    }
    let total_frames = timeline.total_frames();
    let playhead_frame = if total_frames <= 0 {
        0
    } else {
        input.playhead_frame.clamp(0, total_frames - 1)
    };
    let timecode = timeline_timecode(playhead_frame, input.fps);
    let size = preview_render_size(
        input.project_width,
        input.project_height,
        AGENT_TIMELINE_RESULT_MAX_DIMENSION,
    );
    let empty_canvas = authoritative_visible_clip_count(timeline, manifest)? == 0;
    let frame = if empty_canvas {
        composite_empty_timeline_canvas(render, size, &timecode)?
    } else {
        composite_timeline_frame_authorized(
            timeline,
            manifest,
            project_dir,
            render,
            playhead_frame,
            AGENT_TIMELINE_RESULT_MAX_DIMENSION,
            cancel,
            authority,
        )?
    };
    if cancel.is_cancelled() {
        return Err("timeline result capture cancelled".to_string());
    }
    let bytes = encode_png_bytes(&frame)?;
    if bytes.is_empty() || bytes.len() > AGENT_TIMELINE_RESULT_PNG_BYTES_MAX {
        return Err("timeline result PNG exceeded the bounded payload".to_string());
    }
    Ok(TimelineResultPng {
        bytes,
        media_type: "image/png",
        width: frame.width,
        height: frame.height,
        playhead_frame,
        timecode,
        empty_canvas,
    })
}

/// Composite the timeline at `frame` into an RGBA frame at a size capped by
/// `max_size` (longest side). Shared by [`composite_frame`] (which PNG-encodes it
/// for the preview) and [`capture_frame_to_media`] (which writes it to disk and
/// imports it as a still). Out-of-range frames / an empty timeline composite to
/// opaque black — the correct clear color, not an error.
pub fn composite_timeline_frame(
    timeline: &Timeline,
    manifest: &opentake_domain::MediaManifest,
    project_dir: &Option<PathBuf>,
    render: &RenderState,
    frame: i32,
    max_size: u32,
    cancel: &MediaCancelToken,
) -> Result<DecodedFrame, String> {
    composite_timeline_frame_with_authority(
        timeline,
        manifest,
        project_dir,
        render,
        frame,
        max_size,
        cancel,
        None,
        false,
    )
}

/// Strict cover compositor: every planned draw must materialize from a retained
/// pre-authorized source handle. Missing/failed image, video, text, or Lottie
/// materialization is an error rather than a silently omitted layer.
#[allow(clippy::too_many_arguments)]
pub(crate) fn composite_timeline_frame_authorized(
    timeline: &Timeline,
    manifest: &opentake_domain::MediaManifest,
    project_dir: &Option<PathBuf>,
    render: &RenderState,
    frame: i32,
    max_size: u32,
    cancel: &MediaCancelToken,
    authority: &CompositeSourceAuthority,
) -> Result<DecodedFrame, String> {
    composite_timeline_frame_with_authority(
        timeline,
        manifest,
        project_dir,
        render,
        frame,
        max_size,
        cancel,
        Some(authority),
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn composite_timeline_frame_with_authority(
    timeline: &Timeline,
    manifest: &opentake_domain::MediaManifest,
    project_dir: &Option<PathBuf>,
    render: &RenderState,
    frame: i32,
    max_size: u32,
    cancel: &MediaCancelToken,
    authority: Option<&CompositeSourceAuthority>,
    strict_materialization: bool,
) -> Result<DecodedFrame, String> {
    // Project text clips (content + style + box) so the resolver can rasterize
    // them on demand. Keyed by clip id, matching `TextureSource::Text { clip_id }`.
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

    // Project the manifest into render-side lookups.
    let mut sizes: HashMap<String, (u32, u32)> = HashMap::new();
    let mut straight_alpha = HashSet::new();
    let mut media: HashMap<String, MediaInfo> = HashMap::new();
    for entry in &manifest.entries {
        if entry.carries_straight_alpha() {
            straight_alpha.insert(entry.id.clone());
        }
        let path = match &entry.source {
            MediaSource::External { absolute_path } => PathBuf::from(absolute_path),
            MediaSource::Project { relative_path } => match &project_dir {
                Some(base) => base.join(relative_path),
                None => continue,
            },
        };
        if let (Some(w), Some(h)) = (entry.source_width, entry.source_height) {
            if w > 0 && h > 0 {
                sizes.insert(entry.id.clone(), (w as u32, h as u32));
            }
        }
        let retained = authority.and_then(|authority| authority.files.get(&entry.id));
        if strict_materialization && retained.is_none() {
            continue;
        }
        media.insert(
            entry.id.clone(),
            MediaInfo {
                path,
                retained,
                source_fps: entry.source_fps,
            },
        );
    }

    let render_size = preview_render_size(timeline.width, timeline.height, max_size);

    let metrics = ManifestMetrics {
        sizes,
        straight_alpha,
    };
    let plan = try_build_render_plan(timeline, render_size, &metrics)
        .map_err(|error| format!("invalid timeline graph: {error}"))?;
    let frame_plan = plan.frame(timeline, frame);
    let project_root = if strict_materialization {
        None
    } else {
        project_dir
            .as_deref()
            .map(ProjectRoot::open)
            .transpose()
            .map_err(|error| format!("open project LUT storage: {error}"))?
    };

    // Acquire (or reuse) the GPU context, then composite + read back. The lock is
    // held across the render so the `Rc`-based texture cache never crosses threads.
    let mut guard = render
        .ctx
        .lock()
        .map_err(|_| "render state lock poisoned".to_string())?;
    if guard.is_none() {
        let dev = RenderDevice::try_new().map_err(|e| format!("no GPU device: {e}"))?;
        let compositor = Compositor::new(&dev.device);
        let text_rasterizer = CosmicTextRasterizer::new();
        if !text_rasterizer.has_fonts() {
            eprintln!("[render] no system fonts discovered; text clips will render blank");
        }
        *guard = Some(GpuContext {
            device: dev.device,
            queue: dev.queue,
            compositor,
            text_rasterizer,
            lottie: LottieMaterializer::new(),
        });
    }
    let result = {
        let ctx = guard.as_mut().expect("ctx set above");
        let mut texture_cache = TextureCache::new(TEXTURE_CACHE_CAP);
        let mut lut_cache = HashMap::new();
        let mut resolver = MediaResolver {
            device: &ctx.device,
            queue: &ctx.queue,
            cache: &mut texture_cache,
            lottie: &mut ctx.lottie,
            media: &media,
            timeline_fps: plan.fps,
            text: &text,
            text_rasterizer: &ctx.text_rasterizer,
            preview_box: (render_size.width, render_size.height),
            cancel,
            project_root: project_root.as_ref(),
            lut_cache: &mut lut_cache,
            materialization_error: None,
            strict_materialization,
        };
        let interpolation = TextureInterpolationConfig::new(
            plan.fps as f64,
            plan.fps as f64,
            TextureInterpolationMode::OpticalFlow,
            TextureInterpolationFallback::Blend,
        )
        .map_err(str::to_string)?;
        let composite = ctx
            .compositor
            .render_to_rgba_with_interpolation(
                &ctx.device,
                &ctx.queue,
                render_size,
                &frame_plan,
                &mut resolver,
                interpolation,
            )
            .map_err(|e| format!("composite render failed: {e}"));
        match resolver.materialization_error.take() {
            Some(error) => Err(format!("layer materialization failed: {error}")),
            None => composite,
        }
    };
    if result.is_err() {
        // A wgpu device loss or validation failure invalidates every pipeline
        // and parsed-document renderer bound to this context. The next preview
        // request reacquires a fresh device and starts with empty caches.
        guard.take();
    }
    result
}

fn composite_rgba(
    core: &AppCore,
    render: &RenderState,
    frame: i32,
    max_size: u32,
    cancel: &MediaCancelToken,
    sequence_id: Option<&str>,
) -> Result<DecodedFrame, String> {
    let snapshot = core.runtime_snapshot();
    let selected = sequence_id
        .map(|sequence_id| {
            let mut timeline = snapshot
                .timeline
                .nested_sequences
                .iter()
                .find(|sequence| sequence.id == sequence_id)
                .map(|sequence| sequence.timeline.clone())
                .ok_or_else(|| format!("nested sequence not found: {sequence_id}"))?;
            timeline.nested_sequences = snapshot.timeline.nested_sequences.clone();
            timeline
                .nested_sequences
                .iter_mut()
                .find(|sequence| sequence.id == sequence_id)
                .expect("selected sequence came from this registry")
                .timeline = opentake_domain::Timeline::new();
            Ok::<_, String>(timeline)
        })
        .transpose()?;
    composite_timeline_frame(
        selected.as_ref().unwrap_or(&snapshot.timeline),
        &snapshot.media,
        &snapshot.project_dir,
        render,
        frame,
        max_size,
        cancel,
    )
}

/// `composite_frame`: render the timeline at `frame` to a PNG data URL.
///
/// `max_size` caps the longest side (px); omit it for the default preview cap.
#[tauri::command]
pub fn composite_frame(
    core: State<'_, AppCore>,
    render: State<'_, RenderState>,
    request: CompositeFrameRequest,
    max_size: Option<u32>,
) -> Result<CompositeFrameDto, String> {
    let revision = ProjectRevision {
        project_epoch: request.project_epoch,
        version: request.timeline_version,
    };
    if core.project_revision() != revision {
        return Err("preview composite revision was superseded".to_string());
    }
    let cancel = render.preview.begin(
        revision,
        &request.session_id,
        request.session_generation,
        request.seek_generation,
    )?;
    let preview_cap = max_size.unwrap_or(DEFAULT_PREVIEW_CAP);
    let composite = match request.source_media_id.as_deref() {
        Some(media_id) => {
            decode_source_frame(&core, media_id, request.frame, preview_cap, &cancel)?
        }
        None => composite_rgba(
            &core,
            &render,
            request.frame,
            preview_cap,
            &cancel,
            request.sequence_id.as_deref(),
        )?,
    };
    if cancel.is_cancelled()
        || core.project_revision() != revision
        || !render.preview.is_current(
            revision,
            &request.session_id,
            request.session_generation,
            request.seek_generation,
        )
    {
        return Err("preview composite was superseded".to_string());
    }
    let data_url = encode_png_data_url(&composite)?;
    if cancel.is_cancelled()
        || core.project_revision() != revision
        || !render.preview.is_current(
            revision,
            &request.session_id,
            request.session_generation,
            request.seek_generation,
        )
    {
        return Err("preview composite was superseded".to_string());
    }
    if request.source_media_id.is_none() && request.sequence_id.is_none() {
        record_root_timeline_playhead(request.project_epoch, request.frame);
    }
    Ok(CompositeFrameDto {
        width: composite.width,
        height: composite.height,
        data_url,
    })
}

#[tauri::command]
pub fn cancel_composite_frame(
    render: State<'_, RenderState>,
    project_epoch: u64,
    timeline_version: u64,
    session_id: String,
    session_generation: u64,
    minimum_seek_generation: u64,
) {
    let revision = ProjectRevision {
        project_epoch,
        version: timeline_version,
    };
    render.preview.cancel_before(
        revision,
        &session_id,
        session_generation,
        minimum_seek_generation,
    );
}

/// `capture_frame_to_media`: composite the timeline at `frame` and import the
/// result as a NEW still image in the media library — the port of upstream's
/// `captureCurrentFrameToMedia` (EditorViewModel+MediaLibrary.swift:306-390),
/// which composites and then hands the PNG to `importPastedImageData(...)` (the
/// same import machinery a user drop uses), renames it `"{nameBase} {frame}"`,
/// and moves it into the current media-panel folder.
///
/// `name_base` is upstream's `nameBase` (`"Frame"` for the timeline tab, the
/// source asset's name for a single-clip video tab); the imported asset is named
/// `"{name_base} {frame}"`. `folder_id` is the current media-panel folder (the
/// still lands there, else at root). Returns the updated catalog.
///
/// `source_media_id` selects the tab, mirroring upstream's internal
/// `switch tab` branch: `None` composites the whole TIMELINE at `frame` (full
/// canvas resolution, no preview cap — this becomes a real asset), while `Some`
/// decodes that single VIDEO asset's own frame at `frame` (upstream's video-tab
/// path uses `videoComposition = nil`, i.e. the raw asset frame, not a
/// composite). Both then import identically.
#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri injects shared render/media/update state
pub fn capture_frame_to_media(
    core: State<'_, AppCore>,
    render: State<'_, RenderState>,
    media: State<'_, crate::media::MediaState>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    frame: i32,
    name_base: String,
    folder_id: Option<String>,
    source_media_id: Option<String>,
) -> Result<crate::media::MediaListDto, String> {
    let _activity = crate::updater::begin_mutating_activity(&admission)?;
    capture_frame_to_media_impl(&core, || {
        capture_frame_to_media_workflow(
            &core,
            &render,
            media.engine(),
            frame,
            &name_base,
            folder_id,
            source_media_id.as_deref(),
        )
    })
}

fn capture_frame_to_media_impl(
    core: &AppCore,
    workflow: impl FnOnce() -> Result<crate::media::MediaListDto, String>,
) -> Result<crate::media::MediaListDto, String> {
    core.ensure_project_mutable().map_err(|e| e.to_string())?;
    workflow()
}

fn capture_frame_to_media_workflow(
    core: &AppCore,
    render: &RenderState,
    engine: &opentake_media::MediaEngine,
    frame: i32,
    name_base: &str,
    folder_id: Option<String>,
    source_media_id: Option<&str>,
) -> Result<crate::media::MediaListDto, String> {
    // Frame → RGBA. Timeline tab composites; video tab decodes the source frame.
    let composite = match source_media_id {
        None => composite_rgba(core, render, frame, 0, &MediaCancelToken::new(), None)?,
        Some(id) => decode_source_frame(core, id, frame, 0, &MediaCancelToken::new())?,
    };

    // Write the PNG next to the media cache so a subsequent project save can copy
    // it into the bundle like any other external asset. The frame number keys the
    // filename so repeated captures at different frames don't collide.
    let captures_dir = engine.cache_root().join("captures");
    std::fs::create_dir_all(&captures_dir).map_err(|e| format!("create captures dir: {e}"))?;
    let png_path = captures_dir.join(format!("capture-{frame:06}-{}.png", uuid_like()));
    let bytes = encode_png_bytes(&composite)?;
    std::fs::write(&png_path, &bytes).map_err(|e| format!("write capture png: {e}"))?;

    // Import through the SAME path as a user import (posters + manifest entry +
    // MediaChanged event), then rename to the upstream "{nameBase} {frame}" and
    // move into the current folder.
    let entry = crate::media::import_one(core, engine, &png_path)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "capture import failed".to_string())?;
    let name = format!("{name_base} {frame}");
    core.apply(EditCommand::RenameMedia {
        entries: vec![RenameEntry {
            id: entry.id.clone(),
            name,
        }],
    })
    .map_err(|e| e.to_string())?;
    if let Some(fid) = folder_id {
        core.apply(EditCommand::MoveToFolder {
            asset_ids: vec![entry.id.clone()],
            folder_id: Some(fid),
        })
        .map_err(|e| e.to_string())?;
    }

    Ok(crate::media::MediaListDto::from_core(
        core,
        Some(engine.cache_root()),
    ))
}

/// Decode a single VIDEO asset's own frame at project-frame `frame` into a
/// full-resolution RGBA frame (video-tab capture; upstream's `videoComposition =
/// nil` raw-asset path). The frame → time uses the TIMELINE fps, matching
/// upstream's `CMTime(value: frame, timescale: fps)` (fps = timeline fps for both
/// tabs). Errors when the asset is unknown, not a video, or its source is offline.
fn decode_source_frame(
    core: &AppCore,
    media_id: &str,
    frame: i32,
    max_size: u32,
    cancel: &MediaCancelToken,
) -> Result<DecodedFrame, String> {
    let snapshot = core.runtime_snapshot();
    let timeline = snapshot.timeline;
    let manifest = snapshot.media;
    let entry = manifest
        .entries
        .iter()
        .find(|e| e.id == media_id)
        .ok_or_else(|| format!("media not found: {media_id}"))?;
    if entry.kind != ClipType::Video {
        return Err("capture: source tab asset is not a video".to_string());
    }
    let path = match &entry.source {
        MediaSource::External { absolute_path } => PathBuf::from(absolute_path),
        MediaSource::Project { relative_path } => snapshot
            .project_dir
            .map(|base| base.join(relative_path))
            .ok_or_else(|| "project not saved; cannot resolve media path".to_string())?,
    };
    if !path.is_file() {
        return Err(format!("source file not found: {}", path.display()));
    }
    let req = source_frame_request(frame, timeline.fps, max_size);
    let (_, rgba) = decode_frame_at_cancellable(&path, &req, cancel)
        .map_err(|e| format!("decode source frame: {e}"))?;
    Ok(DecodedFrame::new(rgba.width, rgba.height, rgba.rgba, false))
}

fn source_frame_request(frame: i32, timeline_fps: i32, max_size: u32) -> FrameRequest {
    let fps = if timeline_fps > 0 { timeline_fps } else { 30 };
    FrameRequest {
        time_secs: (frame.max(0) as f64) / fps as f64,
        max_size: (max_size, max_size),
        // Source-tab stills and captures promise the frame selected by the app
        // transport. The generic one-second thumbnail tolerance can seek back
        // to frame zero for short generated clips whose opening frame is blank.
        tolerance_secs: 0.0,
        apply_rotation: true,
    }
}

/// A short unique-ish suffix (nanos since epoch) to keep capture filenames from
/// colliding when the same frame is captured twice. Not cryptographic — just a
/// disambiguator so two captures of the same frame don't overwrite each other.
fn uuid_like() -> u128 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed) as u128;
    (nanos << 16) | (counter & 0xffff)
}

fn freeze_capture_png_path(
    captures_dir: &std::path::Path,
    clip_id: &str,
    at_frame: i32,
) -> PathBuf {
    let safe_id = clip_id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    captures_dir.join(format!("freeze_{safe_id}_{at_frame}_{}.png", uuid_like()))
}

#[derive(Debug)]
pub struct PreparedFreezeFrame {
    pub path: PathBuf,
    pub media: opentake_domain::MediaManifestEntry,
}

pub fn capture_freeze_frame(
    core: &AppCore,
    render: &RenderState,
    media: &crate::media::MediaState,
    clip_id: &str,
    at_frame: i32,
) -> Result<PreparedFreezeFrame, String> {
    capture_freeze_frame_impl(core, || {
        capture_freeze_frame_workflow(core, render, media.engine(), clip_id, at_frame)
    })
}

fn capture_freeze_frame_impl(
    core: &AppCore,
    workflow: impl FnOnce() -> Result<PreparedFreezeFrame, String>,
) -> Result<PreparedFreezeFrame, String> {
    core.ensure_project_mutable().map_err(|e| e.to_string())?;
    workflow()
}

fn capture_freeze_frame_workflow(
    core: &AppCore,
    render: &RenderState,
    engine: &opentake_media::MediaEngine,
    clip_id: &str,
    at_frame: i32,
) -> Result<PreparedFreezeFrame, String> {
    let snapshot = core.runtime_snapshot();
    let timeline = snapshot.timeline;
    let manifest = snapshot.media;
    let project_dir = snapshot.project_dir;
    let (solo_timeline, solo_manifest) =
        build_freeze_capture_snapshot(&timeline, &manifest, clip_id)?;
    let composite = composite_timeline_frame(
        &solo_timeline,
        &solo_manifest,
        &project_dir,
        render,
        at_frame,
        0,
        &MediaCancelToken::new(),
    )?;
    let captures_dir = engine.cache_root().join("captures");
    std::fs::create_dir_all(&captures_dir).map_err(|e| format!("create captures dir: {e}"))?;
    let png_path = freeze_capture_png_path(&captures_dir, clip_id, at_frame);
    let bytes = encode_png_bytes(&composite)?;
    std::fs::write(&png_path, &bytes).map_err(|e| format!("write freeze png: {e}"))?;
    let probe = crate::media::probe_media(engine, &png_path);
    let name = png_path
        .file_stem()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Freeze frame".to_string());
    let media = core
        .prepare_media_file_entry(&png_path, name, &probe)
        .map_err(|error| error.to_string())?;
    Ok(PreparedFreezeFrame {
        path: png_path,
        media,
    })
}

fn build_freeze_capture_snapshot(
    timeline: &Timeline,
    manifest: &opentake_domain::MediaManifest,
    clip_id: &str,
) -> Result<(Timeline, opentake_domain::MediaManifest), String> {
    let track = timeline
        .tracks
        .iter()
        .find(|track| track.clips.iter().any(|clip| clip.id == clip_id))
        .ok_or_else(|| format!("clip not found: {clip_id}"))?;
    let clip = track
        .clips
        .iter()
        .find(|clip| clip.id == clip_id)
        .ok_or_else(|| format!("clip not found: {clip_id}"))?;

    let mut solo_track = track.clone();
    solo_track.clips = vec![clip.clone()];
    solo_track.hidden = false;
    solo_track.muted = false;

    let mut solo_timeline = timeline.clone();
    solo_timeline.tracks = vec![solo_track];

    let mut subset = manifest.clone();
    subset.entries.retain(|entry| entry.id == clip.media_ref);
    subset.folders.clear();
    subset.favorites.clear();
    if subset.entries.is_empty() {
        return Err(format!("media not found for clip: {}", clip.media_ref));
    }

    Ok((solo_timeline, subset))
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
    fn composite_request_can_target_a_single_source_asset() {
        let request: CompositeFrameRequest = serde_json::from_value(serde_json::json!({
            "frame": 1572,
            "projectEpoch": 3,
            "timelineVersion": 7,
            "sessionId": "source-still",
            "sessionGeneration": 1,
            "seekGeneration": 4,
            "sourceMediaId": "main10"
        }))
        .expect("source still request decodes");

        assert_eq!(request.source_media_id.as_deref(), Some("main10"));
    }

    #[test]
    fn source_still_request_seeks_the_exact_requested_frame() {
        let request = source_frame_request(22, 30, 0);

        assert!((request.time_secs - (22.0 / 30.0)).abs() < f64::EPSILON);
        assert_eq!(request.max_size, (0, 0));
        assert_eq!(request.tolerance_secs, 0.0);
        assert!(request.apply_rotation);
    }

    #[test]
    fn preview_composite_cancel_floor_kills_old_work_but_not_its_successor() {
        let coordinator = PreviewCompositeCoordinator::default();
        let revision = opentake_core::ProjectRevision {
            project_epoch: 3,
            version: 7,
        };
        let old = coordinator
            .begin(revision, "idle-session", 1, 0)
            .expect("initial request accepted");

        coordinator.cancel_before(revision, "idle-session", 1, 1);
        assert!(old.is_cancelled());

        let current = coordinator
            .begin(revision, "idle-session", 1, 1)
            .expect("request at the cancel floor accepted");
        coordinator.cancel_before(revision, "idle-session", 1, 1);
        assert!(!current.is_cancelled());
        assert!(coordinator.is_current(revision, "idle-session", 1, 1));
    }

    #[test]
    fn preview_composite_rejects_a_generation_below_the_cancel_floor() {
        let coordinator = PreviewCompositeCoordinator::default();
        let revision = opentake_core::ProjectRevision {
            project_epoch: 8,
            version: 2,
        };
        coordinator.cancel_before(revision, "idle-session", 1, 4);

        assert!(coordinator.begin(revision, "idle-session", 1, 3).is_err());
    }
    use opentake_domain::{Clip, MediaManifest, MediaManifestEntry, Track};
    use std::fs;

    fn unknown_core(root: &std::path::Path) -> AppCore {
        let bundle = root.join("Unknown.opentake");
        let project = opentake_project::Project::new(&bundle);
        project.save().expect("save known fixture");
        let path = bundle.join("project.json");
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("read timeline fixture"))
                .expect("decode timeline fixture");
        value["futureTimeline"] = serde_json::json!(true);
        fs::write(
            &path,
            serde_json::to_vec_pretty(&value).expect("encode unknown fixture"),
        )
        .expect("write unknown fixture");
        let core = AppCore::new();
        core.open_project(bundle).expect("unknown project opens");
        core
    }

    fn recursive_tree(root: &std::path::Path) -> Vec<(PathBuf, Vec<u8>)> {
        fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
            if !dir.exists() {
                return;
            }
            let mut paths = fs::read_dir(dir)
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
                    out.push((relative, fs::read(&path).expect("read tree file")));
                }
            }
        }
        let mut out = Vec::new();
        walk(root, root, &mut out);
        out
    }

    #[test]
    fn capture_frame_to_media_refuses_before_capture_creation() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let core = unknown_core(tmp.path());
        let captures = tmp.path().join("cache/captures");
        fs::create_dir_all(captures.join("existing")).expect("create captures fixture");
        fs::write(captures.join("existing/keep.png"), b"before").expect("write captures fixture");
        let before = recursive_tree(&captures);
        let called = std::cell::Cell::new(false);
        let sentinel = captures.join("frame-workflow-ran-before-guard.png");

        let error = capture_frame_to_media_impl(&core, || {
            called.set(true);
            fs::write(&sentinel, b"bad ordering").expect("write workflow sentinel");
            Err("workflow should not run".into())
        })
        .expect_err("capture frame must be rejected");

        assert!(error.contains("compatibility read-only"), "{error}");
        assert!(!called.get());
        assert!(!sentinel.exists());
        assert_eq!(recursive_tree(&captures), before);
    }

    #[test]
    fn capture_freeze_frame_refuses_before_capture_creation() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let core = unknown_core(tmp.path());
        let captures = tmp.path().join("cache/captures");
        fs::create_dir_all(captures.join("existing")).expect("create captures fixture");
        fs::write(captures.join("existing/keep.png"), b"before").expect("write captures fixture");
        let before = recursive_tree(&captures);
        let called = std::cell::Cell::new(false);
        let sentinel = captures.join("freeze-workflow-ran-before-guard.png");

        let error = capture_freeze_frame_impl(&core, || {
            called.set(true);
            fs::write(&sentinel, b"bad ordering").expect("write workflow sentinel");
            Err("workflow should not run".into())
        })
        .expect_err("freeze frame must be rejected");

        assert!(error.contains("compatibility read-only"), "{error}");
        assert!(!called.get());
        assert!(!sentinel.exists());
        assert_eq!(recursive_tree(&captures), before);
    }

    #[test]
    fn project_frame_time_uses_timeline_fps_not_source_fps() {
        // A 59.94fps source on a 30fps timeline still uses the project-frame
        // timebase, matching Swift CompositionBuilder's CMTime(timescale: fps).
        assert!((project_frame_time_secs(155, 30) - 5.1666666667).abs() < 0.0001);
    }

    #[test]
    fn preview_size_even_izes_without_cap() {
        let rs = preview_render_size(1921, 1081, 0);
        assert_eq!(rs, RenderSize::new(1920, 1080));
    }

    #[test]
    fn preview_size_downscales_to_cap_keeping_aspect() {
        // 1920x1080, cap 1280 -> scale 1280/1920 -> 1280x720.
        let rs = preview_render_size(1920, 1080, 1280);
        assert_eq!(rs, RenderSize::new(1280, 720));
    }

    #[test]
    fn preview_size_never_upscales_under_cap() {
        let rs = preview_render_size(640, 480, 1280);
        assert_eq!(rs, RenderSize::new(640, 480));
    }

    #[test]
    fn preview_size_floors_degenerate_canvas() {
        let rs = preview_render_size(0, 0, 1280);
        assert_eq!(rs, RenderSize::new(2, 2));
    }

    #[test]
    fn preview_composite_rejects_missing_image_instead_of_returning_a_black_frame() {
        if RenderDevice::try_new().is_err() {
            return;
        }
        let tmp = tempfile::tempdir().expect("missing image fixture");
        let mut timeline = Timeline {
            width: 32,
            height: 32,
            fps: 30,
            ..Timeline::new()
        };
        let mut clip = Clip::new("missing-clip", "missing-image", 0, 1);
        clip.media_type = ClipType::Image;
        clip.source_clip_type = ClipType::Image;
        let mut track = Track::new("video", ClipType::Video);
        track.clips.push(clip);
        timeline.tracks.push(track);

        let mut manifest = MediaManifest::new();
        manifest.entries.push(MediaManifestEntry {
            id: "missing-image".into(),
            name: "gone.png".into(),
            kind: ClipType::Image,
            source: MediaSource::External {
                absolute_path: tmp.path().join("gone.png").display().to_string(),
            },
            duration: 0.0,
            generation_input: None,
            source_width: Some(32),
            source_height: Some(32),
            source_fps: None,
            has_audio: Some(false),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        });

        let error = composite_timeline_frame(
            &timeline,
            &manifest,
            &None,
            &RenderState::new(),
            0,
            32,
            &MediaCancelToken::new(),
        )
        .expect_err("missing preview media must not become a successful black frame");
        assert!(error.contains("image source missing-image"), "{error}");
    }

    #[test]
    fn encode_png_data_url_has_png_prefix() {
        let frame = DecodedFrame::new(1, 1, vec![10, 20, 30, 255], false);
        let url = encode_png_data_url(&frame).expect("encode");
        assert!(url.starts_with("data:image/png;base64,"));
        // Round-trips to a non-empty payload.
        let b64 = url.strip_prefix("data:image/png;base64,").unwrap();
        assert!(!b64.is_empty());
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .expect("valid base64");
        // PNG magic number.
        assert_eq!(&bytes[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn empty_timeline_result_png_is_bounded_and_contains_background_and_semantic_overlay() {
        let timeline = Timeline {
            width: 320,
            height: 180,
            fps: 24,
            ..Timeline::new()
        };

        let rendered = render_timeline_result_png(
            &timeline,
            &MediaManifest::new(),
            &None,
            &RenderState::new(),
            EmptyTimelineCanvasInput {
                project_width: timeline.width,
                project_height: timeline.height,
                fps: timeline.fps,
                playhead_frame: 10_000,
            },
            &MediaCancelToken::new(),
            &CompositeSourceAuthority::new(HashMap::new()),
        )
        .expect("render deterministic empty canvas");

        assert_eq!(rendered.media_type, "image/png");
        assert!(rendered.bytes.len() <= AGENT_TIMELINE_RESULT_PNG_BYTES_MAX);
        let decoded = image::load_from_memory_with_format(&rendered.bytes, image::ImageFormat::Png)
            .expect("decode result PNG")
            .into_rgba8();
        assert_eq!(decoded.dimensions(), (320, 180));
        assert!(decoded.width().max(decoded.height()) <= AGENT_TIMELINE_RESULT_MAX_DIMENSION);
        let pixels = decoded.pixels().collect::<Vec<_>>();
        assert!(pixels
            .iter()
            .any(|pixel| pixel.0 == EMPTY_TIMELINE_BACKGROUND_RGBA));
        assert!(pixels
            .iter()
            .any(|pixel| pixel.0 == EMPTY_TIMELINE_MARKER_RGBA));
        assert_eq!(rendered.playhead_frame, 0);
        assert_eq!(rendered.timecode, "00:00:00:00");
    }

    #[test]
    fn authoritative_visible_count_uses_render_plan_source_surface_and_track_semantics() {
        let mut timeline = Timeline {
            width: 320,
            height: 180,
            fps: 24,
            ..Timeline::new()
        };
        let mut text = Clip::new("meaningful-text", "", 0, 24);
        text.media_type = ClipType::Text;
        text.source_clip_type = ClipType::Text;
        text.text_content = Some("   ".into());
        text.text_style = Some(TextStyle::default());
        text.transform.width = 0.5;
        text.transform.height = 0.5;
        let mut track = Track::new("text", ClipType::Text);
        track.clips.push(text);
        timeline.tracks.push(track);

        assert_eq!(
            authoritative_visible_clip_count(&timeline, &MediaManifest::new()).unwrap(),
            0,
            "blank text has no meaningful render-plan source"
        );
        timeline.tracks[0].clips[0].text_content = Some("visible".into());
        assert_eq!(
            authoritative_visible_clip_count(&timeline, &MediaManifest::new()).unwrap(),
            1
        );
        timeline.tracks[0].clips[0].opacity = 0.0;
        assert_eq!(
            authoritative_visible_clip_count(&timeline, &MediaManifest::new()).unwrap(),
            0,
            "zero-opacity surfaces are not visible"
        );
        timeline.tracks[0].clips[0].opacity = 1.0;
        timeline.tracks[0].hidden = true;
        assert_eq!(
            authoritative_visible_clip_count(&timeline, &MediaManifest::new()).unwrap(),
            0,
            "hidden tracks never enter the authoritative render plan"
        );
    }

    #[test]
    fn empty_timeline_nonempty_fixture_uses_authoritative_timeline_compositor() {
        let mut timeline = Timeline {
            width: 320,
            height: 180,
            fps: 25,
            ..Timeline::new()
        };
        let mut text = Clip::new("fixture-text", "", 0, 25);
        text.media_type = ClipType::Text;
        text.source_clip_type = ClipType::Text;
        text.text_content = Some("fixture".into());
        text.text_style = Some(TextStyle::default());
        text.transform.width = 0.5;
        text.transform.height = 0.5;
        let mut track = Track::new("fixture-track", ClipType::Text);
        track.clips.push(text);
        timeline.tracks.push(track);
        let render = RenderState::new();
        let authority = CompositeSourceAuthority::new(HashMap::new());
        let cancel = MediaCancelToken::new();

        let rendered = render_timeline_result_png(
            &timeline,
            &MediaManifest::new(),
            &None,
            &render,
            EmptyTimelineCanvasInput {
                project_width: timeline.width,
                project_height: timeline.height,
                fps: timeline.fps,
                playhead_frame: 12,
            },
            &cancel,
            &authority,
        )
        .expect("render non-empty fixture");
        let direct = composite_timeline_frame_authorized(
            &timeline,
            &MediaManifest::new(),
            &None,
            &render,
            12,
            AGENT_TIMELINE_RESULT_MAX_DIMENSION,
            &cancel,
            &authority,
        )
        .and_then(|frame| encode_png_bytes(&frame))
        .expect("direct authoritative render");

        assert!(!rendered.empty_canvas);
        assert_eq!(rendered.bytes, direct);
    }

    #[test]
    fn freeze_capture_snapshot_isolates_target_clip_and_media() {
        let mut timeline = Timeline::new();
        let mut target_track = Track::new("v1", ClipType::Video);
        target_track.hidden = true;
        target_track.muted = true;
        target_track
            .clips
            .push(Clip::new("clip-1", "asset-1", 100, 60));
        let mut overlay_track = Track::new("v2", ClipType::Video);
        overlay_track
            .clips
            .push(Clip::new("clip-2", "asset-2", 100, 60));
        timeline.tracks = vec![target_track, overlay_track];

        let mut manifest = MediaManifest::default();
        manifest.entries.push(MediaManifestEntry {
            id: "asset-1".into(),
            name: "asset-1".into(),
            kind: ClipType::Video,
            source: MediaSource::External {
                absolute_path: "/tmp/a.mov".into(),
            },
            duration: 1.0,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(false),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        });
        manifest.entries.push(MediaManifestEntry {
            id: "asset-2".into(),
            name: "asset-2".into(),
            kind: ClipType::Video,
            source: MediaSource::External {
                absolute_path: "/tmp/b.mov".into(),
            },
            duration: 1.0,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(false),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        });

        let (solo_timeline, solo_manifest) =
            build_freeze_capture_snapshot(&timeline, &manifest, "clip-1").expect("snapshot");
        assert_eq!(solo_timeline.tracks.len(), 1);
        assert_eq!(solo_timeline.tracks[0].clips.len(), 1);
        assert_eq!(solo_timeline.tracks[0].clips[0].id, "clip-1");
        assert!(!solo_timeline.tracks[0].hidden);
        assert!(!solo_timeline.tracks[0].muted);
        assert_eq!(solo_manifest.entries.len(), 1);
        assert_eq!(solo_manifest.entries[0].id, "asset-1");
    }

    #[test]
    fn freeze_capture_png_path_is_unique_for_same_clip_and_frame() {
        let captures_dir = PathBuf::from("/tmp/captures");
        let first = freeze_capture_png_path(&captures_dir, "clip:1", 42);
        let second = freeze_capture_png_path(&captures_dir, "clip:1", 42);
        assert_ne!(first, second);
        assert!(first.to_string_lossy().contains("freeze_clip_1_42_"));
        assert!(second.to_string_lossy().contains("freeze_clip_1_42_"));
    }
}
