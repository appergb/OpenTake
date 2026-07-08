//! Pixel-diff framework for the wgpu compositor vs upstream AVFoundation
//! (SPEC §6.2). CI can't run AVFoundation to mint golden references, so this
//! file uses the **self-consistent + known-point** strategy mandated by SPEC
//! §6.3 steps 1-6:
//!
//! 1. Known-point `affine_transform` unit checks pinning upstream
//!    `CompositionBuilder.affineTransform` L599-614 (no GPU).
//! 2. Quadrant-marker orientation texture - a single y/uv flip is caught by
//!    checking all four corners land in their authored quadrant.
//! 3. Round-trip self-consistency via PSNR (identical input -> >=50 dB).
//! 4. Two-track half-opacity blend verified against hand-computed premult-over.
//! 5. Fade envelope endpoints (`opacity_at(start)=0`, mid = `smoothstep(0.5)=0.5`).
//! 6. Transform / crop keyframe per-frame evaluation = `*_at(f)`.
//! 7. Text overlay composited above video (cosmic-text raster).
//! 8. SSIM tool self-consistency (identical frames -> ~=1.0).
//!
//! GPU tests SKIP gracefully when no adapter is present (CI / headless); the
//! pure-function tests always run.

use std::rc::Rc;

use opentake_domain::{
    AnimPair, Clip, ClipType, Crop, Interpolation, Keyframe, KeyframeTrack, Point, TextStyle,
    Timeline, Track, Transform,
};
use opentake_render::gpu::texture::upload_rgba;
use opentake_render::source::DecodedFrame;
use opentake_render::wgpu;
use opentake_render::{
    affine_transform, build_render_plan, compose, crop_to_uv, Compositor, CosmicTextRasterizer,
    GpuTexture, RenderDevice, RenderSize, SourceMetrics, TextRasterRequest, TextRasterizer,
    TextureCache, TextureResolver, TextureSource,
};

const Q: u32 = 64;
const RS: RenderSize = RenderSize {
    width: Q,
    height: Q,
};

struct Metrics;
impl SourceMetrics for Metrics {
    fn natural_size(&self, _r: &str) -> Option<(u32, u32)> {
        Some((Q, Q))
    }
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

fn approx(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

fn approx_affine(a: [f64; 6], b: [f64; 6]) {
    for i in 0..6 {
        assert!(
            (a[i] - b[i]).abs() < 1e-9,
            "affine[{i}]: {} != {}",
            a[i],
            b[i]
        );
    }
}

fn apply(m: [f64; 6], x: f64, y: f64) -> (f64, f64) {
    (x * m[0] + y * m[2] + m[4], x * m[1] + y * m[3] + m[5])
}

fn psnr(a: &[u8], b: &[u8]) -> f64 {
    let n = a.len().min(b.len());
    if n == 0 {
        return f64::INFINITY;
    }
    let mut se = 0.0;
    for i in 0..n {
        let d = a[i] as f64 - b[i] as f64;
        se += d * d;
    }
    let mse = se / n as f64;
    if mse == 0.0 {
        f64::INFINITY
    } else {
        10.0 * (255.0 * 255.0 / mse).log10()
    }
}

fn ssim(a: &[u8], b: &[u8], w: u32, h: u32) -> f64 {
    const BW: usize = 8;
    let nw = (w as usize) / BW;
    let nh = (h as usize) / BW;
    if nw == 0 || nh == 0 {
        return 0.0;
    }
    let c1 = (0.01 * 255.0_f64).powi(2);
    let c2 = (0.03 * 255.0_f64).powi(2);
    let mut sum = 0.0;
    let mut cnt = 0usize;
    let mut sx = [0.0f64; 64];
    let mut sy = [0.0f64; 64];
    for by in 0..nh {
        for bx in 0..nw {
            for y in 0..BW {
                for x in 0..BW {
                    let i = ((by * BW + y) * w as usize + bx * BW + x) * 4;
                    let ga = (a[i] as f64 + a[i + 1] as f64 + a[i + 2] as f64) / 3.0;
                    let gb = (b[i] as f64 + b[i + 1] as f64 + b[i + 2] as f64) / 3.0;
                    sx[y * BW + x] = ga;
                    sy[y * BW + x] = gb;
                }
            }
            let n = sx.len() as f64;
            let mx = sx.iter().sum::<f64>() / n;
            let my = sy.iter().sum::<f64>() / n;
            let vx = sx.iter().map(|v| (v - mx).powi(2)).sum::<f64>() / n;
            let vy = sy.iter().map(|v| (v - my).powi(2)).sum::<f64>() / n;
            let cxy = sx
                .iter()
                .zip(sy.iter())
                .map(|(x, y)| (x - mx) * (y - my))
                .sum::<f64>()
                / n;
            let s = ((2.0 * mx * my + c1) * (2.0 * cxy + c2))
                / ((mx * mx + my * my + c1) * (vx + vy + c2));
            sum += s;
            cnt += 1;
        }
    }
    sum / cnt as f64
}

fn make_solid(rgba: [u8; 4]) -> DecodedFrame {
    let mut buf = vec![0u8; (Q * Q * 4) as usize];
    for px in buf.chunks_exact_mut(4) {
        px.copy_from_slice(&rgba);
    }
    DecodedFrame::new(Q, Q, buf, true)
}

fn make_quadrant_texture() -> DecodedFrame {
    let mut buf = vec![0u8; (Q * Q * 4) as usize];
    for y in 0..Q as usize {
        for x in 0..Q as usize {
            let i = (y * Q as usize + x) * 4;
            let c: [u8; 4] = if x < Q as usize / 2 {
                if y < Q as usize / 2 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 255]
                }
            } else if y < Q as usize / 2 {
                [0, 255, 0, 255]
            } else {
                [255, 255, 0, 255]
            };
            buf[i..i + 4].copy_from_slice(&c);
        }
    }
    DecodedFrame::new(Q, Q, buf, true)
}

