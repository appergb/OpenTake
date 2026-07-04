//! Model weight download / verify / unzip / install. Port of
//! `Search/Models/ModelDownloader.swift`, adapted to ONNX (no `compileModel`).
//!
//! Install layout:
//! `<models_dir>/<model>-v<version>/{image_encoder.onnx, text_encoder.onnx,
//! tokenizer/, spec.json}`.
//!
//! The manifest/spec types, install-path resolution, installed-state detection,
//! and streaming SHA-256 verification are always available (no network). The
//! actual HTTP download + unzip live behind the `model-download` feature so the
//! default dependency tree carries no HTTP/TLS stack and the default test run
//! stays offline.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{MediaError, Result};
use crate::search::embedder::EmbedderSpec;

/// One downloadable file's manifest entry.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestFile {
    pub name: String,
    pub sha256: String,
    pub bytes: i64,
}

/// Model download manifest (port of `ModelDownloader.Manifest`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub model: String,
    pub version: i32,
    pub embedding_dim: usize,
    pub image_size: u32,
    pub context_length: usize,
    pub image_encoder: ManifestFile,
    pub text_encoder: ManifestFile,
    pub tokenizer: ManifestFile,
}

impl Manifest {
    pub fn spec(&self) -> EmbedderSpec {
        EmbedderSpec {
            model: self.model.clone(),
            version: self.version,
            embedding_dim: self.embedding_dim,
            image_size: self.image_size,
            context_length: self.context_length,
            normalized: false,
        }
    }
}

/// A resolved, installed model on disk.
#[derive(Clone, Debug, PartialEq)]
pub struct InstalledModel {
    pub image_encoder: PathBuf,
    pub text_encoder: PathBuf,
    pub tokenizer_folder: PathBuf,
    pub spec: EmbedderSpec,
}

/// Install directory for a manifest: `<models_dir>/<model>-v<version>`.
pub fn install_dir(models_dir: &Path, m: &Manifest) -> PathBuf {
    models_dir.join(format!("{}-v{}", m.model, m.version))
}

/// Return the installed model if all three artifacts (and `tokenizer.json`)
/// exist. Port of `installed(for:)` adapted to ONNX filenames.
pub fn installed(models_dir: &Path, m: &Manifest) -> Option<InstalledModel> {
    let dir = install_dir(models_dir, m);
    let image = dir.join("image_encoder.onnx");
    let text = dir.join("text_encoder.onnx");
    let tokenizer = dir.join("tokenizer");
    if image.exists() && text.exists() && tokenizer.join("tokenizer.json").exists() {
        Some(InstalledModel {
            image_encoder: image,
            text_encoder: text,
            tokenizer_folder: tokenizer,
            spec: m.spec(),
        })
    } else {
        None
    }
}

/// Streaming SHA-256 verification (1 MiB chunks). `Err(Checksum)` on mismatch.
/// Port of `verify(_:sha256:)`.
pub fn verify_sha256(path: &Path, expected: &str) -> Result<()> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
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
    if hex == expected {
        Ok(())
    } else {
        Err(MediaError::Checksum(
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        ))
    }
}

/// Streaming SHA-256 hex of a byte slice (pure; used by tests and the downloader).
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for b in digest.iter() {
        use std::fmt::Write;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

/// Download, verify, unzip, and install the model. Requires the `model-download`
/// feature (HTTP + zip). Idempotent: returns immediately if already installed.
#[cfg(feature = "model-download")]
pub async fn install(
    models_dir: &Path,
    m: &Manifest,
    base_url: &str,
    on_progress: impl Fn(f64),
) -> Result<InstalledModel> {
    use futures_util::StreamExt;

    if let Some(existing) = installed(models_dir, m) {
        return Ok(existing);
    }

    let staging = std::env::temp_dir().join(format!("opentake-model-{}", uuid_like()));
    std::fs::create_dir_all(&staging)?;

    let files = [&m.image_encoder, &m.text_encoder, &m.tokenizer];
    let total_bytes: i64 = files.iter().map(|f| f.bytes).sum();
    let mut done_bytes: i64 = 0;
    let client = reqwest::Client::new();

    for file in files {
        let url = format!("{}/{}", base_url.trim_end_matches('/'), file.name);
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
        let dest = staging.join(&file.name);
        let mut out = std::fs::File::create(&dest)?;
        let mut stream = resp.bytes_stream();
        let base = done_bytes as f64;
        let mut file_done: i64 = 0;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| MediaError::ModelInstall(format!("stream: {e}")))?;
            use std::io::Write;
            out.write_all(&chunk)?;
            file_done += chunk.len() as i64;
            if total_bytes > 0 {
                on_progress((base + file_done as f64) / total_bytes as f64);
            }
        }
        drop(out);
        verify_sha256(&dest, &file.sha256)?;
        done_bytes += file.bytes;
        if total_bytes > 0 {
            on_progress(done_bytes as f64 / total_bytes as f64);
        }
    }

    // Encoders are plain .onnx; the tokenizer ships as a zip with one top-level
    // folder. Unzip it.
    let tokenizer_zip = staging.join(&m.tokenizer.name);
    let tokenizer_extracted = unzip_single_top_level(&tokenizer_zip, &staging)?;

    let dir = install_dir(models_dir, m);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    std::fs::rename(
        staging.join(&m.image_encoder.name),
        dir.join("image_encoder.onnx"),
    )?;
    std::fs::rename(
        staging.join(&m.text_encoder.name),
        dir.join("text_encoder.onnx"),
    )?;
    std::fs::rename(tokenizer_extracted, dir.join("tokenizer"))?;
    std::fs::write(dir.join("spec.json"), serde_json::to_vec(&m.spec())?)?;

    let _ = std::fs::remove_dir_all(&staging);
    installed(models_dir, m).ok_or_else(|| MediaError::ModelInstall("post-install missing".into()))
}

