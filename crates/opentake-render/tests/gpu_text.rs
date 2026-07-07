//! GPU integration test for the text layer (#65): a timeline text clip flows
//! through `CosmicTextRasterizer` -> box texture -> compositor -> read-back, and
//! paints visible (non-black) pixels on top of the cleared canvas.
//!
//! Like the other GPU tests, this SKIPS gracefully when no GPU device is present
//! (CI / headless). The visible-pixel assertion additionally requires at least
//! one system font; without fonts the rasterizer yields no glyphs (still no
//! panic), so that assertion is guarded by `has_fonts()`.

use std::rc::Rc;

use opentake_domain::{
    Clip, ClipType, Point, TextAlignment, TextLayout, TextStyle, Timeline, Track, Transform,
};
use opentake_render::gpu::texture::upload_rgba;
use opentake_render::wgpu;
use opentake_render::{
    build_render_plan, Compositor, CosmicTextRasterizer, DecodedFrame, GpuTexture, RenderDevice,
    RenderSize, SourceMetrics, TextRasterRequest, TextRasterizer, TextureCache, TextureResolver,
    TextureSource,
};

const RS: RenderSize = RenderSize {
    width: 320,
    height: 120,
};

/// Text natural size comes from the clip box (the plan ignores metrics for text),
/// so this only needs to satisfy the trait.
struct Metrics;
impl SourceMetrics for Metrics {
    fn natural_size(&self, _r: &str) -> Option<(u32, u32)> {
        None
    }
}

/// Resolves a `TextureSource::Text` by rasterizing the one text clip in the test
/// (full-canvas box) — the same path `src-tauri/render.rs` uses in production.
struct TextResolver<'d> {
    device: &'d wgpu::Device,
    queue: &'d wgpu::Queue,
    rasterizer: &'d CosmicTextRasterizer,
    cache: TextureCache,
    content: String,
    style: TextStyle,
}

impl TextureResolver for TextResolver<'_> {
    fn resolve(&mut self, source: &TextureSource, _frame: i64) -> Option<Rc<GpuTexture>> {
        let TextureSource::Text { clip_id } = source else {
            return None;
        };
        let key = format!("t:{clip_id}");
        if let Some(tex) = self.cache.get(&key) {
            return Some(tex);
        }
        let req = TextRasterRequest {
            clip_id,
            content: &self.content,
            style: &self.style,
            box_norm: (0.0, 0.0, 1.0, 1.0),
            canvas: (RS.width, RS.height),
        };
        let frame = self.rasterizer.rasterize(&req)?;
        let tex = upload_rgba(self.device, self.queue, &frame, false, Some("text"));
        Some(self.cache.insert(key, tex))
    }
}

