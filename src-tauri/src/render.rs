//! Timeline composite-frame rendering for the preview (#47-A).
//!
//! Wires the ready-made wgpu compositor (`opentake-render`) to the live editing
//! session: build a `RenderPlan` from the current `Timeline`, evaluate one frame
//! into an ordered draw list, resolve each layer's pixels through ffmpeg decode
//! (`opentake-media`), composite on the GPU, read back, and return the frame as a
//! base64 PNG data URL the WebView paints onto a `<canvas>` (replacing the black
//! placeholder shown on the Timeline tab).
//!
//! Scope (first cut, #47-A): **video + image** layers — the core fix for the
//! black timeline preview. **Text / Lottie** layers are skipped (the resolver
//! returns `None`, so the compositor simply omits them) until their raster paths
//! are wired; see #47 follow-ups (#52/#53).
//!
//! The GPU device + compositor are acquired once and cached in Tauri managed
//! state ([`RenderState`]); only the per-frame texture cache is short-lived. A
//! single `Mutex` serializes composites, which is what we want for the preview
//! (one frame at a time, no GPU contention). The continuous playback engine
//! (#53) will move this onto a dedicated render thread.

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Mutex;

use base64::Engine as _;
use serde::Serialize;
use tauri::State;

use opentake_core::AppCore;
use opentake_domain::MediaSource;
use opentake_media::{decode_frame_at, FrameRequest};
use opentake_render::gpu::texture::upload_rgba;
use opentake_render::wgpu;
use opentake_render::{
    build_render_plan, even, Compositor, DecodedFrame, GpuTexture, RenderDevice, RenderSize,
    SourceMetrics, TextureCache, TextureResolver, TextureSource,
};

/// Cap (longest canvas side, px) for a composite when the caller passes no
/// `max_size`. Keeps the PNG payload small for interactive scrubbing while still
/// looking crisp in the preview pane.
const DEFAULT_PREVIEW_CAP: u32 = 1280;

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

/// Lazily-acquired GPU device + compositor, cached across composite calls.
struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    compositor: Compositor,
}

/// Tauri managed state holding the (lazily created) GPU context. `None` until the
/// first composite; an acquisition failure (no adapter / headless) surfaces to
/// the caller as a command error rather than panicking.
#[derive(Default)]
pub struct RenderState {
    ctx: Mutex<Option<GpuContext>>,
}

impl RenderState {
    /// An empty render state (GPU acquired on first `composite_frame`).
    pub fn new() -> Self {
        RenderState::default()
    }
}

/// Resolvable info for one media asset, projected from the manifest.
struct MediaInfo {
    path: PathBuf,
    /// Source frames-per-second (`0.0` when unknown → resolver falls back to 30).
    fps: f64,
}

/// `SourceMetrics` backed by the media manifest: only intrinsic size is known
/// here (orientation/alpha use the documented identity/false defaults; ffmpeg
/// auto-rotates on decode in this first cut).
struct ManifestMetrics {
    sizes: HashMap<String, (u32, u32)>,
}

impl SourceMetrics for ManifestMetrics {
    fn natural_size(&self, media_ref: &str) -> Option<(u32, u32)> {
        self.sizes.get(media_ref).copied()
    }
}

/// `TextureResolver` that decodes a layer's pixels on demand via ffmpeg and
/// uploads them to the GPU (with a small LRU cache). Video/image only; text and
/// Lottie return `None` (skipped by the compositor) in this cut.
struct MediaResolver<'d> {
    device: &'d wgpu::Device,
    queue: &'d wgpu::Queue,
    cache: TextureCache,
    media: &'d HashMap<String, MediaInfo>,
    /// Downscale box for decoded source frames (matches the preview render size).
    preview_box: (u32, u32),
}

