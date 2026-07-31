//! Single/batch frame decode via the system ffmpeg CLI. Replaces upstream's
//! `AVAssetImageGenerator` (`MediaVisualCache`, `FrameSampler`, `MediaAsset`).
//!
//! `decode_frame_at` seeks near a timestamp (allowing a tolerance to land on the
//! nearest decodable frame) and returns the frame as packed RGBA8.
//! `decode_frames_at` decodes a batch of ascending timestamps, de-duplicating
//! frames whose actual time does not advance (upstream's `t > lastTime` rule).
//!
//! The *scaling math* ([`fit_within`]) is a pure function and unit-tested; the
//! ffmpeg invocation requires the binary and is covered by ignore-by-default
//! integration tests.

use std::path::Path;
use std::thread;
use std::time::Duration;

use ffmpeg_sidecar::event::FfmpegEvent;
use image::ImageEncoder;

use crate::cancel::MediaCancelToken;
use crate::error::{MediaError, Result};
use crate::ff;
use crate::frame::RgbaFrame;

const FRAME_CHILD_POLL_INTERVAL: Duration = Duration::from_millis(5);

/// A frame decode request.
#[derive(Clone, Debug)]
pub struct FrameRequest {
    pub time_secs: f64,
    /// Upper bound box; the frame is scaled down to fit while preserving aspect
    /// ratio (never enlarged). `(0, 0)` disables scaling.
    pub max_size: (u32, u32),
    /// Seek tolerance: ffmpeg seeks to `time - tolerance` and decodes forward.
    pub tolerance_secs: f64,
    /// Apply container rotation (display matrix). Default true.
    pub apply_rotation: bool,
}

impl Default for FrameRequest {
    fn default() -> Self {
        FrameRequest {
            time_secs: 0.0,
            max_size: (0, 0),
            tolerance_secs: 1.0,
            apply_rotation: true,
        }
    }
}

/// Source-frame reconstruction policy used when a timeline requests frames at
/// a different rate than the decoded asset. Optical flow is a deterministic
/// local motion-compensated path; it never implies a cloud/model dependency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameInterpolationMode {
    Nearest,
    Blend,
    OpticalFlow,
}

/// Explicit recovery behavior when optical flow is unavailable on the current
/// device/runtime. The caller chooses quality, determinism, or fail-closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameInterpolationFallback {
    Nearest,
    Blend,
    Error,
}

/// One target-rate sample mapped back into the source-frame interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameRateSample {
    pub timestamp_secs: f64,
    pub source_frame: u64,
    pub next_source_frame: u64,
    pub source_alpha: f64,
}

/// Result of one pair interpolation, including the effective mode after an
/// explicit unsupported-device fallback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameInterpolationResult {
    pub frame: RgbaFrame,
    pub mode_used: FrameInterpolationMode,
}

/// Map a finite source sequence onto a target frame rate while preserving both
/// endpoint timestamps exactly. Interior timestamps follow the target-rate
/// grid; the final sample is pinned to the source's final presentation time.
pub fn convert_frame_rate(
    source_frame_count: u64,
    source_fps: f64,
    target_fps: f64,
) -> Result<Vec<FrameRateSample>> {
    if source_frame_count == 0 {
        return Err(MediaError::Decode(
            "source_frame_count must be greater than zero".to_string(),
        ));
    }
    if !source_fps.is_finite() || source_fps <= 0.0 {
        return Err(MediaError::Decode(
            "source_fps must be finite and greater than zero".to_string(),
        ));
    }
    if !target_fps.is_finite() || target_fps <= 0.0 {
        return Err(MediaError::Decode(
            "target_fps must be finite and greater than zero".to_string(),
        ));
    }
    if source_frame_count == 1 {
        return Ok(vec![FrameRateSample {
            timestamp_secs: 0.0,
            source_frame: 0,
            next_source_frame: 0,
            source_alpha: 0.0,
        }]);
    }

    let source_last = source_frame_count - 1;
    let duration_secs = source_last as f64 / source_fps;
    let target_intervals = (duration_secs * target_fps).round().max(1.0) as u64;
    let mut samples = Vec::with_capacity(target_intervals as usize + 1);
    for output_frame in 0..=target_intervals {
        let timestamp_secs = if output_frame == target_intervals {
            duration_secs
        } else {
            (output_frame as f64 / target_fps).min(duration_secs)
        };
        let source_position = (timestamp_secs * source_fps).clamp(0.0, source_last as f64);
        let source_frame = source_position.floor() as u64;
        let next_source_frame = source_frame.saturating_add(1).min(source_last);
        let source_alpha = if source_frame == next_source_frame {
            0.0
        } else {
            source_position - source_frame as f64
        };
        samples.push(FrameRateSample {
            timestamp_secs,
            source_frame,
            next_source_frame,
            source_alpha,
        });
    }
    Ok(samples)
}

