//! `.waveform` binary cache — a bare little-endian `[f32]` array, byte-compatible
//! with upstream `MediaVisualCache` (`MediaVisualCache.swift:218-227`).
//!
//! Upstream wrote host-endian bytes (arm64 macOS = LE); we fix LE so files
//! written on Apple Silicon round-trip. Read validation matches upstream:
//! non-empty and length a multiple of 4.

use std::path::{Path, PathBuf};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::error::Result;

/// Cache subdirectory name (shared with the thumbnail sprite cache), kept
/// identical to upstream for same-machine cache interop.
pub const CACHE_SUBDIR: &str = "MediaVisualCache";

fn waveform_path(cache_root: &Path, key: &str) -> PathBuf {
    cache_root
        .join(CACHE_SUBDIR)
        .join(format!("{key}.waveform"))
}

/// Read `<cache_root>/MediaVisualCache/<key>.waveform` as a `Vec<f32>`.
/// Returns `None` if the file is absent, empty, or has a length not divisible
/// by 4 (upstream's corruption guard).
pub fn load_waveform(cache_root: &Path, key: &str) -> Option<Vec<f32>> {
    let bytes = std::fs::read(waveform_path(cache_root, key)).ok()?;
    if bytes.is_empty() || bytes.len() % 4 != 0 {
        return None;
    }
    let mut cursor = &bytes[..];
    let mut out = Vec::with_capacity(bytes.len() / 4);
    while let Ok(v) = cursor.read_f32::<LittleEndian>() {
        out.push(v);
    }
    Some(out)
}

/// Write `samples` to `<cache_root>/MediaVisualCache/<key>.waveform` as LE f32.
/// Creates the cache subdirectory if needed.
pub fn save_waveform(cache_root: &Path, key: &str, samples: &[f32]) -> Result<()> {
    let dir = cache_root.join(CACHE_SUBDIR);
    std::fs::create_dir_all(&dir)?;
    let mut buf = Vec::with_capacity(samples.len() * 4);
    for &s in samples {
        buf.write_f32::<LittleEndian>(s)?;
    }
    std::fs::write(dir.join(format!("{key}.waveform")), &buf)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let samples = vec![0.0f32, 0.5, 1.0, -0.25, 0.123456];
        save_waveform(dir.path(), "abc", &samples).unwrap();
        let back = load_waveform(dir.path(), "abc").unwrap();
        assert_eq!(samples, back);
    }

    #[test]
    fn saved_file_is_exactly_len_times_4_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let samples = vec![1.0f32; 7];
        save_waveform(dir.path(), "k", &samples).unwrap();
        let path = waveform_path(dir.path(), "k");
        let meta = std::fs::metadata(path).unwrap();
        assert_eq!(meta.len(), 7 * 4);
    }

    #[test]
    fn load_missing_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_waveform(dir.path(), "nope").is_none());
    }

    #[test]
    fn load_empty_file_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join(CACHE_SUBDIR);
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("empty.waveform"), b"").unwrap();
        assert!(load_waveform(dir.path(), "empty").is_none());
    }

    #[test]
    fn load_non_multiple_of_4_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join(CACHE_SUBDIR);
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("bad.waveform"), [1u8, 2, 3]).unwrap(); // 3 bytes
        assert!(load_waveform(dir.path(), "bad").is_none());
    }

    #[test]
    fn little_endian_byte_layout_is_fixed() {
        let dir = tempfile::tempdir().unwrap();
        save_waveform(dir.path(), "le", &[1.0f32]).unwrap();
        let raw = std::fs::read(waveform_path(dir.path(), "le")).unwrap();
        // 1.0f32 LE == 00 00 80 3F
        assert_eq!(raw, vec![0x00, 0x00, 0x80, 0x3F]);
    }
}
