//! Full-timeline video export (`export_video`).
//!
//! This is the export counterpart to the single-frame preview path
//! ([`crate::render::composite_frame`]): it walks **every** frame of the current
//! timeline, composites each on the GPU through the ready-made wgpu compositor
//! (`opentake-render`), and pipes the RGBA frames into the system ffmpeg encoder
//! (`opentake_media::VideoEncoder`) to produce a real `.mp4` on disk.
//!
//! Scope of this first cut (SPEC §2.4 / §8.2):
//! - **H.264 / .mp4**, **H.265 / .mp4**, and **ProRes 422 / .mov** are wired.
//! - **Linear audio mixdown**: every audio-bearing clip's source window is
//!   decoded to mono f32 at the mix rate, placed at its frame-derived sample
//!   offset, scaled by its `volume_at` envelope, summed, hard-limited, and mux'd
//!   in by the encoder (`-c:v copy` + AAC). A timeline with no audio still
//!   produces the same video-only file as before.
//! - Export renders at the **full** export resolution
//!   ([`opentake_render::export_render_size`]), not the preview cap.
//! - **Progress + cancel** (mirrors upstream `Export/ExportService.swift`'s
//!   200ms `AVAssetExportSession.progress` poll + cooperative cancel): the frame
//!   loop emits a throttled `"export://progress"` Tauri event and checks a
//!   shared [`ExportControl`] flag every frame. A mid-export cancel stops the
//!   loop, best-effort-deletes the partial output file, and returns
//!   `Err(CANCELLED_SENTINEL)` — a stable string the front end matches to show a
//!   neutral "cancelled" state instead of the failure toast.
//!
//! The manifest/text projection, [`opentake_render::SourceMetrics`] adapter, and
//! the on-demand ffmpeg [`opentake_render::TextureResolver`] are intentionally a
//! self-contained copy of the preview path's logic (kept in this module so the
//! preview path in `render.rs` is not touched). A later refactor can hoist the
//! shared projection into a `pub(crate)` helper once both paths are stable.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use opentake_core::AppCore;
use opentake_domain::{Clip, ClipType, MediaSource, TextStyle};
use opentake_media::encode::{mix, ClipAudio, MIX_SAMPLE_RATE};
use opentake_media::{
    decode_frame_at, extract_pcm, ExportPreset, ExportResolution as EncodeResolution, FrameRequest,
    PcmBuffer, PcmFormat, PcmSpec, RgbaFrame, VideoCodec, VideoEncoder,
};
use opentake_render::gpu::texture::upload_rgba;
use opentake_render::{
    build_render_plan, export_render_size, Compositor, CosmicTextRasterizer, DecodedFrame,
    ExportResolution as RenderResolution, GpuTexture, RenderDevice, SourceMetrics,
    TextRasterRequest, TextRasterizer, TextureCache, TextureResolver, TextureSource,
};

/// Per-frame texture cache size. Export advances monotonically, so video-frame
/// hit rate is low; a small cache still helps text/image layers re-used across
/// frames. Bounds VRAM during the export loop.
const TEXTURE_CACHE_CAP: usize = 64;

/// Requested output codec, projected from the front-end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportCodec {
    /// H.264 / `.mp4`.
    #[default]
    H264,
    /// H.265 / `.mp4`.
    H265,
    /// Apple ProRes 422 / `.mov`.
    Prores,
}

/// Requested output short-edge resolution, projected from the front-end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportQuality {
    #[serde(rename = "720p")]
    P720,
    #[default]
    #[serde(rename = "1080p")]
    P1080,
    #[serde(rename = "4k")]
    P4k,
}

impl ExportQuality {
    /// The render-crate resolution selector (drives `export_render_size`).
    fn render_resolution(self) -> RenderResolution {
        match self {
            ExportQuality::P720 => RenderResolution::R720p,
            ExportQuality::P1080 => RenderResolution::R1080p,
            ExportQuality::P4k => RenderResolution::R4k,
        }
    }

    /// The encoder-crate resolution selector (carried into the `ExportPreset`).
    fn encode_resolution(self) -> EncodeResolution {
        match self {
            ExportQuality::P720 => EncodeResolution::P720,
            ExportQuality::P1080 => EncodeResolution::P1080,
            ExportQuality::P4k => EncodeResolution::P2160,
        }
    }
}

