//! Advanced per-clip pixel effects: color grading, chroma key, masks, and a
//! generic named-effect chain. These are the **domain-layer value types** for the
//! A-tier shader features (`docs/ADVANCED-FEATURES.md` A-layer); the wgpu
//! compositor mirrors the pixel math defined here in WGSL, and this module owns
//! the unit-tested reference implementation of that math as pure functions so the
//! algorithms are verifiable without a GPU.
//!
//! Design rules (carried from the rest of the domain crate):
//! - Every type derives `serde` with `camelCase` wire keys and a `Default` that
//!   is a **no-op identity** (an all-default `ColorGrade` does not change pixels,
//!   a default `ChromaKey` keys nothing, a default `Mask` covers everything).
//! - All new `Clip` fields are `#[serde(default)] + Option<T>` / `Vec<T>`, so
//!   reading an older project (without these keys) is non-breaking.
//! - The color chain operates in **linear light** (the render layer converts
//!   BT.709 <-> linear around it). Inputs/outputs of [`ColorGrade::apply_linear`]
//!   are linear RGB. The ordering is locked to the spec:
//!   exposure -> white balance -> lift/gamma/gain -> contrast -> saturation.
//!
//! Floating-point parameters use `f64` for parity with the rest of the domain
//! sampling layer; the GPU consumes `f32` (the precision loss is irrelevant at
//! 8-bit output and keeps the WGSL mirror simple).

use serde::{Deserialize, Serialize};

// ===========================================================================
// Small numeric helpers (shared by the reference pixel math).
// ===========================================================================

