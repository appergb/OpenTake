//! wgpu frame compositor (SPEC §3.1): for each `LayerDraw`, draw a transformed
//! textured quad and alpha-over it onto the canvas render target, then read the
//! target back as RGBA8.
//!
//! One render pipeline; per draw we swap a bind group (texture + uniform). The
//! quad is 4 constant vertices — all geometry lives in the uniform affine.

use std::rc::Rc;

use bytemuck::{Pod, Zeroable};

use opentake_domain::{ColorGrade, LiftGammaGain, MaskShape};

use crate::gpu::texture::GpuTexture;
use crate::gpu::RenderError;
use crate::plan::{FramePlan, LayerDraw, RenderSize, TextureSource};
use crate::source::DecodedFrame;

/// Maximum masks evaluated in-shader per draw (mirrors `MASK_CAP` in
/// `shader.wgsl`). Extra masks on a clip beyond this are ignored by the
/// compositor (the domain still stores and unit-tests all of them).
const MASK_CAP: usize = 4;

/// Flag bits packed into `canvas_op_flags[3]` (bitcast to u32 in WGSL).
const FLAG_PREMULTIPLY: u32 = 1;
const FLAG_GRADE: u32 = 2;
const FLAG_CHROMA: u32 = 4;

/// Mask kind tags (mirror `MaskShape` / the WGSL `MASK_*` consts). Polygon masks
/// are not rendered in-shader (see shader TODO); they encode as `MASK_NOOP` which
/// the shader treats as a full-coverage circle (no clipping).
const MASK_LINEAR: f32 = 0.0;
const MASK_CIRCLE: f32 = 1.0;
/// A circle large enough to cover the whole canvas — used to make an unsupported
/// (polygon) mask a no-op instead of silently clipping.
const MASK_NOOP_GEO: [f32; 4] = [0.5, 0.5, 8.0, 8.0];

/// One mask in the uniform (mirrors WGSL `MaskGpu`): `head = (kind, feather,
/// invert, pad)`, `geo` packs the shape geometry.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default)]
struct MaskGpu {
    head: [f32; 4],
    geo: [f32; 4],
}

/// Uniform mirror of WGSL `struct U` (SPEC §3.2), extended with the A-tier color
/// grade / chroma key / mask parameters. Field order + vec4 alignment match the
/// WGSL struct exactly.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    affine0: [f32; 4],         // a, b, c, d
    crop_uv: [f32; 4],         // u0, v0, u1, v1
    affine1_nat: [f32; 4],     // tx, ty, natW, natH
    canvas_op_flags: [f32; 4], // canvasW, canvasH, opacity, flags-as-f32
    grade_exp_wb: [f32; 4],    // exposure, wb_r, wb_g, wb_b
    grade_lift: [f32; 4],      // lift_r, lift_g, lift_b, contrast
    grade_gamma: [f32; 4],     // gamma_r, gamma_g, gamma_b, saturation
    grade_gain: [f32; 4],      // gain_r, gain_g, gain_b, pad
    chroma0: [f32; 4],         // key_r, key_g, key_b, similarity
    chroma1: [f32; 4],         // smoothness, spill, pad, pad
    mask_meta: [f32; 4],       // mask_count, pad, pad, pad
    masks: [MaskGpu; MASK_CAP],
}

/// Identity color-grade uniform block (exposure 0, wb/gain 1, lift 0, gamma 1,
/// contrast 0, saturation 1). Used when a draw has no grade.
fn identity_grade_blocks() -> ([f32; 4], [f32; 4], [f32; 4], [f32; 4]) {
    (
        [0.0, 1.0, 1.0, 1.0], // exposure, wb
        [0.0, 0.0, 0.0, 0.0], // lift, contrast
        [1.0, 1.0, 1.0, 1.0], // gamma, saturation
        [1.0, 1.0, 1.0, 0.0], // gain, pad
    )
}

/// Pack a [`ColorGrade`] into the four uniform vec4 blocks the shader reads. The
/// white balance is resolved to per-channel gain CPU-side (the shader multiplies
/// it directly), keeping the WGSL mirror of `ColorGrade::apply_linear` simple.
fn grade_blocks(g: &ColorGrade) -> ([f32; 4], [f32; 4], [f32; 4], [f32; 4]) {
    let wb = g.white_balance_gain();
    let LiftGammaGain { lift, gamma, gain } = g.lift_gamma_gain;
    (
        [g.exposure as f32, wb.r as f32, wb.g as f32, wb.b as f32],
        [
            lift.r as f32,
            lift.g as f32,
            lift.b as f32,
            g.contrast as f32,
        ],
        [
            gamma.r as f32,
            gamma.g as f32,
            gamma.b as f32,
            g.saturation as f32,
        ],
        [gain.r as f32, gain.g as f32, gain.b as f32, 0.0],
    )
}

