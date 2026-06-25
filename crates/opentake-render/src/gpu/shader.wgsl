// Frame compositor shader. One textured quad per LayerDraw, alpha-over, with the
// A-tier per-pixel chain (color grade -> chroma key -> masks) applied in the
// fragment stage BEFORE premultiply + global opacity.
//
// PROJECTION CONVENTION (the pixel-diff lifeline, SPEC §1.3/§3.3):
//   - The quad spans [0,1]^2; scaling by `nat` yields SOURCE-pixel coordinates
//     [0,natW]x[0,natH]. Upstream AVFoundation layer-instruction transforms act
//     on this source-pixel space (verified against affineTransform L599).
//   - `affine` (row-major [a,b,c,d,tx,ty], CG semantics p' = p . M) maps source
//     pixels -> CANVAS pixels, origin bottom-left, y up.
//   - Canvas pixels -> NDC. wgpu's NDC y is up, so no extra y-flip on geometry.
//   - The single y-flip reconciling "texture row 0 = top" with "y up" happens on
//     the UV (v = 1 - v), exactly once (SPEC §3.4).
//
// COLOR / CHROMA / MASK MATH MIRROR:
//   The pixel math here is a 1:1 mirror of the unit-tested reference in
//   `opentake_domain::grade` (ColorGrade::apply_linear, ChromaKey::alpha /
//   suppress_spill, Mask::coverage). The PoC composites in the sRGB non-linear
//   domain (SPEC §3.7); the color grade is defined in LINEAR light, so we decode
//   sRGB -> linear around the grade and re-encode. Chroma key and masks operate
//   on the (sampled) color directly, matching the domain reference which is
//   space-agnostic for those stages.
//
// MASK CAP: up to MASK_CAP masks are evaluated in-shader (linear + circle SDF).
// Polygon masks are carried in the domain/plan and fully unit-tested there, but
// their variable-length point list does not fit this fixed uniform; wiring
// polygon points through a storage buffer is a documented render-side TODO.

const MASK_CAP: u32 = 4u;

// Flag bits packed into U.canvas_op_flags.w (bitcast to u32).
const FLAG_PREMULTIPLY: u32 = 1u;   // straight-alpha source needs premultiply
const FLAG_GRADE: u32 = 2u;         // color grade active
const FLAG_CHROMA: u32 = 4u;        // chroma key active

// Mask kind tags (mirror MaskShape; poly is not evaluated in-shader).
const MASK_LINEAR: u32 = 0u;
const MASK_CIRCLE: u32 = 1u;

struct MaskGpu {
    // (kind-as-f32, feather, invert-as-f32, pad)
    head: vec4<f32>,
    // linear: (point.x, point.y, normal.x, normal.y)
    // circle: (center.x, center.y, radius.x, radius.y)
    geo: vec4<f32>,
};

// Laid out as vec4s so every field is 16-byte aligned (no implicit WGSL padding)
// and the Rust POD mirror is unambiguous.
struct U {
    affine0: vec4<f32>,        // a, b, c, d
    crop_uv: vec4<f32>,        // u0, v0, u1, v1
    affine1_nat: vec4<f32>,    // tx, ty, natW, natH
    canvas_op_flags: vec4<f32>, // canvasW, canvasH, opacity, flags(bitcast f32)
    // Color grade (white balance pre-multiplied to per-channel gain on the CPU).
    grade_exp_wb: vec4<f32>,   // exposure(stops), wb_r, wb_g, wb_b
    grade_lift: vec4<f32>,     // lift_r, lift_g, lift_b, contrast
    grade_gamma: vec4<f32>,    // gamma_r, gamma_g, gamma_b, saturation
    grade_gain: vec4<f32>,     // gain_r, gain_g, gain_b, pad
    // Chroma key.
    chroma0: vec4<f32>,        // key_r, key_g, key_b, similarity
    chroma1: vec4<f32>,        // smoothness, spill, pad, pad
    // Mask count (x) + padding.
    mask_meta: vec4<f32>,      // mask_count, pad, pad, pad
    masks: array<MaskGpu, MASK_CAP>,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var t_color: texture_2d<f32>;
@group(0) @binding(2) var s_color: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // Normalized canvas position (0..1) of this fragment, for mask evaluation.
    @location(1) canvas_uv: vec2<f32>,
};

// ---- BT.709 luma + sRGB <-> linear (mirror of opentake_domain::grade) -------

