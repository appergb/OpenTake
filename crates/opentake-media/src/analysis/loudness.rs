//! Deterministic EBU R128 / ITU-R BS.1770 loudness analysis for mono PCM.
//!
//! OpenTake decodes clip windows to 48 kHz mono before analysis. The same
//! computed gain is persisted on the clip and consumed by preview and export;
//! analysis is never repeated during playback or rendering.

use std::sync::Arc;

use thiserror::Error;

use crate::MediaCancelToken;

const ABSOLUTE_GATE_LUFS: f64 = -70.0;
const RELATIVE_GATE_LU: f64 = -10.0;
const LOUDNESS_OFFSET: f64 = -0.691;
const BLOCK_MILLIS: u64 = 400;
const BLOCK_STEP_MILLIS: u64 = 100;
const SILENCE_EPSILON: f64 = 1.0e-12;

pub type LoudnessProgressCallback = Arc<dyn Fn(usize, usize) + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoudnessNormalizationConfig {
    pub target_lufs: f64,
    pub true_peak_ceiling_dbtp: f64,
}

impl Default for LoudnessNormalizationConfig {
    fn default() -> Self {
        Self {
            target_lufs: -16.0,
            true_peak_ceiling_dbtp: -1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoudnessAnalysis {
    pub input_integrated_lufs: f64,
    pub input_true_peak_dbtp: f64,
    pub target_lufs: f64,
    pub true_peak_ceiling_dbtp: f64,
    pub gain_db: f64,
    pub output_integrated_lufs: f64,
    pub output_true_peak_dbtp: f64,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum LoudnessError {
    #[error("loudness_invalid_config: target LUFS and true-peak ceiling must be finite, with ceiling <= 0 dBTP")]
    InvalidConfig,
    #[error("loudness_unreadable_audio: sample rate must be positive and PCM must contain finite samples")]
    UnreadableAudio,
    #[error("loudness_silent_audio: no block passed the EBU R128 absolute gate")]
    SilentAudio,
    #[error("loudness_target_unreachable: the requested target cannot be reached under the true-peak ceiling")]
    TargetUnreachable,
    #[error("loudness_cancelled")]
    Cancelled,
}

#[derive(Clone, Copy)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl Biquad {
    fn process(&mut self, input: f64) -> f64 {
        let output = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;
        output
    }
}

/// Analyze mono PCM with EBU R128 gating and a 4x inter-sample peak estimate.
pub fn analyze_loudness(
    samples: &[f32],
    sample_rate: u32,
    config: LoudnessNormalizationConfig,
) -> Result<LoudnessAnalysis, LoudnessError> {
    analyze_loudness_with_progress(samples, sample_rate, config, &MediaCancelToken::new(), None)
}

pub fn analyze_loudness_with_progress(
    samples: &[f32],
    sample_rate: u32,
    config: LoudnessNormalizationConfig,
    cancel: &MediaCancelToken,
    progress: Option<LoudnessProgressCallback>,
) -> Result<LoudnessAnalysis, LoudnessError> {
    validate(samples, sample_rate, config)?;
    if cancel.checkpoint() {
        return Err(LoudnessError::Cancelled);
    }

    let input_integrated_lufs =
        integrated_loudness(samples, sample_rate, cancel, progress.as_deref())?;
    let input_true_peak_dbtp = true_peak_dbtp(samples);
    if !input_integrated_lufs.is_finite() || !input_true_peak_dbtp.is_finite() {
        return Err(LoudnessError::SilentAudio);
    }

    // Compensate for the shared ceiling stage instead of sacrificing program
    // loudness on high-crest-factor speech. Three correction passes converge
    // the exact persisted gain against the same hard ceiling preview/export
    // use, while remaining deterministic and bounded.
    let mut gain_db = (config.target_lufs - input_integrated_lufs).clamp(-120.0, 60.0);
    let mut output_integrated_lufs = input_integrated_lufs;
    let mut output_true_peak_dbtp = input_true_peak_dbtp;
    for _ in 0..4 {
        if cancel.checkpoint() {
            return Err(LoudnessError::Cancelled);
        }
        let mut normalized = apply_loudness_gain(samples, gain_db);
        crate::encode::mix::apply_true_peak_ceiling(
            &mut normalized,
            Some(config.true_peak_ceiling_dbtp),
        );
        output_integrated_lufs = integrated_loudness(&normalized, sample_rate, cancel, None)?;
        output_true_peak_dbtp = true_peak_dbtp(&normalized);
        let correction = config.target_lufs - output_integrated_lufs;
        if correction.abs() <= 0.05 {
            break;
        }
        gain_db = (gain_db + correction).clamp(-120.0, 60.0);
    }
    if (output_integrated_lufs - config.target_lufs).abs() > 1.0 {
        return Err(LoudnessError::TargetUnreachable);
    }
    if let Some(report) = &progress {
        report(samples.len(), samples.len());
    }
    Ok(LoudnessAnalysis {
        input_integrated_lufs,
        input_true_peak_dbtp,
        target_lufs: config.target_lufs,
        true_peak_ceiling_dbtp: config.true_peak_ceiling_dbtp,
        gain_db,
        output_integrated_lufs,
        output_true_peak_dbtp,
    })
}

fn integrated_loudness(
    samples: &[f32],
    sample_rate: u32,
    cancel: &MediaCancelToken,
    progress: Option<&(dyn Fn(usize, usize) + Send + Sync)>,
) -> Result<f64, LoudnessError> {
    let weighted = k_weight(samples, sample_rate, cancel, progress)?;
    let block_len =
        (((u64::from(sample_rate) * BLOCK_MILLIS) / 1_000) as usize).min(weighted.len());
    let block_step =
        (((u64::from(sample_rate) * BLOCK_STEP_MILLIS) / 1_000) as usize).min(weighted.len());
    if block_len == 0 || block_step == 0 {
        return Err(LoudnessError::SilentAudio);
    }

    let mut block_powers = Vec::with_capacity((weighted.len() - block_len) / block_step + 1);
    let block_count = (weighted.len() - block_len) / block_step + 1;
    for (block_index, start) in (0..=weighted.len() - block_len)
        .step_by(block_step)
        .enumerate()
    {
        if block_index.is_multiple_of(32) {
            if cancel.checkpoint() {
                return Err(LoudnessError::Cancelled);
            }
            if let Some(report) = progress {
                report(weighted.len() + block_index, weighted.len() + block_count);
            }
        }
        let power = weighted[start..start + block_len]
            .iter()
            .map(|sample| sample * sample)
            .sum::<f64>()
            / block_len as f64;
        if power_to_lufs(power) >= ABSOLUTE_GATE_LUFS {
            block_powers.push(power);
        }
    }
    if block_powers.is_empty() {
        return Err(LoudnessError::SilentAudio);
    }

    let absolute_mean = mean(&block_powers);
    let relative_gate = power_to_lufs(absolute_mean) + RELATIVE_GATE_LU;
    let relative_powers = block_powers
        .iter()
        .copied()
        .filter(|power| power_to_lufs(*power) >= relative_gate)
        .collect::<Vec<_>>();
    if relative_powers.is_empty() {
        return Err(LoudnessError::SilentAudio);
    }
    let integrated = power_to_lufs(mean(&relative_powers));
    if let Some(report) = progress {
        let total = weighted.len() + block_count;
        report(total, total);
    }
    Ok(integrated)
}

pub fn apply_loudness_gain(samples: &[f32], gain_db: f64) -> Vec<f32> {
    let gain_db = if gain_db.is_finite() {
        gain_db.clamp(-120.0, 60.0)
    } else {
        0.0
    };
    let gain = 10.0_f64.powf(gain_db / 20.0) as f32;
    samples.iter().map(|sample| *sample * gain).collect()
}

fn validate(
    samples: &[f32],
    sample_rate: u32,
    config: LoudnessNormalizationConfig,
) -> Result<(), LoudnessError> {
    if !config.target_lufs.is_finite()
        || !config.true_peak_ceiling_dbtp.is_finite()
        || config.true_peak_ceiling_dbtp > 0.0
        || !(-70.0..=0.0).contains(&config.target_lufs)
        || !(-20.0..=0.0).contains(&config.true_peak_ceiling_dbtp)
    {
        return Err(LoudnessError::InvalidConfig);
    }
    if sample_rate == 0 || samples.is_empty() || samples.iter().any(|sample| !sample.is_finite()) {
        return Err(LoudnessError::UnreadableAudio);
    }
    Ok(())
}

fn k_weight(
    samples: &[f32],
    sample_rate: u32,
    cancel: &MediaCancelToken,
    progress: Option<&(dyn Fn(usize, usize) + Send + Sync)>,
) -> Result<Vec<f64>, LoudnessError> {
    let mut shelf = shelf_filter(sample_rate as f64);
    let mut high_pass = high_pass_filter(sample_rate as f64);
    let mut output = Vec::with_capacity(samples.len());
    for (index, sample) in samples.iter().enumerate() {
        if index.is_multiple_of(16_384) {
            if cancel.checkpoint() {
                return Err(LoudnessError::Cancelled);
            }
            if let Some(report) = progress {
                report(index, samples.len());
            }
        }
        output.push(high_pass.process(shelf.process(f64::from(*sample))));
    }
    Ok(output)
}

// Coefficients are generated from the analog transfer functions in ITU-R
// BS.1770, allowing analysis at device rates other than 48 kHz.
fn shelf_filter(sample_rate: f64) -> Biquad {
    let f0 = 1_681.974_450_955_533;
    let gain_db = 3.999_843_853_973_347;
    let q = 0.707_175_236_955_419_6;
    let k = (std::f64::consts::PI * f0 / sample_rate).tan();
    let vh = 10.0_f64.powf(gain_db / 20.0);
    let vb = vh.powf(0.499_666_774_154_541_6);
    let a0 = 1.0 + k / q + k * k;
    Biquad {
        b0: (vh + vb * k / q + k * k) / a0,
        b1: 2.0 * (k * k - vh) / a0,
        b2: (vh - vb * k / q + k * k) / a0,
        a1: 2.0 * (k * k - 1.0) / a0,
        a2: (1.0 - k / q + k * k) / a0,
        x1: 0.0,
        x2: 0.0,
        y1: 0.0,
        y2: 0.0,
    }
}

fn high_pass_filter(sample_rate: f64) -> Biquad {
    let f0 = 38.135_470_876_024_44;
    let q = 0.500_327_037_323_877_3;
    let k = (std::f64::consts::PI * f0 / sample_rate).tan();
    let a0 = 1.0 + k / q + k * k;
    Biquad {
        b0: 1.0 / a0,
        b1: -2.0 / a0,
        b2: 1.0 / a0,
        a1: 2.0 * (k * k - 1.0) / a0,
        a2: (1.0 - k / q + k * k) / a0,
        x1: 0.0,
        x2: 0.0,
        y1: 0.0,
        y2: 0.0,
    }
}

fn true_peak_dbtp(samples: &[f32]) -> f64 {
    let mut peak = samples
        .iter()
        .map(|sample| f64::from(sample.abs()))
        .fold(0.0_f64, f64::max);
    // Four-times cubic interpolation catches inter-sample peaks without adding
    // a heavyweight DSP dependency. End points are extended constantly.
    for index in 0..samples.len().saturating_sub(1) {
        let p0 = f64::from(samples[index.saturating_sub(1)]);
        let p1 = f64::from(samples[index]);
        let p2 = f64::from(samples[index + 1]);
        let p3 = f64::from(samples[(index + 2).min(samples.len() - 1)]);
        for phase in 1..4 {
            let t = phase as f64 / 4.0;
            let t2 = t * t;
            let t3 = t2 * t;
            let value = 0.5
                * ((2.0 * p1)
                    + (-p0 + p2) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                    + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3);
            peak = peak.max(value.abs());
        }
    }
    20.0 * peak.max(SILENCE_EPSILON).log10()
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn power_to_lufs(power: f64) -> f64 {
    LOUDNESS_OFFSET + 10.0 * power.max(SILENCE_EPSILON).log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_a_typed_error() {
        let error = analyze_loudness(
            &vec![0.0; 48_000],
            48_000,
            LoudnessNormalizationConfig::default(),
        )
        .unwrap_err();
        assert_eq!(error, LoudnessError::SilentAudio);
    }

    #[test]
    fn pre_cancelled_analysis_stops_before_work() {
        let cancel = MediaCancelToken::new();
        cancel.cancel();
        let error = analyze_loudness_with_progress(
            &vec![0.1; 48_000],
            48_000,
            LoudnessNormalizationConfig::default(),
            &cancel,
            None,
        )
        .unwrap_err();
        assert_eq!(error, LoudnessError::Cancelled);
    }

    #[test]
    fn audible_clip_shorter_than_one_r128_block_is_supported() {
        let samples = (0..4_800)
            .map(|index| (index as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin() * 0.1)
            .collect::<Vec<_>>();
        let analysis = analyze_loudness(&samples, 48_000, LoudnessNormalizationConfig::default())
            .expect("short audible clip");
        assert!(analysis.input_integrated_lufs.is_finite());
    }
}
