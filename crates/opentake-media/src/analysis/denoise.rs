//! Deterministic local STFT noise suppression shared by preview and export.

use std::sync::Arc;

use opentake_domain::{AudioDenoise, DenoiseMode};
use rustfft::{num_complex::Complex32, Fft, FftPlanner};

use crate::MediaCancelToken;

pub type DenoiseProgressCallback = Arc<dyn Fn(usize, usize) + Send + Sync>;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DenoiseError {
    #[error("denoise_invalid_config: {0}")]
    InvalidConfig(String),
    #[error("denoise_cancelled")]
    Cancelled,
}

/// Process interleaved PCM without mutating the source. A zero-strength config
/// is a bit-exact bypass. Each channel is transformed independently so stereo
/// placement is preserved, while all call sites share identical parameters and
/// math.
pub fn denoise_interleaved(
    samples: &[f32],
    channels: usize,
    sample_rate: u32,
    config: AudioDenoise,
    cancel: &MediaCancelToken,
    progress: Option<DenoiseProgressCallback>,
) -> Result<Vec<f32>, DenoiseError> {
    config
        .validate()
        .map_err(|error| DenoiseError::InvalidConfig(error.to_string()))?;
    if channels == 0
        || channels > 8
        || sample_rate < 8_000
        || !samples.len().is_multiple_of(channels)
    {
        return Err(DenoiseError::InvalidConfig(
            "channels, sample rate, or interleaving is unsupported".to_string(),
        ));
    }
    if cancel.checkpoint() {
        return Err(DenoiseError::Cancelled);
    }
    if samples.is_empty() || config.strength == 0.0 {
        return Ok(samples.to_vec());
    }

    let frame_len = if sample_rate >= 32_000 { 1_024 } else { 512 };
    let hop = frame_len / 4;
    let audio_frames = samples.len() / channels;
    let windows = if audio_frames <= frame_len {
        1
    } else {
        1 + (audio_frames - 1) / hop
    };
    let total_steps = channels.saturating_mul(windows).saturating_mul(2).max(1);
    let mut completed = 0usize;

    let window = (0..frame_len)
        .map(|index| {
            let phase = std::f32::consts::TAU * index as f32 / frame_len as f32;
            0.5 - 0.5 * phase.cos()
        })
        .collect::<Vec<_>>();
    let mut planner = FftPlanner::<f32>::new();
    let forward = planner.plan_fft_forward(frame_len);
    let inverse = planner.plan_fft_inverse(frame_len);
    let mut output = vec![0.0_f32; samples.len()];

    for channel in 0..channels {
        let mono = samples
            .iter()
            .skip(channel)
            .step_by(channels)
            .copied()
            .collect::<Vec<_>>();
        let processed = process_channel(
            &mono,
            &window,
            hop,
            windows,
            config,
            cancel,
            &progress,
            total_steps,
            &mut completed,
            &forward,
            &inverse,
        )?;
        for (frame, value) in processed.into_iter().enumerate() {
            output[frame * channels + channel] = value.clamp(-1.0, 1.0);
        }
    }

    if let Some(report) = progress {
        report(total_steps, total_steps);
    }
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
fn process_channel(
    samples: &[f32],
    window: &[f32],
    hop: usize,
    windows: usize,
    config: AudioDenoise,
    cancel: &MediaCancelToken,
    progress: &Option<DenoiseProgressCallback>,
    total_steps: usize,
    completed: &mut usize,
    forward: &Arc<dyn Fft<f32>>,
    inverse: &Arc<dyn Fft<f32>>,
) -> Result<Vec<f32>, DenoiseError> {
    let frame_len = window.len();
    let bins = frame_len / 2 + 1;
    const MAX_NOISE_ESTIMATE_WINDOWS: usize = 512;
    let estimate_stride = windows.div_ceil(MAX_NOISE_ESTIMATE_WINDOWS).max(1);
    let mut powers = (0..bins)
        .map(|_| Vec::with_capacity(windows.min(MAX_NOISE_ESTIMATE_WINDOWS)))
        .collect::<Vec<_>>();
    let mut spectrum = vec![Complex32::new(0.0, 0.0); frame_len];

    for frame_index in 0..windows {
        if cancel.checkpoint() {
            return Err(DenoiseError::Cancelled);
        }
        load_window(samples, window, frame_index * hop, &mut spectrum);
        forward.process(&mut spectrum);
        if frame_index.is_multiple_of(estimate_stride) {
            for bin in 0..bins {
                powers[bin].push(spectrum[bin].norm_sqr());
            }
        }
        report_step(progress, total_steps, completed);
    }

    let noise_power = powers
        .into_iter()
        .map(|mut values| {
            values.sort_by(f32::total_cmp);
            let index = ((values.len().saturating_sub(1)) as f32 * 0.15).round() as usize;
            values[index].max(1.0e-12)
        })
        .collect::<Vec<_>>();
    let strength = config.strength as f32;
    let oversubtraction = match config.mode {
        DenoiseMode::Adaptive => 1.0 + 4.5 * strength,
        DenoiseMode::Voice => 1.0 + 6.0 * strength,
    };
    let floor_gain = 1.0 - 0.92 * strength;
    let mut prior_gain = vec![1.0_f32; bins];
    let mut raw_gain = vec![1.0_f32; bins];
    let mut out = vec![0.0_f32; samples.len()];
    let mut norm = vec![0.0_f32; samples.len()];

    for frame_index in 0..windows {
        if cancel.checkpoint() {
            return Err(DenoiseError::Cancelled);
        }
        let start = frame_index * hop;
        load_window(samples, window, start, &mut spectrum);
        forward.process(&mut spectrum);
        for bin in 0..bins {
            let power = spectrum[bin].norm_sqr().max(1.0e-12);
            let clean_ratio = (1.0 - oversubtraction * noise_power[bin] / power).max(0.0);
            raw_gain[bin] = clean_ratio.sqrt().max(floor_gain);
        }
        for bin in 0..bins {
            let lo = bin.saturating_sub(1);
            let hi = (bin + 1).min(bins - 1);
            let frequency_smoothed = raw_gain[lo..=hi].iter().sum::<f32>() / (hi - lo + 1) as f32;
            let gain = (prior_gain[bin] * 0.25 + frequency_smoothed * 0.75).clamp(floor_gain, 1.0);
            prior_gain[bin] = gain;
            spectrum[bin] *= gain;
            if bin > 0 && bin < frame_len / 2 {
                spectrum[frame_len - bin] *= gain;
            }
        }
        inverse.process(&mut spectrum);
        for index in 0..frame_len {
            let output_index = start + index;
            if output_index >= out.len() {
                break;
            }
            let weight = window[index];
            out[output_index] += spectrum[index].re / frame_len as f32 * weight;
            norm[output_index] += weight * weight;
        }
        report_step(progress, total_steps, completed);
    }

    let input_peak = samples
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0_f32, f32::max)
        .min(1.0);
    let edge_span = (frame_len / 2).min(samples.len().saturating_sub(1)).max(1);
    for (index, (value, weight)) in out.iter_mut().zip(norm).enumerate() {
        let normalized = if weight > 1.0e-6 {
            *value / weight
        } else {
            samples[index]
        };
        // A centered STFT would normally pad both ends before analysis. Keep
        // the implementation allocation-bounded by crossfading the unpadded
        // edge into the processed signal instead. The peak guard prevents
        // low Hann-normalization weights from creating a click or a new peak.
        let edge_distance = index.min(samples.len() - 1 - index);
        let processed_mix = (edge_distance as f32 / edge_span as f32).min(1.0);
        *value = (samples[index] * (1.0 - processed_mix) + normalized * processed_mix)
            .clamp(-input_peak, input_peak);
    }
    Ok(out)
}

fn load_window(samples: &[f32], window: &[f32], start: usize, target: &mut [Complex32]) {
    for (index, complex) in target.iter_mut().enumerate() {
        let value = samples.get(start + index).copied().unwrap_or(0.0);
        *complex = Complex32::new(value * window[index], 0.0);
    }
}

fn report_step(progress: &Option<DenoiseProgressCallback>, total: usize, completed: &mut usize) {
    *completed = completed.saturating_add(1);
    if let Some(report) = progress {
        report((*completed).min(total), total);
    }
}
