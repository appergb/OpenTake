//! Text style and layout. 1:1 port of `TextStyle.swift` (data + hex parsing)
//! and a platform-free approximation of `TextLayout.swift`.
//!
//! AppKit/CoreText helpers (`nsColor`, `swiftUIColor`, `resolvedFont`,
//! `paragraphStyle`, `attributes`, `caTextAlignmentMode`) are pure-UI and live in
//! the render/frontend layer. The numeric hex parser is ported verbatim.
//! `TextLayout::natural_size` is an APPROXIMATION: real glyph metrics require a
//! text engine (cosmic-text) in the render layer — see notes on that function.

use serde::{Deserialize, Serialize};

use crate::text_wire::{
    deserialize_background_on_error, deserialize_border_on_error, deserialize_color_on_error,
    deserialize_default_on_error, deserialize_font_name_on_error, deserialize_font_scale_on_error,
    deserialize_font_size_on_error, deserialize_shadow_on_error,
};

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
    /// Persisted keys owned by RGBA's wire schema.
    pub const WIRE_FIELDS: &'static [&'static str] = &["r", "g", "b", "a"];
    pub const TOLERANT_SCALAR_WIRE_FIELDS: &'static [&'static str] = Self::WIRE_FIELDS;

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

impl Shadow {
    /// Persisted keys owned by Shadow's wire schema.
    pub const WIRE_FIELDS: &'static [&'static str] =
        &["enabled", "color", "offsetX", "offsetY", "blur"];
    pub const TOLERANT_SCALAR_WIRE_FIELDS: &'static [&'static str] =
        &["enabled", "offsetX", "offsetY", "blur"];
    pub const COLOR_WIRE_FIELD: &'static str = "color";
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
    /// Persisted keys owned by Fill's wire schema.
    pub const WIRE_FIELDS: &'static [&'static str] = &["enabled", "color"];
    pub const TOLERANT_SCALAR_WIRE_FIELDS: &'static [&'static str] = &["enabled"];
    pub const COLOR_WIRE_FIELD: &'static str = "color";

    pub fn new(enabled: bool, color: Rgba) -> Self {
        Fill { enabled, color }
    }
}

pub(crate) fn default_font_name() -> String {
    "Helvetica-Bold".to_string()
}
pub(crate) fn default_font_size() -> f64 {
    96.0
}
pub(crate) fn default_font_scale() -> f64 {
    1.0
}
pub(crate) fn default_background_fill() -> Fill {
    Fill::new(false, Rgba::new(0.0, 0.0, 0.0, 0.6))
}
pub(crate) fn default_border_fill() -> Fill {
    Fill::new(false, Rgba::new(0.0, 0.0, 0.0, 1.0))
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextStyle {
    #[serde(
        default = "default_font_name",
        deserialize_with = "deserialize_font_name_on_error"
    )]
    pub font_name: String,
    #[serde(
        default = "default_font_size",
        deserialize_with = "deserialize_font_size_on_error"
    )]
    pub font_size: f64,
    #[serde(
        default = "default_font_scale",
        deserialize_with = "deserialize_font_scale_on_error"
    )]
    pub font_scale: f64,
    #[serde(default, deserialize_with = "deserialize_color_on_error")]
    pub color: Rgba,
    #[serde(default, deserialize_with = "deserialize_default_on_error")]
    pub alignment: TextAlignment,
    #[serde(default, deserialize_with = "deserialize_shadow_on_error")]
    pub shadow: Shadow,
    #[serde(
        default = "default_background_fill",
        deserialize_with = "deserialize_background_on_error"
    )]
    pub background: Fill,
    #[serde(
        default = "default_border_fill",
        deserialize_with = "deserialize_border_on_error"
    )]
    pub border: Fill,
}