/// Parameters for an export, projected from the front-end. `#[serde(default)]`
/// on the optional knobs keeps older callers (and partial payloads) working: a
/// bare `{ "outPath": "..." }` exports H.264 / 1080p.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    /// Absolute path to write the encoded video to. Must end in `.mp4` for the
    /// H.264 path.
    pub out_path: String,
    #[serde(default)]
    pub codec: ExportCodec,
    #[serde(default)]
    pub quality: ExportQuality,
}

/// Summary of a completed export, returned to the front-end.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    /// Absolute path the video was written to.
    pub out_path: String,
    /// Encoded width in pixels (even-ized export render size).
    pub width: u32,
    /// Encoded height in pixels.
    pub height: u32,
    /// Frames-per-second of the output (from the render plan).
    pub fps: i32,
    /// Number of frames written.
    pub frame_count: i32,
}

/// Stable `Err` string [`export_video`] returns when the frame loop stops
/// because [`ExportControl::is_cancelled`] flipped mid-encode. The front end
/// matches this exact string to show a neutral "cancelled" toast instead of the
/// failure path — chosen over a `cancelled: bool` field on [`ExportSummary`]
/// because the loop already threads through `Result<_, String>` at every
/// composite/encode step, so reusing that channel is the lower-churn option.
pub const CANCELLED_SENTINEL: &str = "export cancelled";

/// Shared cancel flag for the in-flight export, managed as Tauri app state
/// (`app.manage(ExportControl::default())`). One export runs at a time in this
/// cut, so a single flag (rather than a per-export token) is sufficient: the
/// command handler resets it to `false` at the start of every `export_video`
/// call, and the frame loop polls it (`Ordering::Relaxed` — a plain progress
/// signal, not synchronizing any other memory) once per frame.
#[derive(Default)]
pub struct ExportControl {
    cancel: Arc<AtomicBool>,
}

impl ExportControl {
    /// Arm for a new export: clears any stale cancel request from a previous run.
    fn reset(&self) {
        self.cancel.store(false, Ordering::Relaxed);
    }

    /// Request cancellation of the in-flight export.
    fn request_cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// True once [`ExportControl::request_cancel`] has been called since the
    /// last [`ExportControl::reset`].
    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// `cancel_export`: request that the in-flight export (if any) stop at the next
/// frame boundary. A no-op when nothing is exporting — the flag is simply
/// cleared again at the start of the next `export_video` call.
#[tauri::command]
pub fn cancel_export(control: State<'_, ExportControl>) {
    control.request_cancel();
}

/// Progress payload for the throttled `"export://progress"` event: `done` of
/// `total` frames composited so far.
#[derive(Clone, Debug, Serialize, PartialEq)]
struct ExportProgress {
    done: i32,
    total: i32,
}

/// Minimum spacing between progress emissions, matching upstream's 200ms
/// `AVAssetExportSession.progress` poll interval.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

/// True when at least [`PROGRESS_INTERVAL`] has elapsed since the last emit —
/// the throttle the frame loop consults before firing another progress event.
/// Pure/pulled out of the loop so it's unit-testable without a GPU.
fn progress_should_emit(last: Instant, now: Instant) -> bool {
    now.saturating_duration_since(last) >= PROGRESS_INTERVAL
}

/// Resolve the requested codec to an ffmpeg [`ExportPreset`], validating that
/// the output extension matches the codec's container.
fn resolve_preset(
    codec: ExportCodec,
    quality: ExportQuality,
    out: &Path,
) -> Result<ExportPreset, String> {
    let ext = out
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match codec {
        ExportCodec::H264 => {
            if ext.as_deref() != Some("mp4") {
                return Err("H.264 export requires an .mp4 output path".to_string());
            }
            Ok(ExportPreset::new(
                VideoCodec::H264,
                quality.encode_resolution(),
            ))
        }
        ExportCodec::H265 => {
            if ext.as_deref() != Some("mp4") {
                return Err("H.265 export requires an .mp4 output path".to_string());
            }
            Ok(ExportPreset::new(
                VideoCodec::H265,
                quality.encode_resolution(),
            ))
        }
        ExportCodec::Prores => {
            if ext.as_deref() != Some("mov") {
                return Err("ProRes export requires a .mov output path".to_string());
            }
            Ok(ExportPreset::new(
                VideoCodec::ProRes422,
                quality.encode_resolution(),
            ))
        }
    }
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

/// `SourceMetrics` backed by the media manifest (intrinsic size only; ffmpeg
/// auto-rotates on decode in this cut).
struct ManifestMetrics {
    sizes: HashMap<String, (u32, u32)>,
}

impl SourceMetrics for ManifestMetrics {
    fn natural_size(&self, media_ref: &str) -> Option<(u32, u32)> {
        self.sizes.get(media_ref).copied()
    }
}

/// `TextureResolver` that decodes a layer's pixels on demand via ffmpeg and
/// uploads them to the GPU. Video keys per source-frame; images key once; text
/// rasterizes its box; Lottie returns `None` (skipped) in this cut. Mirrors the
/// preview resolver, but the decode box is the full export render size.
struct MediaResolver<'d> {
    device: &'d opentake_render::wgpu::Device,
    queue: &'d opentake_render::wgpu::Queue,
    cache: TextureCache,
    media: &'d HashMap<String, MediaInfo>,
    timeline_fps: i32,
    text: &'d HashMap<String, TextInfo>,
    text_rasterizer: &'d CosmicTextRasterizer,
    /// Decode/raster box for source frames (matches the export render size).
    render_box: (u32, u32),
}

impl MediaResolver<'_> {
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
        let tex = upload_rgba(self.device, self.queue, &frame, false, Some("export-text"));
        Some(self.cache.insert(key, tex))
    }
}

