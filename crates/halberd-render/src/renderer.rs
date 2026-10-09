//! Drawing a viewport into an offscreen image on the GPU.
//!
//! Each viewport renders into its own [`ViewportTarget`] (colour and depth
//! images, with multisampling when the GPU supports it). The caller then
//! shows the finished colour image however it likes; the editor hands it to
//! egui as a texture.

use crate::frame::{FrameParams, FrameUniforms, GRID_HALF_EXTENT};
use crate::lines::{LineVertex, axis_lines};
use crate::pipelines::{PipelineKind, create_pipeline};
use crate::scene::{
    PLAYER_OUTLINE_VERTICES, PREVIEW_COLOR, SceneGeometry, box_outline, player_outline,
    shape_outline,
};
use halberd_doc::Document;
use halberd_geom::Brush;
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
    overlay_pipeline: wgpu::RenderPipeline,
    brush_pipeline: wgpu::RenderPipeline,
    axis_buffer: wgpu::Buffer,
    axis_vertex_count: u32,
    scene: SceneBuffers,
    /// Room for the outline of a shape being drawn, rewritten each frame
    /// one is.
    preview_buffer: wgpu::Buffer,
    /// The outline of the shape being drawn (see
    /// [`ViewportRenderer::set_preview_shape`]).
    preview_shape: Vec<LineVertex>,
    /// Room for the player figure, rewritten when it is shown.
    player_buffer: wgpu::Buffer,
}

/// The map's geometry on the GPU, and which document state it shows.
#[derive(Debug, Default)]
struct SceneBuffers {
    /// Document instance, revision and selection revision last uploaded.
    shows: Option<(u64, u64, u64)>,
    faces: GpuList,
    edges: GpuList,
    selected_edges: GpuList,
}

/// Vertices on the GPU, split over as many buffers as the GPU's size limit
/// needs (a huge map would not fit in one).
#[derive(Debug, Default)]
struct GpuList {
    chunks: Vec<(wgpu::Buffer, u32)>,
}

/// Largest buffer this renderer makes, whatever the GPU allows: 64 MiB.
const MAX_CHUNK_BYTES: u64 = 64 * 1024 * 1024;

impl GpuList {
    /// Uploads `data`. `group` vertices always stay together in one buffer
    /// (3 for triangles, 2 for lines).
    fn upload<T: bytemuck::Pod>(
        device: &wgpu::Device,
        label: &str,
        data: &[T],
        group: usize,
    ) -> Self {
        let size = std::mem::size_of::<T>().max(1) as u64;
        let limit = device.limits().max_buffer_size.min(MAX_CHUNK_BYTES);
        let per_chunk = ((limit / size) as usize / group).max(1) * group;
        let chunks = data
            .chunks(per_chunk)
            .map(|chunk| {
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: bytemuck::cast_slice(chunk),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                (buffer, u32::try_from(chunk.len()).unwrap_or(u32::MAX))
            })
            .collect();
        Self { chunks }
    }

    fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        for (buffer, count) in &self.chunks {
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..*count, 0..1);
        }
    }
}

/// Most vertices of a preview outline; a bigger shape's outline is cut
/// short (it is only a preview).
const PREVIEW_VERTICES: usize = 32_768;

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
        let player_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("halberd player figure"),
            size: (PLAYER_OUTLINE_VERTICES * std::mem::size_of::<LineVertex>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
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
        let overlay_pipeline = create_pipeline(
            device,
            &layout,
            &shader,
            sample_count,
            PipelineKind::Overlay,
        );
        let brush_pipeline = create_pipeline(
            device,
            &layout,
            &shader,
            sample_count,
            PipelineKind::Brushes,
        );
        let preview_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("halberd box preview"),
            size: (PREVIEW_VERTICES * std::mem::size_of::<LineVertex>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

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
            overlay_pipeline,
            brush_pipeline,
            axis_buffer,
            axis_vertex_count: axes.len() as u32,
            scene: SceneBuffers::default(),
            preview_buffer,
            preview_shape: Vec::new(),
            player_buffer,
        }
    }

    /// Makes the next frames show `doc` as it is now. Cheap when nothing
    /// changed since the last call: the geometry is only rebuilt when the
    /// document is replaced, edited or its selection changes.
    pub fn update_scene(&mut self, device: &wgpu::Device, doc: &Document) {
        let state = (doc.instance(), doc.revision(), doc.selection_revision());
        if self.scene.shows == Some(state) {
            return;
        }
        self.set_scene(device, &SceneGeometry::from_document(doc));
        self.scene.shows = Some(state);
    }

    /// Replaces the drawn map geometry. [`Self::update_scene`] is the usual
    /// way in; this one is for previews and tests.
    pub fn set_scene(&mut self, device: &wgpu::Device, geometry: &SceneGeometry) {
        self.scene = SceneBuffers {
            shows: None,
            faces: GpuList::upload(device, "halberd brush faces", &geometry.faces, 3),
            edges: GpuList::upload(device, "halberd brush outlines", &geometry.edges, 2),
            selected_edges: GpuList::upload(
                device,
                "halberd selection outlines",
                &geometry.selected_edges,
                2,
            ),
        };
    }

    /// The shape being drawn, outlined (instead of
    /// [`FrameParams::preview`]'s box) while there is a preview. Empty
    /// shows the box.
    pub fn set_preview_shape(&mut self, brushes: &[Brush]) {
        let mut lines = shape_outline(brushes, PREVIEW_COLOR);
        lines.truncate(PREVIEW_VERTICES);
        self.preview_shape = lines;
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
        let preview = params.preview.map(|bounds| {
            if self.preview_shape.is_empty() {
                box_outline(bounds, PREVIEW_COLOR)
            } else {
                self.preview_shape.clone()
            }
        });
        if let Some(lines) = &preview {
            queue.write_buffer(&self.preview_buffer, 0, bytemuck::cast_slice(lines));
        }
        let player = params.player.map(player_outline);
        if let Some(lines) = &player {
            queue.write_buffer(&self.player_buffer, 0, bytemuck::cast_slice(lines));
        }
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
            // Solids first, so the transparent grid is hidden behind them.
            pass.set_pipeline(&self.brush_pipeline);
            self.scene.faces.draw(&mut pass);
            pass.set_pipeline(&self.grid_pipeline);
            pass.draw(0..6, 0..1);
            pass.set_pipeline(&self.line_pipeline);
            pass.set_vertex_buffer(0, self.axis_buffer.slice(..));
            pass.draw(0..self.axis_vertex_count, 0..1);
            self.scene.edges.draw(&mut pass);
            if let Some(lines) = &player {
                pass.set_vertex_buffer(0, self.player_buffer.slice(..));
                pass.draw(0..lines.len() as u32, 0..1);
            }
            // On top of everything: the selection and the box being drawn.
            pass.set_pipeline(&self.overlay_pipeline);
            self.scene.selected_edges.draw(&mut pass);
            if let Some(lines) = &preview {
                pass.set_vertex_buffer(0, self.preview_buffer.slice(..));
                pass.draw(0..lines.len() as u32, 0..1);
            }
        }
        queue.submit([encoder.finish()]);
    }
}
