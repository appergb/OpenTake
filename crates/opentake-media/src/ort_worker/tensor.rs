//! Tensor helpers for the ORT worker: image ↔ NCHW/NHWC conversion and mean/std
//! normalization. Pure (`ndarray` only), unit-tested, and reused by the SigLIP
//! preprocessing and future advanced models.

use ndarray::{Array3, Array4};

use crate::frame::RgbaFrame;

/// Convert an `RgbaFrame` to an `HWC` f32 array in `[0,1]` (alpha dropped).
pub fn frame_to_hwc(frame: &RgbaFrame) -> Array3<f32> {
    let h = frame.height as usize;
    let w = frame.width as usize;
    let mut arr = Array3::<f32>::zeros((h, w, 3));
    for y in 0..h {
        for x in 0..w {
            let base = (y * w + x) * 4;
            if base + 2 < frame.rgba.len() {
                arr[[y, x, 0]] = frame.rgba[base] as f32 / 255.0;
                arr[[y, x, 1]] = frame.rgba[base + 1] as f32 / 255.0;
                arr[[y, x, 2]] = frame.rgba[base + 2] as f32 / 255.0;
            }
        }
    }
    arr
}

/// Normalize an `HWC` array with per-channel `mean`/`std` and convert to a
/// batched `NCHW` (1,3,H,W) tensor. The canonical ONNX image input layout.
pub fn hwc_to_nchw_normalized(hwc: &Array3<f32>, mean: [f32; 3], std: [f32; 3]) -> Array4<f32> {
    let (h, w, _c) = hwc.dim();
    let mut out = Array4::<f32>::zeros((1, 3, h, w));
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                out[[0, c, y, x]] = (hwc[[y, x, c]] - mean[c]) / std[c];
            }
        }
    }
    out
}

/// Mean-pool a flat embedding block `(n, dim)` over the `n` axis into one
/// `dim`-vector. Handy for models that emit token-level outputs.
pub fn mean_pool(block: &[f32], n: usize, dim: usize) -> Vec<f32> {
    if n == 0 || dim == 0 {
        return vec![0.0; dim];
    }
    let mut out = vec![0.0f32; dim];
    for i in 0..n {
        for d in 0..dim {
            out[d] += block[i * dim + d];
        }
    }
    for v in out.iter_mut() {
        *v /= n as f32;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_to_hwc_drops_alpha_and_scales() {
        let f = RgbaFrame::new(2, 1, vec![255, 128, 0, 50, 0, 0, 0, 255]);
        let hwc = frame_to_hwc(&f);
        assert_eq!(hwc.dim(), (1, 2, 3));
        assert!((hwc[[0, 0, 0]] - 1.0).abs() < 1e-6);
        assert!((hwc[[0, 0, 1]] - 128.0 / 255.0).abs() < 1e-6);
        assert!((hwc[[0, 0, 2]] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn hwc_to_nchw_normalizes_and_reshapes() {
        let f = RgbaFrame::new(1, 1, vec![255, 255, 255, 255]);
        let hwc = frame_to_hwc(&f);
        let nchw = hwc_to_nchw_normalized(&hwc, [0.5, 0.5, 0.5], [0.5, 0.5, 0.5]);
        assert_eq!(nchw.dim(), (1, 3, 1, 1));
        // (1.0 - 0.5)/0.5 = 1.0
        for v in nchw.iter() {
            assert!((*v - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn mean_pool_averages_rows() {
        // 2 tokens, dim 3: [1,2,3] and [3,4,5] → [2,3,4].
        let block = vec![1.0, 2.0, 3.0, 3.0, 4.0, 5.0];
        let pooled = mean_pool(&block, 2, 3);
        assert_eq!(pooled, vec![2.0, 3.0, 4.0]);
    }

    #[test]
    fn mean_pool_empty_is_zeros() {
        assert_eq!(mean_pool(&[], 0, 3), vec![0.0, 0.0, 0.0]);
    }
}
