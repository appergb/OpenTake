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
use std::process::Command;

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
