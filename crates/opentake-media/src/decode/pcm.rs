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
use std::sync::Arc;
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
const PCM_CONVERT_CHUNK_FRAMES: usize = 8 * 1024;
const PCM_PROGRESS_TOTAL: usize = 4_000;
const PCM_DECODE_PROGRESS_END: usize = 3_000;

/// Byte-level progress reported while FFmpeg streams decoded PCM to stdout.
pub type PcmProgressCallback = Arc<dyn Fn(usize, usize) + Send + Sync>;

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
    progress_total: usize,
    cancel: MediaCancelToken,
    progress: Option<PcmProgressCallback>,
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
            if let Some(report) = &progress {
                report(total_read.min(progress_total), progress_total);
            }
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
    // Join both handles before propagating either failure. Returning after the
    // first failed join would detach the other pipe reader and could keep the
    // FFmpeg pipe (and its allocation) alive beyond this decode request.
    let stdout = join_reader(readers.stdout, "stdout");
    let stderr = join_reader(readers.stderr, "stderr");
    match (stdout, stderr) {
        (Ok(stdout), Ok(stderr)) => Ok((stdout, stderr)),
        (Err(error), _) | (_, Err(error)) => Err(error),
    }
}

fn terminate_child(child: &mut ffmpeg_sidecar::child::FfmpegChild) {
    let _ = child.kill();
    let _ = child.wait();
}

fn wait_for_pcm_child(
    child: &mut ffmpeg_sidecar::child::FfmpegChild,
    readers: PipeReaders,
    cancel: &MediaCancelToken,
) -> Result<(ExitStatus, StdoutRead, Vec<u8>)> {
    loop {
        if cancel.checkpoint() {
            terminate_child(child);
            let _ = join_pipes(readers);
            return Err(MediaError::Cancelled);
        }
        let status = match child.as_inner_mut().try_wait() {
            Ok(status) => status,
            Err(error) => {
                terminate_child(child);
                let _ = join_pipes(readers);
                return Err(MediaError::Io(error));
            }
        };
        if let Some(status) = status {
            let (stdout, stderr) = join_pipes(readers)?;
            if cancel.is_cancelled() {
                return Err(MediaError::Cancelled);
            }
            return Ok((status, stdout, stderr));
        }
        thread::sleep(CHILD_POLL_INTERVAL);
    }
}

