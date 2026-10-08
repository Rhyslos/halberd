//! Copying a finished viewport image back to the CPU, for tests and
//! screenshots.

use crate::renderer::ViewportTarget;

/// Copies a target's finished image back to the CPU as tightly packed RGBA8
/// rows (sRGB-encoded, as stored). Blocks until the GPU is done. Meant for
/// tests and screenshots, not for every frame.
pub fn read_pixels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &ViewportTarget,
) -> Result<Vec<u8>, String> {
    let [width, height] = target.size();
    let unpadded = width * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded = unpadded.div_ceil(align) * align;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("halberd readback"),
        size: u64::from(padded) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("halberd readback"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: target.color_texture(),
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);

    let (sender, receiver) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .map_err(|e| format!("GPU did not finish: {e}"))?;
    receiver
        .recv()
        .map_err(|_| "GPU readback was abandoned".to_string())?
        .map_err(|e| format!("could not read the image back: {e}"))?;

    let mapped = buffer
        .slice(..)
        .get_mapped_range()
        .map_err(|e| format!("could not read the image back: {e}"))?;
    let mut pixels = Vec::with_capacity((unpadded * height) as usize);
    for row in mapped.chunks(padded as usize).take(height as usize) {
        pixels.extend_from_slice(&row[..unpadded as usize]);
    }
    drop(mapped);
    buffer.unmap();
    Ok(pixels)
}
