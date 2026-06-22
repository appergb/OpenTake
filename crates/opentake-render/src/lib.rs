//! opentake-render — rendering pipeline.
//!
//! RenderPlan (pure function: Timeline -> per-frame composition instructions)
//! + wgpu frame compositor + ffmpeg codec backends (preview + export share one plan).

pub mod gpu;
pub mod plan;
pub mod size;
pub mod source;

/// Re-export of the `wgpu` version this crate is built against, so callers
/// (preview/export backends, integration tests) name device/queue/texture types
/// without depending on `wgpu` directly or risking a version mismatch.
pub use wgpu;

pub use plan::{
    affine_transform, build_render_plan, compose, crop_to_uv, source_frame_index, ClipPlan,
    FramePlan, LayerDraw, RenderPlan, RenderSize, TextureSource,
};
pub use size::{even, export_render_size, ExportResolution};
pub use source::{DecodedFrame, FrameProvider, SourceMetrics};

pub use gpu::{
    Compositor, CosmicTextRasterizer, GpuTexture, NullTextRasterizer, RenderDevice, RenderError,
    TextRasterRequest, TextRasterizer, TextureCache, TextureResolver,
};
