//! Audio-track PCM extraction via the system ffmpeg CLI. Replaces upstream
//! `Transcription.extractAudioTrack` (`Transcription.swift:203-280`), which
//! decoded the first audio track to 16 kHz mono s16le.
//!
//! The canonical output for transcription is **16 kHz mono f32**; the buffer
//! always carries an f32 mono view for downstream consumers (whisper). The
//! `PcmFormat` selects the on-wire sample format ffmpeg emits.
//!
//! The arg builder ([`pcm_args`]) and the s16→f32 conversion are pure and
//! unit-tested; the extraction itself requires ffmpeg.

use std::io::Read;
use std::path::Path;
use std::process::{ChildStderr, ChildStdout, ExitStatus};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::cancel::MediaCancelToken;
use crate::error::{MediaError, Result};
use crate::ff;
use crate::probe;

/// On-wire PCM sample format requested from ffmpeg.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PcmFormat {
    S16Le,
    F32,
}

impl PcmFormat {
    /// ffmpeg `-f` rawvideo-equivalent codec/format token.
    fn ffmpeg_fmt(self) -> &'static str {
        match self {
            PcmFormat::S16Le => "s16le",
            PcmFormat::F32 => "f32le",
        }
    }
    pub(crate) fn bytes_per_sample(self) -> usize {
        match self {
            PcmFormat::S16Le => 2,
            PcmFormat::F32 => 4,
        }
    }
}

const CHILD_POLL_INTERVAL: Duration = Duration::from_millis(5);
const STDERR_DETAIL_LIMIT: usize = 64 * 1024;

struct PipeReaders {
    stdout: JoinHandle<Result<StdoutRead>>,
    stderr: JoinHandle<Result<Vec<u8>>>,
}

struct StdoutRead {
    bytes: Vec<u8>,
    exceeded_cap: bool,
    total_read: usize,
}

fn audio_buffer_too_large(detail: impl std::fmt::Display) -> MediaError {
    MediaError::Decode(format!("audio_buffer_too_large: {detail}"))
}

fn allocation_error(detail: impl std::fmt::Display) -> MediaError {
    MediaError::Decode(format!("audio_allocation_failed: {detail}"))
}

fn expected_pcm_bytes(path: &Path, spec: &PcmSpec, range: Option<(f64, f64)>) -> Result<usize> {
    if spec.sample_rate == 0 || spec.channels == 0 {
        return Err(MediaError::Decode(
            "PCM sample rate and channel count must be non-zero".to_string(),
        ));
    }
    let duration_secs = match range {
        Some((lo, hi)) => (hi - lo.max(0.0)).max(0.0),
        None => {
            let media = probe::probe(path)?;
            if !media.has_audio {
                return Err(MediaError::no_track("audio", path));
            }
            media.duration_secs
        }
    };
    if !duration_secs.is_finite() {
        return Err(audio_buffer_too_large("non-finite duration"));
    }
    let frames = (duration_secs * f64::from(spec.sample_rate)).ceil();
    if frames > usize::MAX as f64 {
        return Err(audio_buffer_too_large("PCM frame count exceeds usize"));
    }
    let frame_bytes = usize::from(spec.channels)
        .checked_mul(spec.format.bytes_per_sample())
        .ok_or_else(|| audio_buffer_too_large("PCM frame byte count overflow"))?;
    (frames as usize)
        .checked_mul(frame_bytes)
        .ok_or_else(|| audio_buffer_too_large("PCM output byte count overflow"))
}

fn read_stdout(
    mut stdout: ChildStdout,
    cap: usize,
    cancel: MediaCancelToken,
) -> Result<StdoutRead> {
    cancel.reader_started();
    let result = (|| {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(cap)
            .map_err(|error| allocation_error(format!("stdout reserve {cap}: {error}")))?;
        let mut exceeded_cap = false;
        let mut total_read = 0_usize;
        let mut chunk = [0_u8; 64 * 1024];
        loop {
            let read = stdout
                .read(&mut chunk)
                .map_err(|error| MediaError::Ffmpeg(format!("read stdout: {error}")))?;
            if read == 0 {
                break;
            }
            total_read = total_read.saturating_add(read);
            let remaining = cap.saturating_sub(bytes.len());
            let retained = remaining.min(read);
            bytes.extend_from_slice(&chunk[..retained]);
            exceeded_cap |= retained < read;
        }
        Ok(StdoutRead {
            bytes,
            exceeded_cap,
            total_read,
        })
    })();
    cancel.reader_finished();
    result
}

