//! What the map looks like as GPU geometry: shaded brush faces, their
//! outlines, and the outline of a box being drawn.

use crate::lines::LineVertex;
use crate::models::PropModel;
use bytemuck::{Pod, Zeroable};
use halberd_doc::{Document, Object};
use halberd_geom::{Aabb, Brush};

mod entities;
mod props;
pub use entities::{LIGHT_COLOR, SPAWN_COLOR, SUN_COLOR, entity_color};
pub use props::{NORMALIZED_UV_FLAG, PROP_COLOR};

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
/// Outline of brushes that belong to an entity: as dark, with a hint of
/// teal that the Wireframe mode brightens.
pub const ENTITY_EDGE_COLOR: [f32; 4] = [0.0, 0.06, 0.06, 1.0];
/// Outline of point entity markers: as dark, with a hint of purple that
/// the Wireframe mode brightens.
pub const POINT_ENTITY_EDGE_COLOR: [f32; 4] = [0.06, 0.0, 0.07, 1.0];
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
    /// Its colour: [`PLAYER_COLOR`] where a player fits,
    /// [`PLAYER_CROUCH_COLOR`] or [`PLAYER_BLOCKED_COLOR`] where not.
    pub color: [f32; 4],
}

/// Colour of the player figure where a player only fits crouching.
pub const PLAYER_CROUCH_COLOR: [f32; 4] = [1.0, 0.62, 0.15, 1.0];
/// Colour of the player figure where a player doesn't fit.
pub const PLAYER_BLOCKED_COLOR: [f32; 4] = [1.0, 0.25, 0.2, 1.0];

/// Vertices in a player outline: a box and a ring at eye height.
pub const PLAYER_OUTLINE_VERTICES: usize = 32;

