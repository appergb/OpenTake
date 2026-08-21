//! Full-timeline video export (`export_video`).
//!
//! This is the export counterpart to the single-frame preview path
//! ([`crate::render::composite_frame`]): it walks **every** frame of the current
//! timeline, composites each on the GPU through the ready-made wgpu compositor
//! (`opentake-render`), and pipes the RGBA frames into the system ffmpeg encoder
//! (`opentake_media::VideoEncoder`) to produce a real `.mp4` on disk.
//!
//! Scope of this first cut (SPEC §2.4 / §8.2):
//! - **H.264 / .mp4**, **H.265 / .mp4**, **ProRes 422 / .mov**, and transparent
//!   **ProRes 4444 / .mov** are wired.
//! - **Linear audio mixdown**: every audio-bearing clip's source window is
//!   decoded to mono f32 at the mix rate, placed at its frame-derived sample
//!   offset, scaled by its `volume_at` envelope, summed, hard-limited, and mux'd
//!   in by the encoder (`-c:v copy` + AAC). A timeline with no audio still
//!   produces the same video-only file as before.
//! - Export renders at the **full** export resolution
//!   ([`opentake_render::export_render_size`]), not the preview cap.
//! - Image and Lottie sources materialize directly as content-hashed GPU
//!   textures; Lottie uses the same Velato/Vello frame contract as preview and
//!   playback, and any unsupported document fails the export explicitly.
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

use std::collections::{HashMap, HashSet};
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

use crate::render::LottieMaterializer;

use opentake_core::AppCore;
use opentake_domain::{AudioDenoise, Clip, ClipType, LutReference, MediaSource, TextStyle};
#[cfg(test)]
use opentake_media::encode::ClipAudio;
use opentake_media::encode::{mix, MIX_SAMPLE_RATE};
use opentake_media::{
    decode_frame_at, extract_pcm, extract_pcm_cancellable_with_progress, interpolate_frame_pair,
    probe, ExportPreset, ExportResolution as EncodeResolution, FrameInterpolationFallback,
    FrameInterpolationMode, FrameRequest, MediaCancelToken, PcmBuffer, PcmFormat,
    PcmProgressCallback, PcmSpec, RgbaFrame, VideoCodec, VideoEncoder,
};
use opentake_project::ProjectRoot;
use opentake_render::gpu::compositor::{
    TextureInterpolationConfig, TextureInterpolationFallback, TextureInterpolationMode,
    TextureResolveRequest,
};
use opentake_render::gpu::texture::upload_rgba;
use opentake_render::{
    export_render_size, try_build_render_plan, AudioClipPlan, Compositor, CosmicTextRasterizer,
    DecodedFrame, ExportResolution as RenderResolution, GpuLutTexture, GpuTexture, RenderDevice,
    SourceMetrics, TextRasterRequest, TextRasterizer, TextureCache, TextureResolver, TextureSource,
};

/// Per-frame texture cache size. Export advances monotonically, so video-frame
/// hit rate is low; a small cache still helps text/image layers re-used across
/// frames. Bounds VRAM during the export loop.
const TEXTURE_CACHE_CAP: usize = 64;
const AUDIO_STREAM_WINDOW_SAMPLES: usize = MIX_SAMPLE_RATE as usize * 2;

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
    /// Apple ProRes 4444 with an alpha plane / `.mov`.
    Prores4444,
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

    /// Linearize a normal export's final success against cancellation. Save-as
    /// workflows use `ExportGuard::commit` later, after their manifest
    /// transaction has passed its own identity checks.
    pub(crate) fn commit_active(&self) -> Result<(), String> {
        let mut state = self
            .operation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let active = state
            .active
            .as_ref()
            .ok_or_else(|| "export generation is no longer active".to_string())?;
        if active.cancel.is_cancelled() {
            return Err(CANCELLED_SENTINEL.to_string());
        }
        state.active = None;
        Ok(())
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

    #[cfg(test)]
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

fn validate_export_cancel_sources(
    control: Option<&ExportControl>,
    external_cancel: Option<&MediaCancelToken>,
) -> Result<(), String> {
    if control.is_some() && external_cancel.is_some() {
        Err("export cannot combine control and external cancellation sources".to_string())
    } else {
        Ok(())
    }
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
        ExportCodec::Prores4444 => {
            if ext.as_deref() != Some("mov") {
                return Err("ProRes 4444 export requires a .mov output path".to_string());
            }
            Ok(ExportPreset::new(
                VideoCodec::ProRes4444,
                quality.encode_resolution(),
            ))
        }
    }
}