fn luma709(c: vec3<f32>) -> f32 {
    return 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

fn smoothstep01(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge0 == edge1) {
        return select(0.0, 1.0, x >= edge0);
    }
    let t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

// ---- Color grade (linear-light chain) ---------------------------------------

const CONTRAST_PIVOT: f32 = 0.18;

fn apply_channel_lgg(x: f32, lift: f32, gamma: f32, gain: f32) -> f32 {
    let v = gain * (x + lift);
    if (abs(gamma - 1.0) > 1e-6 && gamma > 0.0) {
        return pow(max(v, 0.0), 1.0 / gamma);
    }
    return v;
}

// Applies the grade to a LINEAR-rgb triple, returning clamped linear rgb. Mirror
// of ColorGrade::apply_linear (exposure -> wb -> lgg -> contrast -> saturation).
fn apply_grade_linear(rgb_in: vec3<f32>) -> vec3<f32> {
    var c = rgb_in;

    // 1. Exposure (linear gain 2^stops).
    let exposure = u.grade_exp_wb.x;
    c = c * exp2(exposure);

    // 2. White balance (per-channel gain, precomputed CPU-side).
    c = c * u.grade_exp_wb.yzw;

    // 3. Lift / gamma / gain.
    let lift = u.grade_lift.xyz;
    let gamma = u.grade_gamma.xyz;
    let gain = u.grade_gain.xyz;
    c = vec3<f32>(
        apply_channel_lgg(c.r, lift.r, gamma.r, gain.r),
        apply_channel_lgg(c.g, lift.g, gamma.g, gain.g),
        apply_channel_lgg(c.b, lift.b, gamma.b, gain.b),
    );

    // 4. Contrast around the 0.18 pivot.
    let contrast = u.grade_lift.w;
    let slope = 1.0 + contrast;
    c = (c - vec3<f32>(CONTRAST_PIVOT)) * slope + vec3<f32>(CONTRAST_PIVOT);

    // 5. Saturation (luma-preserving lerp toward grey).
    let saturation = u.grade_gamma.w;
    let l = luma709(c);
    c = vec3<f32>(l) + (c - vec3<f32>(l)) * saturation;

    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

// ---- Chroma key (mirror of ChromaKey) ---------------------------------------

fn chroma_cb_cr(c: vec3<f32>) -> vec2<f32> {
    let y = luma709(c);
    let inv = 1.0 / (y + 1e-4);
    return vec2<f32>((c.b - y) * inv, (c.r - y) * inv);
}

fn chroma_alpha(c: vec3<f32>) -> f32 {
    let key = u.chroma0.xyz;
    let similarity = u.chroma0.w;
    let smoothness = max(u.chroma1.x, 0.0);
    let kc = chroma_cb_cr(key);
    let pc = chroma_cb_cr(c);
    let dist = length(pc - kc);
    return smoothstep01(similarity, similarity + smoothness, dist);
}

fn suppress_spill(c: vec3<f32>) -> vec3<f32> {
    let spill = clamp(u.chroma1.y, 0.0, 1.0);
    if (spill <= 0.0) {
        return c;
    }
    let key = u.chroma0.xyz;
    // Green key (common case): suppress green above the r/b average.
    if (key.g >= key.r && key.g >= key.b) {
        let avg = (c.r + c.b) * 0.5;
        let ng = select(c.g, avg + (c.g - avg) * (1.0 - spill), c.g > avg);
        return vec3<f32>(c.r, ng, c.b);
    } else if (key.b >= key.r && key.b >= key.g) {
        let avg = (c.r + c.g) * 0.5;
        let nb = select(c.b, avg + (c.b - avg) * (1.0 - spill), c.b > avg);
        return vec3<f32>(c.r, c.g, nb);
    }
    let avg = (c.g + c.b) * 0.5;
    let nr = select(c.r, avg + (c.r - avg) * (1.0 - spill), c.r > avg);
    return vec3<f32>(nr, c.g, c.b);
}

// ---- Masks (mirror of Mask::coverage; linear + circle in-shader) ------------

fn mask_signed_distance(m: MaskGpu, p: vec2<f32>) -> f32 {
    let kind = u32(m.head.x + 0.5);
    if (kind == MASK_LINEAR) {
        let point = m.geo.xy;
        let normal = m.geo.zw;
        let nlen = length(normal);
        if (nlen <= 1e-6) {
            return 0.0;
        }
        let n = normal / nlen;
        return -dot(p - point, n);
    }
    // Circle (default for any other tag).
    let center = m.geo.xy;
    let radius = max(m.geo.zw, vec2<f32>(1e-6));
    let d = length((p - center) / radius);
    return (d - 1.0) * min(radius.x, radius.y);
}

fn mask_coverage(m: MaskGpu, p: vec2<f32>) -> f32 {
    let sd = mask_signed_distance(m, p);
    let feather = max(m.head.y, 0.0);
    var inside: f32;
    if (feather <= 1e-6) {
        inside = select(0.0, 1.0, sd <= 0.0);
    } else {
        inside = 1.0 - smoothstep01(-feather * 0.5, feather * 0.5, sd);
    }
    let invert = m.head.z > 0.5;
    return select(inside, 1.0 - inside, invert);
}

// Combined coverage = product of every active mask (intersection).
fn masks_coverage(p: vec2<f32>) -> f32 {
    let count = min(u32(u.mask_meta.x + 0.5), MASK_CAP);
    var cov = 1.0;
    for (var i: u32 = 0u; i < count; i = i + 1u) {
        cov = cov * mask_coverage(u.masks[i], p);
    }
    return cov;
}

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VsOut {
    // Triangle-strip quad: (0,0) (1,0) (0,1) (1,1).
    var quad = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
    );
    let q = quad[vi];

    let affine1 = u.affine1_nat.xy;   // tx, ty
    let nat = u.affine1_nat.zw;       // source natural size
    let canvas = u.canvas_op_flags.xy;

    // Quad [0,1] -> source pixels [0,nat].
    let src = q * nat;

    // Source pixels -> canvas pixels via the row-vector affine p' = p . M.
    let px = vec2<f32>(
        src.x * u.affine0.x + src.y * u.affine0.z + affine1.x,
        src.x * u.affine0.y + src.y * u.affine0.w + affine1.y,
    );

    // Canvas pixels (origin bottom-left, y up) -> NDC.
    let ndc = vec2<f32>(
        px.x / canvas.x * 2.0 - 1.0,
        px.y / canvas.y * 2.0 - 1.0,
    );

    // UV: quad corner -> crop sub-rect. Flip v once (texture row 0 = top).
    let uv_lin = mix(u.crop_uv.xy, u.crop_uv.zw, q);
    let uv = vec2<f32>(uv_lin.x, 1.0 - uv_lin.y);

    // Normalized canvas position for mask evaluation. Masks are authored with
    // origin TOP-left, y down (same as the source/UI canvas), so flip y from the
    // NDC's bottom-left origin.
    let canvas_uv = vec2<f32>(px.x / canvas.x, 1.0 - px.y / canvas.y);

    var out: VsOut;
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = uv;
    out.canvas_uv = canvas_uv;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let sampled = textureSample(t_color, s_color, in.uv);

    let flags = bitcast<u32>(u.canvas_op_flags.w);

    // Work on STRAIGHT (non-premultiplied) color so chroma/grade/mask math is
    // unambiguous. The source is either already straight (FLAG_PREMULTIPLY clear)
    // or premultiplied (set) — un-premultiply the latter, run the chain, then
    // premultiply once at the end. This keeps the whole A-tier chain correct
    // regardless of source alpha state.
    var rgb = sampled.rgb;
    var alpha = sampled.a;
    if ((flags & FLAG_PREMULTIPLY) == 0u) {
        // Source was already premultiplied (no premultiply requested) -> recover
        // straight rgb for the chain. (Guard divide-by-zero.)
        if (alpha > 1e-6) {
            rgb = rgb / alpha;
        }
    }
    // else: FLAG_PREMULTIPLY set means the source is STRAIGHT alpha and must be
    // premultiplied by us; rgb is already straight, nothing to undo.

    // 1. Chroma key (matte from straight source color; suppress spill).
    if ((flags & FLAG_CHROMA) != 0u) {
        alpha = alpha * chroma_alpha(rgb);
        rgb = suppress_spill(rgb);
    }

    // 2. Color grade (defined in linear light; decode/encode around it).
    if ((flags & FLAG_GRADE) != 0u) {
        let lin = srgb_to_linear(rgb);
        let graded = apply_grade_linear(lin);
        rgb = linear_to_srgb(graded);
    }

    // 3. Masks (intersected coverage) scale alpha.
    alpha = alpha * masks_coverage(in.canvas_uv);

    // Premultiply once (the compositor blends premultiplied over), then apply the
    // global opacity (which scales premultiplied rgb and a together).
    let out = vec4<f32>(rgb * alpha, alpha);
    return out * u.canvas_op_flags.z;
}
