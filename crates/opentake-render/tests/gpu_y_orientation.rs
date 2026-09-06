//! Regression test for #193 — the compositor mirrored every clip (and every
//! mask) vertically.
//!
//! `Transform.center_y` is normalized canvas space with origin TOP-left (`0` =
//! top edge, `1` = bottom edge; SPEC / `opentake_domain::Transform` doc comment,
//! matches upstream `TransformOverlayView.movedTransform`: dragging DOWN on
//! screen increases `centerY`); `Mask` shapes share the same convention
//! (`opentake_domain::grade::Mask::coverage` doc comment). The vertex shader
//! mapped canvas-pixel Y (origin bottom-left, y up, per `affine_transform`'s CG
//! convention) straight onto wgpu NDC without accounting for wgpu's y-down
//! viewport/framebuffer convention (NDC `+1` = top row, `-1` = bottom row), so a
//! clip authored near the top (`center_y` small) rendered near the bottom of the
//! framebuffer and vice versa — i.e. the rendered row was `1 - center_y` instead
//! of `center_y`. The `canvas_uv` mask-space varying carried a matching
//! compensating flip (`1.0 - px.y / canvas.y`) that had silently canceled out
//! with the buggy NDC line, keeping masks paired with the (also mirrored)
//! framebuffer; fixing the NDC line alone — without also dropping `canvas_uv`'s
//! flip — would have mirrored every mask relative to the content it clips.
//!
//! This test places a small solid-color clip off-center along Y and asserts the
//! non-background pixel centroid lands in the half of the frame the clip's
//! `center_y` actually authors (the SAME solid-color-centroid technique as the
//! other GPU integration tests in this crate), plus a companion test that an
//! off-center mask clips to the correct screen region and does not double-flip.
//!
//! HARD CONSTRAINT: skips gracefully (eprintln + early return) when no GPU
//! adapter is available (CI / headless) — must never FAIL on GPU absence.

use std::rc::Rc;

use opentake_domain::{Clip, ClipType, Mask, MaskShape, Point, Point2, Timeline, Track, Transform};
use opentake_render::gpu::texture::upload_rgba;
use opentake_render::source::DecodedFrame;
use opentake_render::wgpu;
use opentake_render::{
    build_render_plan, Compositor, GpuTexture, RenderDevice, RenderSize, SourceMetrics,
    TextureResolver, TextureSource,
};

const RS: RenderSize = RenderSize {
    width: 64,
    height: 64,
};

struct Metrics;
impl SourceMetrics for Metrics {
    fn natural_size(&self, _r: &str) -> Option<(u32, u32)> {
        Some((64, 64))
    }
}

/// Resolves every source to a single solid premultiplied color (alpha 255).
struct SolidResolver<'d> {
    device: &'d wgpu::Device,
    queue: &'d wgpu::Queue,
    rgba: [u8; 4],
    cached: Option<Rc<GpuTexture>>,
}

impl TextureResolver for SolidResolver<'_> {
    fn resolve(&mut self, _source: &TextureSource, _frame: i64) -> Option<Rc<GpuTexture>> {
        if self.cached.is_none() {
            let mut buf = vec![0u8; 64 * 64 * 4];
            for px in buf.as_chunks_mut::<4>().0.iter_mut() {
                px.copy_from_slice(&self.rgba);
            }
            let frame = DecodedFrame::new(64, 64, buf, true);
            let tex = upload_rgba(self.device, self.queue, &frame, false, Some("solid"));
            self.cached = Some(Rc::new(tex));
        }
        self.cached.clone()
    }
}

