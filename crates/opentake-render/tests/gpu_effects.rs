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
    effect_registry, ChromaKey, Clip, ClipType, ColorGrade, Effect, EffectValidationError,
    HslSecondary, LiftGammaGain, Mask, MaskShape, MaskTransform, Point, Point2, Rgb, Timeline,
    Track, Transform,
};
use opentake_render::gpu::texture::upload_rgba;
use opentake_render::source::DecodedFrame;
use opentake_render::wgpu;
use opentake_render::{
    build_render_plan, Compositor, GpuTexture, RenderDevice, RenderError, RenderSize,
    SourceMetrics, TextureResolver, TextureSource,
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

/// Four equal-width chart bars: red, orange, green and blue. This lets the HSL
/// qualifier test an in-range hue, its feather boundary and two isolated hues
/// in one real compositor submission.
struct ColorChartResolver<'d> {
    device: &'d wgpu::Device,
    queue: &'d wgpu::Queue,
    cached: Option<Rc<GpuTexture>>,
}

impl TextureResolver for ColorChartResolver<'_> {
    fn resolve(&mut self, _source: &TextureSource, _frame: i64) -> Option<Rc<GpuTexture>> {
        if self.cached.is_none() {
            let colors = [
                [255, 0, 0, 255],
                [255, 200, 0, 255],
                [0, 255, 0, 255],
                [0, 0, 255, 255],
            ];
            let mut buf = vec![0u8; 16 * 16 * 4];
            for y in 0..16 {
                for x in 0..16 {
                    let i = (y * 16 + x) * 4;
                    buf[i..i + 4].copy_from_slice(&colors[x / 4]);
                }
            }
            let frame = DecodedFrame::new(16, 16, buf, true);
            let tex = upload_rgba(self.device, self.queue, &frame, false, Some("hsl-chart"));
            self.cached = Some(Rc::new(tex));
        }
        self.cached.clone()
    }
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

fn render_color_chart(dev: &RenderDevice, tl: &Timeline) -> DecodedFrame {
    let plan = build_render_plan(tl, RS, &Metrics);
    let fp = plan.frame(tl, 0);
    let compositor = Compositor::new(&dev.device);
    let mut resolver = ColorChartResolver {
        device: &dev.device,
        queue: &dev.queue,
        cached: None,
    };
    compositor
        .render_to_rgba(&dev.device, &dev.queue, RS, &fp, &mut resolver)
        .expect("render color chart")
}

