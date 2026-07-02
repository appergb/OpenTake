//! whisper ggml model management: install-path resolution, installed-state
//! detection, SHA-1 integrity verification, and (behind the `model-download`
//! feature) an async streaming download with progress.
//!
//! Upstream (`Transcription/Transcription.swift`) uses Apple's on-device
//! `SpeechTranscriber` with `AssetInventory.assetInstallationRequest(...)` — the
//! OS downloads/installs the speech asset transparently the first time a locale
//! is used. OpenTake replaces that Apple-only backend with whisper.cpp, which
//! needs a ggml weight file on disk, so we mirror the *UX* (check → download once
//! → transcribe) with an explicit model instead of an OS asset.
//!
//! **Model choice — `ggml-base` (multilingual, ~142 MiB).** Upstream's
//! `SpeechTranscriber` is multilingual and auto-selects the best supported
//! locale, so the faithful equivalent is a *multilingual* whisper model (not an
//! `.en` variant). `base` is whisper.cpp's default quality/speed/size balance for
//! a CPU build and keeps the one-time download modest.
//!
//! **Integrity — SHA-1.** whisper.cpp publishes SHA-1 checksums for its ggml
//! files (`models/download-ggml-model.sh` / `models/README.md`), so we verify
//! against the published SHA-1 rather than an unverifiable SHA-256. The SHA-1
//! machinery (and the reqwest download) is compiled only under `model-download`;
//! the manifest + path/installed helpers are always available (no network).

use std::path::{Path, PathBuf};

/// Subdirectory under the app models dir where whisper ggml files live, kept
/// distinct from the SigLIP search models (`<model>-v<version>/`).
pub const WHISPER_SUBDIR: &str = "whisper";

/// One downloadable whisper ggml model: filename, published SHA-1, byte size, and
/// the host it is fetched from. `Default` is the app's chosen model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WhisperModel {
    /// ggml filename (also the on-disk name), e.g. `ggml-base.bin`.
    pub file_name: &'static str,
    /// Published SHA-1 (lowercase hex) from whisper.cpp's model list.
    pub sha1: &'static str,
    /// Approximate download size in bytes (for a size hint before downloading).
    pub bytes: u64,
    /// Base URL the file is fetched from (`{base_url}/{file_name}`).
    pub base_url: &'static str,
    /// Short human label for the UI (`"base (multilingual)"`).
    pub label: &'static str,
}

/// The app's default whisper model: multilingual `base` (~142 MiB). SHA-1 from
/// whisper.cpp `models/README.md`. Served from the official Hugging Face repo's
/// `resolve/main` (raw file) endpoint.
pub const DEFAULT_MODEL: WhisperModel = WhisperModel {
    file_name: "ggml-base.bin",
    sha1: "465707469ff3a37a2b9b8d8f89f2f99de7299dac",
    bytes: 147_951_465,
    base_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main",
    label: "base (multilingual)",
};

/// The install path for `model` under `models_dir`:
/// `<models_dir>/whisper/<file_name>`.
pub fn model_path(models_dir: &Path, model: &WhisperModel) -> PathBuf {
    models_dir.join(WHISPER_SUBDIR).join(model.file_name)
}

/// The resolved on-disk model path if the file exists, else `None`. Existence
/// only — integrity is checked at download time (a re-verify on every load would
/// re-hash ~142 MiB per transcription).
pub fn installed(models_dir: &Path, model: &WhisperModel) -> Option<PathBuf> {
    let p = model_path(models_dir, model);
    p.is_file().then_some(p)
}

/// Streaming SHA-1 verification (1 MiB chunks) against the model's published
/// hash. `Err(Checksum)` on mismatch. Compiled only under `model-download` (the
/// only path that produces a file needing verification), so the default tree
/// carries no `sha1` crate.
#[cfg(feature = "model-download")]
pub fn verify_sha1(path: &Path, expected: &str) -> crate::error::Result<()> {
    use crate::error::MediaError;
    use sha1::{Digest, Sha1};

    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        use std::io::Read;
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for b in digest.iter() {
        use std::fmt::Write;
        let _ = write!(hex, "{b:02x}");
    }
    if hex.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(MediaError::Checksum(format!(
            "{} (sha1 {hex} != {expected})",
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        )))
    }
}