impl TextureResolver for MediaResolver<'_> {
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
            // Export advances frame-by-frame; a tight tolerance keeps each
            // composited frame on the exact target time (quality over the
            // scrub-oriented wide tolerance the preview uses).
            tolerance_secs: 0.0,
            apply_rotation: true,
        };
        let (_actual, frame) = decode_frame_at(&info.path, &req).ok()?;
        let decoded = DecodedFrame::new(frame.width, frame.height, frame.rgba, false);
        let tex = upload_rgba(self.device, self.queue, &decoded, false, Some("export-src"));
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

/// PCM spec the export decodes every audio source window into: mono f32 at the
/// shared mix sample rate. Decoding at the mix rate up front makes the mixdown a
/// plain sample-aligned add (no per-clip resampling in this cut).
const AUDIO_DECODE_SPEC: PcmSpec = PcmSpec {
    sample_rate: MIX_SAMPLE_RATE,
    channels: 1,
    format: PcmFormat::F32,
};

/// Project one audio clip into a [`ClipAudio`] for the mixdown: decode its
/// visible source window, place it at its frame-derived sample offset, and build
/// the per-sample `volume_at` gain envelope.
///
/// Returns `Ok(None)` when the clip contributes no audio (no media path, no
/// audio track, zero-length window, or a fully-decoded-to-empty buffer). Decode
/// failures other than "no audio track" propagate as `Err`.
fn project_clip_audio(
    clip: &Clip,
    media: &HashMap<String, MediaInfo>,
    timeline_fps: i32,
) -> Result<Option<ClipAudio>, String> {
    if clip.duration_frames <= 0 || timeline_fps <= 0 {
        return Ok(None);
    }
    let Some(info) = media.get(&clip.media_ref) else {
        return Ok(None);
    };

    let Some((lo, hi)) = clip_source_window_secs(clip, timeline_fps) else {
        return Ok(None);
    };

    let pcm = match extract_pcm(&info.path, &AUDIO_DECODE_SPEC, Some((lo, hi))) {
        Ok(p) => p,
        // A clip pointing at a video with no audio track simply contributes
        // silence — not an export failure.
        Err(opentake_media::MediaError::NoTrack(_, _)) => return Ok(None),
        Err(e) => return Err(format!("audio decode failed for {}: {e}", clip.media_ref)),
    };
    if pcm.samples_f32.is_empty() {
        return Ok(None);
    }

    // Placement: the clip's timeline start frame, in mix samples.
    let start_sample = ((clip.start_frame.max(0) as f64) / timeline_fps as f64
        * MIX_SAMPLE_RATE as f64)
        .round() as usize;

    // Per-sample gain from `volume_at`, sampled at the timeline frame each mix
    // sample falls on. Unity throughout collapses to an empty envelope.
    let samples_per_frame = MIX_SAMPLE_RATE as f64 / timeline_fps as f64;
    let mut gains = Vec::with_capacity(pcm.samples_f32.len());
    let mut all_unity = true;
    for k in 0..pcm.samples_f32.len() {
        let tl_frame = clip.start_frame + (k as f64 / samples_per_frame).floor() as i32;
        let g = clip.volume_at(tl_frame) as f32;
        if (g - 1.0).abs() > f32::EPSILON {
            all_unity = false;
        }
        gains.push(g);
    }

    Ok(Some(ClipAudio {
        start_sample,
        samples: pcm.samples_f32,
        gains: if all_unity { Vec::new() } else { gains },
    }))
}