impl TextStyle {
    /// Persisted keys owned by TextStyle's wire schema. Compatibility scanning
    /// consumes these constants so persistence does not maintain a second copy.
    pub const WIRE_FIELDS: &'static [&'static str] = &[
        "fontName",
        "fontSize",
        "fontScale",
        "color",
        "alignment",
        "shadow",
        "background",
        "border",
    ];
    pub const TOLERANT_SCALAR_WIRE_FIELDS: &'static [&'static str] =
        &["fontName", "fontSize", "fontScale", "alignment"];
    pub const COLOR_WIRE_FIELD: &'static str = "color";
    pub const SHADOW_WIRE_FIELD: &'static str = "shadow";
    pub const FILL_WIRE_FIELDS: &'static [&'static str] = &["background", "border"];
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
    /// Approximate advance for East Asian wide / fullwidth characters, which
    /// render at a full em. Estimating them at the Latin factor (0.6) made
    /// mixed CJK/Latin lines measure ~40% narrow per CJK char; the auto-fit box
    /// then hugged the too-small estimate, the real rasterizer (cosmic-text)
    /// wrapped the overflowing tail onto a second line, and the single-line-tall
    /// box clipped it away (#195 follow-up: "终验 FINAL CHECK" lost "CHECK").
    /// Erring WIDE is the safe direction: a slightly roomy box just renders
    /// centered with margin, while a tight one truncates.
    const APPROX_WIDE_ADVANCE_FACTOR: f64 = 1.0;
    /// Approximate line height as a fraction of the render size.
    const APPROX_LINE_HEIGHT_FACTOR: f64 = 1.2;

    /// Whether `c` occupies a full em (East Asian Wide / Fullwidth): CJK
    /// ideographs, kana, hangul, fullwidth forms, and CJK punctuation. Coarse
    /// range check — this feeds an approximation, so unlisted wide codepoints
    /// merely fall back to the (narrower) Latin factor.
    fn is_wide_char(c: char) -> bool {
        matches!(u32::from(c),
            0x1100..=0x115F        // Hangul Jamo (leading)
            | 0x2E80..=0x303E      // CJK Radicals .. CJK Symbols & Punctuation
            | 0x3041..=0x33FF      // Hiragana .. CJK Compatibility
            | 0x3400..=0x4DBF      // CJK Ext A
            | 0x4E00..=0x9FFF      // CJK Unified Ideographs
            | 0xA000..=0xA4CF      // Yi
            | 0xAC00..=0xD7A3      // Hangul Syllables
            | 0xF900..=0xFAFF      // CJK Compatibility Ideographs
            | 0xFE30..=0xFE4F      // CJK Compatibility Forms
            | 0xFF00..=0xFF60      // Fullwidth Forms
            | 0xFFE0..=0xFFE6      // Fullwidth Signs
            | 0x20000..=0x2FA1F    // CJK Ext B..F + Compat Supplement
        )
    }

    /// Approximate advance of one char in render-size units.
    fn char_advance_factor(c: char) -> f64 {
        if Self::is_wide_char(c) {
            Self::APPROX_WIDE_ADVANCE_FACTOR
        } else {
            Self::APPROX_ADVANCE_FACTOR
        }
    }

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
        // Per-word width honoring wide (CJK/fullwidth) chars at a full em.
        let word_width = |word: &str| -> f64 {
            word.chars()
                .map(|c| render_size * Self::char_advance_factor(c))
                .sum()
        };

        // Greedy word wrap into `max_width`, approximating each line's width.
        let mut lines = 1usize;
        let mut widest = 0.0f64;
        let mut current = 0.0f64;
        for word in measured.split_whitespace() {
            let word_w = word_width(word);
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
            widest = word_width(measured);
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
    fn text_style_malformed_fields_default_independently() {
        let s: TextStyle = serde_json::from_str(
            r#"{
                "fontName": 7,
                "fontSize": 48,
                "fontScale": [],
                "color": {"r": 0.1, "g": 0.2, "b": 0.3, "a": 0.4},
                "alignment": "future",
                "shadow": {
                    "enabled": false,
                    "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 0.5},
                    "offsetX": 1.0,
                    "offsetY": 2.0,
                    "blur": 9.0
                },
                "background": {"enabled": true},
                "border": {
                    "enabled": true,
                    "color": {"r": 0.9, "g": 0.8, "b": 0.7, "a": 0.6}
                }
            }"#,
        )
        .expect("upstream TextStyle defaults each malformed field independently");

        assert_eq!(s.font_name, "Helvetica-Bold");
        approx(s.font_size, 48.0);
        approx(s.font_scale, 1.0);
        approx(s.color.r, 0.1);
        approx(s.color.a, 0.4);
        assert_eq!(s.alignment, TextAlignment::Center);
        assert!(!s.shadow.enabled);
        approx(s.shadow.blur, 9.0);
        assert_eq!(s.background, default_background_fill());
        assert!(s.border.enabled);
        approx(s.border.color.r, 0.9);
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

    /// #195 follow-up: wide (CJK/fullwidth) chars measure a full em, not the
    /// Latin 0.6 factor — a mixed line's estimate must not undershoot what the
    /// real rasterizer lays out, or the auto-fit box wraps + clips the tail
    /// (on-device: "终验 FINAL CHECK" rendered without "CHECK").
    #[test]
    fn wide_chars_measure_a_full_em() {
        let style = TextStyle {
            font_size: 100.0,
            ..TextStyle::default()
        };
        // 4 CJK chars, no spaces: 4 * 1.0em = 400 (+24 shadow pad, +4 slack).
        let (w_cjk, _) = TextLayout::natural_size("终验中文", &style, 10000.0, 1080.0);
        approx(w_cjk, 428.0);
        // 4 Latin chars keep the 0.6 factor: 4 * 60 = 240 (+24 shadow, +4 slack).
        let (w_lat, _) = TextLayout::natural_size("Word", &style, 10000.0, 1080.0);
        approx(w_lat, 268.0);
    }

    #[test]
    fn mixed_cjk_latin_line_width_covers_both_scripts() {
        let style = TextStyle {
            font_size: 100.0,
            ..TextStyle::default()
        };
        // "终验 AB": (2 * 100) + space(60) + (2 * 60) = 380 (+24 shadow, +4 slack).
        let (w, _) = TextLayout::natural_size("终验 AB", &style, 10000.0, 1080.0);
        approx(w, 408.0);
        // Must exceed the old uniform-0.6 estimate (5 * 60 + 24 + 4 = 328).
        assert!(w > 328.0);
    }
}