struct SolidResolver<'d> {
    device: &'d wgpu::Device,
    queue: &'d wgpu::Queue,
    rgba: [u8; 4],
    cached: Option<Rc<GpuTexture>>,
}

impl TextureResolver for SolidResolver<'_> {
    fn resolve(&mut self, _source: &TextureSource, _frame: i64) -> Option<Rc<GpuTexture>> {
        if self.cached.is_none() {
            let frame = make_solid(self.rgba);
            let tex = upload_rgba(self.device, self.queue, &frame, false, Some("solid"));
            self.cached = Some(Rc::new(tex));
        }
        self.cached.clone()
    }
}

struct QuadrantResolver<'d> {
    device: &'d wgpu::Device,
    queue: &'d wgpu::Queue,
    cached: Option<Rc<GpuTexture>>,
}

impl TextureResolver for QuadrantResolver<'_> {
    fn resolve(&mut self, _source: &TextureSource, _frame: i64) -> Option<Rc<GpuTexture>> {
        if self.cached.is_none() {
            let frame = make_quadrant_texture();
            let tex = upload_rgba(self.device, self.queue, &frame, false, Some("quadrant"));
            self.cached = Some(Rc::new(tex));
        }
        self.cached.clone()
    }
}

fn full_canvas_clip(id: &str) -> Clip {
    let mut c = Clip::new(id, "asset", 0, 20);
    c.transform = Transform::from_top_left(Point { x: 0.0, y: 0.0 }, 1.0, 1.0);
    c
}

fn full_canvas_timeline_with(clip: Clip) -> Timeline {
    let mut tl = Timeline::new();
    tl.fps = 30;
    tl.width = Q as i32;
    tl.height = Q as i32;
    let mut track = Track::new("t0", ClipType::Video);
    track.clips.push(clip);
    tl.tracks.push(track);
    tl
}

#[test]
fn affine_centered_half_canvas_matches_hand_computed() {
    let mut t = Transform::from_center(Point { x: 0.5, y: 0.5 }, 0.5, 0.5);
    t.rotation = 0.0;
    let rs = RenderSize::new(100, 100);
    let m = affine_transform(&t, (100.0, 100.0), rs);
    approx_affine(m, [0.5, 0.0, 0.0, 0.5, 25.0, 25.0]);
    let (x0, y0) = apply(m, 0.0, 0.0);
    approx(x0, 25.0);
    approx(y0, 25.0);
    let (x1, y1) = apply(m, 100.0, 100.0);
    approx(x1, 75.0);
    approx(y1, 75.0);
}

