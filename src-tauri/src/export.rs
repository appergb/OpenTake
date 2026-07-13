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
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
#[cfg(test)]
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use same_file::Handle as FileIdentity;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use opentake_core::AppCore;
use opentake_domain::{Clip, ClipType, MediaSource, TextStyle};
use opentake_media::encode::{mix, ClipAudio, MIX_SAMPLE_RATE};
use opentake_media::{
    decode_frame_at, extract_pcm, extract_pcm_cancellable_with_progress, ExportPreset,
    ExportResolution as EncodeResolution, FrameRequest, MediaCancelToken, PcmBuffer, PcmFormat,
    PcmProgressCallback, PcmSpec, RgbaFrame, VideoCodec, VideoEncoder,
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
    /// Whether a non-empty mixed audio buffer was attached to the encoder and
    /// therefore muxed into the completed output.
    pub has_audio: bool,
}

/// Stable `Err` string [`export_video`] returns when the frame loop stops
/// because [`ExportControl::is_cancelled`] flipped mid-encode. The front end
/// matches this exact string to show a neutral "cancelled" toast instead of the
/// failure path — chosen over a `cancelled: bool` field on [`ExportSummary`]
/// because the loop already threads through `Result<_, String>` at every
/// composite/encode step, so reusing that channel is the lower-churn option.
pub const CANCELLED_SENTINEL: &str = "export cancelled";

/// Single-export lease and its cancellation generation, managed as Tauri state.
/// Claiming a lease and publishing its fresh token happen under one mutex, so a
/// concurrent cancel can only target the previous operation or the new one; it
/// can never be erased by a later reset.
#[derive(Default)]
pub struct ExportControl {
    operation: Mutex<ExportOperationState>,
}

#[derive(Default)]
struct ExportOperationState {
    next_generation: u64,
    active: Option<ActiveExport>,
}

struct ActiveExport {
    generation: u64,
    operation_id: String,
    cancel: MediaCancelToken,
}

pub(crate) struct ExportGuard<'a> {
    control: &'a ExportControl,
    generation: u64,
    operation_id: String,
    cancel: MediaCancelToken,
}

impl std::fmt::Debug for ExportGuard<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExportGuard")
            .field("generation", &self.generation)
            .field("operation_id", &self.operation_id)
            .finish_non_exhaustive()
    }
}

impl Drop for ExportGuard<'_> {
    fn drop(&mut self) {
        let mut state = self
            .control
            .operation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state
            .active
            .as_ref()
            .is_some_and(|active| active.generation == self.generation)
        {
            state.active = None;
        }
    }
}

impl ExportControl {
    /// Request cancellation only when the caller owns the active operation.
    /// A delayed cancel for a completed predecessor is an intentional no-op.
    fn request_cancel(&self, operation_id: &str) -> bool {
        let state = self
            .operation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(active) = state
            .active
            .as_ref()
            .filter(|active| active.operation_id == operation_id)
        {
            active.cancel.cancel();
            true
        } else {
            false
        }
    }

    /// True once cancellation was requested for the active generation.
    pub(crate) fn is_cancelled(&self) -> bool {
        self.operation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .active
            .as_ref()
            .is_some_and(|active| active.cancel.is_cancelled())
    }

    pub(crate) fn media_cancel_token(&self) -> MediaCancelToken {
        self.operation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .active
            .as_ref()
            .map(|active| active.cancel.clone())
            .unwrap_or_default()
    }

    pub(crate) fn try_begin(&self, operation_id: &str) -> Result<ExportGuard<'_>, String> {
        self.try_begin_with_hook(operation_id, || {})
    }

    fn try_begin_with_hook(
        &self,
        operation_id: &str,
        after_publish: impl FnOnce(),
    ) -> Result<ExportGuard<'_>, String> {
        validate_export_operation_id(operation_id)?;
        let mut state = self
            .operation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.active.is_some() {
            return Err("another export is already in progress".to_string());
        }
        let generation = state.next_generation;
        state.next_generation = state.next_generation.wrapping_add(1);
        let cancel = MediaCancelToken::new();
        state.active = Some(ActiveExport {
            generation,
            operation_id: operation_id.to_string(),
            cancel: cancel.clone(),
        });
        after_publish();
        drop(state);
        Ok(ExportGuard {
            control: self,
            generation,
            operation_id: operation_id.to_string(),
            cancel,
        })
    }
}

impl ExportGuard<'_> {
    /// Observe cancellation for this exact export generation.
    pub(crate) fn checkpoint(&self) -> Result<(), String> {
        if self.cancel.checkpoint() {
            Err(CANCELLED_SENTINEL.to_string())
        } else {
            Ok(())
        }
    }

    pub(crate) fn cancel_token(&self) -> &MediaCancelToken {
        &self.cancel
    }

    pub(crate) fn operation_id(&self) -> &str {
        &self.operation_id
    }

    /// Linearize the final save-as commit against cancellation.
    ///
    /// This runs inside the core manifest transaction's postcondition. If
    /// cancellation already won, the transaction rolls back. Otherwise the
    /// generation is removed while holding the same mutex used by
    /// `request_cancel`, so every later cancel is a no-op for this completed
    /// operation.
    pub(crate) fn commit(&mut self) -> Result<(), String> {
        let mut state = self
            .control
            .operation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let active = state
            .active
            .as_ref()
            .filter(|active| active.generation == self.generation)
            .ok_or_else(|| "export generation is no longer active".to_string())?;
        if active.cancel.is_cancelled() {
            return Err(CANCELLED_SENTINEL.to_string());
        }
        state.active = None;
        Ok(())
    }
}

fn validate_export_operation_id(operation_id: &str) -> Result<(), String> {
    if operation_id.is_empty()
        || operation_id.len() > 128
        || !operation_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err("invalid export operation id".to_string());
    }
    Ok(())
}

/// `cancel_export`: request that the in-flight export (if any) stop at its next
/// cancellation checkpoint. The request must name the operation that exposed
/// the cancel control; stale requests cannot target a successor generation.
#[tauri::command]
pub fn cancel_export(
    control: State<'_, ExportControl>,
    operation_id: String,
) -> Result<bool, String> {
    validate_export_operation_id(&operation_id)?;
    Ok(control.request_cancel(&operation_id))
}

/// Progress payload for the throttled `"export://progress"` event: `done` of
/// `total` frames composited so far.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ExportProgress {
    operation_id: String,
    done: i32,
    total: i32,
}

