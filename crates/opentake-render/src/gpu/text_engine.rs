//! cosmic-text backed [`TextRasterizer`] (SPEC §4.2). Upstream draws text via
//! `CATextLayer`; we rasterize each text clip's box to a premultiplied-RGBA
//! [`DecodedFrame`] that the compositor places like any other layer.
//!
//! The texture covers the clip's **box** (not the whole canvas): the plan sets a
//! text layer's `nat_size` to the box pixel size, so the existing affine maps the
//! box texture 1:1 onto the canvas at the clip's transform (position / rotation /
//! flip / opacity all handled by the compositor, exactly as for video/image).
//!
//! Style coverage: font family + weight + style, size (canvas-relative like
//! upstream), color, horizontal alignment, optional background fill, drop shadow
//! (offset + box blur), and border stroke — the `TextStyle` fields. Glyph layout
//! + word-wrap + font fallback come from cosmic-text; raster from swash.
//!
//! Style coverage limits / future work (SPEC §4.2, ROADMAP Phase 8):
//! - **Rich text** (multi-span styles): cosmic-text `Buffer::set_rich_text` takes
//!   an iterator of `(text, Attrs)` spans; the current path uses `set_text`
//!   (single style per clip) — switching needs a `TextRasterRequest` shape change.
//! - **Emoji / CJK selection hints**: cosmic-text 0.12 `Attrs` has no
//!   `family_emoji` / `family_asian` (added in 0.14+), so fallback uses its
//!   script-aware fontdb search rather than an explicit preferred family.
//! - **Vertical text**: unsupported (upstream doesn't expose it via `TextStyle`);
//!   v1 leaves it horizontal.

use std::cell::RefCell;

use cosmic_text::{
    fontdb, Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache,
    Weight,
};
use opentake_domain::{Rgba, TextAlignment};

use crate::gpu::text_raster::{TextRasterRequest, TextRasterizer};
use crate::source::DecodedFrame;

/// Upstream font-size basis: sizes are authored against a 1080p-tall canvas and
/// scaled by the actual canvas height (`TextLayerController` / `TextLayout`).
const CANVAS_BASIS_HEIGHT: f64 = 1080.0;

/// Largest text box we will rasterize (px per side). Bounds the CPU/RAM cost of a
/// degenerate transform; real text boxes are well under this.
const MAX_BOX_SIDE: u32 = 8192;

/// cosmic-text rasterizer. Owns a `FontSystem` (system fonts discovered once) and
/// a `SwashCache`, both mutated during layout/raster, so they sit behind a
/// `RefCell` to keep the trait's `&self` rasterize call.
pub struct CosmicTextRasterizer {
    inner: RefCell<Inner>,
}

struct Inner {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl CosmicTextRasterizer {
    /// Build a rasterizer, discovering installed system fonts. This scans font
    /// directories once and is mildly expensive (~tens of ms); construct it once
    /// and reuse.
    pub fn new() -> Self {
        Self::from_font_system(FontSystem::new())
    }

    /// Build the deterministic no-font fallback used by headless runtimes and
    /// tests. Text requests still yield a correctly sized premultiplied frame;
    /// glyph coverage is empty while background and border styles remain live.
    pub fn without_system_fonts() -> Self {
        Self::from_font_system(FontSystem::new_with_locale_and_db(
            "en-US".to_string(),
            fontdb::Database::new(),
        ))
    }

    fn from_font_system(font_system: FontSystem) -> Self {
        CosmicTextRasterizer {
            inner: RefCell::new(Inner {
                font_system,
                swash_cache: SwashCache::new(),
            }),
        }
    }