/// A single small clip (30% of canvas per side) centered at `(0.5, center_y)`.
fn timeline_with_clip_at_center_y(center_y: f64) -> Timeline {
    let mut tl = Timeline::new();
    tl.fps = 30;
    tl.width = 64;
    tl.height = 64;
    let mut clip = Clip::new("c0", "asset", 0, 10);
    clip.transform = Transform {
        center_x: 0.5,
        center_y,
        width: 0.3,
        height: 0.3,
        ..Transform::default()
    };
    let mut track = Track::new("t0", ClipType::Video);
    track.clips.push(clip);
    tl.tracks.push(track);
    tl
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

/// Row-index centroid (0 = top row, H-1 = bottom row) of every non-black pixel.
/// Returns `None` if the frame is entirely background (nothing painted).
fn non_black_row_centroid(frame: &DecodedFrame) -> Option<f64> {
    let mut weight_sum = 0.0f64;
    let mut row_weight_sum = 0.0f64;
    for y in 0..frame.height {
        for x in 0..frame.width {
            let i = (y * frame.width + x) as usize * 4;
            let px = &frame.rgba[i..i + 4];
            // Background clears to opaque black (0,0,0,255); anything brighter
            // is the clip's solid color.
            if px[0] > 8 || px[1] > 8 || px[2] > 8 {
                weight_sum += 1.0;
                row_weight_sum += y as f64;
            }
        }
    }
    if weight_sum <= 0.0 {
        return None;
    }
    Some(row_weight_sum / weight_sum)
}

#[test]
fn clip_authored_near_top_renders_in_top_half() {
    let Some(dev) = device_or_skip("clip_authored_near_top_renders_in_top_half") else {
        return;
    };
    // center_y = 0.2, height = 0.3 -> clip spans normalized y in [0.05, 0.35],
    // i.e. entirely within the top half of the canvas (row 0..~22 of 64).
    let tl = timeline_with_clip_at_center_y(0.2);
    let frame = render(&dev, &tl, [220, 30, 30, 255]);
    let centroid = non_black_row_centroid(&frame).expect("clip must paint visible pixels");
    let h = frame.height as f64;
    assert!(
        centroid < 0.45 * h,
        "center_y=0.2 clip must render in the TOP half (row centroid < {:.1}), got row {:.2} \
         (frame {}x{}) — a value near {:.1} indicates the vertical-mirror regression (#193)",
        0.45 * h,
        centroid,
        frame.width,
        frame.height,
        0.8 * h,
    );
}

#[test]
fn clip_authored_near_bottom_renders_in_bottom_half() {
    let Some(dev) = device_or_skip("clip_authored_near_bottom_renders_in_bottom_half") else {
        return;
    };
    // center_y = 0.8, height = 0.3 -> clip spans normalized y in [0.65, 0.95],
    // i.e. entirely within the bottom half of the canvas.
    let tl = timeline_with_clip_at_center_y(0.8);
    let frame = render(&dev, &tl, [30, 220, 30, 255]);
    let centroid = non_black_row_centroid(&frame).expect("clip must paint visible pixels");
    let h = frame.height as f64;
    assert!(
        centroid > 0.55 * h,
        "center_y=0.8 clip must render in the BOTTOM half (row centroid > {:.1}), got row {:.2} \
         (frame {}x{}) — a value near {:.1} indicates the vertical-mirror regression (#193)",
        0.55 * h,
        centroid,
        frame.width,
        frame.height,
        0.2 * h,
    );
}

/// Cross-check: the two clips above must land on OPPOSITE halves, regardless of
/// which absolute row convention is correct. This alone would not catch a
/// globally-mirrored-but-internally-consistent bug, so it supplements (not
/// replaces) the absolute-half assertions above.
#[test]
fn top_and_bottom_authored_clips_render_on_opposite_halves() {
    let Some(dev) = device_or_skip("top_and_bottom_authored_clips_render_on_opposite_halves")
    else {
        return;
    };
    let top_frame = render(
        &dev,
        &timeline_with_clip_at_center_y(0.2),
        [220, 30, 30, 255],
    );
    let bottom_frame = render(
        &dev,
        &timeline_with_clip_at_center_y(0.8),
        [30, 220, 30, 255],
    );
    let top_centroid = non_black_row_centroid(&top_frame).expect("top clip visible");
    let bottom_centroid = non_black_row_centroid(&bottom_frame).expect("bottom clip visible");
    assert!(
        top_centroid < bottom_centroid,
        "center_y=0.2 clip (row {top_centroid:.2}) must render above center_y=0.8 clip \
         (row {bottom_centroid:.2})"
    );
}

/// Mask-side companion check for #193: `canvas_uv` (which `mask_coverage` reads)
/// is computed directly from `px`, NOT from the NDC expression this fix changes,
/// so an off-center mask must keep clipping to the SAME screen region before and
/// after the vertex-shader fix. A full-canvas white clip masked by a circle
/// centered at `(0.5, 0.2)` (authored near the top, same convention as
/// `Transform.center_y`) must reveal white in the top region and stay black
/// (background) in the bottom region — if `canvas_uv`'s existing flip were
/// mistakenly "corrected" alongside the NDC line (double-flip), this would
/// invert and fail.
#[test]
fn off_center_mask_clips_to_authored_screen_region_not_mirrored() {
    let Some(dev) = device_or_skip("off_center_mask_clips_to_authored_screen_region_not_mirrored")
    else {
        return;
    };
    let mut tl = Timeline::new();
    tl.fps = 30;
    tl.width = 64;
    tl.height = 64;
    let mut clip = Clip::new("c0", "asset", 0, 10);
    clip.transform = Transform::from_top_left(Point { x: 0.0, y: 0.0 }, 1.0, 1.0);
    clip.masks = vec![Mask {
        shape: MaskShape::Circle {
            center: Point2::new(0.5, 0.2),
            radius: Point2::new(0.15, 0.15),
        },
        feather: 0.0,
        invert: false,
        ..Mask::default()
    }];
    let mut track = Track::new("t0", ClipType::Video);
    track.clips.push(clip);
    tl.tracks.push(track);

    let frame = render(&dev, &tl, [255, 255, 255, 255]);
    let w = frame.width as i64;
    let sample = |x: i64, y: i64| -> [u8; 4] {
        let i = (y * w + x) as usize * 4;
        [
            frame.rgba[i],
            frame.rgba[i + 1],
            frame.rgba[i + 2],
            frame.rgba[i + 3],
        ]
    };

    // Mask center in pixels: (0.5*64, 0.2*64) = (32, 13) — well inside the top
    // region. Must be revealed (white), not clipped out.
    let top = sample(32, 13);
    assert!(
        top[0] > 200 && top[1] > 200 && top[2] > 200,
        "mask centered at authored y=0.2 must reveal white near row 13 (top), got {top:?}"
    );

    // Mirror point (0.5*64, 0.8*64) = (32, 51) — the row the mask would clip if
    // canvas_uv were double-flipped alongside the NDC fix. Must stay background
    // black (outside the mask), not white.
    let bottom = sample(32, 51);
    assert!(
        bottom[0] < 10 && bottom[1] < 10 && bottom[2] < 10,
        "row 51 (mirror of the mask's authored position) must stay black — \
         white here would mean canvas_uv got double-flipped alongside the NDC fix, got {bottom:?}"
    );
}

/// Resolves every source to a texture whose TOP half is red and BOTTOM half is
/// blue — the content-orientation companion to the solid-color position tests
/// above. A solid color is blind to a UV flip; this is not.
struct TwoBandResolver<'d> {
    device: &'d wgpu::Device,
    queue: &'d wgpu::Queue,
    cached: Option<Rc<GpuTexture>>,
}