#[inline]
fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// BT.709 relative luma of a linear-RGB triple. Used by saturation and by the
/// chroma-key luma term. Coefficients are the Rec. 709 standard.
#[inline]
pub fn luma709(r: f64, g: f64, b: f64) -> f64 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Smoothstep `t*t*(3 - 2t)` clamped to `[0,1]` — the edge-feather ramp shared by
/// chroma key and masks. (The keyframe `smoothstep` does NOT clamp; feathering
/// must, so this is a distinct local helper.)
#[inline]
pub fn smoothstep01(edge0: f64, edge1: f64, x: f64) -> f64 {
    if edge0 == edge1 {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ===========================================================================
// ColorGrade
// ===========================================================================

fn default_one() -> f64 {
    1.0
}

/// A 3-channel multiplier triple (R, G, B). Used for white-balance gain,
/// lift/gamma/gain, etc. Default is identity `(1, 1, 1)`.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rgb {
    #[serde(default = "default_one")]
    pub r: f64,
    #[serde(default = "default_one")]
    pub g: f64,
    #[serde(default = "default_one")]
    pub b: f64,
}

impl Default for Rgb {
    fn default() -> Self {
        Rgb {
            r: 1.0,
            g: 1.0,
            b: 1.0,
        }
    }
}

impl Rgb {
    pub fn new(r: f64, g: f64, b: f64) -> Self {
        Rgb { r, g, b }
    }

    /// Additive identity `(0, 0, 0)` — the neutral value for *lift* (an offset),
    /// as opposed to the multiplicative `default()` used for gain.
    pub fn zero() -> Self {
        Rgb {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        }
    }

    fn is_one(&self) -> bool {
        self.r == 1.0 && self.g == 1.0 && self.b == 1.0
    }

    fn is_zero(&self) -> bool {
        self.r == 0.0 && self.g == 0.0 && self.b == 0.0
    }
}

/// Lift / Gamma / Gain color-wheel triple (ASC-CDL-style), each a per-channel
/// [`Rgb`]. `lift` is an additive offset (identity `0`), `gamma` is a power
/// (identity `1`), `gain` is a multiplier (identity `1`).
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiftGammaGain {
    #[serde(default = "Rgb::zero")]
    pub lift: Rgb,
    #[serde(default)]
    pub gamma: Rgb,
    #[serde(default)]
    pub gain: Rgb,
}

impl Default for LiftGammaGain {
    fn default() -> Self {
        LiftGammaGain {
            lift: Rgb::zero(),
            gamma: Rgb::default(),
            gain: Rgb::default(),
        }
    }
}

impl LiftGammaGain {
    fn is_identity(&self) -> bool {
        self.lift.is_zero() && self.gamma.is_one() && self.gain.is_one()
    }

    /// Apply one channel: `gain * (x + lift)` then `^(1/gamma)`. Matches the
    /// classic lift/gamma/gain operator (gamma applied last, as a display power).
    #[inline]
    fn apply_channel(x: f64, lift: f64, gamma: f64, gain: f64) -> f64 {
        let v = gain * (x + lift);
        if gamma > 0.0 && (gamma - 1.0).abs() > f64::EPSILON {
            // `v` can be negative after lift; guard the power.
            v.max(0.0).powf(1.0 / gamma)
        } else {
            v
        }
    }
}

/// High-end floating-point color grade, applied in **linear light** in the order
/// locked by the spec:
/// `exposure -> white balance -> lift/gamma/gain -> contrast -> saturation`.
///
/// Every field defaults to a no-op, so `ColorGrade::default()` is the identity
/// transform (verified by [`ColorGrade::is_identity`] and a unit test).
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorGrade {
    /// Exposure in stops; linear scene gain `2^exposure` (identity `0`).
    #[serde(default)]
    pub exposure: f64,
    /// White-balance temperature, `-1..=1` (warm positive). Identity `0`.
    #[serde(default)]
    pub temperature: f64,
    /// White-balance tint, `-1..=1` (magenta positive, green negative). Identity `0`.
    #[serde(default)]
    pub tint: f64,
    /// Lift / gamma / gain color wheels.
    #[serde(default)]
    pub lift_gamma_gain: LiftGammaGain,
    /// Contrast around the 0.18 mid-grey pivot; `0` = identity, positive raises
    /// contrast. Maps to a slope of `1 + contrast`.
    #[serde(default)]
    pub contrast: f64,
    /// Saturation multiplier (identity `1`; `0` = greyscale, `>1` = boosted).
    #[serde(default = "default_one")]
    pub saturation: f64,
}

impl Default for ColorGrade {
    fn default() -> Self {
        ColorGrade {
            exposure: 0.0,
            temperature: 0.0,
            tint: 0.0,
            lift_gamma_gain: LiftGammaGain::default(),
            contrast: 0.0,
            saturation: 1.0,
        }
    }
}

impl ColorGrade {
    /// Mid-grey pivot for the contrast operator (scene-linear 18% grey).
    pub const CONTRAST_PIVOT: f64 = 0.18;

    /// Whether this grade is a no-op (every stage at its identity). The render
    /// layer uses this to skip the grade entirely.
    pub fn is_identity(&self) -> bool {
        self.exposure == 0.0
            && self.temperature == 0.0
            && self.tint == 0.0
            && self.lift_gamma_gain.is_identity()
            && self.contrast == 0.0
            && self.saturation == 1.0
    }

    /// Per-channel white-balance gain derived from `temperature` / `tint`. A
    /// simple, stable approximation (NOT a full chromatic-adaptation transform):
    /// temperature trades red against blue, tint trades green against magenta.
    /// Returns multiplicative gains centered on `(1,1,1)`.
    pub fn white_balance_gain(&self) -> Rgb {
        // 0.25 keeps the full -1..1 range within a sensible +/-25% channel swing.
        let t = self.temperature * 0.25;
        let g = self.tint * 0.25;
        Rgb {
            r: (1.0 + t).max(0.0),
            g: (1.0 + g).max(0.0),
            b: (1.0 - t).max(0.0),
        }
    }

    /// Apply the full chain to a **linear-RGB** triple, returning linear RGB
    /// clamped to `[0,1]`. This is the reference the WGSL fragment shader mirrors.
    pub fn apply_linear(&self, r: f64, g: f64, b: f64) -> (f64, f64, f64) {
        // 1. Exposure (linear gain).
        let exp = if self.exposure != 0.0 {
            2f64.powf(self.exposure)
        } else {
            1.0
        };
        let mut rr = r * exp;
        let mut gg = g * exp;
        let mut bb = b * exp;

        // 2. White balance (per-channel gain).
        if self.temperature != 0.0 || self.tint != 0.0 {
            let wb = self.white_balance_gain();
            rr *= wb.r;
            gg *= wb.g;
            bb *= wb.b;
        }

        // 3. Lift / gamma / gain.
        if !self.lift_gamma_gain.is_identity() {
            let lgg = &self.lift_gamma_gain;
            rr = LiftGammaGain::apply_channel(rr, lgg.lift.r, lgg.gamma.r, lgg.gain.r);
            gg = LiftGammaGain::apply_channel(gg, lgg.lift.g, lgg.gamma.g, lgg.gain.g);
            bb = LiftGammaGain::apply_channel(bb, lgg.lift.b, lgg.gamma.b, lgg.gain.b);
        }

        // 4. Contrast around the mid-grey pivot.
        if self.contrast != 0.0 {
            let slope = 1.0 + self.contrast;
            rr = (rr - Self::CONTRAST_PIVOT) * slope + Self::CONTRAST_PIVOT;
            gg = (gg - Self::CONTRAST_PIVOT) * slope + Self::CONTRAST_PIVOT;
            bb = (bb - Self::CONTRAST_PIVOT) * slope + Self::CONTRAST_PIVOT;
        }

        // 5. Saturation (luma-preserving lerp toward grey).
        if (self.saturation - 1.0).abs() > f64::EPSILON {
            let l = luma709(rr, gg, bb);
            rr = l + (rr - l) * self.saturation;
            gg = l + (gg - l) * self.saturation;
            bb = l + (bb - l) * self.saturation;
        }

        (clamp01(rr), clamp01(gg), clamp01(bb))
    }
}

// ===========================================================================
// ChromaKey
// ===========================================================================

fn default_key_color() -> Rgb {
    // Classic green-screen key (sRGB-ish full green).
    Rgb {
        r: 0.0,
        g: 1.0,
        b: 0.0,
    }
}
fn default_similarity() -> f64 {
    0.15
}
fn default_smoothness() -> f64 {
    0.35
}
fn default_spill() -> f64 {
    0.5
}

/// Green/blue-screen chroma key. The reference algorithm computes a chroma-only
/// distance from the key color (luma-independent, so shadows/highlights on the
/// subject survive), maps it through `similarity`/`smoothness` to an alpha, and
/// optionally suppresses color spill toward the key hue.
///
/// `ChromaKey::default()` keys the canonical green but is only *active* when set
/// on a clip — an all-default value still computes a sensible matte. The render
/// layer treats `None` (no chroma key field) as "disabled".
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromaKey {
    /// The key color to remove (linear or sRGB; the math is hue-based so the
    /// space only shifts the threshold slightly — the render layer feeds the same
    /// space it samples).
    #[serde(default = "default_key_color")]
    pub key_color: Rgb,
    /// Chroma distance below which pixels are fully keyed (transparent). `0..~1`.
    #[serde(default = "default_similarity")]
    pub similarity: f64,
    /// Feather width above `similarity` over which alpha ramps 0 -> 1. `>= 0`.
    #[serde(default = "default_smoothness")]
    pub smoothness: f64,
    /// Spill suppression strength `0..=1` (0 = none, 1 = full desaturation of the
    /// key hue on retained pixels).
    #[serde(default = "default_spill")]
    pub spill: f64,
}

impl Default for ChromaKey {
    fn default() -> Self {
        ChromaKey {
            key_color: default_key_color(),
            similarity: default_similarity(),
            smoothness: default_smoothness(),
            spill: default_spill(),
        }
    }
}

/// Convert a linear/sRGB RGB triple to a 2-D chroma vector `(cb, cr)` —
/// blue-difference and red-difference **normalized by luma**, so brightness
/// cancels out. Two colors with the same hue/saturation but different brightness
/// (e.g. a lit vs. shadowed patch of the same green screen) map to nearly the
/// same point — that is what makes the key luma-independent. The small luma floor
/// keeps near-black stable. This is the form the WGSL fragment shader mirrors.
#[inline]
pub fn chroma_cb_cr(r: f64, g: f64, b: f64) -> (f64, f64) {
    let y = luma709(r, g, b);
    // Floor matches the WGSL mirror; near-black has ill-defined hue anyway.
    let inv = 1.0 / (y + 1e-4);
    ((b - y) * inv, (r - y) * inv)
}

impl ChromaKey {
    /// The matte alpha for one pixel: `1.0` = fully opaque (kept), `0.0` = fully
    /// keyed (transparent). Reference for the WGSL mirror.
    pub fn alpha(&self, r: f64, g: f64, b: f64) -> f64 {
        let (kcb, kcr) = chroma_cb_cr(self.key_color.r, self.key_color.g, self.key_color.b);
        let (pcb, pcr) = chroma_cb_cr(r, g, b);
        let dist = ((pcb - kcb).powi(2) + (pcr - kcr).powi(2)).sqrt();
        // dist <= similarity -> 0 (keyed); dist >= similarity+smoothness -> 1.
        smoothstep01(
            self.similarity,
            self.similarity + self.smoothness.max(0.0),
            dist,
        )
    }

    /// Spill-suppressed color for a retained pixel: pulls the key-hue channel down
    /// toward the average of the other two when it dominates (de-greens edges).
    /// Returns the corrected `(r, g, b)`. Reference for the WGSL mirror.
    pub fn suppress_spill(&self, r: f64, g: f64, b: f64) -> (f64, f64, f64) {
        if self.spill <= 0.0 {
            return (r, g, b);
        }
        let k = self.spill.clamp(0.0, 1.0);
        // Identify the dominant key channel from the key color.
        let (kr, kg, kb) = (self.key_color.r, self.key_color.g, self.key_color.b);
        // Green key (the common case): suppress green above the r/b average.
        if kg >= kr && kg >= kb {
            let avg = (r + b) * 0.5;
            let ng = if g > avg {
                avg + (g - avg) * (1.0 - k)
            } else {
                g
            };
            (r, ng, b)
        } else if kb >= kr && kb >= kg {
            let avg = (r + g) * 0.5;
            let nb = if b > avg {
                avg + (b - avg) * (1.0 - k)
            } else {
                b
            };
            (r, g, nb)
        } else {
            let avg = (g + b) * 0.5;
            let nr = if r > avg {
                avg + (r - avg) * (1.0 - k)
            } else {
                r
            };
            (nr, g, b)
        }
    }
}

// ===========================================================================
// Mask
// ===========================================================================

/// Shape of a [`Mask`]. Coordinates are normalized canvas space `(0..1)`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MaskShape {
    /// Half-plane split by a line through `point` with outward normal `normal`
    /// (the "linear"/gradient mask). The covered side is where the signed
    /// distance along the normal is positive.
    Linear { point: Point2, normal: Point2 },
    /// Axis-aligned ellipse with `center` and per-axis `radius`.
    Circle { center: Point2, radius: Point2 },
    /// Closed polygon (pen tool); even-odd fill of `points` (>= 3).
    Poly { points: Vec<Point2> },
}