/// Pack a draw's masks into the fixed-capacity uniform array, returning the count
/// the shader should evaluate. Linear + circle masks encode directly; polygon
/// masks (unsupported in-shader) encode as a full-coverage no-op so they neither
/// clip nor crash. Masks beyond [`MASK_CAP`] are dropped.
fn pack_masks(draw: &LayerDraw<'_>) -> ([MaskGpu; MASK_CAP], f32) {
    let mut out = [MaskGpu::default(); MASK_CAP];
    let mut n = 0usize;
    for mask in draw.masks.iter() {
        if n >= MASK_CAP {
            break;
        }
        let invert = if mask.invert { 1.0 } else { 0.0 };
        let (kind, geo) = match &mask.shape {
            MaskShape::Linear { point, normal } => (
                MASK_LINEAR,
                [
                    point.x as f32,
                    point.y as f32,
                    normal.x as f32,
                    normal.y as f32,
                ],
            ),
            MaskShape::Circle { center, radius } => (
                MASK_CIRCLE,
                [
                    center.x as f32,
                    center.y as f32,
                    radius.x as f32,
                    radius.y as f32,
                ],
            ),
            // Polygon masks are unsupported in-shader (TODO: storage buffer for
            // points). Encode as a full-canvas circle so they are a visual no-op
            // rather than silently clipping.
            MaskShape::Poly { .. } => (MASK_CIRCLE, MASK_NOOP_GEO),
        };
        out[n] = MaskGpu {
            head: [kind, mask.feather as f32, invert, 0.0],
            geo,
        };
        n += 1;
    }
    (out, n as f32)
}

/// Working color format. The PoC composites in the sRGB non-linear domain
/// (SPEC §3.7): an `Rgba8Unorm` target stores raw encoded bytes and blends them
/// directly, matching AVFoundation most closely. Read-back returns those bytes.
const RT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Resolves a draw's [`TextureSource`] + source frame to a GPU texture. The
/// compositor is decode-agnostic; the integrating layer (or a test) supplies
/// pixels (e.g. via [`crate::source::FrameProvider`] + a cache).
pub trait TextureResolver {
    fn resolve(&mut self, source: &TextureSource, source_frame: i64) -> Option<Rc<GpuTexture>>;
}

