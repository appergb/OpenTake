//! Content-hash cache for rendered motion clips (docs/MOTION-GRAPHICS-PLUGIN.md
//! §2): the cache key is a SHA-256 over everything that affects the pixels —
//! the source (code or template id + params), fps, width, height, and the
//! transparency flag. Same inputs ⇒ same key ⇒ reuse the already-rendered frames;
//! change the source or any param and the key changes, so the next render misses
//! and recomputes. This is the standard content-addressed-cache pattern: the key
//! is path-independent and self-invalidating.
//!
//! The keying ([`content_hash`]) is **pure** and unit-tested with no filesystem.
//! [`MotionCache`] is the thin directory wrapper that maps a key to a folder and
//! reports hit/miss; the renderer writes frames into that folder.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::MotionResult;
use crate::source::{MotionRenderRequest, MotionSource, ParamValue};

/// Compute the content hash (lowercase hex SHA-256) for a render request.
///
/// We feed a canonical, unambiguous byte stream into the hash — each field
/// length-prefixed or delimited so that `("a","bc")` and `("ab","c")` can't
/// collide. `BTreeMap` param ordering makes the template arm deterministic.
pub fn content_hash(req: &MotionRenderRequest) -> String {
    let mut hasher = Sha256::new();

    // Version the key format so a future change to what we hash invalidates old
    // entries instead of silently colliding.
    hasher.update(b"opentake-motion/v1\n");

    // Numeric/flags first (fixed-width, no ambiguity).
    hasher.update(b"fps=");
    hasher.update(req.fps.to_le_bytes());
    hasher.update(b";frames=");
    hasher.update(req.duration_frames.to_le_bytes());
    hasher.update(b";w=");
    hasher.update(req.width.to_le_bytes());
    hasher.update(b";h=");
    hasher.update(req.height.to_le_bytes());
    hasher.update(b";transparent=");
    hasher.update([req.transparent as u8]);
    hasher.update(b"\n");

    // The source.
    match &req.source {
        MotionSource::Code { html_css_js } => {
            hasher.update(b"source=code;len=");
            hasher.update((html_css_js.len() as u64).to_le_bytes());
            hasher.update(b";body=");
            hasher.update(html_css_js.as_bytes());
        }
        MotionSource::Template { id, params } => {
            hasher.update(b"source=template;id_len=");
            hasher.update((id.len() as u64).to_le_bytes());
            hasher.update(b";id=");
            hasher.update(id.as_bytes());
            hasher.update(b";params=");
            // BTreeMap iterates in sorted key order ⇒ deterministic.
            for (name, value) in params {
                hasher.update((name.len() as u64).to_le_bytes());
                hasher.update(name.as_bytes());
                hasher.update(b"=");
                hash_param_value(&mut hasher, value);
                hasher.update(b";");
            }
        }
    }

    hex::encode(hasher.finalize())
}

/// Fold one param value into the hash with a type tag so a string `"1"` and a
/// number `1` hash differently.
fn hash_param_value(hasher: &mut Sha256, value: &ParamValue) {
    match value {
        ParamValue::String(s) => {
            hasher.update(b"s:");
            hasher.update((s.len() as u64).to_le_bytes());
            hasher.update(s.as_bytes());
        }
        ParamValue::Number(n) => {
            hasher.update(b"n:");
            // Canonical bit pattern; normalize -0.0 to 0.0 so they don't diverge.
            let bits = if *n == 0.0 { 0.0f64 } else { *n }.to_bits();
            hasher.update(bits.to_le_bytes());
        }
        ParamValue::Bool(b) => {
            hasher.update(b"b:");
            hasher.update([*b as u8]);
        }
        ParamValue::Color(c) => {
            hasher.update(b"c:");
            // Hash colors case-insensitively (#ABC == #abc on the wire).
            let lower = c.to_ascii_lowercase();
            hasher.update((lower.len() as u64).to_le_bytes());
            hasher.update(lower.as_bytes());
        }
    }
}

/// A content-addressed frame cache rooted at a directory. Each render key maps to
/// `root/<hash>/`; the renderer fills that folder with frame files and the cache
/// reports whether it already exists & looks complete.
#[derive(Clone, Debug)]
pub struct MotionCache {
    root: PathBuf,
}

impl MotionCache {
    /// Create a cache rooted at `root` (not created on disk until
    /// [`MotionCache::ensure_dir`] / a render writes into it).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        MotionCache { root: root.into() }
    }

    /// The cache root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The directory a given request's frames live in (`root/<hash>`).
    pub fn dir_for(&self, req: &MotionRenderRequest) -> PathBuf {
        self.root.join(content_hash(req))
    }

    /// The directory for an explicit hash key.
    pub fn dir_for_hash(&self, hash: &str) -> PathBuf {
        self.root.join(hash)
    }

    /// Whether a complete render for this request is already cached. "Complete"
    /// means the directory exists and holds exactly `duration_frames` frame
    /// files — a partial render (crash mid-way) is treated as a miss so it gets
    /// recomputed rather than served truncated.
    pub fn is_cached(&self, req: &MotionRenderRequest) -> bool {
        let dir = self.dir_for(req);
        count_frame_files(&dir) == Some(req.duration_frames as usize)
    }

    /// Create the cache directory for a request, returning its path.
    pub fn ensure_dir(&self, req: &MotionRenderRequest) -> MotionResult<PathBuf> {
        let dir = self.dir_for(req);
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// The expected per-frame file path inside a render dir: zero-padded so
    /// lexical order == playback order (`frame_00000.png`).
    pub fn frame_file(dir: &Path, frame_index: usize) -> PathBuf {
        dir.join(format!("frame_{frame_index:05}.png"))
    }
}

