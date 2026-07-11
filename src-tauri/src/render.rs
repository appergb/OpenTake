//! Timeline composite-frame rendering for the preview (#47-A).
//!
//! Wires the ready-made wgpu compositor (`opentake-render`) to the live editing
//! session: build a `RenderPlan` from the current `Timeline`, evaluate one frame
//! into an ordered draw list, resolve each layer's pixels through ffmpeg decode
//! (`opentake-media`), composite on the GPU, read back, and return the frame as a
//! base64 PNG data URL the WebView paints onto a `<canvas>` (replacing the black
//! placeholder shown on the Timeline tab).
//!
//! Scope: **video + image + text** layers. Text clips rasterize through
//! `CosmicTextRasterizer` (cosmic-text glyph layout + swash raster) to a
//! premultiplied-RGBA box texture composited last, like upstream's `CATextLayer`
//! (#65). **Lottie** layers are still skipped (the resolver returns `None`, so
//! the compositor omits them) until the bake path is wired (#65 follow-up).
//!
//! The GPU device + compositor are acquired once and cached in Tauri managed
//! state ([`RenderState`]); only the per-frame texture cache is short-lived. A
//! single `Mutex` serializes composites, which is what we want for the preview
//! (one frame at a time, no GPU contention). The continuous playback engine
//! (#53) will move this onto a dedicated render thread.

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use base64::Engine as _;
use serde::Serialize;
use tauri::State;

use opentake_core::{AppCore, EditCommand};
use opentake_domain::{ClipType, MediaSource, TextStyle, Timeline};
use opentake_media::{decode_frame_at, FrameRequest};
use opentake_ops::command::RenameEntry;
use opentake_render::gpu::texture::upload_rgba;
use opentake_render::wgpu;
use opentake_render::{
    build_render_plan, even, Compositor, CosmicTextRasterizer, DecodedFrame, GpuTexture,
    RenderDevice, RenderSize, SourceMetrics, TextRasterRequest, TextRasterizer, TextureCache,
    TextureResolver, TextureSource,
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
    /// Text rasterizer (system fonts discovered once on first composite).
    text_rasterizer: CosmicTextRasterizer,
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
}

/// A text clip projected from the timeline, keyed by clip id. The box's width /
/// height drive the rasterized texture size; position is carried by the layer
/// affine (so x/y are kept only for completeness).
struct TextInfo {
    content: String,
    style: TextStyle,
    box_norm: (f64, f64, f64, f64),
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
    timeline_fps: i32,
    /// Text clips by id (content + style + box) for on-demand rasterization.
    text: &'d HashMap<String, TextInfo>,
    /// cosmic-text rasterizer (system fonts) for text layers.
    text_rasterizer: &'d CosmicTextRasterizer,
    /// Downscale box for decoded source frames (matches the preview render size).
    preview_box: (u32, u32),
}

impl MediaResolver<'_> {
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
        let info = self.text.get(clip_id)?;
        let req = TextRasterRequest {
            clip_id,
            content: &info.content,
            style: &info.style,
            box_norm: info.box_norm,
            canvas: self.preview_box,
        };
        let frame = self.text_rasterizer.rasterize(&req)?;
        let tex = upload_rgba(self.device, self.queue, &frame, false, Some("preview-text"));
        Some(self.cache.insert(key, tex))
    }
}

impl TextureResolver for MediaResolver<'_> {
    fn resolve(&mut self, source: &TextureSource, source_frame: i64) -> Option<Rc<GpuTexture>> {
        // Map the source to (asset id, cache key). Video keys per frame; images
        // key once. Text rasterizes its box; Lottie is not supported yet.
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
            // A wide seek tolerance makes ffmpeg decode far more than the target
            // frame per call (the dominant per-frame CPU/RSS cost during scrub).
            // 0.1s lands on a nearby keyframe with ~10x less waste; the streaming
            // playback engine (#53) replaces this seek-per-frame path entirely.
            max_size: self.preview_box,
            tolerance_secs: 0.1,
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

/// Composite the timeline at `frame` into an RGBA frame at a size capped by
/// `max_size` (longest side). Shared by [`composite_frame`] (which PNG-encodes it
/// for the preview) and [`capture_frame_to_media`] (which writes it to disk and
/// imports it as a still). Out-of-range frames / an empty timeline composite to
/// opaque black — the correct clear color, not an error.
fn composite_rgba_for_snapshot(
    timeline: &Timeline,
    manifest: &opentake_domain::MediaManifest,
    project_dir: &Option<PathBuf>,
    render: &RenderState,
    frame: i32,
    max_size: u32,
) -> Result<DecodedFrame, String> {
    // Project text clips (content + style + box) so the resolver can rasterize
    // them on demand. Keyed by clip id, matching `TextureSource::Text { clip_id }`.
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

    // Project the manifest into render-side lookups.
    let mut sizes: HashMap<String, (u32, u32)> = HashMap::new();
    let mut media: HashMap<String, MediaInfo> = HashMap::new();
    for entry in &manifest.entries {
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
        media.insert(entry.id.clone(), MediaInfo { path });
    }

    let render_size = preview_render_size(timeline.width, timeline.height, max_size);

    let metrics = ManifestMetrics { sizes };
    let plan = build_render_plan(timeline, render_size, &metrics);
    let frame_plan = plan.frame(timeline, frame);

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
        });
    }
    let ctx = guard.as_ref().expect("ctx set above");

    let mut resolver = MediaResolver {
        device: &ctx.device,
        queue: &ctx.queue,
        cache: TextureCache::new(TEXTURE_CACHE_CAP),
        media: &media,
        timeline_fps: plan.fps,
        text: &text,
        text_rasterizer: &ctx.text_rasterizer,
        preview_box: (render_size.width, render_size.height),
    };
    ctx.compositor
        .render_to_rgba(
            &ctx.device,
            &ctx.queue,
            render_size,
            &frame_plan,
            &mut resolver,
        )
        .map_err(|e| format!("composite render failed: {e}"))
}

