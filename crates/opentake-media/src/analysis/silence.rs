#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SilenceDetectionConfig {
    pub sample_rate: u32,
    pub fps: f64,
    pub window_size_samples: usize,
    pub hop_size_samples: usize,
    pub rms_threshold: f32,
    pub min_silence_frames: u64,
}

impl SilenceDetectionConfig {
    pub fn with_window(sample_rate: u32, fps: f64, window_size_samples: usize) -> Self {
        let window_size_samples = window_size_samples.max(1);
        SilenceDetectionConfig {
            sample_rate,
            fps,
            window_size_samples,
            hop_size_samples: (window_size_samples / 2).max(1),
            rms_threshold: 0.01,
            min_silence_frames: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SilenceRange {
    pub start_frame: u64,
    pub end_frame: u64,
}

pub fn detect_silences(samples: &[f32], config: SilenceDetectionConfig) -> Vec<SilenceRange> {
    if samples.is_empty() || config.sample_rate == 0 || !config.fps.is_finite() || config.fps <= 0.0
    {
        return Vec::new();
    }

    let window = config.window_size_samples.max(1);
    let hop = config.hop_size_samples.max(1);
    let mut ranges = Vec::new();
    let mut active_start = None;
    let mut active_end = 0usize;
    let mut start = 0usize;

    while start < samples.len() {
        let end = (start + window).min(samples.len());
        let silent = rms(&samples[start..end]) <= config.rms_threshold;
        if silent {
            active_start.get_or_insert(start);
            active_end = end;
        } else if let Some(silence_start) = active_start.take() {
            push_range(&mut ranges, silence_start, active_end, &config);
        }
        start += hop;
    }

    if let Some(silence_start) = active_start {
        push_range(&mut ranges, silence_start, active_end, &config);
    }

    ranges
}

fn push_range(
    ranges: &mut Vec<SilenceRange>,
    start_sample: usize,
    end_sample: usize,
    config: &SilenceDetectionConfig,
) {
    let start_frame = sample_to_frame(start_sample, config.sample_rate, config.fps);
    let mut end_frame = sample_to_frame(end_sample, config.sample_rate, config.fps);
    if end_frame <= start_frame {
        end_frame = start_frame + 1;
    }
    if end_frame - start_frame >= config.min_silence_frames {
        ranges.push(SilenceRange {
            start_frame,
            end_frame,
        });
    }
}

fn rms(samples: &[f32]) -> f32 {
    let mut sum = 0.0f64;
    for &sample in samples {
        let sample = sample as f64;
        sum += sample * sample;
    }
    (sum / samples.len() as f64).sqrt() as f32
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
    fn alternating_audio_detects_half_open_silence_range() {
        let mut samples = vec![0.5f32; 300];
        samples.extend(std::iter::repeat_n(0.0f32, 400));
        samples.extend(std::iter::repeat_n(0.5f32, 300));

        let config = SilenceDetectionConfig {
            sample_rate: 1_000,
            fps: 10.0,
            window_size_samples: 100,
            hop_size_samples: 100,
            rms_threshold: 0.01,
            min_silence_frames: 2,
        };

        let ranges = detect_silences(&samples, config);

        assert_eq!(
            ranges,
            vec![SilenceRange {
                start_frame: 3,
                end_frame: 7,
            }]
        );
    }
}