fn validate_pcm_output(
    path: &Path,
    status: ExitStatus,
    stdout: StdoutRead,
    stderr: Vec<u8>,
    reader_cap: usize,
) -> Result<Vec<u8>> {
    if stdout.exceeded_cap {
        return Err(audio_buffer_too_large(format!(
            "FFmpeg stdout read {} bytes, exceeding {reader_cap}",
            stdout.total_read
        )));
    }
    if !status.success() {
        let detail = String::from_utf8_lossy(&stderr);
        let suffix = if detail.trim().is_empty() {
            String::new()
        } else {
            format!(": {}", detail.trim())
        };
        return Err(MediaError::Ffmpeg(format!(
            "decode exited {status}{suffix}"
        )));
    }
    if stdout.bytes.is_empty() {
        return Err(MediaError::no_track("audio", path));
    }
    Ok(stdout.bytes)
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
#[cfg(test)]
fn raw_to_mono_f32(bytes: &[u8], spec: &PcmSpec) -> Result<Vec<f32>> {
    raw_to_mono_f32_cancellable(bytes, spec, &MediaCancelToken::new(), None, None)
}

fn raw_to_mono_f32_cancellable(
    bytes: &[u8],
    spec: &PcmSpec,
    cancel: &MediaCancelToken,
    progress: Option<&(dyn Fn(usize, usize) + Send + Sync)>,
    checkpoint_hook: Option<&dyn Fn(usize)>,
) -> Result<Vec<f32>> {
    let bps = spec.format.bytes_per_sample();
    let ch = spec.channels.max(1) as usize;
    let frame_bytes = bps * ch;
    if frame_bytes == 0 {
        return Ok(Vec::new());
    }
    let frames = bytes.len() / frame_bytes;
    let mut out = Vec::new();
    out.try_reserve_exact(frames)
        .map_err(|error| allocation_error(format!("mono f32 reserve {frames}: {error}")))?;
    for f in 0..frames {
        if f.is_multiple_of(PCM_CONVERT_CHUNK_FRAMES) {
            if let Some(hook) = checkpoint_hook {
                hook(f);
            }
            if cancel.checkpoint() {
                return Err(MediaError::Cancelled);
            }
            if let Some(report) = progress {
                let converted =
                    f.saturating_mul(PCM_PROGRESS_TOTAL - PCM_DECODE_PROGRESS_END) / frames.max(1);
                report(PCM_DECODE_PROGRESS_END + converted, PCM_PROGRESS_TOTAL);
            }
        }
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
    if cancel.checkpoint() {
        return Err(MediaError::Cancelled);
    }
    if let Some(report) = progress {
        report(PCM_PROGRESS_TOTAL, PCM_PROGRESS_TOTAL);
    }
    Ok(out)
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
    extract_pcm_cancellable_with_progress(path, spec, range, cancel, None)
}

pub fn extract_pcm_cancellable_with_progress(
    path: &Path,
    spec: &PcmSpec,
    range: Option<(f64, f64)>,
    cancel: &MediaCancelToken,
    progress: Option<PcmProgressCallback>,
) -> Result<PcmBuffer> {
    let decode_progress = progress.as_ref().map(|report| {
        let report = Arc::clone(report);
        Arc::new(move |done: usize, total: usize| {
            let mapped = done
                .min(total.max(1))
                .saturating_mul(PCM_DECODE_PROGRESS_END)
                / total.max(1);
            report(mapped, PCM_PROGRESS_TOTAL);
        }) as PcmProgressCallback
    });
    let raw = decode_raw_pcm_cancellable(path, spec, range, cancel, decode_progress)?;
    let samples = raw_to_mono_f32_cancellable(&raw, spec, cancel, progress.as_deref(), None)?;
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
    progress: Option<PcmProgressCallback>,
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
    let stdout = match child.take_stdout() {
        Some(stdout) => stdout,
        None => {
            terminate_child(&mut child);
            return Err(MediaError::Ffmpeg("FFmpeg stdout pipe missing".to_string()));
        }
    };
    let stderr = match child.take_stderr() {
        Some(stderr) => stderr,
        None => {
            terminate_child(&mut child);
            return Err(MediaError::Ffmpeg("FFmpeg stderr pipe missing".to_string()));
        }
    };
    let stdout_cancel = cancel.clone();
    let stderr_cancel = cancel.clone();
    let stdout_reader = match thread::Builder::new()
        .name("opentake-pcm-stdout".to_string())
        .spawn(move || read_stdout(stdout, reader_cap, expected_bytes, stdout_cancel, progress))
    {
        Ok(reader) => reader,
        Err(error) => {
            terminate_child(&mut child);
            return Err(MediaError::Ffmpeg(format!("spawn stdout reader: {error}")));
        }
    };
    let stderr_reader = match thread::Builder::new()
        .name("opentake-pcm-stderr".to_string())
        .spawn(move || read_stderr(stderr, stderr_cancel))
    {
        Ok(reader) => reader,
        Err(error) => {
            terminate_child(&mut child);
            let _ = join_reader(stdout_reader, "stdout");
            return Err(MediaError::Ffmpeg(format!("spawn stderr reader: {error}")));
        }
    };
    let readers = PipeReaders {
        stdout: stdout_reader,
        stderr: stderr_reader,
    };
    let (status, stdout, stderr) = wait_for_pcm_child(&mut child, readers, cancel)?;
    validate_pcm_output(path, status, stdout, stderr, reader_cap)
}

/// Decoded PCM chunk for streaming playback (#160). Unlike [`PcmBuffer`] (which
/// averages channels to mono for transcription/waveform), `PcmChunk` PRESERVES
/// the multi-channel layout as interleaved f32 — stereo stays stereo — so the
/// Web Audio API can re-wrap it as an `AudioBuffer` without re-decoding.
///
/// `samples` is interleaved: `frame f, channel c` lives at
/// `samples[f * channels + c]`. `frame_count` is the per-channel sample count
/// (so `samples.len() == frame_count * channels as usize`).
#[derive(Clone, Debug, PartialEq)]
pub struct PcmChunk {
    /// Interleaved f32 samples (length = `frame_count * channels`).
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    /// Per-channel sample count (independent of channel count).
    pub frame_count: usize,
}

impl PcmChunk {
    /// Duration in seconds implied by `frame_count` and `sample_rate`.
    pub fn duration_secs(&self) -> f64 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.frame_count as f64 / self.sample_rate as f64
    }
}

/// Build the ffmpeg arg list for a chunked seek+duration decode (`-ss`/`-t`)
/// to raw f32le on stdout. Mirrors [`pcm_args`] but uses `-t` (duration) instead
/// of `-to` (end), matching the streaming-chunk pattern: seek to `start_time_sec`
/// and read exactly `duration_sec` more.
fn pcm_chunk_args(
    path: &Path,
    sample_rate: u32,
    channels: u16,
    start_time_sec: f64,
    duration_sec: f64,
) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    // Seek BEFORE `-i` for fast keyframe-accurate seek (ffmpeg's input-seeking
    // path), which is what chunked streaming wants: it skips the decode cost
    // for the discarded prefix. Same placement as `pcm_args`'s `-ss`.
    args.push("-ss".into());
    args.push(format!("{:.6}", start_time_sec.max(0.0)));
    args.push("-t".into());
    args.push(format!("{:.6}", duration_sec.max(0.0)));
    args.push("-i".into());
    args.push(path.to_string_lossy().into_owned());
    args.push("-vn".into()); // drop video
    args.push("-ac".into());
    args.push(channels.to_string());
    args.push("-ar".into());
    args.push(sample_rate.to_string());
    args.push("-f".into());
    args.push("f32le".into()); // chunk pipeline is f32-only (no s16 path)
    args.push("-".into());
    args
}

