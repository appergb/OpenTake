// Frame compositor shader. One textured quad per LayerDraw, alpha-over.
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

// Laid out as four vec4s so every field is 16-byte aligned (no implicit WGSL
// padding) and the Rust POD mirror is unambiguous.
struct U {
    affine0: vec4<f32>,   // a, b, c, d
    crop_uv: vec4<f32>,   // u0, v0, u1, v1
    // affine1 (tx,ty) + nat (w,h)
    affine1_nat: vec4<f32>,
    // canvas (w,h) + opacity + flags(bitcast to f32)
    canvas_op_flags: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var t_color: texture_2d<f32>;
@group(0) @binding(2) var s_color: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

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

    var out: VsOut;
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    var c = textureSample(t_color, s_color, in.uv);

    let opacity = u.canvas_op_flags.z;
    let flags = bitcast<u32>(u.canvas_op_flags.w);

    // Straight-alpha source -> premultiply (SPEC §3.6). Image/text/Lottie are
    // already premultiplied, so the flag is clear for them.
    if ((flags & 1u) != 0u) {
        c = vec4<f32>(c.rgb * c.a, c.a);
    }

    // Global opacity scales premultiplied rgb and a together.
    return c * opacity;
}
