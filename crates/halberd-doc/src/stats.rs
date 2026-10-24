//! How big the map is and how close it comes to what Source can compile:
//! its size against the ±16384-unit edge, and counts against the engine's
//! limits (brushes, brush sides, entities, lights, brush models,
//! materials). Counts are worked out from the map as Halberd holds it;
//! the compiler's own counts can differ a little (it splits and merges
//! some things), so they are estimates.

use crate::{Document, Object, ObjectId};
use glam::Vec3;
use halberd_geom::{Aabb, Brush};
use std::collections::{BTreeSet, HashSet};

/// Where Source maps end: every brush and entity must lie within
/// ±16384 units on each axis.
pub const MAP_HALF_SIZE: f32 = 16_384.0;

/// One of the engine's limits, as counted for this map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Meter {
    /// What is counted ("Brushes").
    pub name: &'static str,
    /// How many the map has (an estimate).
    pub count: usize,
    /// The most Source allows.
    pub limit: usize,
    /// What happens past the limit, in plain words.
    pub help: &'static str,
}

impl Meter {
    /// How full it is, 0 to 1 (more past the limit).
    pub fn fraction(&self) -> f32 {
        self.count as f32 / self.limit.max(1) as f32
    }
}

/// The map's size and its limits.
#[derive(Debug, Clone, PartialEq)]
pub struct MapStats {
    /// The box around everything in the map, if it has anything.
    pub bounds: Option<Aabb>,
    /// Objects reaching past the map's edge (±16384).
    pub outside: Vec<ObjectId>,
    /// The engine's limits, in the order shown.
    pub meters: Vec<Meter>,
}

/// World lights the lighting compiler makes for an entity of `class`:
/// lights make one; the first sun makes two (sunlight and sky light);
/// `light_dynamic` is a real entity in the game and makes none.
fn world_lights(class: &str, first_sun: &mut bool) -> usize {
    match class {
        "light" | "light_spot" => 1,
        "light_environment" if *first_sun => {
            *first_sun = false;
            2
        }
        _ => 0,
    }
}

/// Brush entities the compiler gives no model of their own (their brushes
/// join the world, or become something else).
fn no_model(class: &str) -> bool {
    matches!(class, "func_detail" | "func_ladder" | "func_occluder")
        || class.starts_with("func_areaportal")
}

/// Brush sides as the compiler stores them: a brush's faces, plus a side
/// for each of the six box directions it lacks (the compiler adds them to
/// keep collisions exact). Brushes slanted in two directions get more
/// (along their slanted edges), so for them this is a lower bound.
fn compiled_sides(brush: &Brush) -> usize {
    let axes = [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
    ];
    let missing = axes
        .iter()
        .filter(|axis| {
            !brush
                .faces()
                .iter()
                .any(|f| f.plane().normal.dot(**axis) > 1.0 - 1e-5)
        })
        .count();
    brush.faces().len() + missing
}

/// Running counts while going through the map.
#[derive(Default)]
struct Counts {
    brushes: usize,
    sides: usize,
    entities: usize,
    lights: usize,
    models: usize,
    materials: HashSet<String>,
    bounds: Option<Aabb>,
    outside: BTreeSet<ObjectId>,
}

impl Counts {
    fn grow(&mut self, b: Aabb) {
        self.bounds = Some(self.bounds.map_or(b, |a| a.union(b)));
    }

    fn add_brush(&mut self, brush: &Brush) {
        self.brushes += 1;
        self.sides += compiled_sides(brush);
        self.grow(brush.bounds());
    }

    fn meters(self) -> Vec<Meter> {
        let mut materials = self.materials;
        materials.retain(|m| !m.trim().is_empty());
        vec![
            Meter {
                name: "Brushes",
                count: self.brushes,
                limit: 8192,
                help: "The compiler stops with \"MAX_MAP_BRUSHES\". Join small brushes, or \
                       use props for detail.",
            },
            Meter {
                name: "Brush sides",
                count: self.sides,
                limit: 65_536,
                help: "The compiler stops with \"MAX_MAP_BRUSHSIDES\". Each brush counts its \
                       faces plus up to six more; brushes slanted in two directions cost more \
                       than shown.",
            },
            Meter {
                name: "Map entities",
                count: self.entities,
                limit: 8192,
                help: "Every entity in the map file, the world and static props included; \
                       the compiler stops with \"MAX_MAP_ENTITIES\". In the game, Garry's \
                       Mod allows 8192 networked entities, shared with players' props.",
            },
            Meter {
                name: "Brush entities (models)",
                count: self.models,
                limit: 1024,
                help: "Each brush entity is a model of its own, except func_detail, \
                       func_ladder, func_occluder and areaportals; the compiler stops with \
                       \"MAX_MAP_MODELS\".",
            },
            Meter {
                name: "Lights",
                count: self.lights,
                limit: 8192,
                help: "World lights: one per light and spotlight, two for the sun. The \
                       lighting compiler stops with \"too many lights\"; glowing materials \
                       add more, and many lights make compiling slow.",
            },
            Meter {
                name: "Materials",
                count: materials.len(),
                limit: 2048,
                help: "Different materials on brushes; the compiler stops with \
                       \"MAX_MAP_TEXDATA\".",
            },
        ]
    }
}

