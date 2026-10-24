//! Material textures on the GPU, for the Textured mode: one layer of a
//! texture array per material, each resized to the same square size, with
//! a small table of each texture's real size (texture coordinates are laid
//! out in its real pixels).
//!
//! The array is made only when the first texture arrives, so the Flat and
//! Wireframe modes, and maps without real textures, cost nothing.

mod layers;

use layers::{Layer, Layers};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

/// Side of every material texture layer, in pixels.
pub const MATERIAL_TEXTURE_SIZE: u32 = 256;
/// Most material textures kept at once (and at most what the GPU allows).
pub const MAX_MATERIAL_TEXTURES: u32 = 256;
/// Mip levels of a layer (256, 128… 1).
const MIP_LEVELS: u32 = 9;

/// The material textures and their table.
#[derive(Debug)]
pub(crate) struct MaterialTextures {
    layers: Layers,
    /// The array, once a texture has arrived; until then a 1-pixel stand-in.
    array: wgpu::Texture,
    real: bool,
    view: wgpu::TextureView,
    pub(crate) sampler: wgpu::Sampler,
    /// Per layer: real width, real height, 1 if loaded.
    pub(crate) table: wgpu::Buffer,
    table_data: Vec<[f32; 4]>,
    /// The table changed without being written to the GPU yet.
    table_dirty: AtomicBool,
}

impl MaterialTextures {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let capacity = device
            .limits()
            .max_texture_array_layers
            .clamp(1, MAX_MATERIAL_TEXTURES);
        let array = create_array(device, 1, 1, 1);
        let view = array_view(&array);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("halberd material sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let table = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("halberd material sizes"),
            size: u64::from(MAX_MATERIAL_TEXTURES) * 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            layers: Layers::new(capacity),
            array,
            real: false,
            view,
            sampler,
            table,
            table_data: vec![[0.0; 4]; MAX_MATERIAL_TEXTURES as usize],
            table_dirty: AtomicBool::new(false),
        }
    }

    /// The array's view, for the bind group.
    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// The layer for material `name`, giving it one if there is room.
    pub(crate) fn layer_for(&mut self, name: &str) -> Option<u32> {
        self.layers.layer_for(name)
    }

    /// Gives back the layers of materials not in `used` (upper case), so
    /// the limit counts only what the map shows now.
    pub(crate) fn keep_only(&mut self, used: &HashSet<String>) {
        for layer in self.layers.keep_only(used) {
            if let Some(entry) = self.table_data.get_mut(layer as usize) {
                *entry = [0.0; 4];
            }
            self.table_dirty.store(true, Ordering::Relaxed);
        }
    }

    /// Writes the table to the GPU if it changed.
    pub(crate) fn flush(&self, queue: &wgpu::Queue) {
        if self.table_dirty.swap(false, Ordering::Relaxed) {
            queue.write_buffer(&self.table, 0, bytemuck::cast_slice(&self.table_data));
        }
    }

    /// Materials whose textures are wanted and not yet asked for; they
    /// count as asked for from now on.
    pub(crate) fn wanted(&mut self) -> Vec<String> {
        self.layers.take_wanted()
    }

    /// Marks material `name` as having no texture that can be shown.
    pub(crate) fn missing(&mut self, name: &str) {
        if let Some(layer) = self.layers.of(name) {
            self.layers.set(layer, Layer::Missing);
        }
    }

    /// Puts material `name`'s texture (`image`: width, height and RGBA
    /// pixels; its real size `real`) on the GPU. Returns true if the bind group must be
    /// made again (the array was just created).
    pub(crate) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        name: &str,
        image: (u32, u32, &[u8]),
        real: (u32, u32),
    ) -> bool {
        let (width, height, pixels) = image;
        let Some(layer) = self.layers.of(name) else {
            return false;
        };
        if width == 0 || height == 0 || pixels.len() != (width as usize) * (height as usize) * 4 {
            self.missing(name);
            return false;
        }
        let mut rebuilt = false;
        if !self.real {
            self.array = create_array(
                device,
                MATERIAL_TEXTURE_SIZE,
                self.layers.capacity(),
                MIP_LEVELS,
            );
            self.view = array_view(&self.array);
            self.real = true;
            rebuilt = true;
        }
        let mut level = resize(pixels, width, height, MATERIAL_TEXTURE_SIZE);
        let mut size = MATERIAL_TEXTURE_SIZE;
        for mip in 0..MIP_LEVELS {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.array,
                    mip_level: mip,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &level,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size * 4),
                    rows_per_image: Some(size),
                },
                wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
            );
            if size == 1 {
                break;
            }
            level = halve(&level, size);
            size /= 2;
        }
        self.table_data[layer as usize] = [real.0.max(1) as f32, real.1.max(1) as f32, 1.0, 0.0];
        queue.write_buffer(&self.table, 0, bytemuck::cast_slice(&self.table_data));
        self.table_dirty.store(false, Ordering::Relaxed);
        self.layers.set(layer, Layer::Loaded);
        rebuilt
    }
}

fn create_array(device: &wgpu::Device, size: u32, layers: u32, mips: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("halberd material textures"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: layers,
        },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn array_view(texture: &wgpu::Texture) -> wgpu::TextureView {
    texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("halberd material textures view"),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    })
}

/// RGBA pixels resized to `size` × `size` (nearest pixel; textures repeat,
/// so their aspect ratio comes back through the texture coordinates).
pub(crate) fn resize(pixels: &[u8], width: u32, height: u32, size: u32) -> Vec<u8> {
    let mut out = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        let sy = (u64::from(y) * u64::from(height) / u64::from(size)) as u32;
        for x in 0..size {
            let sx = (u64::from(x) * u64::from(width) / u64::from(size)) as u32;
            let from = ((sy * width + sx) * 4) as usize;
            let to = ((y * size + x) * 4) as usize;
            out[to..to + 4].copy_from_slice(&pixels[from..from + 4]);
        }
    }
    out
}

/// The next smaller mip level: each 2 × 2 block averaged.
pub(crate) fn halve(pixels: &[u8], size: u32) -> Vec<u8> {
    let half = (size / 2).max(1);
    let mut out = vec![0u8; (half * half * 4) as usize];
    for y in 0..half {
        for x in 0..half {
            for c in 0..4 {
                let at = |xx: u32, yy: u32| u32::from(pixels[((yy * size + xx) * 4 + c) as usize]);
                let sum = at(2 * x, 2 * y)
                    + at(2 * x + 1, 2 * y)
                    + at(2 * x, 2 * y + 1)
                    + at(2 * x + 1, 2 * y + 1);
                out[((y * half + x) * 4 + c) as usize] = (sum / 4) as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resizing_and_halving_keep_the_colours() {
        // 2 × 1: red then blue.
        let pixels = [255, 0, 0, 255, 0, 0, 255, 255];
        let big = resize(&pixels, 2, 1, 4);
        assert_eq!(&big[..4], &[255, 0, 0, 255]);
        assert_eq!(&big[12..16], &[0, 0, 255, 255]);
        let small = halve(&big, 4);
        assert_eq!(small.len(), 2 * 2 * 4);
        assert_eq!(&small[..4], &[255, 0, 0, 255]);
        let tiny = halve(&halve(&big, 4), 2);
        assert_eq!(tiny, vec![127, 0, 127, 255]);
    }
}
