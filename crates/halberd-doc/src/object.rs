//! The things a map is made of.

use glam::Vec3;
use halberd_geom::{Aabb, Brush, Plane};
use halberd_kv::Entry;
use std::fmt;

/// Material given to new brush faces: Hammer's grey measuring texture,
/// which every GMod install has.
pub const DEFAULT_MATERIAL: &str = "DEV/DEV_MEASUREGENERIC01B";
/// Half the size of the box a point entity is shown as, in units.
pub const POINT_ENTITY_HALF_SIZE: f32 = 8.0;

/// A map object's identity. Stays the same for the object's whole life,
/// through undo and redo, and is never given to another object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId(pub(crate) u64);

impl ObjectId {
    /// The number behind the id.
    pub fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// What a brush face carries besides its shape.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceInfo {
    /// The material (texture) on the face, such as `BRICK/BRICKWALL001A`.
    pub material: String,
    /// Everything else a map file stores for the face (its id, texture
    /// alignment, lightmap scale, displacement…), kept exactly as read so
    /// saving loses nothing. Halberd does not change these yet.
    pub file_data: Vec<Entry>,
    /// The face's plane exactly as the file wrote it, so an unchanged face
    /// is written back with the very same text.
    pub file_plane: Option<String>,
    /// The face's plane as the brush had it just after reading, so saving
    /// can tell exactly (not just within rounding) whether the face moved.
    pub loaded_plane: Option<Plane>,
}

impl Default for FaceInfo {
    fn default() -> Self {
        Self {
            material: DEFAULT_MATERIAL.to_string(),
            file_data: Vec::new(),
            file_plane: None,
            loaded_plane: None,
        }
    }
}

/// A brush in the map: its shape, and what each face carries.
#[derive(Debug, Clone, PartialEq)]
pub struct BrushObject {
    brush: Brush,
    faces: Vec<FaceInfo>,
    /// What a map file stores for the brush besides its faces (its id,
    /// Hammer's editor colour and visgroups), kept as read.
    pub file_data: Vec<Entry>,
}

impl BrushObject {
    /// A new brush with the default material on every face.
    pub fn new(brush: Brush) -> Self {
        let faces = vec![FaceInfo::default(); brush.faces().len()];
        Self {
            brush,
            faces,
            file_data: Vec::new(),
        }
    }

    /// A brush with face data given for each of the planes it was built
    /// from: `by_plane[i]` belongs to the face whose source is `i` (see
    /// `halberd_geom::Face::source`). Faces without data get the default.
    pub fn from_parts(brush: Brush, by_plane: &[FaceInfo], file_data: Vec<Entry>) -> Self {
        let faces = brush
            .faces()
            .iter()
            .map(|f| by_plane.get(f.source()).cloned().unwrap_or_default())
            .collect();
        Self {
            brush,
            faces,
            file_data,
        }
    }

    /// The shape.
    pub fn brush(&self) -> &Brush {
        &self.brush
    }

    /// What each face carries, in the same order as `brush().faces()`.
    pub fn faces(&self) -> &[FaceInfo] {
        &self.faces
    }

    /// The same brush with a new shape made from this one's faces (moved,
    /// turned or resized), keeping each face's material and file data.
    pub fn with_shape(&self, brush: Brush) -> Self {
        Self::from_parts(brush, &self.faces, self.file_data.clone())
    }
}

/// An entity: something placed in the map with settings, such as a spawn
/// point, a light, or a brush entity like `func_detail`.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityObject {
    /// What kind of entity, such as `info_player_start`.
    pub classname: String,
    /// Where a point entity is; brush entities may have none.
    pub origin: Option<Vec3>,
    /// The entity's own brushes (for brush entities).
    pub solids: Vec<BrushObject>,
    /// Everything a map file stores for the entity besides its brushes
    /// (settings, outputs, editor data), kept exactly as read and in order.
    /// Halberd does not edit entities yet.
    pub file_data: Vec<Entry>,
}

impl EntityObject {
    /// The box a point entity is shown as. Brush entities are shown by
    /// their brushes alone, even when they have an origin.
    pub fn marker(&self) -> Option<Aabb> {
        if !self.solids.is_empty() {
            return None;
        }
        let origin = self.origin?;
        let half = Vec3::splat(POINT_ENTITY_HALF_SIZE);
        Some(Aabb::from_corners(origin - half, origin + half))
    }

    /// The entity's settings (key and value pairs), in order.
    pub fn settings(&self) -> impl Iterator<Item = (&str, &str)> {
        self.file_data.iter().filter_map(|e| match e {
            Entry::Pair(k, v) => Some((k.as_str(), v.as_str())),
            Entry::Block(_) => None,
        })
    }
}

/// One thing in the map.
#[derive(Debug, Clone, PartialEq)]
pub enum Object {
    /// A world brush.
    Brush(BrushObject),
    /// An entity, with any brushes it owns.
    Entity(EntityObject),
}