/// A normalized 2-D point used by mask shapes. Distinct from
/// [`crate::transform::Point`] only to keep mask serialization self-contained and
/// `Serialize`/`Deserialize`-derivable (the transform `Point` has hand-written
/// (de)serialization elsewhere; here a plain derive is what we want).
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Point2 {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
}

impl Point2 {
    pub fn new(x: f64, y: f64) -> Self {
        Point2 { x, y }
    }
}

/// A vector mask that generates a per-pixel alpha coverage. `feather` softens the
/// edge; `invert` flips inside/outside.
///
/// `Mask::default()` is a full-canvas circle covering everything (coverage `1`
/// everywhere) so a freshly-defaulted mask never accidentally hides content.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mask {
    #[serde(default = "default_mask_shape")]
    pub shape: MaskShape,
    /// Edge feather in normalized canvas units (`>= 0`). `0` = hard edge.
    #[serde(default)]
    pub feather: f64,
    /// Invert coverage (mask out the inside instead of the outside).
    #[serde(default)]
    pub invert: bool,
}

fn default_mask_shape() -> MaskShape {
    // A circle large enough to cover the whole 0..1 canvas (and corners).
    MaskShape::Circle {
        center: Point2::new(0.5, 0.5),
        radius: Point2::new(1.5, 1.5),
    }
}

