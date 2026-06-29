//! wgpu device / queue acquisition (SPEC §3) with runtime capability detection.
//!
//! On macOS this targets Metal. The compositor's smoke test calls
//! [`RenderDevice::try_new`] and SKIPS gracefully when no adapter/device is
//! available (CI without a GPU, headless sandboxes), so tests never hard-fail on
//! GPU absence — the host-capability gate, not a `should_panic`.

use crate::gpu::RenderError;

/// A ready-to-use GPU device + queue for offscreen frame compositing.
pub struct RenderDevice {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl RenderDevice {
    /// Try to acquire a device. Returns `Err` (never panics) when no suitable
    /// adapter or device exists, so callers can skip GPU work cleanly.
    pub fn try_new() -> Result<Self, RenderError> {
        // Keep the backend surface lean; the crate only enables wgsl + metal.
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok_or(RenderError::NoAdapter)?;

        // Request the adapter's REAL limits, not `downlevel_defaults()`. The
        // downlevel baseline caps `max_texture_dimension_2d` at 2048, which is
        // fine for the downscaled preview but makes the FULL-resolution export
        // render target (FHD is borderline, 2K/4K exceed it) fail inside
        // `Device::create_texture` with an uncaptured wgpu error that panics —
        // i.e. "export video" aborted the whole app. Native Metal/Vulkan/DX12
        // report 16384 here, covering every realistic export resolution.
        let required_limits = adapter.limits();

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("opentake-render device"),
                required_features: wgpu::Features::empty(),
                required_limits,
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|e| RenderError::DeviceRequest(e.to_string()))?;

        Ok(RenderDevice { device, queue })
    }
}
