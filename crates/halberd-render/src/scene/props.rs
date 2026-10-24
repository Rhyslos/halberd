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
        // A selected prop's box shows through everything in every mode.
        if selected
            && let Some((min, max)) = model.bounds()
            && let Some(placed) =
                entity.placed_box(Some(halberd_geom::Aabb::from_corners(min, max)))
        {
            self.selected_edges
                .extend(super::box_outline(placed, SELECTED_EDGE_COLOR));
        }
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
                &mut self.selected_prop_edges
            } else {
                &mut self.prop_edges
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PropModel;
    use glam::Vec3;
    use halberd_doc::{Command, Document};
    use halberd_mdl::example::{ExampleModel, box_mesh, model_files};

    fn crate_model() -> PropModel {
        let files = model_files(&ExampleModel {
            materials: &["crate"],
            meshes: &[box_mesh([0.0; 3], [10.0, 20.0, 30.0], 0)],
            ..ExampleModel::default()
        });
        let model = halberd_mdl::Model::parse(&files.mdl, &files.vvd, &files.vtx).unwrap();
        PropModel::new(&model, &["MODELS/CRATE".into()])
    }

    #[test]
    fn a_prop_is_its_model_placed_and_turned_with_outlines_for_wireframe() {
        let model = crate_model();
        let mut doc = Document::new();
        doc.execute(Command::AddEntity(EntityObject::point(
            "prop_static",
            Vec3::new(100.0, 0.0, 0.0),
            &[("model", "models/crate.mdl"), ("angles", "0 90 0")],
        )))
        .unwrap();
        doc.clear_selection();
        let scene = SceneGeometry::from_document_with(&doc, &mut |_| Some(3), &|_| Some(&model));
        assert_eq!(scene.faces.len(), 36, "12 triangles");
        // Turned a quarter: the model's 20-long Y side now runs along -X.
        let xs: Vec<f32> = scene.faces.iter().map(|v| v.position[0]).collect();
        let (lo, hi) = xs
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), &x| (a.min(x), b.max(x)));
        assert!(
            (lo - 80.0).abs() < 1e-3 && (hi - 100.0).abs() < 1e-3,
            "{lo} {hi}"
        );
        let flag = scene.faces[0].texture[3];
        assert_eq!(flag, texture_flag(false, Some(3)) + NORMALIZED_UV_FLAG);
        assert!(scene.edges.is_empty() && scene.selected_edges.is_empty());
        assert_eq!(scene.prop_edges.len(), 30 * 2);
        // Selected: a box outline shows in every mode.
        let id = doc.objects().next().unwrap().0;
        doc.set_selection([id]);
        let scene = SceneGeometry::from_document_with(&doc, &mut |_| None, &|_| Some(&model));
        assert_eq!(scene.selected_edges.len(), 12 * 2);
        assert_eq!(scene.selected_prop_edges.len(), 30 * 2);
        // Without its model, a marker box.
        let scene = SceneGeometry::from_document(&doc);
        assert_eq!(scene.faces.len(), 36);
        assert!(scene.prop_edges.is_empty());
    }
}