#[test]
fn affine_rotation_45_matches_compose_chain() {
    let mut t = Transform::from_top_left(Point { x: 0.0, y: 0.0 }, 1.0, 1.0);
    t.rotation = 45.0;
    let rs = RenderSize::new(100, 100);
    let m = affine_transform(&t, (100.0, 100.0), rs);
    let placed = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let s1 = compose(placed, [1.0, 0.0, 0.0, 1.0, -50.0, -50.0]);
    let r = 45.0_f64.to_radians();
    let (rsin, rcos) = r.sin_cos();
    let rot = [rcos, rsin, -rsin, rcos, 0.0, 0.0];
    let s2 = compose(s1, rot);
    let expected = compose(s2, [1.0, 0.0, 0.0, 1.0, 50.0, 50.0]);
    approx_affine(m, expected);
    let (cx, cy) = apply(m, 50.0, 50.0);
    approx(cx, 50.0);
    approx(cy, 50.0);
}

#[test]
fn affine_flip_horizontal_with_smaller_source() {
    let mut t = Transform::from_top_left(Point { x: 0.0, y: 0.0 }, 1.0, 1.0);
    t.flip_horizontal = true;
    let rs = RenderSize::new(100, 100);
    let m = affine_transform(&t, (200.0, 100.0), rs);
    approx_affine(m, [-0.5, 0.0, 0.0, 1.0, 100.0, 0.0]);
    let (x, y) = apply(m, 0.0, 0.0);
    approx(x, 100.0);
    approx(y, 0.0);
    let (x2, y2) = apply(m, 200.0, 100.0);
    approx(x2, 0.0);
    approx(y2, 100.0);
}

#[test]
fn crop_to_uv_visible_insets_match_domain_fractions() {
    let c = Crop {
        left: 0.1,
        top: 0.2,
        right: 0.3,
        bottom: 0.4,
    };
    let uv = crop_to_uv(c);
    approx(uv.0, 0.1);
    approx(uv.1, 0.2);
    approx(uv.2, 0.7);
    approx(uv.3, 0.6);
}

#[test]
fn compose_with_identity_is_noop() {
    let m = [2.0, 0.5, -0.3, 1.5, 10.0, -4.0];
    approx_affine(compose(m, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]), m);
    approx_affine(compose([1.0, 0.0, 0.0, 1.0, 0.0, 0.0], m), m);
}

#[test]
fn quadrant_markers_land_in_authored_corners() {
    let Some(dev) = device_or_skip("quadrant_markers_land_in_authored_corners") else {
        return;
    };
    let tl = full_canvas_timeline_with(full_canvas_clip("c0"));
    let plan = build_render_plan(&tl, RS, &Metrics);
    let fp = plan.frame(&tl, 0);
    let compositor = Compositor::new(&dev.device);
    let mut resolver = QuadrantResolver {
        device: &dev.device,
        queue: &dev.queue,
        cached: None,
    };
    let frame = compositor
        .render_to_rgba(&dev.device, &dev.queue, RS, &fp, &mut resolver)
        .expect("render");
    let sample = |x: u32, y: u32| {
        let i = (y * Q + x) as usize * 4;
        [
            frame.rgba[i],
            frame.rgba[i + 1],
            frame.rgba[i + 2],
            frame.rgba[i + 3],
        ]
    };
    let tl_px = sample(16, 16);
    assert!(tl_px[0] > 200 && tl_px[1] < 50 && tl_px[2] < 50, "TL red, got {tl_px:?}");
    let tr = sample(48, 16);
    assert!(tr[1] > 200 && tr[0] < 50, "TR green, got {tr:?}");
    let bl = sample(16, 48);
    assert!(bl[2] > 200 && bl[0] < 50, "BL blue, got {bl:?}");
    let br = sample(48, 48);
    assert!(br[0] > 200 && br[1] > 200 && br[2] < 50, "BR yellow, got {br:?}");
}