/// The player figure as a line list: its box, and a ring at eye height.
pub fn player_outline(player: PlayerOutline) -> Vec<LineVertex> {
    let mut lines = box_outline(player.bounds, player.color);
    let eye = player.bounds.min.z + player.eye_height.clamp(0.0, player.bounds.size().z);
    let ring = Aabb::from_corners(player.bounds.min.with_z(eye), player.bounds.max.with_z(eye));
    let corners = ring.corners();
    for i in 0..4 {
        for corner in [corners[i], corners[(i + 1) % 4]] {
            lines.push(LineVertex {
                position: corner.to_array(),
                color: player.color,
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
    /// For the Textured mode: texture coordinates (in textures of
    /// [`TEXTURE_SIZE`] texels), the material's hue (0 to 1), and
    /// [`texture_flag`] (0: drawn plain, as point entity markers).
    pub texture: [f32; 4],
}

/// Texture size the Textured mode lays its pattern out for, in texels, until
/// real textures are read (most Source textures are 128 to 512 across).
pub const TEXTURE_SIZE: f32 = 128.0;

/// A material's hue (0 to 1) for the Textured mode's pattern: the same
/// for the same name whatever its case, and different for most others.
pub fn material_hue(material: &str) -> f32 {
    // FNV-1a over the upper-cased name.
    let mut hash: u32 = 0x811c_9dc5;
    for byte in material.bytes() {
        hash ^= u32::from(byte.to_ascii_uppercase());
        hash = hash.wrapping_mul(0x0100_0193);
    }
    (hash % 1000) as f32 / 1000.0
}

impl FaceVertex {
    /// Vertex layout for the brush pipeline.
    pub(crate) const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<FaceVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x4,
            3 => Float32x4,
        ],
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
    /// Outlines of props' triangles, shown only in the Wireframe mode
    /// (solid props would look scribbled over).
    pub prop_edges: Vec<LineVertex>,
    /// The same for selected props, drawn on top in the Wireframe mode.
    pub selected_prop_edges: Vec<LineVertex>,
}

impl SceneGeometry {
    /// Builds the geometry for every object in `doc`.
    pub fn from_document(doc: &Document) -> Self {
        Self::from_document_with(doc, &mut |_| None, &|_| None)
    }

    /// Builds the geometry, asking `layer_of` which texture layer each
    /// material's real texture is in (`None`: none, the pattern is shown),
    /// and `model_of` for the model a prop shows (`None`: not read yet,
    /// or unreadable; the prop is drawn as a marker box).
    pub fn from_document_with<'m>(
        doc: &Document,
        layer_of: &mut dyn FnMut(&str) -> Option<u32>,
        model_of: &dyn Fn(&str) -> Option<&'m PropModel>,
    ) -> Self {
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
                    let looks = Looks::Faces(brush.faces());
                    scene.add_brush(brush.brush(), base, selected, looks, layer_of);
                }
                // A brush entity is drawn as its brushes, even with an origin.
                Object::Entity(_) if doc.has_brushes(id) => {}
                Object::Entity(entity) if let Some(model) = entity.model().and_then(model_of) => {
                    scene.add_prop(entity, model, selected, layer_of);
                }
                Object::Entity(entity) => {
                    scene.add_point_entity(entity, doc.entity_box(entity), selected, layer_of)
                }
                Object::Shape(shape) => scene.add_shape(shape, selected, layer_of),
            }
        }
        scene
    }

    /// A floor-plan shape: its pieces' sides, with edges only along its
    /// outline (where pieces meet inside it, nothing is drawn).
    fn add_shape(
        &mut self,
        shape: &halberd_doc::ShapeObject,
        selected: bool,
        layer_of: &mut dyn FnMut(&str) -> Option<u32>,
    ) {
        let edge_start = if selected {
            self.selected_edges.len()
        } else {
            self.edges.len()
        };
        for piece in shape.pieces() {
            self.add_brush(
                piece,
                BRUSH_COLOR,
                selected,
                Looks::All(&shape.look),
                layer_of,
            );
        }
        let (edges, color) = if selected {
            (&mut self.selected_edges, SELECTED_EDGE_COLOR)
        } else {
            (&mut self.edges, EDGE_COLOR)
        };
        edges.truncate(edge_start);
        let outline = shape.outline();
        let line = |a: glam::Vec3, b: glam::Vec3| {
            [a, b].map(|p| LineVertex {
                position: p.to_array(),
                color,
            })
        };
        for (i, a) in outline.iter().enumerate() {
            let b = outline[(i + 1) % outline.len()];
            for z in [shape.bottom(), shape.top()] {
                edges.extend(line(a.extend(z), b.extend(z)));
            }
            edges.extend(line(a.extend(shape.bottom()), a.extend(shape.top())));
        }
    }

    fn add_brush(
        &mut self,
        brush: &Brush,
        base: [f32; 4],
        selected: bool,
        looks: Looks<'_>,
        layer_of: &mut dyn FnMut(&str) -> Option<u32>,
    ) {
        let (color, edges, edge_color) = if selected {
            (
                SELECTED_COLOR,
                &mut self.selected_edges,
                SELECTED_EDGE_COLOR,
            )
        } else {
            // Every point entity colour (spawns, lights…) keeps the point
            // entity tint in the Wireframe mode.
            let edge = if base == ENTITY_BRUSH_COLOR {
                ENTITY_EDGE_COLOR
            } else if base == BRUSH_COLOR {
                EDGE_COLOR
            } else {
                POINT_ENTITY_EDGE_COLOR
            };
            (base, &mut self.edges, edge)
        };
        for (k, face) in brush.faces().iter().enumerate() {
            let normal = face.plane().normal.to_array();
            let corners = face.vertices();
            let look = match looks {
                Looks::Faces(infos) => infos.get(k),
                Looks::All(info) => Some(info),
                Looks::None => None,
            };
            let (axes, hue, layer) = match look {
                Some(info) => (
                    Some(info.texture_axes(face.plane().normal)),
                    material_hue(&info.material),
                    layer_of(&info.material),
                ),
                None => (None, 0.0, None),
            };
            let vertex = |i: usize| FaceVertex {
                position: corners[i].to_array(),
                normal,
                color,
                texture: match axes {
                    Some(axes) => {
                        let [u, v] = axes.texels(corners[i]);
                        [
                            u / TEXTURE_SIZE,
                            v / TEXTURE_SIZE,
                            hue,
                            texture_flag(selected, layer),
                        ]
                    }
                    None => [0.0; 4],
                },
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

/// The fourth texture value of a face vertex: 1 for a face with a
/// material, 2 if it is also selected, plus 4 × (layer + 1) when its
/// material's real texture is in a texture layer.
pub fn texture_flag(selected: bool, layer: Option<u32>) -> f32 {
    let flag = if selected { 2.0 } else { 1.0 };
    flag + layer.map_or(0.0, |l| 4.0 * (l as f32 + 1.0))
}

/// What a brush's faces look like, for the Textured mode.
#[derive(Clone, Copy)]
enum Looks<'a> {
    /// Each face's own data, in face order.
    Faces(&'a [halberd_doc::FaceInfo]),
    /// One look for every face (a floor plan shape).
    All(&'a halberd_doc::FaceInfo),
    /// No material (point entity markers).
    None,
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
        assert_eq!(std::mem::size_of::<FaceVertex>(), 56);
        assert_eq!(FaceVertex::LAYOUT.attributes[1].offset, 12);
        assert_eq!(FaceVertex::LAYOUT.attributes[2].offset, 24);
        assert_eq!(FaceVertex::LAYOUT.attributes[3].offset, 40);
    }

    #[test]
    fn faces_carry_texture_coordinates_and_a_hue_per_material() {
        let doc = doc_with_two_boxes();
        let scene = SceneGeometry::from_document(&doc);
        assert!(
            scene.faces.iter().all(|v| v.texture[3] >= 1.0),
            "brushes have a material"
        );
        // New faces are world-aligned at a quarter unit per texel: 64 units
        // across is 256 texels, two 128-texel textures.
        let top: Vec<&FaceVertex> = scene
            .faces
            .iter()
            .filter(|v| v.normal[2] > 0.5 && v.position[0] < 80.0)
            .collect();
        let us: Vec<f32> = top.iter().map(|v| v.texture[0]).collect();
        let span = us.iter().copied().fold(f32::MIN, f32::max)
            - us.iter().copied().fold(f32::MAX, f32::min);
        assert!((span - 2.0).abs() < 1e-4, "{span}");
        assert_eq!(
            material_hue("dev/dev_measuregeneric01b"),
            material_hue("DEV/DEV_MEASUREGENERIC01B")
        );
        assert_ne!(
            material_hue("BRICK/BRICKWALL001A"),
            material_hue("DEV/DEV_MEASUREGENERIC01B")
        );
        assert!((0.0..1.0).contains(&material_hue("")));
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
    fn a_shape_is_outlined_only_along_its_outline() {
        let outline = [
            glam::Vec2::new(0.0, 0.0),
            glam::Vec2::new(256.0, 0.0),
            glam::Vec2::new(256.0, 128.0),
            glam::Vec2::new(128.0, 128.0),
            glam::Vec2::new(128.0, 256.0),
            glam::Vec2::new(0.0, 256.0),
        ];
        let shape = halberd_doc::ShapeObject::new(&outline, 0.0, 64.0, Default::default()).unwrap();
        let mut doc = Document::new();
        doc.execute(Command::AddShape(shape)).unwrap();
        doc.clear_selection();
        let scene = SceneGeometry::from_document(&doc);
        assert!(!scene.faces.is_empty());
        // Six sides: a bottom, a top and an upright line for each corner,
        // with no line where the pieces meet inside.
        assert_eq!(scene.edges.len(), 6 * 3 * 2);
        let ids: Vec<_> = doc.objects().map(|(id, _)| id).collect();
        doc.set_selection(ids);
        let scene = SceneGeometry::from_document(&doc);
        assert_eq!(scene.selected_edges.len(), 6 * 3 * 2);
        assert!(scene.edges.is_empty());
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
            color: PLAYER_COLOR,
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
        assert_eq!(count(&scene, LIGHT_COLOR), 36, "the light only");
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
    fn entity_outlines_carry_their_kind_for_the_wireframe_mode() {
        let mut doc = doc_with_entities();
        doc.clear_selection();
        let scene = SceneGeometry::from_document(&doc);
        let count = |color| scene.edges.iter().filter(|v| v.color == color).count();
        assert_eq!(count(ENTITY_EDGE_COLOR), 2 * 48, "func_detail's brushes");
        assert_eq!(count(POINT_ENTITY_EDGE_COLOR), 48, "the light");
        assert_eq!(count(EDGE_COLOR), 0);
        // All still dark, so solid faces read the same in the Flat mode.
        for color in [EDGE_COLOR, ENTITY_EDGE_COLOR, POINT_ENTITY_EDGE_COLOR] {
            assert!(color[..3].iter().all(|c| *c < 0.08));
        }
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