/// Unzip a zip that contains exactly one top-level entry; returns its path.
#[cfg(feature = "model-download")]
fn unzip_single_top_level(zip_path: &Path, into: &Path) -> Result<PathBuf> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| MediaError::ModelInstall(format!("zip open: {e}")))?;
    let out_root = into.join(format!(
        "{}-extracted",
        zip_path.file_stem().unwrap_or_default().to_string_lossy()
    ));
    std::fs::create_dir_all(&out_root)?;
    let mut top_levels = std::collections::BTreeSet::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| MediaError::ModelInstall(format!("zip entry: {e}")))?;
        let Some(path) = entry.enclosed_name() else {
            continue;
        };
        if let Some(first) = path.components().next() {
            top_levels.insert(first.as_os_str().to_string_lossy().into_owned());
        }
        let out_path = out_root.join(&path);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }
    if top_levels.len() != 1 {
        return Err(MediaError::ModelInstall(format!(
            "expected one top-level entry, found {}",
            top_levels.len()
        )));
    }
    Ok(out_root.join(top_levels.into_iter().next().unwrap()))
}

/// Tiny unique suffix without pulling a uuid dependency.
#[cfg(feature = "model-download")]
fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn manifest() -> Manifest {
        Manifest {
            model: "siglip2-base-patch16-256".into(),
            version: 1,
            embedding_dim: 768,
            image_size: 256,
            context_length: 64,
            image_encoder: ManifestFile {
                name: "image_encoder.onnx".into(),
                sha256: "x".into(),
                bytes: 10,
            },
            text_encoder: ManifestFile {
                name: "text_encoder.onnx".into(),
                sha256: "y".into(),
                bytes: 20,
            },
            tokenizer: ManifestFile {
                name: "tokenizer.zip".into(),
                sha256: "z".into(),
                bytes: 30,
            },
        }
    }

    #[test]
    fn install_dir_uses_model_and_version() {
        let d = install_dir(Path::new("/models"), &manifest());
        assert_eq!(d, PathBuf::from("/models/siglip2-base-patch16-256-v1"));
    }

    #[test]
    fn installed_none_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(installed(dir.path(), &manifest()).is_none());
    }

    #[test]
    fn installed_detected_when_all_artifacts_present() {
        let dir = tempfile::tempdir().unwrap();
        let m = manifest();
        let id = install_dir(dir.path(), &m);
        std::fs::create_dir_all(id.join("tokenizer")).unwrap();
        std::fs::write(id.join("image_encoder.onnx"), b"i").unwrap();
        std::fs::write(id.join("text_encoder.onnx"), b"t").unwrap();
        std::fs::write(id.join("tokenizer/tokenizer.json"), b"{}").unwrap();
        let got = installed(dir.path(), &m).unwrap();
        assert_eq!(got.spec.embedding_dim, 768);
        assert!(got.image_encoder.ends_with("image_encoder.onnx"));
    }

    #[test]
    fn installed_partial_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let m = manifest();
        let id = install_dir(dir.path(), &m);
        std::fs::create_dir_all(&id).unwrap();
        std::fs::write(id.join("image_encoder.onnx"), b"i").unwrap();
        // missing text encoder + tokenizer.
        assert!(installed(dir.path(), &m).is_none());
    }

    #[test]
    fn verify_sha256_matches_and_mismatches() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"hello world").unwrap();
        f.flush().unwrap();
        let expected = sha256_hex(b"hello world");
        assert!(verify_sha256(f.path(), &expected).is_ok());
        assert!(matches!(
            verify_sha256(f.path(), "deadbeef"),
            Err(MediaError::Checksum(_))
        ));
    }

    #[test]
    fn sha256_hex_is_64_chars() {
        assert_eq!(sha256_hex(b"abc").len(), 64);
    }

    #[test]
    fn manifest_spec_roundtrips() {
        let m = manifest();
        assert_eq!(m.spec().embedding_dim, 768);
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("imageEncoder"));
        assert!(json.contains("embeddingDim"));
        let back: Manifest = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }
}
