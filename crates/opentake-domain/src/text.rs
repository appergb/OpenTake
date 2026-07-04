//! Text style and layout. 1:1 port of `TextStyle.swift` (data + hex parsing)
//! and a platform-free approximation of `TextLayout.swift`.
//!
//! AppKit/CoreText helpers (`nsColor`, `swiftUIColor`, `resolvedFont`,
//! `paragraphStyle`, `attributes`, `caTextAlignmentMode`) are pure-UI and live in
//! the render/frontend layer. The numeric hex parser is ported verbatim.
//! `TextLayout::natural_size` is an APPROXIMATION: real glyph metrics require a
//! text engine (cosmic-text) in the render layer — see notes on that function.

use serde::{Deserialize, Serialize};

/// sRGB color with straight alpha. Defaults to opaque white, matching upstream.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct Rgba {
    #[serde(default = "one")]
    pub r: f64,
    #[serde(default = "one")]
    pub g: f64,
    #[serde(default = "one")]
    pub b: f64,
    #[serde(default = "one")]
    pub a: f64,
}

fn one() -> f64 {
    1.0
}

impl Default for Rgba {
    fn default() -> Self {
        Rgba {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }
    }
}

impl Rgba {
    pub fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Rgba { r, g, b, a }
    }

    /// Parse `#RGB`, `#RRGGBB`, or `#RRGGBBAA` (leading `#` optional). Returns
    /// `None` on any malformed input. 1:1 port of upstream `init?(hex:)`.
    pub fn from_hex(hex: &str) -> Option<Rgba> {
        let mut s = hex.trim();
        s = s.strip_prefix('#').unwrap_or(s);
        let chars: Vec<char> = s.chars().collect();

        // Parse `len` hex chars starting at `start` into a 0..=1 component.
        // For len==1 the nibble is duplicated (e.g. "f" -> "ff"), as upstream.
        let component = |start: usize, len: usize| -> Option<f64> {
            let slice: String = chars[start..start + len].iter().collect();
            let byte_str = if len == 1 {
                format!("{slice}{slice}")
            } else {
                slice
            };
            u8::from_str_radix(&byte_str, 16)
                .ok()
                .map(|n| n as f64 / 255.0)
        };

        match chars.len() {
            3 => Some(Rgba::new(
                component(0, 1)?,
                component(1, 1)?,
                component(2, 1)?,
                1.0,
            )),
            6 => Some(Rgba::new(
                component(0, 2)?,
                component(2, 2)?,
                component(4, 2)?,
                1.0,
            )),
            8 => Some(Rgba::new(
                component(0, 2)?,
                component(2, 2)?,
                component(4, 2)?,
                component(6, 2)?,
            )),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextAlignment {
    Left,
    /// Upstream default.
    #[default]
    Center,
    Right,
}

impl TextAlignment {
    pub const ALL: [TextAlignment; 3] = [
        TextAlignment::Left,
        TextAlignment::Center,
        TextAlignment::Right,
    ];
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shadow {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    /// Alpha doubles as opacity; the render layer keeps shadow opacity at 1.
    #[serde(default = "shadow_default_color")]
    pub color: Rgba,
    /// Canvas points; scaled at render time.
    #[serde(default)]
    pub offset_x: f64,
    #[serde(default = "minus_two")]
    pub offset_y: f64,
    #[serde(default = "six")]
    pub blur: f64,
}

fn bool_true() -> bool {
    true
}
fn minus_two() -> f64 {
    -2.0
}
fn six() -> f64 {
    6.0
}
fn shadow_default_color() -> Rgba {
    Rgba::new(0.0, 0.0, 0.0, 0.6)
}

impl Default for Shadow {
    fn default() -> Self {
        Shadow {
            enabled: true,
            color: Rgba::new(0.0, 0.0, 0.0, 0.6),
            offset_x: 0.0,
            offset_y: -2.0,
            blur: 6.0,
        }
    }
}

/// Toggleable solid color — used for the text box background and border.
/// Defaults to disabled with opaque white (matches upstream `Fill()`).
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Fill {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub color: Rgba,
}

impl Fill {
    pub fn new(enabled: bool, color: Rgba) -> Self {
        Fill { enabled, color }
    }
}

fn default_font_name() -> String {
    "Helvetica-Bold".to_string()
}
fn default_font_size() -> f64 {
    96.0
}
fn default_font_scale() -> f64 {
    1.0
}
fn default_background_fill() -> Fill {
    Fill::new(false, Rgba::new(0.0, 0.0, 0.0, 0.6))
}
fn default_border_fill() -> Fill {
    Fill::new(false, Rgba::new(0.0, 0.0, 0.0, 1.0))
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextStyle {
    #[serde(default = "default_font_name")]
    pub font_name: String,
    #[serde(default = "default_font_size")]
    pub font_size: f64,
    #[serde(default = "default_font_scale")]
    pub font_scale: f64,
    #[serde(default)]
    pub color: Rgba,
    #[serde(default)]
    pub alignment: TextAlignment,
    #[serde(default)]
    pub shadow: Shadow,
    #[serde(default = "default_background_fill")]
    pub background: Fill,
    #[serde(default = "default_border_fill")]
    pub border: Fill,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            font_name: "Helvetica-Bold".to_string(),
            font_size: 96.0,
            font_scale: 1.0,
            color: Rgba::default(),
            alignment: TextAlignment::Center,
            shadow: Shadow::default(),
            background: Fill::new(false, Rgba::new(0.0, 0.0, 0.0, 0.6)),
            border: Fill::new(false, Rgba::new(0.0, 0.0, 0.0, 1.0)),
        }
    }
}

/// Natural bounding size of a rendered text clip. 1:1 port of constants and the
/// canvas-scale basis from `TextLayout.swift`.
///
/// IMPORTANT: [`natural_size`](TextLayout::natural_size) is an APPROXIMATION.
/// Upstream measures real glyph runs via `NSAttributedString.boundingRect`
/// (CoreText). This crate is platform-free and zero-dependency, so it estimates
/// advance width with a fixed per-character factor and line height from the
/// render size. The canvas-scale basis (`canvas_height / 1080`), the shadow
/// padding (`12 * 2`), and the `+4` slack are reproduced exactly so the shape of
/// the formula matches; the *width* will differ from CoreText and must be
/// recomputed by the render-layer text engine (cosmic-text) for pixel parity.
pub struct TextLayout;

impl TextLayout {
    pub const SHADOW_PADDING: f64 = 12.0;
    pub const REFERENCE_CANVAS_HEIGHT: f64 = 1080.0;

    /// Approximate average glyph advance as a fraction of the render size. Used
    /// only by the platform-free approximation; the render layer overrides this
    /// with real metrics.
    const APPROX_ADVANCE_FACTOR: f64 = 0.6;
    /// Approximate line height as a fraction of the render size.
    const APPROX_LINE_HEIGHT_FACTOR: f64 = 1.2;

    /// Approximate natural size. See the type-level note: this is NOT pixel-exact
    /// with upstream CoreText measurement.
    pub fn natural_size(
        content: &str,
        style: &TextStyle,
        max_width: f64,
        canvas_height: f64,
    ) -> (f64, f64) {
        let measured = if content.is_empty() { " " } else { content };
        let canvas_scale = canvas_height / Self::REFERENCE_CANVAS_HEIGHT;
        let render_size = style.font_size * style.font_scale * canvas_scale;

        let advance = render_size * Self::APPROX_ADVANCE_FACTOR;
        let line_height = render_size * Self::APPROX_LINE_HEIGHT_FACTOR;

        // Greedy word wrap into `max_width`, approximating each line's width.
        let mut lines = 1usize;
        let mut widest = 0.0f64;
        let mut current = 0.0f64;
        for word in measured.split_whitespace() {
            let word_w = word.chars().count() as f64 * advance;
            let space_w = if current > 0.0 { advance } else { 0.0 };
            if current > 0.0 && current + space_w + word_w > max_width {
                widest = widest.max(current);
                current = word_w;
                lines += 1;
            } else {
                current += space_w + word_w;
            }
        }
        widest = widest.max(current);
        // Single token with no spaces still has a width.
        if widest == 0.0 {
            widest = measured.chars().count() as f64 * advance;
        }

        let bounding_w = widest;
        let bounding_h = line_height * lines as f64;

        let slack = 4.0;
        let shadow_pad = if style.shadow.enabled {
            Self::SHADOW_PADDING * 2.0
        } else {
            0.0
        };
        (
            (bounding_w.ceil() + shadow_pad + slack).max(1.0),
            (bounding_h.ceil() + slack).max(1.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-12, "{a} != {b}");
    }

    #[test]
    fn rgba_default_is_opaque_white() {
        let c = Rgba::default();
        approx(c.r, 1.0);
        approx(c.g, 1.0);
        approx(c.b, 1.0);
        approx(c.a, 1.0);
    }

    #[test]
    fn hex_three_digit_expands_nibbles() {
        let c = Rgba::from_hex("#f08").unwrap();
        approx(c.r, 255.0 / 255.0);
        approx(c.g, 0.0);
        approx(c.b, 136.0 / 255.0); // 0x88
        approx(c.a, 1.0);
    }

    #[test]
    fn hex_six_digit() {
        let c = Rgba::from_hex("#FF8800").unwrap();
        approx(c.r, 1.0);
        approx(c.g, 136.0 / 255.0);
        approx(c.b, 0.0);
        approx(c.a, 1.0);
    }

    #[test]
    fn hex_eight_digit_with_alpha() {
        let c = Rgba::from_hex("00FF0080").unwrap();
        approx(c.r, 0.0);
        approx(c.g, 1.0);
        approx(c.b, 0.0);
        approx(c.a, 128.0 / 255.0);
    }

    #[test]
    fn hex_without_hash_and_with_whitespace() {
        let c = Rgba::from_hex("  ffffff  ").unwrap();
        approx(c.r, 1.0);
        approx(c.a, 1.0);
    }

    #[test]
    fn hex_invalid_returns_none() {
        assert!(Rgba::from_hex("#12").is_none()); // length 2
        assert!(Rgba::from_hex("#xyz").is_none()); // non-hex
        assert!(Rgba::from_hex("#1234567").is_none()); // length 7
        assert!(Rgba::from_hex("").is_none());
    }

    #[test]
    fn text_style_defaults_match_upstream() {
        let s = TextStyle::default();
        assert_eq!(s.font_name, "Helvetica-Bold");
        approx(s.font_size, 96.0);
        approx(s.font_scale, 1.0);
        assert_eq!(s.alignment, TextAlignment::Center);
        assert!(s.shadow.enabled);
        approx(s.shadow.offset_y, -2.0);
        approx(s.shadow.blur, 6.0);
        approx(s.shadow.color.a, 0.6);
        assert!(!s.background.enabled);
        approx(s.background.color.a, 0.6);
        assert!(!s.border.enabled);
        approx(s.border.color.a, 1.0);
    }

    #[test]
    fn text_style_decodes_with_missing_fields() {
        let s: TextStyle = serde_json::from_str("{}").unwrap();
        assert_eq!(s.font_name, "Helvetica-Bold");
        approx(s.font_size, 96.0);
        assert_eq!(s.alignment, TextAlignment::Center);
        assert!(s.shadow.enabled);
    }

    #[test]
    fn text_style_partial_decode_keeps_other_defaults() {
        let s: TextStyle = serde_json::from_str(r#"{"fontSize":48,"alignment":"left"}"#).unwrap();
        approx(s.font_size, 48.0);
        assert_eq!(s.alignment, TextAlignment::Left);
        // untouched fields still default
        approx(s.font_scale, 1.0);
        assert_eq!(s.font_name, "Helvetica-Bold");
    }

    #[test]
    fn text_style_roundtrip_camel_case() {
        let s = TextStyle {
            font_name: "Times-Bold".to_string(),
            alignment: TextAlignment::Right,
            background: Fill::new(true, Rgba::new(0.1, 0.2, 0.3, 1.0)),
            ..Default::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"fontName\":\"Times-Bold\""));
        assert!(json.contains("\"fontScale\":1.0"));
        assert!(json.contains("\"offsetY\":-2.0"));
        let back: TextStyle = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn alignment_lowercase_wire_form() {
        assert_eq!(
            serde_json::to_string(&TextAlignment::Center).unwrap(),
            "\"center\""
        );
    }

    #[test]
    fn natural_size_scales_with_canvas_and_adds_padding() {
        let style = TextStyle::default(); // shadow enabled
        let (w, h) = TextLayout::natural_size("Hi", &style, 10000.0, 1080.0);
        // At canvas 1080 scale=1, render_size=96. Non-trivial positive size.
        assert!(w > 0.0 && h > 0.0);
        // Shadow padding (12*2) present: disabling shadow yields a smaller width.
        let mut no_shadow = TextStyle::default();
        no_shadow.shadow.enabled = false;
        let (w2, _) = TextLayout::natural_size("Hi", &no_shadow, 10000.0, 1080.0);
        approx(w - w2, TextLayout::SHADOW_PADDING * 2.0);
    }

    #[test]
    fn natural_size_empty_uses_space_and_is_positive() {
        let style = TextStyle::default();
        let (w, h) = TextLayout::natural_size("", &style, 10000.0, 1080.0);
        assert!(w >= 1.0 && h >= 1.0);
    }

    #[test]
    fn natural_size_canvas_half_height_halves_render_basis() {
        let style = {
            let mut s = TextStyle::default();
            s.shadow.enabled = false;
            s
        };
        let (_, h_full) = TextLayout::natural_size("Word", &style, 10000.0, 1080.0);
        let (_, h_half) = TextLayout::natural_size("Word", &style, 10000.0, 540.0);
        // Half canvas -> ~half line height (allowing for ceil + slack).
        assert!(h_half < h_full);
    }
}
