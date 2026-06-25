//! The bridge into `opentake-render`: a rendered motion clip exposed as an
//! ordinary clip source so the wgpu compositor treats it like any other texture
//! (docs/MOTION-GRAPHICS-PLUGIN.md §2 — "对合成器而言它只是又一个纹理来源,零特殊
//! 处理").
//!
//! `opentake-render` *defines* the [`SourceMetrics`] / [`FrameProvider`] traits
//! and the [`DecodedFrame`] type; this module *implements* them over a
//! [`RenderedClip`]. The compositor asks for the clip's natural size (the canvas
//! it was rendered at) and pulls decoded RGBA frames on demand.
//!
//! ## Decoder injection
//!
//! Decoding a frame file back to RGBA is deliberately *not* hard-wired to a PNG
//! library here. Frames may be produced by the [`StubRenderer`](crate::renderer)
//! (our tiny stored-block PNG) or, in production, by real headless Chromium
//! (standard PNG) or a future raw-RGBA fast path. So [`MotionClipSource`] takes a
//! `FrameDecoder` — a `Fn(&Path) -> Option<DecodedFrame>` — supplied by the
//! integrating layer (which already owns an image/codec stack). Tests inject the
//! stub's own decoder; the app injects `image`/ffmpeg. This keeps this crate's
//! default dependency surface free of a decoder while still being fully testable.

use std::path::Path;

use opentake_render::{DecodedFrame, FrameProvider, SourceMetrics};

use crate::source::RenderedClip;

/// A function that decodes a frame file into straight-or-premultiplied RGBA.
/// Returns `None` on a missing/corrupt file (the compositor treats that frame as
/// absent, same as a failed video decode).
pub type FrameDecoder<'a> = dyn Fn(&Path) -> Option<DecodedFrame> + 'a;

/// A [`RenderedClip`] adapted to the render crate's clip-source traits.
///
/// `media_ref` semantics: every method ignores the `media_ref` argument because
/// this adapter wraps exactly one clip. In the wider system a motion clip's ref
/// resolves (via the caller's resolver) to *this* source instance, mirroring how
/// image/video refs resolve to their decoders. The adapter is single-clip on
/// purpose — the compositor builds one per motion clip.
pub struct MotionClipSource<'a> {
    clip: RenderedClip,
    decode: Box<FrameDecoder<'a>>,
}

impl<'a> MotionClipSource<'a> {
    /// Wrap a clip with a frame decoder. The decoder maps a frame file path to
    /// decoded RGBA (see [`FrameDecoder`]).
    pub fn new(clip: RenderedClip, decode: impl Fn(&Path) -> Option<DecodedFrame> + 'a) -> Self {
        MotionClipSource {
            clip,
            decode: Box::new(decode),
        }
    }

    /// The wrapped clip.
    pub fn clip(&self) -> &RenderedClip {
        &self.clip
    }

    /// Decode the frame at a 0-based index, clamping past-the-end to the last
    /// frame (freeze-frame hold, consistent with [`RenderedClip::frame_path`]).
    pub fn frame(&self, frame: i64) -> Option<DecodedFrame> {
        let idx = if frame < 0 { 0usize } else { frame as usize };
        let path = self.clip.frame_path(idx)?;
        (self.decode)(path)
    }
}

impl SourceMetrics for MotionClipSource<'_> {
    /// The motion clip's natural size is the canvas it was rendered at.
    fn natural_size(&self, _media_ref: &str) -> Option<(u32, u32)> {
        Some((self.clip.width, self.clip.height))
    }

    /// Motion frames carry straight alpha when the clip is transparent, so the
    /// compositor must premultiply before blending (same contract as alpha video).
    fn needs_premultiply(&self, _media_ref: &str) -> bool {
        self.clip.transparent
    }
}

impl FrameProvider for MotionClipSource<'_> {
    /// A motion clip is a frame sequence: `source_frame` indexes directly into
    /// the rendered frames (the plan builder maps timeline frames → source frames
    /// upstream; for a 1:1 motion overlay these coincide).
    fn decoded_frame(&self, _media_ref: &str, source_frame: i64) -> Option<DecodedFrame> {
        self.frame(source_frame)
    }

    /// Not an image source — motion clips are always sequences, so the single-
    /// frame image path is unused. Returning the first frame keeps a caller that
    /// mistakenly treats it as an image from getting nothing.
    fn image_pixels(&self, _media_ref: &str) -> Option<DecodedFrame> {
        self.frame(0)
    }

    /// Not a Lottie source.
    fn lottie_frame(&self, _media_ref: &str, frame: i64) -> Option<DecodedFrame> {
        self.frame(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::MotionCache;
    use crate::renderer::{MotionRenderer, StubRenderer};
    use crate::source::{MotionRenderRequest, MotionSource};

    /// A decoder built on the `image` dev-dep, used to read the stub's PNGs back.
    fn image_decoder(path: &Path) -> Option<DecodedFrame> {
        let img = image::open(path).ok()?.to_rgba8();
        let (w, h) = img.dimensions();
        Some(DecodedFrame::new(w, h, img.into_raw(), false))
    }

    fn render_clip(transparent: bool) -> (RenderedClip, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let renderer = StubRenderer::new(MotionCache::new(tmp.path()));
        let req = MotionRenderRequest::new(MotionSource::code("<div>x</div>"), 30, 4, 6, 4)
            .with_transparent(transparent);
        let clip = renderer.render(&req).unwrap();
        (clip, tmp)
    }

    #[test]
    fn natural_size_is_render_canvas() {
        let (clip, _tmp) = render_clip(true);
        let src = MotionClipSource::new(clip, image_decoder);
        assert_eq!(src.natural_size("ref"), Some((6, 4)));
    }

    #[test]
    fn needs_premultiply_tracks_transparency() {
        let (t_clip, _t) = render_clip(true);
        let t_src = MotionClipSource::new(t_clip, image_decoder);
        assert!(t_src.needs_premultiply("ref"));

        let (o_clip, _o) = render_clip(false);
        let o_src = MotionClipSource::new(o_clip, image_decoder);
        assert!(!o_src.needs_premultiply("ref"));
    }

    #[test]
    fn decoded_frame_returns_rgba_of_right_shape() {
        let (clip, _tmp) = render_clip(true);
        let src = MotionClipSource::new(clip, image_decoder);
        let f = src.decoded_frame("ref", 0).expect("frame 0 decodes");
        assert_eq!(f.width, 6);
        assert_eq!(f.height, 4);
        assert_eq!(f.rgba.len(), 6 * 4 * 4);
    }

    #[test]
    fn frame_index_clamps_past_end() {
        let (clip, _tmp) = render_clip(true);
        let last_path = clip.frames.last().unwrap().clone();
        let src = MotionClipSource::new(clip, image_decoder);
        // Frame 999 clamps to the last frame -> still decodes.
        let f = src
            .decoded_frame("ref", 999)
            .expect("clamped frame decodes");
        let last = image::open(&last_path).unwrap().to_rgba8();
        assert_eq!(f.rgba, last.into_raw());
    }

    #[test]
    fn missing_decoder_result_is_none() {
        let (clip, _tmp) = render_clip(true);
        // A decoder that always fails surfaces None (compositor treats as absent).
        let src = MotionClipSource::new(clip, |_p: &Path| None);
        assert!(src.decoded_frame("ref", 0).is_none());
    }

    #[test]
    fn negative_source_frame_maps_to_first() {
        let (clip, _tmp) = render_clip(false);
        let src = MotionClipSource::new(clip, image_decoder);
        assert!(src.decoded_frame("ref", -5).is_some());
    }
}