impl TextureResolver for TwoBandResolver<'_> {
    fn resolve(&mut self, _source: &TextureSource, _frame: i64) -> Option<Rc<GpuTexture>> {
        if self.cached.is_none() {
            let mut buf = vec![0u8; 64 * 64 * 4];
            for y in 0..64usize {
                for x in 0..64usize {
                    let i = (y * 64 + x) * 4;
                    // Texture row 0 (top) red, bottom rows blue; premultiplied.
                    let c: [u8; 4] = if y < 32 {
                        [255, 0, 0, 255]
                    } else {
                        [0, 0, 255, 255]
                    };
                    buf[i..i + 4].copy_from_slice(&c);
                }
            }
            let frame = DecodedFrame::new(64, 64, buf, true);
            let tex = upload_rgba(self.device, self.queue, &frame, false, Some("two-band"));
            self.cached = Some(Rc::new(tex));
        }
        self.cached.clone()
    }
}

fn full_canvas_timeline(flip_vertical: bool) -> Timeline {
    let mut tl = Timeline::new();
    tl.fps = 30;
    tl.width = 64;
    tl.height = 64;
    let mut clip = Clip::new("c0", "asset", 0, 10);
    clip.transform = Transform {
        flip_vertical,
        ..Transform::default()
    };
    let mut track = Track::new("t0", ClipType::Video);
    track.clips.push(clip);
    tl.tracks.push(track);
    tl
}

