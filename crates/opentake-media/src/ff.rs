//! Thin internal helpers for driving bundled or development `ffmpeg`/`ffprobe`
//! binaries.
//!
//! We deliberately do **not** link libav*: the local toolchain is ffmpeg 8.1
//! (libavcodec 62) which the C-binding crates do not support, and pkg-config is
//! absent. ffmpeg-sidecar shells out to binaries on `PATH`; these helpers wrap
//! binary discovery and one-shot ffprobe JSON queries so the higher-level decode
//! modules stay readable.
//!
//! Packaged binaries live beside the OpenTake executable. Environment overrides
//! `OPENTAKE_FFMPEG` / `OPENTAKE_FFPROBE` remain available to tests and
//! development tools; the desktop shell pins them to the bundled sidecars before
//! media initialization.

use std::ffi::OsString;
use std::io::{Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ffmpeg_sidecar::command::FfmpegCommand;

fn sidecar_filename(binary: &str) -> String {
    if cfg!(windows) {
        format!("{binary}.exe")
    } else {
        binary.to_string()
    }
}

/// Return a regular, non-symlink sidecar next to `executable`.
///
/// Keeping this pure helper separate makes the packaged-path security boundary
/// deterministic to test without mutating the process executable or PATH.
pub fn packaged_sidecar_beside(executable: &Path, binary: &str) -> Option<PathBuf> {
    let parent = executable.parent()?;
    let candidate = parent.join(sidecar_filename(binary));
    let metadata = std::fs::symlink_metadata(&candidate).ok()?;
    if metadata.file_type().is_file() && !metadata.file_type().is_symlink() {
        Some(candidate)
    } else {
        None
    }
}

/// Find a verified-by-the-package-manager sidecar beside the current executable.
/// Runtime code still checks that the path is a regular file; the build/package
/// pipeline owns its pinned SHA-256 and version verification.
pub fn packaged_sidecar_path(binary: &str) -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    packaged_sidecar_beside(&executable, binary)
}

/// Resolve one CLI tool without mutating global process state in tests.
///
/// An explicit override always wins, then a regular non-symlink packaged
/// sidecar beside the executable, and finally the platform command name on
/// `PATH`.
fn resolve_cli_path(
    override_path: Option<OsString>,
    executable: Option<&Path>,
    binary: &str,
) -> OsString {
    override_path
        .or_else(|| {
            executable
                .and_then(|path| packaged_sidecar_beside(path, binary))
                .map(PathBuf::into_os_string)
        })
        .unwrap_or_else(|| OsString::from(binary))
}

/// Path to `ffmpeg`: explicit development override, packaged sidecar, then PATH.
pub fn ffmpeg_path() -> OsString {
    let executable = std::env::current_exe().ok();
    resolve_cli_path(
        std::env::var_os("OPENTAKE_FFMPEG"),
        executable.as_deref(),
        "ffmpeg",
    )
}

/// Path to `ffprobe`: explicit development override, packaged sidecar, then PATH.
pub fn ffprobe_path() -> OsString {
    let executable = std::env::current_exe().ok();
    resolve_cli_path(
        std::env::var_os("OPENTAKE_FFPROBE"),
        executable.as_deref(),
        "ffprobe",
    )
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
        let temp = tempfile::tempdir().unwrap();
        let executable = temp.path().join(if cfg!(windows) {
            "opentake.exe"
        } else {
            "opentake"
        });
        std::fs::write(&executable, b"app").unwrap();
        std::fs::write(temp.path().join(sidecar_filename("ffmpeg")), b"sidecar").unwrap();

        assert_eq!(
            resolve_cli_path(
                Some(OsString::from("/opt/opentake/custom-ffmpeg")),
                Some(&executable),
                "ffmpeg",
            ),
            OsString::from("/opt/opentake/custom-ffmpeg"),
        );
    }

    #[test]
    fn default_ffprobe_is_ffprobe() {
        assert_eq!(
            resolve_cli_path(None, None, "ffprobe"),
            OsString::from("ffprobe")
        );
    }

    #[test]
    fn packaged_sidecar_must_be_regular_and_beside_executable() {
        let temp = tempfile::tempdir().unwrap();
        let executable = temp.path().join(if cfg!(windows) {
            "opentake.exe"
        } else {
            "opentake"
        });
        std::fs::write(&executable, b"app").unwrap();
        let sidecar = temp.path().join(sidecar_filename("ffmpeg"));
        std::fs::write(&sidecar, b"sidecar").unwrap();

        assert_eq!(
            packaged_sidecar_beside(&executable, "ffmpeg"),
            Some(sidecar.clone())
        );
        assert_eq!(
            resolve_cli_path(None, Some(&executable), "ffmpeg"),
            sidecar.into_os_string()
        );
        assert_eq!(packaged_sidecar_beside(&executable, "ffprobe"), None);
    }

    #[cfg(unix)]
    #[test]
    fn packaged_sidecar_rejects_symlink() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let executable = temp.path().join("opentake");
        let outside = temp.path().join("outside");
        let sidecar = temp.path().join("ffmpeg");
        std::fs::write(&executable, b"app").unwrap();
        std::fs::write(&outside, b"untrusted").unwrap();
        symlink(&outside, &sidecar).unwrap();

        assert_eq!(packaged_sidecar_beside(&executable, "ffmpeg"), None);
    }
}