/// Interpolate two equal-size RGBA frames at `alpha` in `[0, 1]`.
///
/// The optical-flow path estimates a deterministic global luminance motion
/// vector, warps both endpoints toward the requested instant, then blends the
/// aligned pixels. This traditional local path is intentionally model-free and
/// provides a stable baseline for preview/export parity.
pub fn interpolate_frame_pair(
    first: &RgbaFrame,
    last: &RgbaFrame,
    alpha: f64,
    requested: FrameInterpolationMode,
    fallback: FrameInterpolationFallback,
    optical_flow_available: bool,
) -> Result<FrameInterpolationResult> {
    if first.width != last.width
        || first.height != last.height
        || first.rgba.len() != last.rgba.len()
    {
        return Err(MediaError::Decode(
            "interpolation frames must have identical dimensions".to_string(),
        ));
    }
    if !alpha.is_finite() {
        return Err(MediaError::Decode(
            "interpolation alpha must be finite".to_string(),
        ));
    }

    let mode_used = if requested == FrameInterpolationMode::OpticalFlow && !optical_flow_available {
        match fallback {
            FrameInterpolationFallback::Nearest => FrameInterpolationMode::Nearest,
            FrameInterpolationFallback::Blend => FrameInterpolationMode::Blend,
            FrameInterpolationFallback::Error => {
                return Err(MediaError::Decode(
                    "optical-flow interpolation is unavailable and fallback is Error".to_string(),
                ));
            }
        }
    } else {
        requested
    };

    let alpha = alpha.clamp(0.0, 1.0);
    let frame = if alpha == 0.0 {
        first.clone()
    } else if alpha == 1.0 {
        last.clone()
    } else {
        match mode_used {
            FrameInterpolationMode::Nearest => {
                if alpha < 0.5 {
                    first.clone()
                } else {
                    last.clone()
                }
            }
            FrameInterpolationMode::Blend => blend_frames(first, last, alpha),
            FrameInterpolationMode::OpticalFlow => optical_flow_frame(first, last, alpha),
        }
    };

    Ok(FrameInterpolationResult { frame, mode_used })
}

fn blend_frames(first: &RgbaFrame, last: &RgbaFrame, alpha: f64) -> RgbaFrame {
    let rgba = first
        .rgba
        .iter()
        .zip(&last.rgba)
        .map(|(&a, &b)| lerp_channel(a, b, alpha))
        .collect();
    RgbaFrame::new(first.width, first.height, rgba)
}

fn optical_flow_frame(first: &RgbaFrame, last: &RgbaFrame, alpha: f64) -> RgbaFrame {
    let first_center = luminance_centroid(first);
    let last_center = luminance_centroid(last);
    let Some(((first_x, first_y), (last_x, last_y))) = first_center.zip(last_center) else {
        return blend_frames(first, last, alpha);
    };
    let motion_x = last_x - first_x;
    let motion_y = last_y - first_y;
    let mut rgba = vec![0; first.rgba.len()];
    for y in 0..first.height {
        for x in 0..first.width {
            let x = x as f64;
            let y = y as f64;
            let from_first = sample_bilinear(first, x - alpha * motion_x, y - alpha * motion_y);
            let from_last = sample_bilinear(
                last,
                x + (1.0 - alpha) * motion_x,
                y + (1.0 - alpha) * motion_y,
            );
            let offset = ((y as u32 * first.width + x as u32) * 4) as usize;
            for channel in 0..4 {
                rgba[offset + channel] =
                    lerp_channel(from_first[channel], from_last[channel], alpha);
            }
        }
    }
    RgbaFrame::new(first.width, first.height, rgba)
}