impl Default for Mask {
    fn default() -> Self {
        Mask {
            shape: default_mask_shape(),
            feather: 0.0,
            invert: false,
        }
    }
}

impl Mask {
    /// Signed distance to the shape boundary at normalized point `(x, y)`,
    /// **negative inside**, positive outside (standard SDF convention). The
    /// polygon variant returns the unsigned distance with an inside/outside sign
    /// from an even-odd test (an exact polygon SDF is overkill for feathering).
    pub fn signed_distance(&self, x: f64, y: f64) -> f64 {
        match &self.shape {
            MaskShape::Linear { point, normal } => {
                // Signed distance along the (assumed unit-ish) normal. We
                // normalize defensively so feather widths are in canvas units.
                let nlen = (normal.x * normal.x + normal.y * normal.y).sqrt();
                if nlen <= f64::EPSILON {
                    return 0.0;
                }
                let nx = normal.x / nlen;
                let ny = normal.y / nlen;
                // Positive on the +normal side; SDF is negative inside the covered
                // half-plane, so negate.
                -((x - point.x) * nx + (y - point.y) * ny)
            }
            MaskShape::Circle { center, radius } => {
                let rx = radius.x.max(f64::EPSILON);
                let ry = radius.y.max(f64::EPSILON);
                // Map to the unit circle, then scale the distance back by the
                // smaller radius so `feather` reads in canvas units.
                let dx = (x - center.x) / rx;
                let dy = (y - center.y) / ry;
                let d = (dx * dx + dy * dy).sqrt();
                (d - 1.0) * rx.min(ry)
            }
            MaskShape::Poly { points } => poly_signed_distance(points, x, y),
        }
    }