/// Decode + mix every audio-bearing clip on the timeline into one mono buffer.
///
/// Walks audio and video clips (video clips can carry an audio track), projects
/// each through [`project_clip_audio`], and linearly mixes the lot. Returns
/// `None` when nothing contributes audio (→ the caller keeps the video-only
/// output). Errors surface decode/mix failures to the front-end.
fn mix_timeline_audio(
    timeline: &opentake_domain::Timeline,
    media: &HashMap<String, MediaInfo>,
) -> Result<Option<PcmBuffer>, String> {
    let mut clips_audio: Vec<ClipAudio> = Vec::new();
    for track in &timeline.tracks {
        if track.muted {
            continue;
        }
        for clip in &track.clips {
            // Only audio and video clips carry sound; text/image/lottie don't.
            if clip.media_type != ClipType::Audio && clip.media_type != ClipType::Video {
                continue;
            }
            if let Some(ca) = project_clip_audio(clip, media, timeline.fps)? {
                clips_audio.push(ca);
            }
        }
    }
    if clips_audio.is_empty() {
        return Ok(None);
    }
    let mixed = mix::mix_clips(&clips_audio).map_err(|e| format!("audio mix failed: {e}"))?;
    if mixed.is_empty() {
        return Ok(None);
    }
    Ok(Some(PcmBuffer {
        spec: AUDIO_DECODE_SPEC,
        samples_f32: mixed,
    }))
}

/// `export_video`: render the whole timeline to a video file on disk.
///
/// Composites every frame at the full export resolution and encodes them to
/// `req.out_path` per the requested codec/container. An empty timeline still
/// produces a valid (possibly zero-frame) file — out-of-range frames composite
/// to opaque black, which is the correct clear color, not an error.
///
/// Emits throttled `"export://progress"` events via `app` and polls `control`
/// for a mid-encode cancel every frame (see the module doc). This is a sync
/// (non-`async`) command, so Tauri runs it on a worker thread — `cancel_export`
/// (and the WebView's event loop delivering `"export://progress"`) keep running
/// concurrently while this call is in flight.
///
/// GPU acquisition / decode / encode failures surface to the front-end as
/// `Err(String)` (the Tauri boundary contract); a mid-export cancel surfaces as
/// `Err(`[`CANCELLED_SENTINEL`]`)`.
#[tauri::command]
pub fn export_video(
    app: AppHandle,
    core: State<'_, AppCore>,
    control: State<'_, ExportControl>,
    req: ExportRequest,
) -> Result<ExportSummary, String> {
    // Snapshot the session up front; no session lock is held during GPU/encode.
    let timeline = core.get_timeline().timeline;
    let manifest = core.media();
    let project_dir = core.project_dir();
    control.reset();
    let on_progress = |done: i32, total: i32| {
        let _ = app.emit("export://progress", ExportProgress { done, total });
    };
    run_export_with_control(
        &timeline,
        &manifest,
        &project_dir,
        &req,
        Some(&control),
        Some(&on_progress),
    )
}

/// The export orchestration, decoupled from Tauri/`AppCore` so it can be driven
/// directly by an ffmpeg-gated integration test with a hand-built timeline +
/// manifest. The command wrapper only snapshots the live session and delegates
/// here. `pub` for the integration test in `tests/export_integration.rs`. No
/// cancel/progress wiring — the integration test doesn't need either, so this
/// keeps its existing 4-argument signature and delegates to
/// [`run_export_with_control`] with both plumbed as absent.
pub fn run_export(
    timeline: &opentake_domain::Timeline,
    manifest: &opentake_domain::MediaManifest,
    project_dir: &Option<PathBuf>,
    req: &ExportRequest,
) -> Result<ExportSummary, String> {
    run_export_with_control(timeline, manifest, project_dir, req, None, None)
}