fn luminance_centroid(frame: &RgbaFrame) -> Option<(f64, f64)> {
    let mut weighted_x = 0.0;
    let mut weighted_y = 0.0;
    let mut total = 0.0;
    for y in 0..frame.height {
        for x in 0..frame.width {
            let offset = ((y * frame.width + x) * 4) as usize;
            let r = frame.rgba[offset] as f64;
            let g = frame.rgba[offset + 1] as f64;
            let b = frame.rgba[offset + 2] as f64;
            let a = frame.rgba[offset + 3] as f64 / 255.0;
            let weight = (0.2126 * r + 0.7152 * g + 0.0722 * b) * a;
            weighted_x += x as f64 * weight;
            weighted_y += y as f64 * weight;
            total += weight;
        }
    }
    (total > f64::EPSILON).then_some((weighted_x / total, weighted_y / total))
}

fn sample_bilinear(frame: &RgbaFrame, x: f64, y: f64) -> [u8; 4] {
    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let fx = x - x0 as f64;
    let fy = y - y0 as f64;
    let mut out = [0; 4];
    for (channel, value) in out.iter_mut().enumerate() {
        let p00 = sample_channel(frame, x0, y0, channel);
        let p10 = sample_channel(frame, x0 + 1, y0, channel);
        let p01 = sample_channel(frame, x0, y0 + 1, channel);
        let p11 = sample_channel(frame, x0 + 1, y0 + 1, channel);
        let top = p00 + (p10 - p00) * fx;
        let bottom = p01 + (p11 - p01) * fx;
        *value = (top + (bottom - top) * fy).round().clamp(0.0, 255.0) as u8;
    }
    out
}

fn sample_channel(frame: &RgbaFrame, x: i64, y: i64, channel: usize) -> f64 {
    if x < 0 || y < 0 || x >= frame.width as i64 || y >= frame.height as i64 {
        return if channel == 3 { 255.0 } else { 0.0 };
    }
    let offset = ((y as u32 * frame.width + x as u32) * 4) as usize;
    frame.rgba[offset + channel] as f64
}

fn lerp_channel(first: u8, last: u8, alpha: f64) -> u8 {
    (first as f64 + (last as f64 - first as f64) * alpha)
        .round()
        .clamp(0.0, 255.0) as u8
}

/// Scale `(w, h)` down to fit within `max` while preserving aspect ratio. Never
/// enlarges. A zero in either `max` dimension disables that bound. Mirrors
/// `AVAssetImageGenerator.maximumSize` semantics ("not larger than this box,
/// keep aspect ratio"). Output dimensions are at least 1.
pub fn fit_within(w: u32, h: u32, max: (u32, u32)) -> (u32, u32) {
    if w == 0 || h == 0 {
        return (w.max(1), h.max(1));
    }
    let (mw, mh) = max;
    let mut scale = 1.0f64;
    if mw > 0 {
        scale = scale.min(mw as f64 / w as f64);
    }
    if mh > 0 {
        scale = scale.min(mh as f64 / h as f64);
    }
    if scale >= 1.0 {
        return (w, h); // never enlarge
    }
    let nw = ((w as f64 * scale).round() as u32).max(1);
    let nh = ((h as f64 * scale).round() as u32).max(1);
    (nw, nh)
}