/// True if `b` reaches past the map's edge.
fn beyond(b: &Aabb) -> bool {
    b.min.min_element() < -MAP_HALF_SIZE || b.max.max_element() > MAP_HALF_SIZE
}

impl Document {
    /// The map's size and how close it comes to the engine's limits.
    pub fn stats(&self) -> MapStats {
        let mut counts = Counts {
            entities: 1, // the world itself
            models: 1,   // the world is model 0
            ..Counts::default()
        };
        let mut first_sun = true;
        for (id, object) in self.objects() {
            match object {
                Object::Brush(brush) => {
                    counts.add_brush(brush.brush());
                    counts.materials.extend(
                        brush
                            .faces()
                            .iter()
                            .map(|f| f.material.to_ascii_uppercase()),
                    );
                    if beyond(&brush.brush().bounds()) {
                        counts.outside.insert(self.selectable(id, false));
                    }
                }
                Object::Shape(shape) => {
                    for piece in shape.pieces() {
                        counts.add_brush(piece);
                    }
                    counts
                        .materials
                        .insert(shape.look.material.to_ascii_uppercase());
                    if beyond(&shape.bounds()) {
                        counts.outside.insert(id);
                    }
                }
                Object::Entity(entity) => {
                    self.count_entity(id, entity, &mut counts, &mut first_sun)
                }
            }
        }
        MapStats {
            bounds: counts.bounds,
            outside: counts.outside.iter().copied().collect(),
            meters: counts.meters(),
        }
    }

