//! The things a map is made of.

use glam::Vec3;
use halberd_geom::{Aabb, Brush, FaceFate, Plane};
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
    /// The brush entity (such as `func_detail`) this brush belongs to, or
    /// `None` for a world brush. Set by the document.
    entity: Option<ObjectId>,
}

impl BrushObject {
    /// A new brush with the default material on every face.
    pub fn new(brush: Brush) -> Self {
        let faces = vec![FaceInfo::default(); brush.faces().len()];
        Self {
            brush,
            faces,
            file_data: Vec::new(),
            entity: None,
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
            entity: None,
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
        Self {
            entity: self.entity,
            ..Self::from_parts(brush, &self.faces, self.file_data.clone())
        }
    }

    /// The same brush with a new shape from editing its corners, edges or
    /// faces ([`Brush::with_moved_points`]), with what became of each face.
    /// Faces keep their material and file data, except that a face that was
    /// split or merged (not `whole`) loses its saved id and displacement:
    /// two faces must not share an id, and a displacement only fits the
    /// face it was made for. A new id is given when saving.
    pub fn with_shape_mapped(&self, brush: Brush, fates: &[FaceFate]) -> Self {
        let mut shaped = self.with_shape(brush);
        for (info, fate) in shaped.faces.iter_mut().zip(fates) {
            if !fate.whole {
                info.file_data.retain(|entry| match entry {
                    Entry::Pair(key, _) => !key.eq_ignore_ascii_case("id"),
                    Entry::Block(block) => !block.name.eq_ignore_ascii_case("dispinfo"),
                });
            }
        }
        shaped
    }

    /// The brush entity this brush belongs to, or `None` for a world brush.
    pub fn entity(&self) -> Option<ObjectId> {
        self.entity
    }

    pub(crate) fn set_entity(&mut self, entity: Option<ObjectId>) {
        self.entity = entity;
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
    /// Everything a map file stores for the entity besides its brushes
    /// (settings, outputs, editor data), kept exactly as read and in order.
    /// Halberd does not edit entities yet.
    pub file_data: Vec<Entry>,
}

impl EntityObject {
    /// The box a point entity is shown as, around its origin. (Brush
    /// entities are shown by their brushes alone, even when they have an
    /// origin; the document knows which entities have brushes.)
    pub fn marker(&self) -> Option<Aabb> {
        let origin = self.origin?;
        let half = Vec3::splat(POINT_ENTITY_HALF_SIZE);
        Some(Aabb::from_corners(origin - half, origin + half))
    }

    /// True if the entity's file data holds brushes Halberd could not show
    /// (kept as read, to be saved back).
    pub fn has_kept_solids(&self) -> bool {
        self.file_data
            .iter()
            .any(|e| matches!(e, Entry::Block(b) if b.name.eq_ignore_ascii_case("solid")))
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
    /// A brush: a world brush, or one belonging to a brush entity (see
    /// [`BrushObject::entity`]).
    Brush(BrushObject),
    /// An entity. A brush entity's brushes are objects of their own.
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

    /// The box around the object itself: a brush's shape, or an entity's
    /// marker. A brush entity without an origin has none; see
    /// `Document::bounds_of` for the box around its brushes.
    pub fn bounds(&self) -> Option<Aabb> {
        match self {
            Self::Brush(b) => Some(b.brush().bounds()),
            Self::Entity(e) => e.marker(),
        }
    }

    /// Distance along a ray to where it hits the object itself (a brush,
    /// or an entity's marker), if it does.
    pub fn ray_hit(&self, origin: Vec3, direction: Vec3) -> Option<f32> {
        match self {
            Self::Brush(b) => b.brush().ray_hit(origin, direction),
            Self::Entity(e) => e
                .marker()
                .and_then(|m| Brush::cuboid(m).ok())
                .and_then(|m| m.ray_hit(origin, direction)),
        }
    }

    /// The brush, if this is a brush.
    pub fn as_brush(&self) -> Option<&BrushObject> {
        match self {
            Self::Brush(b) => Some(b),
            Self::Entity(_) => None,
        }
    }

    /// The entity, if this is an entity.
    pub fn as_entity(&self) -> Option<&EntityObject> {
        match self {
            Self::Entity(e) => Some(e),
            Self::Brush(_) => None,
        }
    }
}

/// One thing read from a map file, for [`crate::Document::from_map`]: a
/// world brush, or an entity with the brushes it owns.
#[derive(Debug, Clone, PartialEq)]
pub enum MapObject {
    /// A world brush.
    Brush(BrushObject),
    /// An entity and its brushes (none for a point entity).
    Entity(EntityObject, Vec<BrushObject>),
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
    fn a_point_entity_is_a_small_box() {
        let point = Object::Entity(EntityObject {
            classname: "light".into(),
            origin: Some(Vec3::new(100.0, 0.0, 50.0)),
            file_data: vec![Entry::Pair("classname".into(), "light".into())],
        });
        let bounds = point.bounds().unwrap();
        assert_eq!(bounds.center(), Vec3::new(100.0, 0.0, 50.0));
        assert_eq!(bounds.size(), Vec3::splat(16.0));
        assert!(
            point
                .ray_hit(Vec3::new(100.0, 0.0, 500.0), Vec3::NEG_Z)
                .is_some()
        );
        assert_eq!(point.kind_name(), "Entity");
        let e = point.as_entity().unwrap();
        assert_eq!(e.settings().collect::<Vec<_>>(), [("classname", "light")]);
        assert!(point.as_brush().is_none());

        let detail = Object::Entity(EntityObject {
            classname: "func_detail".into(),
            origin: None,
            file_data: Vec::new(),
        });
        assert_eq!(detail.bounds(), None, "shown by its brushes only");
    }

    #[test]
    fn a_brush_keeps_its_entity_through_reshaping() {
        let mut b = BrushObject::new(cube(0.0));
        assert_eq!(b.entity(), None);
        b.set_entity(Some(ObjectId(7)));
        let moved = b.with_shape(cube(32.0));
        assert_eq!(moved.entity(), Some(ObjectId(7)));
    }

    #[test]
    fn split_faces_lose_their_id_and_displacement_but_keep_their_material() {
        use halberd_kv::Block;
        let info: Vec<FaceInfo> = (0..6)
            .map(|i| FaceInfo {
                material: format!("MAT{i}"),
                file_data: vec![
                    Entry::Pair("id".into(), (i + 1).to_string()),
                    Entry::Pair("material".into(), format!("MAT{i}")),
                    Entry::Block(Block::new("dispinfo")),
                ],
                ..FaceInfo::default()
            })
            .collect();
        let object = BrushObject::from_parts(cube(0.0), &info, Vec::new());
        let lifted = object
            .brush()
            .points()
            .iter()
            .position(|p| *p == Vec3::splat(64.0))
            .unwrap();
        let (bent, fates) = object
            .brush()
            .with_moved_points(&[(lifted, Vec3::new(80.0, 80.0, 96.0))])
            .unwrap();
        let edited = object.with_shape_mapped(bent, &fates);
        assert_eq!(edited.faces().len(), 9);
        for (info, fate) in edited.faces().iter().zip(&fates) {
            assert!(info.material.starts_with("MAT"));
            let has_id = info
                .file_data
                .iter()
                .any(|e| matches!(e, Entry::Pair(k, _) if k == "id"));
            let has_disp = info
                .file_data
                .iter()
                .any(|e| matches!(e, Entry::Block(b) if b.name == "dispinfo"));
            assert_eq!(has_id, fate.whole);
            assert_eq!(has_disp, fate.whole);
        }
        // No two faces share an id.
        let ids: Vec<&String> = edited
            .faces()
            .iter()
            .flat_map(|f| &f.file_data)
            .filter_map(|e| match e {
                Entry::Pair(k, v) if k == "id" => Some(v),
                _ => None,
            })
            .collect();
        let unique: std::collections::BTreeSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len());
    }
}
