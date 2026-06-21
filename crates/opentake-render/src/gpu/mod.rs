//! GPU frame compositor (SPEC §3). wgpu render graph that turns a
//! [`FramePlan`](crate::plan::FramePlan) into an RGBA8 canvas frame: upload
//! source textures, draw a transformed quad per layer, alpha-over, read back.
//!
//! On macOS this runs on Metal. Device acquisition is fallible (no panic) so
//! tests skip cleanly on hosts without a GPU.

pub mod color;
pub mod compositor;
pub mod device;
pub mod text_raster;
pub mod texture;

pub use color::{linear_to_srgb, srgb_to_linear};
pub use compositor::{Compositor, TextureResolver};
pub use device::RenderDevice;
pub use text_raster::{NullTextRasterizer, TextRasterRequest, TextRasterizer};
pub use texture::{upload_rgba, GpuTexture, TextureCache};

/// Errors from GPU device acquisition and frame compositing.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// No suitable GPU adapter on this host (CI / headless). Callers should skip.
    #[error("no suitable GPU adapter available")]
    NoAdapter,
    #[error("failed to request GPU device: {0}")]
    DeviceRequest(String),
    #[error("frame read-back failed: {0}")]
    Readback(String),
}