/// Build the ffmpeg arg list for decoding one frame to rawvideo RGBA on stdout.
/// Pure so the exact CLI contract is testable.
fn frame_args(path: &Path, req: &FrameRequest) -> Vec<String> {
    let seek = (req.time_secs - req.tolerance_secs).max(0.0);
    let mut args: Vec<String> = Vec::new();
    // Fast input seek to just before the target keyframe window.
    args.push("-ss".into());
    args.push(format!("{seek:.6}"));
    args.push("-i".into());
    args.push(path.to_string_lossy().into_owned());
    // Grab a single frame at/after the seek point.
    args.push("-frames:v".into());
    args.push("1".into());

    let mut filters: Vec<String> = Vec::new();
    if req.apply_rotation {
        // Honor the display matrix when transposing (ffmpeg applies it via the
        // autorotate behavior; the scale filter runs after rotation).
        // Nothing to add here — ffmpeg autorotates by default for the decoder.
    }
    if req.max_size.0 > 0 || req.max_size.1 > 0 {
        // Downscale-only, keep aspect: scale='min(iw,MW)':-2 style. We use
        // force_original_aspect_ratio=decrease against the box.
        let mw = if req.max_size.0 > 0 {
            req.max_size.0.to_string()
        } else {
            "iw".to_string()
        };
        let mh = if req.max_size.1 > 0 {
            req.max_size.1.to_string()
        } else {
            "ih".to_string()
        };
        filters.push(format!(
            "scale=w={mw}:h={mh}:force_original_aspect_ratio=decrease"
        ));
    }
    if !filters.is_empty() {
        args.push("-vf".into());
        args.push(filters.join(","));
    }
    args.push("-pix_fmt".into());
    args.push("rgba".into());
    args.push("-f".into());
    args.push("rawvideo".into());
    args.push("-".into());
    args
}

/// Decode the frame at/after `req.time_secs`, returning `(actual_secs, frame)`.
pub fn decode_frame_at(path: &Path, req: &FrameRequest) -> Result<(f64, RgbaFrame)> {
    decode_frame_at_cancellable(path, req, &MediaCancelToken::new())
}

pub fn decode_frame_at_cancellable(
    path: &Path,
    req: &FrameRequest,
    cancel: &MediaCancelToken,
) -> Result<(f64, RgbaFrame)> {
    if cancel.is_cancelled() {
        return Err(MediaError::Cancelled);
    }
    let mut child = ff::ffmpeg()
        .args(frame_args(path, req))
        .spawn()
        .map_err(|e| MediaError::Ffmpeg(format!("spawn: {e}")))?;
    cancel.child_spawned();

    let iter = match child.iter() {
        Ok(iter) => iter,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(MediaError::Ffmpeg(format!("iter: {error}")));
        }
    };
    let reader_cancel = cancel.clone();
    let requested_time = req.time_secs;
    let reader = match thread::Builder::new()
        .name("opentake-frame-events".to_string())
        .spawn(move || {
            reader_cancel.reader_started();
            let result = iter
                .filter_map(|event| match event {
                    FfmpegEvent::OutputFrame(frame) if frame.width > 0 && frame.height > 0 => {
                        let actual = requested_time.max(frame.timestamp as f64);
                        Some((
                            actual,
                            RgbaFrame::new(frame.width, frame.height, frame.data),
                        ))
                    }
                    _ => None,
                })
                .next();
            reader_cancel.reader_finished();
            result
        }) {
        Ok(reader) => reader,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(MediaError::Ffmpeg(format!(
                "spawn frame event reader: {error}"
            )));
        }
    };

    loop {
        if cancel.checkpoint() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(MediaError::Cancelled);
        }
        if reader.is_finished() {
            // The iterator stops after the first frame. Explicitly terminate
            // the single-frame command before joining so no producer remains
            // blocked trying to publish a later event into a dropped receiver.
            let _ = child.kill();
            let _ = child.wait();
            let result = reader
                .join()
                .map_err(|_| MediaError::Ffmpeg("frame event reader panicked".to_string()))?;
            if cancel.is_cancelled() {
                return Err(MediaError::Cancelled);
            }
            return result
                .ok_or_else(|| MediaError::Decode(format!("no frame at {:.3}s", req.time_secs)));
        }
        match child.as_inner_mut().try_wait() {
            Ok(Some(_)) => {
                let result = reader
                    .join()
                    .map_err(|_| MediaError::Ffmpeg("frame event reader panicked".to_string()))?;
                if cancel.is_cancelled() {
                    return Err(MediaError::Cancelled);
                }
                return result.ok_or_else(|| {
                    MediaError::Decode(format!("no frame at {:.3}s", req.time_secs))
                });
            }
            Ok(None) => thread::sleep(FRAME_CHILD_POLL_INTERVAL),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(MediaError::Io(error));
            }
        }
    }
}

