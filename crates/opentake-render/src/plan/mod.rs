//! Pure `Timeline -> RenderPlan` planning (SPEC §2). Zero IO, fully unit-tested,
//! decoupled from the GPU. This is upstream `buildVisuals` re-expressed as
//! per-frame VALUES instead of AVFoundation ramp instructions.

pub mod affine;
pub mod build;
pub mod types;

#[cfg(test)]
mod tests;

pub use affine::{affine_transform, compose, crop_to_uv};
pub use build::{build_render_plan, source_frame_index};
pub use types::{ClipPlan, FramePlan, LayerDraw, RenderPlan, RenderSize, TextureSource};