/// A textured-quad compositor bound to one device.
pub struct Compositor {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl Compositor {
    /// Build the pipeline, sampler, and bind-group layout.
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("opentake-render compositor shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("opentake-render bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("opentake-render pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Premultiplied alpha-over (SPEC §3.6): src + dst*(1-src.a) for both
        // color and alpha.
        let blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("opentake-render compositor pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: RT_FORMAT,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("opentake-render sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Compositor {
            pipeline,
            bind_group_layout,
            sampler,
        }
    }

    /// Render one frame to an offscreen RGBA8 target and read it back.
    ///
    /// Clears to `frame_plan.clear_rgba` (opaque black), then composites each
    /// draw in order (later = on top). Draws whose texture can't be resolved are
    /// skipped (offline/unprocessable sources contribute nothing, mirroring
    /// upstream's offline handling).
    pub fn render_to_rgba(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: RenderSize,
        frame_plan: &FramePlan<'_>,
        resolver: &mut dyn TextureResolver,
    ) -> Result<DecodedFrame, RenderError> {
        let rt = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("opentake-render target"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: RT_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let rt_view = rt.create_view(&wgpu::TextureViewDescriptor::default());

        // Resolve textures + build uniforms/bind groups up front (keeps the
        // render pass borrow-clean). Hold the Rc textures alive for the pass.
        struct Prepared {
            bind_group: wgpu::BindGroup,
            _tex: Rc<GpuTexture>,
        }
        let mut prepared: Vec<Prepared> = Vec::with_capacity(frame_plan.draws.len());

        for draw in &frame_plan.draws {
            let Some(tex) = resolver.resolve(draw.source, draw.source_frame) else {
                continue;
            };
            // Assemble flags + the A-tier parameter blocks for this draw.
            let mut flags: u32 = if draw.needs_premultiply {
                FLAG_PREMULTIPLY
            } else {
                0
            };
            let (grade_exp_wb, grade_lift, grade_gamma, grade_gain) = match draw.color_grade {
                Some(g) if !g.is_identity() => {
                    flags |= FLAG_GRADE;
                    grade_blocks(g)
                }
                _ => identity_grade_blocks(),
            };
            let (chroma0, chroma1) = match draw.chroma_key {
                Some(k) => {
                    flags |= FLAG_CHROMA;
                    (
                        [
                            k.key_color.r as f32,
                            k.key_color.g as f32,
                            k.key_color.b as f32,
                            k.similarity as f32,
                        ],
                        [k.smoothness as f32, k.spill as f32, 0.0, 0.0],
                    )
                }
                None => ([0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0]),
            };
            let (masks, mask_count) = pack_masks(draw);
            let u = Uniforms {
                affine0: [
                    draw.affine[0] as f32,
                    draw.affine[1] as f32,
                    draw.affine[2] as f32,
                    draw.affine[3] as f32,
                ],
                crop_uv: [
                    draw.crop_uv.0 as f32,
                    draw.crop_uv.1 as f32,
                    draw.crop_uv.2 as f32,
                    draw.crop_uv.3 as f32,
                ],
                affine1_nat: [
                    draw.affine[4] as f32,
                    draw.affine[5] as f32,
                    // Source natural size the affine was built with — NOT the
                    // decoded texture resolution. The preview decodes at a
                    // downscaled max_size, so `tex.width/height` here mismatched
                    // the affine's nat and rendered the layer shrunk into the
                    // bottom-left corner, jittering as the texture size varied
                    // (#125). UV (crop_uv) samples the texture 0..1, so its actual
                    // pixel size is irrelevant to geometry.
                    draw.nat_size.0 as f32,
                    draw.nat_size.1 as f32,
                ],
                canvas_op_flags: [
                    size.width as f32,
                    size.height as f32,
                    draw.opacity as f32,
                    f32::from_bits(flags),
                ],
                grade_exp_wb,
                grade_lift,
                grade_gamma,
                grade_gain,
                chroma0,
                chroma1,
                mask_meta: [mask_count, 0.0, 0.0, 0.0],
                masks,
            };
            let ubuf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("opentake-render uniform"),
                size: std::mem::size_of::<Uniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&ubuf, 0, bytemuck::bytes_of(&u));

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("opentake-render bind group"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: ubuf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&tex.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            prepared.push(Prepared {
                bind_group,
                _tex: tex,
            });
        }

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let [r, g, b, a] = frame_plan.clear_rgba;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("opentake-render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &rt_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            for p in &prepared {
                pass.set_bind_group(0, &p.bind_group, &[]);
                pass.draw(0..4, 0..1);
            }
        }

        let frame = read_back(device, queue, &mut encoder, &rt, size)?;
        queue.submit(Some(encoder.finish()));
        // `read_back` mapped the staging buffer after submit via poll; finalize.
        frame.finish(device)
    }
}

/// Holds the staging buffer until its contents are mapped and copied out.
struct PendingReadback {
    buffer: wgpu::Buffer,
    size: RenderSize,
    padded_bytes_per_row: u32,
}

impl PendingReadback {
    fn finish(self, device: &wgpu::Device) -> Result<DecodedFrame, RenderError> {
        let slice = self.buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .map_err(|_| RenderError::Readback("map channel closed".into()))?
            .map_err(|e| RenderError::Readback(e.to_string()))?;

        let data = slice.get_mapped_range();
        let row_bytes = self.size.width as usize * 4;
        let mut rgba = vec![0u8; row_bytes * self.size.height as usize];
        for y in 0..self.size.height as usize {
            let src = y * self.padded_bytes_per_row as usize;
            let dst = y * row_bytes;
            rgba[dst..dst + row_bytes].copy_from_slice(&data[src..src + row_bytes]);
        }
        drop(data);
        self.buffer.unmap();
        Ok(DecodedFrame::new(
            self.size.width,
            self.size.height,
            rgba,
            // Compositor output is premultiplied (alpha-over result).
            true,
        ))
    }
}

/// Encode the RT -> buffer copy (256-aligned rows) and return a pending readback
/// to be finalized after `queue.submit`.
fn read_back(
    device: &wgpu::Device,
    _queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    rt: &wgpu::Texture,
    size: RenderSize,
) -> Result<PendingReadback, RenderError> {
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let unpadded = size.width * 4;
    let padded = unpadded.div_ceil(align) * align;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("opentake-render readback"),
        size: (padded * size.height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: rt,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(size.height),
            },
        },
        wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
    );
    Ok(PendingReadback {
        buffer,
        size,
        padded_bytes_per_row: padded,
    })
}
