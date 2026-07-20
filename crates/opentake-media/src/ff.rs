//! Thin internal helpers for driving the system `ffmpeg`/`ffprobe` binaries.
//!
//! We deliberately do **not** link libav*: the local toolchain is ffmpeg 8.1
//! (libavcodec 62) which the C-binding crates do not support, and pkg-config is
//! absent. ffmpeg-sidecar shells out to binaries on `PATH`; these helpers wrap
//! binary discovery and one-shot ffprobe JSON queries so the higher-level decode
//! modules stay readable.
//!
//! Environment overrides `OPENTAKE_FFMPEG` / `OPENTAKE_FFPROBE` let callers (and
//! packaged builds) point at a bundled binary.

use std::ffi::OsString;
use std::io::{Seek, SeekFrom};
use std::process::{Command, Stdio};

use ffmpeg_sidecar::command::FfmpegCommand;

/// Path to the `ffmpeg` binary: `$OPENTAKE_FFMPEG`, else `ffmpeg` on `PATH`.
pub fn ffmpeg_path() -> OsString {
    std::env::var_os("OPENTAKE_FFMPEG").unwrap_or_else(|| OsString::from("ffmpeg"))
}

/// Path to the `ffprobe` binary: `$OPENTAKE_FFPROBE`, else `ffprobe` on `PATH`.
pub fn ffprobe_path() -> OsString {
    std::env::var_os("OPENTAKE_FFPROBE").unwrap_or_else(|| OsString::from("ffprobe"))
}

/// A fresh `FfmpegCommand` bound to [`ffmpeg_path`].
pub fn ffmpeg() -> FfmpegCommand {
    FfmpegCommand::new_with_path(ffmpeg_path())
}

/// Whether `ffmpeg` is runnable (`-version` exits 0). Used by tests/integration
/// to skip when the binary is unavailable, keeping the default test run green on
/// machines without ffmpeg.
pub fn ffmpeg_available() -> bool {
    Command::new(ffmpeg_path())
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Whether `ffprobe` is runnable.
pub fn ffprobe_available() -> bool {
    Command::new(ffprobe_path())
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Ensure ffmpeg/ffprobe are available, auto-downloading them when the
/// `ffmpeg-download` feature is enabled and the binaries aren't on PATH.
///
/// After a successful download the binaries land adjacent to the current
/// executable (ffmpeg-sidecar's `sidecar_dir`), so we set the
/// `OPENTAKE_FFMPEG` / `OPENTAKE_FFPROBE` env overrides to point at them —
/// [`ffmpeg_path`] / [`ffprobe_path`] read those overrides, so the rest of the
/// media layer finds the binaries without each call site changing.
///
/// When the feature is disabled (the default for offline library builds), this
/// returns an error if the binaries are missing, so callers can log a clear
/// message rather than failing silently on the first decode.
pub fn ensure_ffmpeg() -> crate::error::Result<()> {
    if ffmpeg_available() && ffprobe_available() {
        return Ok(());
    }

    #[cfg(feature = "ffmpeg-download")]
    {
        // `auto_download` internally checks `ffmpeg_is_installed` (adjacent to
        // the exe, then PATH) and short-circuits, so calling it when ffmpeg IS
        // available is a no-op. On a fresh install it fetches + unpacks the
        // platform release into `sidecar_dir` (next to the app binary).
        ffmpeg_sidecar::download::auto_download()
            .map_err(|e| crate::error::MediaError::Ffmpeg(format!("auto-download: {e}")))?;

        // Point our env overrides at the freshly downloaded binaries so
        // `ffmpeg_path()` / `ffprobe_path()` resolve to them on every call.
        let ff = ffmpeg_sidecar::paths::ffmpeg_path();
        if ff.is_file() {
            std::env::set_var("OPENTAKE_FFMPEG", &ff);
        }
        // ffprobe sits in the same directory as ffmpeg.
        if let Some(parent) = ff.parent() {
            let probe = parent.join(if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" });
            if probe.is_file() {
                std::env::set_var("OPENTAKE_FFPROBE", &probe);
            }
        }

        if ffmpeg_available() {
            return Ok(());
        }
        return Err(crate::error::MediaError::Ffmpeg(
            "ffmpeg still unavailable after auto-download".into(),
        ));
    }

    #[cfg(not(feature = "ffmpeg-download"))]
    {
        Err(crate::error::MediaError::Ffmpeg(
            "ffmpeg not found on PATH; enable the `ffmpeg-download` feature for auto-download"
                .into(),
        ))
    }
}

/// Run `ffprobe -of json -show_streams -show_format <path>` and return parsed
/// JSON. Zero decoding — header/stream parameters only.
pub fn ffprobe_json(path: &std::path::Path) -> crate::error::Result<serde_json::Value> {
    let out = Command::new(ffprobe_path())
        .args([
            "-v",
            "quiet",
            "-of",
            "json",
            "-show_streams",
            "-show_format",
        ])
        .arg(path)
        .output()
        .map_err(|e| crate::error::MediaError::Ffmpeg(format!("ffprobe spawn: {e}")))?;
    if !out.status.success() {
        return Err(crate::error::MediaError::Ffmpeg(format!(
            "ffprobe exited {}",
            out.status
        )));
    }
    serde_json::from_slice(&out.stdout)
        .map_err(|e| crate::error::MediaError::Ffmpeg(format!("ffprobe json: {e}")))
}

/// Probe an already-open regular file without resolving an ambient pathname.
/// ffprobe's `fd:` protocol keeps normal file seek semantics (unlike `pipe:`),
/// which is required by containers whose metadata lives near the end.
pub fn ffprobe_json_file(file: &std::fs::File) -> crate::error::Result<serde_json::Value> {
    let mut input = file
        .try_clone()
        .map_err(|e| crate::error::MediaError::Ffmpeg(format!("ffprobe input clone: {e}")))?;
    input
        .seek(SeekFrom::Start(0))
        .map_err(|e| crate::error::MediaError::Ffmpeg(format!("ffprobe input rewind: {e}")))?;
    let out = Command::new(ffprobe_path())
        .args([
            "-v",
            "quiet",
            "-of",
            "json",
            "-show_streams",
            "-show_format",
            "fd:",
        ])
        .stdin(Stdio::from(input))
        .output()
        .map_err(|e| crate::error::MediaError::Ffmpeg(format!("ffprobe spawn: {e}")))?;
    if !out.status.success() {
        return Err(crate::error::MediaError::Ffmpeg(format!(
            "ffprobe fd input exited {}",
            out.status
        )));
    }
    serde_json::from_slice(&out.stdout)
        .map_err(|e| crate::error::MediaError::Ffmpeg(format!("ffprobe json: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_override_is_respected_for_ffmpeg() {
        // We can't safely mutate process env in parallel tests for the *default*,
        // but we can assert the default value when the var is unset in this proc.
        if std::env::var_os("OPENTAKE_FFMPEG").is_none() {
            assert_eq!(ffmpeg_path(), OsString::from("ffmpeg"));
        }
    }

    #[test]
    fn default_ffprobe_is_ffprobe() {
        if std::env::var_os("OPENTAKE_FFPROBE").is_none() {
            assert_eq!(ffprobe_path(), OsString::from("ffprobe"));
        }
    }
}
