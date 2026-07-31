use opentake_domain::{AudioDenoise, DenoiseMode};
use opentake_media::analysis::{denoise_interleaved, DenoiseError};
use opentake_media::MediaCancelToken;

const SAMPLE_RATE: u32 = 48_000;

fn speech_fixture(seconds: usize) -> Vec<f32> {
    (0..SAMPLE_RATE as usize * seconds)
        .map(|index| {
            let time = index as f32 / SAMPLE_RATE as f32;
            let phrase = if (time * 2.5).fract() < 0.68 {
                1.0
            } else {
                0.0
            };
            let attack = ((time * 2.5).fract() * 12.0).min(1.0);
            phrase
                * attack
                * ((std::f32::consts::TAU * 173.0 * time).sin() * 0.24
                    + (std::f32::consts::TAU * 346.0 * time).sin() * 0.08
                    + (std::f32::consts::TAU * 691.0 * time).sin() * 0.035)
        })
        .collect()
}

fn noisy_fixture(clean: &[f32]) -> Vec<f32> {
    let mut state = 0x5eed_1234_u32;
    clean
        .iter()
        .map(|sample| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let white = (state as f64 / u32::MAX as f64 * 2.0 - 1.0) as f32;
            sample + white * 0.075
        })
        .collect()
}

fn snr_db(clean: &[f32], candidate: &[f32]) -> f64 {
    let signal = clean
        .iter()
        .map(|value| f64::from(*value).powi(2))
        .sum::<f64>();
    let error = clean
        .iter()
        .zip(candidate)
        .map(|(expected, actual)| f64::from(*actual - *expected).powi(2))
        .sum::<f64>();
    10.0 * (signal / error.max(1.0e-20)).log10()
}

#[test]
fn deterministic_noise_fixture_and_bypass() {
    let clean = speech_fixture(5);
    let noisy = noisy_fixture(&clean);
    let source_before = noisy.clone();
    let config = AudioDenoise {
        mode: DenoiseMode::Adaptive,
        strength: 0.9,
        preview_enabled: true,
    };
    let processed = denoise_interleaved(
        &noisy,
        1,
        SAMPLE_RATE,
        config,
        &MediaCancelToken::new(),
        None,
    )
    .expect("denoise deterministic fixture");

    let input_snr = snr_db(&clean, &noisy);
    let output_snr = snr_db(&clean, &processed);
    assert!(
        output_snr >= input_snr + 3.0,
        "input SNR={input_snr:.2} dB output SNR={output_snr:.2} dB"
    );
    assert!(processed.iter().all(|sample| sample.abs() <= 1.0));
    let input_peak = noisy
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0_f32, f32::max);
    let output_peak = processed
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0_f32, f32::max);
    assert!(
        output_peak <= input_peak + 1.0e-6,
        "denoise must not introduce a new peak: input={input_peak:.6} output={output_peak:.6}"
    );
    assert_eq!(
        noisy, source_before,
        "processing must not mutate source PCM"
    );

    let bypass = denoise_interleaved(
        &noisy,
        1,
        SAMPLE_RATE,
        AudioDenoise {
            strength: 0.0,
            ..config
        },
        &MediaCancelToken::new(),
        None,
    )
    .expect("bypass");
    assert_eq!(bypass, noisy, "zero strength is a bit-exact bypass");

    let cancelled = MediaCancelToken::new();
    cancelled.cancel();
    assert!(matches!(
        denoise_interleaved(&noisy, 1, SAMPLE_RATE, config, &cancelled, None),
        Err(DenoiseError::Cancelled)
    ));
}
