//! Regression test for #125 — the preview decodes source frames at a downscaled
//! `max_size`, so a layer's GPU texture is SMALLER than its source natural size.
//!
//! The shader scales its `[0,1]` quad by the SOURCE natural size (the value the
//! affine was built with), NOT by the decoded texture's resolution. If the
//! compositor feeds the texture size into the `nat` uniform instead, a full-canvas
//! clip renders shrunk into the bottom-left corner (and jitters as the per-frame
//! decoded size varies). This test composites a 4×4 texture for a 16×16 source on
//! a 16×16 canvas and asserts the layer still fills the whole frame.
//!
//! HARD CONSTRAINT: skips gracefully (eprintln + early return) when no GPU adapter
//! is available — never fails on GPU absence.

use std::rc::Rc;

use opentake_domain::{Clip, ClipType, Point, Timeline, Track, Transform};
use opentake_render::gpu::texture::upload_rgba;
use opentake_render::source::DecodedFrame;
use opentake_render::wgpu;
use opentake_render::{
    build_render_plan, Compositor, GpuTexture, RenderDevice, RenderSize, SourceMetrics,
    TextureResolver, TextureSource,
};

const RS: RenderSize = RenderSize {
    width: 16,
    height: 16,
};

/// Source display size is 16×16 (== canvas); the decoded texture will be 4×4.
struct Metrics;
impl SourceMetrics for Metrics {
    fn natural_size(&self, _r: &str) -> Option<(u32, u32)> {
        Some((16, 16))
    }
}

/// Resolves to a 4×4 solid texture — a quarter of the source/canvas size per
/// axis, exactly the downscale the preview applies (decode `max_size` < source).
struct SmallTexResolver<'d> {
    device: &'d wgpu::Device,
    queue: &'d wgpu::Queue,
    rgba: [u8; 4],
}
impl TextureResolver for SmallTexResolver<'_> {
    fn resolve(&mut self, _s: &TextureSource, _f: i64) -> Option<Rc<GpuTexture>> {
        let mut buf = vec![0u8; 4 * 4 * 4];
        for px in buf.chunks_exact_mut(4) {
            px.copy_from_slice(&self.rgba);
        }
        let frame = DecodedFrame::new(4, 4, buf, true); // premultiplied solid
        Some(Rc::new(upload_rgba(
            self.device,
            self.queue,
            &frame,
            false,
            Some("small"),
        )))
    }
}

#[test]
fn downscaled_texture_full_canvas_still_fills_whole_frame() {
    let dev = match RenderDevice::try_new() {
        Ok(d) => d,
        Err(e) => {
            eprintln!(
                "[skip] downscaled_texture_full_canvas_still_fills_whole_frame: no GPU ({e})"
            );
            return;
        }
    };

    let mut tl = Timeline::new();
    tl.fps = 30;
    tl.width = 16;
    tl.height = 16;
    let mut clip = Clip::new("c0", "asset", 0, 10);
    clip.transform = Transform::from_top_left(Point { x: 0.0, y: 0.0 }, 1.0, 1.0);
    let mut track = Track::new("t0", ClipType::Video);
    track.clips.push(clip);
    tl.tracks.push(track);

    let plan = build_render_plan(&tl, RS, &Metrics);
    let fp = plan.frame(&tl, 0);
    assert_eq!(fp.draws.len(), 1);
    // The draw must carry the SOURCE natural size (16×16), independent of the 4×4
    // decoded texture — this is the contract the fix restores.
    assert_eq!(fp.draws[0].nat_size, (16.0, 16.0));

    let compositor = Compositor::new(&dev.device);
    let mut resolver = SmallTexResolver {
        device: &dev.device,
        queue: &dev.queue,
        rgba: [40, 200, 80, 255],
    };
    let frame = compositor
        .render_to_rgba(&dev.device, &dev.queue, RS, &fp, &mut resolver)
        .expect("render");

    // A full-canvas clip with a downscaled texture must fill EVERY pixel with the
    // source color. Before the fix the content rendered only in the bottom-left
    // 4×4 quarter (quad scaled by tex 4 instead of nat 16), leaving the rest
    // opaque black — so any pixel being black catches the regression.
    for (i, px) in frame.rgba.chunks_exact(4).enumerate() {
        assert_eq!(
            px,
            &[40, 200, 80, 255],
            "pixel {i} must be the source color — a full-canvas downscaled texture must fill the whole frame (#125)"
        );
    }
}
