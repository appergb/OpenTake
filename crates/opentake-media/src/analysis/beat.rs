#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeatDetectionConfig {
    pub sample_rate: u32,
    pub fps: f64,
    pub window_size_samples: usize,
    pub hop_size_samples: usize,
    pub min_onset_strength: f32,
    pub min_gap_frames: u64,
}

impl BeatDetectionConfig {
    pub fn with_window(sample_rate: u32, fps: f64, window_size_samples: usize) -> Self {
        let window_size_samples = window_size_samples.max(1);
        BeatDetectionConfig {
            sample_rate,
            fps,
            window_size_samples,
            hop_size_samples: (window_size_samples / 2).max(1),
            min_onset_strength: 0.08,
            min_gap_frames: 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeatOnset {
    pub frame: u64,
    pub strength: f32,
}

pub fn detect_beats(samples: &[f32], config: BeatDetectionConfig) -> Vec<BeatOnset> {
    if samples.is_empty() || config.sample_rate == 0 || !config.fps.is_finite() || config.fps <= 0.0
    {
        return Vec::new();
    }

    let window = config.window_size_samples.max(1);
    let hop = config.hop_size_samples.max(1);
    let energies = window_energies(samples, window, hop);
    if energies.len() < 2 {
        return Vec::new();
    }

    let peak_delta = energies
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).max(0.0))
        .fold(0.0f32, f32::max);
    if peak_delta <= f32::EPSILON {
        return Vec::new();
    }

    let mut beats = Vec::new();
    let mut last_frame = None;
    for i in 1..energies.len() {
        let delta = (energies[i] - energies[i - 1]).max(0.0);
        let strength = delta / peak_delta;
        if strength < config.min_onset_strength {
            continue;
        }

        let frame = sample_to_frame(i * hop, config.sample_rate, config.fps);
        if last_frame.is_some_and(|last| frame < last + config.min_gap_frames) {
            continue;
        }

        beats.push(BeatOnset { frame, strength });
        last_frame = Some(frame);
    }
    beats
}

fn window_energies(samples: &[f32], window: usize, hop: usize) -> Vec<f32> {
    let mut out = Vec::new();
    let mut start = 0;
    while start < samples.len() {
        let end = (start + window).min(samples.len());
        let slice = &samples[start..end];
        let mut sum = 0.0f64;
        for &sample in slice {
            let sample = sample as f64;
            sum += sample * sample;
        }
        out.push((sum / slice.len() as f64) as f32);
        start += hop;
    }
    out
}

fn sample_to_frame(sample: usize, sample_rate: u32, fps: f64) -> u64 {
    ((sample as f64 / sample_rate as f64) * fps)
        .floor()
        .max(0.0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulse_audio_detects_beat_frame_with_strength() {
        let mut samples = vec![0.0f32; 1_000];
        for sample in &mut samples[500..530] {
            *sample = 1.0;
        }

        let config = BeatDetectionConfig {
            sample_rate: 1_000,
            fps: 10.0,
            window_size_samples: 100,
            hop_size_samples: 100,
            min_onset_strength: 0.05,
            min_gap_frames: 1,
        };

        let beats = detect_beats(&samples, config);

        let beat = beats
            .iter()
            .find(|beat| beat.frame == 5)
            .expect("pulse should produce a beat on frame 5");
        assert!(beat.strength > 0.0);
    }
}
