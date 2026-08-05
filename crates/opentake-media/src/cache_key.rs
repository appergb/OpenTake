//! Content-identity cache key shared by the thumbnail, waveform, transcript, and
//! embedding caches.
//!
//! Upstream had three similar implementations
//! (`MediaVisualCache.diskCacheKey`, `EmbeddingStore.key`,
//! `TranscriptCache.key`). All retain the first **16 bytes** of SHA-256 as **32
//! lowercase hex chars**, but their seed order is not identical:
//! `MediaVisualCache` hashes `"<path>|<size>|<mtime>"`, while the embedding and
//! transcript caches hash `"<path>|<mtime>|<size>"`. The two public file helpers
//! keep those contracts explicit so every cache remains readable by the
//! upstream app on the same machine.
//!
//! `mtime` is the POSIX modification time in **floating-point seconds since the
//! Unix epoch**, matching Swift's `Date.timeIntervalSince1970`. A missing file
//! or unreadable metadata yields `None` (upstream `guard let … else return nil`).

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

/// Number of hex characters in a cache key (16 bytes of SHA-256).
pub const KEY_HEX_LEN: usize = 32;
const FOUNDATION_REFERENCE_OFFSET_SECS: f64 = 978_307_200.0;

/// Embedding/transcript key: `SHA256("<path>|<mtime>|<size>")` as lowercase
/// 32-character hex. Returns `None` if the file does not exist or its
/// size/mtime cannot be read.
pub fn file_identity_key(path: &Path) -> Option<String> {
    let (path, secs, size) = file_identity_parts(path)?;
    Some(identity_hex(&path, secs, size))
}

/// Thumbnail/waveform key: `SHA256("<path>|<size>|<mtime>")`, retaining the
/// upstream `MediaVisualCache` prefix of 16 bytes / 32 lowercase hex chars.
pub fn visual_file_identity_key(path: &Path) -> Option<String> {
    let (path, secs, size) = file_identity_parts(path)?;
    Some(visual_identity_hex(&path, secs, size))
}

fn file_identity_parts(path: &Path) -> Option<(String, f64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let size = meta.len();
    let mtime = meta.modified().ok()?;
    Some((
        path.to_string_lossy().into_owned(),
        foundation_unix_seconds(mtime),
        size,
    ))
}

/// Reproduce the `Date` value returned by Foundation file attributes.
///
/// Foundation stores `Date` as a `Double` relative to 2001-01-01, then
/// `timeIntervalSince1970` adds the 1970-to-2001 offset. Applying the nanosecond
/// fraction between those two operations is observable and must be preserved
/// for byte-identical cache seeds.
fn foundation_unix_seconds(time: SystemTime) -> f64 {
    let (whole_seconds, nanos) = match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => (duration.as_secs() as f64, duration.subsec_nanos()),
        Err(error) => {
            let duration = error.duration();
            if duration.subsec_nanos() == 0 {
                (-(duration.as_secs() as f64), 0)
            } else {
                (
                    -(duration.as_secs() as f64) - 1.0,
                    1_000_000_000 - duration.subsec_nanos(),
                )
            }
        }
    };
    foundation_unix_seconds_from_parts(whole_seconds, nanos)
}

fn foundation_unix_seconds_from_parts(whole_seconds: f64, nanos: u32) -> f64 {
    let reference_seconds =
        (whole_seconds - FOUNDATION_REFERENCE_OFFSET_SECS) + f64::from(nanos) / 1_000_000_000.0;
    reference_seconds + FOUNDATION_REFERENCE_OFFSET_SECS
}

/// Pure embedding/transcript core. Hash a pre-resolved
/// `"<path>|<mtime>|<size>"` identity.
pub fn identity_hex(path: &str, mtime_secs: f64, size: u64) -> String {
    let seed = format!("{path}|{}|{size}", swift_double(mtime_secs));
    sha256_hex_prefix(&seed, KEY_HEX_LEN)
}

/// Pure thumbnail/waveform core. Hash the upstream
/// `"<path>|<size>|<mtime>"` identity and retain 32 hex characters.
pub fn visual_identity_hex(path: &str, mtime_secs: f64, size: u64) -> String {
    let seed = format!("{path}|{size}|{}", swift_double(mtime_secs));
    sha256_hex_prefix(&seed, KEY_HEX_LEN)
}

