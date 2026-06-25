//! Visual embedder trait + spec + SigLIP image preprocessing. Port of
//! `Search/Models/VisualEmbedder.swift`.
//!
//! Preprocessing ([`preprocess_image`]) is pure and unit-tested: squash-resize
//! to a square (no aspect crop), composite alpha over black, normalize with the
//! model's mean/std into an `NCHW` f32 tensor. The real ONNX backend lives in
//! `ort_embedder` (feature `ort-backend`); tests use [`MockEmbedder`].

use image::{imageops::FilterType, RgbaImage};
use ndarray::Array4;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::frame::RgbaFrame;

/// Model spec, port of `VisualEmbedder.Spec`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbedderSpec {
    pub model: String,
    pub version: i32,
    pub embedding_dim: usize,
    pub image_size: u32,
    pub context_length: usize,
    /// Whether the exported model already L2-normalizes its output (SPEC §0.8).
    #[serde(default)]
    pub normalized: bool,
}

/// SigLIP's default preprocessing mean (per RGB channel).
pub const SIGLIP_MEAN: [f32; 3] = [0.5, 0.5, 0.5];
/// SigLIP's default preprocessing std (per RGB channel).
pub const SIGLIP_STD: [f32; 3] = [0.5, 0.5, 0.5];

/// Dual-encoder embedder. Implementations return a `embedding_dim`-length vector.
pub trait Embedder: Send + Sync {
    fn spec(&self) -> &EmbedderSpec;
    fn encode_image(&self, frame: &RgbaFrame) -> Result<Vec<f32>>;
    fn encode_text(&self, text: &str) -> Result<Vec<f32>>;
}

/// L2-normalize a vector in place (used when `spec.normalized == false` is
/// overridden, or by backends that need it). No-op for a zero vector.
pub fn l2_normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Squash-resize `frame` to `size × size` (ignoring aspect ratio), compositing
/// any alpha over opaque black, then normalize to an `NCHW` (1,3,size,size) f32
/// tensor using `mean`/`std`. Verbatim intent of `VisualEmbedder.pixelBuffer`
/// (`:63-87`): black backdrop + squash-resize + sRGB bytes → model tensor.
pub fn preprocess_image(
    frame: &RgbaFrame,
    size: u32,
    mean: [f32; 3],
    std: [f32; 3],
) -> Array4<f32> {
    // 1) Composite over black (drop alpha) into an RgbaImage we can resize.
    let mut src = RgbaImage::new(frame.width.max(1), frame.height.max(1));
    if frame.width > 0 && frame.height > 0 && !frame.rgba.is_empty() {
        for (i, px) in src.pixels_mut().enumerate() {
            let base = i * 4;
            if base + 3 < frame.rgba.len() {
                let a = frame.rgba[base + 3] as f32 / 255.0;
                let r = (frame.rgba[base] as f32 * a).round() as u8;
                let g = (frame.rgba[base + 1] as f32 * a).round() as u8;
                let b = (frame.rgba[base + 2] as f32 * a).round() as u8;
                *px = image::Rgba([r, g, b, 255]);
            }
        }
    }

    // 2) Squash-resize to an exact square (no aspect preservation).
    let resized = image::imageops::resize(&src, size, size, FilterType::Triangle);

    // 3) Normalize into NCHW f32.
    let mut tensor = Array4::<f32>::zeros((1, 3, size as usize, size as usize));
    for y in 0..size as usize {
        for x in 0..size as usize {
            let px = resized.get_pixel(x as u32, y as u32);
            for c in 0..3 {
                let v = px[c] as f32 / 255.0;
                tensor[[0, c, y, x]] = (v - mean[c]) / std[c];
            }
        }
    }
    tensor
}

#[cfg(test)]
pub(crate) mod test_support {
    //! Deterministic mock embedder for offline tests. The "embedding" is a tiny
    //! deterministic projection of the input so similarity ordering is testable
    //! without any model weights.
    use super::*;

    pub struct MockEmbedder {
        pub spec: EmbedderSpec,
    }

    impl MockEmbedder {
        /// A small-dim mock (dim 4) for fast tests.
        pub fn small() -> Self {
            MockEmbedder {
                spec: EmbedderSpec {
                    model: "mock".into(),
                    version: 1,
                    embedding_dim: 4,
                    image_size: 8,
                    context_length: 8,
                    normalized: true,
                },
            }
        }
    }

