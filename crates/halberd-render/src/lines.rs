//! Coloured line geometry: the world axes.

use bytemuck::{Pod, Zeroable};

/// One end of a line, as the line shader reads it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct LineVertex {
    /// Position in world units.
    pub position: [f32; 3],
    /// Linear-space colour with alpha.
    pub color: [f32; 4],
}

impl LineVertex {
    /// Vertex layout for the line pipeline.
    pub(crate) const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<LineVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
    };
}

/// X axis colour (red), linear space.
pub const X_AXIS_COLOR: [f32; 4] = [0.75, 0.05, 0.05, 1.0];
/// Y axis colour (green), linear space.
pub const Y_AXIS_COLOR: [f32; 4] = [0.08, 0.55, 0.08, 1.0];
/// Z axis colour (blue), linear space.
pub const Z_AXIS_COLOR: [f32; 4] = [0.08, 0.20, 0.85, 1.0];
/// How tall the Z axis line is drawn, in units.
pub const Z_AXIS_HEIGHT: f32 = 1024.0;

/// The world axes through the origin, as a line list: X (red) and Y (green)
/// across the whole grid, positive halves bright and negative halves dimmed
/// so direction is obvious, and a blue Z line rising from the origin.
pub fn axis_lines(half_extent: f32) -> Vec<LineVertex> {
    let dim = |c: [f32; 4]| [c[0] * 0.35, c[1] * 0.35, c[2] * 0.35, 1.0];
    let line = |from: [f32; 3], to: [f32; 3], color: [f32; 4]| {
        [
            LineVertex {
                position: from,
                color,
            },
            LineVertex {
                position: to,
                color,
            },
        ]
    };
    let e = half_extent;
    [
        line([0.0, 0.0, 0.0], [e, 0.0, 0.0], X_AXIS_COLOR),
        line([0.0, 0.0, 0.0], [-e, 0.0, 0.0], dim(X_AXIS_COLOR)),
        line([0.0, 0.0, 0.0], [0.0, e, 0.0], Y_AXIS_COLOR),
        line([0.0, 0.0, 0.0], [0.0, -e, 0.0], dim(Y_AXIS_COLOR)),
        line([0.0, 0.0, 0.0], [0.0, 0.0, Z_AXIS_HEIGHT], Z_AXIS_COLOR),
    ]
    .concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_layout_matches_the_shader() {
        assert_eq!(std::mem::size_of::<LineVertex>(), 28);
        assert_eq!(LineVertex::LAYOUT.attributes[1].offset, 12);
    }

    #[test]
    fn axes_form_complete_lines_through_the_origin() {
        let lines = axis_lines(100.0);
        assert_eq!(lines.len() % 2, 0, "a line list needs pairs");
        assert_eq!(lines.len(), 10);
        for pair in lines.chunks(2) {
            assert_eq!(pair[0].position, [0.0, 0.0, 0.0]);
            assert_eq!(pair[0].color, pair[1].color);
        }
    }

    #[test]
    fn positive_axes_are_brighter_than_negative() {
        let lines = axis_lines(100.0);
        let x_pos = lines
            .iter()
            .find(|v| v.position == [100.0, 0.0, 0.0])
            .unwrap();
        let x_neg = lines
            .iter()
            .find(|v| v.position == [-100.0, 0.0, 0.0])
            .unwrap();
        assert!(x_pos.color[0] > x_neg.color[0]);
    }
}