    /// Whether any font face is available. Text cannot raster to visible pixels
    /// without one (headless CI may have none); callers/tests can branch on this.
    pub fn has_fonts(&self) -> bool {
        self.inner
            .borrow()
            .font_system
            .db()
            .faces()
            .next()
            .is_some()
    }
}

impl Default for CosmicTextRasterizer {
    fn default() -> Self {
        CosmicTextRasterizer::new()
    }
}

impl TextRasterizer for CosmicTextRasterizer {
    fn rasterize(&self, req: &TextRasterRequest<'_>) -> Option<DecodedFrame> {
        let inner = &mut *self.inner.borrow_mut();
        rasterize_box(inner, req)
    }
}

/// Box pixel size from the normalized box + canvas, clamped to sane bounds.
/// The box comes from `clip.transform` (top-left + width/height) — i.e. upstream
/// `layer.frame = (tl.x*W, tl.y*H, transform.width*W, transform.height*H)` at
/// `TextLayerController.applyStyle` L157-163 — **not** `TextLayout.naturalSize`
/// (that measures glyph bounds for clip placement only; the shadow padding
/// `12*2` and `+4` slack live there, not in this rasterizer's box).
fn box_pixels(box_norm: (f64, f64, f64, f64), canvas: (u32, u32)) -> Option<(u32, u32)> {
    let (_, _, bw, bh) = box_norm;
    let w = (bw * canvas.0 as f64).round();
    let h = (bh * canvas.1 as f64).round();
    if !(w.is_finite() && h.is_finite()) || w < 1.0 || h < 1.0 {
        return None;
    }
    Some(((w as u32).min(MAX_BOX_SIDE), (h as u32).min(MAX_BOX_SIDE)))
}

/// Map the domain text alignment to cosmic-text's.
fn to_align(a: TextAlignment) -> Align {
    match a {
        TextAlignment::Left => Align::Left,
        TextAlignment::Center => Align::Center,
        TextAlignment::Right => Align::Right,
    }
}

/// Map the domain text style to cosmic-text `Attrs`. Upstream names are often
/// PostScript ("Helvetica-Bold"); `fontdb::Family::Name` matches by *Typographic
/// Family* and **rejects** suffixes like _Bold_/_Italic_, so we split the family
/// off the first `-` and infer weight + style from the suffix. fontdb has no
/// PostScript-name lookup and no `Style::Specific`; this is the closest match
/// within the cosmic-text API — upstream `NSFont(name:size:)` is stricter, so a
/// PostScript name whose family+weight pair is absent from the font database
/// falls back to the nearest weight (matches upstream's `?? boldSystemFont`
/// fallback when `NSFont(name:)` returns nil).
fn attrs_for<'a>(font_name: &'a str) -> Attrs<'a> {
    let raw = font_name.trim();
    let lower = raw.to_ascii_lowercase();
    let weight = if lower.contains("black") || lower.contains("heavy") {
        Weight::BLACK
    } else if lower.contains("extrabold") {
        Weight::EXTRA_BOLD
    } else if lower.contains("semibold") || lower.contains("demibold") {
        Weight::SEMIBOLD
    } else if lower.contains("medium") {
        Weight::MEDIUM
    } else if lower.contains("extralight") || lower.contains("ultralight") {
        Weight::EXTRA_LIGHT
    } else if lower.contains("light") {
        Weight::LIGHT
    } else if lower.contains("thin") {
        Weight::THIN
    } else if lower.contains("bold") {
        Weight::BOLD
    } else {
        Weight::NORMAL
    };
    let style = if lower.contains("oblique") {
        Style::Oblique
    } else if lower.contains("italic") {
        Style::Italic
    } else {
        Style::Normal
    };
    let base = raw.split('-').next().unwrap_or(raw).trim();
    let family = if base.is_empty() {
        Family::SansSerif
    } else {
        Family::Name(base)
    };
    Attrs::new().family(family).weight(weight).style(style)
}

