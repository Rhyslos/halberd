//! How big the map is and how close it comes to what Source can compile:
//! its size against the ±16384-unit edge, and counts against the engine's
//! limits (brushes, brush sides, entities, lights, brush models,
//! materials). Counts are worked out from the map as Halberd holds it;
//! the compiler's own counts can differ a little (it splits and merges
//! some things), so they are estimates.

use crate::{Document, Object, ObjectId};
use glam::Vec3;
use halberd_geom::{Aabb, Brush};
use std::collections::HashSet;

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

/// Classes the compiler turns into world lights.
fn is_light(class: &str) -> bool {
    matches!(
        class,
        "light" | "light_spot" | "light_environment" | "light_dynamic"
    )
}

/// Brush entities the compiler folds into the world (no model of their
/// own).
fn merged_into_world(class: &str) -> bool {
    matches!(class, "func_detail" | "func_viewcontrol")
}

/// Brush sides as the compiler stores them: a brush's faces, plus a side
/// for each of the six box directions it lacks (the compiler adds them to
/// keep collisions exact).
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
                .any(|f| f.plane().normal.dot(**axis) > 0.9999)
        })
        .count();
    brush.faces().len() + missing
}

impl Document {
    /// The map's size and how close it comes to the engine's limits.
    pub fn stats(&self) -> MapStats {
        let mut brushes = 0usize;
        let mut sides = 0usize;
        let mut entities = 1usize; // the world itself
        let mut lights = 0usize;
        let mut models = 1usize; // the world is model 0
        let mut materials: HashSet<String> = HashSet::new();
        let mut bounds: Option<Aabb> = None;
        let mut outside = Vec::new();
        let mut grow = |b: Aabb| {
            bounds = Some(bounds.map_or(b, |a| a.union(b)));
        };
        let beyond =
            |b: &Aabb| b.min.min_element() < -MAP_HALF_SIZE || b.max.max_element() > MAP_HALF_SIZE;
        for (id, object) in self.objects() {
            match object {
                Object::Brush(brush) => {
                    brushes += 1;
                    sides += compiled_sides(brush.brush());
                    materials.extend(
                        brush
                            .faces()
                            .iter()
                            .map(|f| f.material.to_ascii_uppercase()),
                    );
                    let b = brush.brush().bounds();
                    grow(b);
                    if beyond(&b) {
                        outside.push(id);
                    }
                }
                Object::Shape(shape) => {
                    brushes += shape.pieces().len();
                    sides += shape.pieces().iter().map(compiled_sides).sum::<usize>();
                    materials.insert(shape.look.material.to_ascii_uppercase());
                    let b = shape.bounds();
                    grow(b);
                    if beyond(&b) {
                        outside.push(id);
                    }
                }
                Object::Entity(entity) => {
                    let class = entity.classname.to_ascii_lowercase();
                    let brush_entity = self.has_brushes(id);
                    if brush_entity && merged_into_world(&class) {
                        continue;
                    }
                    // Static props are compiled into a list of their own,
                    // not into the entities.
                    if class != "prop_static" {
                        entities += 1;
                    }
                    if brush_entity {
                        models += 1;
                    }
                    if is_light(&class) {
                        lights += 1;
                    }
                    if let Some(b) = self.bounds_of(id).filter(|_| !brush_entity) {
                        grow(b);
                        if beyond(&b) {
                            outside.push(id);
                        }
                    }
                }
            }
        }
        materials.retain(|m| !m.trim().is_empty());
        let meters = vec![
            Meter {
                name: "Brushes",
                count: brushes,
                limit: 8192,
                help: "The compiler stops with \"MAX_MAP_BRUSHES\". Join small brushes, or \
                       use props for detail.",
            },
            Meter {
                name: "Brush sides",
                count: sides,
                limit: 65_536,
                help: "The compiler stops with \"MAX_MAP_BRUSHSIDES\". Each brush counts its \
                       faces plus up to six more; slanted and many-sided brushes cost most.",
            },
            Meter {
                name: "Map entities",
                count: entities,
                limit: 8192,
                help: "Garry's Mod allows 8192 networked entities while playing, and the \
                       map's own count towards them, leaving less room for players' props.",
            },
            Meter {
                name: "Brush entities (models)",
                count: models,
                limit: 1024,
                help: "Each brush entity except func_detail is a model of its own; the \
                       compiler stops with \"MAX_MAP_MODELS\".",
            },
            Meter {
                name: "Lights",
                count: lights,
                limit: 8192,
                help: "The compiler stops with \"MAX_MAP_WORLDLIGHTS\"; long before that, \
                       many lights make compiling slow.",
            },
            Meter {
                name: "Materials",
                count: materials.len(),
                limit: 2048,
                help: "Different materials on brushes; the compiler stops with \
                       \"MAX_MAP_TEXDATA\".",
            },
        ];
        MapStats {
            bounds,
            outside,
            meters,
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
        let stats = doc.stats();
        assert_eq!(meter(&stats, "Brushes"), 3);
        assert_eq!(meter(&stats, "Brush sides"), 18, "a box has all six sides");
        // The world, the door and the light; func_detail joins the world,
        // and static props are compiled separately.
        assert_eq!(meter(&stats, "Map entities"), 3);
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
}
