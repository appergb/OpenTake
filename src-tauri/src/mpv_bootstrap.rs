//! Mirror the `libmpv-wrapper` dylib to where the libmpv plugin's loader looks.
//!
//! `tauri-plugin-libmpv` dlopens `libmpv-wrapper.<ext>` from `exe_dir` or
//! `exe_dir/lib` ONLY (see the plugin's `desktop.rs::get_wrapper`). But Tauri
//! lands `bundle.resources` under `Contents/Resources/` on macOS, so a packaged
//! app would never find the wrapper and playback would silently degrade to the
//! legacy path. Mirror the resource into `exe_dir/lib` once at startup.
//!
//! Failures only log: the preview engine's startup watchdog already falls back
//! to the legacy `<video>` path with a toast, so a missing wrapper can't brick
//! playback. Writing into the .app works for unsigned/ad-hoc builds; a signed +
//! notarized distribution must instead place the wrapper at bundle time (seal),
//! which is tracked with the ffmpeg sidecar work (#131).

use std::path::Path;

#[cfg(target_os = "windows")]
const WRAPPER: &str = "libmpv-wrapper.dll";
#[cfg(target_os = "macos")]
const WRAPPER: &str = "libmpv-wrapper.dylib";
#[cfg(all(unix, not(target_os = "macos")))]
const WRAPPER: &str = "libmpv-wrapper.so";

pub fn ensure_wrapper(app: &tauri::AppHandle) {
    use tauri::path::BaseDirectory;
    use tauri::Manager;

    let src = match app
        .path()
        .resolve(format!("lib/{WRAPPER}"), BaseDirectory::Resource)
    {
        Ok(p) if p.is_file() => p,
        // Dev tree without `setup-lib`, or non-bundled layout: nothing to
        // mirror. The plugin's own exe-adjacent search still applies.
        _ => return,
    };
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(exe_dir) = exe.parent() else {
        return;
    };
    let dst = exe_dir.join("lib").join(WRAPPER);
    if !needs_copy(&src, &dst) {
        return;
    }
    let copy = || -> std::io::Result<()> {
        std::fs::create_dir_all(dst.parent().expect("dst has a parent"))?;
        std::fs::copy(&src, &dst)?;
        Ok(())
    };
    if let Err(e) = copy() {
        eprintln!(
            "[mpv] wrapper mirror failed ({} -> {}): {e}",
            src.display(),
            dst.display()
        );
    }
}

/// Copy when the destination is missing or differs in size (a cheap change
/// detector — the wrapper is a single release artifact, not user data).
fn needs_copy(src: &Path, dst: &Path) -> bool {
    match (src.metadata(), dst.metadata()) {
        (Ok(s), Ok(d)) => s.len() != d.len(),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "opentake-mpv-bootstrap-{}-{name}",
            std::process::id()
        ))
    }

    #[test]
    fn copies_when_dst_missing_or_stale_and_skips_when_synced() {
        let src = tmp("src.bin");
        let dst = tmp("dst.bin");
        std::fs::write(&src, b"wrapper-bytes").unwrap();
        let _ = std::fs::remove_file(&dst);

        assert!(needs_copy(&src, &dst), "missing dst must copy");
        std::fs::write(&dst, b"short").unwrap();
        assert!(needs_copy(&src, &dst), "size mismatch must copy");
        std::fs::write(&dst, b"wrapper-bytes").unwrap();
        assert!(!needs_copy(&src, &dst), "same size must skip");

        std::fs::remove_file(&src).unwrap();
        std::fs::remove_file(&dst).unwrap();
    }
}