fn read_stderr(mut stderr: ChildStderr, cancel: MediaCancelToken) -> Result<Vec<u8>> {
    cancel.reader_started();
    let result = (|| {
        let mut detail = Vec::new();
        detail
            .try_reserve_exact(STDERR_DETAIL_LIMIT)
            .map_err(|error| allocation_error(format!("stderr reserve: {error}")))?;
        let mut chunk = [0_u8; 8 * 1024];
        loop {
            let read = stderr
                .read(&mut chunk)
                .map_err(|error| MediaError::Ffmpeg(format!("read stderr: {error}")))?;
            if read == 0 {
                break;
            }
            let retained = STDERR_DETAIL_LIMIT.saturating_sub(detail.len()).min(read);
            detail.extend_from_slice(&chunk[..retained]);
        }
        Ok(detail)
    })();
    cancel.reader_finished();
    result
}

fn join_reader<T>(handle: JoinHandle<Result<T>>, name: &str) -> Result<T> {
    handle
        .join()
        .map_err(|_| MediaError::Ffmpeg(format!("{name} reader panicked")))?
}

fn join_pipes(readers: PipeReaders) -> Result<(StdoutRead, Vec<u8>)> {
    let stdout = join_reader(readers.stdout, "stdout")?;
    let stderr = join_reader(readers.stderr, "stderr")?;
    Ok((stdout, stderr))
}

fn wait_for_pcm_child(
    child: &mut ffmpeg_sidecar::child::FfmpegChild,
    readers: PipeReaders,
    cancel: &MediaCancelToken,
) -> Result<(ExitStatus, StdoutRead, Vec<u8>)> {
    loop {
        if cancel.checkpoint() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = join_pipes(readers);
            return Err(MediaError::Cancelled);
        }
        if let Some(status) = child.as_inner_mut().try_wait().map_err(MediaError::Io)? {
            let (stdout, stderr) = join_pipes(readers)?;
            if cancel.is_cancelled() {
                return Err(MediaError::Cancelled);
            }
            return Ok((status, stdout, stderr));
        }
        thread::sleep(CHILD_POLL_INTERVAL);
    }
}

/// Requested PCM layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PcmSpec {
    pub sample_rate: u32,
    pub channels: u16,
    pub format: PcmFormat,
}

/// Decoded PCM. `samples_f32` is always a mono f32 view (downstream-friendly);
/// when the requested spec has multiple channels they are averaged into mono.
#[derive(Clone, Debug, PartialEq)]
pub struct PcmBuffer {
    pub spec: PcmSpec,
    pub samples_f32: Vec<f32>,
}

impl PcmBuffer {
    /// Duration in seconds implied by the mono sample count and sample rate.
    pub fn duration_secs(&self) -> f64 {
        if self.spec.sample_rate == 0 {
            return 0.0;
        }
        self.samples_f32.len() as f64 / self.spec.sample_rate as f64
    }
}

/// Build the ffmpeg arg list for decoding the first audio track to raw PCM on
/// stdout, honoring an optional `[lo, hi)` absolute-seconds range.
fn pcm_args(path: &Path, spec: &PcmSpec, range: Option<(f64, f64)>) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    if let Some((lo, hi)) = range {
        args.push("-ss".into());
        args.push(format!("{:.6}", lo.max(0.0)));
        args.push("-to".into());
        args.push(format!("{hi:.6}"));
    }
    args.push("-i".into());
    args.push(path.to_string_lossy().into_owned());
    args.push("-vn".into()); // drop video
    args.push("-ac".into());
    args.push(spec.channels.to_string());
    args.push("-ar".into());
    args.push(spec.sample_rate.to_string());
    args.push("-f".into());
    args.push(spec.format.ffmpeg_fmt().into());
    args.push("-".into());
    args
}

/// Convert interleaved raw PCM bytes to mono f32, averaging `channels`.
fn raw_to_mono_f32(bytes: &[u8], spec: &PcmSpec) -> Vec<f32> {
    let bps = spec.format.bytes_per_sample();
    let ch = spec.channels.max(1) as usize;
    let frame_bytes = bps * ch;
    if frame_bytes == 0 {
        return Vec::new();
    }
    let frames = bytes.len() / frame_bytes;
    let mut out = Vec::with_capacity(frames);
    for f in 0..frames {
        let base = f * frame_bytes;
        let mut sum = 0.0f32;
        for c in 0..ch {
            let off = base + c * bps;
            let s = match spec.format {
                PcmFormat::S16Le => {
                    let v = i16::from_le_bytes([bytes[off], bytes[off + 1]]);
                    v as f32 / 32768.0
                }
                PcmFormat::F32 => {
                    f32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
                }
            };
            sum += s;
        }
        out.push(sum / ch as f32);
    }
    out
}