/// Count `frame_*.png` files in a directory, or `None` if it doesn't exist.
fn count_frame_files(dir: &Path) -> Option<usize> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut n = 0usize;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("frame_") && name.ends_with(".png") {
            n += 1;
        }
    }
    Some(n)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn code_req(body: &str) -> MotionRenderRequest {
        MotionRenderRequest::new(MotionSource::code(body), 30, 60, 1920, 1080)
    }

    #[test]
    fn hash_is_64_hex_chars() {
        let h = content_hash(&code_req("<div/>"));
        assert_eq!(h.len(), 64);
        assert!(h.bytes().all(|b| b.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_is_stable_for_identical_requests() {
        assert_eq!(
            content_hash(&code_req("<a/>")),
            content_hash(&code_req("<a/>"))
        );
    }

    #[test]
    fn hash_changes_with_source_body() {
        assert_ne!(
            content_hash(&code_req("<a/>")),
            content_hash(&code_req("<b/>"))
        );
    }

    #[test]
    fn hash_changes_with_size_fps_and_transparency() {
        let base = code_req("<x/>");
        let mut bigger = base.clone();
        bigger.width = 1280;
        assert_ne!(content_hash(&base), content_hash(&bigger));

        let mut faster = base.clone();
        faster.fps = 60;
        assert_ne!(content_hash(&base), content_hash(&faster));

        let opaque = base.clone().with_transparent(false);
        assert_ne!(content_hash(&base), content_hash(&opaque));

        let mut longer = base.clone();
        longer.duration_frames = 120;
        assert_ne!(content_hash(&base), content_hash(&longer));
    }

    #[test]
    fn template_param_order_does_not_affect_hash() {
        // BTreeMap canonicalizes order, so two insert orders hash identically.
        let mut a = BTreeMap::new();
        a.insert("b".to_string(), ParamValue::Number(2.0));
        a.insert("a".to_string(), ParamValue::String("x".into()));
        let mut b = BTreeMap::new();
        b.insert("a".to_string(), ParamValue::String("x".into()));
        b.insert("b".to_string(), ParamValue::Number(2.0));

        let ra = MotionRenderRequest::new(
            MotionSource::Template {
                id: "t".into(),
                params: a,
            },
            30,
            60,
            100,
            100,
        );
        let rb = MotionRenderRequest::new(
            MotionSource::Template {
                id: "t".into(),
                params: b,
            },
            30,
            60,
            100,
            100,
        );
        assert_eq!(content_hash(&ra), content_hash(&rb));
    }

    #[test]
    fn template_param_value_type_changes_hash() {
        let mut as_str = BTreeMap::new();
        as_str.insert("v".to_string(), ParamValue::String("1".into()));
        let mut as_num = BTreeMap::new();
        as_num.insert("v".to_string(), ParamValue::Number(1.0));

        let r1 = MotionRenderRequest::new(
            MotionSource::Template {
                id: "t".into(),
                params: as_str,
            },
            30,
            60,
            100,
            100,
        );
        let r2 = MotionRenderRequest::new(
            MotionSource::Template {
                id: "t".into(),
                params: as_num,
            },
            30,
            60,
            100,
            100,
        );
        assert_ne!(content_hash(&r1), content_hash(&r2));
    }

    #[test]
    fn code_and_template_with_same_string_do_not_collide() {
        let code = MotionRenderRequest::new(MotionSource::code("t"), 30, 60, 100, 100);
        let tmpl = MotionRenderRequest::new(MotionSource::template("t"), 30, 60, 100, 100);
        assert_ne!(content_hash(&code), content_hash(&tmpl));
    }

    #[test]
    fn dir_for_is_root_join_hash() {
        let cache = MotionCache::new("/cache/motion");
        let req = code_req("<x/>");
        let expected = PathBuf::from("/cache/motion").join(content_hash(&req));
        assert_eq!(cache.dir_for(&req), expected);
    }

    #[test]
    fn frame_file_is_zero_padded() {
        let p = MotionCache::frame_file(Path::new("/d"), 7);
        assert_eq!(p, PathBuf::from("/d/frame_00007.png"));
    }

    #[test]
    fn is_cached_false_for_missing_dir() {
        let cache = MotionCache::new("/definitely/not/here");
        assert!(!cache.is_cached(&code_req("<x/>")));
    }

    #[test]
    fn is_cached_true_only_when_frame_count_matches() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = MotionCache::new(tmp.path());
        let req = MotionRenderRequest::new(MotionSource::code("<x/>"), 30, 3, 64, 64);
        let dir = cache.ensure_dir(&req).unwrap();

        // No frames yet -> miss.
        assert!(!cache.is_cached(&req));

        // Two of three frames -> still a miss (partial render).
        std::fs::write(MotionCache::frame_file(&dir, 0), b"x").unwrap();
        std::fs::write(MotionCache::frame_file(&dir, 1), b"x").unwrap();
        assert!(!cache.is_cached(&req));

        // All three -> hit.
        std::fs::write(MotionCache::frame_file(&dir, 2), b"x").unwrap();
        assert!(cache.is_cached(&req));
    }
}
