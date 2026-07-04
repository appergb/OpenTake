//! Text rasterization interface (SPEC §4.2). Upstream renders text via
//! `CATextLayer` + the CoreAnimationTool; OpenTake rasterizes each text clip to a
//! premultiplied-RGBA texture that composites like any other layer.
//!
//! Full glyph layout (cosmic-text) + raster (tiny-skia / Vello), with shadow /
//! border / background / alignment / word-wrap, lands in the advanced/motion
//! phase. This module defines the trait boundary now and ships a null
//! implementation that returns `None` (never `todo!()` / `unimplemented!()`), so
//! the compositor can already route text clips and tests never trip an
//! unimplemented panic.

use opentake_domain::TextStyle;

use crate::source::DecodedFrame;

/// Inputs needed to rasterize one text clip at a given canvas size.
#[derive(Clone, PartialEq, Debug)]
pub struct TextRasterRequest<'a> {
    pub clip_id: &'a str,
    pub content: &'a str,
    pub style: &'a TextStyle,
    /// Normalized text box on the canvas (top-left x/y, width/height in 0..1).
    pub box_norm: (f64, f64, f64, f64),
    /// Canvas pixel size.
    pub canvas: (u32, u32),
}

/// Rasterizes a text clip to a premultiplied-RGBA [`DecodedFrame`].
pub trait TextRasterizer {
    /// Render the request, or `None` if text rendering is unavailable in this
    /// build (the null backend) or the request is degenerate.
    fn rasterize(&self, request: &TextRasterRequest<'_>) -> Option<DecodedFrame>;
}

/// Placeholder backend: produces no texture. Lets the pipeline compile, route
/// text clips, and run end-to-end without a glyph engine. Replaced by the
/// cosmic-text backend in a later phase.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullTextRasterizer;

impl TextRasterizer for NullTextRasterizer {
    fn rasterize(&self, _request: &TextRasterRequest<'_>) -> Option<DecodedFrame> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_rasterizer_returns_none_without_panicking() {
        let style = TextStyle::default();
        let req = TextRasterRequest {
            clip_id: "t0",
            content: "hello",
            style: &style,
            box_norm: (0.1, 0.1, 0.8, 0.2),
            canvas: (1920, 1080),
        };
        assert!(NullTextRasterizer.rasterize(&req).is_none());
    }
}