#[test]
fn advertised_effect_registry_has_preview_export_golden_fixtures() {
    let Some(dev) = device_or_skip("advertised_effect_registry_has_preview_export_golden_fixtures")
    else {
        return;
    };

    let registry = effect_registry();
    assert_eq!(
        registry
            .iter()
            .map(|effect| effect.name)
            .collect::<Vec<_>>(),
        ["grayscale", "sepia", "invert"]
    );

    // Golden center pixels for a fixed opaque source. Each advertised effect is
    // exercised at its persisted default and a non-default amount. Preview and
    // export deliberately render through fresh resolver state and must agree
    // byte-for-byte.
    let fixtures = [
        ("grayscale", None, [81, 81, 81, 255]),
        ("grayscale", Some(0.4), [164, 50, 140, 255]),
        ("sepia", None, [144, 128, 99, 255]),
        ("sepia", Some(0.4), [189, 69, 148, 255]),
        ("invert", None, [35, 225, 75, 255]),
        ("invert", Some(0.4), [146, 108, 138, 255]),
    ];
    for (name, amount, golden) in fixtures {
        let mut timeline = full_canvas_timeline();
        let effect = amount.map_or_else(
            || Effect::new(name),
            |value| Effect::new(name).with_param("amount", value),
        );
        effect.validate().expect("advertised effect validates");
        timeline.tracks[0].clips[0].effects = vec![effect];

        let preview = render(&dev, &timeline, [220, 30, 180, 255]);
        let export = render(&dev, &timeline, [220, 30, 180, 255]);
        assert_eq!(preview.rgba, export.rgba, "preview/export drift for {name}");
        let actual = center_pixel(&preview);
        for channel in 0..4 {
            assert!(
                (actual[channel] as i32 - golden[channel]).abs() <= 3,
                "{name} amount={amount:?}: expected {golden:?}, got {actual:?}"
            );
        }
    }

    // The sequence itself is authored state: changing order must change pixels.
    let mut first = full_canvas_timeline();
    first.tracks[0].clips[0].effects = vec![Effect::new("sepia"), Effect::new("invert")];
    let mut second = full_canvas_timeline();
    second.tracks[0].clips[0].effects = vec![Effect::new("invert"), Effect::new("sepia")];
    assert_ne!(
        center_pixel(&render(&dev, &first, [220, 30, 180, 255])),
        center_pixel(&render(&dev, &second, [220, 30, 180, 255])),
        "effect order must be rendered, not stored as inert metadata"
    );

    // Disabled registered effects remain persisted but are skipped by the
    // render chain in both preview and export.
    let source = [220, 30, 180, 255];
    let mut disabled = full_canvas_timeline();
    disabled.tracks[0].clips[0].effects = vec![Effect {
        enabled: false,
        ..Effect::new("invert")
    }];
    let baseline = render(&dev, &full_canvas_timeline(), source);
    let disabled_preview = render(&dev, &disabled, source);
    let disabled_export = render(&dev, &disabled, source);
    assert_eq!(disabled_preview.rgba, baseline.rgba);
    assert_eq!(disabled_export.rgba, baseline.rgba);

    let mut invalid = full_canvas_timeline();
    invalid.tracks[0].clips[0].effects = vec![Effect::new("unadvertised")];
    let plan = build_render_plan(&invalid, RS, &Metrics);
    let frame_plan = plan.frame(&invalid, 0);
    let compositor = Compositor::new(&dev.device);
    let mut resolver = SolidResolver {
        device: &dev.device,
        queue: &dev.queue,
        rgba: [220, 30, 180, 255],
        cached: None,
    };
    let error = compositor
        .render_to_rgba(&dev.device, &dev.queue, RS, &frame_plan, &mut resolver)
        .expect_err("unknown effects must fail instead of rendering unchanged");
    assert!(matches!(
        error,
        RenderError::InvalidEffect(EffectValidationError::UnknownEffect { ref name })
            if name == "unadvertised"
    ));
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
fn lift_gamma_gain_matches_cpu_reference() {
    let grade = ColorGrade {
        lift_gamma_gain: LiftGammaGain {
            lift: Rgb::new(0.08, -0.03, 0.12),
            gamma: Rgb::new(1.8, 0.75, 1.25),
            gain: Rgb::new(0.82, 1.15, 0.93),
        },
        ..Default::default()
    };
    let source = [96_u8, 128, 192, 255];
    let linear = source[..3]
        .iter()
        .map(|channel| opentake_render::gpu::srgb_to_linear(f64::from(*channel) / 255.0))
        .collect::<Vec<_>>();

    let source_formula = |x: f64, lift: f64, gamma: f64, gain: f64| {
        gain * (x + lift * (1.0 - x)).max(0.0).powf(1.0 / gamma)
    };
    let expected_linear = [
        source_formula(linear[0], 0.08, 1.8, 0.82),
        source_formula(linear[1], -0.03, 0.75, 1.15),
        source_formula(linear[2], 0.12, 1.25, 0.93),
    ];
    let cpu = grade.apply_linear(linear[0], linear[1], linear[2]);
    for (actual, expected) in [cpu.0, cpu.1, cpu.2].into_iter().zip(expected_linear) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "CPU color-wheel reference drift: expected {expected}, got {actual}"
        );
    }

    let Some(dev) = device_or_skip("lift_gamma_gain_matches_cpu_reference") else {
        return;
    };
    let mut timeline = full_canvas_timeline();
    timeline.tracks[0].clips[0].color_grade = Some(grade);
    let preview = render(&dev, &timeline, source);
    let export = render(&dev, &timeline, source);
    assert_eq!(preview.rgba, export.rgba, "preview/export LGG drift");

    let expected = expected_linear.map(|channel| {
        (opentake_render::gpu::linear_to_srgb(channel.clamp(0.0, 1.0)) * 255.0).round() as u8
    });
    let actual = center_pixel(&preview);
    for channel in 0..3 {
        assert!(
            (i16::from(actual[channel]) - i16::from(expected[channel])).abs() <= 2,
            "GPU LGG channel {channel}: expected {expected:?}, got {actual:?}"
        );
    }
    assert_eq!(actual[3], 255);

    // A malformed persisted grade is rejected before source resolution or any
    // GPU submission, so preview/export cannot silently diverge or render an
    // unchanged frame.
    let mut invalid = full_canvas_timeline();
    invalid.tracks[0].clips[0].color_grade = Some(ColorGrade {
        lift_gamma_gain: LiftGammaGain {
            gamma: Rgb::new(0.0, 1.0, 1.0),
            ..Default::default()
        },
        ..Default::default()
    });
    let invalid_plan = build_render_plan(&invalid, RS, &Metrics);
    let invalid_frame = invalid_plan.frame(&invalid, 0);
    let compositor = Compositor::new(&dev.device);
    let mut resolver = SolidResolver {
        device: &dev.device,
        queue: &dev.queue,
        rgba: source,
        cached: None,
    };
    let error = compositor
        .render_to_rgba(&dev.device, &dev.queue, RS, &invalid_frame, &mut resolver)
        .expect_err("zero gamma must be rejected before source resolution");
    assert!(matches!(
        error,
        RenderError::InvalidColorGrade(ref invalid)
            if invalid.to_string()
                == "liftGammaGain.gamma.r must be finite and within (0, 4]"
    ));
    assert!(
        resolver.cached.is_none(),
        "invalid grade resolved source data"
    );
}