    impl Embedder for MockEmbedder {
        fn spec(&self) -> &EmbedderSpec {
            &self.spec
        }

        fn encode_image(&self, frame: &RgbaFrame) -> Result<Vec<f32>> {
            // Average channel intensities → 4-vec [r,g,b,brightness], normalized.
            let mut acc = [0.0f64; 4];
            let mut n = 0.0f64;
            for px in frame.rgba.chunks_exact(4) {
                acc[0] += px[0] as f64;
                acc[1] += px[1] as f64;
                acc[2] += px[2] as f64;
                acc[3] += (px[0] as f64 + px[1] as f64 + px[2] as f64) / 3.0;
                n += 1.0;
            }
            let n = n.max(1.0);
            let mut v: Vec<f32> = acc.iter().map(|x| (x / n / 255.0) as f32).collect();
            l2_normalize(&mut v);
            Ok(v)
        }

        fn encode_text(&self, text: &str) -> Result<Vec<f32>> {
            // Deterministic hash → 4-vec, normalized.
            let mut v = [0.0f32; 4];
            for (i, b) in text.bytes().enumerate() {
                v[i % 4] += b as f32;
            }
            let mut v = v.to_vec();
            l2_normalize(&mut v);
            Ok(v)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preprocess_outputs_nchw_shape() {
        let frame = RgbaFrame::black(10, 20);
        let t = preprocess_image(&frame, 16, SIGLIP_MEAN, SIGLIP_STD);
        assert_eq!(t.shape(), &[1, 3, 16, 16]);
    }

    #[test]
    fn preprocess_black_maps_to_minus_one_with_half_meanstd() {
        // black (0) → (0 - 0.5)/0.5 = -1.0 on every channel.
        let frame = RgbaFrame::black(4, 4);
        let t = preprocess_image(&frame, 4, SIGLIP_MEAN, SIGLIP_STD);
        for v in t.iter() {
            assert!((*v + 1.0).abs() < 1e-6, "expected -1.0, got {v}");
        }
    }

    #[test]
    fn preprocess_white_maps_to_plus_one() {
        let frame = RgbaFrame::new(2, 2, vec![255; 2 * 2 * 4]);
        let t = preprocess_image(&frame, 2, SIGLIP_MEAN, SIGLIP_STD);
        for v in t.iter() {
            assert!((*v - 1.0).abs() < 1e-6, "expected 1.0, got {v}");
        }
    }

    #[test]
    fn preprocess_squashes_non_square_to_square() {
        // A 100x10 frame must still produce a square tensor (no crop).
        let frame = RgbaFrame::black(100, 10);
        let t = preprocess_image(&frame, 32, SIGLIP_MEAN, SIGLIP_STD);
        assert_eq!(t.shape(), &[1, 3, 32, 32]);
    }

    #[test]
    fn preprocess_composites_alpha_over_black() {
        // Half-transparent red (255,0,0,128) over black → premultiplied ~128.
        let frame = RgbaFrame::new(1, 1, vec![255, 0, 0, 128]);
        let t = preprocess_image(&frame, 1, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        // R channel ≈ 128/255 ≈ 0.502; G,B ≈ 0.
        let r = t[[0, 0, 0, 0]];
        assert!((r - 0.502).abs() < 0.01, "r={r}");
        assert!(t[[0, 1, 0, 0]].abs() < 1e-6);
        assert!(t[[0, 2, 0, 0]].abs() < 1e-6);
    }

    #[test]
    fn l2_normalize_unit_length() {
        let mut v = vec![3.0f32, 4.0];
        l2_normalize(&mut v);
        let norm = (v[0] * v[0] + v[1] * v[1]).sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn l2_normalize_zero_is_noop() {
        let mut v = vec![0.0f32, 0.0];
        l2_normalize(&mut v);
        assert_eq!(v, vec![0.0, 0.0]);
    }

    #[test]
    fn spec_serde_camel_case_and_normalized_default() {
        let json =
            r#"{"model":"m","version":1,"embeddingDim":768,"imageSize":256,"contextLength":64}"#;
        let s: EmbedderSpec = serde_json::from_str(json).unwrap();
        assert_eq!(s.embedding_dim, 768);
        assert!(!s.normalized); // defaulted
        let round = serde_json::to_string(&s).unwrap();
        assert!(round.contains("embeddingDim"));
        assert!(round.contains("imageSize"));
        assert!(round.contains("contextLength"));
    }
}
