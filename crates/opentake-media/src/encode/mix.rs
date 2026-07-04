//! Pure linear audio mixdown for the export pipeline.
//!
//! The export orchestrator decodes each audio clip's source window to mono f32
//! PCM (via [`crate::decode::extract_pcm`]), then this module lays every clip
//! into one shared timeline buffer at its frame-derived sample offset, applies a
//! per-sample gain (the clip's `volume_at` envelope, projected to the mix rate),
//! sums overlapping clips, and hard-limits the result to `[-1.0, 1.0]`.
//!
//! Everything here is a pure function over plain `f32` slices — no ffmpeg, no
//! domain types — so the linear-mix math is unit-tested offline. The encoder
//! ([`crate::encode::VideoEncoder`]) muxes the produced buffer as a second
//! ffmpeg input; the orchestrator (`src-tauri/src/export.rs`) supplies the clip
//! placements.
//!
//! Scope of this first cut: a **linear** mixdown skeleton (sum + clamp). No
//! resampling curve, no pan/stereo field, no dynamics — those are follow-ups.
//! All clips are decoded at the mix sample rate up front, so mixing is a plain
//! sample-aligned add.

/// The canonical mixdown sample rate. 48 kHz is the export-audio standard and
/// what the encoder requests from ffmpeg for the muxed AAC/LPCM track.
pub const MIX_SAMPLE_RATE: u32 = 48_000;

/// One audio clip's contribution to the mix: a mono f32 source window plus the
/// per-sample gain to apply, laid down starting at `start_sample` on the shared
/// timeline buffer.
///
/// `gains` is either empty (→ unity gain for every sample) or exactly as long as
/// `samples` (→ element-wise gain, e.g. a `volume_at` fade envelope sampled at
/// the mix rate). A mismatched non-empty length is treated as a hard error by
/// [`mix_clips`] so callers can't silently drift the envelope.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipAudio {
    /// Sample offset of this clip's first sample on the timeline (>= 0).
    pub start_sample: usize,
    /// Mono f32 PCM for the clip's visible source window, at [`MIX_SAMPLE_RATE`].
    pub samples: Vec<f32>,
    /// Per-sample linear gain. Empty = unity; else must match `samples.len()`.
    pub gains: Vec<f32>,
}

impl ClipAudio {
    /// A clip with a single static `gain` applied to every sample.
    pub fn with_static_gain(start_sample: usize, samples: Vec<f32>, gain: f32) -> Self {
        let gains = if (gain - 1.0).abs() < f32::EPSILON {
            Vec::new()
        } else {
            vec![gain; samples.len()]
        };
        ClipAudio {
            start_sample,
            samples,
            gains,
        }
    }

    /// Last timeline sample index this clip touches (exclusive end).
    fn end_sample(&self) -> usize {
        self.start_sample + self.samples.len()
    }
}

/// Mix every clip into one mono f32 buffer.
///
/// The output length is the furthest `end_sample` across all clips (so trailing
/// silence past the last clip is not emitted). Overlapping clips sum; the final
/// buffer is hard-limited to `[-1.0, 1.0]`. An empty input yields an empty
/// buffer (the caller then mux's no audio).
///
/// Returns `Err` if any clip's non-empty `gains` length doesn't match its
/// `samples` length — a programming error in the caller's per-sample envelope.
pub fn mix_clips(clips: &[ClipAudio]) -> Result<Vec<f32>, String> {
    for (i, c) in clips.iter().enumerate() {
        if !c.gains.is_empty() && c.gains.len() != c.samples.len() {
            return Err(format!(
                "clip {i}: gains len {} != samples len {}",
                c.gains.len(),
                c.samples.len()
            ));
        }
    }

    let total = clips.iter().map(ClipAudio::end_sample).max().unwrap_or(0);
    let mut out = vec![0.0f32; total];

    for c in clips {
        for (k, &s) in c.samples.iter().enumerate() {
            let g = if c.gains.is_empty() { 1.0 } else { c.gains[k] };
            out[c.start_sample + k] += s * g;
        }
    }

    for v in &mut out {
        *v = v.clamp(-1.0, 1.0);
    }
    Ok(out)
}

