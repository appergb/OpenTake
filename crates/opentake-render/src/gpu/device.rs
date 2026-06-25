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

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("opentake-render device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|e| RenderError::DeviceRequest(e.to_string()))?;

        Ok(RenderDevice { device, queue })
    }
}