fn text_timeline() -> Timeline {
    let mut tl = Timeline::new();
    tl.fps = 30;
    tl.width = RS.width as i32;
    tl.height = RS.height as i32;
    let mut clip = Clip::new("txt", "", 0, 10);
    clip.media_type = ClipType::Text;
    clip.text_content = Some("Hi".to_string());
    // Large font so glyphs clearly paint at this canvas size; no shadow so the
    // black canvas stays black except where glyphs land.
    let mut style = TextStyle {
        font_size: 400.0,
        ..TextStyle::default()
    };
    style.shadow.enabled = false;
    clip.text_style = Some(style);
    clip.transform = Transform::from_top_left(Point { x: 0.0, y: 0.0 }, 1.0, 1.0);
    let mut track = Track::new("t0", ClipType::Text);
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

#[test]
fn text_clip_composites_visible_pixels() {
    let Some(dev) = device_or_skip("text_clip_composites_visible_pixels") else {
        return;
    };
    let tl = text_timeline();
    let plan = build_render_plan(&tl, RS, &Metrics);
    let fp = plan.frame(&tl, 0);
    // The text clip produces exactly one draw, and it is a Text source.
    assert_eq!(fp.draws.len(), 1);
    assert!(matches!(fp.draws[0].source, TextureSource::Text { .. }));

    let rasterizer = CosmicTextRasterizer::new();
    let compositor = Compositor::new(&dev.device);
    let mut resolver = TextResolver {
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
    };
    let frame = compositor
        .render_to_rgba(&dev.device, &dev.queue, RS, &fp, &mut resolver)
        .expect("render");

    // The canvas clears to opaque black; glyphs add color. With a font present,
    // at least one pixel must differ from black.
    if rasterizer.has_fonts() {
        let any_lit = frame
            .rgba
            .chunks_exact(4)
            .any(|px| px[0] > 8 || px[1] > 8 || px[2] > 8);
        assert!(
            any_lit,
            "expected visible text pixels on the composited frame"
        );
    } else {
        eprintln!("[note] no system fonts; skipped visible-pixel assertion");
    }
    // Frame is the canvas size and opaque.
    assert_eq!(frame.width, RS.width);
    assert_eq!(frame.height, RS.height);
}

// ---------------------------------------------------------------------------
// Non-GPU rasterizer assertions (SPEC §4.2 / PR-8 痛点 4): these call
// `CosmicTextRasterizer` directly (no compositor / GPU device) and verify the
// font-metric / shadow / alignment / wrapping behavior that must match upstream
// `CATextLayer` (TextLayerController.applyStyle). All gracefully skip when no
// system font is available (headless CI) — the trait never panics either way.
// ---------------------------------------------------------------------------

/// Count pixels with non-zero alpha (glyph + shadow + background footprint).
fn lit_count(frame: &DecodedFrame) -> u32 {
    frame.rgba.chunks_exact(4).filter(|px| px[3] > 0).count() as u32
}

/// X centroid (mean x) of lit pixels, or 0.0 if none. Used to assert that
/// horizontal alignment actually shifts the glyph run inside the box.
fn x_centroid(frame: &DecodedFrame) -> f64 {
    let mut sum = 0u64;
    let mut n = 0u64;
    for y in 0..frame.height {
        for x in 0..frame.width {
            if frame.rgba[((y * frame.width + x) * 4 + 3) as usize] > 0 {
                sum += x as u64;
                n += 1;
            }
        }
    }
    if n == 0 {
        0.0
    } else {
        sum as f64 / n as f64
    }
}

/// Vertical span (first..=last lit row, inclusive) of lit pixels.
fn y_span(frame: &DecodedFrame) -> u32 {
    let mut y0 = frame.height;
    let mut y1 = 0u32;
    for y in 0..frame.height {
        for x in 0..frame.width {
            if frame.rgba[((y * frame.width + x) * 4 + 3) as usize] > 0 {
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    if y1 < y0 {
        0
    } else {
        y1 - y0 + 1
    }
}

#[test]
fn font_size_scales_with_canvas_height() {
    // Upstream basis: scale = canvasHeight / 1080 (referenceCanvasHeight, L150/L155).
    // Same font_size + box_norm, taller canvas -> larger glyphs -> more lit pixels.
    let r = CosmicTextRasterizer::new();
    if !r.has_fonts() {
        eprintln!("[skip] no system fonts");
        return;
    }
    let mut style = TextStyle {
        font_size: 96.0,
        ..TextStyle::default()
    };
    style.shadow.enabled = false; // isolate glyph area from shadow bleed
    let mk = |canvas: (u32, u32)| {
        let f = r
            .rasterize(&TextRasterRequest {
                clip_id: "t",
                content: "Hello",
                style: &style,
                box_norm: (0.0, 0.0, 1.0, 1.0),
                canvas,
            })
            .expect("frame");
        lit_count(&f)
    };
    let n540 = mk((960, 540)); // scale 0.5
    let n1080 = mk((1920, 1080)); // scale 1.0
    let n2160 = mk((3840, 2160)); // scale 2.0
    assert!(
        n540 < n1080,
        "540p ({n540}) must paint fewer pixels than 1080p ({n1080})"
    );
    assert!(
        n1080 < n2160,
        "1080p ({n1080}) must paint fewer pixels than 2160p ({n2160})"
    );
}

#[test]
fn shadow_paints_pixels_outside_glyph_footprint() {
    // Upstream `layer.shadowRadius = blur * scale` (L183) — enabling the shadow
    // must add lit pixels beyond the glyph footprint (offset + blur spread).
    let r = CosmicTextRasterizer::new();
    if !r.has_fonts() {
        eprintln!("[skip] no system fonts");
        return;
    }
    let canvas = (640, 360);
    let mut style = TextStyle {
        font_size: 120.0,
        ..TextStyle::default()
    };
    style.shadow.enabled = false;
    let no_shadow = r
        .rasterize(&TextRasterRequest {
            clip_id: "t",
            content: "Hi",
            style: &style,
            box_norm: (0.0, 0.0, 1.0, 1.0),
            canvas,
        })
        .expect("frame");
    let n_no = lit_count(&no_shadow);
    style.shadow.enabled = true;
    style.shadow.blur = 8.0;
    let with_shadow = r
        .rasterize(&TextRasterRequest {
            clip_id: "t",
            content: "Hi",
            style: &style,
            box_norm: (0.0, 0.0, 1.0, 1.0),
            canvas,
        })
        .expect("frame");
    let n_yes = lit_count(&with_shadow);
    assert!(
        n_yes > n_no,
        "shadow ({n_yes}) must add lit pixels beyond glyphs ({n_no})"
    );
}

#[test]
fn alignment_shifts_glyph_x_centroid() {
    // Upstream `layer.alignmentMode` (L170): left/center/right must move the
    // glyph run's x centroid monotonically left -> center -> right.
    let r = CosmicTextRasterizer::new();
    if !r.has_fonts() {
        eprintln!("[skip] no system fonts");
        return;
    }
    let canvas = (800, 200);
    let centroid = |align: TextAlignment| -> f64 {
        let mut s = TextStyle {
            font_size: 80.0,
            ..TextStyle::default()
        };
        s.alignment = align;
        s.shadow.enabled = false;
        let f = r
            .rasterize(&TextRasterRequest {
                clip_id: "t",
                content: "Hi",
                style: &s,
                box_norm: (0.0, 0.0, 1.0, 1.0),
                canvas,
            })
            .expect("frame");
        x_centroid(&f)
    };
    let left = centroid(TextAlignment::Left);
    let center = centroid(TextAlignment::Center);
    let right = centroid(TextAlignment::Right);
    assert!(left < center, "left ({left}) < center ({center})");
    assert!(center < right, "center ({center}) < right ({right})");
}

#[test]
fn long_text_wraps_in_narrow_box() {
    // Upstream uses `.byWordWrapping` (TextStyle L133); cosmic-text wraps the same
    // way. A long string in a narrow box must span more rows than a single word.
    let r = CosmicTextRasterizer::new();
    if !r.has_fonts() {
        eprintln!("[skip] no system fonts");
        return;
    }
    let canvas = (400, 400);
    let mut style = TextStyle {
        font_size: 60.0,
        ..TextStyle::default()
    };
    style.shadow.enabled = false;
    let long = r
        .rasterize(&TextRasterRequest {
            clip_id: "t",
            content: "The quick brown fox jumps over the lazy dog",
            style: &style,
            box_norm: (0.0, 0.0, 1.0, 1.0),
            canvas,
        })
        .expect("frame");
    let short = r
        .rasterize(&TextRasterRequest {
            clip_id: "t",
            content: "Hi",
            style: &style,
            box_norm: (0.0, 0.0, 1.0, 1.0),
            canvas,
        })
        .expect("frame");
    let long_span = y_span(&long);
    let short_span = y_span(&short);
    assert!(
        long_span > short_span,
        "wrapped long text y-span ({long_span}) > single line ({short_span})"
    );
}

#[test]
fn rasterize_is_deterministic_ssim_one() {
    // SSIM self-consistency: same input -> same output. Identical bytes is a
    // strictly stronger claim than SSIM = 1.0 (identical images trivially score
    // 1.0 under any SSIM variant), so we assert byte equality directly.
    let r = CosmicTextRasterizer::new();
    if !r.has_fonts() {
        eprintln!("[skip] no system fonts");
        return;
    }
    let style = TextStyle::default();
    let req = TextRasterRequest {
        clip_id: "t",
        content: "Hello World",
        style: &style,
        box_norm: (0.0, 0.0, 1.0, 1.0),
        canvas: (640, 360),
    };
    let a = r.rasterize(&req).expect("frame");
    let b = r.rasterize(&req).expect("frame");
    assert_eq!((a.width, a.height), (b.width, b.height));
    assert_eq!(
        a.rgba, b.rgba,
        "rasterize must be deterministic (SSIM = 1.0)"
    );
}

#[test]
fn natural_size_shadow_padding_matches_upstream() {
    // TextLayout.shadowPadding = 12 (upstream L6); enabled shadow adds 12*2 = 24px
    // to the measured width (upstream TextLayout L28). The +4 slack is always
    // present. This is the box *placement* metric — the rasterizer's box comes
    // from `clip.transform` (see text_engine.rs `box_pixels`), not this value,
    // but the constant must still match upstream for placement parity.
    let on = TextStyle::default(); // shadow enabled
    let mut off = on.clone();
    off.shadow.enabled = false;
    let (w_on, _) = TextLayout::natural_size("Hi", &on, 10000.0, 1080.0);
    let (w_off, _) = TextLayout::natural_size("Hi", &off, 10000.0, 1080.0);
    assert_eq!(w_on - w_off, TextLayout::SHADOW_PADDING * 2.0);
    assert_eq!(TextLayout::SHADOW_PADDING, 12.0);
    assert_eq!(TextLayout::REFERENCE_CANVAS_HEIGHT, 1080.0);
}
