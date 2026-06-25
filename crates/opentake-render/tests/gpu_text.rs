//! GPU integration test for the text layer (#65): a timeline text clip flows
//! through `CosmicTextRasterizer` -> box texture -> compositor -> read-back, and
//! paints visible (non-black) pixels on top of the cleared canvas.
//!
//! Like the other GPU tests, this SKIPS gracefully when no GPU device is present
//! (CI / headless). The visible-pixel assertion additionally requires at least
//! one system font; without fonts the rasterizer yields no glyphs (still no
//! panic), so that assertion is guarded by `has_fonts()`.

use std::rc::Rc;

use opentake_domain::{Clip, ClipType, Point, TextStyle, Timeline, Track, Transform};
use opentake_render::gpu::texture::upload_rgba;
use opentake_render::wgpu;
use opentake_render::{
    build_render_plan, Compositor, CosmicTextRasterizer, GpuTexture, RenderDevice, RenderSize,
    SourceMetrics, TextRasterRequest, TextRasterizer, TextureCache, TextureResolver, TextureSource,
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