    /// Per-pixel coverage in `[0,1]` for normalized point `(x, y)`: `1` fully
    /// inside, `0` fully outside, feathered across the boundary, then inverted if
    /// requested. Reference for the WGSL mirror.
    pub fn coverage(&self, x: f64, y: f64) -> f64 {
        let sd = self.signed_distance(x, y);
        let f = self.feather.max(0.0);
        // sd <= -f/2 -> fully inside (1); sd >= +f/2 -> fully outside (0).
        let inside = if f <= f64::EPSILON {
            if sd <= 0.0 {
                1.0
            } else {
                0.0
            }
        } else {
            // smoothstep from outside(0) to inside(1): high coverage at low sd.
            1.0 - smoothstep01(-f * 0.5, f * 0.5, sd)
        };
        if self.invert {
            1.0 - inside
        } else {
            inside
        }
    }
}

/// Even-odd point-in-polygon with an approximate signed distance: returns
/// `-min_edge_distance` inside, `+min_edge_distance` outside. Adequate for
/// feathering; not a mathematically exact polygon SDF.
fn poly_signed_distance(points: &[Point2], px: f64, py: f64) -> f64 {
    if points.len() < 3 {
        // Degenerate polygon covers nothing.
        return f64::INFINITY;
    }
    let mut inside = false;
    let mut min_d2 = f64::INFINITY;
    let n = points.len();
    let mut j = n - 1;
    for i in 0..n {
        let pi = points[i];
        let pj = points[j];
        // Even-odd ray cast.
        let intersects = ((pi.y > py) != (pj.y > py))
            && (px < (pj.x - pi.x) * (py - pi.y) / (pj.y - pi.y) + pi.x);
        if intersects {
            inside = !inside;
        }
        // Distance to the edge segment (pi, pj).
        let d2 = point_segment_dist2(px, py, pi.x, pi.y, pj.x, pj.y);
        if d2 < min_d2 {
            min_d2 = d2;
        }
        j = i;
    }
    let d = min_d2.sqrt();
    if inside {
        -d
    } else {
        d
    }
}

#[inline]
fn point_segment_dist2(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let abx = bx - ax;
    let aby = by - ay;
    let apx = px - ax;
    let apy = py - ay;
    let denom = abx * abx + aby * aby;
    let t = if denom <= f64::EPSILON {
        0.0
    } else {
        ((apx * abx + apy * aby) / denom).clamp(0.0, 1.0)
    };
    let cx = ax + abx * t;
    let cy = ay + aby * t;
    let dx = px - cx;
    let dy = py - cy;
    dx * dx + dy * dy
}

// ===========================================================================
// Effect (generic named-parameter effect chain)
// ===========================================================================

/// A generic named pixel effect with a flat parameter map — the extensible chain
/// the spec calls for (`Clip.effects: Vec<Effect>`, each = one wgpu pass). The
/// `name` selects a shader/kernel; `params` are its named scalar inputs and
/// `enabled` lets a clip carry a disabled effect without removing it.
///
/// Concrete effects (blur, glow, sharpen, ...) are deferred (see module TODO);
/// this type and its serde/round-trip are the stable contract that ops + agent
/// tools target now, and the render layer can grow per-name handling
/// incrementally without further domain changes.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    /// Effect identifier (e.g. `"gaussianBlur"`). Free-form; the render layer maps
    /// known names to passes and ignores unknown ones.
    pub name: String,
    /// Named scalar parameters. Insertion-stable ordering is not required; the
    /// render layer reads by key.
    #[serde(default)]
    pub params: std::collections::BTreeMap<String, f64>,
    /// Whether the effect is active. Defaults to `true`.
    #[serde(default = "bool_true")]
    pub enabled: bool,
}

fn bool_true() -> bool {
    true
}

impl Effect {
    /// Construct an enabled effect with no parameters.
    pub fn new(name: impl Into<String>) -> Self {
        Effect {
            name: name.into(),
            params: std::collections::BTreeMap::new(),
            enabled: true,
        }
    }

    /// Builder-style parameter setter.
    pub fn with_param(mut self, key: impl Into<String>, value: f64) -> Self {
        self.params.insert(key.into(), value);
        self
    }