/// 0..1 channel to 0..=255.
fn ch8(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Rasterize one text clip's box to premultiplied RGBA, or `None` for empty
/// content / a degenerate box.
fn rasterize_box(inner: &mut Inner, req: &TextRasterRequest<'_>) -> Option<DecodedFrame> {
    if req.content.trim().is_empty() {
        return None;
    }
    let (bw, bh) = box_pixels(req.box_norm, req.canvas)?;
    let style = req.style;

    let fs = &mut inner.font_system;
    let swash = &mut inner.swash_cache;

    // cosmic-text 0.12 assumes a default font exists and panics while shaping
    // an empty database. Preserve the public non-crashing headless contract by
    // rendering the non-glyph box styles directly when no face is available.
    if fs.db().faces().next().is_none() {
        let mut out = vec![0u8; (bw * bh * 4) as usize];
        if style.background.enabled {
            fill_rect(&mut out, bw, bh, 0, 0, bw, bh, style.background.color);
        }
        if style.border.enabled {
            let scale = req.canvas.1 as f64 / CANVAS_BASIS_HEIGHT;
            let thickness = (2.0 * scale).round().max(1.0) as u32;
            stroke_rect(&mut out, bw, bh, thickness, style.border.color);
        }
        return Some(DecodedFrame::new(bw, bh, out, true));
    }

    // Canvas-relative font size (upstream basis) — at least 1px.
    let font_px =
        (style.font_size * style.font_scale * (req.canvas.1 as f64 / CANVAS_BASIS_HEIGHT)).max(1.0);
    let line_height = (font_px * 1.2) as f32;
    let metrics = Metrics::new(font_px as f32, line_height);

    let mut buffer = Buffer::new(fs, metrics);
    buffer.set_size(fs, Some(bw as f32), Some(bh as f32));
    buffer.set_text(
        fs,
        req.content,
        attrs_for(&style.font_name),
        Shaping::Advanced,
    );
    let align = to_align(style.alignment);
    for line in buffer.lines.iter_mut() {
        line.set_align(Some(align));
    }
    buffer.shape_until_scroll(fs, false);

    // Coverage mask over the box: 0..=255 per pixel, max-combined so overlapping
    // glyph cells keep the strongest coverage. White default color → the closure
    // color's alpha IS the glyph coverage.
    let mut mask = vec![0u8; (bw * bh) as usize];
    buffer.draw(
        fs,
        swash,
        Color::rgba(255, 255, 255, 255),
        |x, y, w, h, color| {
            let cov = color.a();
            if cov == 0 {
                return;
            }
            for dy in 0..h as i32 {
                for dx in 0..w as i32 {
                    let px = x + dx;
                    let py = y + dy;
                    if px < 0 || py < 0 || px >= bw as i32 || py >= bh as i32 {
                        continue;
                    }
                    let idx = (py as u32 * bw + px as u32) as usize;
                    if cov > mask[idx] {
                        mask[idx] = cov;
                    }
                }
            }
        },
    );

    // Composite into a premultiplied-RGBA canvas: background → shadow → text →
    // border (bottom to top).
    let mut out = vec![0u8; (bw * bh * 4) as usize];

    if style.background.enabled {
        let c = style.background.color;
        fill_rect(&mut out, bw, bh, 0, 0, bw, bh, c);
    }

    if style.shadow.enabled {
        let scale = req.canvas.1 as f64 / CANVAS_BASIS_HEIGHT;
        // Upstream `layer.shadowRadius = max(0, blur * scale)` (L183); the box-blur
        // half-window below uses the same value as the box radius — 3 stacked box
        // passes approximate the Gaussian CoreImage applies for that radius.
        let radius = (style.shadow.blur * scale).round() as u32;
        let blurred = if radius > 0 {
            box_blur(&mask, bw, bh, radius)
        } else {
            mask.clone()
        };
        // Upstream `textRoot.isGeometryFlipped = true` flips Y to top-origin, so a
        // shadow `offset_y = -2` renders the shadow *below* the glyphs (CALayer
        // shadowOffset is in the layer's flipped space). cosmic-text is already
        // top-origin (image rows go down), so the equivalent is `dy = +2` → negate
        // the authored offset_y when translating to image rows.
        let dx = (style.shadow.offset_x * scale).round() as i32;
        let dy = -(style.shadow.offset_y * scale).round() as i32;
        composite_mask(&mut out, bw, bh, &blurred, dx, dy, style.shadow.color);
    }

    composite_mask(&mut out, bw, bh, &mask, 0, 0, style.color);

    if style.border.enabled {
        let scale = req.canvas.1 as f64 / CANVAS_BASIS_HEIGHT;
        let thickness = (2.0 * scale).round().max(1.0) as u32;
        stroke_rect(&mut out, bw, bh, thickness, style.border.color);
    }

    Some(DecodedFrame::new(bw, bh, out, true))
}

/// Alpha-over a straight-alpha source pixel onto a premultiplied-RGBA buffer.
fn over(out: &mut [u8], idx: usize, sr: u8, sg: u8, sb: u8, sa: u8) {
    if sa == 0 {
        return;
    }
    let sa_f = sa as f32 / 255.0;
    let inv = 1.0 - sa_f;
    // src premultiplied
    let spr = sr as f32 * sa_f;
    let spg = sg as f32 * sa_f;
    let spb = sb as f32 * sa_f;
    let o = idx * 4;
    out[o] = (spr + out[o] as f32 * inv).round().clamp(0.0, 255.0) as u8;
    out[o + 1] = (spg + out[o + 1] as f32 * inv).round().clamp(0.0, 255.0) as u8;
    out[o + 2] = (spb + out[o + 2] as f32 * inv).round().clamp(0.0, 255.0) as u8;
    out[o + 3] = (sa as f32 + out[o + 3] as f32 * inv)
        .round()
        .clamp(0.0, 255.0) as u8;
}

/// Composite a coverage mask in `color` onto the premultiplied buffer at offset
/// `(dx, dy)`. The effective alpha is `coverage * color.a`.
fn composite_mask(out: &mut [u8], w: u32, h: u32, mask: &[u8], dx: i32, dy: i32, color: Rgba) {
    let (cr, cg, cb) = (ch8(color.r), ch8(color.g), ch8(color.b));
    let ca = color.a.clamp(0.0, 1.0);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let cov = mask[(y as u32 * w + x as u32) as usize];
            if cov == 0 {
                continue;
            }
            let tx = x + dx;
            let ty = y + dy;
            if tx < 0 || ty < 0 || tx >= w as i32 || ty >= h as i32 {
                continue;
            }
            let sa = (cov as f64 * ca).round() as u8;
            over(out, (ty as u32 * w + tx as u32) as usize, cr, cg, cb, sa);
        }
    }
}