#[test]
fn quadrant_round_trip_psnr_is_high() {
    let Some(dev) = device_or_skip("quadrant_round_trip_psnr_is_high") else {
        return;
    };
    let tl = full_canvas_timeline_with(full_canvas_clip("c0"));
    let plan = build_render_plan(&tl, RS, &Metrics);
    let fp = plan.frame(&tl, 0);
    let compositor = Compositor::new(&dev.device);
    let mut resolver = QuadrantResolver {
        device: &dev.device,
        queue: &dev.queue,
        cached: None,
    };
    let frame = compositor
        .render_to_rgba(&dev.device, &dev.queue, RS, &fp, &mut resolver)
        .expect("render");
    let src = make_quadrant_texture();
    let p = psnr(&src.rgba, &frame.rgba);
    assert!(p >= 50.0, "round-trip PSNR {p:.2} dB < 50 dB");
}

#[test]
fn half_opacity_two_track_blend_matches_hand_computed() {
    let Some(dev) = device_or_skip("half_opacity_two_track_blend_matches_hand_computed") else {
        return;
    };
    let mut tl = Timeline::new();
    tl.fps = 30;
    tl.width = Q as i32;
    tl.height = Q as i32;
    let mut bot = full_canvas_clip("bottom");
    bot.opacity = 0.5;
    let mut top = full_canvas_clip("top");
    top.opacity = 0.5;
    let mut t0 = Track::new("t0", ClipType::Video);
    t0.clips.push(top);
    let mut t1 = Track::new("t1", ClipType::Video);
    t1.clips.push(bot);
    tl.tracks.push(t0);
    tl.tracks.push(t1);

    let plan = build_render_plan(&tl, RS, &Metrics);
    let fp = plan.frame(&tl, 0);
    assert_eq!(fp.draws.len(), 2);
    assert_eq!(fp.draws[0].clip_id, "bottom");
    assert_eq!(fp.draws[1].clip_id, "top");

    struct TwoColor<'d> {
        device: &'d wgpu::Device,
        queue: &'d wgpu::Queue,
        n: usize,
    }
    impl TextureResolver for TwoColor<'_> {
        fn resolve(&mut self, _s: &TextureSource, _f: i64) -> Option<Rc<GpuTexture>> {
            let c = if self.n == 0 { [255, 0, 0, 255] } else { [0, 255, 0, 255] };
            self.n += 1;
            Some(Rc::new(upload_rgba(
                self.device,
                self.queue,
                &make_solid(c),
                false,
                Some("two"),
            )))
        }
    }
    let compositor = Compositor::new(&dev.device);
    let mut resolver = TwoColor {
        device: &dev.device,
        queue: &dev.queue,
        n: 0,
    };
    let frame = compositor
        .render_to_rgba(&dev.device, &dev.queue, RS, &fp, &mut resolver)
        .expect("render");
    let i = (Q / 2 * Q + Q / 2) as usize * 4;
    let [r, g, b, a] = [
        frame.rgba[i],
        frame.rgba[i + 1],
        frame.rgba[i + 2],
        frame.rgba[i + 3],
    ];
    assert!((55..=75).contains(&r), "R {r} not in 55..=75");
    assert!((120..=135).contains(&g), "G {g} not in 120..=135");
    assert!(b < 10, "B {b} should be ~0");
    assert_eq!(a, 255, "result must be opaque");
}

#[test]
fn fade_envelope_smoothstep_endpoints_and_midpoint() {
    let mut clip = full_canvas_clip("c0");
    clip.fade_in_frames = 10;
    clip.fade_in_interpolation = Interpolation::Smooth;
    clip.opacity = 1.0;
    approx(clip.opacity_at(0), 0.0);
    approx(clip.opacity_at(5), 0.5);
    approx(clip.opacity_at(10), 1.0);
    let mut lin = full_canvas_clip("c1");
    lin.fade_in_frames = 10;
    lin.fade_in_interpolation = Interpolation::Linear;
    approx(clip.opacity_at(2), 0.104);
    approx(lin.opacity_at(2), 0.2);
}

