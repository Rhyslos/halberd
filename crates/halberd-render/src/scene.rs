//! What the map looks like as GPU geometry: shaded brush faces, their
//! outlines, and the outline of a box being drawn.

use crate::lines::LineVertex;
use bytemuck::{Pod, Zeroable};
use halberd_doc::{Document, Object};
use halberd_geom::{Aabb, Brush};

/// Brush face colour, linear space (neutral grey with a hint of blue).
pub const BRUSH_COLOR: [f32; 4] = [0.30, 0.31, 0.34, 1.0];
/// Colour of brushes that belong to an entity (such as `func_detail`),
/// linear space (a cool teal-grey, so they stand apart from world brushes).
pub const ENTITY_BRUSH_COLOR: [f32; 4] = [0.20, 0.32, 0.34, 1.0];
/// Colour of the box a point entity is shown as, linear space (purple).
pub const POINT_ENTITY_COLOR: [f32; 4] = [0.42, 0.12, 0.52, 1.0];
/// Selected brush face colour, linear space (Hammer's red selection).
pub const SELECTED_COLOR: [f32; 4] = [0.55, 0.10, 0.08, 1.0];
/// Brush outline colour, linear space (dark, so faces read as solid).
pub const EDGE_COLOR: [f32; 4] = [0.02, 0.02, 0.025, 1.0];
/// Selected brush outline colour, linear space; drawn on top of everything.
pub const SELECTED_EDGE_COLOR: [f32; 4] = [1.0, 0.22, 0.12, 1.0];
/// Outline of a box being drawn, linear space (warm yellow).
pub const PREVIEW_COLOR: [f32; 4] = [1.0, 0.80, 0.25, 1.0];
/// The player scale figure, linear space (light blue).
pub const PLAYER_COLOR: [f32; 4] = [0.20, 0.62, 1.0, 1.0];

/// A player-sized figure to show scale: the space a player takes up, and
/// the height of their eyes above their feet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerOutline {
    /// The space a standing player takes up.
    pub bounds: Aabb,
    /// Eye height above the feet, in units.
    pub eye_height: f32,
}

/// Vertices in a player outline: a box and a ring at eye height.
pub const PLAYER_OUTLINE_VERTICES: usize = 32;