    /// Read a parameter, or `default` when absent.
    pub fn param(&self, key: &str, default: f64) -> f64 {
        self.params.get(key).copied().unwrap_or(default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    fn approx_eps(a: f64, b: f64, eps: f64) {
        assert!((a - b).abs() < eps, "{a} != {b} (eps {eps})");
    }

    // --- ColorGrade identity / defaults ---

    #[test]
    fn color_grade_default_is_identity() {
        let g = ColorGrade::default();
        assert!(g.is_identity());
        let (r, gg, b) = g.apply_linear(0.3, 0.5, 0.7);
        approx(r, 0.3);
        approx(gg, 0.5);
        approx(b, 0.7);
    }

    #[test]
    fn exposure_one_stop_doubles_linear() {
        let g = ColorGrade {
            exposure: 1.0,
            ..Default::default()
        };
        let (r, gg, b) = g.apply_linear(0.1, 0.2, 0.25);
        approx(r, 0.2);
        approx(gg, 0.4);
        approx(b, 0.5);
        assert!(!g.is_identity());
    }

    #[test]
    fn exposure_clamps_at_one() {
        let g = ColorGrade {
            exposure: 2.0,
            ..Default::default()
        };
        // 0.5 * 4 = 2.0 -> clamps to 1.0
        let (r, _, _) = g.apply_linear(0.5, 0.0, 0.0);
        approx(r, 1.0);
    }

    #[test]
    fn saturation_zero_is_greyscale() {
        let g = ColorGrade {
            saturation: 0.0,
            ..Default::default()
        };
        let (r, gg, b) = g.apply_linear(0.2, 0.6, 0.9);
        let l = luma709(0.2, 0.6, 0.9);
        approx(r, l);
        approx(gg, l);
        approx(b, l);
    }

    #[test]
    fn saturation_preserves_luma() {
        let g = ColorGrade {
            saturation: 1.8,
            ..Default::default()
        };
        let (r, gg, b) = g.apply_linear(0.2, 0.5, 0.4);
        // Luma is preserved by a luma-centered saturation lerp (before clamping,
        // which doesn't trigger here since values stay in range).
        approx_eps(luma709(r, gg, b), luma709(0.2, 0.5, 0.4), 1e-9);
    }

    #[test]
    fn contrast_keeps_pivot_fixed() {
        let g = ColorGrade {
            contrast: 0.5,
            ..Default::default()
        };
        // The 0.18 pivot is the fixed point of the contrast operator.
        let (r, _, _) = g.apply_linear(ColorGrade::CONTRAST_PIVOT, 0.0, 0.0);
        approx(r, ColorGrade::CONTRAST_PIVOT);
    }

    #[test]
    fn white_balance_warm_boosts_red_cuts_blue() {
        let g = ColorGrade {
            temperature: 1.0,
            ..Default::default()
        };
        let wb = g.white_balance_gain();
        assert!(wb.r > 1.0);
        assert!(wb.b < 1.0);
        approx(wb.g, 1.0);
    }

    #[test]
    fn lift_gamma_gain_gain_scales() {
        let g = ColorGrade {
            lift_gamma_gain: LiftGammaGain {
                gain: Rgb::new(0.5, 1.0, 1.0),
                ..Default::default()
            },
            ..Default::default()
        };
        let (r, gg, _) = g.apply_linear(0.4, 0.4, 0.0);
        approx(r, 0.2); // 0.4 * 0.5
        approx(gg, 0.4); // unchanged
    }

    #[test]
    fn lift_lifts_blacks() {
        let g = ColorGrade {
            lift_gamma_gain: LiftGammaGain {
                lift: Rgb::new(0.1, 0.0, 0.0),
                ..Default::default()
            },
            ..Default::default()
        };
        let (r, _, _) = g.apply_linear(0.0, 0.0, 0.0);
        approx(r, 0.1);
    }

    // --- ColorGrade serde ---

    #[test]
    fn color_grade_roundtrip_camel_case() {
        let g = ColorGrade {
            exposure: 0.5,
            temperature: 0.2,
            tint: -0.1,
            lift_gamma_gain: LiftGammaGain {
                lift: Rgb::new(0.02, 0.0, 0.0),
                gamma: Rgb::new(1.0, 1.1, 1.0),
                gain: Rgb::new(1.0, 1.0, 0.9),
            },
            contrast: 0.3,
            saturation: 1.2,
        };
        let json = serde_json::to_string(&g).unwrap();
        assert!(json.contains("\"liftGammaGain\""));
        assert!(json.contains("\"exposure\":0.5"));
        let back: ColorGrade = serde_json::from_str(&json).unwrap();
        assert_eq!(g, back);
    }

    #[test]
    fn color_grade_decodes_missing_fields_as_identity() {
        let g: ColorGrade = serde_json::from_str("{}").unwrap();
        assert!(g.is_identity());
    }

    #[test]
    fn color_grade_partial_decode_keeps_other_defaults() {
        let g: ColorGrade = serde_json::from_str(r#"{"exposure":1.0}"#).unwrap();
        approx(g.exposure, 1.0);
        approx(g.saturation, 1.0); // still identity default
        assert!(g.lift_gamma_gain.is_identity());
    }

    // --- ChromaKey ---

    #[test]
    fn chroma_key_default_keys_green() {
        let k = ChromaKey::default();
        // Pure green is the key -> alpha 0 (fully keyed).
        approx(k.alpha(0.0, 1.0, 0.0), 0.0);
        // Pure red is far from green chroma -> alpha 1 (kept).
        approx(k.alpha(1.0, 0.0, 0.0), 1.0);
    }

    #[test]
    fn chroma_key_is_luma_independent() {
        let k = ChromaKey::default();
        // Pure green and a much darker green of the same hue both key to ~0
        // alpha: the luma-normalized chroma cancels the brightness difference.
        let a_pure = k.alpha(0.0, 1.0, 0.0);
        let a_dark = k.alpha(0.0, 0.3, 0.0);
        approx(a_pure, 0.0);
        assert!(a_dark < 0.05, "dark green of same hue should key: {a_dark}");
    }

    #[test]
    fn chroma_key_feather_ramps_alpha() {
        let k = ChromaKey {
            key_color: Rgb::new(0.0, 1.0, 0.0),
            similarity: 0.2,
            smoothness: 0.4,
            spill: 0.0,
        };
        // Green diluted toward white lands in the feather band [0.2, 0.6] of the
        // luma-normalized distance -> partial alpha.
        let a = k.alpha(0.2, 1.0, 0.2);
        assert!(a > 0.0 && a < 1.0, "expected partial alpha, got {a}");
    }

    #[test]
    fn chroma_key_spill_suppresses_green() {
        let k = ChromaKey {
            spill: 1.0,
            ..Default::default()
        };
        // A pixel with excess green gets it pulled to the r/b average.
        let (r, g, b) = k.suppress_spill(0.2, 0.9, 0.4);
        approx(r, 0.2);
        approx(b, 0.4);
        approx(g, (0.2 + 0.4) * 0.5); // fully suppressed to the average
    }

    #[test]
    fn chroma_key_no_spill_is_identity() {
        let k = ChromaKey {
            spill: 0.0,
            ..Default::default()
        };
        let (r, g, b) = k.suppress_spill(0.2, 0.9, 0.4);
        approx(r, 0.2);
        approx(g, 0.9);
        approx(b, 0.4);
    }

    #[test]
    fn chroma_key_roundtrip() {
        let k = ChromaKey {
            key_color: Rgb::new(0.0, 0.0, 1.0),
            similarity: 0.3,
            smoothness: 0.05,
            spill: 0.7,
        };
        let json = serde_json::to_string(&k).unwrap();
        assert!(json.contains("\"keyColor\""));
        assert!(json.contains("\"similarity\":0.3"));
        let back: ChromaKey = serde_json::from_str(&json).unwrap();
        assert_eq!(k, back);
    }

    #[test]
    fn chroma_key_decodes_missing_fields() {
        let k: ChromaKey = serde_json::from_str("{}").unwrap();
        assert_eq!(k, ChromaKey::default());
    }

    // --- Mask ---

    #[test]
    fn mask_default_covers_everything() {
        let m = Mask::default();
        approx(m.coverage(0.5, 0.5), 1.0);
        approx(m.coverage(0.0, 0.0), 1.0);
        approx(m.coverage(1.0, 1.0), 1.0);
    }

    #[test]
    fn circle_mask_inside_outside() {
        let m = Mask {
            shape: MaskShape::Circle {
                center: Point2::new(0.5, 0.5),
                radius: Point2::new(0.2, 0.2),
            },
            feather: 0.0,
            invert: false,
        };
        approx(m.coverage(0.5, 0.5), 1.0); // center
        approx(m.coverage(0.5, 0.9), 0.0); // outside radius
    }

    #[test]
    fn circle_mask_invert_flips() {
        let m = Mask {
            shape: MaskShape::Circle {
                center: Point2::new(0.5, 0.5),
                radius: Point2::new(0.2, 0.2),
            },
            feather: 0.0,
            invert: true,
        };
        approx(m.coverage(0.5, 0.5), 0.0); // center now masked out
        approx(m.coverage(0.5, 0.9), 1.0); // outside now covered
    }

    #[test]
    fn circle_mask_feather_is_partial_at_edge() {
        let m = Mask {
            shape: MaskShape::Circle {
                center: Point2::new(0.5, 0.5),
                radius: Point2::new(0.2, 0.2),
            },
            feather: 0.1,
            invert: false,
        };
        // Exactly on the boundary -> ~0.5 coverage.
        let c = m.coverage(0.7, 0.5);
        approx_eps(c, 0.5, 1e-9);
    }

    #[test]
    fn linear_mask_half_plane() {
        // Line through (0.5, _) with normal +x. Per the SDF convention, the
        // covered side is where the signed distance ALONG the normal is positive,
        // i.e. x > 0.5.
        let m = Mask {
            shape: MaskShape::Linear {
                point: Point2::new(0.5, 0.5),
                normal: Point2::new(1.0, 0.0),
            },
            feather: 0.0,
            invert: false,
        };
        approx(m.coverage(0.8, 0.5), 1.0); // +normal side covered
        approx(m.coverage(0.2, 0.5), 0.0); // -normal side not
    }

    #[test]
    fn poly_triangle_inside_outside() {
        let m = Mask {
            shape: MaskShape::Poly {
                points: vec![
                    Point2::new(0.1, 0.1),
                    Point2::new(0.9, 0.1),
                    Point2::new(0.5, 0.9),
                ],
            },
            feather: 0.0,
            invert: false,
        };
        approx(m.coverage(0.5, 0.3), 1.0); // inside the triangle
        approx(m.coverage(0.05, 0.05), 0.0); // outside (a corner region)
    }

    #[test]
    fn poly_degenerate_covers_nothing() {
        let m = Mask {
            shape: MaskShape::Poly {
                points: vec![Point2::new(0.0, 0.0), Point2::new(1.0, 1.0)],
            },
            feather: 0.0,
            invert: false,
        };
        approx(m.coverage(0.5, 0.5), 0.0);
    }

    #[test]
    fn mask_roundtrip_tagged_shape() {
        let m = Mask {
            shape: MaskShape::Circle {
                center: Point2::new(0.4, 0.6),
                radius: Point2::new(0.3, 0.2),
            },
            feather: 0.05,
            invert: true,
        };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("\"kind\":\"circle\""));
        assert!(json.contains("\"feather\":0.05"));
        let back: Mask = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn mask_linear_roundtrip() {
        let m = Mask {
            shape: MaskShape::Linear {
                point: Point2::new(0.5, 0.5),
                normal: Point2::new(0.0, 1.0),
            },
            feather: 0.0,
            invert: false,
        };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("\"kind\":\"linear\""));
        let back: Mask = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn mask_poly_roundtrip() {
        let m = Mask {
            shape: MaskShape::Poly {
                points: vec![
                    Point2::new(0.0, 0.0),
                    Point2::new(1.0, 0.0),
                    Point2::new(0.5, 1.0),
                ],
            },
            feather: 0.0,
            invert: false,
        };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("\"kind\":\"poly\""));
        let back: Mask = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }

    // --- Effect ---

    #[test]
    fn effect_new_is_enabled_no_params() {
        let e = Effect::new("gaussianBlur");
        assert_eq!(e.name, "gaussianBlur");
        assert!(e.enabled);
        assert!(e.params.is_empty());
        approx(e.param("radius", 3.0), 3.0); // default fallback
    }

    #[test]
    fn effect_with_param_and_read() {
        let e = Effect::new("glow").with_param("intensity", 0.8);
        approx(e.param("intensity", 0.0), 0.8);
    }

    #[test]
    fn effect_roundtrip_with_params() {
        let e = Effect::new("sharpen").with_param("amount", 0.5);
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"name\":\"sharpen\""));
        assert!(json.contains("\"amount\":0.5"));
        let back: Effect = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn effect_decodes_default_enabled() {
        // Missing `enabled` -> true; missing `params` -> empty.
        let e: Effect = serde_json::from_str(r#"{"name":"blur"}"#).unwrap();
        assert!(e.enabled);
        assert!(e.params.is_empty());
    }

    #[test]
    fn effect_decodes_disabled() {
        let e: Effect = serde_json::from_str(r#"{"name":"blur","enabled":false}"#).unwrap();
        assert!(!e.enabled);
    }
}
