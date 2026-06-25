//! GPU integration tests for the A-tier per-pixel chain (color grade, chroma
//! key, masks) rendered through the real wgpu compositor.
//!
//! Like `gpu_smoke.rs`, every test SKIPS gracefully (eprintln + early return)
//! when no GPU device is available (CI / headless) — it must never FAIL on GPU
//! absence. The pure pixel math is exhaustively unit-tested in
//! `opentake_domain::grade`; these tests verify the WGSL mirror is wired up and
//! produces the expected effect end-to-end.

use std::rc::Rc;

use opentake_domain::{
    ChromaKey, Clip, ClipType, ColorGrade, Mask, MaskShape, Point, Point2, Rgb, Timeline, Track,
    Transform,
};
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

/// Resolves every source to a single solid premultiplied color (alpha 255, so
/// premultiplied == straight). The compositor un-premultiplies internally before
/// the chain, so a fully-opaque solid is the clean test input.
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
            let frame = DecodedFrame::new(16, 16, buf, true);
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

fn pixel_at(frame: &DecodedFrame, x: u32, y: u32) -> [u8; 4] {
    let i = (y * frame.width + x) as usize * 4;
    [
        frame.rgba[i],
        frame.rgba[i + 1],
        frame.rgba[i + 2],
        frame.rgba[i + 3],
    ]
}

fn device_or_skip(test: &str) -> Option<RenderDevice> {
    match RenderDevice::try_new() {
        Ok(d) => Some(d),
        Err(e) => {
            eprintln!("[skip] {test}: no GPU device ({e})");
            None
        }
    }
}

fn render(dev: &RenderDevice, tl: &Timeline, rgba: [u8; 4]) -> DecodedFrame {
    let plan = build_render_plan(tl, RS, &Metrics);
    let fp = plan.frame(tl, 0);
    let compositor = Compositor::new(&dev.device);
    let mut resolver = SolidResolver {
        device: &dev.device,
        queue: &dev.queue,
        rgba,
        cached: None,
    };
    compositor
        .render_to_rgba(&dev.device, &dev.queue, RS, &fp, &mut resolver)
        .expect("render")
}

#[test]
fn color_grade_zero_saturation_greyscales() {
    let Some(dev) = device_or_skip("color_grade_zero_saturation_greyscales") else {
        return;
    };
    let mut tl = full_canvas_timeline();
    tl.tracks[0].clips[0].color_grade = Some(ColorGrade {
        saturation: 0.0,
        ..Default::default()
    });
    // A saturated red, fully opaque, full-canvas.
    let frame = render(&dev, &tl, [220, 30, 30, 255]);
    let c = center_pixel(&frame);
    // Greyscale -> R == G == B (within a small rounding tolerance from the
    // sRGB<->linear round-trip and 8-bit quantization).
    let (r, g, b) = (c[0] as i32, c[1] as i32, c[2] as i32);
    assert!(
        (r - g).abs() <= 3 && (g - b).abs() <= 3,
        "expected grey, got {c:?}"
    );
    assert_eq!(c[3], 255, "opaque");
}

#[test]
fn color_grade_exposure_brightens() {
    let Some(dev) = device_or_skip("color_grade_exposure_brightens") else {
        return;
    };
    // Baseline mid-grey with no grade.
    let base = render(&dev, &full_canvas_timeline(), [100, 100, 100, 255]);
    let base_c = center_pixel(&base);

    // +1 stop exposure should brighten every channel.
    let mut tl = full_canvas_timeline();
    tl.tracks[0].clips[0].color_grade = Some(ColorGrade {
        exposure: 1.0,
        ..Default::default()
    });
    let bright = render(&dev, &tl, [100, 100, 100, 255]);
    let bright_c = center_pixel(&bright);

    assert!(
        bright_c[0] > base_c[0] && bright_c[1] > base_c[1] && bright_c[2] > base_c[2],
        "exposure +1 should brighten: base {base_c:?} bright {bright_c:?}"
    );
}

