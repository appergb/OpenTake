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

/// Fast installed-state check. A receipt is written only after all checks pass.
/// Full hashes are rechecked by `verify_installed` before loading a model.
pub fn installed(models_dir: &Path, m: &Manifest) -> Option<InstalledModel> {
    let dir = install_dir(models_dir, m);
    let receipt: Manifest =
        serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).ok()?).ok()?;
    if receipt != *m {
        return None;
    }
    let image = dir.join("image_encoder.onnx");
    let text = dir.join("text_encoder.onnx");
    let tokenizer = dir.join("tokenizer");
    for (path, file) in [(&image, &m.image_encoder), (&text, &m.text_encoder)] {
        if !matches_size(path, file) {
            return None;
        }
    }
    let token = tokenizer.join("tokenizer.json");
    if !token.is_file()
        || (m.tokenizer.name.ends_with(".json") && !matches_size(&token, &m.tokenizer))
    {
        return None;
    }
    Some(InstalledModel {
        image_encoder: image,
        text_encoder: text,
        tokenizer_folder: tokenizer,
        spec: m.spec(),
    })
}

fn matches_size(path: &Path, file: &ManifestFile) -> bool {
    file.bytes > 0
        && std::fs::metadata(path)
            .is_ok_and(|meta| meta.is_file() && meta.len() == file.bytes as u64)
}

/// Verify the actual installed bytes before inference, including offline copies.
pub fn verify_installed(models_dir: &Path, m: &Manifest) -> Result<InstalledModel> {
    let model = installed(models_dir, m)
        .ok_or_else(|| MediaError::ModelInstall("model missing or incomplete".into()))?;
    verify_sha256(&model.image_encoder, &m.image_encoder.sha256)?;
    verify_sha256(&model.text_encoder, &m.text_encoder.sha256)?;
    if m.tokenizer.name.ends_with(".json") {
        verify_sha256(
            &model.tokenizer_folder.join("tokenizer.json"),
            &m.tokenizer.sha256,
        )?;
    }
    Ok(model)
}

#[cfg(feature = "model-download")]
fn validate_manifest(m: &Manifest) -> Result<()> {
    use std::path::Component;
    if m.model.is_empty()
        || !m
            .model
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(MediaError::ModelInstall("invalid model name".into()));
    }
    for f in [&m.image_encoder, &m.text_encoder, &m.tokenizer] {
        if f.name.is_empty()
            || !Path::new(&f.name)
                .components()
                .all(|p| matches!(p, Component::Normal(_)))
            || f.name.contains('\\')
            || f.bytes <= 0
            || f.sha256.len() != 64
            || !f
                .sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(MediaError::ModelInstall(format!(
                "invalid manifest entry: {}",
                f.name
            )));
        }
    }
    Ok(())
}

#[cfg(feature = "model-download")]
fn verify_file(path: &Path, file: &ManifestFile) -> Result<()> {
    if !matches_size(path, file) {
        return Err(MediaError::ModelInstall(format!(
            "size mismatch: {} (expected {})",
            file.name, file.bytes
        )));
    }
    verify_sha256(path, &file.sha256)
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

/// Download and verify all artifacts before atomically publishing the directory.
/// Temporary files are removed on errors and cancellation.
#[cfg(feature = "model-download")]
pub async fn install(
    models_dir: &Path,
    m: &Manifest,
    base_url: &str,
    on_progress: impl Fn(f64),
) -> Result<InstalledModel> {
    use futures_util::StreamExt;
    validate_manifest(m)?;
    if let Ok(existing) = verify_installed(models_dir, m) {
        on_progress(1.0);
        return Ok(existing);
    }
    std::fs::create_dir_all(models_dir)?;
    let staging = tempfile::Builder::new()
        .prefix(".search-download-")
        .tempdir_in(models_dir)?;
    let files = [&m.image_encoder, &m.text_encoder, &m.tokenizer];
    let total_bytes: f64 = files.iter().map(|f| f.bytes as f64).sum();
    let mut done_bytes = 0.0;
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(3600))
        .build()
        .map_err(|e| MediaError::ModelInstall(format!("HTTP client: {e}")))?;
    for file in files {
        let url = format!("{}/{}", base_url.trim_end_matches('/'), file.name);
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| MediaError::ModelInstall(format!("GET {}: {e}", file.name)))?;
        if !resp.status().is_success() {
            return Err(MediaError::ModelInstall(format!(
                "GET {} -> {}",
                file.name,
                resp.status()
            )));
        }
        let dest = staging.path().join(&file.name);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(&dest)?;
        let mut stream = resp.bytes_stream();
        let mut file_done = 0u64;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|e| MediaError::ModelInstall(format!("stream {}: {e}", file.name)))?;
            file_done += chunk.len() as u64;
            if file_done > file.bytes as u64 {
                return Err(MediaError::ModelInstall(format!(
                    "size exceeded: {}",
                    file.name
                )));
            }
            use std::io::Write;
            out.write_all(&chunk)?;
            on_progress(((done_bytes + file_done as f64) / total_bytes).min(0.99));
        }
        out.sync_all()?;
        drop(out);
        verify_file(&dest, file)?;
        done_bytes += file.bytes as f64;
    }
    let model = publish_staging(models_dir, m, staging.path())?;
    on_progress(1.0);
    Ok(model)
}

