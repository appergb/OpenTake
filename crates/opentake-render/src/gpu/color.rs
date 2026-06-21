//! sRGB <-> linear conversion (SPEC §3.7). The PoC composites in the sRGB
//! non-linear domain to stay closest to AVFoundation (smallest pixel diff);
//! these helpers exist for the deferred linear-light quality upgrade and are
//! kept exact (IEC 61966-2-1 piecewise curve).

/// sRGB-encoded component (0..1) -> linear (0..1).
pub fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear component (0..1) -> sRGB-encoded (0..1).
pub fn linear_to_srgb(c: f64) -> f64 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    /// Round-trips are only continuous away from the piecewise boundary; near it
    /// the forward/inverse take different branches, so a 1e-6 tolerance is the
    /// right grain (matches standard sRGB implementations).
    fn approx_rt(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    #[test]
    fn endpoints_are_fixed() {
        approx(srgb_to_linear(0.0), 0.0);
        approx(srgb_to_linear(1.0), 1.0);
        approx(linear_to_srgb(0.0), 0.0);
        approx(linear_to_srgb(1.0), 1.0);
    }

    #[test]
    fn round_trips() {
        // Sample within each segment, away from the 0.04045 boundary.
        for &v in &[0.01, 0.03, 0.1, 0.5, 0.9, 0.99] {
            approx_rt(linear_to_srgb(srgb_to_linear(v)), v);
        }
    }

    #[test]
    fn linear_segment_below_threshold() {
        // Below 0.04045 the curve is the linear / 12.92 segment.
        approx(srgb_to_linear(0.04), 0.04 / 12.92);
    }
}