pub(crate) fn emit_export_progress(app: &AppHandle, operation_id: &str, done: i32, total: i32) {
    let _ = app.emit(
        "export://progress",
        ExportProgress {
            operation_id: operation_id.to_string(),
            done,
            total,
        },
    );
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

pub(crate) type AudioExportProgress = Arc<dyn Fn(i32, i32) + Send + Sync>;

pub(crate) const AUDIO_PROGRESS_TOTAL: i32 = 1_000;
const AUDIO_DECODE_END: i32 = 800;
const AUDIO_MIX_START: i32 = 850;
const AUDIO_MIX_END: i32 = 980;
const AUDIO_WAV_START: i32 = AUDIO_MIX_END;
const AUDIO_WAV_END: i32 = 990;
const AUDIO_CANCEL_CHUNK_SAMPLES: usize = 8 * 1024;
const VIDEO_RENDER_END: i32 = 550;
const VIDEO_AUDIO_END: i32 = 800;
const VIDEO_FINALIZE_END: i32 = 980;
const VIDEO_EXPORT_END: i32 = 990;

fn decode_pcm_with_export_control<F>(
    control: &ExportControl,
    path: &Path,
    range: Option<(f64, f64)>,
    progress: Option<PcmProgressCallback>,
    decode: F,
) -> opentake_media::Result<PcmBuffer>
where
    F: FnOnce(
        &Path,
        &PcmSpec,
        Option<(f64, f64)>,
        &MediaCancelToken,
        Option<PcmProgressCallback>,
    ) -> opentake_media::Result<PcmBuffer>,
{
    let cancel = control.media_cancel_token();
    decode(path, &AUDIO_DECODE_SPEC, range, &cancel, progress)
}

fn check_audio_cancel(control: &ExportControl) -> Result<(), String> {
    if control.is_cancelled() {
        Err(CANCELLED_SENTINEL.to_string())
    } else {
        Ok(())
    }
}

#[cfg(test)]
fn retime_pcm_to_len(samples: &[f32], target_len: usize) -> Vec<f32> {
    retime_pcm_to_len_with_control(samples, target_len, None)
        .expect("retime without cancellation cannot fail")
}

fn retime_pcm_to_len_with_control(
    samples: &[f32],
    target_len: usize,
    control: Option<&ExportControl>,
) -> Result<Vec<f32>, String> {
    if samples.is_empty() || target_len == 0 {
        return Ok(Vec::new());
    }

    let source_span = (samples.len() - 1) as f64;
    let target_span = target_len.saturating_sub(1) as f64;
    let mut retimed = Vec::with_capacity(target_len);
    for index in 0..target_len {
        if index.is_multiple_of(AUDIO_CANCEL_CHUNK_SAMPLES) {
            if let Some(control) = control {
                check_audio_cancel(control)?;
            }
        }
        let value = if samples.len() == 1 || target_len == 1 {
            samples[0]
        } else {
            let source = index as f64 * source_span / target_span;
            let lo = source.floor() as usize;
            let hi = source.ceil() as usize;
            let fraction = (source - lo as f64) as f32;
            samples[lo] + (samples[hi] - samples[lo]) * fraction
        };
        retimed.push(value);
    }
    Ok(retimed)
}

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
    control: Option<&ExportControl>,
    decode_progress: Option<PcmProgressCallback>,
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

    let decoded = match control {
        Some(control) => decode_pcm_with_export_control(
            control,
            &info.path,
            Some((lo, hi)),
            decode_progress,
            extract_pcm_cancellable_with_progress,
        ),
        None => extract_pcm(&info.path, &AUDIO_DECODE_SPEC, Some((lo, hi))),
    };
    let pcm = match decoded {
        Ok(p) => p,
        // A clip pointing at a video with no audio track simply contributes
        // silence — not an export failure.
        Err(opentake_media::MediaError::NoTrack(_, _)) => return Ok(None),
        Err(opentake_media::MediaError::Cancelled) => return Err(CANCELLED_SENTINEL.to_string()),
        Err(e) => return Err(format!("audio decode failed for {}: {e}", clip.media_ref)),
    };
    if let Some(control) = control {
        check_audio_cancel(control)?;
    }
    if pcm.samples_f32.is_empty() {
        return Ok(None);
    }

    let target_len = ((clip.duration_frames as f64) / timeline_fps as f64 * MIX_SAMPLE_RATE as f64)
        .round() as usize;
    let samples = retime_pcm_to_len_with_control(&pcm.samples_f32, target_len, control)?;
    if samples.is_empty() {
        return Ok(None);
    }

    // Placement: the clip's timeline start frame, in mix samples.
    let start_sample = ((clip.start_frame.max(0) as f64) / timeline_fps as f64
        * MIX_SAMPLE_RATE as f64)
        .round() as usize;

    // Per-sample gain from `volume_at`, sampled at the timeline frame each mix
    // sample falls on. Unity throughout collapses to an empty envelope.
    let samples_per_frame = MIX_SAMPLE_RATE as f64 / timeline_fps as f64;
    let mut gains = Vec::with_capacity(samples.len());
    let mut all_unity = true;
    for k in 0..samples.len() {
        if k.is_multiple_of(AUDIO_CANCEL_CHUNK_SAMPLES) {
            if let Some(control) = control {
                check_audio_cancel(control)?;
            }
        }
        let tl_frame = clip.start_frame + (k as f64 / samples_per_frame).floor() as i32;
        let g = clip.volume_at(tl_frame) as f32;
        if (g - 1.0).abs() > f32::EPSILON {
            all_unity = false;
        }
        gains.push(g);
    }

    Ok(Some(ClipAudio {
        start_sample,
        samples,
        gains: if all_unity { Vec::new() } else { gains },
    }))
}

fn mix_clips_with_control(
    clips: &[ClipAudio],
    control: &ExportControl,
    progress: Option<&dyn Fn(usize, usize)>,
) -> Result<Vec<f32>, String> {
    for (index, clip) in clips.iter().enumerate() {
        if !clip.gains.is_empty() && clip.gains.len() != clip.samples.len() {
            return Err(format!(
                "audio mix failed: clip {index}: gains len {} != samples len {}",
                clip.gains.len(),
                clip.samples.len()
            ));
        }
    }
    let total = clips
        .iter()
        .map(|clip| clip.start_sample + clip.samples.len())
        .max()
        .unwrap_or(0);
    let work_total = clips
        .iter()
        .map(|clip| clip.samples.len())
        .sum::<usize>()
        .saturating_add(total)
        .max(1);
    let mut completed = 0_usize;
    let mut mixed = vec![0.0_f32; total];

    for clip in clips {
        for (offset, &sample) in clip.samples.iter().enumerate() {
            if completed.is_multiple_of(AUDIO_CANCEL_CHUNK_SAMPLES) {
                check_audio_cancel(control)?;
                if let Some(report) = progress {
                    report(completed, work_total);
                }
            }
            let gain = if clip.gains.is_empty() {
                1.0
            } else {
                clip.gains[offset]
            };
            mixed[clip.start_sample + offset] += sample * gain;
            completed += 1;
        }
    }
    for sample in &mut mixed {
        if completed.is_multiple_of(AUDIO_CANCEL_CHUNK_SAMPLES) {
            check_audio_cancel(control)?;
            if let Some(report) = progress {
                report(completed, work_total);
            }
        }
        *sample = sample.clamp(-1.0, 1.0);
        completed += 1;
    }
    check_audio_cancel(control)?;
    if let Some(report) = progress {
        report(work_total, work_total);
    }
    Ok(mixed)
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
    control: Option<&ExportControl>,
    on_progress: Option<AudioExportProgress>,
) -> Result<Option<PcmBuffer>, String> {
    let eligible_count = timeline
        .tracks
        .iter()
        .filter(|track| !track.muted)
        .flat_map(|track| &track.clips)
        .filter(|clip| matches!(clip.media_type, ClipType::Audio | ClipType::Video))
        .count();
    let mut clips_audio: Vec<ClipAudio> = Vec::new();
    let mut eligible_index = 0_usize;
    for track in &timeline.tracks {
        if track.muted {
            continue;
        }
        for clip in &track.clips {
            // Only audio and video clips carry sound; text/image/lottie don't.
            if clip.media_type != ClipType::Audio && clip.media_type != ClipType::Video {
                continue;
            }
            let decode_progress = match (control, &on_progress) {
                (Some(_), Some(emit)) => {
                    let emit = Arc::clone(emit);
                    let index = eligible_index;
                    Some(Arc::new(move |done: usize, total: usize| {
                        let total = total.max(1);
                        let numerator = index
                            .saturating_mul(AUDIO_DECODE_END as usize)
                            .saturating_mul(total)
                            .saturating_add(
                                done.min(total).saturating_mul(AUDIO_DECODE_END as usize),
                            );
                        let denominator = eligible_count.saturating_mul(total).max(1);
                        emit((numerator / denominator) as i32, AUDIO_PROGRESS_TOTAL);
                    }) as PcmProgressCallback)
                }
                _ => None,
            };
            eligible_index += 1;
            if let Some(ca) =
                project_clip_audio(clip, media, timeline.fps, control, decode_progress)?
            {
                clips_audio.push(ca);
            }
        }
    }
    if clips_audio.is_empty() {
        return Ok(None);
    }
    let mixed = match control {
        Some(control) => {
            if let Some(emit) = &on_progress {
                emit(AUDIO_MIX_START, AUDIO_PROGRESS_TOTAL);
            }
            let mix_progress = |done: usize, total: usize| {
                if let Some(emit) = &on_progress {
                    let span = (AUDIO_MIX_END - AUDIO_MIX_START) as usize;
                    let mapped = AUDIO_MIX_START
                        + (done.min(total.max(1)).saturating_mul(span) / total.max(1)) as i32;
                    emit(mapped, AUDIO_PROGRESS_TOTAL);
                }
            };
            mix_clips_with_control(&clips_audio, control, Some(&mix_progress))?
        }
        None => mix::mix_clips(&clips_audio).map_err(|e| format!("audio mix failed: {e}"))?,
    };
    if mixed.is_empty() {
        return Ok(None);
    }
    Ok(Some(PcmBuffer {
        spec: AUDIO_DECODE_SPEC,
        samples_f32: mixed,
    }))
}

pub(crate) fn mix_timeline_audio_for_manifest_with_control(
    timeline: &opentake_domain::Timeline,
    manifest: &opentake_domain::MediaManifest,
    project_dir: &Option<PathBuf>,
    control: &ExportControl,
    on_progress: Option<AudioExportProgress>,
) -> Result<Option<PcmBuffer>, String> {
    let (_sizes, media) = project_media(manifest, project_dir);
    mix_timeline_audio(timeline, &media, Some(control), on_progress)
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
    operation_id: String,
) -> Result<ExportSummary, String> {
    let guard = control.try_begin(&operation_id)?;
    // Snapshot the session up front; no session lock is held during GPU/encode.
    let timeline = core.get_timeline().timeline;
    let manifest = core.media();
    let project_dir = core.project_dir();
    let progress_operation_id = guard.operation_id().to_string();
    let on_progress: AudioExportProgress = Arc::new(move |done: i32, total: i32| {
        let _ = app.emit(
            "export://progress",
            ExportProgress {
                operation_id: progress_operation_id.clone(),
                done,
                total,
            },
        );
    });
    run_export_with_control(
        &timeline,
        &manifest,
        &project_dir,
        &req,
        ExportRunOptions {
            control: Some(&control),
            on_progress: Some(on_progress),
            ..ExportRunOptions::default()
        },
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
    run_export_with_control(
        timeline,
        manifest,
        project_dir,
        req,
        ExportRunOptions::default(),
    )
}

/// Shared orchestration behind [`run_export`] and [`export_video`]: `control`
/// (checked once per frame) and `on_progress` (called at most every
/// [`PROGRESS_INTERVAL`], plus once more at 100% when the loop finishes) are
/// both optional so callers with no Tauri context (the integration test) can
/// omit them.
#[derive(Default)]
pub(crate) struct ExportRunOptions<'a> {
    pub(crate) control: Option<&'a ExportControl>,
    pub(crate) on_progress: Option<AudioExportProgress>,
    pub(crate) frame_range: Option<(i32, i32)>,
    pub(crate) output_file: Option<File>,
    pub(crate) defer_completion: bool,
}