#[test]
fn color_grade_identity_is_passthrough() {
    let Some(dev) = device_or_skip("color_grade_identity_is_passthrough") else {
        return;
    };
    // An identity grade is dropped at plan-build time, but even if forced it must
    // be a visual no-op. Compare a graded-with-identity render to an ungraded one.
    let plain = render(&dev, &full_canvas_timeline(), [123, 77, 200, 255]);

    let mut tl = full_canvas_timeline();
    tl.tracks[0].clips[0].color_grade = Some(ColorGrade::default());
    let graded = render(&dev, &tl, [123, 77, 200, 255]);

    let a = center_pixel(&plain);
    let b = center_pixel(&graded);
    for i in 0..4 {
        assert!(
            (a[i] as i32 - b[i] as i32).abs() <= 1,
            "identity grade must be passthrough: {a:?} vs {b:?}"
        );
    }
}

#[test]
fn chroma_key_removes_green() {
    let Some(dev) = device_or_skip("chroma_key_removes_green") else {
        return;
    };
    let mut tl = full_canvas_timeline();
    tl.tracks[0].clips[0].chroma_key = Some(ChromaKey::default());
    // Pure green source -> keyed out -> alpha 0 -> reveals the opaque black
    // background.
    let frame = render(&dev, &tl, [0, 255, 0, 255]);
    let c = center_pixel(&frame);
    assert_eq!(c[3], 255, "background is opaque black");
    assert!(
        c[0] < 10 && c[1] < 10 && c[2] < 10,
        "keyed green should reveal black, got {c:?}"
    );
}

#[test]
fn chroma_key_keeps_non_key_color() {
    let Some(dev) = device_or_skip("chroma_key_keeps_non_key_color") else {
        return;
    };
    let mut tl = full_canvas_timeline();
    tl.tracks[0].clips[0].chroma_key = Some(ChromaKey {
        key_color: Rgb::new(0.0, 1.0, 0.0),
        similarity: 0.15,
        smoothness: 0.2,
        spill: 0.0,
    });
    // Red is far from the green key -> kept opaque.
    let frame = render(&dev, &tl, [220, 20, 20, 255]);
    let c = center_pixel(&frame);
    assert_eq!(c[3], 255, "non-key color stays opaque");
    assert!(c[0] > 180, "red channel preserved, got {c:?}");
}

#[test]
fn circle_mask_clips_to_center() {
    let Some(dev) = device_or_skip("circle_mask_clips_to_center") else {
        return;
    };
    let mut tl = full_canvas_timeline();
    tl.tracks[0].clips[0].masks = vec![Mask {
        shape: MaskShape::Circle {
            center: Point2::new(0.5, 0.5),
            radius: Point2::new(0.25, 0.25),
        },
        feather: 0.0,
        invert: false,
    }];
    // White source, masked to a small centered circle over black.
    let frame = render(&dev, &tl, [255, 255, 255, 255]);
    // Center is inside the circle -> white.
    let c = center_pixel(&frame);
    assert!(c[0] > 200, "center inside mask should be white, got {c:?}");
    // A corner is outside the circle -> black background.
    let corner = pixel_at(&frame, 0, 0);
    assert!(
        corner[0] < 10 && corner[1] < 10 && corner[2] < 10,
        "corner outside mask should be black, got {corner:?}"
    );
    assert_eq!(corner[3], 255, "background opaque");
}

#[test]
fn inverted_mask_clips_out_center() {
    let Some(dev) = device_or_skip("inverted_mask_clips_out_center") else {
        return;
    };
    let mut tl = full_canvas_timeline();
    tl.tracks[0].clips[0].masks = vec![Mask {
        shape: MaskShape::Circle {
            center: Point2::new(0.5, 0.5),
            radius: Point2::new(0.25, 0.25),
        },
        feather: 0.0,
        invert: true,
    }];
    let frame = render(&dev, &tl, [255, 255, 255, 255]);
    // Inverted: center is now masked OUT -> black.
    let c = center_pixel(&frame);
    assert!(
        c[0] < 10 && c[1] < 10 && c[2] < 10,
        "inverted mask should clip out the center, got {c:?}"
    );
    // Corner is now kept -> white.
    let corner = pixel_at(&frame, 0, 0);
    assert!(
        corner[0] > 200,
        "corner kept by inverted mask, got {corner:?}"
    );
}
