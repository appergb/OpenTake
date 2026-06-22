//! GPU compositor smoke test (SPEC §6.3 step 2). Creates a REAL wgpu device,
//! renders one offscreen frame, and verifies the read-back pixels.
//!
//! HARD CONSTRAINT: if no GPU device is available (CI / headless sandbox), the
//! test SKIPS gracefully (eprintln + early return) — it must never FAIL on GPU
//! absence. Run with `--nocapture` to see the skip notice.

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

struct Metrics;
impl SourceMetrics for Metrics {
    fn natural_size(&self, _r: &str) -> Option<(u32, u32)> {
        Some((16, 16))
    }
}

/// Resolves every source to a single solid-color texture (premultiplied input
/// flag follows the draw, not the texture).
struct SolidResolver<'d> {
    device: &'d wgpu::Device,
    queue: &'d wgpu::Queue,
    rgba: [u8; 4],
    cached: Option<Rc<GpuTexture>>,
}

impl TextureResolver for SolidResolver<'_> {
    fn resolve(&mut self, _source: &TextureSource, _frame: i64) -> Option<Rc<GpuTexture>> {
        if self.cached.is_none() {
            let mut buf = vec![0u8; 16 * 16 * 4];
            for px in buf.chunks_exact_mut(4) {
                px.copy_from_slice(&self.rgba);
            }
            let frame = DecodedFrame::new(16, 16, buf, true); // already premultiplied
            let tex = upload_rgba(self.device, self.queue, &frame, false, Some("solid"));
            self.cached = Some(Rc::new(tex));
        }
        self.cached.clone()
    }
}

fn full_canvas_timeline() -> Timeline {
    let mut tl = Timeline::new();
    tl.fps = 30;
    tl.width = 16;
    tl.height = 16;
    let mut clip = Clip::new("c0", "asset", 0, 10);
    clip.transform = Transform::from_top_left(Point { x: 0.0, y: 0.0 }, 1.0, 1.0);
    let mut track = Track::new("t0", ClipType::Video);
    track.clips.push(clip);
    tl.tracks.push(track);
    tl
}

fn center_pixel(frame: &DecodedFrame) -> [u8; 4] {
    let x = frame.width / 2;
    let y = frame.height / 2;
    let i = (y * frame.width + x) as usize * 4;
    [
        frame.rgba[i],
        frame.rgba[i + 1],
        frame.rgba[i + 2],
        frame.rgba[i + 3],
    ]
}

/// Acquire a device or skip the whole test (return None) with a notice.
fn device_or_skip(test: &str) -> Option<RenderDevice> {
    match RenderDevice::try_new() {
        Ok(d) => Some(d),
        Err(e) => {
            eprintln!("[skip] {test}: no GPU device ({e})");
            None
        }
    }
}

#[test]
fn empty_plan_clears_to_opaque_black() {
    let Some(dev) = device_or_skip("empty_plan_clears_to_opaque_black") else {
        return;
    };
    let tl = Timeline::new();
    let plan = build_render_plan(&tl, RS, &Metrics);
    let fp = plan.frame(&tl, 0);

    let compositor = Compositor::new(&dev.device);
    let mut resolver = SolidResolver {
        device: &dev.device,
        queue: &dev.queue,
        rgba: [255, 0, 0, 255],
        cached: None,
    };
    let frame = compositor
        .render_to_rgba(&dev.device, &dev.queue, RS, &fp, &mut resolver)
        .expect("render");

    // Every pixel opaque black.
    for px in frame.rgba.chunks_exact(4) {
        assert_eq!(px, &[0, 0, 0, 255], "clear color must be opaque black");
    }
}

#[test]
fn full_canvas_clip_fills_with_source_color() {
    let Some(dev) = device_or_skip("full_canvas_clip_fills_with_source_color") else {
        return;
    };
    let tl = full_canvas_timeline();
    let plan = build_render_plan(&tl, RS, &Metrics);
    let fp = plan.frame(&tl, 0);
    assert_eq!(fp.draws.len(), 1);

    let compositor = Compositor::new(&dev.device);
    let mut resolver = SolidResolver {
        device: &dev.device,
        queue: &dev.queue,
        rgba: [200, 40, 40, 255],
        cached: None,
    };
    let frame = compositor
        .render_to_rgba(&dev.device, &dev.queue, RS, &fp, &mut resolver)
        .expect("render");

    // Center pixel must be the source color (full-canvas quad covers everything).
    let c = center_pixel(&frame);
    assert_eq!(c, [200, 40, 40, 255], "center should be the source color");

    // Corners too (the quad maps source [0,nat] across the whole canvas).
    let top_left = [frame.rgba[0], frame.rgba[1], frame.rgba[2], frame.rgba[3]];
    assert_eq!(top_left, [200, 40, 40, 255], "corner should be covered");
}