pub(crate) fn run_export_with_control(
    timeline: &opentake_domain::Timeline,
    manifest: &opentake_domain::MediaManifest,
    project_dir: &Option<PathBuf>,
    req: &ExportRequest,
    mut options: ExportRunOptions<'_>,
) -> Result<ExportSummary, String> {
    let control = options.control;
    let on_progress = options.on_progress;
    let defer_completion = options.defer_completion;
    let reserved_output = options.output_file.is_some();
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
    if !text_rasterizer.has_fonts() {
        eprintln!("[render] no system fonts discovered; text clips will render blank");
    }

    let mut encoder = match options.output_file.take() {
        Some(output) => VideoEncoder::new_with_file(
            &out_path,
            output,
            render_size.width,
            render_size.height,
            plan.fps,
            &preset,
        ),
        None => VideoEncoder::new(
            &out_path,
            render_size.width,
            render_size.height,
            plan.fps,
            &preset,
        ),
    }
    .map_err(|e| format!("encoder init failed: {e}"))?;

    let (start_frame, end_frame) = match options.frame_range {
        None => (0, plan.total_frames),
        Some((lo, hi)) => {
            let lo = lo.max(0).min(plan.total_frames);
            let hi = hi.max(lo).min(plan.total_frames);
            (lo, hi)
        }
    };
    let range_total = end_frame - start_frame;

    let mut last_progress_emit = Instant::now();
    for f in start_frame..end_frame {
        if control.is_some_and(|c| c.is_cancelled()) {
            // `abort` kills + waits on the ffmpeg child (unlike a plain `drop`,
            // which would orphan the process and race the file removal below).
            encoder.abort();
            // Best-effort cleanup of the partial file — a leftover half-encoded
            // video must not look like a finished export. Missing/unwritable is
            // not itself an error worth surfacing over the cancel.
            if !reserved_output {
                let _ = std::fs::remove_file(&out_path);
            }
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

        if let Some(emit) = &on_progress {
            let now = Instant::now();
            let done = f - start_frame + 1;
            let is_last = done == range_total;
            if is_last || progress_should_emit(last_progress_emit, now) {
                let mapped = if range_total == 0 {
                    VIDEO_RENDER_END
                } else {
                    done.saturating_mul(VIDEO_RENDER_END) / range_total
                };
                emit(mapped, AUDIO_PROGRESS_TOTAL);
                last_progress_emit = now;
            }
        }
    }

    // Decode + linearly mix every audio-bearing clip, then hand the mixed PCM to
    // the encoder so `finish` mux's it into the container. No audio → video-only.
    let audio_progress = on_progress.as_ref().map(|emit| {
        let emit = Arc::clone(emit);
        Arc::new(move |done: i32, total: i32| {
            let span = VIDEO_AUDIO_END - VIDEO_RENDER_END;
            let mapped =
                VIDEO_RENDER_END + done.min(total.max(1)).saturating_mul(span) / total.max(1);
            emit(mapped, AUDIO_PROGRESS_TOTAL);
        }) as AudioExportProgress
    });
    let mixed_audio = mix_timeline_audio(timeline, &media, control, audio_progress)?;
    let mut has_audio = false;
    if let Some(pcm) = mixed_audio {
        // Once any audio overlaps the exported interval, pad its trailing
        // silence to the exact video duration. The muxer uses `-shortest`, so
        // this keeps the completed container's frame_count/fps duration true.
        let pcm = slice_pcm(pcm, start_frame, end_frame, plan.fps);
        has_audio = !pcm.samples_f32.is_empty();
        encoder.push_audio(pcm);
    }

    let cancel = control
        .map(ExportControl::media_cancel_token)
        .unwrap_or_default();
    let finalize_progress = on_progress.as_ref().map(|emit| {
        let emit = Arc::clone(emit);
        move |done: usize, total: usize| {
            let span = (VIDEO_FINALIZE_END - VIDEO_AUDIO_END) as usize;
            let mapped = VIDEO_AUDIO_END
                + (done.min(total.max(1)).saturating_mul(span) / total.max(1)) as i32;
            emit(mapped, AUDIO_PROGRESS_TOTAL);
        }
    });
    match encoder.finish_cancellable(
        &cancel,
        finalize_progress
            .as_ref()
            .map(|callback| callback as &opentake_media::encode::EncodeProgressCallback),
    ) {
        Ok(()) => {}
        Err(opentake_media::MediaError::Cancelled) => {
            if !reserved_output {
                let _ = std::fs::remove_file(&out_path);
            }
            return Err(CANCELLED_SENTINEL.to_string());
        }
        Err(error) => {
            if !reserved_output {
                let _ = std::fs::remove_file(&out_path);
            }
            return Err(format!("encoder finish failed: {error}"));
        }
    }
    if control.is_some_and(ExportControl::is_cancelled) {
        if !reserved_output {
            let _ = std::fs::remove_file(&out_path);
        }
        return Err(CANCELLED_SENTINEL.to_string());
    }
    if let Some(emit) = &on_progress {
        emit(completion_progress(defer_completion), AUDIO_PROGRESS_TOTAL);
    }

    Ok(ExportSummary {
        out_path: req.out_path.clone(),
        width: render_size.width,
        height: render_size.height,
        fps: plan.fps,
        frame_count: range_total,
        has_audio,
    })
}

fn completion_progress(defer_completion: bool) -> i32 {
    if defer_completion {
        VIDEO_EXPORT_END
    } else {
        AUDIO_PROGRESS_TOTAL
    }
}

fn slice_pcm(pcm: PcmBuffer, start_frame: i32, end_frame: i32, fps: i32) -> PcmBuffer {
    if fps <= 0 || start_frame >= end_frame {
        return PcmBuffer {
            spec: pcm.spec,
            samples_f32: Vec::new(),
        };
    }
    let rate = pcm.spec.sample_rate as f64;
    let lo = ((start_frame.max(0) as f64) / fps as f64 * rate).round() as usize;
    let hi = ((end_frame.max(0) as f64) / fps as f64 * rate).round() as usize;
    let target_len = hi.saturating_sub(lo);
    let lo = lo.min(pcm.samples_f32.len());
    let hi = hi.max(lo).min(pcm.samples_f32.len());
    let mut samples_f32 = pcm.samples_f32[lo..hi].to_vec();
    if !samples_f32.is_empty() {
        samples_f32.resize(target_len, 0.0);
    }
    PcmBuffer {
        spec: pcm.spec,
        samples_f32,
    }
}

#[cfg(test)]
pub(crate) fn write_wav_s16le(samples: &[f32], sample_rate: u32, out: &Path) -> Result<(), String> {
    let mut output = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(out)
        .map_err(|error| format!("open WAV output: {error}"))?;
    let result = write_wav_s16le_cancellable_to_file(
        samples,
        sample_rate,
        &mut output,
        &MediaCancelToken::new(),
        None,
        None,
    );
    if result.is_err() {
        drop(output);
        let _ = std::fs::remove_file(out);
    }
    result
}

pub(crate) fn write_wav_s16le_cancellable_to_file(
    samples: &[f32],
    sample_rate: u32,
    file: &mut File,
    cancel: &MediaCancelToken,
    on_progress: Option<&dyn Fn(i32, i32)>,
    checkpoint_hook: Option<&dyn Fn(usize)>,
) -> Result<(), String> {
    let data_bytes = samples
        .len()
        .checked_mul(2)
        .ok_or_else(|| "wav output is too large".to_string())?;
    let data_len = u32::try_from(data_bytes).map_err(|_| "wav output is too large".to_string())?;
    let chunk_size = 36 + data_len;
    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&chunk_size.to_le_bytes());
    header.extend_from_slice(b"WAVE");
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes());
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    header.extend_from_slice(&2u16.to_le_bytes());
    header.extend_from_slice(&16u16.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_len.to_le_bytes());

    (|| {
        if cancel.checkpoint() {
            return Err(CANCELLED_SENTINEL.to_string());
        }
        file.set_len(0)
            .map_err(|error| format!("truncate WAV output: {error}"))?;
        file.seek(SeekFrom::Start(0))
            .map_err(|error| format!("seek WAV output: {error}"))?;
        file.write_all(&header)
            .map_err(|error| format!("write wav header: {error}"))?;
        for (chunk_index, chunk) in samples.chunks(AUDIO_CANCEL_CHUNK_SAMPLES).enumerate() {
            let done = chunk_index.saturating_mul(AUDIO_CANCEL_CHUNK_SAMPLES);
            if let Some(hook) = checkpoint_hook {
                hook(done);
            }
            if cancel.checkpoint() {
                return Err(CANCELLED_SENTINEL.to_string());
            }
            let data = opentake_media::encode::mono_f32_to_s16le(chunk);
            file.write_all(&data)
                .map_err(|error| format!("write wav samples: {error}"))?;
            if let Some(report) = on_progress {
                let span = (AUDIO_WAV_END - AUDIO_WAV_START) as usize;
                let completed = (done + chunk.len()).min(samples.len());
                let mapped = AUDIO_WAV_START
                    + (completed.saturating_mul(span) / samples.len().max(1)) as i32;
                report(mapped, AUDIO_PROGRESS_TOTAL);
            }
        }
        if cancel.checkpoint() {
            return Err(CANCELLED_SENTINEL.to_string());
        }
        file.flush()
            .map_err(|error| format!("flush wav output: {error}"))?;
        Ok(())
    })()
}

