//! Texture upload + a small content-hash LRU cache (SPEC §4.4).
//!
//! Images / text / Lottie frames are keyed by an opaque content hash (computed
//! by the caller — render adds no hashing dependency) and capped by an LRU so
//! VRAM doesn't grow unbounded. Video frames are NOT long-lived here; the
//! compositor uploads the current frame on demand.

use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use crate::source::DecodedFrame;

/// A GPU texture plus a bindable view, reference-counted so a cache entry and an
/// in-flight draw can share it.
pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

/// Upload a [`DecodedFrame`] as an RGBA8 texture.
///
/// `srgb` selects the texture format: `Rgba8UnormSrgb` makes the sampler return
/// linear values (hardware sRGB decode); `Rgba8Unorm` keeps raw bytes. The PoC
/// composites in the sRGB non-linear domain (SPEC §3.7), so callers pass
/// `srgb = false` to sample raw encoded bytes and blend them directly.
pub fn upload_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    frame: &DecodedFrame,
    srgb: bool,
    label: Option<&str>,
) -> GpuTexture {
    let format = if srgb {
        wgpu::TextureFormat::Rgba8UnormSrgb
    } else {
        wgpu::TextureFormat::Rgba8Unorm
    };
    let size = wgpu::Extent3d {
        width: frame.width,
        height: frame.height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &frame.rgba,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(frame.width * 4),
            rows_per_image: Some(frame.height),
        },
        size,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    GpuTexture {
        texture,
        view,
        width: frame.width,
        height: frame.height,
    }
}

/// LRU cache mapping an opaque content-hash key to a shared [`GpuTexture`].
pub struct TextureCache {
    capacity: usize,
    map: HashMap<String, Rc<GpuTexture>>,
    order: VecDeque<String>,
}

impl TextureCache {
    /// New cache holding at most `capacity` textures (>= 1).
    pub fn new(capacity: usize) -> Self {
        TextureCache {
            capacity: capacity.max(1),
            map: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Fetch a cached texture, marking it most-recently-used.
    pub fn get(&mut self, key: &str) -> Option<Rc<GpuTexture>> {
        if self.map.contains_key(key) {
            self.touch(key);
            self.map.get(key).cloned()
        } else {
            None
        }
    }

    /// Insert (or replace) a texture, evicting the least-recently-used entry when
    /// over capacity. Returns the shared handle.
    pub fn insert(&mut self, key: impl Into<String>, tex: GpuTexture) -> Rc<GpuTexture> {
        let key = key.into();
        let rc = Rc::new(tex);
        if self.map.insert(key.clone(), rc.clone()).is_some() {
            self.touch(&key);
        } else {
            self.order.push_back(key.clone());
            while self.map.len() > self.capacity {
                if let Some(evict) = self.order.pop_front() {
                    self.map.remove(&evict);
                } else {
                    break;
                }
            }
        }
        rc
    }

    fn touch(&mut self, key: &str) {
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            let k = self.order.remove(pos).expect("position valid");
            self.order.push_back(k);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // GpuTexture needs a device; the cache eviction logic is tested via a
    // device-backed smoke test in tests/. Here we only test the key bookkeeping
    // by constructing the cache and checking capacity math indirectly is not
    // possible without textures — so the cache's eviction is covered in the GPU
    // smoke test (conditionally). This unit test just guards `new`'s clamp.
    #[test]
    fn capacity_floored_at_one() {
        let c = TextureCache::new(0);
        assert_eq!(c.capacity, 1);
        assert!(c.is_empty());
    }
}
