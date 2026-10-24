//! Props in the scene: their model's triangles, placed and turned.

use super::{
    FaceVertex, POINT_ENTITY_EDGE_COLOR, SELECTED_COLOR, SELECTED_EDGE_COLOR, SceneGeometry,
    material_hue, texture_flag,
};
use crate::lines::LineVertex;
use crate::models::PropModel;
use halberd_doc::EntityObject;

/// Colour of props in the Flat mode.
pub const PROP_COLOR: [f32; 4] = [0.36, 0.33, 0.30, 1.0];
/// Added to a vertex's texture flag when its texture coordinates already
/// run 0 to 1 across the texture (models), rather than in texels.
pub const NORMALIZED_UV_FLAG: f32 = 2048.0;

impl SceneGeometry {
    /// A prop: `model`'s triangles where `entity` stands, turned as it is.
    pub(super) fn add_prop(
        &mut self,
        entity: &EntityObject,
        model: &PropModel,
        selected: bool,
        layer_of: &mut dyn FnMut(&str) -> Option<u32>,
    ) {
        let Some(origin) = entity.origin else {
            return;
        };
        let rotation = entity.rotation();
        let place = |p: [f32; 3]| (origin + rotation * glam::Vec3::from(p)).to_array();
        let (color, edge_color) = if selected {
            (SELECTED_COLOR, SELECTED_EDGE_COLOR)
        } else {
            (PROP_COLOR, POINT_ENTITY_EDGE_COLOR)
        };
        for mesh in &model.meshes {
            let hue = material_hue(&mesh.material);
            let flag = texture_flag(selected, layer_of(&mesh.material)) + NORMALIZED_UV_FLAG;
            let vertices: Vec<FaceVertex> = mesh
                .vertices
                .iter()
                .map(|v| FaceVertex {
                    position: place(v.position),
                    normal: (rotation * glam::Vec3::from(v.normal)).to_array(),
                    color,
                    texture: [v.uv[0], v.uv[1], hue, flag],
                })
                .collect();
            for &i in &mesh.indices {
                if let Some(v) = vertices.get(i as usize) {
                    self.faces.push(*v);
                }
            }
            let edges = if selected {
                &mut self.selected_edges
            } else {
                &mut self.edges
            };
            for [a, b] in &mesh.edges {
                if let (Some(a), Some(b)) = (vertices.get(*a as usize), vertices.get(*b as usize)) {
                    edges.extend([a.position, b.position].map(|position| LineVertex {
                        position,
                        color: edge_color,
                    }));
                }
            }
        }
    }
}