/// Offline installation from the same repository-relative files as the download.
/// Does not contact the network. Checks source hashes before copying, then checks
/// the staged copies; an existing installation is untouched on verification errors.
#[cfg(feature = "model-download")]
pub fn install_from_directory(
    models_dir: &Path,
    m: &Manifest,
    source: &Path,
) -> Result<InstalledModel> {
    validate_manifest(m)?;
    let files = [&m.image_encoder, &m.text_encoder, &m.tokenizer];
    for file in files {
        verify_file(&source.join(&file.name), file)?;
    }
    if let Ok(existing) = verify_installed(models_dir, m) {
        return Ok(existing);
    }
    std::fs::create_dir_all(models_dir)?;
    let staging = tempfile::Builder::new()
        .prefix(".search-offline-")
        .tempdir_in(models_dir)?;
    for file in files {
        let dest = staging.path().join(&file.name);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(source.join(&file.name), &dest)?;
        verify_file(&dest, file)?;
    }
    publish_staging(models_dir, m, staging.path())
}

#[cfg(feature = "model-download")]
fn publish_staging(models_dir: &Path, m: &Manifest, staging: &Path) -> Result<InstalledModel> {
    let prepared = staging.join("installed");
    std::fs::create_dir(&prepared)?;
    std::fs::rename(
        staging.join(&m.image_encoder.name),
        prepared.join("image_encoder.onnx"),
    )?;
    std::fs::rename(
        staging.join(&m.text_encoder.name),
        prepared.join("text_encoder.onnx"),
    )?;
    if m.tokenizer.name.ends_with(".json") {
        std::fs::create_dir(prepared.join("tokenizer"))?;
        std::fs::rename(
            staging.join(&m.tokenizer.name),
            prepared.join("tokenizer/tokenizer.json"),
        )?;
    } else {
        let extracted = unzip_single_top_level(&staging.join(&m.tokenizer.name), staging)?;
        if !extracted.join("tokenizer.json").is_file() {
            return Err(MediaError::ModelInstall(
                "tokenizer.json missing from archive".into(),
            ));
        }
        std::fs::rename(extracted, prepared.join("tokenizer"))?;
    }
    std::fs::write(prepared.join("spec.json"), serde_json::to_vec(&m.spec())?)?;
    std::fs::write(prepared.join("manifest.json"), serde_json::to_vec(m)?)?;
    let dir = install_dir(models_dir, m);
    // Preserve a previous (incomplete/corrupt) directory until replacement succeeds.
    let backup = staging.join("previous");
    let previous = dir.exists();
    if previous {
        std::fs::rename(&dir, &backup)?;
    }
    if let Err(error) = std::fs::rename(&prepared, &dir) {
        if previous {
            std::fs::rename(&backup, &dir)?;
        }
        return Err(error.into());
    }
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
        let mut m = manifest();
        m.image_encoder.bytes = 1;
        m.text_encoder.bytes = 1;
        let id = install_dir(dir.path(), &m);
        std::fs::create_dir_all(id.join("tokenizer")).unwrap();
        std::fs::write(id.join("manifest.json"), serde_json::to_vec(&m).unwrap()).unwrap();
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
    #[test]
    fn installed_rejects_unverified_loose_files() {
        let root = tempfile::tempdir().unwrap();
        let m = manifest();
        let dir = install_dir(root.path(), &m);
        std::fs::create_dir_all(dir.join("tokenizer")).unwrap();
        std::fs::write(dir.join("image_encoder.onnx"), b"i").unwrap();
        std::fs::write(dir.join("text_encoder.onnx"), b"t").unwrap();
        std::fs::write(dir.join("tokenizer/tokenizer.json"), b"{}").unwrap();
        assert!(installed(root.path(), &m).is_none());
    }

    #[cfg(feature = "model-download")]
    #[tokio::test]
    async fn downloads_nested_onnx_and_raw_tokenizer() {
        use std::io::Read;
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.local_addr().unwrap());
        let mut m = manifest();
        for (f, name, bytes) in [
            (
                &mut m.image_encoder,
                "onnx/vision_model.onnx",
                b"image".as_slice(),
            ),
            (
                &mut m.text_encoder,
                "onnx/text_model.onnx",
                b"text".as_slice(),
            ),
            (&mut m.tokenizer, "tokenizer.json", b"{}".as_slice()),
        ] {
            f.name = name.into();
            f.sha256 = sha256_hex(bytes);
            f.bytes = bytes.len() as i64;
        }
        let serving = std::thread::spawn(move || {
            for (path, body) in [
                ("onnx/vision_model.onnx", "image"),
                ("onnx/text_model.onnx", "text"),
                ("tokenizer.json", "{}"),
            ] {
                let (mut stream, _) = server.accept().unwrap();
                let mut request = [0; 4096];
                let n = stream.read(&mut request).unwrap();
                assert!(
                    String::from_utf8_lossy(&request[..n]).starts_with(&format!("GET /{path} "))
                );
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        let root = tempfile::tempdir().unwrap();
        let result = install(root.path(), &m, &url, |_| {}).await;
        // If the downloader fails early, do not block waiting for unused requests.
        assert!(result.is_ok(), "{result:?}");
        serving.join().unwrap();
        let got = result.unwrap();
        assert_eq!(
            std::fs::read(got.tokenizer_folder.join("tokenizer.json")).unwrap(),
            b"{}"
        );
        assert_eq!(
            install(root.path(), &m, "http://127.0.0.1:1", |_| {})
                .await
                .unwrap(),
            got
        );
    }

    #[cfg(feature = "model-download")]
    fn offline_fixture(source: &Path) -> Manifest {
        let mut m = manifest();
        for (file, name, bytes) in [
            (
                &mut m.image_encoder,
                "onnx/vision_model.onnx",
                b"image".as_slice(),
            ),
            (
                &mut m.text_encoder,
                "onnx/text_model.onnx",
                b"text".as_slice(),
            ),
            (&mut m.tokenizer, "tokenizer.json", b"{}".as_slice()),
        ] {
            file.name = name.into();
            file.sha256 = sha256_hex(bytes);
            file.bytes = bytes.len() as i64;
            let path = source.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
        m
    }

    #[cfg(feature = "model-download")]
    #[test]
    fn offline_install_verifies_bytes_and_preserves_existing_on_failure() {
        let source = tempfile::tempdir().unwrap();
        let models = tempfile::tempdir().unwrap();
        let m = offline_fixture(source.path());
        let got = install_from_directory(models.path(), &m, source.path()).unwrap();
        assert_eq!(verify_installed(models.path(), &m).unwrap(), got);
        std::fs::write(source.path().join(&m.image_encoder.name), b"wrong").unwrap();
        assert!(matches!(
            install_from_directory(models.path(), &m, source.path()),
            Err(MediaError::Checksum(_))
        ));
        assert!(verify_installed(models.path(), &m).is_ok());
        std::fs::write(&got.image_encoder, b"wrong").unwrap();
        assert!(verify_installed(models.path(), &m).is_err());
        std::fs::write(&got.image_encoder, b"i").unwrap();
        assert!(installed(models.path(), &m).is_none());
    }

    #[cfg(feature = "model-download")]
    #[test]
    fn offline_install_rejects_bad_size_hash_and_traversal() {
        let source = tempfile::tempdir().unwrap();
        let models = tempfile::tempdir().unwrap();
        let m = offline_fixture(source.path());
        for mutation in 0..3 {
            let mut bad = m.clone();
            match mutation {
                0 => bad.image_encoder.bytes += 1,
                1 => bad.image_encoder.sha256.clear(),
                _ => bad.image_encoder.name = "../outside.onnx".into(),
            }
            assert!(install_from_directory(models.path(), &bad, source.path()).is_err());
        }
        assert_eq!(std::fs::read_dir(models.path()).unwrap().count(), 0);
    }

    #[cfg(feature = "model-download")]
    #[tokio::test]
    async fn http_failure_cleans_staging_and_does_not_install() {
        use std::io::Read;
        for (status, body, expected_size) in [
            ("404 Not Found", "", 5),
            ("200 OK", "bad", 5),
            ("200 OK", "wrong", 5),
            ("200 OK", "toolong", 5),
        ] {
            let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}", server.local_addr().unwrap());
            let serving = std::thread::spawn(move || {
                let (mut stream, _) = server.accept().unwrap();
                let mut request = [0; 4096];
                let _ = stream.read(&mut request).unwrap();
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            });
            let source = tempfile::tempdir().unwrap();
            let models = tempfile::tempdir().unwrap();
            let mut m = offline_fixture(source.path());
            m.image_encoder.bytes = expected_size;
            assert!(install(models.path(), &m, &url, |_| {}).await.is_err());
            serving.join().unwrap();
            assert_eq!(std::fs::read_dir(models.path()).unwrap().count(), 0);
        }
    }
}