#[test]
fn hsl_secondary_hue_boundary_feather_and_isolation() {
    let grade = ColorGrade {
        hsl_secondary: Some(HslSecondary {
            hue_center: 0.0,
            hue_width: 0.24,
            feather: 0.08,
            hue_shift: 0.20,
            saturation: -0.25,
            lightness: 0.10,
        }),
        ..Default::default()
    };
    grade.validate().expect("bounded HSL secondary validates");

    // Persisted authored state must survive a save/reopen boundary exactly.
    let json = serde_json::to_string(&grade).expect("serialize HSL secondary");
    let reopened: ColorGrade = serde_json::from_str(&json).expect("reopen HSL secondary");
    assert_eq!(reopened, grade);

    let Some(dev) = device_or_skip("hsl_secondary_hue_boundary_feather_and_isolation") else {
        return;
    };
    let plain = render_color_chart(&dev, &full_canvas_timeline());
    let mut timeline = full_canvas_timeline();
    timeline.tracks[0].clips[0].color_grade = Some(grade);
    let preview = render_color_chart(&dev, &timeline);
    let export = render_color_chart(&dev, &timeline);
    assert_eq!(preview.rgba, export.rgba, "preview/export HSL drift");

    let delta = |x: u32| {
        let before = pixel_at(&plain, x, 8);
        let after = pixel_at(&preview, x, 8);
        before[..3]
            .iter()
            .zip(&after[..3])
            .map(|(a, b)| (i16::from(*a) - i16::from(*b)).unsigned_abs())
            .max()
            .unwrap()
    };
    let selected_red = delta(2);
    let feathered_orange = delta(6);
    let isolated_green = delta(10);
    let isolated_blue = delta(14);
    assert!(
        selected_red > 20,
        "selected red did not change: {selected_red}"
    );
    assert!(
        feathered_orange > 2 && feathered_orange < selected_red,
        "orange must receive only the feathered adjustment: red={selected_red}, orange={feathered_orange}"
    );
    assert!(isolated_green <= 2, "green leaked by {isolated_green}");
    assert!(isolated_blue <= 2, "blue leaked by {isolated_blue}");
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
        ..Mask::default()
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
        ..Mask::default()
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

#[test]
fn linear_circle_and_polygon_masks_match_cpu_reference_in_preview_and_export() {
    let Some(dev) =
        device_or_skip("linear_circle_and_polygon_masks_match_cpu_reference_in_preview_and_export")
    else {
        return;
    };
    let shapes = [
        MaskShape::Linear {
            point: Point2::new(0.45, 0.55),
            normal: Point2::new(0.8, -0.3),
        },
        MaskShape::Circle {
            center: Point2::new(0.55, 0.45),
            radius: Point2::new(0.31, 0.22),
        },
        MaskShape::Poly {
            points: vec![
                Point2::new(0.2, 0.2),
                Point2::new(0.82, 0.28),
                Point2::new(0.68, 0.82),
                Point2::new(0.28, 0.72),
            ],
        },
    ];

    for shape in shapes {
        for feather in [0.0, 0.18] {
            let mask = Mask {
                shape: shape.clone(),
                feather,
                invert: feather > 0.0,
                transform: if matches!(&shape, MaskShape::Poly { .. }) {
                    MaskTransform {
                        offset: Point2::new(0.07, -0.04),
                        scale: Point2::new(0.82, 1.13),
                        rotation_degrees: 17.0,
                    }
                } else {
                    MaskTransform::default()
                },
            };
            let mut timeline = full_canvas_timeline();
            timeline.tracks[0].clips[0].masks = vec![mask.clone()];

            // Paused preview and export both consume the same FramePlan and
            // compositor boundary. Render twice with fresh resolvers to guard
            // against path-local state and compare both against the CPU mirror.
            let preview = render(&dev, &timeline, [255, 255, 255, 255]);
            let export = render(&dev, &timeline, [255, 255, 255, 255]);
            assert_eq!(preview.rgba, export.rgba);

            for y in 0..RS.height {
                for x in 0..RS.width {
                    let expected = (mask.coverage(
                        (x as f64 + 0.5) / RS.width as f64,
                        (y as f64 + 0.5) / RS.height as f64,
                    ) * 255.0)
                        .round() as i32;
                    let actual = pixel_at(&preview, x, y)[0] as i32;
                    assert!(
                        (actual - expected).abs() <= 3,
                        "shape={shape:?} feather={feather} pixel=({x},{y}) expected={expected} actual={actual}"
                    );
                }
            }
        }
    }

    // Multiple masks intersect in authored order. Compare the real GPU result
    // against the product of both CPU coverage functions at every pixel.
    let masks = vec![
        Mask {
            shape: MaskShape::Circle {
                center: Point2::new(0.38, 0.5),
                radius: Point2::new(0.34, 0.3),
            },
            feather: 0.08,
            ..Mask::default()
        },
        Mask {
            shape: MaskShape::Circle {
                center: Point2::new(0.62, 0.5),
                radius: Point2::new(0.34, 0.3),
            },
            feather: 0.08,
            ..Mask::default()
        },
    ];
    let mut timeline = full_canvas_timeline();
    timeline.tracks[0].clips[0].masks = masks.clone();
    let preview = render(&dev, &timeline, [255, 255, 255, 255]);
    let export = render(&dev, &timeline, [255, 255, 255, 255]);
    assert_eq!(preview.rgba, export.rgba);
    for y in 0..RS.height {
        for x in 0..RS.width {
            let px = (x as f64 + 0.5) / RS.width as f64;
            let py = (y as f64 + 0.5) / RS.height as f64;
            let expected = (masks
                .iter()
                .map(|mask| mask.coverage(px, py))
                .product::<f64>()
                * 255.0)
                .round() as i32;
            let actual = pixel_at(&preview, x, y)[0] as i32;
            assert!(
                (actual - expected).abs() <= 3,
                "multiple masks pixel=({x},{y}) expected={expected} actual={actual}"
            );
        }
    }
}
