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
    fn bytes_per_sample(self) -> usize {
        match self {
            PcmFormat::S16Le => 2,
            PcmFormat::F32 => 4,
        }
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
    // Cheap guard: confirm an audio track exists before spawning the decoder.
    if let Ok(p) = probe::probe(path) {
        if !p.has_audio {
            return Err(MediaError::no_track("audio", path));
        }
    }

    let mut child = ff::ffmpeg()
        .args(pcm_args(path, spec, range))
        .spawn()
        .map_err(|e| MediaError::Ffmpeg(format!("spawn: {e}")))?;

    // Read raw PCM straight off stdout (don't route through the event parser,
    // which is tuned for video frames).
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

    let samples = raw_to_mono_f32(&raw, spec);
    Ok(PcmBuffer {
        spec: *spec,
        samples_f32: samples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