#[test]
fn fade_midframe_renders_half_brightness() {
    let Some(dev) = device_or_skip("fade_midframe_renders_half_brightness") else {
        return;
    };
    let mut clip = full_canvas_clip("c0");
    clip.fade_in_frames = 10;
    clip.fade_in_interpolation = Interpolation::Smooth;
    clip.opacity = 1.0;
    let tl = full_canvas_timeline_with(clip);
    let plan = build_render_plan(&tl, RS, &Metrics);
    let fp = plan.frame(&tl, 5);
    approx(fp.draws[0].opacity, 0.5);

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
    let i = (Q / 2 * Q + Q / 2) as usize * 4;
    let c = &frame.rgba[i..i + 4];
    for &ch in &c[0..3] {
        assert!((120..=135).contains(&ch), "expected ~half-white, got {ch}");
    }
    assert_eq!(c[3], 255);
}

#[test]
fn transform_keyframe_midframe_affine_equals_transform_at() {
    let mut clip = full_canvas_clip("c0");
    clip.position_track = Some(KeyframeTrack::from_keyframes(vec![
        Keyframe::with_interpolation(0, AnimPair::new(0.1, 0.1), Interpolation::Linear),
        Keyframe::with_interpolation(10, AnimPair::new(0.5, 0.5), Interpolation::Linear),
    ]));
    clip.scale_track = Some(KeyframeTrack::from_keyframes(vec![
        Keyframe::with_interpolation(0, AnimPair::new(1.0, 1.0), Interpolation::Linear),
        Keyframe::with_interpolation(10, AnimPair::new(0.5, 0.5), Interpolation::Linear),
    ]));
    let tl = full_canvas_timeline_with(clip.clone());
    let plan = build_render_plan(&tl, RS, &Metrics);
    let fp = plan.frame(&tl, 5);
    assert_eq!(fp.draws.len(), 1);
    let expected = affine_transform(&clip.transform_at(5), (Q as f64, Q as f64), RS);
    approx_affine(fp.draws[0].affine, expected);
    let t0 = clip.transform_at(0);
    approx(t0.center_x, 0.6);
    approx(t0.center_y, 0.6);
    approx(t0.width, 1.0);
    let t10 = clip.transform_at(10);
    approx(t10.center_x, 0.75);
    approx(t10.center_y, 0.75);
    approx(t10.width, 0.5);
}

#[test]
fn crop_keyframe_uv_varies_across_frames() {
    let mut clip = full_canvas_clip("c0");
    clip.crop_track = Some(KeyframeTrack::from_keyframes(vec![
        Keyframe::with_interpolation(
            0,
            Crop {
                left: 0.0,
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            },
            Interpolation::Linear,
        ),
        Keyframe::with_interpolation(
            10,
            Crop {
                left: 0.25,
                top: 0.25,
                right: 0.25,
                bottom: 0.25,
            },
            Interpolation::Linear,
        ),
    ]));
    let tl = full_canvas_timeline_with(clip);
    let plan = build_render_plan(&tl, RS, &Metrics);
    let fp0 = plan.frame(&tl, 0);
    let fp10 = plan.frame(&tl, 10);
    assert_eq!(fp0.draws[0].crop_uv, (0.0, 0.0, 1.0, 1.0));
    let (u0, v0, u1, v1) = fp10.draws[0].crop_uv;
    approx(u0, 0.25);
    approx(v0, 0.25);
    approx(u1, 0.75);
    approx(v1, 0.75);
}