/// Convert a mono f32 buffer to interleaved 16-bit little-endian PCM bytes (the
/// wire format the encoder writes into a temporary WAV for muxing). Each sample
/// is scaled by 32767 and clamped, matching ffmpeg's `s16le` expectation.
pub fn mono_f32_to_s16le(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        let scaled = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        out.extend_from_slice(&scaled.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_yields_empty_buffer() {
        assert_eq!(mix_clips(&[]).unwrap(), Vec::<f32>::new());
    }

    #[test]
    fn single_clip_unity_gain_passes_through() {
        let c = ClipAudio {
            start_sample: 0,
            samples: vec![0.1, -0.2, 0.3],
            gains: Vec::new(),
        };
        assert_eq!(mix_clips(&[c]).unwrap(), vec![0.1, -0.2, 0.3]);
    }

    #[test]
    fn clip_offset_lays_after_leading_silence() {
        let c = ClipAudio {
            start_sample: 2,
            samples: vec![0.5, 0.5],
            gains: Vec::new(),
        };
        // two leading zeros, then the clip
        assert_eq!(mix_clips(&[c]).unwrap(), vec![0.0, 0.0, 0.5, 0.5]);
    }

    #[test]
    fn overlapping_clips_sum() {
        let a = ClipAudio {
            start_sample: 0,
            samples: vec![0.2, 0.2, 0.2],
            gains: Vec::new(),
        };
        let b = ClipAudio {
            start_sample: 1,
            samples: vec![0.3, 0.3],
            gains: Vec::new(),
        };
        // index1: 0.2+0.3=0.5 ; index2: 0.2+0.3=0.5
        assert_eq!(mix_clips(&[a, b]).unwrap(), vec![0.2, 0.5, 0.5]);
    }

    #[test]
    fn summed_overshoot_is_hard_limited() {
        let a = ClipAudio {
            start_sample: 0,
            samples: vec![0.8],
            gains: Vec::new(),
        };
        let b = ClipAudio {
            start_sample: 0,
            samples: vec![0.8],
            gains: Vec::new(),
        };
        // 1.6 -> clamped to 1.0
        assert_eq!(mix_clips(&[a, b]).unwrap(), vec![1.0]);
        // and the negative rail
        let c = ClipAudio {
            start_sample: 0,
            samples: vec![-0.9, -0.9],
            gains: Vec::new(),
        };
        let d = ClipAudio {
            start_sample: 0,
            samples: vec![-0.9, -0.9],
            gains: Vec::new(),
        };
        assert_eq!(mix_clips(&[c, d]).unwrap(), vec![-1.0, -1.0]);
    }

    #[test]
    fn per_sample_gain_is_applied() {
        let c = ClipAudio {
            start_sample: 0,
            samples: vec![1.0, 1.0, 1.0],
            gains: vec![0.0, 0.5, 1.0],
        };
        assert_eq!(mix_clips(&[c]).unwrap(), vec![0.0, 0.5, 1.0]);
    }

    #[test]
    fn static_gain_helper_skips_envelope_at_unity() {
        let c = ClipAudio::with_static_gain(0, vec![0.4, 0.4], 1.0);
        assert!(c.gains.is_empty(), "unity gain stores no envelope");
        let c2 = ClipAudio::with_static_gain(0, vec![0.4, 0.4], 0.5);
        assert_eq!(c2.gains, vec![0.5, 0.5]);
        assert_eq!(mix_clips(&[c2]).unwrap(), vec![0.2, 0.2]);
    }

    #[test]
    fn mismatched_gain_length_errors() {
        let c = ClipAudio {
            start_sample: 0,
            samples: vec![0.1, 0.2],
            gains: vec![1.0], // wrong length
        };
        let err = mix_clips(&[c]).unwrap_err();
        assert!(err.contains("gains len"), "got: {err}");
    }

    #[test]
    fn output_length_is_furthest_clip_end() {
        let a = ClipAudio {
            start_sample: 0,
            samples: vec![0.1],
            gains: Vec::new(),
        };
        let b = ClipAudio {
            start_sample: 10,
            samples: vec![0.1, 0.1],
            gains: Vec::new(),
        };
        // furthest end = 10 + 2 = 12
        assert_eq!(mix_clips(&[a, b]).unwrap().len(), 12);
    }

    #[test]
    fn s16le_encodes_unit_floats() {
        // 0.0 -> 0 ; 1.0 -> 32767 ; -1.0 -> -32767
        let bytes = mono_f32_to_s16le(&[0.0, 1.0, -1.0]);
        assert_eq!(bytes.len(), 6);
        assert_eq!(i16::from_le_bytes([bytes[0], bytes[1]]), 0);
        assert_eq!(i16::from_le_bytes([bytes[2], bytes[3]]), 32767);
        assert_eq!(i16::from_le_bytes([bytes[4], bytes[5]]), -32767);
    }

    #[test]
    fn s16le_clamps_out_of_range() {
        let bytes = mono_f32_to_s16le(&[2.0, -2.0]);
        assert_eq!(i16::from_le_bytes([bytes[0], bytes[1]]), 32767);
        assert_eq!(i16::from_le_bytes([bytes[2], bytes[3]]), -32767);
    }
}
