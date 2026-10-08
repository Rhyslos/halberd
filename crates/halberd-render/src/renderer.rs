//! Drawing a viewport into an offscreen image on the GPU.
//!
//! Each viewport renders into its own [`ViewportTarget`] (colour and depth
//! images, with multisampling when the GPU supports it). The caller then
//! shows the finished colour image however it likes; the editor hands it to
//! egui as a texture.

use crate::frame::{FrameParams, FrameUniforms, GRID_HALF_EXTENT};
use crate::lines::{LineVertex, axis_lines};
use wgpu::util::DeviceExt;

/// Colour format the viewport renders in: 8-bit RGBA, sRGB-encoded, so
/// shaders work in linear colour and the GPU encodes on write.
pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
/// Format of [`ViewportTarget::display_view`]: the same bytes, read as plain
/// RGBA without sRGB decoding. egui expects this for native textures; giving
/// it the sRGB view would decode the colours twice and darken the image.
pub const DISPLAY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// Depth buffer format.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Background colour, linear space (dark blue-grey).
pub const BACKGROUND: wgpu::Color = wgpu::Color {
    r: 0.009,
    g: 0.010,
    b: 0.013,
    a: 1.0,
};
/// The viewport shaders, in WGSL.
pub const SHADER_SOURCE: &str = include_str!("viewport.wgsl");

/// Picks the multisample count for smooth lines: 4 when the GPU supports it
/// for both colour and depth, otherwise 1 (no multisampling).
pub fn best_sample_count(adapter: &wgpu::Adapter) -> u32 {
    let supports = |format| {
        adapter
            .get_texture_format_features(format)
            .flags
            .sample_count_supported(4)
    };
    if supports(COLOR_FORMAT) && supports(DEPTH_FORMAT) {
        4
    } else {
        1
    }
}

/// GPU images one viewport draws into. Create with [`ViewportRenderer::create_target`].
#[derive(Debug)]
pub struct ViewportTarget {
    size: [u32; 2],
    /// Multisampled colour image (only when multisampling).
    msaa_view: Option<wgpu::TextureView>,
    /// The finished, single-sample colour image.
    resolved: wgpu::Texture,
    resolved_view: wgpu::TextureView,
    display_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
}

impl ViewportTarget {
    /// Size in pixels.
    pub fn size(&self) -> [u32; 2] {
        self.size
    }

    /// The finished colour image as rendered (sRGB view).
    pub fn color_view(&self) -> &wgpu::TextureView {
        &self.resolved_view
    }

    /// The finished colour image for showing in egui: the same pixels,
    /// viewed as plain RGBA ([`DISPLAY_FORMAT`]).
    pub fn display_view(&self) -> &wgpu::TextureView {
        &self.display_view
    }

    /// The finished colour image as a texture (for copying).
    pub fn color_texture(&self) -> &wgpu::Texture {
        &self.resolved
    }
}

/// Everything needed to draw viewports on one GPU device.
#[derive(Debug)]
pub struct ViewportRenderer {
    sample_count: u32,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    grid_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    axis_buffer: wgpu::Buffer,
    axis_vertex_count: u32,
}

