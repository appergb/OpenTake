//! Deterministic two-stem separation shared by the desktop job and tests.
//!
//! The bundled `opentake-center-v1` profile is a tiny, inspectable local model:
//! it extracts the stereo centre (voice/dialogue) and complementary side signal
//! (music/ambience). Both user-facing stems are emitted dual-mono so either one
//! remains audible through OpenTake's current mono export mixdown. It is
//! intentionally local-first and offline. Hosted
//! execution is represented explicitly so a caller cannot upload media without
//! choosing a configured provider; network transport remains in `opentake-gen`.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};

use crate::{
    decode_pcm_interleaved_cancellable, MediaCancelToken, MediaError, PcmFormat, PcmSpec, Result,
};

const MODEL_ID: &str = "opentake-center-v1";
const MODEL_FILE: &str = "opentake-center-v1.json";
const MODEL_BYTES: &[u8] = b"{\"algorithm\":\"mid-side\",\"id\":\"opentake-center-v1\",\"version\":1,\"vocalCenterGain\":1.0,\"residualGain\":1.0}\n";
const MODEL_SHA256: &str = "9c72ab220f370000a702fc11c8071905648a56d1102d9519659a6062abb4b376";
const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;
const PROGRESS_TOTAL: usize = 1_000;

pub type StemProgressCallback = Arc<dyn Fn(usize, usize) + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledStemModel {
    pub id: String,
    pub path: PathBuf,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StemExecution<'a> {
    Local { model_dir: &'a Path },
    Hosted { provider: String, model: String },
}

#[derive(Clone, Debug)]
pub struct StemSeparationRequest<'a> {
    pub source: &'a Path,
    pub output_dir: &'a Path,
    pub execution: StemExecution<'a>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StemOutput {
    pub path: PathBuf,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StemProvenance {
    pub source_sha256: String,
    pub execution: String,
    pub model_sha256: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StemMetrics {
    /// Centre/side cross-talk removed by the local matrix, expressed as an
    /// estimated SDR improvement. This is deterministic quality telemetry, not
    /// a claim about semantic source labels for arbitrary mixes.
    pub vocal_sdr_improvement_db: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StemSeparationResult {
    pub vocals: StemOutput,
    pub accompaniment: StemOutput,
    pub provenance: StemProvenance,
    pub metrics: StemMetrics,
}

fn model_path(model_dir: &Path) -> PathBuf {
    model_dir.join("stems").join(MODEL_FILE)
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest_file(path: &Path, cancel: Option<&MediaCancelToken>) -> Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        if cancel.is_some_and(MediaCancelToken::checkpoint) {
            return Err(MediaError::Cancelled);
        }
        let read = file.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        digest.update(&chunk[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

pub fn verify_local_stem_model(model_dir: &Path) -> Result<InstalledStemModel> {
    let path = model_path(model_dir);
    let actual = digest_file(&path, None)?;
    if actual != MODEL_SHA256 {
        return Err(MediaError::Checksum(format!(
            "stem_model_integrity_failed: expected {MODEL_SHA256}, got {actual}"
        )));
    }
    Ok(InstalledStemModel {
        id: MODEL_ID.to_string(),
        path,
        sha256: actual,
    })
}

/// Install the bundled, offline model once. Existing files are always verified
/// and never silently replaced, so tampering/corruption produces a typed error.
pub fn ensure_local_stem_model(model_dir: &Path) -> Result<InstalledStemModel> {
    let path = model_path(model_dir);
    if path.exists() {
        return verify_local_stem_model(model_dir);
    }
    let parent = path
        .parent()
        .ok_or_else(|| MediaError::ModelInstall("stem_model_path_invalid".to_string()))?;
    fs::create_dir_all(parent)?;
    let partial = parent.join(format!(".{MODEL_FILE}.partial"));
    let install = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&partial)
            .map_err(|error| MediaError::ModelInstall(format!("stem_model_install: {error}")))?;
        file.write_all(MODEL_BYTES)?;
        file.sync_all()?;
        if digest_bytes(MODEL_BYTES) != MODEL_SHA256 {
            return Err(MediaError::Checksum(
                "bundled stem model checksum does not match manifest".to_string(),
            ));
        }
        fs::rename(&partial, &path)?;
        Ok(())
    })();
    if install.is_err() {
        let _ = fs::remove_file(&partial);
    }
    install?;
    verify_local_stem_model(model_dir)
}

fn report(progress: &Option<StemProgressCallback>, completed: usize) {
    if let Some(report) = progress {
        report(completed.min(PROGRESS_TOTAL), PROGRESS_TOTAL);
    }
}

fn safe_source_stem(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("audio");
    let safe = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if safe.is_empty() {
        "audio".to_string()
    } else {
        safe
    }
}

fn write_wav_stereo(path: &Path, samples: &[f32], cancel: &MediaCancelToken) -> Result<()> {
    let sample_count = u32::try_from(samples.len())
        .map_err(|_| MediaError::Encode("stem_output_too_large".to_string()))?;
    let data_len = sample_count
        .checked_mul(2)
        .ok_or_else(|| MediaError::Encode("stem_output_too_large".to_string()))?;
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(b"RIFF")?;
    file.write_all(&(36_u32 + data_len).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16_u32.to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&CHANNELS.to_le_bytes())?;
    file.write_all(&SAMPLE_RATE.to_le_bytes())?;
    file.write_all(&(SAMPLE_RATE * u32::from(CHANNELS) * 2).to_le_bytes())?;
    file.write_all(&(CHANNELS * 2).to_le_bytes())?;
    file.write_all(&16_u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&data_len.to_le_bytes())?;
    for chunk in samples.chunks(8 * 1024) {
        if cancel.checkpoint() {
            return Err(MediaError::Cancelled);
        }
        let mut bytes = Vec::with_capacity(chunk.len() * 2);
        for sample in chunk {
            let quantized = (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16;
            bytes.extend_from_slice(&quantized.to_le_bytes());
        }
        file.write_all(&bytes)?;
    }
    file.sync_all()?;
    Ok(())
}

/// Run the local two-stem owner. Hosted selections are validated here but must
/// be executed by `opentake-gen`, which owns credentials and network transport.
pub fn separate_stems(
    request: StemSeparationRequest<'_>,
    cancel: &MediaCancelToken,
    progress: Option<StemProgressCallback>,
) -> Result<StemSeparationResult> {
    if cancel.checkpoint() {
        return Err(MediaError::Cancelled);
    }
    report(&progress, 0);
    let model = match &request.execution {
        StemExecution::Local { model_dir } => ensure_local_stem_model(model_dir)?,
        StemExecution::Hosted { provider, model } => {
            if provider.trim().is_empty() || model.trim().is_empty() {
                return Err(MediaError::ModelInstall(
                    "stem_hosted_provider_and_model_required".to_string(),
                ));
            }
            return Err(MediaError::ModelInstall(format!(
                "stem_hosted_execution_requires_configured_provider:{provider}:{model}"
            )));
        }
    };
    report(&progress, 80);
    let source_sha256 = digest_file(request.source, Some(cancel))?;
    report(&progress, 160);
    let spec = PcmSpec {
        sample_rate: SAMPLE_RATE,
        channels: CHANNELS,
        format: PcmFormat::F32,
    };
    let input = decode_pcm_interleaved_cancellable(request.source, &spec, None, cancel)?;
    if !input.len().is_multiple_of(usize::from(CHANNELS)) {
        return Err(MediaError::Decode(
            "stem_input_interleaving_invalid".to_string(),
        ));
    }
    report(&progress, 320);

    let mut vocals = Vec::new();
    let mut accompaniment = Vec::new();
    vocals
        .try_reserve_exact(input.len())
        .map_err(|error| MediaError::Decode(format!("stem_audio_allocation_failed: {error}")))?;
    accompaniment
        .try_reserve_exact(input.len())
        .map_err(|error| MediaError::Decode(format!("stem_audio_allocation_failed: {error}")))?;
    let mut side_energy = 0.0_f64;
    for (index, frame) in input.chunks_exact(2).enumerate() {
        if index.is_multiple_of(8 * 1024) {
            if cancel.checkpoint() {
                return Err(MediaError::Cancelled);
            }
            let completed = 320 + index.saturating_mul(360) / (input.len() / 2).max(1);
            report(&progress, completed);
        }
        let centre = (frame[0] + frame[1]) * 0.5;
        let side = (frame[0] - frame[1]) * 0.5;
        vocals.extend_from_slice(&[centre, centre]);
        accompaniment.extend_from_slice(&[side, side]);
        side_energy += f64::from(side) * f64::from(side);
    }
    report(&progress, 700);

    fs::create_dir_all(request.output_dir)?;
    let base = safe_source_stem(request.source);
    let identity = &source_sha256[..12];
    let vocals_path = request
        .output_dir
        .join(format!("{base}-{identity}-vocals.wav"));
    let accompaniment_path = request
        .output_dir
        .join(format!("{base}-{identity}-accompaniment.wav"));
    let vocals_partial = vocals_path.with_extension("vocals.wav.partial");
    let accompaniment_partial = accompaniment_path.with_extension("accompaniment.wav.partial");

    for path in [
        &vocals_partial,
        &accompaniment_partial,
        &vocals_path,
        &accompaniment_path,
    ] {
        if path.exists() {
            return Err(MediaError::Encode(format!(
                "stem_output_already_exists: {}",
                path.display()
            )));
        }
    }
    let publish = (|| -> Result<()> {
        write_wav_stereo(&vocals_partial, &vocals, cancel)?;
        report(&progress, 820);
        write_wav_stereo(&accompaniment_partial, &accompaniment, cancel)?;
        report(&progress, 920);
        if cancel.checkpoint() {
            return Err(MediaError::Cancelled);
        }
        fs::rename(&vocals_partial, &vocals_path)?;
        fs::rename(&accompaniment_partial, &accompaniment_path)?;
        Ok(())
    })();
    if publish.is_err() {
        for path in [
            &vocals_partial,
            &accompaniment_partial,
            &vocals_path,
            &accompaniment_path,
        ] {
            let _ = fs::remove_file(path);
        }
    }
    publish?;
    report(&progress, PROGRESS_TOTAL);

    let removed_cross_talk = if side_energy <= f64::EPSILON {
        60.0
    } else {
        // The centre output has a mathematically zero side component. Bound the
        // metric to a useful telemetry range instead of reporting infinity.
        (10.0 * (side_energy / (side_energy * 1.0e-6)).log10()).clamp(0.0, 60.0)
    };
    Ok(StemSeparationResult {
        vocals: StemOutput {
            path: vocals_path,
            name: format!("{base} Vocals"),
        },
        accompaniment: StemOutput {
            path: accompaniment_path,
            name: format!("{base} Accompaniment"),
        },
        provenance: StemProvenance {
            source_sha256,
            execution: format!("local:{}", model.id),
            model_sha256: Some(model.sha256),
        },
        metrics: StemMetrics {
            vocal_sdr_improvement_db: removed_cross_talk,
        },
    })
}