/// Convert interleaved raw f32le bytes to interleaved f32 samples. Channels
/// are preserved (no mono averaging) — the chunk pipeline keeps stereo.
fn raw_f32_to_interleaved(bytes: &[u8], channels: u16) -> (Vec<f32>, usize) {
    let ch = channels.max(1) as usize;
    let total_samples = bytes.len() / 4;
    let frame_count = total_samples / ch;
    let mut out = Vec::with_capacity(total_samples);
    for i in 0..total_samples {
        let off = i * 4;
        out.push(f32::from_le_bytes([
            bytes[off],
            bytes[off + 1],
            bytes[off + 2],
            bytes[off + 3],
        ]));
    }
    (out, frame_count)
}

/// Decode a chunk of `media_path`'s first audio track to interleaved f32,
/// preserving the multi-channel layout (stereo stays stereo). `start_time_sec`
/// is the seek position; `duration_sec` is the chunk length. Defaults match the
/// streaming pipeline: 48 kHz, stereo, 5 s chunks (#160).
///
/// Uses ffmpeg with `-ss` (input seek) and `-t` (duration) for efficient
/// chunked extraction — same spawn/read/wait/error pattern as [`extract_pcm`].
/// Errors with `NoTrack("audio", …)` when the file has no audio stream.
pub fn extract_pcm_chunk(
    media_path: &str,
    start_time_sec: f64,
    duration_sec: f64,
    target_sample_rate: u32,
    channels: u16,
) -> Result<PcmChunk> {
    let path = Path::new(media_path);
    // Cheap guard: confirm an audio track exists before spawning the decoder.
    if let Ok(p) = probe::probe(path) {
        if !p.has_audio {
            return Err(MediaError::no_track("audio", path));
        }
    }

    let mut child = ff::ffmpeg()
        .args(pcm_chunk_args(
            path,
            target_sample_rate,
            channels,
            start_time_sec,
            duration_sec,
        ))
        .spawn()
        .map_err(|e| MediaError::Ffmpeg(format!("spawn: {e}")))?;

    // Read raw f32le straight off stdout.
    let mut raw = Vec::new();
    if let Some(mut stdout) = child.take_stdout() {
        stdout
            .read_to_end(&mut raw)
            .map_err(|e| MediaError::Ffmpeg(format!("read stdout: {e}")))?;
    }
    let status = child.wait().map_err(MediaError::Io)?;
    if !status.success() && raw.is_empty() {
        return Err(MediaError::no_track("audio", path));
    }
    // ffmpeg can exit 0 with empty stdout when metadata says audio exists but
    // no decodable samples: treat as no audio track (same guard as `extract_pcm`).
    if raw.is_empty() {
        return Err(MediaError::no_track("audio", path));
    }

    let (samples, frame_count) = raw_f32_to_interleaved(&raw, channels);
    Ok(PcmChunk {
        samples,
        sample_rate: target_sample_rate,
        channels,
        frame_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
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

    fn write_silence_wav(path: &Path, sample_rate: u32, samples: usize) {
        let data_len = samples.checked_mul(2).expect("wav data length") as u32;
        let mut wav = Vec::with_capacity(44 + data_len as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        wav.resize(44 + data_len as usize, 0);
        std::fs::write(path, wav).expect("write wav fixture");
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
    fn pcm_decode_reports_non_terminal_progress_for_multiple_stdout_chunks() {
        assert!(
            crate::ff::ffmpeg_available(),
            "required progress test needs a runnable FFmpeg"
        );
        let temp = tempfile::tempdir().expect("create progress fixture directory");
        let input = temp.path().join("two-seconds.wav");
        write_silence_wav(&input, 48_000, 96_000);
        let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let callback_observed = Arc::clone(&observed);
        let progress: PcmProgressCallback = Arc::new(move |done, total| {
            callback_observed
                .lock()
                .expect("progress lock")
                .push((done, total));
        });

        let pcm = extract_pcm_cancellable_with_progress(
            &input,
            &f32_mono_spec(),
            Some((0.0, 2.0)),
            &MediaCancelToken::new(),
            Some(progress),
        )
        .expect("decode progress fixture");

        let observed = observed.lock().expect("progress lock");
        assert_eq!(pcm.samples_f32.len(), 96_000);
        assert!(
            observed.len() > 1,
            "large decode must report multiple chunks"
        );
        assert!(observed.iter().any(|(done, total)| done < total));
        assert_eq!(
            observed.last(),
            Some(&(PCM_PROGRESS_TOTAL, PCM_PROGRESS_TOTAL))
        );
        assert!(observed.windows(2).all(|pair| pair[0].0 <= pair[1].0));
    }

    #[cfg(unix)]
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

    #[cfg(windows)]
    #[test]
    fn windows_cancelling_running_pcm_child_reaps_both_pipe_readers() {
        assert!(
            crate::ff::ffmpeg_available(),
            "required cancellation test needs a runnable FFmpeg"
        );
        let cancel = MediaCancelToken::new();
        let mut child = crate::ff::ffmpeg()
            .args([
                "-re",
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=48000:cl=mono",
                "-t",
                "30",
                "-f",
                "f32le",
                "-",
            ])
            .spawn()
            .expect("spawn blocking PCM FFmpeg");
        cancel.child_spawned();
        let stdout = child.take_stdout().expect("PCM stdout");
        let stderr = child.take_stderr().expect("PCM stderr");
        let stdout_cancel = cancel.clone();
        let stderr_cancel = cancel.clone();
        let readers = PipeReaders {
            stdout: std::thread::spawn(move || {
                read_stdout(stdout, 1024 * 1024, 1024 * 1024, stdout_cancel, None)
            }),
            stderr: std::thread::spawn(move || read_stderr(stderr, stderr_cancel)),
        };
        let worker_cancel = cancel.clone();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let result = wait_for_pcm_child(&mut child, readers, &worker_cancel);
            let reaped = child
                .as_inner_mut()
                .try_wait()
                .expect("inspect cancelled PCM child")
                .is_some();
            done_tx
                .send((result.map(|_| ()), reaped))
                .expect("publish PCM cancellation");
        });

        let deadline = Instant::now() + Duration::from_secs(5);
        while cancel.active_reader_count() < 2 && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(cancel.active_reader_count(), 2);
        cancel.cancel();
        let (result, reaped) = done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("cancelled PCM wait must return promptly");

        assert!(matches!(result, Err(MediaError::Cancelled)));
        assert!(reaped, "cancelled PCM child must be killed and waited");
        worker.join().expect("PCM cancellation worker joins");
        assert_eq!(cancel.active_reader_count(), 0);
    }

    #[test]
    fn cancellation_inside_raw_conversion_stops_the_actual_sample_loop() {
        let spec = f32_mono_spec();
        let frames = PCM_CONVERT_CHUNK_FRAMES * 4;
        let raw = vec![0_u8; frames * spec.format.bytes_per_sample()];
        let cancel = MediaCancelToken::new();
        let worker_cancel = cancel.clone();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let hook = move |_frame: usize| {
                entered_tx.send(()).expect("conversion checkpoint entered");
                release_rx.recv().expect("release conversion checkpoint");
            };
            let result =
                raw_to_mono_f32_cancellable(&raw, &spec, &worker_cancel, None, Some(&hook));
            done_tx.send(result).expect("publish conversion result");
        });

        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("actual conversion loop reached its checkpoint");
        cancel.cancel();
        release_tx.send(()).expect("release conversion loop");
        let result = done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("conversion cancellation must return promptly");

        assert!(matches!(result, Err(MediaError::Cancelled)));
        assert_eq!(cancel.checkpoint_count(), 1);
        worker.join().expect("conversion worker joins");
    }

    #[test]
    fn reader_failure_still_joins_the_other_started_reader() {
        let stdout = std::thread::spawn(|| {
            Err(MediaError::Ffmpeg(
                "deterministic stdout read failure".to_string(),
            ))
        });
        let (stderr_entered_tx, stderr_entered_rx) = mpsc::channel();
        let (stderr_release_tx, stderr_release_rx) = mpsc::channel();
        let stderr = std::thread::spawn(move || {
            stderr_entered_tx.send(()).expect("stderr reader entered");
            stderr_release_rx.recv().expect("release stderr reader");
            Ok(Vec::new())
        });
        let (done_tx, done_rx) = mpsc::channel();
        let joiner = std::thread::spawn(move || {
            done_tx
                .send(join_pipes(PipeReaders { stdout, stderr }))
                .expect("publish join result");
        });
        stderr_entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("stderr reader started");

        let early = done_rx.recv_timeout(Duration::from_millis(100)).ok();
        stderr_release_tx.send(()).expect("release stderr reader");
        let returned_early = early.is_some();
        let result = match early {
            Some(result) => result,
            None => done_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("join waits for both readers"),
        };
        joiner.join().expect("join coordinator");

        assert!(
            !returned_early,
            "reader failure must not detach the other reader"
        );
        assert!(matches!(result, Err(MediaError::Ffmpeg(_))));
    }

    #[cfg(unix)]
    #[test]
    fn nonzero_ffmpeg_exit_with_partial_stdout_is_a_hard_error() {
        let status = Command::new("sh")
            .args(["-c", "exit 7"])
            .status()
            .expect("obtain deterministic nonzero status");
        let stdout = StdoutRead {
            bytes: vec![1, 2, 3, 4],
            exceeded_cap: false,
            total_read: 4,
        };

        let error = validate_pcm_output(
            Path::new("/partial.wav"),
            status,
            stdout,
            b"decoder failed after producing bytes".to_vec(),
            8,
        )
        .expect_err("partial stdout must never mask a nonzero FFmpeg exit");

        assert!(matches!(error, MediaError::Ffmpeg(_)));
        assert!(error.to_string().contains("decoder failed"));
    }

    #[test]
    fn pcm_chunk_duration_from_interleaved_stereo() {
        // 2ch, 48k, 2s → frame_count = 96_000, samples.len = 192_000.
        let c = PcmChunk {
            samples: vec![0.0; 192_000],
            sample_rate: 48_000,
            channels: 2,
            frame_count: 96_000,
        };
        assert!((c.duration_secs() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn pcm_chunk_duration_handles_zero_rate() {
        let c = PcmChunk {
            samples: vec![0.0; 100],
            sample_rate: 0,
            channels: 2,
            frame_count: 50,
        };
        assert_eq!(c.duration_secs(), 0.0);
    }

    #[test]
    fn pcm_chunk_args_emits_ss_and_t() {
        let args = pcm_chunk_args(Path::new("/a.mp4"), 48_000, 2, 1.5, 5.0);
        let ss = args.iter().position(|a| a == "-ss").unwrap();
        assert_eq!(args[ss + 1], "1.500000");
        let t = args.iter().position(|a| a == "-t").unwrap();
        assert_eq!(args[t + 1], "5.000000");
        assert!(args.windows(2).any(|w| w == ["-ar", "48000"]));
        assert!(args.windows(2).any(|w| w == ["-ac", "2"]));
        assert!(args.windows(2).any(|w| w == ["-f", "f32le"]));
        assert!(args.iter().any(|a| a == "-vn"));
        assert!(!args.iter().any(|a| a == "-to"));
    }

    #[test]
    fn pcm_chunk_args_clamps_negative_seek() {
        let args = pcm_chunk_args(Path::new("/a.mp4"), 48_000, 2, -1.0, 5.0);
        let ss = args.iter().position(|a| a == "-ss").unwrap();
        assert_eq!(args[ss + 1], "0.000000");
    }

    #[test]
    fn raw_f32_interleaved_preserves_stereo_channels() {
        // frame0: L=1.0 R=0.0 ; frame1: L=-0.5 R=0.5 → no averaging.
        let mut bytes = Vec::new();
        for v in [1.0f32, 0.0, -0.5, 0.5] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        let (samples, frames) = raw_f32_to_interleaved(&bytes, 2);
        assert_eq!(samples.len(), 4);
        assert_eq!(frames, 2);
        assert_eq!(samples, vec![1.0, 0.0, -0.5, 0.5]);
    }

    #[test]
    fn raw_f32_interleaved_mono_passthrough() {
        let mut bytes = Vec::new();
        for v in [0.25f32, -0.75, 0.5] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        let (samples, frames) = raw_f32_to_interleaved(&bytes, 1);
        assert_eq!(samples, vec![0.25, -0.75, 0.5]);
        assert_eq!(frames, 3);
    }

    #[test]
    fn raw_f32_interleaved_partial_trailing_frame_ignored() {
        // 9 bytes = 2 full f32 samples + 1 stray byte → 2 samples.
        let bytes = [0u8, 0, 0, 0, 0, 0, 0x80, 0x3f, 7];
        let (samples, frames) = raw_f32_to_interleaved(&bytes, 1);
        assert_eq!(samples.len(), 2);
        assert_eq!(frames, 2);
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
        let out = raw_to_mono_f32(&bytes, &spec).unwrap();
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
        let out = raw_to_mono_f32(&bytes, &spec).unwrap();
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
        let out = raw_to_mono_f32(&[0, 0, 7], &spec).unwrap();
        assert_eq!(out.len(), 1);
    }
}