fn metadata_is_symlink_or_reparse(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}

fn ensure_project_media_dir(project_dir: &Path) -> Result<PathBuf, String> {
    let project_root = project_dir
        .canonicalize()
        .map_err(|error| format!("failed to resolve project directory: {error}"))?;
    let media_dir = project_dir.join("media");
    match std::fs::symlink_metadata(&media_dir) {
        Ok(metadata) => {
            if metadata_is_symlink_or_reparse(&metadata) || !metadata.is_dir() {
                return Err("project media path must be a real directory".to_string());
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            std::fs::create_dir(&media_dir)
                .map_err(|error| format!("failed to create project media dir: {error}"))?;
        }
        Err(error) => return Err(format!("failed to inspect project media dir: {error}")),
    }

    let metadata = std::fs::symlink_metadata(&media_dir)
        .map_err(|error| format!("failed to inspect project media dir: {error}"))?;
    if metadata_is_symlink_or_reparse(&metadata) || !metadata.is_dir() {
        return Err("project media path must be a real directory".to_string());
    }
    let resolved = media_dir
        .canonicalize()
        .map_err(|error| format!("failed to resolve project media dir: {error}"))?;
    if resolved.parent() != Some(project_root.as_path()) {
        return Err("project media directory escapes the project".to_string());
    }
    Ok(media_dir)
}

fn open_media_directory_nofollow(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x1;
        const FILE_SHARE_WRITE: u32 = 0x2;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options
            // Denying delete sharing pins this directory name for the entire
            // ProjectMediaOutput lifetime, so the subsequent full-path
            // create_new cannot be redirected through a junction handoff.
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let directory = options.open(path).map_err(|error| {
        format!("open project media directory without following links: {error}")
    })?;
    let metadata = directory
        .metadata()
        .map_err(|error| format!("inspect opened project media directory: {error}"))?;
    if metadata_is_symlink_or_reparse(&metadata) || !metadata.is_dir() {
        return Err("project media path must be a real directory".to_string());
    }
    Ok(directory)
}

#[cfg(unix)]
fn reserve_output_file(path: &Path, parent_handle: &File) -> Result<File, String> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;

    let file_name = path
        .file_name()
        .ok_or_else(|| "project media output has no file name".to_string())?;
    let file_name_c = CString::new(file_name.as_bytes())
        .map_err(|_| "project media output contains a NUL byte".to_string())?;
    // SAFETY: `parent_handle` is an open directory, `file_name_c` is a validated
    // single C string, and a successful descriptor is immediately owned by File.
    let descriptor = unsafe {
        libc::openat(
            parent_handle.as_raw_fd(),
            file_name_c.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            (libc::S_IRUSR | libc::S_IWUSR) as libc::c_uint,
        )
    };
    if descriptor < 0 {
        return Err(format!(
            "failed to reserve project media output: {}",
            io::Error::last_os_error()
        ));
    }
    // SAFETY: `openat` returned a new owned descriptor and no other owner exists.
    let file = unsafe { File::from_raw_fd(descriptor) };
    let opened = match file.metadata() {
        Ok(metadata) => metadata,
        Err(error) => {
            let _ = remove_reserved_output(parent_handle, &file, file_name);
            return Err(format!("failed to inspect reserved media output: {error}"));
        }
    };
    let visible = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            let _ = remove_reserved_output(parent_handle, &file, file_name);
            return Err(format!(
                "failed to revalidate reserved media output: {error}"
            ));
        }
    };
    let identity_matches = opened.dev() == visible.dev() && opened.ino() == visible.ino();
    if !identity_matches || metadata_is_symlink_or_reparse(&visible) || !visible.is_file() {
        let _ = remove_reserved_output(parent_handle, &file, file_name);
        return Err("project media output changed during reservation".to_string());
    }
    Ok(file)
}

#[cfg(not(unix))]
fn reserve_output_file(path: &Path, parent_handle: &File) -> Result<File, String> {
    let mut options = OpenOptions::new();
    // Keep Rust's semantic access flags in sync with the platform-specific
    // access_mode below. OpenOptions validates create_new before issuing the
    // Windows call and rejects creation unless write or append was requested.
    options.read(true).write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const DELETE: u32 = 0x0001_0000;
        const GENERIC_READ: u32 = 0x8000_0000;
        const GENERIC_WRITE: u32 = 0x4000_0000;
        const FILE_SHARE_READ: u32 = 0x1;
        const FILE_SHARE_WRITE: u32 = 0x2;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options
            .access_mode(GENERIC_READ | GENERIC_WRITE | DELETE)
            // Keep final-name replacement impossible through commit. We still
            // request DELETE ourselves so RAII cleanup can use the retained
            // handle if finalization fails.
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options
        .open(path)
        .map_err(|error| format!("failed to reserve project media output: {error}"))?;
    let metadata = match file.metadata() {
        Ok(metadata) => metadata,
        Err(error) => {
            if let Some(name) = path.file_name() {
                let _ = remove_reserved_output(parent_handle, &file, name);
            }
            return Err(format!("failed to inspect reserved media output: {error}"));
        }
    };
    if metadata_is_symlink_or_reparse(&metadata) || !metadata.is_file() {
        if let Some(name) = path.file_name() {
            let _ = remove_reserved_output(parent_handle, &file, name);
        }
        return Err("project media output must be a regular file".to_string());
    }
    Ok(file)
}

pub(crate) struct ProjectMediaOutput {
    path: PathBuf,
    media_dir: PathBuf,
    final_name: OsString,
    directory: File,
    file: File,
    directory_identity: FileIdentity,
    file_identity: FileIdentity,
    keep: bool,
}

impl ProjectMediaOutput {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn writer(&self) -> Result<File, String> {
        self.file
            .try_clone()
            .map_err(|error| format!("clone project media output: {error}"))
    }