impl ViewportRenderer {
    /// Builds shaders, pipelines and fixed geometry. `sample_count` is 1 or
    /// the value of [`best_sample_count`].
    pub fn new(device: &wgpu::Device, sample_count: u32) -> Self {
        let sample_count = if sample_count == 4 { 4 } else { 1 };
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("halberd viewport shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("halberd frame uniforms"),
            size: std::mem::size_of::<FrameUniforms>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("halberd frame layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("halberd frame bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("halberd viewport layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let grid_pipeline =
            create_pipeline(device, &layout, &shader, sample_count, PipelineKind::Grid);
        let line_pipeline =
            create_pipeline(device, &layout, &shader, sample_count, PipelineKind::Lines);

        let axes = axis_lines(GRID_HALF_EXTENT);
        let axis_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("halberd axes"),
            contents: bytemuck::cast_slice(&axes),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            sample_count,
            uniform_buffer,
            bind_group,
            grid_pipeline,
            line_pipeline,
            axis_buffer,
            axis_vertex_count: axes.len() as u32,
        }
    }

    /// The multisample count in use.
    pub fn sample_count(&self) -> u32 {
        self.sample_count
    }

    /// Creates images to draw a viewport of `size` pixels into. Sizes are
    /// clamped to at least 1×1 and at most the GPU's largest image.
    pub fn create_target(&self, device: &wgpu::Device, size: [u32; 2]) -> ViewportTarget {
        let max = device.limits().max_texture_dimension_2d.max(1);
        let size = [size[0].clamp(1, max), size[1].clamp(1, max)];
        let extent = wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        };
        let texture = |label, format, usage, samples, view_formats: &[wgpu::TextureFormat]| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: extent,
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats,
            })
        };
        let resolved = texture(
            "halberd viewport colour",
            COLOR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            1,
            &[DISPLAY_FORMAT],
        );
        let msaa_view = (self.sample_count > 1).then(|| {
            texture(
                "halberd viewport colour (multisampled)",
                COLOR_FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
                self.sample_count,
                &[],
            )
            .create_view(&wgpu::TextureViewDescriptor::default())
        });
        let depth_view = texture(
            "halberd viewport depth",
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
            self.sample_count,
            &[],
        )
        .create_view(&wgpu::TextureViewDescriptor::default());
        let resolved_view = resolved.create_view(&wgpu::TextureViewDescriptor::default());
        let display_view = resolved.create_view(&wgpu::TextureViewDescriptor {
            label: Some("halberd viewport display view"),
            format: Some(DISPLAY_FORMAT),
            ..Default::default()
        });
        ViewportTarget {
            size,
            msaa_view,
            resolved,
            resolved_view,
            display_view,
            depth_view,
        }
    }

    /// Draws one frame into `target` and submits it to the GPU.
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &ViewportTarget,
        params: &FrameParams,
    ) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&FrameUniforms::new(params)),
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("halberd viewport frame"),
        });
        {
            let (view, resolve_target, store) = match &target.msaa_view {
                Some(msaa) => (msaa, Some(&target.resolved_view), wgpu::StoreOp::Discard),
                None => (&target.resolved_view, None, wgpu::StoreOp::Store),
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("halberd viewport pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(BACKGROUND),
                        store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &target.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_pipeline(&self.grid_pipeline);
            pass.draw(0..6, 0..1);
            pass.set_pipeline(&self.line_pipeline);
            pass.set_vertex_buffer(0, self.axis_buffer.slice(..));
            pass.draw(0..self.axis_vertex_count, 0..1);
        }
        queue.submit([encoder.finish()]);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum PipelineKind {
    Grid,
    Lines,
}

fn create_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    sample_count: u32,
    kind: PipelineKind,
) -> wgpu::RenderPipeline {
    let (label, vs, fs, topology, buffers, blend, depth_write): (_, _, _, _, &[_], _, _) =
        match kind {
            PipelineKind::Grid => (
                "halberd grid pipeline",
                "grid_vs",
                "grid_fs",
                wgpu::PrimitiveTopology::TriangleList,
                &[],
                Some(wgpu::BlendState::ALPHA_BLENDING),
                // Transparent: tests depth but does not hide what is drawn later.
                false,
            ),
            PipelineKind::Lines => (
                "halberd line pipeline",
                "line_vs",
                "line_fs",
                wgpu::PrimitiveTopology::LineList,
                &[Some(LineVertex::LAYOUT)],
                None,
                true,
            ),
        };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(vs),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers,
        },
        primitive: wgpu::PrimitiveState {
            topology,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(depth_write),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            ..Default::default()
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fs),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: COLOR_FORMAT,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// Copies a target's finished image back to the CPU as tightly packed RGBA8
/// rows (sRGB-encoded, as stored). Blocks until the GPU is done. Meant for
/// tests and screenshots, not for every frame.
pub fn read_pixels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &ViewportTarget,
) -> Result<Vec<u8>, String> {
    let [width, height] = target.size;
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
            texture: &target.resolved,
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
