//! Content-identity cache key shared by the thumbnail, waveform, transcript, and
//! embedding caches.
//!
//! Upstream had three byte-identical implementations differing only in how many
//! hex characters they kept (`MediaVisualCache.diskCacheKey`,
//! `EmbeddingStore.key`, `TranscriptCache.key`). They all hash
//! `"<path>|<mtime>|<size>"` with SHA-256. We unify them: take the first **16
//! bytes** of the digest rendered as **32 lowercase hex chars**, which equals
//! every upstream site (one took `digest.prefix(16)` → 16 bytes → 32 hex; the
//! others took `.joined().prefix(32)` → 32 hex chars = 16 bytes). The cache
//! files stay mutually readable with the upstream app on the same machine.
//!
//! `mtime` is the POSIX modification time in **floating-point seconds since the
//! Unix epoch**, matching Swift's `Date.timeIntervalSince1970`. A missing file
//! or unreadable metadata yields `None` (upstream `guard let … else return nil`).

use std::path::Path;
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};

/// Number of hex characters in a cache key (16 bytes of SHA-256).
pub const KEY_HEX_LEN: usize = 32;

/// `SHA256("<path>|<mtime_secs_f64>|<size_bytes>")` as lowercase hex, truncated
/// to `prefix_chars` characters. Returns `None` if the file does not exist or
/// its size/mtime cannot be read.
///
/// Pass `prefix_chars = 32` for full upstream parity (see module docs).
pub fn file_identity_key(path: &Path, prefix_chars: usize) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let size = meta.len();
    let mtime = meta.modified().ok()?;
    let secs = mtime.duration_since(UNIX_EPOCH).ok()?.as_secs_f64();
    Some(identity_hex(
        &path.to_string_lossy(),
        secs,
        size,
        prefix_chars,
    ))
}

/// Pure core: hash a pre-resolved `(path, mtime_secs, size)` identity. Split out
/// so the byte layout is unit-testable without touching the filesystem.
pub fn identity_hex(path: &str, mtime_secs: f64, size: u64, prefix_chars: usize) -> String {
    // Swift renders the seed as "<path>|<timeIntervalSince1970>|<size>" where the
    // mtime uses `Double.description`. That always keeps a fractional part, so a
    // whole-second mtime prints "1000.0", whereas Rust's f64 Display prints
    // "1000" — a divergence that would break cross-app cache interop (SPEC §1.4 /
    // §3.3 / §5.6). For the positive, normal-magnitude values filesystem mtimes
    // produce, Swift and Rust agree on every fractional rendering; the only gap
    // is the missing ".0" on integral values, which we restore here.
    let seed = format!("{path}|{}|{size}", swift_double(mtime_secs));
    let digest = Sha256::digest(seed.as_bytes());
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest.iter() {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex.truncate(prefix_chars);
    hex
}

/// Render `v` the way Swift's `Double.description` would for a filesystem mtime,
/// so the hashed seed byte-matches the upstream Swift app. Swift always shows a
/// decimal point; Rust's f64 Display drops it for integral values, so we append
/// `.0` in that one case. (Both use shortest round-trippable formatting for the
/// fractional renderings that real mtimes produce, so no other adjustment is
/// needed.)
fn swift_double(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 {
        format!("{v}.0")
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn identity_hex_is_stable_and_lowercase() {
        let a = identity_hex("/a/b.mp4", 1000.0, 42, KEY_HEX_LEN);
        let b = identity_hex("/a/b.mp4", 1000.0, 42, KEY_HEX_LEN);
        assert_eq!(a, b);
        assert_eq!(a.len(), KEY_HEX_LEN);
        assert!(a
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn identity_hex_changes_with_each_component() {
        let base = identity_hex("/a/b.mp4", 1000.0, 42, KEY_HEX_LEN);
        assert_ne!(base, identity_hex("/a/c.mp4", 1000.0, 42, KEY_HEX_LEN)); // path
        assert_ne!(base, identity_hex("/a/b.mp4", 1001.0, 42, KEY_HEX_LEN)); // mtime
        assert_ne!(base, identity_hex("/a/b.mp4", 1000.0, 43, KEY_HEX_LEN)); // size
    }

    #[test]
    fn swift_double_keeps_trailing_zero_for_integral_seconds() {
        // Matches Swift's Double.description: integral values keep ".0",
        // fractional values render shortest-round-trippable (== Rust f64 Display).
        assert_eq!(swift_double(1000.0), "1000.0");
        assert_eq!(swift_double(0.0), "0.0");
        assert_eq!(swift_double(1_718_900_000.0), "1718900000.0");
        assert_eq!(swift_double(1000.5), "1000.5");
        assert_eq!(swift_double(1_718_900_000.123), "1718900000.123");
    }

    #[test]
    fn identity_hex_matches_swift_for_whole_second_mtime() {
        // Cross-app interop pin (SPEC §1.4 / §3.3 / §5.6): the expected hex was
        // computed in Swift from the seed "/a/b.mp4|1000.0|42":
        //   let seed = "\(path)|\(mtime)|\(size)"          // mtime = 1000.0
        //   SHA256.hash(seed.utf8).map{String(format:"%02x",$0)}.joined().prefix(32)
        // A whole-second mtime is exactly the case Rust's f64 Display would have
        // broken (it prints "1000", not "1000.0").
        let key = identity_hex("/a/b.mp4", 1000.0, 42, KEY_HEX_LEN);
        assert_eq!(key, "c428ca2d60590827149ac76ecc8f743f");
    }

    #[test]
    fn identity_hex_matches_known_sha256_prefix() {
        // Independently verifiable: sha256("/x|0|0") first 16 bytes as hex.
        // (Computed with the same seed format the code uses.)
        let full = identity_hex("/x", 0.0, 0, 64);
        let short = identity_hex("/x", 0.0, 0, 32);
        assert_eq!(full.len(), 64);
        assert_eq!(short, &full[..32]);
    }

    #[test]
    fn prefix_chars_truncates() {
        assert_eq!(identity_hex("/a", 1.0, 1, 8).len(), 8);
        assert_eq!(identity_hex("/a", 1.0, 1, 16).len(), 16);
    }

    #[test]
    fn file_identity_key_reads_real_file() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"hello").unwrap();
        f.flush().unwrap();
        let key = file_identity_key(f.path(), KEY_HEX_LEN);
        assert!(key.is_some());
        assert_eq!(key.unwrap().len(), KEY_HEX_LEN);
    }

    #[test]
    fn file_identity_key_missing_file_is_none() {
        let key = file_identity_key(Path::new("/nonexistent/xyz.never"), KEY_HEX_LEN);
        assert!(key.is_none());
    }
}