/// Decode one cancellable frame and encode it as PNG bytes without publishing
/// a cache file. Project-scoped prewarm jobs stage these bytes and let their
/// epoch guard perform the final atomic rename.
pub fn decode_frame_png_cancellable(
    path: &Path,
    req: &FrameRequest,
    cancel: &MediaCancelToken,
) -> Result<(f64, Vec<u8>)> {
    let (actual, frame) = decode_frame_at_cancellable(path, req, cancel)?;
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            &frame.rgba,
            frame.width,
            frame.height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|error| MediaError::Encode(format!("png: {error}")))?;
    Ok((actual, bytes))
}

pub fn decode_frames_at_cancellable(
    path: &Path,
    times_secs: &[f64],
    base: &FrameRequest,
    cancel: &MediaCancelToken,
) -> Vec<Result<(f64, RgbaFrame)>> {
    let mut out = Vec::with_capacity(times_secs.len());
    let mut last_time = f64::NEG_INFINITY;
    for &time in times_secs {
        if cancel.checkpoint() {
            out.push(Err(MediaError::Cancelled));
            break;
        }
        let request = FrameRequest {
            time_secs: time,
            ..base.clone()
        };
        match decode_frame_at_cancellable(path, &request, cancel) {
            Ok((actual, frame)) if actual > last_time => {
                last_time = actual;
                out.push(Ok((actual, frame)));
            }
            Ok(_) | Err(MediaError::Decode(_)) => {}
            Err(error) => out.push(Err(error)),
        }
    }
    out
}