#[test]
fn text_overlay_visible_above_video() {
    let Some(dev) = device_or_skip("text_overlay_visible_above_video") else {
        return;
    };
    let mut tl = Timeline::new();
    tl.fps = 30;
    tl.width = Q as i32;
    tl.height = Q as i32;
    let mut video = full_canvas_clip("vid");
    video.media_type = ClipType::Video;
    let mut vt = Track::new("tv", ClipType::Video);
    vt.clips.push(video);
    tl.tracks.push(vt);
    let mut txt = Clip::new("txt", "", 0, 10);
    txt.media_type = ClipType::Text;
    txt.text_content = Some("Hi".to_string());
    let mut style = TextStyle {
        font_size: 400.0,
        ..TextStyle::default()
    };
    style.shadow.enabled = false;
    txt.text_style = Some(style);
    txt.transform = Transform::from_top_left(Point { x: 0.0, y: 0.0 }, 1.0, 1.0);
    let mut tt = Track::new("tt", ClipType::Text);
    tt.clips.push(txt);
    tl.tracks.push(tt);

    let plan = build_render_plan(&tl, RS, &Metrics);
    let fp = plan.frame(&tl, 0);
    assert_eq!(fp.draws.len(), 2);
    assert!(matches!(fp.draws[1].source, TextureSource::Text { .. }));

    let rasterizer = CosmicTextRasterizer::new();
    let compositor = Compositor::new(&dev.device);
    struct MixedResolver<'d> {
        device: &'d wgpu::Device,
        queue: &'d wgpu::Queue,
        rasterizer: &'d CosmicTextRasterizer,
        cache: TextureCache,
        content: String,
        style: TextStyle,
        vid_cached: Option<Rc<GpuTexture>>,
    }
    impl TextureResolver for MixedResolver<'_> {
        fn resolve(&mut self, source: &TextureSource, _f: i64) -> Option<Rc<GpuTexture>> {
            match source {
                TextureSource::Decoded { .. } | TextureSource::Image { .. } => {
                    if self.vid_cached.is_none() {
                        let f = make_solid([0, 0, 255, 255]);
                        self.vid_cached = Some(Rc::new(upload_rgba(
                            self.device,
                            self.queue,
                            &f,
                            false,
                            Some("v"),
                        )));
                    }
                    self.vid_cached.clone()
                }
                TextureSource::Text { clip_id } => {
                    let key = format!("t:{clip_id}");
                    if let Some(t) = self.cache.get(&key) {
                        return Some(t);
                    }
                    let req = TextRasterRequest {
                        clip_id,
                        content: &self.content,
                        style: &self.style,
                        box_norm: (0.0, 0.0, 1.0, 1.0),
                        canvas: (Q, Q),
                    };
                    let frame = self.rasterizer.rasterize(&req)?;
                    let tex = upload_rgba(self.device, self.queue, &frame, false, Some("text"));
                    Some(self.cache.insert(key, tex))
                }
                _ => None,
            }
        }
    }
    let mut resolver = MixedResolver {
        device: &dev.device,
        queue: &dev.queue,
        rasterizer: &rasterizer,
        cache: TextureCache::new(8),
        content: "Hi".to_string(),
        style: {
            let mut s = TextStyle {
                font_size: 400.0,
                ..TextStyle::default()
            };
            s.shadow.enabled = false;
            s
        },
        vid_cached: None,
    };
    let frame = compositor
        .render_to_rgba(&dev.device, &dev.queue, RS, &fp, &mut resolver)
        .expect("render");

    if rasterizer.has_fonts() {
        let any_text = frame.rgba.chunks_exact(4).any(|px| px[0] > 30 && px[1] > 30);
        assert!(any_text, "expected visible text pixels above blue video");
    } else {
        eprintln!("[note] no system fonts; skipped text-visible assertion");
    }
    assert_eq!(frame.width, Q);
    assert_eq!(frame.height, Q);
}

#[test]
fn ssim_identical_frames_score_near_one() {
    let Some(dev) = device_or_skip("ssim_identical_frames_score_near_one") else {
        return;
    };
    let tl = full_canvas_timeline_with(full_canvas_clip("c0"));
    let plan = build_render_plan(&tl, RS, &Metrics);
    let compositor = Compositor::new(&dev.device);
    let render_once = |resolver: &mut dyn TextureResolver| {
        let fp = plan.frame(&tl, 0);
        compositor
            .render_to_rgba(&dev.device, &dev.queue, RS, &fp, resolver)
            .expect("render")
    };
    let mut r1 = QuadrantResolver {
        device: &dev.device,
        queue: &dev.queue,
        cached: None,
    };
    let mut r2 = QuadrantResolver {
        device: &dev.device,
        queue: &dev.queue,
        cached: None,
    };
    let f1 = render_once(&mut r1);
    let f2 = render_once(&mut r2);
    let s = ssim(&f1.rgba, &f2.rgba, f1.width, f1.height);
    assert!(s > 0.98, "SSIM of identical frames {s:.4} <= 0.98");
    let solid = make_solid([0, 0, 0, 255]);
    let s2 = ssim(&f1.rgba, &solid.rgba, f1.width, f1.height);
    assert!(s2 < s, "SSIM vs solid {s2:.4} should be below identical {s:.4}");
}
