//! What the map looks like as GPU geometry: shaded brush faces, their
//! outlines, and the outline of a box being drawn.

use crate::lines::LineVertex;
use bytemuck::{Pod, Zeroable};
use halberd_doc::{Document, Object};
use halberd_geom::{Aabb, Brush};

/// Brush face colour, linear space (neutral grey with a hint of blue).
pub const BRUSH_COLOR: [f32; 4] = [0.30, 0.31, 0.34, 1.0];
/// Selected brush face colour, linear space (Hammer's red selection).
pub const SELECTED_COLOR: [f32; 4] = [0.55, 0.10, 0.08, 1.0];
/// Brush outline colour, linear space (dark, so faces read as solid).
pub const EDGE_COLOR: [f32; 4] = [0.02, 0.02, 0.025, 1.0];
/// Selected brush outline colour, linear space; drawn on top of everything.
pub const SELECTED_EDGE_COLOR: [f32; 4] = [1.0, 0.22, 0.12, 1.0];
/// Outline of a box being drawn, linear space (warm yellow).
pub const PREVIEW_COLOR: [f32; 4] = [1.0, 0.80, 0.25, 1.0];

/// One corner of a brush face triangle, as the brush shader reads it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct FaceVertex {
    /// Position in world units.
    pub position: [f32; 3],
    /// The face's outward normal, for shading.
    pub normal: [f32; 3],
    /// Linear-space colour.
    pub color: [f32; 4],
}

impl FaceVertex {
    /// Vertex layout for the brush pipeline.
    pub(crate) const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<FaceVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4],
    };
}

/// Everything needed to draw the map's objects.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SceneGeometry {
    /// Brush faces as a triangle list.
    pub faces: Vec<FaceVertex>,
    /// Outlines of unselected brushes as a line list, hidden behind solids.
    pub edges: Vec<LineVertex>,
    /// Outlines of selected brushes as a line list, drawn on top of
    /// everything so the selection is always visible.
    pub selected_edges: Vec<LineVertex>,
}

impl SceneGeometry {
    /// Builds the geometry for every object in `doc`.
    pub fn from_document(doc: &Document) -> Self {
        let mut scene = Self::default();
        for (id, object) in doc.objects() {
            match object {
                Object::Brush(brush) => scene.add_brush(brush, doc.is_selected(id)),
            }
        }
        scene
    }

    fn add_brush(&mut self, brush: &Brush, selected: bool) {
        let (color, edges, edge_color) = if selected {
            (
                SELECTED_COLOR,
                &mut self.selected_edges,
                SELECTED_EDGE_COLOR,
            )
        } else {
            (BRUSH_COLOR, &mut self.edges, EDGE_COLOR)
        };
        for face in brush.faces() {
            let normal = face.plane().normal.to_array();
            let corners = face.vertices();
            let vertex = |i: usize| FaceVertex {
                position: corners[i].to_array(),
                normal,
                color,
            };
            // A convex polygon is a fan of triangles from its first corner.
            for i in 1..corners.len().saturating_sub(1) {
                self.faces.extend([vertex(0), vertex(i), vertex(i + 1)]);
            }
            for (i, corner) in corners.iter().enumerate() {
                let next = corners[(i + 1) % corners.len()];
                edges.extend([
                    LineVertex {
                        position: corner.to_array(),
                        color: edge_color,
                    },
                    LineVertex {
                        position: next.to_array(),
                        color: edge_color,
                    },
                ]);
            }
        }
    }
}

/// The twelve edges of a box as a line list, in one colour.
pub fn box_outline(bounds: Aabb, color: [f32; 4]) -> Vec<LineVertex> {
    bounds
        .edges()
        .iter()
        .flat_map(|[a, b]| {
            [
                LineVertex {
                    position: a.to_array(),
                    color,
                },
                LineVertex {
                    position: b.to_array(),
                    color,
                },
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    use halberd_doc::Command;

    fn doc_with_two_boxes() -> Document {
        let cube = |x: f32| {
            Brush::cuboid(Aabb::from_corners(
                Vec3::new(x, 0.0, 0.0),
                Vec3::new(x + 64.0, 64.0, 64.0),
            ))
            .unwrap()
        };
        let mut doc = Document::new();
        doc.execute(Command::AddBrushes(vec![cube(0.0), cube(100.0)]))
            .unwrap();
        doc
    }

    #[test]
    fn vertex_layout_matches_the_shader() {
        assert_eq!(std::mem::size_of::<FaceVertex>(), 40);
        assert_eq!(FaceVertex::LAYOUT.attributes[1].offset, 12);
        assert_eq!(FaceVertex::LAYOUT.attributes[2].offset, 24);
    }

    #[test]
    fn an_empty_map_draws_nothing() {
        assert_eq!(
            SceneGeometry::from_document(&Document::new()),
            SceneGeometry::default()
        );
    }

    #[test]
    fn each_box_is_twelve_triangles_and_its_outline() {
        let mut doc = doc_with_two_boxes();
        doc.clear_selection();
        let scene = SceneGeometry::from_document(&doc);
        assert_eq!(scene.faces.len(), 2 * 12 * 3);
        // Each face outlines its own four edges: 6 faces × 4 edges × 2 ends.
        assert_eq!(scene.edges.len(), 2 * 48);
        assert!(scene.selected_edges.is_empty());
        assert!(scene.faces.iter().all(|v| v.color == BRUSH_COLOR));
    }

    #[test]
    fn selected_brushes_are_red_and_outlined_on_top() {
        let mut doc = doc_with_two_boxes();
        let first = doc.objects().next().unwrap().0;
        doc.set_selection([first]);
        let scene = SceneGeometry::from_document(&doc);
        let red = scene
            .faces
            .iter()
            .filter(|v| v.color == SELECTED_COLOR)
            .count();
        assert_eq!(red, 36);
        assert_eq!(scene.selected_edges.len(), 48);
        assert_eq!(scene.edges.len(), 48);
    }

    #[test]
    fn triangles_face_outwards() {
        let scene = SceneGeometry::from_document(&doc_with_two_boxes());
        for tri in scene.faces.chunks(3) {
            let p = |i: usize| Vec3::from_array(tri[i].position);
            let winding = (p(1) - p(0)).cross(p(2) - p(0));
            assert!(winding.dot(Vec3::from_array(tri[0].normal)) > 0.0);
        }
    }

    #[test]
    fn box_outline_has_twelve_lines() {
        let lines = box_outline(Aabb::from_corners(Vec3::ZERO, Vec3::ONE), PREVIEW_COLOR);
        assert_eq!(lines.len(), 24);
        assert!(lines.iter().all(|v| v.color == PREVIEW_COLOR));
    }
}
