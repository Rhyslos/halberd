//! What the renderer needs to know each frame, and how it is laid out for
//! the GPU.

use crate::scene::PlayerOutline;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use halberd_geom::Aabb;

/// Half the width of the drawn grid: Source maps end at ±16384 units, so the
/// grid shows where a map can go.
pub const GRID_HALF_EXTENT: f32 = 16_384.0;
/// Major grid lines appear every this many minor lines.
pub const MAJOR_EVERY: f32 = 8.0;
/// Spacing of the largest grid level, in units.
pub const SUPER_SPACING: f32 = 1024.0;

/// Everything that changes from frame to frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameParams {
    /// World-to-screen transform (depth 0 at near, 1 at far).
    pub view_projection: Mat4,
    /// Where the camera is.
    pub camera_position: Vec3,
    /// The editor's grid size in units (a power of two).
    pub grid_size: f32,
    /// A box being drawn, shown as an outline on top of everything.
    pub preview: Option<Aabb>,
    /// A player-sized figure for scale, hidden behind solid brushes.
    pub player: Option<PlayerOutline>,
    /// Where the grid is drawn.
    pub grid: GridPlane,
    /// How brushes are drawn.
    pub mode: RenderMode,
}

/// How brushes are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderMode {
    /// Outlines only, in a light colour (Hammer's 2D views).
    Wireframe,
    /// Solid faces in one colour, shaded by which way they face.
    #[default]
    Flat,
    /// Solid faces showing each face's material and texture alignment.
    Textured,
}

impl RenderMode {
    /// The value the shader reads.
    fn shader_value(self) -> f32 {
        match self {
            Self::Flat => 0.0,
            Self::Textured => 1.0,
            Self::Wireframe => 2.0,
        }
    }
}

/// Where the grid is drawn: flat across one world axis, at a position along
/// it, fading into the distance or not.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridPlane {
    /// The axis the grid is flat across (0 X, 1 Y, 2 Z).
    pub axis: usize,
    /// Its position along that axis.
    pub offset: f32,
    /// Whether it fades towards the horizon (the 3D view); side views show
    /// it evenly.
    pub fade: bool,
}

impl Default for GridPlane {
    /// The ground: flat across Z at height 0, fading into the distance.
    fn default() -> Self {
        Self {
            axis: 2,
            offset: 0.0,
            fade: true,
        }
    }
}

/// The three grid levels drawn: minor (the editor grid), major and super.
pub fn grid_spacings(grid_size: f32) -> [f32; 3] {
    let minor = if grid_size.is_finite() {
        grid_size.clamp(1.0, 4096.0)
    } else {
        16.0
    };
    let major = minor * MAJOR_EVERY;
    let super_level = if SUPER_SPACING > major {
        SUPER_SPACING
    } else {
        major * MAJOR_EVERY
    };
    [minor, major, super_level]
}

/// The frame data exactly as the shader's `Frame` struct expects it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(crate) struct FrameUniforms {
    view_projection: [[f32; 4]; 4],
    camera: [f32; 4],
    grid: [f32; 4],
    plane: [f32; 4],
}

impl FrameUniforms {
    pub(crate) fn new(params: &FrameParams) -> Self {
        let [minor, major, super_level] = grid_spacings(params.grid_size);
        let p = params.camera_position;
        Self {
            view_projection: params.view_projection.to_cols_array_2d(),
            camera: [p.x, p.y, p.z, 1.0],
            grid: [minor, major, super_level, GRID_HALF_EXTENT],
            plane: [
                params.grid.axis.min(2) as f32,
                if params.grid.offset.is_finite() {
                    params.grid.offset
                } else {
                    0.0
                },
                if params.grid.fade { 1.0 } else { 0.0 },
                params.mode.shader_value(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniforms_match_the_shader_layout() {
        // mat4x4<f32> + three vec4<f32>, no padding.
        assert_eq!(std::mem::size_of::<FrameUniforms>(), 112);
        assert_eq!(std::mem::size_of::<FrameUniforms>() % 16, 0);
    }

    #[test]
    fn default_grid_levels() {
        assert_eq!(grid_spacings(16.0), [16.0, 128.0, 1024.0]);
    }

    #[test]
    fn coarse_grids_keep_levels_increasing() {
        for size in [1.0, 2.0, 64.0, 128.0, 256.0, 1024.0, 4096.0] {
            let [a, b, c] = grid_spacings(size);
            assert!(a < b && b < c, "{size}: {a} {b} {c}");
        }
    }

    #[test]
    fn bad_grid_sizes_are_repaired() {
        assert_eq!(grid_spacings(f32::NAN)[0], 16.0);
        assert_eq!(grid_spacings(0.0)[0], 1.0);
        assert_eq!(grid_spacings(1e9)[0], 4096.0);
    }

    #[test]
    fn uniforms_carry_the_camera_and_grid() {
        let u = FrameUniforms::new(&FrameParams {
            view_projection: Mat4::IDENTITY,
            camera_position: Vec3::new(1.0, 2.0, 3.0),
            grid_size: 32.0,
            preview: None,
            player: None,
            grid: GridPlane {
                axis: 1,
                offset: -50.0,
                fade: false,
            },
            mode: RenderMode::Wireframe,
        });
        assert_eq!(u.plane, [1.0, -50.0, 0.0, 2.0]);
        assert_eq!(u.camera, [1.0, 2.0, 3.0, 1.0]);
        assert_eq!(u.grid, [32.0, 256.0, 1024.0, GRID_HALF_EXTENT]);
        assert_eq!(u.view_projection, Mat4::IDENTITY.to_cols_array_2d());
    }
}
