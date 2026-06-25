//! Waveform generation: ffmpeg audio decode → mono f32 → RMS downsample →
//! normalized `0=loud, 1=silence` buckets, with an optional `.waveform` disk
//! cache.
//!
//! Replaces upstream's `DSWaveformImage` dependency (`MediaVisualCache.swift`).
//! The count formula and normalization are byte-for-byte intent-compatible; the
//! exact bucket *values* are visually equivalent but not bit-identical to the
//! third-party analyzer (SPEC §4.3 — waveform is a UI affordance, not a
//! frame-level edit quantity).
//!
//! Decoding goes through the same `ffmpeg` CLI backend as probe/thumbnail/PCM
//! extraction (`crate::extract_pcm`) rather than a separate pure-Rust decoder:
//! a dedicated decoder only covered a subset of containers/codecs (e.g. it could
//! not decode the audio track inside many `.mov` files or non-AAC codecs), so a
//! clip whose `media_ref` pointed at such a source rendered with NO waveform
//! while its thumbnail/probe (ffmpeg) worked fine. Sharing one backend makes the
//! waveform succeed for everything ffmpeg can read.

mod dsp;
pub mod store;

pub use dsp::{
    rms_downsample_normalized, waveform_sample_count, BUCKETS_PER_SECOND, MAX_BUCKETS, MIN_BUCKETS,
};

use std::path::Path;

use crate::cache_key::{file_identity_key, KEY_HEX_LEN};
use crate::error::Result;
use crate::{extract_pcm, PcmFormat, PcmSpec};

/// Sample rate for waveform decode. The exact rate is immaterial — the signal is
/// RMS-downsampled to a fixed bucket count derived from duration — so a single
/// modest mono rate keeps the decode cheap.
const WAVEFORM_SAMPLE_RATE: u32 = 22_050;

/// Generate normalized waveform buckets for `path` (no caching).
/// Bucket count follows [`waveform_sample_count`]. Decodes the first audio track
/// to mono f32 via ffmpeg; errors (propagated) when there is no audio track.
pub fn waveform(path: &Path, duration_secs: f64) -> Result<Vec<f32>> {
    let spec = PcmSpec {
        sample_rate: WAVEFORM_SAMPLE_RATE,
        channels: 1,
        format: PcmFormat::F32,
    };
    let pcm = extract_pcm(path, &spec, None)?;
    let count = waveform_sample_count(duration_secs);
    Ok(rms_downsample_normalized(&pcm.samples_f32, count))
}

/// Like [`waveform`] but reads/writes the `.waveform` disk cache under
/// `<cache_root>/MediaVisualCache/<key>.waveform`.
pub fn waveform_cached(cache_root: &Path, path: &Path, duration_secs: f64) -> Result<Vec<f32>> {
    if let Some(key) = file_identity_key(path, KEY_HEX_LEN) {
        if let Some(cached) = store::load_waveform(cache_root, &key) {
            return Ok(cached);
        }
        let samples = waveform(path, duration_secs)?;
        let _ = store::save_waveform(cache_root, &key, &samples);
        return Ok(samples);
    }
    waveform(path, duration_secs)
}