/// The player figure as a line list: its box, and a ring at eye height.
pub fn player_outline(player: PlayerOutline) -> Vec<LineVertex> {
    let mut lines = box_outline(player.bounds, PLAYER_COLOR);
    let eye = player.bounds.min.z + player.eye_height.clamp(0.0, player.bounds.size().z);
    let ring = Aabb::from_corners(player.bounds.min.with_z(eye), player.bounds.max.with_z(eye));
    let corners = ring.corners();
    for i in 0..4 {
        for corner in [corners[i], corners[(i + 1) % 4]] {
            lines.push(LineVertex {
                position: corner.to_array(),
                color: PLAYER_COLOR,
            });
        }
    }
    lines
}

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
            // A brush entity's brushes are selected with it, or one by one.
            let selected = doc.is_shown_selected(id);
            match object {
                Object::Brush(brush) => {
                    let base = if brush.entity().is_some() {
                        ENTITY_BRUSH_COLOR
                    } else {
                        BRUSH_COLOR
                    };
                    scene.add_brush(brush.brush(), base, selected);
                }
                // A brush entity is drawn as its brushes, even with an origin.
                Object::Entity(_) if doc.has_brushes(id) => {}
                Object::Entity(entity) => {
                    if let Some(marker) = entity.marker().and_then(|m| Brush::cuboid(m).ok()) {
                        scene.add_brush(&marker, POINT_ENTITY_COLOR, selected);
                    }
                }
            }
        }
        scene
    }

    fn add_brush(&mut self, brush: &Brush, base: [f32; 4], selected: bool) {
        let (color, edges, edge_color) = if selected {
            (
                SELECTED_COLOR,
                &mut self.selected_edges,
                SELECTED_EDGE_COLOR,
            )
        } else {
            (base, &mut self.edges, EDGE_COLOR)
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

/// The edges of every face of `brushes`, as a line list (for the outline
/// of a shape being drawn).
pub fn shape_outline(brushes: &[Brush], color: [f32; 4]) -> Vec<LineVertex> {
    let mut lines = Vec::new();
    for face in brushes.iter().flat_map(Brush::faces) {
        let corners = face.vertices();
        for (i, corner) in corners.iter().enumerate() {
            let next = corners[(i + 1) % corners.len()];
            for p in [*corner, next] {
                lines.push(LineVertex {
                    position: p.to_array(),
                    color,
                });
            }
        }
    }
    lines
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
    fn player_outline_is_a_box_with_an_eye_ring() {
        let player = PlayerOutline {
            bounds: Aabb::from_corners(Vec3::new(-16.0, -16.0, 0.0), Vec3::new(16.0, 16.0, 72.0)),
            eye_height: 64.0,
        };
        let lines = player_outline(player);
        assert_eq!(lines.len(), PLAYER_OUTLINE_VERTICES);
        assert!(lines[24..].iter().all(|v| v.position[2] == 64.0));
    }

    #[test]
    fn box_outline_has_twelve_lines() {
        let lines = box_outline(Aabb::from_corners(Vec3::ZERO, Vec3::ONE), PREVIEW_COLOR);
        assert_eq!(lines.len(), 24);
        assert!(lines.iter().all(|v| v.color == PREVIEW_COLOR));
    }

    /// A func_detail with an origin (which must not be drawn) and two
    /// brushes, then a light.
    fn doc_with_entities() -> Document {
        use halberd_doc::{BrushObject, EntityObject, MapObject};
        let cube = |x: f32| {
            Brush::cuboid(Aabb::from_corners(Vec3::splat(x), Vec3::splat(x + 64.0))).unwrap()
        };
        let entity = |classname: &str, origin| EntityObject {
            classname: classname.into(),
            origin,
            file_data: Vec::new(),
        };
        let detail = MapObject::Entity(
            entity("func_detail", Some(Vec3::splat(500.0))),
            vec![BrushObject::new(cube(0.0)), BrushObject::new(cube(100.0))],
        );
        let light = MapObject::Entity(entity("light", Some(Vec3::splat(300.0))), Vec::new());
        Document::from_map(vec![detail, light], Default::default()).unwrap()
    }

    #[test]
    fn entity_brushes_are_teal_and_select_with_their_entity() {
        let mut doc = doc_with_entities();
        let scene = SceneGeometry::from_document(&doc);
        let count =
            |scene: &SceneGeometry, color| scene.faces.iter().filter(|v| v.color == color).count();
        assert_eq!(count(&scene, ENTITY_BRUSH_COLOR), 2 * 36);
        assert_eq!(count(&scene, POINT_ENTITY_COLOR), 36, "the light only");
        let detail = doc.objects().next().unwrap().0;
        doc.set_selection([detail]);
        let scene = SceneGeometry::from_document(&doc);
        assert_eq!(count(&scene, SELECTED_COLOR), 2 * 36, "both brushes");
        // One brush on its own.
        let first = doc.brushes_of(detail).next().unwrap();
        doc.set_selection([first]);
        let scene = SceneGeometry::from_document(&doc);
        assert_eq!(count(&scene, SELECTED_COLOR), 36);
        assert_eq!(count(&scene, ENTITY_BRUSH_COLOR), 36);
    }

    #[test]
    fn a_shape_outline_traces_every_face_edge() {
        let cube = Brush::cuboid(Aabb::from_corners(Vec3::ZERO, Vec3::splat(64.0))).unwrap();
        let lines = shape_outline(&[cube.clone(), cube], PREVIEW_COLOR);
        // 2 boxes × 6 faces × 4 edges × 2 ends.
        assert_eq!(lines.len(), 96);
        assert!(lines.iter().all(|v| v.color == PREVIEW_COLOR));
        assert!(shape_outline(&[], PREVIEW_COLOR).is_empty());
    }
}