    fn count_entity(
        &self,
        id: ObjectId,
        entity: &crate::EntityObject,
        counts: &mut Counts,
        first_sun: &mut bool,
    ) {
        let class = entity.classname.to_ascii_lowercase();
        counts.entities += 1;
        let brush_entity = self.has_brushes(id) || entity.has_kept_solids();
        if brush_entity && !no_model(&class) {
            counts.models += 1;
        }
        counts.lights += world_lights(&class, first_sun);
        if brush_entity {
            return;
        }
        // A prop fills its model's box; other point entities are where
        // their origin is.
        let placed = match entity.model() {
            Some(_) => self.bounds_of(id),
            None => entity.origin.map(|o| Aabb::from_corners(o, o)),
        };
        if let Some(b) = placed {
            counts.grow(b);
            if beyond(&b) {
                counts.outside.insert(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BrushObject, Command, EntityObject, MapObject};

    fn cube(at: f32, size: f32) -> Brush {
        Brush::cuboid(Aabb::from_corners(Vec3::splat(at), Vec3::splat(at + size))).unwrap()
    }

    fn meter(stats: &MapStats, name: &str) -> usize {
        stats.meters.iter().find(|m| m.name == name).unwrap().count
    }

    #[test]
    fn an_empty_map_counts_only_the_world() {
        let stats = Document::new().stats();
        assert_eq!(stats.bounds, None);
        assert!(stats.outside.is_empty());
        assert_eq!(meter(&stats, "Brushes"), 0);
        assert_eq!(meter(&stats, "Map entities"), 1);
        assert_eq!(meter(&stats, "Brush entities (models)"), 1);
    }

    #[test]
    fn brushes_sides_entities_and_lights_are_counted_as_the_compiler_would() {
        let detail = MapObject::Entity(
            EntityObject {
                classname: "func_detail".into(),
                origin: None,
                file_data: Vec::new(),
            },
            vec![BrushObject::new(cube(0.0, 64.0))],
        );
        let door = MapObject::Entity(
            EntityObject {
                classname: "func_door".into(),
                origin: None,
                file_data: Vec::new(),
            },
            vec![BrushObject::new(cube(100.0, 64.0))],
        );
        let mut doc = Document::from_map(vec![detail, door], Default::default()).unwrap();
        doc.execute(Command::AddBrushes(vec![cube(-200.0, 32.0)]))
            .unwrap();
        doc.execute(Command::AddEntity(EntityObject::point(
            "light",
            Vec3::ZERO,
            &[],
        )))
        .unwrap();
        doc.execute(Command::AddEntity(EntityObject::point(
            "prop_static",
            Vec3::ZERO,
            &[],
        )))
        .unwrap();
        let stats = doc.stats();
        assert_eq!(meter(&stats, "Brushes"), 3);
        assert_eq!(meter(&stats, "Brush sides"), 18, "a box has all six sides");
        // Every entity in the file counts: the world, func_detail, the
        // door, the light and the static prop.
        assert_eq!(meter(&stats, "Map entities"), 5);
        assert_eq!(meter(&stats, "Brush entities (models)"), 2);
        assert_eq!(meter(&stats, "Lights"), 1);
        assert_eq!(meter(&stats, "Materials"), 1);
        let b = stats.bounds.unwrap();
        assert_eq!(b.min, Vec3::splat(-200.0));
        assert_eq!(b.max, Vec3::splat(164.0));
    }

    #[test]
    fn a_slanted_brush_costs_extra_sides_and_objects_past_the_edge_are_listed() {
        let wedge = Brush::from_planes(&[
            halberd_geom::Plane::new(
                Vec3::new(1.0, 0.0, 1.0).normalize(),
                Vec3::new(32.0, 0.0, 32.0),
            )
            .unwrap(),
            halberd_geom::Plane::new(Vec3::NEG_X, Vec3::ZERO).unwrap(),
            halberd_geom::Plane::new(Vec3::NEG_Z, Vec3::ZERO).unwrap(),
            halberd_geom::Plane::new(Vec3::Y, Vec3::new(0.0, 64.0, 0.0)).unwrap(),
            halberd_geom::Plane::new(Vec3::NEG_Y, Vec3::ZERO).unwrap(),
        ])
        .unwrap();
        assert_eq!(compiled_sides(&wedge), 5 + 2, "+X and +Z are added");
        let mut doc = Document::new();
        doc.execute(Command::AddBrushes(vec![
            cube(16_000.0, 512.0),
            cube(0.0, 64.0),
        ]))
        .unwrap();
        let stats = doc.stats();
        assert_eq!(stats.outside.len(), 1);
        let far = stats.meters[0];
        assert!(far.fraction() < 0.01);
    }

    #[test]
    fn lights_count_as_the_lighting_compiler_makes_them() {
        let mut doc = Document::new();
        for class in [
            "light",
            "light_spot",
            "light_dynamic",
            "light_environment",
            "light_environment",
        ] {
            doc.execute(Command::AddEntity(EntityObject::point(
                class,
                Vec3::ZERO,
                &[],
            )))
            .unwrap();
        }
        // 1 + 1 + 0 (a real entity) + 2 (sunlight and sky light) + 0.
        assert_eq!(meter(&doc.stats(), "Lights"), 4);
    }

    #[test]
    fn the_edge_is_judged_by_what_crosses_it() {
        // A light whose origin is inside isn't flagged for its marker box;
        // a door past the edge is listed once, as the door.
        let door = MapObject::Entity(
            EntityObject {
                classname: "func_door".into(),
                origin: None,
                file_data: Vec::new(),
            },
            vec![
                BrushObject::new(cube(16_400.0, 64.0)),
                BrushObject::new(cube(16_500.0, 64.0)),
            ],
        );
        let ladder = MapObject::Entity(
            EntityObject {
                classname: "func_ladder".into(),
                origin: None,
                file_data: Vec::new(),
            },
            vec![BrushObject::new(cube(0.0, 32.0))],
        );
        let mut doc = Document::from_map(vec![door, ladder], Default::default()).unwrap();
        doc.execute(Command::AddEntity(EntityObject::point(
            "light",
            Vec3::new(16_380.0, 0.0, 0.0),
            &[],
        )))
        .unwrap();
        let stats = doc.stats();
        let door_id = doc.objects().next().unwrap().0;
        assert_eq!(stats.outside, vec![door_id]);
        assert_eq!(
            meter(&stats, "Brush entities (models)"),
            2,
            "the world and the door"
        );
    }
}
