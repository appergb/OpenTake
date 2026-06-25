//! `RgbaFrame` — the plain pixel value type exchanged across the media/render
//! boundary. No wgpu / ffmpeg types leak through it (SPEC §8.2). Tightly packed
//! RGBA8, row-major, top-left origin.

/// A decoded frame as packed RGBA8 (`rgba.len() == width * height * 4`).
#[derive(Clone, PartialEq, Eq)]
pub struct RgbaFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl RgbaFrame {
    /// Construct, asserting the buffer length matches the dimensions.
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        debug_assert_eq!(rgba.len(), width as usize * height as usize * 4);
        RgbaFrame {
            width,
            height,
            rgba,
        }
    }

    /// A solid opaque-black frame (used as the SigLIP squash-resize backdrop and
    /// for tests).
    pub fn black(width: u32, height: u32) -> Self {
        let mut rgba = vec![0u8; width as usize * height as usize * 4];
        for px in rgba.chunks_exact_mut(4) {
            px[3] = 255;
        }
        RgbaFrame {
            width,
            height,
            rgba,
        }
    }

    /// Number of pixels.
    pub fn pixel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }
}

impl std::fmt::Debug for RgbaFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Don't dump the pixel buffer; show shape only.
        f.debug_struct("RgbaFrame")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("bytes", &self.rgba.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_is_opaque_and_correctly_sized() {
        let f = RgbaFrame::black(3, 2);
        assert_eq!(f.width, 3);
        assert_eq!(f.height, 2);
        assert_eq!(f.rgba.len(), 3 * 2 * 4);
        assert_eq!(f.pixel_count(), 6);
        // every pixel is (0,0,0,255)
        for px in f.rgba.chunks_exact(4) {
            assert_eq!(px, &[0, 0, 0, 255]);
        }
    }

    #[test]
    fn debug_does_not_include_pixels() {
        let f = RgbaFrame::black(2, 2);
        let s = format!("{f:?}");
        assert!(s.contains("width"));
        assert!(s.contains("bytes"));
    }
}
