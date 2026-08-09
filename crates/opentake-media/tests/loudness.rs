use opentake_media::analysis::{
    analyze_loudness, apply_loudness_gain, LoudnessNormalizationConfig,
};
use opentake_media::encode::mix::apply_true_peak_ceiling;

const SAMPLE_RATE: u32 = 48_000;

fn sine_fixture(amplitude: f32, frequency_hz: f32, duration_seconds: usize) -> Vec<f32> {
    let sample_count = SAMPLE_RATE as usize * duration_seconds;
    (0..sample_count)
        .map(|index| {
            let phase = index as f32 * frequency_hz * std::f32::consts::TAU / SAMPLE_RATE as f32;
            phase.sin() * amplitude
        })
        .collect()
}

#[test]
fn normalization_reaches_configured_lufs_within_tolerance() {
    let samples = sine_fixture(0.08, 997.0, 4);
    let config = LoudnessNormalizationConfig {
        target_lufs: -16.0,
        true_peak_ceiling_dbtp: -1.0,
    };

    let analysis = analyze_loudness(&samples, SAMPLE_RATE, config).expect("analyze fixture");
    // Cross-checked against FFmpeg 8.1 loudnorm for the same 997 Hz / 0.08
    // amplitude / 48 kHz fixture (`input_i=-24.95`, `input_tp=-21.94`).
    assert!((analysis.input_integrated_lufs - -24.95).abs() <= 0.2);
    assert!((analysis.input_true_peak_dbtp - -21.94).abs() <= 0.1);
    let normalized = apply_loudness_gain(&samples, analysis.gain_db);
    let measured = analyze_loudness(
        &normalized,
        SAMPLE_RATE,
        LoudnessNormalizationConfig {
            target_lufs: analysis.output_integrated_lufs,
            true_peak_ceiling_dbtp: 0.0,
        },
    )
    .expect("measure normalized fixture");

    assert!(
        (measured.input_integrated_lufs - config.target_lufs).abs() <= 1.0,
        "measured={} target={} gain={} input={} peak={}",
        measured.input_integrated_lufs,
        config.target_lufs,
        analysis.gain_db,
        analysis.input_integrated_lufs,
        measured.input_true_peak_dbtp,
    );
    assert!(measured.input_true_peak_dbtp <= config.true_peak_ceiling_dbtp + 0.05);
}

fn verify_program_fixture(mut samples: Vec<f32>) {
    let config = LoudnessNormalizationConfig::default();
    let analysis = analyze_loudness(&samples, SAMPLE_RATE, config).expect("analyze fixture");
    samples = apply_loudness_gain(&samples, analysis.gain_db);
    apply_true_peak_ceiling(&mut samples, Some(config.true_peak_ceiling_dbtp));
    let measured = analyze_loudness(&samples, SAMPLE_RATE, config).expect("measure output");
    assert!(
        (measured.input_integrated_lufs - config.target_lufs).abs() <= 1.0,
        "measured={} target={} gain={} input={} peak={}",
        measured.input_integrated_lufs,
        config.target_lufs,
        analysis.gain_db,
        analysis.input_integrated_lufs,
        measured.input_true_peak_dbtp,
    );
    assert!(measured.input_true_peak_dbtp <= config.true_peak_ceiling_dbtp + 0.05);
}

#[test]
fn speech_and_music_fixtures_reach_target_without_exceeding_true_peak() {
    let sample_count = SAMPLE_RATE as usize * 5;
    let speech = (0..sample_count)
        .map(|index| {
            let time = index as f32 / SAMPLE_RATE as f32;
            let syllable = if (time * 3.2).fract() < 0.62 {
                1.0
            } else {
                0.08
            };
            let voiced = (std::f32::consts::TAU * 173.0 * time).sin() * 0.035
                + (std::f32::consts::TAU * 346.0 * time).sin() * 0.018;
            let plosive = if index % 31_337 < 12 { 0.72 } else { 0.0 };
            voiced * syllable + plosive
        })
        .collect();
    verify_program_fixture(speech);

    let music = (0..sample_count)
        .map(|index| {
            let time = index as f32 / SAMPLE_RATE as f32;
            let tonal = (std::f32::consts::TAU * 220.0 * time).sin() * 0.045
                + (std::f32::consts::TAU * 329.63 * time).sin() * 0.035
                + (std::f32::consts::TAU * 440.0 * time).sin() * 0.025;
            let beat_phase = index % (SAMPLE_RATE as usize / 2);
            let beat = if beat_phase < 240 {
                0.35 * (1.0 - beat_phase as f32 / 240.0)
            } else {
                0.0
            };
            tonal + beat
        })
        .collect();
    verify_program_fixture(music);
}