#[test]
fn opacity_half_blends_over_black() {
    let Some(dev) = device_or_skip("opacity_half_blends_over_black") else {
        return;
    };
    // White source at 50% opacity over black -> ~128 grey (premultiplied over).
    let mut tl = full_canvas_timeline();
    tl.tracks[0].clips[0].opacity = 0.5;
    let plan = build_render_plan(&tl, RS, &Metrics);
    let fp = plan.frame(&tl, 0);
    approx_opacity(fp.draws[0].opacity, 0.5);

    let compositor = Compositor::new(&dev.device);
    let mut resolver = SolidResolver {
        device: &dev.device,
        queue: &dev.queue,
        rgba: [255, 255, 255, 255],
        cached: None,
    };
    let frame = compositor
        .render_to_rgba(&dev.device, &dev.queue, RS, &fp, &mut resolver)
        .expect("render");

    let c = center_pixel(&frame);
    // premultiplied: out = src*0.5 + black*(1-0.5) = 0.5 white. ~127/128.
    for &ch in &c[0..3] {
        assert!(
            (120..=135).contains(&ch),
            "expected ~half-white, got {ch} (full pixel {c:?})"
        );
    }
    assert_eq!(c[3], 255, "background opacity keeps result opaque");
}

#[test]
fn two_tracks_top_layer_wins_when_opaque() {
    let Some(dev) = device_or_skip("two_tracks_top_layer_wins_when_opaque") else {
        return;
    };
    // Bottom track + top track, both full-canvas opaque. Top must win.
    let mut tl = Timeline::new();
    tl.fps = 30;
    tl.width = 16;
    tl.height = 16;
    let mk = |id: &str| {
        let mut c = Clip::new(id, "asset", 0, 10);
        c.transform = Transform::from_top_left(Point { x: 0.0, y: 0.0 }, 1.0, 1.0);
        c
    };
    let mut t0 = Track::new("t0", ClipType::Video);
    t0.clips.push(mk("bottom"));
    let mut t1 = Track::new("t1", ClipType::Video);
    t1.clips.push(mk("top"));
    tl.tracks.push(t0);
    tl.tracks.push(t1);

    let plan = build_render_plan(&tl, RS, &Metrics);
    let fp = plan.frame(&tl, 0);
    assert_eq!(fp.draws.len(), 2);
    // Draw order: bottom first, top second (verified by the plan layer).
    assert_eq!(fp.draws[0].clip_id, "bottom");
    assert_eq!(fp.draws[1].clip_id, "top");

    let compositor = Compositor::new(&dev.device);
    // Draws resolve in plan order: first = bottom (red), second = top (green).
    // The opaque green top must overwrite the red bottom.
    struct DrawOrder<'d> {
        device: &'d wgpu::Device,
        queue: &'d wgpu::Queue,
        n: usize,
    }
    impl TextureResolver for DrawOrder<'_> {
        fn resolve(&mut self, _source: &TextureSource, _f: i64) -> Option<Rc<GpuTexture>> {
            let color = if self.n == 0 {
                [255, 0, 0, 255] // bottom
            } else {
                [0, 255, 0, 255] // top
            };
            self.n += 1;
            Some(make_solid(self.device, self.queue, color))
        }
    }
    let mut resolver = DrawOrder {
        device: &dev.device,
        queue: &dev.queue,
        n: 0,
    };
    let frame = compositor
        .render_to_rgba(&dev.device, &dev.queue, RS, &fp, &mut resolver)
        .expect("render");
    let c = center_pixel(&frame);
    // Top (green, drawn second) is opaque and covers the bottom (red).
    assert_eq!(c, [0, 255, 0, 255], "top opaque layer must win");
}

#[test]
fn read_back_round_trips_through_png() {
    let Some(dev) = device_or_skip("read_back_round_trips_through_png") else {
        return;
    };
    let tl = full_canvas_timeline();
    let plan = build_render_plan(&tl, RS, &Metrics);
    let fp = plan.frame(&tl, 0);
    let compositor = Compositor::new(&dev.device);
    let mut resolver = SolidResolver {
        device: &dev.device,
        queue: &dev.queue,
        rgba: [10, 120, 250, 255],
        cached: None,
    };
    let frame = compositor
        .render_to_rgba(&dev.device, &dev.queue, RS, &fp, &mut resolver)
        .expect("render");

    // Encode -> decode PNG in-memory (no disk, no network) and compare.
    let img = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba.clone())
        .expect("valid rgba");
    let mut bytes: Vec<u8> = Vec::new();
    {
        use image::ImageEncoder;
        let enc = image::codecs::png::PngEncoder::new(&mut bytes);
        enc.write_image(
            img.as_raw(),
            frame.width,
            frame.height,
            image::ExtendedColorType::Rgba8,
        )
        .expect("encode png");
    }
    let decoded = image::load_from_memory(&bytes)
        .expect("decode png")
        .to_rgba8();
    assert_eq!(
        decoded.as_raw(),
        &frame.rgba,
        "png round-trip preserves pixels"
    );
}

fn make_solid(device: &wgpu::Device, queue: &wgpu::Queue, rgba: [u8; 4]) -> Rc<GpuTexture> {
    let mut buf = vec![0u8; 16 * 16 * 4];
    for px in buf.chunks_exact_mut(4) {
        px.copy_from_slice(&rgba);
    }
    let frame = DecodedFrame::new(16, 16, buf, true);
    Rc::new(upload_rgba(device, queue, &frame, false, Some("solid2")))
}

fn approx_opacity(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}
