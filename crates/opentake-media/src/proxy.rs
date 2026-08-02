//! Project-local low-resolution proxy creation.
//!
//! The source is read-only and hashed before and after transcoding. FFmpeg
//! writes to a sibling partial file which is renamed only after a successful
//! probe, so cancellation and failures never expose a truncated proxy.

use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::cancel::MediaCancelToken;
use crate::error::{MediaError, Result};
use crate::{ff, probe};

pub type ProxyProgressCallback = Arc<dyn Fn(usize, usize) + Send + Sync>;

#[derive(Clone, Copy, Debug)]
pub struct ProxyRequest<'a> {
    pub source: &'a Path,
    pub output: &'a Path,
    pub max_size: (u32, u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyResult {
    pub path: PathBuf,
    pub source_sha256: String,
    pub width: u32,
    pub height: u32,
}

fn report(progress: &Option<ProxyProgressCallback>, done: usize) {
    if let Some(callback) = progress {
        callback(done, 1000);
    }
}

pub fn file_sha256(path: &Path) -> Result<String> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn partial_path(output: &Path) -> PathBuf {
    match output.extension().and_then(|extension| extension.to_str()) {
        Some(extension) => output.with_extension(format!("{extension}.partial")),
        None => output.with_extension("partial"),
    }
}

fn cleanup_partial(path: &Path) {
    if path.is_file() {
        let _ = fs::remove_file(path);
    }
}

pub fn create_proxy(
    request: ProxyRequest<'_>,
    cancel: &MediaCancelToken,
    progress: Option<ProxyProgressCallback>,
) -> Result<ProxyResult> {
    report(&progress, 0);
    if cancel.is_cancelled() {
        return Err(MediaError::Cancelled);
    }
    if request.max_size.0 == 0 || request.max_size.1 == 0 {
        return Err(MediaError::Ffmpeg(
            "proxy dimensions must be positive".to_string(),
        ));
    }
    if !request.source.is_file() {
        return Err(MediaError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            request.source.display().to_string(),
        )));
    }
    if request.output.exists() {
        return Err(MediaError::Ffmpeg(
            "proxy destination already exists".to_string(),
        ));
    }

    let partial = partial_path(request.output);
    if partial.exists() {
        return Err(MediaError::Ffmpeg(
            "proxy partial destination already exists".to_string(),
        ));
    }
    if let Some(parent) = request.output.parent() {
        fs::create_dir_all(parent)?;
    }

    let source_sha256 = file_sha256(request.source)?;
    report(&progress, 100);
    if cancel.is_cancelled() {
        return Err(MediaError::Cancelled);
    }

    let scale = format!(
        "scale=w={}:h={}:force_original_aspect_ratio=decrease:force_divisible_by=2",
        request.max_size.0, request.max_size.1
    );
    let mut child = Command::new(ff::ffmpeg_path())
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"])
        .arg("-i")
        .arg(request.source)
        .args(["-map", "0:v:0", "-map", "0:a?", "-vf"])
        .arg(scale)
        .args([
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "23",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-movflags",
            "+faststart",
            "-f",
            "mp4",
        ])
        .arg(&partial)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| MediaError::Ffmpeg(format!("proxy spawn: {error}")))?;
    cancel.child_spawned();

    loop {
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            cleanup_partial(&partial);
            return Err(MediaError::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() || !partial.is_file() {
                    cleanup_partial(&partial);
                    return Err(MediaError::Ffmpeg(
                        "proxy transcode did not complete".to_string(),
                    ));
                }
                break;
            }
            Ok(None) => {
                report(&progress, 500);
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                cleanup_partial(&partial);
                return Err(error.into());
            }
        }
    }

    report(&progress, 900);
    if cancel.is_cancelled() {
        cleanup_partial(&partial);
        return Err(MediaError::Cancelled);
    }
    if file_sha256(request.source)? != source_sha256 {
        cleanup_partial(&partial);
        return Err(MediaError::Checksum(
            "source changed while proxy was being created".to_string(),
        ));
    }

    let metadata = match probe(&partial) {
        Ok(metadata) if metadata.has_video => metadata,
        Ok(_) => {
            cleanup_partial(&partial);
            return Err(MediaError::no_track("video", &partial));
        }
        Err(error) => {
            cleanup_partial(&partial);
            return Err(error);
        }
    };
    let (width, height) = match (metadata.width, metadata.height) {
        (Some(width), Some(height)) if width > 0 && height > 0 => (width, height),
        _ => {
            cleanup_partial(&partial);
            return Err(MediaError::Decode(
                "proxy has no usable dimensions".to_string(),
            ));
        }
    };
    if request.output.exists() {
        cleanup_partial(&partial);
        return Err(MediaError::Ffmpeg(
            "proxy destination appeared during transcode".to_string(),
        ));
    }
    fs::rename(&partial, request.output)?;
    report(&progress, 1000);

    Ok(ProxyResult {
        path: request.output.to_path_buf(),
        source_sha256,
        width,
        height,
    })
}