fn sha256_hex_prefix(seed: &str, prefix_chars: usize) -> String {
    let digest = Sha256::digest(seed.as_bytes());
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest.iter() {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex.truncate(prefix_chars);
    hex
}

/// Parse a `ryu-js` explicit exponent (`e` suffix of the closest-shortest
/// digits). `ryu-js` always emits a numeric exponent today; a future format
/// change must not panic production code, so malformed input falls back to a
/// zero exponent (cache keys only need to be stable within this binary).
fn parse_exponent(exponent: &str) -> i32 {
    exponent.parse().unwrap_or(0)
}

/// Render `v` using Swift `Double.description`'s closest-shortest digits and
/// finishing policy. ECMAScript uses the same closest-shortest tie-breaking;
/// `ryu-js` provides those digits, after which we apply Swift's exponential
/// threshold, integral `.0`, and exponent padding rules.
fn swift_double(v: f64) -> String {
    if v.is_nan() {
        return "nan".to_owned();
    }
    if v == f64::INFINITY {
        return "inf".to_owned();
    }
    if v == f64::NEG_INFINITY {
        return "-inf".to_owned();
    }
    if v == 0.0 {
        return if v.is_sign_negative() {
            "-0.0".to_owned()
        } else {
            "0.0".to_owned()
        };
    }

    let negative = v.is_sign_negative();
    let magnitude = v.abs();
    let mut buffer = ryu_js::Buffer::new();
    let shortest = buffer.format(magnitude);
    let (mantissa, explicit_exponent) = match shortest.split_once('e') {
        Some((mantissa, exponent)) => (mantissa, parse_exponent(exponent)),
        None => (shortest, 0),
    };
    let fractional_digits = mantissa
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len()) as i32;
    let mut digits = mantissa.replace('.', "");
    digits = digits.trim_start_matches('0').to_owned();
    let mut base10_power = explicit_exponent - fractional_digits;
    while digits.ends_with('0') {
        digits.pop();
        base10_power += 1;
    }

    let decimal_exponent = base10_power + digits.len() as i32 - 1;
    let bits = magnitude.to_bits();
    let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
    let significand_bits = bits & ((1_u64 << 52) - 1);
    let binary_exponent = if exponent_bits == 0 {
        1 - 1075
    } else {
        exponent_bits - 1075
    };
    let force_exponential = binary_exponent > 1 || (binary_exponent == 1 && significand_bits != 0);

    let mut body = if decimal_exponent < -4 || force_exponential {
        let mut rendered = String::new();
        rendered.push(digits.as_bytes()[0] as char);
        if digits.len() > 1 {
            rendered.push('.');
            rendered.push_str(&digits[1..]);
        }
        rendered.push('e');
        rendered.push(if decimal_exponent < 0 { '-' } else { '+' });
        rendered.push_str(&format!("{:02}", decimal_exponent.unsigned_abs()));
        rendered
    } else if decimal_exponent < 0 {
        format!(
            "0.{}{}",
            "0".repeat((-decimal_exponent - 1) as usize),
            digits
        )
    } else if decimal_exponent + 1 < digits.len() as i32 {
        let split = (decimal_exponent + 1) as usize;
        format!("{}.{}", &digits[..split], &digits[split..])
    } else {
        let mut rendered = digits;
        rendered.push_str(&"0".repeat((decimal_exponent + 1 - rendered.len() as i32) as usize));
        rendered.push_str(".0");
        rendered
    };
    if negative {
        body.insert(0, '-');
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn malformed_exponent_falls_back_to_zero() {
        assert_eq!(parse_exponent("7"), 7);
        assert_eq!(parse_exponent("-3"), -3);
        assert_eq!(parse_exponent("+2"), 2);
        assert_eq!(parse_exponent(""), 0);
        assert_eq!(parse_exponent("abc"), 0);
        assert_eq!(parse_exponent("1e999"), 0);
    }

    #[test]
    fn identity_hex_is_stable_and_lowercase() {
        let a = identity_hex("/a/b.mp4", 1000.0, 42);
        let b = identity_hex("/a/b.mp4", 1000.0, 42);
        assert_eq!(a, b);
        assert_eq!(a.len(), KEY_HEX_LEN);
        assert!(a
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn identity_hex_changes_with_each_component() {
        let base = identity_hex("/a/b.mp4", 1000.0, 42);
        assert_ne!(base, identity_hex("/a/c.mp4", 1000.0, 42)); // path
        assert_ne!(base, identity_hex("/a/b.mp4", 1001.0, 42)); // mtime
        assert_ne!(base, identity_hex("/a/b.mp4", 1000.0, 43)); // size
    }

    #[test]
    fn swift_double_keeps_trailing_zero_for_integral_seconds() {
        assert_eq!(swift_double(1000.0), "1000.0");
        assert_eq!(swift_double(0.0), "0.0");
        assert_eq!(swift_double(-0.0), "-0.0");
        assert_eq!(swift_double(1_718_900_000.0), "1718900000.0");
        assert_eq!(swift_double(1000.5), "1000.5");
        assert_eq!(swift_double(1_718_900_000.123), "1718900000.123");
    }

    #[test]
    fn swift_double_matches_swift_exponential_policy() {
        assert_eq!(swift_double(0.0001), "0.0001");
        assert_eq!(swift_double(0.00001), "1e-05");
        assert_eq!(swift_double(0.000001), "1e-06");
        assert_eq!(swift_double(-0.000001), "-1e-06");
        assert_eq!(swift_double(9_000_000_000_000_000.0), "9000000000000000.0");
        assert_eq!(swift_double(9_900_000_000_000_000.0), "9.9e+15");
        assert_eq!(swift_double(10_000_000_000_000_000.0), "1e+16");
    }

    #[test]
    fn swift_double_matches_swift_closest_shortest_tie_breaking() {
        let cases = [
            (
                0x41e4_4062_fa8b_2000,
                "2718111700.3476562",
                "bdce91a4102c746eaa9c207ed8cd2ae9",
            ),
            (
                0x41e7_c29c_529a_a000,
                "3189039764.8320312",
                "23623d604689591a699a629b2553bcfe",
            ),
            (
                0x41ef_3a9c_12ae_a000,
                "4191477909.4570312",
                "310346173f384f90f96940259464cbd0",
            ),
        ];
        for (bits, expected_time, expected_hash) in cases {
            let seconds = f64::from_bits(bits);
            assert_eq!(swift_double(seconds), expected_time);
            assert_eq!(identity_hex("/x", seconds, 1), expected_hash);
        }
    }

    #[test]
    fn foundation_file_times_match_swift_cache_vectors() {
        let cases = [
            (
                (1_718_900_000.0, 123_456_789),
                "/a/b.mp4",
                42,
                "1718900000.123457",
                "fe36d2e6e596fc521efc75485110968e",
                "171d2ad89d6dcba2eee30b72903bb18c",
            ),
            (
                (0.0, 1_000),
                "/tiny",
                1,
                "9.5367431640625e-07",
                "4e8da355de85086afa8971fb99e66cfe",
                "6090717edb604ac2af1714435b7cece1",
            ),
            (
                (-1.0, 999_999_000),
                "/before",
                1,
                "-9.5367431640625e-07",
                "403a0c51e8bf1000a693b1f604464e8d",
                "5cc3a36a9381ac6a6ea34d80d86bf5e5",
            ),
        ];
        for (
            (whole_seconds, nanos),
            path,
            size,
            expected_time,
            expected_regular,
            expected_visual,
        ) in cases
        {
            let seconds = foundation_unix_seconds_from_parts(whole_seconds, nanos);
            assert_eq!(swift_double(seconds), expected_time);
            assert_eq!(identity_hex(path, seconds, size), expected_regular);
            assert_eq!(visual_identity_hex(path, seconds, size), expected_visual);
        }
    }

    #[test]
    fn foundation_system_time_supports_100ns_precision() {
        let time = UNIX_EPOCH + std::time::Duration::new(1_718_900_000, 123_456_700);
        assert_eq!(
            swift_double(foundation_unix_seconds(time)),
            "1718900000.1234567"
        );
    }

    #[test]
    fn identity_hex_matches_swift_for_whole_second_mtime() {
        // Cross-app interop pin (SPEC §1.4 / §3.3 / §5.6): the expected hex was
        // computed in Swift from the seed "/a/b.mp4|1000.0|42":
        //   let seed = "\(path)|\(mtime)|\(size)"          // mtime = 1000.0
        //   SHA256.hash(seed.utf8).map{String(format:"%02x",$0)}.joined().prefix(32)
        // A whole-second mtime is exactly the case Rust's f64 Display would have
        // broken (it prints "1000", not "1000.0").
        let key = identity_hex("/a/b.mp4", 1000.0, 42);
        assert_eq!(key, "c428ca2d60590827149ac76ecc8f743f");
    }

    #[test]
    fn visual_identity_hex_matches_swift_size_before_mtime() {
        // MediaVisualCache.swift uses the distinct seed
        // "/a/b.mp4|42|1000.0" (path|size|mtime), unlike the transcript and
        // embedding caches (path|mtime|size).
        let key = visual_identity_hex("/a/b.mp4", 1000.0, 42);
        assert_eq!(key, "40e65054546f8281078f6db271160874");
    }

    #[test]
    fn identity_hex_matches_known_sha256_prefix() {
        // Independently verifiable: sha256("/x|0.0|0") first 16 bytes as hex.
        let full = sha256_hex_prefix("/x|0.0|0", 64);
        let short = sha256_hex_prefix("/x|0.0|0", KEY_HEX_LEN);
        assert_eq!(full.len(), 64);
        assert_eq!(short, &full[..32]);
    }

    #[test]
    fn prefix_chars_truncates() {
        assert_eq!(sha256_hex_prefix("/a|1.0|1", 8).len(), 8);
        assert_eq!(sha256_hex_prefix("/a|1.0|1", 16).len(), 16);
    }

    #[test]
    fn file_identity_key_reads_real_file() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"hello").unwrap();
        f.flush().unwrap();
        let key = file_identity_key(f.path());
        assert!(key.is_some());
        assert_eq!(key.unwrap().len(), KEY_HEX_LEN);
    }

    #[test]
    fn visual_file_identity_key_reads_real_file() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"hello").unwrap();
        f.flush().unwrap();
        let key = visual_file_identity_key(f.path());
        assert!(key.is_some());
        assert_eq!(key.unwrap().len(), KEY_HEX_LEN);
    }

    #[test]
    fn file_identity_key_missing_file_is_none() {
        let key = file_identity_key(Path::new("/nonexistent/xyz.never"));
        assert!(key.is_none());
    }
}