fn export_clear_rgba(codec: ExportCodec) -> [f64; 4] {
    if codec == ExportCodec::Prores4444 {
        [0.0, 0.0, 0.0, 0.0]
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

/// Resolvable info for one media asset, projected from the manifest.
struct MediaInfo {
    path: PathBuf,
    source_fps: Option<f64>,
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

/// `TextureResolver` that decodes a layer's pixels on demand via ffmpeg and
/// uploads them to the GPU. Video keys per source-frame; image and Lottie keys
/// include source content hashes; text rasterizes its box. Mirrors the preview
/// resolver, but the decode box is the full export render size.
struct MediaResolver<'d> {
    device: &'d opentake_render::wgpu::Device,
    queue: &'d opentake_render::wgpu::Queue,
    cache: &'d mut TextureCache,
    lottie: &'d mut LottieMaterializer,
    media: &'d HashMap<String, MediaInfo>,
    timeline_fps: i32,
    text: &'d HashMap<String, TextInfo>,
    text_rasterizer: &'d CosmicTextRasterizer,
    /// Decode/raster box for source frames (matches the export render size).
    render_box: (u32, u32),
    project_root: Option<&'d ProjectRoot>,
    lut_cache: &'d mut HashMap<String, Rc<GpuLutTexture>>,
    materialization_error: Option<String>,
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
        let info = self.media.get(media_ref)?;
        let source_fps = info.source_fps.unwrap_or(interpolation.source_fps);
        if !source_fps.is_finite() || source_fps <= 0.0 {
            return None;
        }
        let timestamp = source_frame.max(0) as f64 / interpolation.target_fps;
        let source_position = timestamp * source_fps;
        let first_index = source_position.floor().max(0.0) as i64;
        let next_index = source_position.ceil().max(0.0) as i64;
        let alpha = source_position - first_index as f64;
        let decode = |index: i64| {
            decode_frame_at(
                &info.path,
                &FrameRequest {
                    time_secs: index as f64 / source_fps,
                    max_size: self.render_box,
                    tolerance_secs: 0.0,
                    apply_rotation: true,
                },
            )
            .ok()
            .map(|(_, frame)| frame)
        };
        let first = decode(first_index)?;
        let last = if next_index == first_index {
            first.clone()
        } else {
            // A half-open media duration may not expose the mathematical next
            // frame at the tail. Hold the last decodable endpoint instead of
            // dropping the whole layer to black.
            decode(next_index).unwrap_or_else(|| first.clone())
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
        let frame = interpolate_frame_pair(&first, &last, alpha, requested, fallback, true)
            .ok()?
            .frame;
        let decoded = DecodedFrame::new(frame.width, frame.height, frame.rgba, false);
        let tex = upload_rgba(
            self.device,
            self.queue,
            &decoded,
            false,
            Some("export-optical-flow"),
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
                let info = self.media.get(media_ref)?;
                return match self.lottie.resolve(
                    self.device,
                    self.queue,
                    self.cache,
                    &info.path,
                    source_frame,
                    self.render_box,
                    "export-lottie",
                ) {
                    Ok(texture) => Some(texture),
                    Err(error) => {
                        eprintln!("[export] {error}");
                        self.materialization_error = Some(error);
                        None
                    }
                };
            }
        };

        let info = self.media.get(media_ref)?;
        let key = if is_image {
            let content_hash = opentake_media::file_sha256(&info.path).ok()?;
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
            "export-lut",
        )?;
        if let Some(texture) = &resolved {
            self.lut_cache.insert(reference.id.clone(), texture.clone());
        }
        Ok(resolved)
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
        media.insert(
            entry.id.clone(),
            MediaInfo {
                path,
                source_fps: entry.source_fps,
            },
        );
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
const AUDIO_MIX_START: i32 = 850;
const AUDIO_MIX_END: i32 = 980;
#[cfg(test)]
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

#[cfg(test)]
fn check_audio_cancel(control: &ExportControl) -> Result<(), String> {
    check_audio_cancel_with_external(Some(control), None)
}

fn check_audio_cancel_with_external(
    control: Option<&ExportControl>,
    external_cancel: Option<&MediaCancelToken>,
) -> Result<(), String> {
    if control.is_some_and(ExportControl::is_cancelled)
        || external_cancel.is_some_and(MediaCancelToken::is_cancelled)
    {
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

#[cfg(test)]
fn retime_pcm_to_len_with_control(
    samples: &[f32],
    target_len: usize,
    control: Option<&ExportControl>,
) -> Result<Vec<f32>, String> {
    retime_pcm_to_len_with_external(samples, target_len, control, None)
}

fn retime_pcm_to_len_with_external(
    samples: &[f32],
    target_len: usize,
    control: Option<&ExportControl>,
    external_cancel: Option<&MediaCancelToken>,
) -> Result<Vec<f32>, String> {
    if samples.is_empty() || target_len == 0 {
        return Ok(Vec::new());
    }

    let source_span = (samples.len() - 1) as f64;
    let target_span = target_len.saturating_sub(1) as f64;
    let mut retimed = Vec::with_capacity(target_len);
    for index in 0..target_len {
        if index.is_multiple_of(AUDIO_CANCEL_CHUNK_SAMPLES) {
            check_audio_cancel_with_external(control, external_cancel)?;
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
trait AudioPlanLike {
    fn clip(&self) -> &Clip;
    fn volume_at(&self, frame: i32) -> f64;
    fn true_peak_ceiling_dbtp(&self) -> Option<f64>;
    fn audio_denoise(&self) -> Option<AudioDenoise>;
}

impl AudioPlanLike for Clip {
    fn clip(&self) -> &Clip {
        self
    }

    fn volume_at(&self, frame: i32) -> f64 {
        Clip::volume_at(self, frame)
    }

    fn true_peak_ceiling_dbtp(&self) -> Option<f64> {
        self.loudness_normalization
            .map(|normalization| normalization.true_peak_ceiling_dbtp)
    }

    fn audio_denoise(&self) -> Option<AudioDenoise> {
        self.audio_denoise
    }
}

impl AudioPlanLike for AudioClipPlan {
    fn clip(&self) -> &Clip {
        &self.clip
    }

    fn volume_at(&self, frame: i32) -> f64 {
        AudioClipPlan::volume_at(self, frame)
    }

    fn true_peak_ceiling_dbtp(&self) -> Option<f64> {
        std::iter::once(&self.gain_clip)
            .chain(self.compound_ancestors.iter())
            .filter_map(|clip| {
                clip.loudness_normalization
                    .map(|normalization| normalization.true_peak_ceiling_dbtp)
            })
            .min_by(f64::total_cmp)
    }

    fn audio_denoise(&self) -> Option<AudioDenoise> {
        std::iter::once(&self.gain_clip)
            .chain(self.compound_ancestors.iter())
            .find_map(|clip| clip.audio_denoise)
    }
}

#[cfg(test)]
fn project_clip_audio<T: AudioPlanLike>(
    plan: &T,
    media: &HashMap<String, MediaInfo>,
    timeline_fps: i32,
    control: Option<&ExportControl>,
    decode_progress: Option<PcmProgressCallback>,
) -> Result<Option<ClipAudio>, String> {
    let clip = plan.clip();
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
    let samples = apply_export_denoise(&samples, 1, plan.audio_denoise(), control)?;
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
        let g = plan.volume_at(tl_frame) as f32;
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

#[cfg(test)]
fn apply_export_denoise(
    samples: &[f32],
    channels: usize,
    config: Option<AudioDenoise>,
    control: Option<&ExportControl>,
) -> Result<Vec<f32>, String> {
    apply_export_denoise_with_external(samples, channels, config, control, None)
}

fn apply_export_denoise_with_external(
    samples: &[f32],
    channels: usize,
    config: Option<AudioDenoise>,
    control: Option<&ExportControl>,
    external_cancel: Option<&MediaCancelToken>,
) -> Result<Vec<f32>, String> {
    let Some(config) = config else {
        return Ok(samples.to_vec());
    };
    let cancel = control
        .map(ExportControl::media_cancel_token)
        .or_else(|| external_cancel.cloned())
        .unwrap_or_default();
    opentake_media::analysis::denoise_interleaved(
        samples,
        channels,
        MIX_SAMPLE_RATE,
        config,
        &cancel,
        None,
    )
    .map_err(|error| match error {
        opentake_media::analysis::DenoiseError::Cancelled => CANCELLED_SENTINEL.to_string(),
        other => format!("audio denoise failed: {other}"),
    })
}

/// Decode + mix every audio-bearing clip on the timeline into one mono buffer.
///
/// Walks audio and video clips (video clips can carry an audio track), projects
/// each through [`project_clip_audio`], and linearly mixes the lot. Returns
/// `None` when nothing contributes audio (→ the caller keeps the video-only
/// output). Errors surface decode/mix failures to the front-end.
#[cfg(test)]
fn mix_timeline_audio(
    timeline: &opentake_domain::Timeline,
    media: &HashMap<String, MediaInfo>,
    control: Option<&ExportControl>,
    on_progress: Option<AudioExportProgress>,
) -> Result<Option<PcmBuffer>, String> {
    let clips = timeline
        .tracks
        .iter()
        .filter(|track| !track.muted)
        .flat_map(|track| &track.clips)
        .filter(|clip| matches!(clip.media_type, ClipType::Audio | ClipType::Video))
        .cloned()
        .collect::<Vec<_>>();
    let mut samples_f32 = Vec::new();
    let has_audio = stream_flattened_audio(
        &clips,
        media,
        AudioStreamOptions {
            timeline_fps: timeline.fps,
            start_frame: 0,
            end_frame: timeline.total_frames(),
            control,
            external_cancel: None,
            on_progress,
        },
        |samples| {
            samples_f32
                .try_reserve(samples.len())
                .map_err(|error| format!("audio output allocation failed: {error}"))?;
            samples_f32.extend_from_slice(samples);
            Ok(())
        },
    )?;
    Ok(has_audio.then_some(PcmBuffer {
        spec: AUDIO_DECODE_SPEC,
        samples_f32,
    }))
}

struct AudioStreamOptions<'a> {
    timeline_fps: i32,
    start_frame: i32,
    end_frame: i32,
    control: Option<&'a ExportControl>,
    external_cancel: Option<&'a MediaCancelToken>,
    on_progress: Option<AudioExportProgress>,
}

fn stream_flattened_audio<T: AudioPlanLike>(
    clips: &[T],
    media: &HashMap<String, MediaInfo>,
    options: AudioStreamOptions<'_>,
    mut emit: impl FnMut(&[f32]) -> Result<(), String>,
) -> Result<bool, String> {
    let AudioStreamOptions {
        timeline_fps,
        start_frame,
        end_frame,
        control,
        external_cancel,
        on_progress,
    } = options;
    if timeline_fps <= 0 || start_frame >= end_frame {
        return Ok(false);
    }
    let mut audible_media = HashSet::new();
    for plan in clips {
        let clip = plan.clip();
        let clip_end = clip.start_frame.saturating_add(clip.duration_frames);
        if clip.duration_frames <= 0 || clip_end <= start_frame || clip.start_frame >= end_frame {
            continue;
        }
        let Some(info) = media.get(&clip.media_ref) else {
            continue;
        };
        let metadata = opentake_media::probe(&info.path)
            .map_err(|error| format!("audio probe failed for {}: {error}", clip.media_ref))?;
        if metadata.has_audio {
            audible_media.insert(clip.media_ref.clone());
        }
    }
    if audible_media.is_empty() {
        return Ok(false);
    }

    let sample_at_frame = |frame: i32| {
        ((frame.max(0) as f64 / timeline_fps as f64) * MIX_SAMPLE_RATE as f64).round() as usize
    };
    let range_start = sample_at_frame(start_frame);
    let range_end = sample_at_frame(end_frame);
    let total_samples = range_end.saturating_sub(range_start);
    let true_peak_ceiling_dbtp = clips
        .iter()
        .filter_map(AudioPlanLike::true_peak_ceiling_dbtp)
        .min_by(f64::total_cmp);
    let cancel = control
        .map(ExportControl::media_cancel_token)
        .or_else(|| external_cancel.cloned())
        .unwrap_or_default();

    for relative_start in (0..total_samples).step_by(AUDIO_STREAM_WINDOW_SAMPLES) {
        check_audio_cancel_with_external(control, external_cancel)?;
        let window_len = AUDIO_STREAM_WINDOW_SAMPLES.min(total_samples - relative_start);
        let window_start = range_start.saturating_add(relative_start);
        let window_end = window_start.saturating_add(window_len);
        let mut mixed = vec![0.0_f32; window_len];
        for plan in clips {
            let clip = plan.clip();
            if !audible_media.contains(&clip.media_ref) || clip.duration_frames <= 0 {
                continue;
            }
            let clip_start = sample_at_frame(clip.start_frame);
            let clip_end = sample_at_frame(clip.start_frame.saturating_add(clip.duration_frames));
            let overlap_start = window_start.max(clip_start);
            let overlap_end = window_end.min(clip_end);
            if overlap_start >= overlap_end || clip_end <= clip_start {
                continue;
            }
            let Some(info) = media.get(&clip.media_ref) else {
                continue;
            };
            let Some((source_lo, source_hi)) = clip_source_window_secs(clip, timeline_fps) else {
                continue;
            };
            let source_span = source_hi - source_lo;
            let relative_lo = (overlap_start - clip_start) as f64 / (clip_end - clip_start) as f64;
            let relative_hi = (overlap_end - clip_start) as f64 / (clip_end - clip_start) as f64;
            let source_range = (
                source_lo + source_span * relative_lo,
                source_lo + source_span * relative_hi,
            );
            let decoded = match control {
                Some(control) => decode_pcm_with_export_control(
                    control,
                    &info.path,
                    Some(source_range),
                    None,
                    extract_pcm_cancellable_with_progress,
                ),
                None => match external_cancel {
                    Some(cancel) => extract_pcm_cancellable_with_progress(
                        &info.path,
                        &AUDIO_DECODE_SPEC,
                        Some(source_range),
                        cancel,
                        None,
                    ),
                    None => extract_pcm(&info.path, &AUDIO_DECODE_SPEC, Some(source_range)),
                },
            };
            let pcm = match decoded {
                Ok(pcm) => pcm,
                Err(opentake_media::MediaError::NoTrack(_, _)) => continue,
                Err(opentake_media::MediaError::Cancelled) => {
                    return Err(CANCELLED_SENTINEL.to_string());
                }
                Err(error) => {
                    return Err(format!(
                        "audio decode failed for {}: {error}",
                        clip.media_ref
                    ));
                }
            };
            let target_len = overlap_end - overlap_start;
            let retimed = retime_pcm_to_len_with_external(
                &pcm.samples_f32,
                target_len,
                control,
                external_cancel,
            )?;
            let processed = apply_export_denoise_with_external(
                &retimed,
                1,
                plan.audio_denoise(),
                control,
                external_cancel,
            )?;
            let output_start = overlap_start - window_start;
            for (offset, sample) in processed.into_iter().take(target_len).enumerate() {
                if offset.is_multiple_of(AUDIO_CANCEL_CHUNK_SAMPLES) {
                    check_audio_cancel_with_external(control, external_cancel)?;
                }
                let absolute_sample = overlap_start.saturating_add(offset);
                let timeline_frame = ((absolute_sample as f64 / MIX_SAMPLE_RATE as f64)
                    * timeline_fps as f64)
                    .floor() as i32;
                mixed[output_start + offset] += sample * plan.volume_at(timeline_frame) as f32;
            }
        }
        for sample in &mut mixed {
            *sample = sample.clamp(-1.0, 1.0);
        }
        mix::apply_true_peak_ceiling(&mut mixed, true_peak_ceiling_dbtp);
        emit(&mixed)?;
        if let Some(report) = &on_progress {
            let completed = relative_start.saturating_add(window_len);
            let span = (AUDIO_MIX_END - AUDIO_MIX_START) as usize;
            let mapped =
                AUDIO_MIX_START + (completed.saturating_mul(span) / total_samples.max(1)) as i32;
            report(mapped, AUDIO_PROGRESS_TOTAL);
        }
        if cancel.checkpoint() || external_cancel.is_some_and(MediaCancelToken::is_cancelled) {
            return Err(CANCELLED_SENTINEL.to_string());
        }
    }
    Ok(true)
}

/// Mix a timeline in bounded windows and append each window directly to a WAV.
///
/// The output header is written lazily after the audio preflight succeeds, so
/// callers can distinguish a silent timeline without materializing a full PCM
/// buffer or leaving a header-only file behind.
pub(crate) fn write_timeline_audio_wav_for_manifest_with_control(
    timeline: &opentake_domain::Timeline,
    manifest: &opentake_domain::MediaManifest,
    project_dir: &Option<PathBuf>,
    file: &mut File,
    control: &ExportControl,
    on_progress: Option<AudioExportProgress>,
) -> Result<Option<usize>, String> {
    let (_sizes, media) = project_media(manifest, project_dir);
    let clips = timeline
        .tracks
        .iter()
        .filter(|track| !track.muted)
        .flat_map(|track| &track.clips)
        .filter(|clip| matches!(clip.media_type, ClipType::Audio | ClipType::Video))
        .cloned()
        .collect::<Vec<_>>();
    let end_frame = timeline.total_frames();
    let expected_samples = if timeline.fps > 0 {
        ((end_frame.max(0) as f64 / timeline.fps as f64) * MIX_SAMPLE_RATE as f64).round() as usize
    } else {
        0
    };
    let mut written_samples = 0_usize;
    let cancel = control.media_cancel_token();
    let has_audio = stream_flattened_audio(
        &clips,
        &media,
        AudioStreamOptions {
            timeline_fps: timeline.fps,
            start_frame: 0,
            end_frame,
            control: Some(control),
            external_cancel: None,
            on_progress: on_progress.clone(),
        },
        |samples| {
            if written_samples == 0 {
                write_wav_header(file, expected_samples, MIX_SAMPLE_RATE)?;
            }
            if cancel.checkpoint() {
                return Err(CANCELLED_SENTINEL.to_string());
            }
            let data = opentake_media::encode::mono_f32_to_s16le(samples);
            file.write_all(&data)
                .map_err(|error| format!("write wav samples: {error}"))?;
            written_samples = written_samples.saturating_add(samples.len());
            Ok(())
        },
    )?;
    if !has_audio {
        return Ok(None);
    }
    if written_samples != expected_samples {
        return Err(format!(
            "WAV sample count mismatch: wrote {written_samples}, expected {expected_samples}"
        ));
    }
    file.flush()
        .map_err(|error| format!("flush wav output: {error}"))?;
    if let Some(report) = &on_progress {
        report(AUDIO_WAV_END, AUDIO_PROGRESS_TOTAL);
    }
    // Post-write verification (same contract as the video path above): probe the
    // finished WAV back through its retained handle and fail unless it reads as
    // mono s16 PCM at the expected length. The reserved-output drop guard
    // removes the partial file on error.
    let probe = opentake_media::probe::probe_file(file)
        .map_err(|error| format!("wav output validation failed: {error}"))?;
    validate_export_probe(
        &probe,
        &ExportProbeExpectations {
            video_codec: None,
            audio_codec: Some(ProbeAudioCodec::PcmS16Le),
            expected_duration_secs: written_samples as f64 / MIX_SAMPLE_RATE as f64,
            duration_tolerance_secs: 0.05,
        },
    )?;
    Ok(Some(written_samples))
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
    let snapshot = core.runtime_snapshot();
    let timeline = snapshot.timeline;
    let manifest = snapshot.media;
    let project_dir = snapshot.project_dir;
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
    pub(crate) external_cancel: Option<MediaCancelToken>,
    pub(crate) on_progress: Option<AudioExportProgress>,
    pub(crate) frame_range: Option<(i32, i32)>,
    pub(crate) output_file: Option<File>,
    pub(crate) defer_completion: bool,
}

/// Best-effort cleanup for a normal video export's partial output. The parent
/// directory is opened before the output file is created and the output handle
/// is retained by the guard, so even encoder initialization failures clean the
/// same inode through the identity-safe removal path. Reserved project-media
/// outputs stay owned by `ProjectMediaOutput`, whose descriptor/identity-safe
/// Drop path performs the stronger cleanup contract.
struct ExportOutputCleanup {
    path: PathBuf,
    enabled: bool,
    active: bool,
    succeeded: bool,
    directory: Option<File>,
    file: Option<File>,
    final_name: Option<OsString>,
}

impl ExportOutputCleanup {
    fn new(path: PathBuf, enabled: bool) -> Result<Self, String> {
        if !enabled {
            return Ok(Self {
                path,
                enabled,
                active: false,
                succeeded: false,
                directory: None,
                file: None,
                final_name: None,
            });
        }
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let final_name = path
            .file_name()
            .ok_or_else(|| "export output has no file name".to_string())?
            .to_os_string();
        let directory = open_media_directory_nofollow(parent)?;
        Ok(Self {
            path,
            enabled,
            active: true,
            succeeded: false,
            directory: Some(directory),
            file: None,
            final_name: Some(final_name),
        })
    }

    fn attach_output(&mut self, output: File) {
        if self.enabled {
            self.file = Some(output);
        }
    }

    fn open_output_file(&self) -> Result<File, String> {
        let directory = self
            .directory
            .as_ref()
            .ok_or_else(|| "export cleanup directory handle is missing".to_string())?;
        open_output_file_in_parent(&self.path, directory)
    }

    fn encoder_file(&self) -> Result<File, String> {
        self.file
            .as_ref()
            .ok_or_else(|| "export cleanup output handle is not attached".to_string())?
            .try_clone()
            .map_err(|error| format!("clone export output for encoder: {error}"))
    }

    fn mark_success(&mut self) {
        self.succeeded = true;
    }

    fn verify_visible_identity(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        let directory = self
            .directory
            .as_ref()
            .ok_or_else(|| "export cleanup directory handle is missing".to_string())?;
        let file = self
            .file
            .as_ref()
            .ok_or_else(|| "export cleanup output handle is missing".to_string())?;
        let visible_parent =
            std::fs::symlink_metadata(self.path.parent().unwrap_or_else(|| Path::new(".")))
                .map_err(|error| format!("identify visible export directory: {error}"))?;
        if metadata_is_symlink_or_reparse(&visible_parent) || !visible_parent.is_dir() {
            return Err("export output parent must remain a real directory".to_string());
        }
        let visible_directory =
            FileIdentity::from_path(self.path.parent().unwrap_or_else(|| Path::new(".")))
                .map_err(|error| format!("identify visible export directory: {error}"))?;
        let retained_directory = FileIdentity::from_file(
            directory
                .try_clone()
                .map_err(|error| format!("clone retained export directory: {error}"))?,
        )
        .map_err(|error| format!("identify retained export directory: {error}"))?;
        if visible_directory != retained_directory {
            return Err("export output parent changed during export".to_string());
        }
        let visible_file_metadata = std::fs::symlink_metadata(&self.path)
            .map_err(|error| format!("identify visible export output: {error}"))?;
        if metadata_is_symlink_or_reparse(&visible_file_metadata)
            || !visible_file_metadata.is_file()
        {
            return Err("export output must remain a real file".to_string());
        }
        let visible_file = FileIdentity::from_path(&self.path)
            .map_err(|error| format!("identify visible export output: {error}"))?;
        let retained_file = FileIdentity::from_file(
            file.try_clone()
                .map_err(|error| format!("clone retained export output: {error}"))?,
        )
        .map_err(|error| format!("identify retained export output: {error}"))?;
        if visible_file != retained_file {
            return Err("export output changed during export".to_string());
        }
        Ok(())
    }
}

impl Drop for ExportOutputCleanup {
    fn drop(&mut self) {
        if self.enabled && self.active && !self.succeeded {
            if let (Some(directory), Some(file), Some(final_name)) =
                (&self.directory, &self.file, &self.final_name)
            {
                if let Err(error) = destroy_and_remove_reserved_output(directory, file, final_name)
                {
                    eprintln!("[export] failed to clean ordinary partial output: {error}");
                }
            }
        }
    }
}

pub(crate) fn run_export_with_control(
    timeline: &opentake_domain::Timeline,
    manifest: &opentake_domain::MediaManifest,
    project_dir: &Option<PathBuf>,
    req: &ExportRequest,
    mut options: ExportRunOptions<'_>,
) -> Result<ExportSummary, String> {
    let control = options.control;
    let external_cancel = options.external_cancel.clone();
    validate_export_cancel_sources(control, external_cancel.as_ref())?;
    let on_progress = options.on_progress;
    let defer_completion = options.defer_completion;
    let reserved_output = options.output_file.is_some();
    let out_path = PathBuf::from(&req.out_path);
    let preset = resolve_preset(req.codec, req.quality, &out_path)?;

    let text = project_text(timeline);
    let (sizes, media) = project_media(manifest, project_dir);
    let straight_alpha = manifest
        .entries
        .iter()
        .filter(|entry| entry.carries_straight_alpha())
        .map(|entry| entry.id.clone())
        .collect();

    let render_size = export_render_size(
        (timeline.width, timeline.height),
        req.quality.render_resolution(),
    );

    let metrics = ManifestMetrics {
        sizes,
        straight_alpha,
    };
    let plan = try_build_render_plan(timeline, render_size, &metrics)
        .map_err(|error| format!("invalid timeline graph: {error}"))?;
    let project_root = project_dir
        .as_ref()
        .map(ProjectRoot::open)
        .transpose()
        .map_err(|error| format!("open project LUT storage: {error}"))?;

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
    // Fail closed: a text-bearing export with no font faces would complete
    // "successfully" with invisible text. Reject it before the encoder starts;
    // the preview path (render.rs) deliberately stays lenient.
    ensure_text_export_fonts(!plan.text_plans.is_empty(), &text_rasterizer)?;

    // Declare this before the encoder so Rust drops the encoder first (which
    // reaps ffmpeg) and only then removes an error/cancelled partial output.
    let mut output_cleanup = ExportOutputCleanup::new(out_path.clone(), !reserved_output)?;
    let encoder_output = match options.output_file.take() {
        Some(output) => output,
        None => {
            let output = output_cleanup.open_output_file()?;
            output_cleanup.attach_output(output);
            output_cleanup.encoder_file()?
        }
    };
    let mut encoder = VideoEncoder::new_with_file(
        &out_path,
        encoder_output,
        render_size.width,
        render_size.height,
        plan.fps,
        &preset,
    )
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
    let mut lut_cache = HashMap::new();
    let mut texture_cache = TextureCache::new(TEXTURE_CACHE_CAP);
    let mut lottie = LottieMaterializer::new();
    for f in start_frame..end_frame {
        if control.is_some_and(|c| c.is_cancelled())
            || external_cancel
                .as_ref()
                .is_some_and(MediaCancelToken::is_cancelled)
        {
            // `abort` kills + waits on the ffmpeg child (unlike a plain `drop`,
            // which would orphan the process and race the file removal below).
            encoder.abort();
            // Best-effort cleanup of the partial file — a leftover half-encoded
            // video must not look like a finished export. Missing/unwritable is
            // not itself an error worth surfacing over the cancel.
            return Err(CANCELLED_SENTINEL.to_string());
        }

        let mut frame_plan = plan.frame(timeline, f);
        frame_plan.clear_rgba = export_clear_rgba(req.codec);
        let mut resolver = MediaResolver {
            device: &dev.device,
            queue: &dev.queue,
            cache: &mut texture_cache,
            lottie: &mut lottie,
            media: &media,
            timeline_fps: plan.fps,
            text: &text,
            text_rasterizer: &text_rasterizer,
            render_box: (render_size.width, render_size.height),
            project_root: project_root.as_ref(),
            lut_cache: &mut lut_cache,
            materialization_error: None,
        };
        let interpolation = TextureInterpolationConfig::new(
            plan.fps as f64,
            plan.fps as f64,
            TextureInterpolationMode::OpticalFlow,
            TextureInterpolationFallback::Blend,
        )
        .map_err(str::to_string)?;
        let composite = compositor
            .render_to_rgba_with_interpolation(
                &dev.device,
                &dev.queue,
                render_size,
                &frame_plan,
                &mut resolver,
                interpolation,
            )
            .map_err(|e| format!("composite render failed at frame {f}: {e}"))?;
        if let Some(error) = resolver.materialization_error.take() {
            encoder.abort();
            return Err(format!(
                "export materialization failed at frame {f}: {error}"
            ));
        }
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

    // Decode + linearly mix every audio-bearing clip in bounded windows, then
    // append each window to the encoder's private PCM spool. `finish` muxes that
    // file into the container; no audio keeps the output video-only.
    let audio_progress = on_progress.as_ref().map(|emit| {
        let emit = Arc::clone(emit);
        Arc::new(move |done: i32, total: i32| {
            let span = VIDEO_AUDIO_END - VIDEO_RENDER_END;
            let mapped =
                VIDEO_RENDER_END + done.min(total.max(1)).saturating_mul(span) / total.max(1);
            emit(mapped, AUDIO_PROGRESS_TOTAL);
        }) as AudioExportProgress
    });
    let cancel = control
        .map(ExportControl::media_cancel_token)
        .or_else(|| external_cancel.clone())
        .unwrap_or_default();
    let has_audio = stream_flattened_audio(
        &plan.audio_clips,
        &media,
        AudioStreamOptions {
            timeline_fps: plan.fps,
            start_frame,
            end_frame,
            control,
            external_cancel: external_cancel.as_ref(),
            on_progress: audio_progress,
        },
        |samples| {
            encoder
                .push_audio_chunk(AUDIO_DECODE_SPEC, samples, &cancel)
                .map_err(|error| format!("audio spool failed: {error}"))
        },
    )?;
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
            return Err(CANCELLED_SENTINEL.to_string());
        }
        Err(error) => {
            return Err(format!("encoder finish failed: {error}"));
        }
    }
    if control.is_some_and(ExportControl::is_cancelled)
        || external_cancel
            .as_ref()
            .is_some_and(MediaCancelToken::is_cancelled)
    {
        return Err(CANCELLED_SENTINEL.to_string());
    }
    // Bind the visible pathname to the retained output before any probe reads
    // it. Keep the second verification below as a post-probe race check.
    output_cleanup.verify_visible_identity()?;
    // Post-encode verification (mirrors motion.rs's post-encode probe): the
    // ffmpeg child may exit 0 while the output is truncated or corrupt, so a
    // clean exit alone is not proof of a usable file. Probe the produced file
    // and fail (removing the partial output, consistent with the cancel/error
    // cleanup path above) unless the streams, codec, and duration match the
    // request. A zero-frame export is documented as valid and is skipped.
    if range_total > 0 {
        let fps = plan.fps.max(1) as f64;
        let expectations = ExportProbeExpectations {
            video_codec: Some(match preset.codec {
                VideoCodec::H264 => ProbeVideoCodec::H264,
                VideoCodec::H265 => ProbeVideoCodec::H265,
                VideoCodec::ProRes422 | VideoCodec::ProRes4444 => ProbeVideoCodec::ProRes,
            }),
            audio_codec: has_audio.then_some(match preset.codec {
                VideoCodec::ProRes422 | VideoCodec::ProRes4444 => ProbeAudioCodec::PcmS16Le,
                _ => ProbeAudioCodec::Aac,
            }),
            expected_duration_secs: range_total as f64 / fps,
            duration_tolerance_secs: 1.5 / fps,
        };
        let probe_result = probe(&out_path)
            .map_err(|error| format!("output validation failed: {error}"))
            .and_then(|probe| validate_export_probe(&probe, &expectations));
        probe_result?;
    }
    output_cleanup.verify_visible_identity()?;
    if !defer_completion {
        if let Some(control) = control {
            control.commit_active()?;
        }
        if let Some(external_cancel) = external_cancel.as_ref() {
            if !external_cancel.try_commit() {
                return Err(CANCELLED_SENTINEL.to_string());
            }
        }
    }
    // Revalidate immediately after the cancellation commit as well. A path
    // replacement in either side of the final linearization fails closed and
    // leaves the retained original for the cleanup guard.
    output_cleanup.verify_visible_identity()?;
    output_cleanup.mark_success();
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

// MARK: - Post-encode output validation

/// Expected video codec family of a finished export, as reported by ffprobe's
/// `codec_name`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProbeVideoCodec {
    H264,
    H265,
    ProRes,
}

impl ProbeVideoCodec {
    /// ffprobe `codec_name` values this family accepts. `prores_ks` is the
    /// encoder token; the demuxed name is `prores`, so both are accepted.
    fn accepts(&self, codec_name: &str) -> bool {
        match self {
            ProbeVideoCodec::H264 => codec_name == "h264",
            ProbeVideoCodec::H265 => matches!(codec_name, "hevc" | "h265"),
            ProbeVideoCodec::ProRes => matches!(codec_name, "prores" | "prores_ks"),
        }
    }
}

/// Expected audio codec family of a finished export, as reported by ffprobe's
/// `codec_name`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProbeAudioCodec {
    Aac,
    PcmS16Le,
}

impl ProbeAudioCodec {
    fn accepts(&self, codec_name: &str) -> bool {
        match self {
            ProbeAudioCodec::Aac => codec_name == "aac",
            ProbeAudioCodec::PcmS16Le => codec_name == "pcm_s16le",
        }
    }
}

/// Expected stream/codec/duration contract for a completed media encode,
/// checked by [`validate_export_probe`] before the export is reported as
/// success. Text-only exports (SRT/VTT/XMEML/EDL/OTIO) are not media encodes
/// and never reach this check.
#[derive(Clone, Debug, PartialEq)]
struct ExportProbeExpectations {
    /// Expected video codec family of the primary video stream. `None` for
    /// audio-only exports (WAV).
    video_codec: Option<ProbeVideoCodec>,
    /// Expected audio codec family. `None` when the export carries no audio.
    audio_codec: Option<ProbeAudioCodec>,
    /// Expected container duration in seconds (`frames / fps`).
    expected_duration_secs: f64,
    /// Allowed duration drift in seconds (a couple of frame periods).
    duration_tolerance_secs: f64,
}

/// Verify a finished media encode against its request: the expected streams
/// exist, their codecs match the requested encoder (or family), and the
/// container duration is within tolerance of frames/fps. Mirrors the motion
/// path's post-encode probe (`motion.rs` `render_and_encode`); unlike a
/// non-zero-exit check alone, this also catches an ffmpeg child that exits 0
/// while leaving a truncated or corrupt output.
fn validate_export_probe(
    probe: &opentake_media::MediaProbe,
    expectations: &ExportProbeExpectations,
) -> Result<(), String> {
    if let Some(expected_video) = expectations.video_codec {
        if !probe.has_video {
            return Err("output validation failed: no video stream in exported file".to_string());
        }
        match probe.video_codec.as_deref() {
            Some(codec_name) if expected_video.accepts(codec_name) => {}
            Some(codec_name) => {
                return Err(format!(
                    "output validation failed: video codec '{codec_name}' does not match the \
                     requested encoder (expected h264/hevc/prores)"
                ));
            }
            None => {
                return Err("output validation failed: video stream reports no codec".to_string());
            }
        }
    }
    if let Some(expected_audio) = expectations.audio_codec {
        if !probe.has_audio {
            return Err("output validation failed: no audio stream in exported file".to_string());
        }
        match probe.audio_codec.as_deref() {
            Some(codec_name) if expected_audio.accepts(codec_name) => {}
            Some(codec_name) => {
                return Err(format!(
                    "output validation failed: audio codec '{codec_name}' does not match the \
                     requested encoder (expected aac/pcm_s16le)"
                ));
            }
            None => {
                return Err("output validation failed: audio stream reports no codec".to_string());
            }
        }
    }
    let drift = (probe.duration_secs - expectations.expected_duration_secs).abs();
    if drift > expectations.duration_tolerance_secs {
        return Err(format!(
            "output validation failed: exported duration {}s does not match the expected {}s \
             (off by {drift:.3}s)",
            probe.duration_secs, expectations.expected_duration_secs
        ));
    }
    Ok(())
}

/// Fail-closed guard for text-bearing exports: an export whose plan contains
/// text clips must not report success when the rasterizer has no font faces —
/// the composited text would be invisible (background/border only, as
/// `CosmicTextRasterizer` documents). The preview path stays lenient (a
/// preview can simply be re-run); this is export-only.
fn ensure_text_export_fonts(
    has_text_clips: bool,
    rasterizer: &CosmicTextRasterizer,
) -> Result<(), String> {
    if has_text_clips && !rasterizer.has_fonts() {
        return Err(
            "cannot export: no system fonts found on this machine, text clips would render \
             invisible"
                .to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
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

#[cfg(test)]
pub(crate) fn write_wav_s16le_cancellable_to_file(
    samples: &[f32],
    sample_rate: u32,
    file: &mut File,
    cancel: &MediaCancelToken,
    on_progress: Option<&dyn Fn(i32, i32)>,
    checkpoint_hook: Option<&dyn Fn(usize)>,
) -> Result<(), String> {
    write_wav_header(file, samples.len(), sample_rate)?;

    (|| {
        if cancel.checkpoint() {
            return Err(CANCELLED_SENTINEL.to_string());
        }
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

fn write_wav_header(file: &mut File, sample_count: usize, sample_rate: u32) -> Result<(), String> {
    let data_bytes = sample_count
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

    file.set_len(0)
        .map_err(|error| format!("truncate WAV output: {error}"))?;
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("seek WAV output: {error}"))?;
    file.write_all(&header)
        .map_err(|error| format!("write wav header: {error}"))
}

fn metadata_is_symlink_or_reparse(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
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

/// Create/truncate a normal export output relative to the retained parent
/// directory. Unix uses `openat` so a parent rename between directory capture
/// and file creation cannot redirect the output into a replacement directory;
/// Windows keeps the retained directory handle open with delete sharing denied.
#[cfg(unix)]
fn open_output_file_in_parent(path: &Path, parent: &File) -> Result<File, String> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;

    let name = path
        .file_name()
        .ok_or_else(|| "export output has no file name".to_string())?;
    let name = CString::new(name.as_bytes())
        .map_err(|_| "export output contains a NUL byte".to_string())?;
    let flags = libc::O_RDWR | libc::O_CREAT | libc::O_TRUNC | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags, 0o600) };
    if fd < 0 {
        return Err(format!(
            "open export output relative to retained parent: {}",
            io::Error::last_os_error()
        ));
    }
    // SAFETY: `fd` is a fresh descriptor returned by openat and is now owned by
    // the File constructed below.
    let file = unsafe { File::from_raw_fd(fd) };
    let metadata = file
        .metadata()
        .map_err(|error| format!("inspect export output: {error}"))?;
    if metadata_is_symlink_or_reparse(&metadata) || !metadata.is_file() {
        return Err("export output must be a regular file".to_string());
    }
    Ok(file)
}

#[cfg(not(unix))]
fn open_output_file_in_parent(path: &Path, _parent: &File) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x1;
        const FILE_SHARE_WRITE: u32 = 0x2;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options
        .open(path)
        .map_err(|error| format!("open export output: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("inspect export output: {error}"))?;
    if metadata_is_symlink_or_reparse(&metadata) || !metadata.is_file() {
        return Err("export output must be a regular file".to_string());
    }
    Ok(file)
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
            external_cancel: None,
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

    #[test]
    fn denoise_export_uses_shared_processing_owner() {
        let config = opentake_domain::AudioDenoise {
            mode: opentake_domain::DenoiseMode::Voice,
            strength: 0.8,
            preview_enabled: false,
        };
        let input = vec![0.2, -0.1, 0.15, -0.05, 0.1, 0.0, 0.05, 0.05];
        let exported = apply_export_denoise(&input, 1, Some(config), None).expect("export denoise");
        let shared = opentake_media::analysis::denoise_interleaved(
            &input,
            1,
            MIX_SAMPLE_RATE,
            config,
            &MediaCancelToken::new(),
            None,
        )
        .expect("shared denoise");
        assert_eq!(exported, shared);
    }

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
    fn external_cancel_is_seen_by_audio_checkpoint() {
        let external = MediaCancelToken::new();
        assert!(check_audio_cancel_with_external(None, Some(&external)).is_ok());
        external.cancel();
        assert_eq!(
            check_audio_cancel_with_external(None, Some(&external)).unwrap_err(),
            CANCELLED_SENTINEL
        );
    }

    #[test]
    fn export_rejects_ambiguous_cancel_sources() {
        let control = ExportControl::default();
        let external = MediaCancelToken::new();
        assert_eq!(
            validate_export_cancel_sources(Some(&control), Some(&external)).unwrap_err(),
            "export cannot combine control and external cancellation sources"
        );
        assert!(validate_export_cancel_sources(Some(&control), None).is_ok());
        assert!(validate_export_cancel_sources(None, Some(&external)).is_ok());
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
    fn export_commit_active_linearizes_against_concurrent_cancel() {
        for generation in 0..32 {
            let control = std::sync::Arc::new(ExportControl::default());
            let operation_id = format!("race-{generation}");
            let guard = control.try_begin(&operation_id).expect("start export");
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
            let commit_control = std::sync::Arc::clone(&control);
            let cancel_control = std::sync::Arc::clone(&control);
            let commit_barrier = std::sync::Arc::clone(&barrier);
            let cancel_barrier = std::sync::Arc::clone(&barrier);
            let commit_thread = std::thread::spawn(move || {
                commit_barrier.wait();
                commit_control.commit_active()
            });
            let cancel_id = operation_id.clone();
            let cancel_thread = std::thread::spawn(move || {
                cancel_barrier.wait();
                cancel_control.request_cancel(&cancel_id)
            });
            barrier.wait();
            let commit_result = commit_thread.join().expect("commit thread");
            let cancel_result = cancel_thread.join().expect("cancel thread");

            if commit_result.is_ok() {
                assert!(!cancel_result, "cancel must lose after commit linearizes");
            } else {
                assert!(cancel_result, "cancel must win before a rejected commit");
            }
            drop(guard);
            assert!(!control.is_cancelled());
        }
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
    fn resolve_preset_accepts_prores_4444_mov() {
        let preset = resolve_preset(
            ExportCodec::Prores4444,
            ExportQuality::P1080,
            Path::new("/out.mov"),
        )
        .expect("prores 4444 mov should resolve");
        assert_eq!(preset.codec, VideoCodec::ProRes4444);
        assert_eq!(preset.resolution, EncodeResolution::P1080);
    }

    #[test]
    fn prores_4444_export_uses_transparent_clear_color() {
        assert_eq!(
            export_clear_rgba(ExportCodec::Prores4444),
            [0.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(export_clear_rgba(ExportCodec::Prores), [0.0, 0.0, 0.0, 1.0]);
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
    fn export_output_cleanup_removes_active_partial_output_on_drop() {
        let project = tempfile::tempdir().expect("project");
        let output = project.path().join("partial.mp4");
        fs::write(&output, b"partial").expect("partial output");
        let output_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&output)
            .expect("open partial output");

        let mut cleanup = ExportOutputCleanup::new(output.clone(), true).expect("create cleanup");
        cleanup.attach_output(output_file);
        let _encoder_file = cleanup.encoder_file().expect("clone encoder file");
        drop(cleanup);

        assert!(!output.exists());
    }

    #[test]
    fn export_output_cleanup_keeps_successful_output_and_reserved_output() {
        let project = tempfile::tempdir().expect("project");
        let successful = project.path().join("successful.mp4");
        fs::write(&successful, b"complete").expect("successful output");
        let successful_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&successful)
            .expect("open successful output");
        let mut keep = ExportOutputCleanup::new(successful.clone(), true).expect("create cleanup");
        keep.attach_output(successful_file);
        let _encoder_file = keep.encoder_file().expect("clone encoder file");
        keep.verify_visible_identity()
            .expect("verify successful output");
        keep.mark_success();
        drop(keep);
        assert!(successful.exists());

        let reserved = project.path().join("reserved.mp4");
        fs::write(&reserved, b"reserved").expect("reserved output");
        let reserved_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&reserved)
            .expect("open reserved output");
        let mut reserved_cleanup =
            ExportOutputCleanup::new(reserved.clone(), false).expect("create cleanup");
        reserved_cleanup.attach_output(reserved_file);
        drop(reserved_cleanup);
        assert!(reserved.exists());
    }

    #[cfg(unix)]
    #[test]
    fn export_output_cleanup_does_not_remove_a_replaced_visible_path() {
        let project = tempfile::tempdir().expect("project");
        let output = project.path().join("race.mp4");
        let moved = project.path().join("race-original.mp4");
        fs::write(&output, b"original").expect("original output");
        let output_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&output)
            .expect("open original output");
        let mut cleanup = ExportOutputCleanup::new(output.clone(), true).expect("create cleanup");
        cleanup.attach_output(output_file);
        fs::rename(&output, &moved).expect("move original output");
        fs::write(&output, b"replacement").expect("replacement output");
        drop(cleanup);

        assert_eq!(fs::read(&output).expect("read replacement"), b"replacement");
    }

    #[cfg(unix)]
    #[test]
    fn export_output_identity_rejects_a_symlinked_visible_path() {
        let project = tempfile::tempdir().expect("project");
        let output = project.path().join("race.mp4");
        let moved = project.path().join("race-original.mp4");
        fs::write(&output, b"original").expect("original output");
        let output_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&output)
            .expect("open original output");
        let mut cleanup = ExportOutputCleanup::new(output.clone(), true).expect("create cleanup");
        cleanup.attach_output(output_file);
        fs::rename(&output, &moved).expect("move original output");
        std::os::unix::fs::symlink(&moved, &output).expect("replace output with symlink");

        assert!(
            cleanup.verify_visible_identity().is_err(),
            "a symlinked visible output must never be accepted as the retained file"
        );
        drop(cleanup);
        assert!(
            output.exists(),
            "cleanup must not remove the replacement link"
        );
    }

    #[cfg(unix)]
    #[test]
    fn export_output_cleanup_uses_retained_parent_after_parent_swap() {
        let project = tempfile::tempdir().expect("project");
        let parent = project.path().join("parent");
        let moved_parent = project.path().join("parent-original");
        fs::create_dir(&parent).expect("parent");
        let output = parent.join("race.mp4");
        let moved_output = moved_parent.join("race.mp4");
        fs::write(&output, b"original").expect("original output");
        let output_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&output)
            .expect("open original output");
        let mut cleanup = ExportOutputCleanup::new(output.clone(), true).expect("create cleanup");
        cleanup.attach_output(output_file);
        fs::rename(&parent, &moved_parent).expect("move original parent");
        fs::create_dir(&parent).expect("replacement parent");
        fs::write(&output, b"replacement").expect("replacement output");
        drop(cleanup);

        assert_eq!(fs::read(&output).expect("read replacement"), b"replacement");
        assert!(!moved_output.exists(), "retained original must be cleaned");
    }

    #[cfg(unix)]
    #[test]
    fn export_output_creation_stays_in_retained_parent_after_parent_swap() {
        let project = tempfile::tempdir().expect("project");
        let parent = project.path().join("parent");
        let moved_parent = project.path().join("parent-original");
        fs::create_dir(&parent).expect("parent");
        let output = parent.join("race.mp4");
        let moved_output = moved_parent.join("race.mp4");
        let mut cleanup = ExportOutputCleanup::new(output.clone(), true).expect("create cleanup");

        fs::rename(&parent, &moved_parent).expect("move original parent");
        fs::create_dir(&parent).expect("replacement parent");
        let output_file = cleanup
            .open_output_file()
            .expect("create output in retained parent");
        cleanup.attach_output(output_file);
        let _encoder_file = cleanup.encoder_file().expect("clone encoder file");
        drop(cleanup);

        assert!(
            !output.exists(),
            "replacement parent must not receive output"
        );
        assert!(!moved_output.exists(), "retained output must be cleaned");
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
                source_fps: None,
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
                source_fps: None,
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

    // MARK: - Post-encode output validation

    /// Fabricate a probe as ffprobe would report it (stream codecs + a
    /// container duration), so the pure validator is testable without ffprobe.
    fn fabricated_probe(
        video_codec: Option<&str>,
        audio_codec: Option<&str>,
        duration_secs: f64,
    ) -> opentake_media::MediaProbe {
        let mut streams: Vec<serde_json::Value> = Vec::new();
        if let Some(codec) = video_codec {
            streams.push(serde_json::json!({
                "codec_type": "video",
                "codec_name": codec,
                "width": 1280, "height": 720,
                "avg_frame_rate": "30/1",
                "duration": format!("{duration_secs}"),
            }));
        }
        if let Some(codec) = audio_codec {
            streams.push(serde_json::json!({
                "codec_type": "audio",
                "codec_name": codec,
                "channels": 2,
            }));
        }
        opentake_media::parse_probe(&serde_json::json!({
            "streams": streams,
            "format": { "duration": format!("{duration_secs}") },
        }))
    }

    fn h264_aac_expectations() -> ExportProbeExpectations {
        ExportProbeExpectations {
            video_codec: Some(ProbeVideoCodec::H264),
            audio_codec: Some(ProbeAudioCodec::Aac),
            expected_duration_secs: 2.0,
            duration_tolerance_secs: 0.05,
        }
    }

    #[test]
    fn validate_export_probe_accepts_matching_output() {
        let probe = fabricated_probe(Some("h264"), Some("aac"), 2.0);
        assert!(validate_export_probe(&probe, &h264_aac_expectations()).is_ok());
    }

    #[test]
    fn validate_export_probe_rejects_missing_video_stream() {
        let probe = fabricated_probe(None, Some("aac"), 2.0);
        let error = validate_export_probe(&probe, &h264_aac_expectations())
            .expect_err("missing video stream must fail");
        assert!(error.contains("no video stream"), "{error}");
    }

    #[test]
    fn validate_export_probe_rejects_wrong_video_codec() {
        let probe = fabricated_probe(Some("mpeg4"), Some("aac"), 2.0);
        let error = validate_export_probe(&probe, &h264_aac_expectations())
            .expect_err("wrong video codec must fail");
        assert!(error.contains("mpeg4"), "{error}");
        assert!(error.contains("does not match"), "{error}");
    }

    #[test]
    fn validate_export_probe_accepts_hevc_family_name_for_h265() {
        let probe = fabricated_probe(Some("hevc"), Some("aac"), 2.0);
        let expectations = ExportProbeExpectations {
            video_codec: Some(ProbeVideoCodec::H265),
            ..h264_aac_expectations()
        };
        assert!(validate_export_probe(&probe, &expectations).is_ok());
    }

    #[test]
    fn validate_export_probe_accepts_prores_family_name() {
        let probe = fabricated_probe(Some("prores"), Some("pcm_s16le"), 2.0);
        let expectations = ExportProbeExpectations {
            video_codec: Some(ProbeVideoCodec::ProRes),
            audio_codec: Some(ProbeAudioCodec::PcmS16Le),
            ..h264_aac_expectations()
        };
        assert!(validate_export_probe(&probe, &expectations).is_ok());
    }

    #[test]
    fn validate_export_probe_rejects_missing_audio_stream_when_expected() {
        let probe = fabricated_probe(Some("h264"), None, 2.0);
        let error = validate_export_probe(&probe, &h264_aac_expectations())
            .expect_err("missing audio stream must fail");
        assert!(error.contains("no audio stream"), "{error}");
    }

    #[test]
    fn validate_export_probe_rejects_wrong_audio_codec() {
        let probe = fabricated_probe(Some("h264"), Some("mp3"), 2.0);
        let error = validate_export_probe(&probe, &h264_aac_expectations())
            .expect_err("wrong audio codec must fail");
        assert!(error.contains("mp3"), "{error}");
        assert!(error.contains("does not match"), "{error}");
    }

    #[test]
    fn validate_export_probe_rejects_duration_drift_beyond_tolerance() {
        let probe = fabricated_probe(Some("h264"), Some("aac"), 1.0);
        let error = validate_export_probe(&probe, &h264_aac_expectations())
            .expect_err("duration drift must fail");
        assert!(error.contains("duration"), "{error}");
    }

    #[test]
    fn validate_export_probe_tolerates_small_duration_drift() {
        let probe = fabricated_probe(Some("h264"), Some("aac"), 2.02);
        assert!(validate_export_probe(&probe, &h264_aac_expectations()).is_ok());
    }

    #[test]
    fn validate_export_probe_accepts_audio_only_wav_output() {
        let probe = fabricated_probe(None, Some("pcm_s16le"), 0.5);
        let expectations = ExportProbeExpectations {
            video_codec: None,
            audio_codec: Some(ProbeAudioCodec::PcmS16Le),
            expected_duration_secs: 0.5,
            duration_tolerance_secs: 0.05,
        };
        assert!(validate_export_probe(&probe, &expectations).is_ok());
    }

    #[test]
    fn wav_export_probe_reads_real_written_file() {
        if !opentake_media::ffmpeg_status::ffprobe_available() {
            eprintln!("[skip] ffprobe unavailable");
            return;
        }
        let tmp = tempfile::tempdir().expect("temp dir");
        let out = tmp.path().join("out.wav");
        std::fs::write(&out, b"").expect("create wav output");
        write_wav_s16le(&[0.25; 480], 48_000, &out).expect("write wav");
        let probe = opentake_media::probe(&out).expect("probe written wav");
        assert!(probe.has_audio && !probe.has_video);
        assert_eq!(probe.audio_codec.as_deref(), Some("pcm_s16le"));
        assert!((probe.duration_secs - 0.01).abs() < 0.01);
    }

    // MARK: - Fail-closed text export font guard

    #[test]
    fn text_export_fails_closed_when_fonts_absent() {
        let headless = CosmicTextRasterizer::without_system_fonts();
        assert!(!headless.has_fonts());
        let error = ensure_text_export_fonts(true, &headless)
            .expect_err("text-bearing export without fonts must fail");
        assert!(error.contains("no system fonts"), "{error}");
        assert!(error.contains("invisible"), "{error}");
    }

    #[test]
    fn text_export_allows_fontless_run_without_text_clips() {
        let headless = CosmicTextRasterizer::without_system_fonts();
        assert!(ensure_text_export_fonts(false, &headless).is_ok());
    }

    #[test]
    fn text_export_allows_text_clips_when_fonts_available() {
        let rasterizer = CosmicTextRasterizer::new();
        if !rasterizer.has_fonts() {
            eprintln!("[skip] no system fonts on this machine");
            return;
        }
        assert!(ensure_text_export_fonts(true, &rasterizer).is_ok());
    }
}
