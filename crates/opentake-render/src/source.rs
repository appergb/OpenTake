//! Media-source contracts (SPEC §5.3). `opentake-render` DEFINES these traits;
//! `opentake-media` (or the caller) IMPLEMENTS them. This keeps the render crate
//! free of any decode/filesystem dependency: the plan builder only asks for a
//! source's intrinsic size / orientation / alpha flags, and the compositor only
//! asks for decoded pixels on demand.
//!
//! `media_ref` resolution (ref -> path) is the caller's job (upstream's
//! `MediaResolver`); render never touches the filesystem.

/// A decoded frame as packed RGBA8 (row-major, top-left origin), structurally
/// identical to `opentake_media::RgbaFrame`. Defined here so the render crate
/// does not depend on `opentake-media`; the integrating layer converts between
/// the two trivially (same field layout). SPEC §5.3.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DecodedFrame {
    pub width: u32,
    pub height: u32,
    /// `rgba.len() == width * height * 4`.
    pub rgba: Vec<u8>,
    /// Whether the RGB is already premultiplied by alpha.
    pub premultiplied: bool,
}

impl DecodedFrame {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>, premultiplied: bool) -> Self {
        debug_assert_eq!(rgba.len(), width as usize * height as usize * 4);
        DecodedFrame {
            width,
            height,
            rgba,
            premultiplied,
        }
    }
}

/// Source intrinsic size / orientation, queried once while building the plan
/// (pure metadata lookups; no decoding).
pub trait SourceMetrics {
    /// Video: decoded frame size; image: pixel size; Lottie: canvas size.
    /// Mirrors upstream `imageNativeSize` (L90) / `naturalSize`.
    fn natural_size(&self, media_ref: &str) -> Option<(u32, u32)>;

    /// Container display matrix -> row-major 6-tuple `[a, b, c, d, tx, ty]`
    /// (identity when absent). Mirrors upstream `preferredTransform` (L169).
    fn preferred_transform(&self, _media_ref: &str) -> [f64; 6] {
        [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]
    }

    /// Whether the source carries straight alpha that must be premultiplied
    /// before compositing (mirrors upstream `trackContainsAlpha`, L34).
    fn needs_premultiply(&self, media_ref: &str) -> bool {
        let _ = media_ref;
        false
    }

    /// Lottie internal frame count (used for the modulo wrap in SPEC §4.3).
    fn lottie_frame_count(&self, media_ref: &str) -> Option<i64> {
        let _ = media_ref;
        None
    }
}

/// Per-frame pixel supply, pulled lazily by the compositor while rendering.
pub trait FrameProvider {
    /// Pixels for `media_ref` at `source_frame` (SPEC §2.5). Preview: decode to
    /// the nearest keyframe and drop forward; export: decode sequentially.
    fn decoded_frame(&self, media_ref: &str, source_frame: i64) -> Option<DecodedFrame>;

    /// Image pixels (single frame; sRGB premultiplied-equivalent, mirrors
    /// upstream `createPixelBuffer`, L101).
    fn image_pixels(&self, media_ref: &str) -> Option<DecodedFrame>;

    /// Lottie internal-frame raster (premultiplied RGBA).
    fn lottie_frame(&self, media_ref: &str, frame: i64) -> Option<DecodedFrame>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoded_frame_holds_shape() {
        let f = DecodedFrame::new(2, 1, vec![1, 2, 3, 4, 5, 6, 7, 8], false);
        assert_eq!(f.width, 2);
        assert_eq!(f.height, 1);
        assert_eq!(f.rgba.len(), 8);
        assert!(!f.premultiplied);
    }

    /// A `SourceMetrics` using only the defaulted methods still compiles and
    /// returns the documented identity / false / None.
    struct MinimalMetrics;
    impl SourceMetrics for MinimalMetrics {
        fn natural_size(&self, _r: &str) -> Option<(u32, u32)> {
            Some((100, 50))
        }
    }

    #[test]
    fn source_metrics_defaults() {
        let m = MinimalMetrics;
        assert_eq!(m.natural_size("x"), Some((100, 50)));
        assert_eq!(m.preferred_transform("x"), [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        assert!(!m.needs_premultiply("x"));
        assert_eq!(m.lottie_frame_count("x"), None);
    }
}