/// Decode `path`'s first audio track to the requested PCM spec, returning a mono
/// f32 buffer. `range` is an absolute-seconds `[lo, hi)` window. Errors with
/// `NoTrack("audio", …)` when the file has no audio stream.
pub fn extract_pcm(path: &Path, spec: &PcmSpec, range: Option<(f64, f64)>) -> Result<PcmBuffer> {
    extract_pcm_cancellable(path, spec, range, &MediaCancelToken::new())
}

pub fn extract_pcm_cancellable(
    path: &Path,
    spec: &PcmSpec,
    range: Option<(f64, f64)>,
    cancel: &MediaCancelToken,
) -> Result<PcmBuffer> {
    let raw = decode_raw_pcm_cancellable(path, spec, range, cancel)?;
    let samples = raw_to_mono_f32(&raw, spec);
    Ok(PcmBuffer {
        spec: *spec,
        samples_f32: samples,
    })
}

pub(super) fn decode_raw_pcm_cancellable(
    path: &Path,
    spec: &PcmSpec,
    range: Option<(f64, f64)>,
    cancel: &MediaCancelToken,
) -> Result<Vec<u8>> {
    if cancel.is_cancelled() {
        return Err(MediaError::Cancelled);
    }
    let probed = if path.is_file() {
        let media = probe::probe(path)?;
        if !media.has_audio {
            return Err(MediaError::no_track("audio", path));
        }
        Some(media)
    } else {
        None
    };
    let effective_range = match range {
        Some(range) => Some(range),
        None => {
            let media = probed.unwrap_or(probe::probe(path)?);
            Some((0.0, media.duration_secs))
        }
    };
    let expected_bytes = expected_pcm_bytes(path, spec, effective_range)?;
    let frame_bytes = usize::from(spec.channels)
        .checked_mul(spec.format.bytes_per_sample())
        .ok_or_else(|| audio_buffer_too_large("PCM frame byte count overflow"))?;
    let reader_cap = expected_bytes
        .checked_add(frame_bytes)
        .ok_or_else(|| audio_buffer_too_large("PCM reader cap overflow"))?;

    let mut child = ff::ffmpeg()
        .args(pcm_args(path, spec, effective_range))
        .spawn()
        .map_err(|e| MediaError::Ffmpeg(format!("spawn: {e}")))?;
    cancel.child_spawned();
    let stdout = child
        .take_stdout()
        .ok_or_else(|| MediaError::Ffmpeg("FFmpeg stdout pipe missing".to_string()))?;
    let stderr = child
        .take_stderr()
        .ok_or_else(|| MediaError::Ffmpeg("FFmpeg stderr pipe missing".to_string()))?;
    let stdout_cancel = cancel.clone();
    let stderr_cancel = cancel.clone();
    let readers = PipeReaders {
        stdout: thread::Builder::new()
            .name("opentake-pcm-stdout".to_string())
            .spawn(move || read_stdout(stdout, reader_cap, stdout_cancel))
            .map_err(|error| MediaError::Ffmpeg(format!("spawn stdout reader: {error}")))?,
        stderr: thread::Builder::new()
            .name("opentake-pcm-stderr".to_string())
            .spawn(move || read_stderr(stderr, stderr_cancel))
            .map_err(|error| MediaError::Ffmpeg(format!("spawn stderr reader: {error}")))?,
    };
    let (status, stdout, stderr) = wait_for_pcm_child(&mut child, readers, cancel)?;
    if stdout.exceeded_cap {
        return Err(audio_buffer_too_large(format!(
            "FFmpeg stdout read {} bytes, exceeding {reader_cap}",
            stdout.total_read
        )));
    }
    let raw = stdout.bytes;
    if !status.success() && raw.is_empty() {
        let detail = String::from_utf8_lossy(&stderr);
        if !detail.trim().is_empty() {
            return Err(MediaError::Ffmpeg(format!(
                "decode exited {status}: {}",
                detail.trim()
            )));
        }
        return Err(MediaError::no_track("audio", path));
    }
    // ffmpeg can exit 0 with empty stdout when metadata says audio exists but
    // no decodable samples: treat as no audio track so the waveform cache
    // isn't poisoned with all-1.0 silence.
    if raw.is_empty() {
        return Err(MediaError::no_track("audio", path));
    }

    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::process::Command;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use crate::MediaCancelToken;

    fn f32_mono_spec() -> PcmSpec {
        PcmSpec {
            sample_rate: 48_000,
            channels: 1,
            format: PcmFormat::F32,
        }
    }

    #[test]
    fn pre_cancelled_pcm_decode_does_not_spawn_ffmpeg() {
        let cancel = MediaCancelToken::new();
        cancel.cancel();

        let error = extract_pcm_cancellable(
            Path::new("/definitely/missing/pre-cancelled.wav"),
            &f32_mono_spec(),
            Some((0.0, 1.0)),
            &cancel,
        )
        .expect_err("pre-cancelled decode must fail before path probing or spawn");

        assert!(matches!(error, MediaError::Cancelled));
        assert_eq!(cancel.spawned_child_count(), 0);
    }

    #[test]
    fn cancelling_running_pcm_decode_kills_child_and_reaps_readers() {
        assert!(
            crate::ff::ffmpeg_available(),
            "required cancellation test needs a runnable FFmpeg"
        );
        let temp = tempfile::tempdir().expect("create cancellation fixture directory");
        let fifo = temp.path().join("blocking-input.wav");
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
                extract_pcm_cancellable(&fifo, &f32_mono_spec(), Some((0.0, 30.0)), &worker_cancel);
            done_tx.send(result).expect("publish decoder result");
        });

        let deadline = Instant::now() + Duration::from_secs(5);
        while cancel.spawned_child_count() == 0 && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(
            cancel.spawned_child_count(),
            1,
            "the test must cancel a live FFmpeg child"
        );
        cancel.cancel();

        let result = done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("cancelled decode must kill FFmpeg and join both pipe readers");
        assert!(matches!(result, Err(MediaError::Cancelled)));
        worker.join().expect("decoder worker must be reaped");
        assert_eq!(cancel.active_reader_count(), 0);
    }

    #[test]
    fn duration_from_mono_samples() {
        let b = PcmBuffer {
            spec: PcmSpec {
                sample_rate: 16_000,
                channels: 1,
                format: PcmFormat::F32,
            },
            samples_f32: vec![0.0; 32_000],
        };
        assert!((b.duration_secs() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn pcm_args_range_emits_ss_and_to() {
        let spec = PcmSpec {
            sample_rate: 16_000,
            channels: 1,
            format: PcmFormat::F32,
        };
        let args = pcm_args(Path::new("/a.mp4"), &spec, Some((1.5, 4.0)));
        let ss = args.iter().position(|a| a == "-ss").unwrap();
        assert_eq!(args[ss + 1], "1.500000");
        let to = args.iter().position(|a| a == "-to").unwrap();
        assert_eq!(args[to + 1], "4.000000");
        assert!(args.windows(2).any(|w| w == ["-ar", "16000"]));
        assert!(args.windows(2).any(|w| w == ["-ac", "1"]));
        assert!(args.windows(2).any(|w| w == ["-f", "f32le"]));
        assert!(args.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn pcm_args_no_range_has_no_seek() {
        let spec = PcmSpec {
            sample_rate: 48_000,
            channels: 2,
            format: PcmFormat::S16Le,
        };
        let args = pcm_args(Path::new("/a.mp4"), &spec, None);
        assert!(!args.iter().any(|a| a == "-ss"));
        assert!(args.windows(2).any(|w| w == ["-f", "s16le"]));
        assert!(args.windows(2).any(|w| w == ["-ac", "2"]));
    }

    #[test]
    fn raw_s16_mono_converts_to_unit_floats() {
        let spec = PcmSpec {
            sample_rate: 16_000,
            channels: 1,
            format: PcmFormat::S16Le,
        };
        // samples: 0, 16384 (~0.5), -32768 (-1.0)
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0i16.to_le_bytes());
        bytes.extend_from_slice(&16384i16.to_le_bytes());
        bytes.extend_from_slice(&(-32768i16).to_le_bytes());
        let out = raw_to_mono_f32(&bytes, &spec);
        assert_eq!(out.len(), 3);
        assert!((out[0] - 0.0).abs() < 1e-6);
        assert!((out[1] - 0.5).abs() < 1e-3);
        assert!((out[2] + 1.0).abs() < 1e-6);
    }

    #[test]
    fn raw_stereo_f32_averages_channels() {
        let spec = PcmSpec {
            sample_rate: 16_000,
            channels: 2,
            format: PcmFormat::F32,
        };
        // frame0: L=1.0 R=0.0 → 0.5 ; frame1: L=-0.5 R=0.5 → 0.0
        let mut bytes = Vec::new();
        for v in [1.0f32, 0.0, -0.5, 0.5] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        let out = raw_to_mono_f32(&bytes, &spec);
        assert_eq!(out.len(), 2);
        assert!((out[0] - 0.5).abs() < 1e-6);
        assert!((out[1] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn raw_partial_trailing_frame_ignored() {
        let spec = PcmSpec {
            sample_rate: 16_000,
            channels: 1,
            format: PcmFormat::S16Le,
        };
        // 3 bytes = 1 full s16 sample + 1 stray byte → 1 sample.
        let out = raw_to_mono_f32(&[0, 0, 7], &spec);
        assert_eq!(out.len(), 1);
    }
}
