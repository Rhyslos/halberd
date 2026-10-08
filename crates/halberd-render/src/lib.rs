//! GPU rendering of the viewports.
//!
//! Draws each viewport with wgpu into its own offscreen image: the ground
//! grid (to the edges of a Source map), the world axes, the map's brushes
//! (shaded, outlined, selected ones in red) and the outline of a box being
//! drawn. Props, entities, gizmos and the lighting preview will follow.
//!
//! This crate knows nothing about egui or windows: it reads the map
//! document ([`ViewportRenderer::update_scene`]), receives a
//! [`FrameParams`] (camera matrices, grid size, box preview) and produces
//! an image in a [`ViewportTarget`]. It never changes the document.

mod frame;
mod lines;
mod pipelines;
mod readback;
mod renderer;
mod scene;

pub use frame::{FrameParams, GRID_HALF_EXTENT, MAJOR_EVERY, SUPER_SPACING, grid_spacings};
pub use lines::{LineVertex, X_AXIS_COLOR, Y_AXIS_COLOR, Z_AXIS_COLOR, Z_AXIS_HEIGHT, axis_lines};
pub use readback::read_pixels;
pub use renderer::{
    BACKGROUND, COLOR_FORMAT, DEPTH_FORMAT, DISPLAY_FORMAT, SHADER_SOURCE, ViewportRenderer,
    ViewportTarget, best_sample_count,
};
pub use scene::{
    BRUSH_COLOR, EDGE_COLOR, FaceVertex, PLAYER_COLOR, PLAYER_OUTLINE_VERTICES, PREVIEW_COLOR,
    PlayerOutline, SELECTED_COLOR, SELECTED_EDGE_COLOR, SceneGeometry, box_outline, player_outline,
};

#[cfg(test)]
mod shader_tests {
    use super::SHADER_SOURCE;
    use wgpu::naga;

    /// Parses and validates the WGSL with the same compiler wgpu uses, so
    /// shader mistakes fail the tests even on machines without a GPU.
    #[test]
    fn shader_is_valid() {
        let module = naga::front::wgsl::parse_str(SHADER_SOURCE)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(SHADER_SOURCE)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(SHADER_SOURCE)));
    }

    #[test]
    fn shader_has_the_expected_entry_points() {
        let module = naga::front::wgsl::parse_str(SHADER_SOURCE).unwrap();
        let mut names: Vec<_> = module
            .entry_points
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "brush_fs", "brush_vs", "grid_fs", "grid_vs", "line_fs", "line_vs"
            ]
        );
    }
}
