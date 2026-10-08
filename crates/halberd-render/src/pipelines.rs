//! The render pipelines: how each kind of geometry is drawn.

use crate::lines::LineVertex;
use crate::renderer::{COLOR_FORMAT, DEPTH_FORMAT};
use crate::scene::FaceVertex;

/// The kinds of geometry the viewport draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PipelineKind {
    /// The ground grid: transparent, tested against depth but not written.
    Grid,
    /// Lines hidden behind solids: axes, brush outlines.
    Lines,
    /// Lines drawn on top of everything: the selection, box previews.
    Overlay,
    /// Solid, shaded brush faces.
    Brushes,
}

/// Builds the pipeline for one kind of geometry.
pub(crate) fn create_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    sample_count: u32,
    kind: PipelineKind,
) -> wgpu::RenderPipeline {
    let line_buffers: &[Option<wgpu::VertexBufferLayout<'static>>] = &[Some(LineVertex::LAYOUT)];
    let face_buffers: &[Option<wgpu::VertexBufferLayout<'static>>] = &[Some(FaceVertex::LAYOUT)];
    let (label, vs, fs, topology, buffers) = match kind {
        PipelineKind::Grid => (
            "halberd grid pipeline",
            "grid_vs",
            "grid_fs",
            wgpu::PrimitiveTopology::TriangleList,
            &[][..],
        ),
        PipelineKind::Lines => (
            "halberd line pipeline",
            "line_vs",
            "line_fs",
            wgpu::PrimitiveTopology::LineList,
            line_buffers,
        ),
        PipelineKind::Overlay => (
            "halberd overlay pipeline",
            "line_vs",
            "line_fs",
            wgpu::PrimitiveTopology::LineList,
            line_buffers,
        ),
        PipelineKind::Brushes => (
            "halberd brush pipeline",
            "brush_vs",
            "brush_fs",
            wgpu::PrimitiveTopology::TriangleList,
            face_buffers,
        ),
    };
    let blend = (kind == PipelineKind::Grid).then_some(wgpu::BlendState::ALPHA_BLENDING);
    let depth_write = matches!(kind, PipelineKind::Lines | PipelineKind::Brushes);
    let depth_compare = if kind == PipelineKind::Overlay {
        wgpu::CompareFunction::Always
    } else {
        wgpu::CompareFunction::LessEqual
    };
    // Faces are pushed very slightly back so their own outlines, at exactly
    // the same depth, are drawn cleanly on top instead of flickering.
    let bias = if kind == PipelineKind::Brushes {
        wgpu::DepthBiasState {
            constant: 4,
            slope_scale: 2.0,
            clamp: 0.0,
        }
    } else {
        wgpu::DepthBiasState::default()
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
            // Brushes are closed and wound outwards, so their back faces
            // never show; skipping them halves the work.
            cull_mode: (kind == PipelineKind::Brushes).then_some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(depth_write),
            depth_compare: Some(depth_compare),
            stencil: wgpu::StencilState::default(),
            bias,
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
