//! Render-layer geometry projection (SPEC §2.6). The ONLY math this crate adds
//! on top of the already-ported domain sampling: the normalized-canvas -> pixel
//! affine, CG `concatenating`, and crop -> UV. These are exactly what
//! AVFoundation did for upstream and that the domain layer (correctly) does not.
//!
//! ## Affine representation and CG semantics (the pixel-diff lifeline)
//!
//! A `CGAffineTransform` is `[a, b, c, d, tx, ty]`, applied to a point as a ROW
//! vector on the left:
//!
//! ```text
//!                 | a  b  0 |
//! (x', y', 1) = (x, y, 1) · | c  d  0 |   =>   x' = x·a + y·c + tx
//!                 | tx ty 1 |               y' = x·b + y·d + ty
//! ```
//!
//! CG's `A.concatenating(B)` means `A · B` — apply `A` first, then `B`. We store
//! row-major `[a, b, c, d, tx, ty]` so the 6-tuple maps 1:1 onto
//! `CGAffineTransform`'s fields, and [`compose`] implements `concatenating`.
//! The WGSL vertex shader uses the SAME `p · M` convention (SPEC §3.3), so the
//! tuple is uploaded unchanged.

use opentake_domain::{Crop, Transform};

use super::types::RenderSize;

/// Identity affine `[1, 0, 0, 1, 0, 0]`.
pub const IDENTITY: [f64; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// Build a scale affine `[sx, 0, 0, sy, 0, 0]`.
#[inline]
fn scale(sx: f64, sy: f64) -> [f64; 6] {
    [sx, 0.0, 0.0, sy, 0.0, 0.0]
}

/// Build a translation affine `[1, 0, 0, 1, tx, ty]`.
#[inline]
fn translate(tx: f64, ty: f64) -> [f64; 6] {
    [1.0, 0.0, 0.0, 1.0, tx, ty]
}

/// Build a rotation affine. `radians` positive is counter-clockwise in CG's
/// coordinate space (origin bottom-left, y up), matching
/// `CGAffineTransform(rotationAngle:)`:
/// `[cos, sin, -sin, cos, 0, 0]`.
#[inline]
fn rotate(radians: f64) -> [f64; 6] {
    let (s, c) = radians.sin_cos();
    [c, s, -s, c, 0.0, 0.0]
}

/// CG `a.concatenating(b)` == `a · b` (apply `a` first, then `b`).
///
/// With row-vector convention `p' = p · a · b`, so `compose(a, b)` is the
/// matrix product `a · b`. Verified against `CGPoint.applying` in tests.
pub fn compose(a: [f64; 6], b: [f64; 6]) -> [f64; 6] {
    let [a0, a1, a2, a3, a4, a5] = a;
    let [b0, b1, b2, b3, b4, b5] = b;
    // a as 3x3 (last column 0,0,1) times b as 3x3.
    [
        a0 * b0 + a1 * b2,         // a
        a0 * b1 + a1 * b3,         // b
        a2 * b0 + a3 * b2,         // c
        a2 * b1 + a3 * b3,         // d
        a4 * b0 + a5 * b2 + b4,    // tx
        a4 * b1 + a5 * b3 + b5,    // ty
    ]
}

/// Apply an affine to a point as a row vector: `p · M`. (Test helper / parity
/// check against CG `CGPoint.applying`.)
pub fn apply_point(m: [f64; 6], x: f64, y: f64) -> (f64, f64) {
    (x * m[0] + y * m[2] + m[4], x * m[1] + y * m[3] + m[5])
}

/// Maps a clip's [`Transform`] (normalized 0–1 canvas coordinates) to the affine
/// an AVFoundation layer instruction expects. Line-for-line port of upstream
/// `CompositionBuilder.affineTransform(for:natSize:renderSize:)` (L599-614).
///
/// `nat` is the clip's source display size (pixels); `rs` is the canvas size
/// (pixels). The result places the source quad onto the canvas in pixel space.
pub fn affine_transform(t: &Transform, nat: (f64, f64), rs: RenderSize) -> [f64; 6] {
    let tl = t.top_left();
    let sx = (rs.width_f() / nat.0) * t.width * if t.flip_horizontal { -1.0 } else { 1.0 };
    let sy = (rs.height_f() / nat.1) * t.height * if t.flip_vertical { -1.0 } else { 1.0 };
    let tx = if t.flip_horizontal {
        tl.x + t.width
    } else {
        tl.x
    } * rs.width_f();
    let ty = if t.flip_vertical {
        tl.y + t.height
    } else {
        tl.y
    } * rs.height_f();

    // placed = scale(sx, sy).concatenating(translate(tx, ty))
    let placed = compose(scale(sx, sy), translate(tx, ty));
    if t.rotation == 0.0 {
        return placed;
    }
    let cx = t.center_x * rs.width_f();
    let cy = t.center_y * rs.height_f();
    // placed
    //   .concatenating(translate(-cx, -cy))
    //   .concatenating(rotate(rotation * pi/180))
    //   .concatenating(translate(cx, cy))
    let step1 = compose(placed, translate(-cx, -cy));
    let step2 = compose(step1, rotate(t.rotation * std::f64::consts::PI / 180.0));
    compose(step2, translate(cx, cy))
}

/// Source crop (normalized inset, origin top-left) -> texture UV sub-rect
/// `(u0, v0, u1, v1)`. SPEC §3.4.
///
/// Visible region in source coords is `[left, 1-right] x [top, 1-bottom]`. The
/// domain `visible_*_fraction` already clamps to >= 0; we additionally clamp the
/// resulting UV bounds to `[0, 1]` and guarantee `u0 <= u1`, `v0 <= v1` so the
/// sampler never reads outside the texture (mirrors upstream's `max(1, ...)`
/// one-source-pixel floor in UV space).
///
/// V runs the same direction as U here (top -> bottom); the single y-flip needed
/// to reconcile "row 0 = top" textures with CG's "y up" geometry happens exactly
/// once in the shader (SPEC §3.4), not here.
pub fn crop_to_uv(c: Crop) -> (f64, f64, f64, f64) {
    let u0 = c.left.clamp(0.0, 1.0);
    let v0 = c.top.clamp(0.0, 1.0);
    let u1 = (1.0 - c.right).clamp(0.0, 1.0);
    let v1 = (1.0 - c.bottom).clamp(0.0, 1.0);
    (u0.min(u1), v0.min(v1), u0.max(u1), v0.max(v1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::Point;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    fn approx_affine(a: [f64; 6], b: [f64; 6]) {
        for i in 0..6 {
            assert!((a[i] - b[i]).abs() < 1e-9, "elem {i}: {} != {}", a[i], b[i]);
        }
    }

    #[test]
    fn compose_with_identity_is_noop() {
        let m = [2.0, 0.5, -0.3, 1.5, 10.0, -4.0];
        approx_affine(compose(m, IDENTITY), m);
        approx_affine(compose(IDENTITY, m), m);
    }

    #[test]
    fn compose_matches_cg_concatenating_apply_order() {
        // CG: p.applying(A.concatenating(B)) == p.applying(A).applying(B)
        let a = compose(scale(2.0, 3.0), translate(1.0, 2.0)); // scale then translate
        let b = rotate(std::f64::consts::FRAC_PI_2); // +90 deg
        let m = compose(a, b);

        let p = (1.0, 1.0);
        let via_m = apply_point(m, p.0, p.1);
        let pa = apply_point(a, p.0, p.1);
        let via_seq = apply_point(b, pa.0, pa.1);
        approx(via_m.0, via_seq.0);
        approx(via_m.1, via_seq.1);
    }

    #[test]
    fn translate_then_apply() {
        let m = translate(5.0, -3.0);
        let (x, y) = apply_point(m, 2.0, 2.0);
        approx(x, 7.0);
        approx(y, -1.0);
    }

    #[test]
    fn rotate_90_maps_x_axis_to_y_axis() {
        // CG rotationAngle(+90deg): (1,0) -> (cos90, sin90) = (0, 1).
        let m = rotate(std::f64::consts::FRAC_PI_2);
        let (x, y) = apply_point(m, 1.0, 0.0);
        approx(x, 0.0);
        approx(y, 1.0);
    }

    #[test]
    fn identity_transform_full_canvas_centered() {
        // Transform::default() = center (0.5,0.5), size 1x1, no rotation/flip.
        // nat == render -> affine = [1,0,0,1,0,0] (full-canvas, no offset).
        let t = Transform::default();
        let rs = RenderSize::new(1920, 1080);
        let m = affine_transform(&t, (1920.0, 1080.0), rs);
        approx_affine(m, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn scale_only_when_nat_differs_from_render() {
        // nat half the render size -> sx = render/nat = 2, full-canvas placement
        // means the source quad scales 2x. topLeft (0,0).
        let t = Transform::from_top_left(Point { x: 0.0, y: 0.0 }, 1.0, 1.0);
        let rs = RenderSize::new(1920, 1080);
        let m = affine_transform(&t, (960.0, 540.0), rs);
        approx_affine(m, [2.0, 0.0, 0.0, 2.0, 0.0, 0.0]);
    }

    #[test]
    fn flip_horizontal_negates_sx_and_offsets_tx() {
        // SPEC §1.3: sx negative; tx = (topLeft.x + width) * render.w.
        let mut t = Transform::from_top_left(Point { x: 0.0, y: 0.0 }, 1.0, 1.0);
        t.flip_horizontal = true;
        let rs = RenderSize::new(1920, 1080);
        let m = affine_transform(&t, (1920.0, 1080.0), rs);
        // sx = (1920/1920)*1*(-1) = -1; tx = (0 + 1)*1920 = 1920.
        approx(m[0], -1.0);
        approx(m[4], 1920.0);
        approx(m[3], 1.0);
        approx(m[5], 0.0);
    }

    #[test]
    fn flip_vertical_negates_sy_and_offsets_ty() {
        let mut t = Transform::from_top_left(Point { x: 0.0, y: 0.0 }, 1.0, 1.0);
        t.flip_vertical = true;
        let rs = RenderSize::new(1920, 1080);
        let m = affine_transform(&t, (1920.0, 1080.0), rs);
        approx(m[3], -1.0);
        approx(m[5], 1080.0);
        approx(m[0], 1.0);
        approx(m[4], 0.0);
    }

    #[test]
    fn rotation_90_at_center_matches_hand_computed_matrix() {
        // `affine_transform` maps SOURCE-pixel coordinates ([0,natW]x[0,natH],
        // upstream AVFoundation layer-instruction semantics) to canvas pixels.
        // Centered, full-canvas clip (nat == render == 100x100) rotated 90 deg
        // about the canvas center (0.5,0.5 -> pixel 50,50).
        let rs = RenderSize::new(100, 100);
        let mut t = Transform::from_top_left(Point { x: 0.0, y: 0.0 }, 1.0, 1.0);
        t.rotation = 90.0;
        let m = affine_transform(&t, (100.0, 100.0), rs);

        // Expected: placed = identity (sx=sy=1, tx=ty=0).
        // then ∘ translate(-50,-50) ∘ rotate(90) ∘ translate(50,50).
        let placed = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let s1 = compose(placed, translate(-50.0, -50.0));
        let s2 = compose(s1, rotate(std::f64::consts::FRAC_PI_2));
        let expected = compose(s2, translate(50.0, 50.0));
        approx_affine(m, expected);

        // The SOURCE center pixel (50,50) stays at the canvas center under pure
        // rotation about that center.
        let (cx, cy) = apply_point(m, 50.0, 50.0);
        approx(cx, 50.0);
        approx(cy, 50.0);

        // A source corner rotates: (100,100) -> rotate 90 about center -> (0,100).
        let (qx, qy) = apply_point(m, 100.0, 100.0);
        approx(qx, 0.0);
        approx(qy, 100.0);
    }

    #[test]
    fn full_canvas_quad_maps_source_pixels_to_canvas_pixels() {
        // The whole point of `affine_transform`: a full-canvas clip maps the
        // source corner (natW,natH) to the canvas corner (render.w,render.h).
        let rs = RenderSize::new(1920, 1080);
        let t = Transform::from_top_left(Point { x: 0.0, y: 0.0 }, 1.0, 1.0);
        let m = affine_transform(&t, (640.0, 480.0), rs); // arbitrary source size
        let (x, y) = apply_point(m, 640.0, 480.0);
        approx(x, 1920.0);
        approx(y, 1080.0);
        // Source origin maps to canvas origin.
        let (ox, oy) = apply_point(m, 0.0, 0.0);
        approx(ox, 0.0);
        approx(oy, 0.0);
    }

    #[test]
    fn crop_to_uv_identity_is_full_texture() {
        let uv = crop_to_uv(Crop::default());
        assert_eq!(uv, (0.0, 0.0, 1.0, 1.0));
    }

    #[test]
    fn crop_to_uv_insets() {
        let c = Crop {
            left: 0.1,
            top: 0.2,
            right: 0.15,
            bottom: 0.25,
        };
        let uv = crop_to_uv(c);
        approx(uv.0, 0.1);
        approx(uv.1, 0.2);
        approx(uv.2, 1.0 - 0.15);
        approx(uv.3, 1.0 - 0.25);
    }

    #[test]
    fn crop_to_uv_overinset_clamps_without_inversion() {
        // left+right > 1: domain visible fraction is 0; UV must not invert.
        let c = Crop {
            left: 0.7,
            top: 0.0,
            right: 0.7,
            bottom: 0.0,
        };
        let uv = crop_to_uv(c);
        assert!(uv.0 <= uv.2, "u0 {} must be <= u1 {}", uv.0, uv.2);
        assert!(uv.1 <= uv.3);
        // u1 = 1 - 0.7 = 0.3, u0 = 0.7 -> after min/max: (0.3, _, 0.7, _)
        approx(uv.0, 0.3);
        approx(uv.2, 0.7);
    }
}