    pub(crate) fn verify_identity(&self) -> Result<(), String> {
        let directory_metadata = std::fs::symlink_metadata(&self.media_dir)
            .map_err(|error| format!("revalidate project media directory: {error}"))?;
        if metadata_is_symlink_or_reparse(&directory_metadata) || !directory_metadata.is_dir() {
            return Err("project media directory changed during export".to_string());
        }
        let visible_directory = FileIdentity::from_path(&self.media_dir)
            .map_err(|error| format!("identify visible project media directory: {error}"))?;
        if visible_directory != self.directory_identity {
            return Err("project media directory changed during export".to_string());
        }
        let file_metadata = std::fs::symlink_metadata(&self.path)
            .map_err(|error| format!("revalidate project media output: {error}"))?;
        if metadata_is_symlink_or_reparse(&file_metadata) || !file_metadata.is_file() {
            return Err("project media output changed during export".to_string());
        }
        let visible_file = FileIdentity::from_path(&self.path)
            .map_err(|error| format!("identify visible project media output: {error}"))?;
        if visible_file != self.file_identity {
            return Err("project media output changed during export".to_string());
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn prepare_commit(&self) -> Result<(), String> {
        self.file
            .sync_all()
            .map_err(|error| format!("sync project media output: {error}"))?;
        self.verify_identity()
    }

    pub(crate) fn prepare_commit_cancellable(
        &self,
        guard: &ExportGuard<'_>,
        after_sync: impl FnOnce(),
    ) -> Result<(), String> {
        guard.checkpoint()?;
        self.file
            .sync_all()
            .map_err(|error| format!("sync project media output: {error}"))?;
        after_sync();
        guard.checkpoint()?;
        self.verify_identity()?;
        guard.checkpoint()
    }

    pub(crate) fn mark_kept(mut self) -> PathBuf {
        self.keep = true;
        self.path.clone()
    }

    #[cfg(test)]
    pub(crate) fn keep(self) -> Result<PathBuf, String> {
        self.prepare_commit()?;
        Ok(self.mark_kept())
    }
}

impl Drop for ProjectMediaOutput {
    fn drop(&mut self) {
        if self.keep {
            return;
        }
        if let Err(error) =
            destroy_and_remove_reserved_output(&self.directory, &self.file, &self.final_name)
        {
            eprintln!("[export] failed to fully destroy reserved output: {error}");
        }
    }
}

/// Destroy the payload through the retained application-owned descriptor
/// before making any pathname deletion decision. Truncation and its sync are
/// both attempted even if one fails, and handle/identity-safe deletion is
/// attempted last. This keeps a moved Unix inode from retaining rendered bytes
/// while still leaving an attacker replacement at the final name untouched.
fn destroy_and_remove_reserved_output(
    directory: &File,
    file: &File,
    name: &std::ffi::OsStr,
) -> Result<(), String> {
    let mut errors = Vec::new();
    if let Err(error) = file.set_len(0) {
        errors.push(format!("truncate retained output: {error}"));
    }
    if let Err(error) = file.sync_all() {
        errors.push(format!("sync retained output truncation: {error}"));
    }
    if let Err(error) = remove_reserved_output(directory, file, name) {
        errors.push(format!("remove retained output: {error}"));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

#[cfg(unix)]
fn remove_reserved_output(directory: &File, file: &File, name: &std::ffi::OsStr) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;

    let name = CString::new(name.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "output name contains NUL"))?;
    let mut opened: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: `file` is a retained live descriptor and `opened` is writable.
    if unsafe { libc::fstat(file.as_raw_fd(), &mut opened) } < 0 {
        return Err(io::Error::last_os_error());
    }
    // Revalidate the directory entry through the retained parent. If an
    // attacker replaced the name, leave their object untouched instead of
    // unlinking a path that no longer names our reserved file.
    let mut visible: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: the retained directory descriptor and validated child name are
    // live, and `visible` points to writable storage for one `stat` value.
    let inspected = unsafe {
        libc::fstatat(
            directory.as_raw_fd(),
            name.as_ptr(),
            &mut visible,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if inspected < 0 {
        let error = io::Error::last_os_error();
        return if error.kind() == io::ErrorKind::NotFound {
            Ok(())
        } else {
            Err(error)
        };
    }
    if opened.st_dev != visible.st_dev || opened.st_ino != visible.st_ino {
        return Ok(());
    }
    // SAFETY: `directory` is the retained no-follow directory and `name` is a
    // single validated component, so cleanup cannot traverse a swapped path.
    if unsafe { libc::unlinkat(directory.as_raw_fd(), name.as_ptr(), 0) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(windows)]
fn remove_reserved_output(
    _directory: &File,
    file: &File,
    _name: &std::ffi::OsStr,
) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileDispositionInfo, SetFileInformationByHandle, FILE_DISPOSITION_INFO,
    };

    let info = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: the retained file handle stays live for the call and the buffer
    // is the SDK layout supplied by windows-sys for FileDispositionInfo.
    let removed = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfo,
            (&info as *const FILE_DISPOSITION_INFO).cast(),
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    };
    if removed == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn remove_reserved_output(
    _directory: &File,
    _file: &File,
    _name: &std::ffi::OsStr,
) -> io::Result<()> {
    Ok(())
}

pub(crate) fn reserve_project_media_output(
    project_dir: &Path,
    stem: &str,
    ext: &str,
) -> Result<ProjectMediaOutput, String> {
    reserve_project_media_output_with_after_open(project_dir, stem, ext, |_| {})
}

fn reserve_project_media_output_with_after_open(
    project_dir: &Path,
    stem: &str,
    ext: &str,
    after_directory_open: impl FnOnce(&Path),
) -> Result<ProjectMediaOutput, String> {
    if !matches!(ext, "mp4" | "wav") {
        return Err("unsupported save-as-media extension".to_string());
    }
    let mut safe_stem: String = stem
        .chars()
        .take(64)
        .map(|value| {
            if value.is_ascii_alphanumeric() || matches!(value, '_' | '-') {
                value
            } else {
                '_'
            }
        })
        .collect();
    if safe_stem.is_empty() {
        safe_stem.push_str("media");
    }
    let media_dir = ensure_project_media_dir(project_dir)?;
    let directory = open_media_directory_nofollow(&media_dir)?;
    let directory_identity = FileIdentity::from_file(
        directory
            .try_clone()
            .map_err(|error| format!("clone project media directory handle: {error}"))?,
    )
    .map_err(|error| format!("identify project media directory: {error}"))?;
    after_directory_open(&media_dir);

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    loop {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let final_name = OsString::from(format!("{safe_stem}_{nanos:x}_{counter:x}.{ext}"));
        let path = media_dir.join(&final_name);
        match reserve_output_file(&path, &directory) {
            Ok(file) => {
                let file_identity = file
                    .try_clone()
                    .map_err(|error| format!("clone project media output handle: {error}"))
                    .and_then(|clone| {
                        FileIdentity::from_file(clone)
                            .map_err(|error| format!("identify project media output: {error}"))
                    });
                let file_identity = match file_identity {
                    Ok(identity) => identity,
                    Err(error) => {
                        let _ = remove_reserved_output(&directory, &file, &final_name);
                        return Err(error);
                    }
                };
                let output = ProjectMediaOutput {
                    path,
                    media_dir: media_dir.clone(),
                    final_name,
                    directory,
                    file,
                    directory_identity,
                    file_identity,
                    keep: false,
                };
                output.verify_identity()?;
                return Ok(output);
            }
            Err(_) if path.exists() => continue,
            Err(error) => return Err(error),
        }
    }
}

#[cfg(all(test, windows))]
fn reserve_project_media_output_with_hook(
    project_dir: &Path,
    stem: &str,
    ext: &str,
    after_directory_open: impl FnOnce(&Path),
) -> Result<ProjectMediaOutput, String> {
    reserve_project_media_output_with_after_open(project_dir, stem, ext, after_directory_open)
}

#[cfg(test)]
pub(crate) fn unique_project_media_path(
    project_dir: &Path,
    stem: &str,
    ext: &str,
) -> Result<PathBuf, String> {
    reserve_project_media_output(project_dir, stem, ext)?.keep()
}

#[cfg(test)]
pub(crate) fn cleanup_partial_output<T>(
    path: &Path,
    result: Result<T, String>,
) -> Result<T, String> {
    if result.is_err() {
        let _ = std::fs::remove_file(path);
    }
    result
}

fn validate_save_range(total_frames: i32, in_frame: i32, out_frame: i32) -> Result<(), String> {
    if in_frame < 0 || out_frame <= in_frame || out_frame > total_frames {
        return Err(format!(
            "save range must satisfy 0 <= inFrame < outFrame <= {total_frames}"
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRangeAsMediaRequest {
    in_frame: i32,
    out_frame: i32,
    operation_id: String,
}

#[tauri::command]
pub fn save_range_as_media(
    app: AppHandle,
    core: State<'_, AppCore>,
    control: State<'_, ExportControl>,
    media: State<'_, crate::media::MediaState>,
    prewarm: State<'_, crate::media::prewarm::PrewarmScheduler>,
    request: SaveRangeAsMediaRequest,
) -> Result<crate::media::MediaListDto, String> {
    let SaveRangeAsMediaRequest {
        in_frame,
        out_frame,
        operation_id,
    } = request;
    save_range_as_media_impl(&core, || {
        save_range_as_media_workflow(
            &app,
            &core,
            &control,
            media.engine(),
            &prewarm,
            SaveRangeAsMediaRequest {
                in_frame,
                out_frame,
                operation_id,
            },
        )
    })
}

fn save_range_as_media_impl(
    core: &AppCore,
    workflow: impl FnOnce() -> Result<crate::media::MediaListDto, String>,
) -> Result<crate::media::MediaListDto, String> {
    core.ensure_project_mutable().map_err(|e| e.to_string())?;
    workflow()
}

fn save_range_as_media_workflow(
    app: &AppHandle,
    core: &AppCore,
    control: &ExportControl,
    engine: &opentake_media::MediaEngine,
    prewarm: &crate::media::prewarm::PrewarmScheduler,
    request: SaveRangeAsMediaRequest,
) -> Result<crate::media::MediaListDto, String> {
    let SaveRangeAsMediaRequest {
        in_frame,
        out_frame,
        operation_id,
    } = request;
    let snapshot = core.runtime_snapshot();
    let project_dir = snapshot
        .project_dir
        .clone()
        .ok_or("save your project before saving a range as media")?;
    let total_frames = snapshot.timeline.total_frames();
    validate_save_range(total_frames, in_frame, out_frame)?;

    let mut guard = control.try_begin(&operation_id)?;
    let output = reserve_project_media_output(
        &project_dir,
        &format!("range_{in_frame}_{out_frame}"),
        "mp4",
    )?;
    let out_path = output.path().to_path_buf();
    let output_file = output.writer()?;
    let progress_app = app.clone();
    let progress_operation_id = guard.operation_id().to_string();
    let on_progress: AudioExportProgress = Arc::new(move |done: i32, total: i32| {
        let app = &progress_app;
        let _ = app.emit(
            "export://progress",
            ExportProgress {
                operation_id: progress_operation_id.clone(),
                done,
                total,
            },
        );
    });
    let req = ExportRequest {
        out_path: out_path.to_string_lossy().into_owned(),
        codec: ExportCodec::H264,
        quality: ExportQuality::P1080,
    };
    let project_dir_option = Some(project_dir.clone());
    let summary = run_export_with_control(
        &snapshot.timeline,
        &snapshot.media,
        &project_dir_option,
        &req,
        ExportRunOptions {
            control: Some(control),
            on_progress: Some(Arc::clone(&on_progress)),
            frame_range: Some((in_frame, out_frame)),
            output_file: Some(output_file),
            defer_completion: true,
        },
    )?;
    crate::media::finalize_saved_media(
        crate::media::SavedMediaFinalizationContext {
            core,
            engine,
            prewarm,
            expected_project_epoch: snapshot.project_epoch,
            expected_project_dir: &project_dir,
            metadata: crate::media::SavedMediaMetadata::Video(summary),
            on_progress: on_progress.as_ref(),
        },
        output,
        &mut guard,
    )
}

// MARK: - Self-contained `.opentake` bundle export (#29 / upstream `.palmier`)

/// C1A missing-media compatibility DTO retained for Rust integration tests.
/// No registered Tauri command or Web UI entry exposes it while the secure
/// native workflow is under construction.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MissingMediaDto {
    /// The manifest entry id.
    pub id: String,
    /// The manifest entry display name.
    pub name: String,
}

impl From<opentake_project::MissingMedia> for MissingMediaDto {
    fn from(m: opentake_project::MissingMedia) -> Self {
        MissingMediaDto {
            id: m.id,
            name: m.name,
        }
    }
}

/// C1A bundle-report compatibility DTO retained for Rust integration tests.
/// No registered Tauri command or Web UI entry exposes it while the secure
/// native workflow is under construction.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BundleReportDto {
    /// Absolute path the bundle was written to.
    pub out_path: String,
    /// Ids of entries that were external and are now bundled internally.
    pub collected: Vec<String>,
    /// Count of already-internal media files copied across.
    pub copied_internal: usize,
    /// Entries whose source file could not be found (kept as dangling refs).
    pub missing: Vec<MissingMediaDto>,
    /// Total bytes copied into the new bundle's `media/` directory.
    pub total_bytes: u64,
}

impl BundleReportDto {
    /// Project an [`opentake_project::ArchiveReport`] plus the destination path
    /// into the camelCase DTO the front end consumes.
    fn from_report(out_path: String, report: opentake_project::ArchiveReport) -> Self {
        BundleReportDto {
            out_path,
            collected: report.collected,
            copied_internal: report.copied_internal,
            missing: report
                .missing
                .into_iter()
                .map(MissingMediaDto::from)
                .collect(),
            total_bytes: report.total_bytes,
        }
    }
}

/// C1A non-command archive seam. It is public only for Rust integration tests;
/// the registered Tauri handler and UI entry are intentionally absent.
/// `source_bundle` remains optional so never-saved-project parity can be tested.
pub fn run_bundle_export(
    timeline: &opentake_domain::Timeline,
    manifest: &opentake_domain::MediaManifest,
    generation_log: &opentake_project::GenerationLog,
    source_bundle: Option<&Path>,
    compatibility: &opentake_project::ProjectCompatibility,
    out_path: String,
) -> Result<BundleReportDto, String> {
    compatibility.ensure_writable().map_err(|e| e.to_string())?;
    let dest = PathBuf::from(&out_path);
    let report =
        opentake_project::archive(timeline, manifest, generation_log, source_bundle, &dest)
            .map_err(|e| e.to_string())?;
    Ok(BundleReportDto::from_report(out_path, report))
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
    use std::fs;
    use std::path::Path;

    fn unknown_core(root: &Path) -> AppCore {
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

    fn recursive_tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        fn walk(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
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
    fn save_range_refuses_before_output_creation() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let core = unknown_core(tmp.path());
        let saves = tmp.path().join("cache/saves");
        fs::create_dir_all(saves.join("existing")).expect("create saves fixture");
        fs::write(saves.join("existing/keep.bin"), b"before").expect("write saves fixture");
        let before = recursive_tree(&saves);
        let called = std::cell::Cell::new(false);
        let sentinel = saves.join("range-workflow-ran-before-guard.bin");

        let error = save_range_as_media_impl(&core, || {
            called.set(true);
            fs::write(&sentinel, b"bad ordering").expect("write workflow sentinel");
            Err("workflow should not run".into())
        })
        .expect_err("range export must be rejected");

        assert!(error.contains("compatibility read-only"), "{error}");
        assert!(!called.get());
        assert!(!sentinel.exists());
        assert_eq!(recursive_tree(&saves), before);
    }

    #[test]
    fn export_control_starts_uncancelled() {
        let control = ExportControl::default();
        let _guard = control.try_begin("test-export").expect("start export");
        assert!(!control.is_cancelled());
    }

    #[test]
    fn export_control_rejects_invalid_external_operation_ids() {
        let control = ExportControl::default();

        assert_eq!(
            control.try_begin("").expect_err("empty id must fail"),
            "invalid export operation id"
        );
        assert_eq!(
            control
                .try_begin("contains whitespace")
                .expect_err("unsafe id must fail"),
            "invalid export operation id"
        );
        assert!(control.try_begin("save-as:valid-id_123").is_ok());
    }

    #[test]
    fn export_progress_serializes_operation_identity_in_web_shape() {
        let value = serde_json::to_value(ExportProgress {
            operation_id: "save-as:test".to_string(),
            done: 4,
            total: 10,
        })
        .expect("serialize progress payload");

        assert_eq!(value["operationId"], "save-as:test");
        assert_eq!(value["done"], 4);
        assert_eq!(value["total"], 10);
        assert!(value.get("operation_id").is_none());
    }

    #[test]
    fn export_control_request_cancel_flips_the_flag() {
        let control = ExportControl::default();
        let _guard = control.try_begin("test-export").expect("start export");
        assert!(control.request_cancel("test-export"));
        assert!(control.is_cancelled());
    }

    #[test]
    fn export_control_new_generation_does_not_inherit_prior_cancel() {
        let control = ExportControl::default();
        let first = control
            .try_begin("first-export")
            .expect("start first export");
        assert!(control.request_cancel("first-export"));
        assert!(control.is_cancelled());
        drop(first);
        let _second = control
            .try_begin("second-export")
            .expect("start second export");
        assert!(!control.is_cancelled());
    }

    #[test]
    fn export_guard_cancel_wins_before_commit() {
        let control = ExportControl::default();
        let mut guard = control.try_begin("test-export").expect("start export");
        assert!(control.request_cancel("test-export"));

        assert_eq!(
            guard
                .commit()
                .expect_err("cancelled generation cannot commit"),
            CANCELLED_SENTINEL
        );
    }

    #[test]
    fn export_guard_commit_wins_before_late_cancel() {
        let control = ExportControl::default();
        let mut guard = control.try_begin("test-export").expect("start export");
        guard.commit().expect("commit active generation");

        assert!(!control.request_cancel("test-export"));
        assert!(!control.is_cancelled());
        assert!(!guard.cancel_token().is_cancelled());
    }

    #[test]
    fn stale_operation_cancel_cannot_cancel_successor_generation() {
        let control = ExportControl::default();
        let mut first = control
            .try_begin("save-as-first")
            .expect("start first export");
        first.commit().expect("commit first export");
        let second = control
            .try_begin("save-as-second")
            .expect("start successor export");

        assert!(!control.request_cancel("save-as-first"));
        assert!(!second.cancel_token().is_cancelled());
        assert!(control.request_cancel("save-as-second"));
        assert!(second.cancel_token().is_cancelled());
    }

    #[test]
    fn export_control_cancel_is_observable_across_threads() {
        let control = Arc::new(ExportControl::default());
        let _guard = control.try_begin("test-export").expect("start export");
        let canceller = Arc::clone(&control);
        std::thread::spawn(move || canceller.request_cancel("test-export"))
            .join()
            .expect("cancel thread");
        assert!(control.is_cancelled());
    }

    #[test]
    fn export_control_cancel_cannot_be_erased_during_lease_publication() {
        use std::sync::mpsc;

        let control = Arc::new(ExportControl::default());
        let worker_control = Arc::clone(&control);
        let (published_tx, published_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let (guard_ready_tx, guard_ready_rx) = mpsc::channel();
        let (release_guard_tx, release_guard_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let guard = worker_control
                .try_begin_with_hook("publication-test", || {
                    published_tx.send(()).expect("lease generation published");
                    resume_rx.recv().expect("resume lease publication");
                })
                .expect("start export");
            guard_ready_tx.send(()).expect("guard ready");
            release_guard_rx.recv().expect("release guard");
            drop(guard);
        });

        published_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("new generation is installed before begin returns");
        let cancel_control = Arc::clone(&control);
        let (cancel_started_tx, cancel_started_rx) = mpsc::channel();
        let (cancel_done_tx, cancel_done_rx) = mpsc::channel();
        let canceller = std::thread::spawn(move || {
            cancel_started_tx.send(()).expect("cancel started");
            cancel_control.request_cancel("publication-test");
            cancel_done_tx.send(()).expect("cancel completed");
        });
        cancel_started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("cancel invoked while begin is paused");
        resume_tx.send(()).expect("resume begin");
        guard_ready_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("begin returned");
        cancel_done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("cancel targets the published generation");

        assert!(
            control.is_cancelled(),
            "begin must not clear the cancellation"
        );
        release_guard_tx.send(()).expect("release operation guard");
        canceller.join().expect("cancel thread joins");
        worker.join().expect("begin thread joins");
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
    fn save_as_defers_terminal_progress_until_identity_checked_import() {
        assert_eq!(completion_progress(true), VIDEO_EXPORT_END);
        assert!(completion_progress(true) < AUDIO_PROGRESS_TOTAL);
        assert_eq!(completion_progress(false), AUDIO_PROGRESS_TOTAL);
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
    fn save_clip_slice_pcm_cuts_requested_frame_window() {
        let pcm = PcmBuffer {
            spec: PcmSpec {
                sample_rate: 4,
                channels: 1,
                format: PcmFormat::F32,
            },
            samples_f32: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        };
        let sliced = slice_pcm(pcm, 1, 2, 2);
        assert_eq!(sliced.samples_f32, vec![2.0, 3.0]);
    }

    #[test]
    fn save_range_audio_pads_trailing_silence_to_reported_video_duration() {
        let pcm = PcmBuffer {
            spec: PcmSpec {
                sample_rate: 4,
                channels: 1,
                format: PcmFormat::F32,
            },
            samples_f32: vec![0.25, 0.5],
        };

        let sliced = slice_pcm(pcm, 0, 2, 2);

        assert_eq!(sliced.samples_f32, vec![0.25, 0.5, 0.0, 0.0]);
    }

    #[test]
    fn save_range_after_all_audio_does_not_attach_a_silent_track() {
        let pcm = PcmBuffer {
            spec: PcmSpec {
                sample_rate: 4,
                channels: 1,
                format: PcmFormat::F32,
            },
            samples_f32: vec![0.25, 0.5],
        };

        let sliced = slice_pcm(pcm, 1, 2, 2);

        assert!(sliced.samples_f32.is_empty());
    }

    #[test]
    fn project_media_path_is_unique_sanitized_and_inside_media_dir() {
        let project = tempfile::tempdir().expect("project");
        let first = unique_project_media_path(project.path(), "../clip / unsafe", "mp4")
            .expect("first path");
        let second = unique_project_media_path(project.path(), "../clip / unsafe", "mp4")
            .expect("second path");

        assert_ne!(first, second);
        assert_eq!(first.parent(), Some(project.path().join("media").as_path()));
        assert_eq!(
            first.extension().and_then(|value| value.to_str()),
            Some("mp4")
        );
        let name = first
            .file_name()
            .and_then(|value| value.to_str())
            .expect("file name");
        assert!(!name.contains('/'));
        assert!(!name.contains(".."));
    }

    #[cfg(unix)]
    #[test]
    fn project_media_symlink_is_rejected_without_writing_outside_bundle() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("fixture root");
        let project = root.path().join("Project.opentake");
        let outside = root.path().join("outside");
        fs::create_dir(&project).expect("project directory");
        fs::create_dir(&outside).expect("outside directory");
        symlink(&outside, project.join("media")).expect("redirect project media directory");

        let error = unique_project_media_path(&project, "escaped", "mp4")
            .expect_err("media symlink must be rejected");

        assert!(error.contains("real directory"), "{error}");
        assert_eq!(fs::read_dir(&outside).expect("read outside").count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn preexisting_output_symlink_is_never_reserved_or_truncated() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("fixture root");
        let media = root.path().join("media");
        fs::create_dir(&media).expect("media directory");
        let outside = root.path().join("outside.bin");
        fs::write(&outside, b"keep").expect("outside fixture");
        let candidate = media.join("candidate.wav");
        symlink(&outside, &candidate).expect("candidate symlink");
        let directory = open_media_directory_nofollow(&media).expect("open media directory");

        let error =
            reserve_output_file(&candidate, &directory).expect_err("create-new rejects symlink");

        assert!(error.contains("reserve"), "{error}");
        assert_eq!(
            fs::read(&outside).expect("outside remains readable"),
            b"keep"
        );
    }

    #[cfg(unix)]
    #[test]
    fn reserved_output_detects_final_name_swap_without_deleting_replacement() {
        let project = tempfile::tempdir().expect("project");
        let output = reserve_project_media_output(project.path(), "identity", "wav")
            .expect("reserve output");
        let visible_path = output.path().to_path_buf();
        output
            .writer()
            .expect("clone output")
            .write_all(b"reserved")
            .expect("write reserved output");
        let moved_original = visible_path.with_extension("moved");
        fs::rename(&visible_path, &moved_original).expect("move retained output");
        fs::write(&visible_path, b"replacement").expect("install replacement");

        let error = output
            .verify_identity()
            .expect_err("visible replacement must fail identity validation");
        assert!(error.contains("output changed"), "{error}");
        drop(output);

        assert_eq!(
            fs::read(&visible_path).expect("replacement remains"),
            b"replacement"
        );
        if moved_original.exists() {
            assert_eq!(
                fs::metadata(&moved_original)
                    .expect("inspect retained moved output")
                    .len(),
                0,
                "failure cleanup must destroy the retained output payload"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn reserved_output_detects_parent_swap_and_cleans_through_retained_parent() {
        let project = tempfile::tempdir().expect("project");
        let output = reserve_project_media_output(project.path(), "identity", "wav")
            .expect("reserve output");
        let final_name = output.path().file_name().expect("reserved name").to_owned();
        let media = project.path().join("media");
        let moved_media = project.path().join("media-moved");
        fs::rename(&media, &moved_media).expect("move original media directory");
        fs::create_dir(&media).expect("install replacement media directory");
        fs::write(media.join("keep.txt"), b"replacement").expect("replacement marker");

        let error = output
            .verify_identity()
            .expect_err("visible parent replacement must fail identity validation");
        assert!(error.contains("directory changed"), "{error}");
        drop(output);

        assert!(
            !moved_media.join(final_name).exists(),
            "drop must clean the reserved file through retained handles"
        );
        assert_eq!(
            fs::read(media.join("keep.txt")).expect("replacement marker remains"),
            b"replacement"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_project_media_junction_is_rejected_without_writing_target() {
        let root = tempfile::tempdir().expect("fixture root");
        let project = root.path().join("Project.opentake");
        let outside = root.path().join("outside");
        fs::create_dir(&project).expect("project directory");
        fs::create_dir(&outside).expect("outside directory");
        let junction = project.join("media");
        let status = std::process::Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(&junction)
            .arg(&outside)
            .status()
            .expect("create media junction");
        assert!(status.success(), "mklink /J must create test junction");

        let error = reserve_project_media_output(&project, "escaped", "wav")
            .err()
            .expect("media junction must be rejected");

        assert!(error.contains("real directory"), "{error}");
        assert_eq!(fs::read_dir(&outside).expect("read target").count(), 0);
    }

    #[cfg(windows)]
    #[test]
    fn windows_directory_handoff_blocks_junction_replacement_before_child_create() {
        use std::sync::Barrier;

        let root = tempfile::tempdir().expect("fixture root");
        let project = root.path().join("Project.opentake");
        let media = project.join("media");
        let moved_media = project.join("media-moved");
        let outside = root.path().join("outside");
        fs::create_dir(&project).expect("project directory");
        fs::create_dir(&outside).expect("outside directory");
        let barrier = Arc::new(Barrier::new(2));
        let attack_barrier = Arc::clone(&barrier);
        let attack_media = media.clone();
        let attack_moved = moved_media.clone();
        let attack_outside = outside.clone();

        let output =
            reserve_project_media_output_with_hook(&project, "handoff", "wav", move |_| {
                let attacker = std::thread::spawn(move || {
                    attack_barrier.wait();
                    let rename = fs::rename(&attack_media, &attack_moved);
                    if rename.is_ok() {
                        let status = std::process::Command::new("cmd")
                            .arg("/C")
                            .arg("mklink")
                            .arg("/J")
                            .arg(&attack_media)
                            .arg(&attack_outside)
                            .status()
                            .expect("attempt replacement junction");
                        return Err(format!(
                            "directory rename unexpectedly succeeded; junction status={status}"
                        ));
                    }
                    Ok(())
                });
                barrier.wait();
                attacker
                    .join()
                    .expect("junction attacker joins")
                    .expect("retained directory handle must deny rename/delete sharing");
            })
            .expect("reserve output after blocked handoff attack");

        assert_eq!(output.path().parent(), Some(media.as_path()));
        assert_eq!(fs::read_dir(&outside).expect("read outside").count(), 0);
        assert!(!moved_media.exists());
        drop(output);
        assert_eq!(fs::read_dir(&outside).expect("reread outside").count(), 0);
    }

    #[cfg(windows)]
    #[test]
    fn windows_retained_output_handle_blocks_final_name_replacement() {
        let project = tempfile::tempdir().expect("project");
        let output = reserve_project_media_output(project.path(), "identity", "wav")
            .expect("reserve output");
        let visible_path = output.path().to_path_buf();
        let moved = visible_path.with_extension("moved");

        assert!(
            fs::rename(&visible_path, &moved).is_err(),
            "output handle must deny delete sharing through commit"
        );
        assert!(visible_path.is_file());
        assert!(!moved.exists());
        drop(output);
        assert!(!visible_path.exists());
    }

    #[test]
    fn wav_cancellation_inside_write_loop_removes_partial_output() {
        use std::sync::mpsc;

        let project = tempfile::tempdir().expect("project");
        let output = reserve_project_media_output(project.path(), "cancelled_audio", "wav")
            .expect("reserved output");
        let output_path = output.path().to_path_buf();
        let mut writer = output.writer().expect("clone reserved output");
        let samples = vec![0.25_f32; AUDIO_CANCEL_CHUNK_SAMPLES * 4];
        let cancel = MediaCancelToken::new();
        let worker_cancel = cancel.clone();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let hook = move |_done: usize| {
                entered_tx.send(()).expect("WAV loop entered");
                release_rx.recv().expect("release WAV loop");
            };
            let result = write_wav_s16le_cancellable_to_file(
                &samples,
                48_000,
                &mut writer,
                &worker_cancel,
                None,
                Some(&hook),
            );
            done_tx.send(result).expect("publish WAV result");
        });

        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("actual WAV write loop reached its checkpoint");
        cancel.cancel();
        release_tx.send(()).expect("release WAV loop");
        let result = done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("WAV cancellation must return promptly");

        assert_eq!(result.unwrap_err(), CANCELLED_SENTINEL);
        worker.join().expect("WAV worker joins");
        drop(output);
        assert!(
            !output_path.exists(),
            "cancelled WAV must remove reserved partial output"
        );
    }

    #[test]
    fn export_control_rejects_second_save_as_media() {
        let control = ExportControl::default();
        let first = control
            .try_begin("first-save")
            .expect("first export starts");
        let error = control
            .try_begin("second-save")
            .expect_err("second export must be rejected");
        assert_eq!(error, "another export is already in progress");
        drop(first);
        assert!(
            control.try_begin("third-save").is_ok(),
            "guard drop must release the export slot"
        );
    }

    #[test]
    fn export_cancel_interrupts_blocking_audio_decoder_before_completion() {
        use std::sync::mpsc;

        let control = Arc::new(ExportControl::default());
        let worker_control = Arc::clone(&control);
        let allow_natural_completion = Arc::new(AtomicBool::new(false));
        let worker_allow_natural_completion = Arc::clone(&allow_natural_completion);
        let (entered_tx, entered_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let _guard = worker_control
                .try_begin("audio-save")
                .expect("start audio save");
            let result = decode_pcm_with_export_control(
                &worker_control,
                Path::new("/blocking.wav"),
                Some((0.0, 3_600.0)),
                None,
                move |_path, spec, _range, cancel, _progress| {
                    entered_tx.send(()).expect("decoder entered");
                    while !cancel.checkpoint() {
                        if worker_allow_natural_completion.load(Ordering::Acquire) {
                            return Ok(PcmBuffer {
                                spec: *spec,
                                samples_f32: vec![0.0],
                            });
                        }
                        std::thread::yield_now();
                    }
                    Err(opentake_media::MediaError::Cancelled)
                },
            );
            done_tx.send(result).expect("publish decoder result");
        });

        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("blocking decoder started");
        assert!(control.request_cancel("audio-save"));
        let result = done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("cancel must return before natural decode completion");

        assert!(matches!(result, Err(opentake_media::MediaError::Cancelled)));
        assert!(!allow_natural_completion.load(Ordering::Acquire));
        worker.join().expect("decoder worker joins");
    }

    #[test]
    fn retime_pcm_matches_speed_two_clip_timeline_duration() {
        let decoded_at_speed_two = vec![0.0, 0.25, 0.5, 0.75, 1.0, 0.75, 0.5, 0.25];
        let timeline_len = decoded_at_speed_two.len() / 2;
        let retimed = retime_pcm_to_len(&decoded_at_speed_two, timeline_len);

        assert_eq!(retimed.len(), timeline_len);
        assert_eq!(retimed.first(), decoded_at_speed_two.first());
        assert_eq!(retimed.last(), decoded_at_speed_two.last());
    }

    #[test]
    fn failed_save_removes_partial_output() {
        let project = tempfile::tempdir().expect("project");
        let output = project.path().join("media/partial.mp4");
        fs::create_dir_all(output.parent().expect("parent")).expect("media dir");
        fs::write(&output, b"partial").expect("partial output");

        let result = cleanup_partial_output::<()>(&output, Err("render failed".to_string()));

        assert_eq!(result.unwrap_err(), "render failed");
        assert!(!output.exists());
    }

    #[test]
    fn save_range_validates_half_open_bounds_before_output_path_creation() {
        assert!(validate_save_range(100, 10, 20).is_ok());
        assert!(validate_save_range(100, -1, 20).is_err());
        assert!(validate_save_range(100, 20, 20).is_err());
        assert!(validate_save_range(100, 20, 101).is_err());
    }

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
        let got = project_clip_audio(&clip, &media, 30, None, None).expect("ok");
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
        assert!(project_clip_audio(&clip, &media, 30, None, None)
            .expect("ok")
            .is_none());
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
        assert!(mix_timeline_audio(&tl, &media, None, None)
            .expect("ok")
            .is_none());
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
        assert!(mix_timeline_audio(&tl, &media, None, None)
            .expect("ok")
            .is_none());
    }

    // MARK: - `.opentake` bundle export DTOs

    #[test]
    fn missing_media_dto_serializes_camelcase() {
        // The front end reads `{ id, name }` — both already single words, so this
        // pins the field names (and the `serde(rename_all = "camelCase")` on the
        // struct) against an accidental rename that would silently break the
        // dialog's missing-media list. camelCase IPC drift is this repo's #1 bug.
        let dto = MissingMediaDto {
            id: "asset-7".into(),
            name: "b-roll.mov".into(),
        };
        let json = serde_json::to_value(&dto).expect("serialize");
        assert_eq!(
            json,
            serde_json::json!({ "id": "asset-7", "name": "b-roll.mov" })
        );
    }

    #[test]
    fn bundle_report_dto_serializes_camelcase_multiword_fields() {
        // `outPath`, `copiedInternal`, and `totalBytes` are the multi-word fields
        // the TS `BundleReport` interface must match verbatim; assert the exact
        // JSON keys so a Rust-side rename can't diverge from the front end.
        let dto = BundleReportDto {
            out_path: "/tmp/My Film.opentake".into(),
            collected: vec!["asset-1".into(), "asset-2".into()],
            copied_internal: 3,
            missing: vec![MissingMediaDto {
                id: "asset-9".into(),
                name: "gone.mp4".into(),
            }],
            total_bytes: 123_456,
        };
        let json = serde_json::to_value(&dto).expect("serialize");
        assert_eq!(
            json,
            serde_json::json!({
                "outPath": "/tmp/My Film.opentake",
                "collected": ["asset-1", "asset-2"],
                "copiedInternal": 3,
                "missing": [{ "id": "asset-9", "name": "gone.mp4" }],
                "totalBytes": 123_456,
            })
        );
    }

    #[test]
    fn bundle_report_dto_from_report_maps_every_field() {
        // The projection from the engine's `ArchiveReport` (+ dest path) into the
        // camelCase DTO must carry each field 1:1, including converting the
        // engine's `MissingMedia` into the front-end `MissingMediaDto`.
        let report = opentake_project::ArchiveReport {
            collected: vec!["ext-1".into()],
            copied_internal: 2,
            missing: vec![opentake_project::MissingMedia {
                id: "m-1".into(),
                name: "lost.png".into(),
            }],
            total_bytes: 4096,
        };
        let dto = BundleReportDto::from_report("/out/x.opentake".into(), report);
        assert_eq!(dto.out_path, "/out/x.opentake");
        assert_eq!(dto.collected, vec!["ext-1".to_string()]);
        assert_eq!(dto.copied_internal, 2);
        assert_eq!(dto.total_bytes, 4096);
        assert_eq!(
            dto.missing,
            vec![MissingMediaDto {
                id: "m-1".into(),
                name: "lost.png".into()
            }]
        );
    }
}
