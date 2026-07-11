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