fn composite_rgba(
    core: &AppCore,
    render: &RenderState,
    frame: i32,
    max_size: u32,
) -> Result<DecodedFrame, String> {
    let timeline = core.get_timeline().timeline;
    let manifest = core.media();
    let project_dir = core.project_dir();
    composite_rgba_for_snapshot(&timeline, &manifest, &project_dir, render, frame, max_size)
}

/// `composite_frame`: render the timeline at `frame` to a PNG data URL.
///
/// `max_size` caps the longest side (px); omit it for the default preview cap.
#[tauri::command]
pub fn composite_frame(
    core: State<'_, AppCore>,
    render: State<'_, RenderState>,
    frame: i32,
    max_size: Option<u32>,
) -> Result<CompositeFrameDto, String> {
    let composite = composite_rgba(
        &core,
        &render,
        frame,
        max_size.unwrap_or(DEFAULT_PREVIEW_CAP),
    )?;
    let data_url = encode_png_data_url(&composite)?;
    Ok(CompositeFrameDto {
        width: composite.width,
        height: composite.height,
        data_url,
    })
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
fn ensure_capture_frame_mutable(core: &AppCore) -> Result<(), String> {
    core.ensure_project_mutable().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn capture_frame_to_media(
    core: State<'_, AppCore>,
    render: State<'_, RenderState>,
    media: State<'_, crate::media::MediaState>,
    frame: i32,
    name_base: String,
    folder_id: Option<String>,
    source_media_id: Option<String>,
) -> Result<crate::media::MediaListDto, String> {
    ensure_capture_frame_mutable(&core)?;
    let engine = media.engine();

    // Frame → RGBA. Timeline tab composites; video tab decodes the source frame.
    let composite = match &source_media_id {
        None => composite_rgba(&core, &render, frame, 0)?,
        Some(id) => decode_source_frame(&core, id, frame)?,
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
    let entry = crate::media::import_one(&core, engine, &png_path)
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
        &core,
        Some(engine.cache_root()),
    ))
}

/// Decode a single VIDEO asset's own frame at project-frame `frame` into a
/// full-resolution RGBA frame (video-tab capture; upstream's `videoComposition =
/// nil` raw-asset path). The frame → time uses the TIMELINE fps, matching
/// upstream's `CMTime(value: frame, timescale: fps)` (fps = timeline fps for both
/// tabs). Errors when the asset is unknown, not a video, or its source is offline.
fn decode_source_frame(core: &AppCore, media_id: &str, frame: i32) -> Result<DecodedFrame, String> {
    let timeline = core.get_timeline().timeline;
    let manifest = core.media();
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
        MediaSource::Project { relative_path } => core
            .project_dir()
            .map(|base| base.join(relative_path))
            .ok_or_else(|| "project not saved; cannot resolve media path".to_string())?,
    };
    if !path.is_file() {
        return Err(format!("source file not found: {}", path.display()));
    }
    let fps = if timeline.fps > 0 { timeline.fps } else { 30 };
    let req = FrameRequest {
        time_secs: (frame.max(0) as f64) / fps as f64,
        max_size: (0, 0), // full resolution
        ..FrameRequest::default()
    };
    let (_, rgba) =
        decode_frame_at(&path, &req).map_err(|e| format!("decode source frame: {e}"))?;
    Ok(DecodedFrame::new(rgba.width, rgba.height, rgba.rgba, false))
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

fn ensure_capture_freeze_mutable(core: &AppCore) -> Result<(), String> {
    core.ensure_project_mutable().map_err(|e| e.to_string())
}

pub fn capture_freeze_frame(
    core: &AppCore,
    render: &RenderState,
    media: &crate::media::MediaState,
    clip_id: &str,
    at_frame: i32,
) -> Result<String, String> {
    ensure_capture_freeze_mutable(core)?;
    let engine = media.engine();
    let timeline = core.get_timeline().timeline;
    let manifest = core.media();
    let project_dir = core.project_dir();
    let (solo_timeline, solo_manifest) =
        build_freeze_capture_snapshot(&timeline, &manifest, clip_id)?;
    let composite = composite_rgba_for_snapshot(
        &solo_timeline,
        &solo_manifest,
        &project_dir,
        render,
        at_frame,
        0,
    )?;
    let captures_dir = engine.cache_root().join("captures");
    std::fs::create_dir_all(&captures_dir).map_err(|e| format!("create captures dir: {e}"))?;
    let png_path = freeze_capture_png_path(&captures_dir, clip_id, at_frame);
    let bytes = encode_png_bytes(&composite)?;
    std::fs::write(&png_path, &bytes).map_err(|e| format!("write freeze png: {e}"))?;
    let entry = crate::media::import_one(core, engine, &png_path)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "freeze frame import failed".to_string())?;
    Ok(entry.id)
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

        ensure_capture_frame_mutable(&core).expect_err("capture frame must be rejected");

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

        ensure_capture_freeze_mutable(&core).expect_err("freeze frame must be rejected");

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