/// Shared orchestration behind [`run_export`] and [`export_video`]: `control`
/// (checked once per frame) and `on_progress` (called at most every
/// [`PROGRESS_INTERVAL`], plus once more at 100% when the loop finishes) are
/// both optional so callers with no Tauri context (the integration test) can
/// omit them.
fn run_export_with_control(
    timeline: &opentake_domain::Timeline,
    manifest: &opentake_domain::MediaManifest,
    project_dir: &Option<PathBuf>,
    req: &ExportRequest,
    control: Option<&ExportControl>,
    on_progress: Option<&dyn Fn(i32, i32)>,
) -> Result<ExportSummary, String> {
    let out_path = PathBuf::from(&req.out_path);
    let preset = resolve_preset(req.codec, req.quality, &out_path)?;

    let text = project_text(timeline);
    let (sizes, media) = project_media(manifest, project_dir);

    let render_size = export_render_size(
        (timeline.width, timeline.height),
        req.quality.render_resolution(),
    );

    let metrics = ManifestMetrics { sizes };
    let plan = build_render_plan(timeline, render_size, &metrics);

    // Acquire the GPU device + compositor for this export. Unlike the preview
    // (which caches the context in Tauri state for repeated scrubs), an export is
    // a one-shot batch, so a local context is simplest and avoids contending with
    // the preview's lock.
    let dev = RenderDevice::try_new().map_err(|e| format!("no GPU device: {e}"))?;
    let compositor = Compositor::new(&dev.device);
    let text_rasterizer = CosmicTextRasterizer::new();

    let mut encoder = VideoEncoder::new(
        &out_path,
        render_size.width,
        render_size.height,
        plan.fps,
        &preset,
    )
    .map_err(|e| format!("encoder init failed: {e}"))?;

    let mut last_progress_emit = Instant::now();
    for f in 0..plan.total_frames {
        if control.is_some_and(|c| c.is_cancelled()) {
            // `abort` kills + waits on the ffmpeg child (unlike a plain `drop`,
            // which would orphan the process and race the file removal below).
            encoder.abort();
            // Best-effort cleanup of the partial file — a leftover half-encoded
            // video must not look like a finished export. Missing/unwritable is
            // not itself an error worth surfacing over the cancel.
            let _ = std::fs::remove_file(&out_path);
            return Err(CANCELLED_SENTINEL.to_string());
        }

        let frame_plan = plan.frame(timeline, f);
        let mut resolver = MediaResolver {
            device: &dev.device,
            queue: &dev.queue,
            cache: TextureCache::new(TEXTURE_CACHE_CAP),
            media: &media,
            timeline_fps: plan.fps,
            text: &text,
            text_rasterizer: &text_rasterizer,
            render_box: (render_size.width, render_size.height),
        };
        let composite = compositor
            .render_to_rgba(
                &dev.device,
                &dev.queue,
                render_size,
                &frame_plan,
                &mut resolver,
            )
            .map_err(|e| format!("composite render failed at frame {f}: {e}"))?;
        encoder
            .push_frame(&RgbaFrame::new(
                composite.width,
                composite.height,
                composite.rgba,
            ))
            .map_err(|e| format!("encode frame {f} failed: {e}"))?;

        if let Some(emit) = on_progress {
            let now = Instant::now();
            let done = f + 1;
            let is_last = done == plan.total_frames;
            if is_last || progress_should_emit(last_progress_emit, now) {
                emit(done, plan.total_frames);
                last_progress_emit = now;
            }
        }
    }

    // Decode + linearly mix every audio-bearing clip, then hand the mixed PCM to
    // the encoder so `finish` mux's it into the container. No audio → video-only.
    if let Some(pcm) = mix_timeline_audio(timeline, &media)? {
        encoder.push_audio(pcm);
    }

    encoder
        .finish()
        .map_err(|e| format!("encoder finish failed: {e}"))?;

    Ok(ExportSummary {
        out_path: req.out_path.clone(),
        width: render_size.width,
        height: render_size.height,
        fps: plan.fps,
        frame_count: plan.total_frames,
    })
}

fn project_frame_time_secs(source_frame: i64, timeline_fps: i32) -> f64 {
    let fps = if timeline_fps > 0 {
        timeline_fps as f64
    } else {
        30.0
    };
    (source_frame.max(0) as f64) / fps
}