/// Decode a batch of ascending `times_secs`. De-duplicates frames whose decoded
/// timestamp does not strictly advance past the previous one (`t > lastTime`).
/// Returns `(actual_secs, frame)` pairs in ascending actual time. Frames that
/// fail to decode are skipped.
pub fn decode_frames_at(
    path: &Path,
    times_secs: &[f64],
    base: &FrameRequest,
) -> Vec<Result<(f64, RgbaFrame)>> {
    let mut out = Vec::with_capacity(times_secs.len());
    let mut last_time = f64::NEG_INFINITY;
    for &t in times_secs {
        let req = FrameRequest {
            time_secs: t,
            ..base.clone()
        };
        match decode_frame_at(path, &req) {
            Ok((actual, frame)) => {
                if actual <= last_time {
                    continue; // duplicate of an already-emitted keyframe
                }
                last_time = actual;
                out.push(Ok((actual, frame)));
            }
            Err(MediaError::Decode(_)) => continue, // skip undecodable point
            Err(e) => out.push(Err(e)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::process::Command;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    // --- fit_within: pure scaling math ---

    #[test]
    fn cancelling_frame_decode_with_no_iterator_events_kills_child_and_joins_reader() {
        assert!(
            crate::ff::ffmpeg_available(),
            "required cancellation test needs a runnable FFmpeg"
        );
        let temp = tempfile::tempdir().expect("create frame cancellation fixture directory");
        let fifo = temp.path().join("blocking-video-input");
        let status = Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("spawn mkfifo");
        assert!(
            status.success(),
            "mkfifo must create a blocking media input"
        );

        let cancel = MediaCancelToken::new();
        let worker_cancel = cancel.clone();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let result =
                decode_frame_at_cancellable(&fifo, &FrameRequest::default(), &worker_cancel);
            done_tx.send(result).expect("publish frame decoder result");
        });

        let deadline = Instant::now() + Duration::from_secs(5);
        while cancel.spawned_child_count() == 0 && Instant::now() < deadline {
            std::thread::yield_now();
        }
        let spawned = cancel.spawned_child_count();
        cancel.cancel();
        assert_eq!(spawned, 1, "the test must cancel a live FFmpeg child");

        let result = done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("cancellation must not wait for the first iterator event");
        assert!(matches!(result, Err(MediaError::Cancelled)));
        worker.join().expect("frame decoder worker must be reaped");
        assert_eq!(cancel.active_reader_count(), 0);
    }

    #[test]
    fn fit_within_no_box_keeps_size() {
        assert_eq!(fit_within(1920, 1080, (0, 0)), (1920, 1080));
    }

    #[test]
    fn fit_within_never_enlarges() {
        // box bigger than image → unchanged.
        assert_eq!(fit_within(100, 50, (1000, 1000)), (100, 50));
    }

    #[test]
    fn fit_within_scales_down_keeping_aspect() {
        // 1920x1080 into 120x68 box → width-limited: scale ~0.0625 → 120x68.
        let (w, h) = fit_within(1920, 1080, (120, 68));
        assert_eq!(w, 120);
        assert_eq!(h, 68);
    }

    #[test]
    fn fit_within_portrait_into_square_box() {
        // 1080x1920 into 512x512 → height-limited: scale 512/1920 → 288x512.
        let (w, h) = fit_within(1080, 1920, (512, 512));
        assert_eq!(h, 512);
        assert_eq!(w, 288);
    }

    #[test]
    fn fit_within_single_dim_box() {
        // only width bound (120), height unbounded.
        let (w, h) = fit_within(600, 300, (120, 0));
        assert_eq!(w, 120);
        assert_eq!(h, 60);
    }

    #[test]
    fn fit_within_min_one_pixel() {
        let (w, h) = fit_within(10000, 1, (5, 5));
        assert!(w >= 1 && h >= 1);
    }

    #[test]
    fn fit_within_zero_input() {
        assert_eq!(fit_within(0, 0, (10, 10)), (1, 1));
    }

    // --- frame_args: CLI contract ---

    #[test]
    fn frame_args_seek_is_time_minus_tolerance_clamped() {
        let req = FrameRequest {
            time_secs: 5.0,
            tolerance_secs: 1.0,
            ..Default::default()
        };
        let args = frame_args(Path::new("/x.mp4"), &req);
        let ss = args.iter().position(|a| a == "-ss").unwrap();
        assert_eq!(args[ss + 1], "4.000000");
        // clamps to 0
        let req0 = FrameRequest {
            time_secs: 0.5,
            tolerance_secs: 2.0,
            ..Default::default()
        };
        let args0 = frame_args(Path::new("/x.mp4"), &req0);
        let ss0 = args0.iter().position(|a| a == "-ss").unwrap();
        assert_eq!(args0[ss0 + 1], "0.000000");
    }

    #[test]
    fn frame_args_request_rgba_rawvideo_one_frame() {
        let args = frame_args(Path::new("/x.mp4"), &FrameRequest::default());
        assert!(args.windows(2).any(|w| w == ["-pix_fmt", "rgba"]));
        assert!(args.windows(2).any(|w| w == ["-f", "rawvideo"]));
        assert!(args.windows(2).any(|w| w == ["-frames:v", "1"]));
        assert_eq!(args.last().unwrap(), "-");
    }

    #[test]
    fn frame_args_adds_scale_filter_only_when_boxed() {
        let plain = frame_args(Path::new("/x.mp4"), &FrameRequest::default());
        assert!(!plain.iter().any(|a| a == "-vf"));

        let boxed = frame_args(
            Path::new("/x.mp4"),
            &FrameRequest {
                max_size: (120, 68),
                ..Default::default()
            },
        );
        let vf = boxed.iter().position(|a| a == "-vf").unwrap();
        assert!(boxed[vf + 1].contains("force_original_aspect_ratio=decrease"));
        assert!(boxed[vf + 1].contains("w=120"));
        assert!(boxed[vf + 1].contains("h=68"));
    }
}