impl TextureResolver for MediaResolver<'_> {
    fn resolve(&mut self, source: &TextureSource, source_frame: i64) -> Option<Rc<GpuTexture>> {
        // Map the source to (asset id, cache key). Video keys per frame; images
        // key once. Text/Lottie are not supported yet.
        let (media_ref, key, is_image) = match source {
            TextureSource::Decoded { media_ref } => {
                (media_ref, format!("v:{media_ref}:{source_frame}"), false)
            }
            TextureSource::Image { media_ref } => (media_ref, format!("i:{media_ref}"), true),
            TextureSource::Lottie { .. } | TextureSource::Text { .. } => return None,
        };

        if let Some(tex) = self.cache.get(&key) {
            return Some(tex);
        }

        let info = self.media.get(media_ref)?;
        let time_secs = if is_image {
            0.0
        } else {
            let fps = if info.fps > 0.0 { info.fps } else { 30.0 };
            (source_frame.max(0) as f64) / fps
        };

        let req = FrameRequest {
            time_secs,
            max_size: self.preview_box,
            tolerance_secs: 1.0,
            apply_rotation: true,
        };
        let (_actual, frame) = decode_frame_at(&info.path, &req).ok()?;
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

/// Encode an RGBA composite as a base64 PNG `data:` URL.
fn encode_png_data_url(frame: &DecodedFrame) -> Result<String, String> {
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
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:image/png;base64,{b64}"))
}

/// `composite_frame`: render the timeline at `frame` to a PNG data URL.
///
/// `max_size` caps the longest side (px); omit it for the default preview cap.
/// Out-of-range frames (and an empty timeline) composite to opaque black, which
/// is the correct clear color — not an error.
#[tauri::command]
pub fn composite_frame(
    core: State<'_, AppCore>,
    render: State<'_, RenderState>,
    frame: i32,
    max_size: Option<u32>,
) -> Result<CompositeFrameDto, String> {
    // Snapshot the session under its own lock, released before any GPU work.
    let timeline = core.get_timeline().timeline;
    let manifest = core.media();

    // Project the manifest into render-side lookups (external assets only; a
    // project-relative asset needs the bundle base, not produced by importing).
    let mut sizes: HashMap<String, (u32, u32)> = HashMap::new();
    let mut media: HashMap<String, MediaInfo> = HashMap::new();
    for entry in &manifest.entries {
        let path = match &entry.source {
            MediaSource::External { absolute_path } => PathBuf::from(absolute_path),
            MediaSource::Project { .. } => continue,
        };
        if let (Some(w), Some(h)) = (entry.source_width, entry.source_height) {
            if w > 0 && h > 0 {
                sizes.insert(entry.id.clone(), (w as u32, h as u32));
            }
        }
        media.insert(
            entry.id.clone(),
            MediaInfo {
                path,
                fps: entry.source_fps.unwrap_or(0.0),
            },
        );
    }

    let render_size = preview_render_size(
        timeline.width,
        timeline.height,
        max_size.unwrap_or(DEFAULT_PREVIEW_CAP),
    );

    let metrics = ManifestMetrics { sizes };
    let plan = build_render_plan(&timeline, render_size, &metrics);
    let frame_plan = plan.frame(&timeline, frame);

    // Acquire (or reuse) the GPU context, then composite + read back. The lock is
    // held across the render so the `Rc`-based texture cache never crosses threads.
    let mut guard = render
        .ctx
        .lock()
        .map_err(|_| "render state lock poisoned".to_string())?;
    if guard.is_none() {
        let dev = RenderDevice::try_new().map_err(|e| format!("no GPU device: {e}"))?;
        let compositor = Compositor::new(&dev.device);
        *guard = Some(GpuContext {
            device: dev.device,
            queue: dev.queue,
            compositor,
        });
    }
    let ctx = guard.as_ref().expect("ctx set above");

    let mut resolver = MediaResolver {
        device: &ctx.device,
        queue: &ctx.queue,
        cache: TextureCache::new(TEXTURE_CACHE_CAP),
        media: &media,
        preview_box: (render_size.width, render_size.height),
    };
    let composite = ctx
        .compositor
        .render_to_rgba(
            &ctx.device,
            &ctx.queue,
            render_size,
            &frame_plan,
            &mut resolver,
        )
        .map_err(|e| format!("composite render failed: {e}"))?;

    let data_url = encode_png_data_url(&composite)?;
    Ok(CompositeFrameDto {
        width: composite.width,
        height: composite.height,
        data_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
