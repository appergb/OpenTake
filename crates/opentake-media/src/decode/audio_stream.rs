//! Interleaved (multi-channel) PCM decode for streaming-playback audio
//! (#160 / #63).
//!
//! `pcm.rs`'s [`extract_pcm`](crate::decode::extract_pcm) averages every channel
//! into a mono f32 view (right for transcription/waveform). Playback wants real
//! stereo, so this decodes the audio track and keeps the channels **interleaved**
//! — it deliberately does NOT reuse `raw_to_mono_f32`.
//!
//! This is the one-shot preload form (decode a clip's window up front); the
//! chunked / background streaming form is the remaining half of #160.

use std::io::Read;
use std::path::Path;

use crate::decode::pcm::{PcmFormat, PcmSpec};
use crate::error::{MediaError, Result};
use crate::ff;
use crate::probe;

/// Build the ffmpeg args to decode the first audio track to raw interleaved PCM
/// on stdout, honoring an optional `[lo, hi)` absolute-seconds range. Mirrors
/// `pcm::pcm_args` but is kept self-contained (no shared mono path).
fn interleaved_args(path: &Path, spec: &PcmSpec, range: Option<(f64, f64)>) -> Vec<String> {
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
    args.push(
        match spec.format {
            PcmFormat::F32 => "f32le",
            PcmFormat::S16Le => "s16le",
        }
        .to_string(),
    );
    args.push("-".into());
    args
}

/// Convert raw interleaved PCM bytes to interleaved f32, **without** folding
/// channels (the playback mixer pans/sums per channel later).
fn raw_to_interleaved_f32(bytes: &[u8], spec: &PcmSpec) -> Vec<f32> {
    match spec.format {
        PcmFormat::F32 => bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect(),
        PcmFormat::S16Le => bytes
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
            .collect(),
    }
}

/// Decode `path`'s first audio track to interleaved f32 at the requested spec
/// (channels preserved). `range` is an absolute-seconds `[lo, hi)` window. Errors
/// with `NoTrack("audio", …)` when the file has no audio stream.
pub fn decode_pcm_interleaved(
    path: &Path,
    spec: &PcmSpec,
    range: Option<(f64, f64)>,
) -> Result<Vec<f32>> {
    // Cheap guard: confirm an audio track exists before spawning the decoder.
    if let Ok(p) = probe::probe(path) {
        if !p.has_audio {
            return Err(MediaError::no_track("audio", path));
        }
    }

    let mut child = ff::ffmpeg()
        .args(interleaved_args(path, spec, range))
        .spawn()
        .map_err(|e| MediaError::Ffmpeg(format!("spawn: {e}")))?;

    // Read raw PCM straight off stdout (the event parser is tuned for video).
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

    Ok(raw_to_interleaved_f32(&raw, spec))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(channels: u16, format: PcmFormat) -> PcmSpec {
        PcmSpec {
            sample_rate: 48_000,
            channels,
            format,
        }
    }

    #[test]
    fn args_request_interleaved_stereo_f32le_with_range() {
        let args = interleaved_args(
            Path::new("/a.mp4"),
            &spec(2, PcmFormat::F32),
            Some((1.0, 2.0)),
        );
        assert!(args.windows(2).any(|w| w == ["-ac", "2"]));
        assert!(args.windows(2).any(|w| w == ["-ar", "48000"]));
        assert!(args.windows(2).any(|w| w == ["-f", "f32le"]));
        assert!(args.iter().any(|a| a == "-vn"));
        let ss = args.iter().position(|a| a == "-ss").unwrap();
        assert_eq!(args[ss + 1], "1.000000");
        let to = args.iter().position(|a| a == "-to").unwrap();
        assert_eq!(args[to + 1], "2.000000");
        assert_eq!(args.last().unwrap(), "-");
    }

    #[test]
    fn args_have_no_seek_without_range() {
        let args = interleaved_args(Path::new("/a.mp4"), &spec(2, PcmFormat::F32), None);
        assert!(!args.iter().any(|a| a == "-ss"));
    }

    #[test]
    fn f32_interleaved_keeps_channels_unfolded() {
        // Stereo: (L=1.0 R=-1.0), (L=0.5 R=0.0) — NOT averaged to mono.
        let mut bytes = Vec::new();
        for v in [1.0f32, -1.0, 0.5, 0.0] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        let out = raw_to_interleaved_f32(&bytes, &spec(2, PcmFormat::F32));
        assert_eq!(out, vec![1.0, -1.0, 0.5, 0.0]);
    }

    #[test]
    fn s16_interleaved_converts_to_unit_floats_unfolded() {
        let mut bytes = Vec::new();
        for v in [0i16, 16384, -32768, 0] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        let out = raw_to_interleaved_f32(&bytes, &spec(2, PcmFormat::S16Le));
        assert_eq!(out.len(), 4);
        assert!((out[0] - 0.0).abs() < 1e-6);
        assert!((out[1] - 0.5).abs() < 1e-3);
        assert!((out[2] + 1.0).abs() < 1e-6);
    }

    #[test]
    fn trailing_partial_sample_is_ignored() {
        // 5 bytes of f32 = 1 full sample + 1 stray byte → 1 sample.
        let out = raw_to_interleaved_f32(&[0, 0, 0, 63, 7], &spec(1, PcmFormat::F32));
        assert_eq!(out.len(), 1);
    }
}