impl Object {
    /// What kind of object this is, for lists such as the Scene panel.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Brush(_) => "Brush",
            Self::Entity(_) => "Entity",
        }
    }

    /// The smallest axis-aligned box around the object.
    pub fn bounds(&self) -> Aabb {
        match self {
            Self::Brush(b) => b.brush().bounds(),
            Self::Entity(e) => e
                .solids
                .iter()
                .map(|s| s.brush().bounds())
                .chain(e.marker())
                .reduce(Aabb::union)
                .unwrap_or_else(|| Aabb::from_corners(Vec3::ZERO, Vec3::ZERO)),
        }
    }

    /// Distance along a ray to where it hits the object, if it does.
    pub fn ray_hit(&self, origin: Vec3, direction: Vec3) -> Option<f32> {
        match self {
            Self::Brush(b) => b.brush().ray_hit(origin, direction),
            Self::Entity(e) => {
                let marker = e
                    .marker()
                    .and_then(|m| Brush::cuboid(m).ok())
                    .and_then(|m| m.ray_hit(origin, direction));
                e.solids
                    .iter()
                    .filter_map(|s| s.brush().ray_hit(origin, direction))
                    .chain(marker)
                    .reduce(f32::min)
            }
        }
    }

    /// The brush, if this is a world brush.
    pub fn as_brush(&self) -> Option<&BrushObject> {
        match self {
            Self::Brush(b) => Some(b),
            Self::Entity(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halberd_geom::Plane;

    fn cube(x: f32) -> Brush {
        Brush::cuboid(Aabb::from_corners(Vec3::splat(x), Vec3::splat(x + 64.0))).unwrap()
    }

    #[test]
    fn face_data_follows_faces_through_moves_and_rebuilds() {
        let planes: Vec<Plane> = cube(0.0).faces().iter().map(|f| f.plane()).collect();
        let info: Vec<FaceInfo> = (0..6)
            .map(|i| FaceInfo {
                material: format!("MAT{i}"),
                ..FaceInfo::default()
            })
            .collect();
        let brush = Brush::from_planes(&planes).unwrap();
        let object = BrushObject::from_parts(brush, &info, Vec::new());
        let top = |o: &BrushObject| {
            o.brush()
                .faces()
                .iter()
                .zip(o.faces())
                .find(|(f, _)| f.plane().normal.z > 0.9)
                .map(|(_, i)| i.material.clone())
                .unwrap()
        };
        let before = top(&object);
        let moved = object.with_shape(object.brush().translated(Vec3::X * 32.0).unwrap());
        assert_eq!(top(&moved), before);
        let turned = moved.with_shape(
            moved
                .brush()
                .rotated(Vec3::X, std::f32::consts::PI, moved.brush().center())
                .unwrap(),
        );
        // Turned upside down: the old top is now at the bottom.
        let bottom = turned
            .brush()
            .faces()
            .iter()
            .zip(turned.faces())
            .find(|(f, _)| f.plane().normal.z < -0.9)
            .map(|(_, i)| i.material.clone())
            .unwrap();
        assert_eq!(bottom, before);
    }

    #[test]
    fn new_brushes_get_the_default_material() {
        let b = BrushObject::new(cube(0.0));
        assert_eq!(b.faces().len(), 6);
        assert!(b.faces().iter().all(|f| f.material == DEFAULT_MATERIAL));
    }

    #[test]
    fn a_point_entity_is_a_small_box_and_a_brush_entity_is_its_brushes() {
        let point = Object::Entity(EntityObject {
            classname: "light".into(),
            origin: Some(Vec3::new(100.0, 0.0, 50.0)),
            solids: Vec::new(),
            file_data: vec![Entry::Pair("classname".into(), "light".into())],
        });
        assert_eq!(point.bounds().center(), Vec3::new(100.0, 0.0, 50.0));
        assert_eq!(point.bounds().size(), Vec3::splat(16.0));
        assert!(
            point
                .ray_hit(Vec3::new(100.0, 0.0, 500.0), Vec3::NEG_Z)
                .is_some()
        );
        assert_eq!(point.kind_name(), "Entity");
        let Object::Entity(e) = &point else { panic!() };
        assert_eq!(e.settings().collect::<Vec<_>>(), [("classname", "light")]);

        let detail = Object::Entity(EntityObject {
            classname: "func_detail".into(),
            origin: None,
            solids: vec![BrushObject::new(cube(0.0)), BrushObject::new(cube(100.0))],
            file_data: Vec::new(),
        });
        assert_eq!(detail.bounds().size(), Vec3::new(164.0, 164.0, 164.0));
        assert!(
            detail
                .ray_hit(Vec3::new(132.0, 132.0, 500.0), Vec3::NEG_Z)
                .is_some()
        );
        assert!(detail.as_brush().is_none());
    }
}