/// Download `model` into `<models_dir>/whisper/` with streamed progress, verify
/// its SHA-1, and atomically move it into place. Idempotent: returns the existing
/// path immediately if already installed. Requires the `model-download` feature
/// (reqwest + sha1). `on_progress(fraction)` is called with `0.0..=1.0` as bytes
/// arrive. Mirrors `search::model_download::install`'s download/verify/rename
/// shape, specialized to a single un-zipped ggml file.
#[cfg(feature = "model-download")]
pub async fn download(
    models_dir: &Path,
    model: &WhisperModel,
    on_progress: impl Fn(f64),
) -> crate::error::Result<PathBuf> {
    use crate::error::MediaError;
    use futures_util::StreamExt;

    if let Some(existing) = installed(models_dir, model) {
        return Ok(existing);
    }

    let dir = models_dir.join(WHISPER_SUBDIR);
    std::fs::create_dir_all(&dir)?;
    // Download to a staging file first so a partial/aborted download never looks
    // installed; rename into place only after SHA-1 verification.
    let staging = dir.join(format!("{}.part", model.file_name));

    let url = format!(
        "{}/{}",
        model.base_url.trim_end_matches('/'),
        model.file_name
    );
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| MediaError::ModelInstall(format!("GET {url}: {e}")))?;
    if !resp.status().is_success() {
        return Err(MediaError::ModelInstall(format!(
            "GET {url} -> {}",
            resp.status()
        )));
    }
    // Prefer the server's Content-Length for the progress denominator; fall back
    // to the manifest's byte estimate if the header is absent.
    let total = resp.content_length().unwrap_or(model.bytes).max(1);

    let mut out = std::fs::File::create(&staging)?;
    let mut stream = resp.bytes_stream();
    let mut done: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| MediaError::ModelInstall(format!("stream: {e}")))?;
        use std::io::Write;
        out.write_all(&chunk)?;
        done += chunk.len() as u64;
        on_progress((done as f64 / total as f64).min(1.0));
    }
    drop(out);

    verify_sha1(&staging, model.sha1).inspect_err(|_| {
        let _ = std::fs::remove_file(&staging);
    })?;

    let final_path = model_path(models_dir, model);
    std::fs::rename(&staging, &final_path)?;
    on_progress(1.0);
    Ok(final_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_path_is_under_whisper_subdir() {
        let p = model_path(Path::new("/models"), &DEFAULT_MODEL);
        assert_eq!(p, PathBuf::from("/models/whisper/ggml-base.bin"));
    }

    #[test]
    fn installed_none_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(installed(dir.path(), &DEFAULT_MODEL).is_none());
    }

    #[test]
    fn installed_some_when_file_present() {
        let dir = tempfile::tempdir().unwrap();
        let p = model_path(dir.path(), &DEFAULT_MODEL);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, b"ggml").unwrap();
        assert_eq!(installed(dir.path(), &DEFAULT_MODEL), Some(p));
    }

    #[test]
    fn default_model_is_multilingual_base() {
        // Guards the model choice: multilingual (no `.en`) base weights.
        assert_eq!(DEFAULT_MODEL.file_name, "ggml-base.bin");
        assert!(!DEFAULT_MODEL.file_name.contains(".en"));
        assert_eq!(DEFAULT_MODEL.sha1.len(), 40); // SHA-1 hex length
    }

    #[cfg(feature = "model-download")]
    #[test]
    fn verify_sha1_matches_and_mismatches() {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"hello world").unwrap();
        f.flush().unwrap();
        // Known SHA-1 of "hello world".
        let expected = "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed";
        assert!(verify_sha1(f.path(), expected).is_ok());
        assert!(verify_sha1(f.path(), "deadbeef").is_err());
    }
}
