//! Pure waveform DSP: the sample-count formula and the RMS downsample +
//! normalization. No IO, no decoding — fully unit-testable.

/// Buckets per second of audio (upstream `MediaVisualCache.waveformSampleCount`).
pub const BUCKETS_PER_SECOND: f64 = 150.0;
/// Lower bound on bucket count.
pub const MIN_BUCKETS: usize = 4000;
/// Hard upper bound on bucket count.
pub const MAX_BUCKETS: usize = 20_000;

/// Number of normalized waveform buckets for a clip of `duration` seconds.
///
/// Verbatim port of `waveformSampleCount` (`MediaVisualCache.swift:186-190`):
/// - non-finite or `<= 0` duration → [`MIN_BUCKETS`]
/// - `duration >= MAX_BUCKETS / BUCKETS_PER_SECOND` (≈133.3 s) → [`MAX_BUCKETS`]
/// - otherwise → `max(MIN_BUCKETS, floor(duration * BUCKETS_PER_SECOND))`
pub fn waveform_sample_count(duration: f64) -> usize {
    if !duration.is_finite() || duration <= 0.0 {
        return MIN_BUCKETS;
    }
    if duration >= MAX_BUCKETS as f64 / BUCKETS_PER_SECOND {
        return MAX_BUCKETS;
    }
    MIN_BUCKETS.max((duration * BUCKETS_PER_SECOND) as usize)
}

/// Downsample mono `samples` into `count` normalized buckets, **0 = loud,
/// 1 = silence** (upstream's inverted convention,
/// `MediaVisualCache.swift:11`).
///
/// Each bucket holds the RMS of its slice of samples. The RMS envelope is scaled
/// to the loudest bucket (full-scale normalization) and then inverted:
/// `out = 1 - rms_bucket / peak_rms`. A fully silent input yields all-ones; a
/// full-scale input yields values near zero.
///
/// `count == 0` → empty. Fewer samples than buckets still produces `count`
/// values (empty buckets are treated as silence → `1.0`).
pub fn rms_downsample_normalized(samples: &[f32], count: usize) -> Vec<f32> {
    if count == 0 {
        return Vec::new();
    }
    if samples.is_empty() {
        // No audio data decoded: report full silence.
        return vec![1.0; count];
    }

    let n = samples.len();
    let mut rms = vec![0.0f32; count];
    for (bucket, slot) in rms.iter_mut().enumerate() {
        // Half-open slice [lo, hi) for this bucket, spreading samples evenly.
        let lo = bucket * n / count;
        let hi = ((bucket + 1) * n / count).max(lo + 1).min(n);
        let slice = &samples[lo..hi];
        let mut sum_sq = 0.0f64;
        for &s in slice {
            sum_sq += (s as f64) * (s as f64);
        }
        *slot = (sum_sq / slice.len() as f64).sqrt() as f32;
    }

    // Full-scale normalization against the loudest bucket, then invert.
    let peak = rms.iter().copied().fold(0.0f32, f32::max);
    if peak <= f32::EPSILON {
        return vec![1.0; count];
    }
    for v in rms.iter_mut() {
        let amp = (*v / peak).clamp(0.0, 1.0);
        *v = 1.0 - amp;
    }
    rms
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- waveform_sample_count: boundary table ---

    #[test]
    fn count_zero_or_negative_or_nan_is_min() {
        assert_eq!(waveform_sample_count(0.0), MIN_BUCKETS);
        assert_eq!(waveform_sample_count(-5.0), MIN_BUCKETS);
        assert_eq!(waveform_sample_count(f64::NAN), MIN_BUCKETS);
        assert_eq!(waveform_sample_count(f64::INFINITY), MIN_BUCKETS); // not finite
    }

    #[test]
    fn count_one_second_is_min_floor() {
        // 1 * 150 = 150 < 4000 → clamps up to MIN_BUCKETS.
        assert_eq!(waveform_sample_count(1.0), MIN_BUCKETS);
    }

    #[test]
    fn count_mid_range_is_duration_times_150() {
        // 100 s → 15000 buckets (between min and max).
        assert_eq!(waveform_sample_count(100.0), 15_000);
        // 30 s → 4500.
        assert_eq!(waveform_sample_count(30.0), 4_500);
    }

    #[test]
    fn count_at_and_above_cap_is_max() {
        let cap = MAX_BUCKETS as f64 / BUCKETS_PER_SECOND; // ≈133.333
        assert_eq!(waveform_sample_count(cap), MAX_BUCKETS); // boundary is inclusive (>=)
        assert_eq!(waveform_sample_count(cap + 0.001), MAX_BUCKETS);
        assert_eq!(waveform_sample_count(1000.0), MAX_BUCKETS);
    }

    #[test]
    fn count_just_below_cap_is_not_max() {
        let just_below = MAX_BUCKETS as f64 / BUCKETS_PER_SECOND - 1.0; // ~132.3s
        let c = waveform_sample_count(just_below);
        assert!(c < MAX_BUCKETS);
        assert_eq!(c, (just_below * BUCKETS_PER_SECOND) as usize);
    }

    // --- rms_downsample_normalized ---

    #[test]
    fn downsample_zero_count_is_empty() {
        assert!(rms_downsample_normalized(&[0.1, 0.2], 0).is_empty());
    }

    #[test]
    fn downsample_empty_input_is_full_silence() {
        let out = rms_downsample_normalized(&[], 5);
        assert_eq!(out, vec![1.0; 5]);
    }

    #[test]
    fn downsample_full_silence_is_all_ones() {
        let silent = vec![0.0f32; 1000];
        let out = rms_downsample_normalized(&silent, 10);
        assert_eq!(out.len(), 10);
        for v in out {
            assert!((v - 1.0).abs() < 1e-6, "silence must map to ~1.0, got {v}");
        }
    }

    #[test]
    fn downsample_full_scale_sine_is_near_zero() {
        // A full-amplitude tone: loudest bucket → ~0 after inversion.
        let mut s = Vec::with_capacity(2000);
        for i in 0..2000 {
            s.push((i as f32 * 0.3).sin());
        }
        let out = rms_downsample_normalized(&s, 20);
        let min = out.iter().copied().fold(f32::INFINITY, f32::min);
        assert!(min < 0.2, "loudest bucket should be near 0, got {min}");
    }

    #[test]
    fn downsample_monotonic_loudness_inversion() {
        // First half quiet, second half loud → first buckets ~1, last buckets ~0.
        let mut s = vec![0.01f32; 1000];
        s.extend(std::iter::repeat_n(1.0f32, 1000));
        let out = rms_downsample_normalized(&s, 4);
        assert_eq!(out.len(), 4);
        // quiet region (high value) > loud region (low value)
        assert!(out[0] > out[3], "quiet→loud must be decreasing: {out:?}");
        assert!(out[3] < 0.1);
    }

    #[test]
    fn downsample_produces_exact_count_even_when_undersampled() {
        // 3 samples, 10 buckets — must still return 10 values.
        let out = rms_downsample_normalized(&[1.0, 0.0, 1.0], 10);
        assert_eq!(out.len(), 10);
    }

    #[test]
    fn downsample_values_in_unit_range() {
        let mut s = Vec::new();
        for i in 0..500 {
            s.push(((i as f32) / 500.0) * 2.0 - 1.0); // ramp -1..1
        }
        let out = rms_downsample_normalized(&s, 16);
        for v in out {
            assert!((0.0..=1.0).contains(&v), "out of [0,1]: {v}");
        }
    }
}