/// Fill an axis-aligned rect with a straight-alpha color (alpha-over).
#[allow(clippy::too_many_arguments)]
fn fill_rect(out: &mut [u8], w: u32, h: u32, x0: u32, y0: u32, rw: u32, rh: u32, color: Rgba) {
    let (cr, cg, cb, ca) = (ch8(color.r), ch8(color.g), ch8(color.b), ch8(color.a));
    for y in y0..(y0 + rh).min(h) {
        for x in x0..(x0 + rw).min(w) {
            over(out, (y * w + x) as usize, cr, cg, cb, ca);
        }
    }
}

/// Stroke the box perimeter with a straight-alpha color. Thickness is already
/// scaled from the upstream 2px-at-1080p basis by the caller.
fn stroke_rect(out: &mut [u8], w: u32, h: u32, thickness: u32, color: Rgba) {
    let t = thickness.min(w).min(h);
    fill_rect(out, w, h, 0, 0, w, t, color); // top
    fill_rect(out, w, h, 0, h.saturating_sub(t), w, t, color); // bottom
    fill_rect(out, w, h, 0, 0, t, h, color); // left
    fill_rect(out, w, h, w.saturating_sub(t), 0, t, h, color); // right
}

/// Separable box blur of a coverage mask (used for the drop shadow). Three
/// stacked passes per axis approximate a Gaussian — the standard "3-box ≈ 1
/// Gaussian" result (Wikipedia "Box blur"). Upstream's `CALayer.shadowRadius` is
/// a Gaussian blur with `radius = shadow.blur * scale`, so we feed that same
/// value in as the box half-window radius (no `/2`).
fn box_blur(mask: &[u8], w: u32, h: u32, radius: u32) -> Vec<u8> {
    if radius == 0 || mask.is_empty() {
        return mask.to_vec();
    }
    let mut cur = mask.to_vec();
    let mut tmp = vec![0u8; mask.len()];
    for _ in 0..3 {
        blur_axis(&cur, &mut tmp, w, h, radius, true);
        core::mem::swap(&mut cur, &mut tmp);
    }
    for _ in 0..3 {
        blur_axis(&cur, &mut tmp, w, h, radius, false);
        core::mem::swap(&mut cur, &mut tmp);
    }
    cur
}