fn render_two_band(dev: &RenderDevice, tl: &Timeline) -> DecodedFrame {
    let plan = build_render_plan(tl, RS, &Metrics);
    let fp = plan.frame(tl, 0);
    let compositor = Compositor::new(&dev.device);
    let mut resolver = TwoBandResolver {
        device: &dev.device,
        queue: &dev.queue,
        cached: None,
    };
    compositor
        .render_to_rgba(&dev.device, &dev.queue, RS, &fp, &mut resolver)
        .expect("render")
}

fn avg_row_rgb(frame: &DecodedFrame, y: u32) -> (f64, f64, f64) {
    let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
    for x in 0..frame.width {
        let i = ((y * frame.width + x) as usize) * 4;
        r += frame.rgba[i] as f64;
        g += frame.rgba[i + 1] as f64;
        b += frame.rgba[i + 2] as f64;
    }
    let n = frame.width as f64;
    (r / n, g / n, b / n)
}

/// #193 follow-up: the NDC y-flip fix must NOT invert texture CONTENT. The UV
/// `v` mapping has to stay paired with the flipped vertex positions so that a
/// texture's top row still renders at the top of its placed box. Regression:
/// the initial #193 fix flipped NDC y but left the legacy `1.0 - uv.y` flip
/// (which had been compensating the old mirrored NDC), turning every clip's
/// pixels upside down while its box sat at the right place.
#[test]
fn texture_top_row_renders_at_top_of_placed_box() {
    let Some(dev) = device_or_skip("texture_top_row_renders_at_top_of_placed_box") else {
        return;
    };
    let frame = render_two_band(&dev, &full_canvas_timeline(false));
    let (tr, _, tb) = avg_row_rgb(&frame, 8);
    let (br, _, bb) = avg_row_rgb(&frame, 56);
    assert!(
        tr > 180.0 && tb < 60.0,
        "top of frame must be the texture's RED top band, got avg rgb row8 = ({tr:.0}, _, {tb:.0})"
    );
    assert!(
        bb > 180.0 && br < 60.0,
        "bottom of frame must be the texture's BLUE bottom band, got avg rgb row56 = ({br:.0}, _, {bb:.0})"
    );
}

/// `flip_vertical: true` must still flip the content (blue band on top) — the
/// UV fix must not eat the authored flip.
#[test]
fn flip_vertical_still_inverts_content() {
    let Some(dev) = device_or_skip("flip_vertical_still_inverts_content") else {
        return;
    };
    let frame = render_two_band(&dev, &full_canvas_timeline(true));
    let (tr, _, tb) = avg_row_rgb(&frame, 8);
    let (br, _, bb) = avg_row_rgb(&frame, 56);
    assert!(
        tb > 180.0 && tr < 60.0,
        "flipped clip: top must be BLUE, got avg rgb row8 = ({tr:.0}, _, {tb:.0})"
    );
    assert!(
        br > 180.0 && bb < 60.0,
        "flipped clip: bottom must be RED, got avg rgb row56 = ({br:.0}, _, {bb:.0})"
    );
}