fn clip_source_window_secs(clip: &Clip, timeline_fps: i32) -> Option<(f64, f64)> {
    if clip.duration_frames <= 0 || timeline_fps <= 0 {
        return None;
    }
    let fps = timeline_fps as f64;
    let lo = clip.trim_start_frame.max(0) as f64 / fps;
    let consumed = clip.source_frames_consumed().max(0);
    if consumed == 0 {
        return None;
    }
    Some((lo, lo + consumed as f64 / fps))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn export_control_starts_uncancelled() {
        let control = ExportControl::default();
        assert!(!control.is_cancelled());
    }

    #[test]
    fn export_control_request_cancel_flips_the_flag() {
        let control = ExportControl::default();
        control.request_cancel();
        assert!(control.is_cancelled());
    }

    #[test]
    fn export_control_reset_clears_a_prior_cancel() {
        // Mirrors `export_video`'s "reset at the start of every call" — a stale
        // cancel from a finished export must not poison the next one.
        let control = ExportControl::default();
        control.request_cancel();
        control.reset();
        assert!(!control.is_cancelled());
    }

    #[test]
    fn export_control_cancel_is_observable_through_a_clone() {
        // `cancel_export` (a separate Tauri command) sets the flag on its own
        // `State<ExportControl>` handle; a clone shares the same underlying
        // `Arc<AtomicBool>`, matching how Tauri hands out the same managed
        // instance to every command.
        let control = ExportControl::default();
        let clone = ExportControl {
            cancel: control.cancel.clone(),
        };
        clone.request_cancel();
        assert!(control.is_cancelled());
    }

    #[test]
    fn progress_should_emit_false_before_the_interval_elapses() {
        let last = Instant::now();
        let now = last + Duration::from_millis(50);
        assert!(!progress_should_emit(last, now));
    }

    #[test]
    fn progress_should_emit_true_once_the_interval_elapses() {
        let last = Instant::now();
        let now = last + PROGRESS_INTERVAL;
        assert!(progress_should_emit(last, now));
    }

    #[test]
    fn progress_should_emit_true_well_past_the_interval() {
        let last = Instant::now();
        let now = last + Duration::from_secs(1);
        assert!(progress_should_emit(last, now));
    }

    #[test]
    fn quality_maps_to_both_resolution_selectors() {
        assert_eq!(
            ExportQuality::P720.render_resolution(),
            RenderResolution::R720p
        );
        assert_eq!(
            ExportQuality::P720.encode_resolution(),
            EncodeResolution::P720
        );
        assert_eq!(
            ExportQuality::P1080.render_resolution(),
            RenderResolution::R1080p
        );
        assert_eq!(
            ExportQuality::P1080.encode_resolution(),
            EncodeResolution::P1080
        );
        assert_eq!(
            ExportQuality::P4k.render_resolution(),
            RenderResolution::R4k
        );
        assert_eq!(
            ExportQuality::P4k.encode_resolution(),
            EncodeResolution::P2160
        );
    }

    #[test]
    fn resolve_preset_accepts_h264_mp4() {
        let preset = resolve_preset(
            ExportCodec::H264,
            ExportQuality::P1080,
            Path::new("/out.mp4"),
        )
        .expect("h264 mp4 should resolve");
        assert_eq!(preset.codec, VideoCodec::H264);
        assert_eq!(preset.resolution, EncodeResolution::P1080);
    }

    #[test]
    fn resolve_preset_rejects_wrong_extension_for_h264() {
        let err = resolve_preset(
            ExportCodec::H264,
            ExportQuality::P1080,
            Path::new("/out.mov"),
        )
        .unwrap_err();
        assert!(err.contains(".mp4"), "got: {err}");
    }

    #[test]
    fn resolve_preset_accepts_h265_mp4() {
        let preset = resolve_preset(
            ExportCodec::H265,
            ExportQuality::P1080,
            Path::new("/out.mp4"),
        )
        .expect("h265 mp4 should resolve");
        assert_eq!(preset.codec, VideoCodec::H265);
        assert_eq!(preset.resolution, EncodeResolution::P1080);
    }

    #[test]
    fn resolve_preset_rejects_wrong_extension_for_h265() {
        let err = resolve_preset(
            ExportCodec::H265,
            ExportQuality::P1080,
            Path::new("/out.mov"),
        )
        .unwrap_err();
        assert!(err.contains(".mp4"), "got: {err}");

        let err = resolve_preset(
            ExportCodec::H265,
            ExportQuality::P1080,
            Path::new("/out.png"),
        )
        .unwrap_err();
        assert!(err.contains(".mp4"), "got: {err}");
    }

    #[test]
    fn resolve_preset_accepts_prores_mov() {
        let preset = resolve_preset(
            ExportCodec::Prores,
            ExportQuality::P1080,
            Path::new("/out.mov"),
        )
        .expect("prores mov should resolve");
        assert_eq!(preset.codec, VideoCodec::ProRes422);
        assert_eq!(preset.resolution, EncodeResolution::P1080);
    }

    #[test]
    fn resolve_preset_rejects_wrong_extension_for_prores() {
        let err = resolve_preset(
            ExportCodec::Prores,
            ExportQuality::P1080,
            Path::new("/out.mp4"),
        )
        .unwrap_err();
        assert!(err.contains(".mov"), "got: {err}");
    }

    #[test]
    fn export_request_defaults_to_h264_1080p() {
        // A bare payload (only outPath) relies on #[serde(default)] for the knobs.
        let req: ExportRequest =
            serde_json::from_str(r#"{ "outPath": "/tmp/x.mp4" }"#).expect("parse");
        assert_eq!(req.codec, ExportCodec::H264);
        assert_eq!(req.quality, ExportQuality::P1080);
        assert_eq!(req.out_path, "/tmp/x.mp4");
    }

    #[test]
    fn export_quality_parses_named_variants() {
        let req: ExportRequest = serde_json::from_str(
            r#"{ "outPath": "/tmp/x.mp4", "codec": "h264", "quality": "720p" }"#,
        )
        .expect("parse");
        assert_eq!(req.quality, ExportQuality::P720);
    }

    use opentake_domain::{Timeline, Track};

    #[test]
    fn clip_source_window_uses_timeline_fps_not_media_source_fps() {
        let mut clip = Clip::new("c1", "asset-1", 0, 60);
        clip.trim_start_frame = 15;
        clip.speed = 1.0;

        let (lo, hi) = clip_source_window_secs(&clip, 30).expect("window");

        assert!((lo - 0.5).abs() < 0.0001);
        assert!((hi - 2.5).abs() < 0.0001);
    }

    #[test]
    fn project_clip_audio_skips_clip_with_no_media_entry() {
        // No matching manifest entry → no audio contribution, no decode attempt.
        let clip = Clip::new("c1", "missing-asset", 0, 30);
        let media: HashMap<String, MediaInfo> = HashMap::new();
        let got = project_clip_audio(&clip, &media, 30).expect("ok");
        assert!(got.is_none());
    }

    #[test]
    fn project_clip_audio_skips_zero_duration() {
        let clip = Clip::new("c1", "asset-1", 0, 0);
        let mut media: HashMap<String, MediaInfo> = HashMap::new();
        media.insert(
            "asset-1".into(),
            MediaInfo {
                path: PathBuf::from("/nonexistent.wav"),
            },
        );
        // duration 0 short-circuits before any decode is attempted.
        assert!(project_clip_audio(&clip, &media, 30).expect("ok").is_none());
    }

    #[test]
    fn mix_timeline_audio_none_when_only_text_clips() {
        // A text clip carries no sound; with no audio/video clips there's nothing
        // to decode, so the result is None without touching the media map.
        let mut tl = Timeline::new();
        let mut track = Track::new("t1", ClipType::Text);
        let mut clip = Clip::new("c1", "asset-1", 0, 30);
        clip.media_type = ClipType::Text;
        track.clips.push(clip);
        tl.tracks.push(track);
        let media: HashMap<String, MediaInfo> = HashMap::new();
        assert!(mix_timeline_audio(&tl, &media).expect("ok").is_none());
    }

    #[test]
    fn mix_timeline_audio_skips_muted_tracks() {
        // A muted audio track is excluded; with no other audio the result is None
        // and the (missing-path) asset is never decoded.
        let mut tl = Timeline::new();
        let mut track = Track::new("t1", ClipType::Audio);
        track.muted = true;
        let mut clip = Clip::new("c1", "asset-1", 0, 30);
        clip.media_type = ClipType::Audio;
        track.clips.push(clip);
        tl.tracks.push(track);
        let mut media: HashMap<String, MediaInfo> = HashMap::new();
        media.insert(
            "asset-1".into(),
            MediaInfo {
                path: PathBuf::from("/nonexistent.wav"),
            },
        );
        assert!(mix_timeline_audio(&tl, &media).expect("ok").is_none());
    }
}