/// One separable box-blur pass along a single axis (`horizontal` picks H vs V).
/// Sliding-window sum with edge clamping; window = `2*radius + 1`.
fn blur_axis(src: &[u8], dst: &mut [u8], w: u32, h: u32, radius: u32, horizontal: bool) {
    let r = radius as i32;
    let win = (2 * r + 1) as f64;
    let (wmax, hmax) = (w as i32, h as i32);
    for y in 0..hmax {
        for x in 0..wmax {
            let mut sum = 0u32;
            for k in -r..=r {
                let (sx, sy) = if horizontal {
                    ((x + k).clamp(0, wmax - 1), y)
                } else {
                    (x, (y + k).clamp(0, hmax - 1))
                };
                sum += src[(sy as u32 * w + sx as u32) as usize] as u32;
            }
            dst[(y as u32 * w + x as u32) as usize] = (sum as f64 / win) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::TextStyle;

    fn req<'a>(content: &'a str, style: &'a TextStyle) -> TextRasterRequest<'a> {
        TextRasterRequest {
            clip_id: "t0",
            content,
            style,
            box_norm: (0.1, 0.8, 0.8, 0.15),
            canvas: (1280, 720),
        }
    }

    #[test]
    fn empty_content_is_none() {
        let r = CosmicTextRasterizer::new();
        let style = TextStyle::default();
        assert!(r.rasterize(&req("   ", &style)).is_none());
    }

    #[test]
    fn degenerate_box_is_none() {
        let r = CosmicTextRasterizer::new();
        let style = TextStyle::default();
        let request = TextRasterRequest {
            clip_id: "t0",
            content: "hi",
            style: &style,
            box_norm: (0.0, 0.0, 0.0, 0.0),
            canvas: (1280, 720),
        };
        assert!(r.rasterize(&request).is_none());
    }

    #[test]
    fn renders_box_sized_premultiplied_frame() {
        let r = CosmicTextRasterizer::new();
        let style = TextStyle::default();
        let frame = r.rasterize(&req("Hello", &style)).expect("frame");
        // Box is 0.8*1280 x 0.15*720 = 1024 x 108.
        assert_eq!(frame.width, 1024);
        assert_eq!(frame.height, 108);
        assert!(frame.premultiplied);
        assert_eq!(frame.rgba.len(), (1024 * 108 * 4) as usize);
        // With fonts available, "Hello" must paint some non-transparent pixels.
        if r.has_fonts() {
            let painted = frame.rgba.as_chunks::<4>().0.iter().any(|px| px[3] > 0);
            assert!(
                painted,
                "expected visible text pixels when fonts are present"
            );
        }
    }

    #[test]
    fn background_fill_paints_even_without_glyphs() {
        let r = CosmicTextRasterizer::new();
        let style = TextStyle {
            background: opentake_domain::Fill::new(true, Rgba::new(0.0, 0.0, 0.0, 1.0)),
            ..TextStyle::default()
        };
        let frame = r.rasterize(&req("Hi", &style)).expect("frame");
        // Background is opaque, so every pixel has full alpha regardless of fonts.
        let opaque = frame.rgba.as_chunks::<4>().0.iter().all(|px| px[3] == 255);
        assert!(opaque, "opaque background should fill the whole box");
    }

    #[test]
    fn over_blends_premultiplied() {
        let mut buf = vec![0u8; 4];
        // Opaque red over transparent → premultiplied opaque red.
        over(&mut buf, 0, 255, 0, 0, 255);
        assert_eq!(buf, vec![255, 0, 0, 255]);
        // Half-alpha white over opaque red → premult blend.
        let mut buf2 = vec![255u8, 0, 0, 255];
        over(&mut buf2, 0, 255, 255, 255, 128);
        // src_premult白≈128, dst*(1-0.502)≈127 → ~255 on R; alpha stays 255.
        assert!(buf2[0] > 200 && buf2[3] == 255, "{buf2:?}");
    }
}
