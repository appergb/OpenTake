use std::fs;
use std::path::Path;
use std::sync::Arc;

use opentake_media::analysis::stems::{
    ensure_local_stem_model, separate_stems, verify_local_stem_model, StemExecution,
    StemProgressCallback, StemSeparationRequest,
};
use opentake_media::{MediaCancelToken, MediaError};
use tempfile::TempDir;

const SAMPLE_RATE: u32 = 48_000;
const FRAMES: usize = 48_000;

fn write_stereo_fixture(path: &Path) {
    let data_len = (FRAMES * 2 * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE * 4).to_le_bytes());
    wav.extend_from_slice(&4_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for frame in 0..FRAMES {
        let t = frame as f32 / SAMPLE_RATE as f32;
        let vocal = 0.28 * (std::f32::consts::TAU * 440.0 * t).sin();
        let music = 0.22 * (std::f32::consts::TAU * 997.0 * t).sin();
        for sample in [vocal + music, vocal - music] {
            wav.extend_from_slice(&((sample.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
    }
    fs::write(path, wav).expect("write deterministic stereo fixture");
}

fn assert_clean_output_dir(path: &Path) {
    let entries = fs::read_dir(path)
        .expect("read output directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect output directory");
    assert!(
        entries.is_empty(),
        "cancelled job must clean partial outputs"
    );
}

#[test]
fn local_or_explicit_provider_selection_cancellation_provenance_and_cleanup() {
    let temp = TempDir::new().expect("temp root");
    let source = temp.path().join("center-vocal.wav");
    let models = temp.path().join("models");
    let outputs = temp.path().join("outputs");
    write_stereo_fixture(&source);
    fs::create_dir_all(&outputs).expect("create outputs");

    let installed = ensure_local_stem_model(&models).expect("install bundled local model");
    assert!(installed.path.is_file());
    assert_eq!(verify_local_stem_model(&models).unwrap(), installed);

    let progress_values = Arc::new(std::sync::Mutex::new(Vec::new()));
    let progress_capture = progress_values.clone();
    let progress: StemProgressCallback = Arc::new(move |done, total| {
        progress_capture.lock().unwrap().push((done, total));
    });
    let result = separate_stems(
        StemSeparationRequest {
            source: &source,
            output_dir: &outputs,
            execution: StemExecution::Local { model_dir: &models },
        },
        &MediaCancelToken::new(),
        Some(progress),
    )
    .expect("separate local stems");

    assert!(result.vocals.path.is_file());
    assert!(result.accompaniment.path.is_file());
    assert_eq!(result.provenance.execution, "local:opentake-center-v1");
    assert_eq!(result.provenance.source_sha256.len(), 64);
    assert_eq!(
        result.provenance.model_sha256,
        Some(installed.sha256.clone())
    );
    assert!(result.metrics.vocal_sdr_improvement_db >= 12.0);
    let spec = opentake_media::PcmSpec {
        sample_rate: SAMPLE_RATE,
        channels: 2,
        format: opentake_media::PcmFormat::F32,
    };
    let mixture = opentake_media::decode_pcm_interleaved(&source, &spec, None).unwrap();
    let separated_vocals =
        opentake_media::decode_pcm_interleaved(&result.vocals.path, &spec, None).unwrap();
    let separated_accompaniment =
        opentake_media::decode_pcm_interleaved(&result.accompaniment.path, &spec, None).unwrap();
    let mut reference = Vec::with_capacity(FRAMES * 2);
    let mut accompaniment_reference = Vec::with_capacity(FRAMES * 2);
    for frame in 0..FRAMES {
        let t = frame as f32 / SAMPLE_RATE as f32;
        let vocal = 0.28 * (std::f32::consts::TAU * 440.0 * t).sin();
        let music = 0.22 * (std::f32::consts::TAU * 997.0 * t).sin();
        reference.extend_from_slice(&[vocal, vocal]);
        // A user-facing accompaniment stem must remain audible after a mono
        // export/downmix, so the isolated side signal is emitted dual-mono.
        accompaniment_reference.extend_from_slice(&[music, music]);
    }
    let sdr = |candidate: &[f32]| {
        let signal = reference
            .iter()
            .map(|sample| f64::from(*sample) * f64::from(*sample))
            .sum::<f64>();
        let error = reference
            .iter()
            .zip(candidate)
            .map(|(expected, actual)| {
                let delta = f64::from(*expected - *actual);
                delta * delta
            })
            .sum::<f64>()
            .max(1.0e-12);
        10.0 * (signal / error).log10()
    };
    let measured_improvement = sdr(&separated_vocals) - sdr(&mixture);
    assert!(
        measured_improvement >= 12.0,
        "decoded vocals must improve SDR by >= 12 dB, got {measured_improvement:.3} dB"
    );
    let accompaniment_signal = accompaniment_reference
        .iter()
        .map(|sample| f64::from(*sample) * f64::from(*sample))
        .sum::<f64>();
    let accompaniment_error = accompaniment_reference
        .iter()
        .zip(&separated_accompaniment)
        .map(|(expected, actual)| {
            let delta = f64::from(*expected - *actual);
            delta * delta
        })
        .sum::<f64>()
        .max(1.0e-12);
    let accompaniment_sdr = 10.0 * (accompaniment_signal / accompaniment_error).log10();
    assert!(
        accompaniment_sdr >= 60.0,
        "decoded accompaniment must be mono-compatible, got {accompaniment_sdr:.3} dB SDR"
    );
    let mut mono_compatible_mix = Vec::with_capacity(FRAMES * 2);
    for frame in 0..FRAMES {
        let t = frame as f32 / SAMPLE_RATE as f32;
        let vocal = 0.28 * (std::f32::consts::TAU * 440.0 * t).sin();
        let music = 0.22 * (std::f32::consts::TAU * 997.0 * t).sin();
        mono_compatible_mix.extend_from_slice(&[vocal + music, vocal + music]);
    }
    let reconstruction_signal = mono_compatible_mix
        .iter()
        .map(|sample| f64::from(*sample) * f64::from(*sample))
        .sum::<f64>();
    let reconstruction_error = mono_compatible_mix
        .iter()
        .zip(separated_vocals.iter().zip(&separated_accompaniment))
        .map(|(expected, (vocals, accompaniment))| {
            let delta = f64::from(*expected - (*vocals + *accompaniment));
            delta * delta
        })
        .sum::<f64>()
        .max(1.0e-12);
    let reconstruction_sdr = 10.0 * (reconstruction_signal / reconstruction_error).log10();
    assert!(
        reconstruction_sdr >= 60.0,
        "stem sum must reconstruct the documented mono-compatible mixture at >= 60 dB SDR, got {reconstruction_sdr:.3} dB"
    );
    let progress_values = progress_values.lock().unwrap();
    assert_eq!(progress_values.first().copied(), Some((0, 1000)));
    assert_eq!(progress_values.last().copied(), Some((1000, 1000)));

    let corrupt = fs::read(&installed.path).expect("read installed model");
    fs::write(&installed.path, [corrupt, b"corrupt".to_vec()].concat())
        .expect("corrupt installed model");
    assert!(matches!(
        verify_local_stem_model(&models),
        Err(MediaError::Checksum(_))
    ));

    let cancelled_outputs = temp.path().join("cancelled");
    fs::create_dir_all(&cancelled_outputs).unwrap();
    let cancel = MediaCancelToken::new();
    cancel.cancel();
    let cancelled = separate_stems(
        StemSeparationRequest {
            source: &source,
            output_dir: &cancelled_outputs,
            execution: StemExecution::Local { model_dir: &models },
        },
        &cancel,
        None,
    );
    assert!(matches!(cancelled, Err(MediaError::Cancelled)));
    assert_clean_output_dir(&cancelled_outputs);

    let hosted_outputs = temp.path().join("hosted");
    fs::create_dir_all(&hosted_outputs).unwrap();
    let hosted = separate_stems(
        StemSeparationRequest {
            source: &source,
            output_dir: &hosted_outputs,
            execution: StemExecution::Hosted {
                provider: "".to_string(),
                model: "vendor/stems-v1".to_string(),
            },
        },
        &MediaCancelToken::new(),
        None,
    );
    assert!(matches!(hosted, Err(MediaError::ModelInstall(_))));
    assert_clean_output_dir(&hosted_outputs);
}
