//! Edits to the map.

use crate::{BrushObject, EntityObject, Object, ObjectId};
use halberd_geom::{Brush, FaceFate};

/// An edit to the map. Every change goes through one, so it can be undone.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Adds brushes. They become the selection.
    AddBrushes(Vec<Brush>),
    /// Adds brushes with their faces' data (a chosen material). They
    /// become the selection.
    AddBrushObjects(Vec<BrushObject>),
    /// Adds a floor-plan shape. It becomes the selection.
    AddShape(crate::ShapeObject),
    /// Puts a material on faces. For each object, which of its faces (by
    /// number; `None`: all of them). A brush entity's faces are those of
    /// its brushes; a shape's are all one look. The selection does not
    /// change.
    SetMaterial {
        /// The material (`BRICK/BRICKWALL001A`).
        material: String,
        /// The objects and faces to put it on.
        faces: Vec<(ObjectId, Option<Vec<usize>>)>,
    },
    /// Removes objects. Removed objects leave the selection.
    Remove(Vec<ObjectId>),
    /// Replaces a brush's shape, for example after changing its size or
    /// position. The selection does not change.
    ReplaceBrush {
        /// The brush to change.
        id: ObjectId,
        /// Its new shape.
        brush: Brush,
    },
    /// Gives several brushes new shapes at once, after moving, rotating or
    /// scaling them. The selection does not change.
    TransformBrushes {
        /// What was done, for the Edit menu.
        kind: TransformKind,
        /// Each brush with its new shape.
        brushes: Vec<(ObjectId, Brush)>,
        /// Each floor-plan shape moved along with them, as it is now.
        shapes: Vec<(ObjectId, crate::ShapeObject)>,
        /// Each point entity moved (or turned) along with them, as it is
        /// now.
        entities: Vec<(ObjectId, EntityObject)>,
    },
    /// Adds a point entity, such as a prop. It becomes the selection.
    AddEntity(EntityObject),
    /// Changes an entity's settings (key and value pairs), adding those it
    /// lacks. The selection does not change.
    SetEntitySettings {
        /// The entity.
        id: ObjectId,
        /// The settings to change.
        settings: Vec<(String, String)>,
    },
    /// Ties brushes (and the brushes of brush entities among `objects`) to
    /// a new brush entity, as Hammer's "Tie to entity". Brush entities left
    /// with no brushes go. The selection does not change.
    TieToEntity {
        /// The brushes, or brush entities standing for theirs.
        objects: Vec<ObjectId>,
        /// The new entity (its class and settings; no origin needed).
        entity: EntityObject,
    },
    /// Replaces an entity's connections (its outputs to other entities'
    /// inputs). Lines Halberd can't read are kept.
    SetConnections {
        /// The entity.
        id: ObjectId,
        /// Its connections, in order.
        connections: Vec<crate::Connection>,
    },
    /// Moves brushes (or a brush entity's brushes) back to the world.
    /// Brush entities left with no brushes go.
    MoveToWorld(Vec<ObjectId>),
    /// Changes an entity's class.
    SetEntityClass {
        /// The entity.
        id: ObjectId,
        /// Its new class (letters, digits and `_`).
        classname: String,
    },
    /// Removes settings from an entity (never its class or origin).
    RemoveEntitySettings {
        /// The entity.
        id: ObjectId,
        /// The keys to remove (any case).
        keys: Vec<String>,
    },
    /// Changes one of the map's own settings (the world's, such as its
    /// sky, `skyname`).
    SetWorldSetting {
        /// The setting.
        key: String,
        /// Its new value.
        value: String,
    },
    /// Gives brushes new shapes after moving, rotating or scaling some of
    /// their corners, edges or faces. Each new shape comes with what became
    /// of each face ([`Brush::with_moved_points`]): a face that was split or
    /// merged loses its saved face id and displacement, which only fit the
    /// old face. The selection does not change.
    EditBrushes {
        /// What was done, for the Edit menu.
        kind: TransformKind,
        /// Which parts were edited, and how many.
        parts: (ElementKind, usize),
        /// Each brush with its new shape and what became of each face.
        brushes: Vec<(ObjectId, Brush, Vec<FaceFate>)>,
    },
    /// A shape operation (extrude, join, split…), as one undo step: some
    /// brushes get new shapes and face data, some are added, some removed.
    /// Changed brushes stay in their entity; added ones join the selection
    /// (and objects an undo brings back join it too).
    ShapeEdit {
        /// What was done, for the Edit menu ("Extrude face").
        label: String,
        /// Brushes with their new shape and face data.
        changed: Vec<(ObjectId, BrushObject)>,
        /// New brushes, each with the brush entity it joins.
        added: Vec<AddedBrush>,
        /// Objects taken out.
        removed: Vec<ObjectId>,
        /// True if the added brushes join the selection (and, on undo, the
        /// objects brought back do); false leaves the selection alone.
        select_added: bool,
    },
}

/// What a [`Command::ShapeEdit`] adds: a brush, and the brush entity it
/// belongs to (`None`: a world brush).
pub type AddedBrush = (BrushObject, Option<ObjectId>);

/// A part of a brush that can be picked and edited on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElementKind {
    /// A corner.
    Vertex,
    /// An edge between two corners.
    Edge,
    /// A flat side.
    Face,
}

impl ElementKind {
    /// The part's name, for one or several ("vertex", "vertices").
    pub fn name(self, count: usize) -> &'static str {
        match (self, count == 1) {
            (Self::Vertex, true) => "vertex",
            (Self::Vertex, false) => "vertices",
            (Self::Edge, true) => "edge",
            (Self::Edge, false) => "edges",
            (Self::Face, true) => "face",
            (Self::Face, false) => "faces",
        }
    }
}

/// What a [`Command::TransformBrushes`] did, for its name in the Edit menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformKind {
    /// Moved.
    Move,
    /// Rotated.
    Rotate,
    /// Scaled or resized.
    Scale,
    /// Mirrored.
    Mirror,
    /// Corners merged into one.
    Merge,
    /// Parts taken away.
    Delete,
}

impl TransformKind {
    /// The verb for the Edit menu ("Move").
    pub fn verb(self) -> &'static str {
        match self {
            Self::Move => "Move",
            Self::Rotate => "Rotate",
            Self::Scale => "Scale",
            Self::Mirror => "Mirror",
            Self::Merge => "Merge",
            Self::Delete => "Delete",
        }
    }
}

impl Command {
    /// A short description for the Edit menu and the Console, such as
    /// "Create box" or "Delete 3 objects".
    pub fn describe(&self) -> String {
        match self {
            Self::AddBrushes(brushes) if brushes.len() == 1 => "Create brush".into(),
            Self::AddBrushes(brushes) => format!("Create {} brushes", brushes.len()),
            Self::AddBrushObjects(brushes) if brushes.len() == 1 => "Create brush".into(),
            Self::AddBrushObjects(brushes) => format!("Create {} brushes", brushes.len()),
            Self::AddShape(_) => "Create floor plan shape".into(),
            Self::AddEntity(entity) if entity.classname.starts_with("prop_") => "Place prop".into(),
            Self::AddEntity(entity) => format!("Add {}", entity.classname),
            Self::SetEntitySettings { .. } => "Change entity settings".into(),
            Self::TieToEntity { entity, .. } => format!("Tie to {}", entity.classname),
            Self::MoveToWorld(_) => "Move to world".into(),
            Self::SetConnections { .. } => "Change outputs".into(),
            Self::SetEntityClass { classname, .. } => format!("Change class to {classname}"),
            Self::RemoveEntitySettings { keys, .. } if keys.len() == 1 => {
                format!("Remove {}", keys[0])
            }
            Self::RemoveEntitySettings { .. } => "Remove entity settings".into(),
            Self::SetWorldSetting { key, .. } if key.eq_ignore_ascii_case("skyname") => {
                "Change sky".into()
            }
            Self::SetWorldSetting { key, .. } => format!("Change map setting {key}"),
            Self::SetMaterial { faces, .. } => {
                let picked: Option<usize> = faces
                    .iter()
                    .map(|(_, f)| f.as_ref().map(Vec::len))
                    .sum::<Option<usize>>();
                match picked {
                    Some(1) => "Apply material to face".into(),
                    Some(n) => format!("Apply material to {n} faces"),
                    None => "Apply material".into(),
                }
            }
            Self::ReplaceBrush { .. } => "Change brush".into(),
            Self::ShapeEdit { label, .. } => label.clone(),
            Self::EditBrushes {
                kind,
                parts: (part, count),
                ..
            } => {
                if *count == 1 {
                    format!("{} {}", kind.verb(), part.name(1))
                } else {
                    format!("{} {count} {}", kind.verb(), part.name(*count))
                }
            }
            Self::TransformBrushes {
                kind,
                brushes,
                shapes,
                entities,
            } => {
                let verb = kind.verb();
                match (brushes.len(), shapes.len(), entities.len()) {
                    (1, 0, 0) => format!("{verb} brush"),
                    (n, 0, 0) => format!("{verb} {n} brushes"),
                    (0, 1, 0) => format!("{verb} shape"),
                    (0, n, 0) => format!("{verb} {n} shapes"),
                    (0, 0, 1) => format!("{verb} entity"),
                    (0, 0, n) => format!("{verb} {n} entities"),
                    (b, s, e) => format!("{verb} {} objects", b + s + e),
                }
            }
            Self::Remove(ids) => {
                // The same id twice still removes one object.
                let count = ids.iter().collect::<std::collections::BTreeSet<_>>().len();
                if count == 1 {
                    "Delete 1 object".into()
                } else {
                    format!("Delete {count} objects")
                }
            }
        }
    }
}

/// What an applied command did, kept so it can be reversed. Objects carry
/// their ids, so undo and redo bring back the very same objects.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Change {
    /// These objects were put into the map (and selected).
    Inserted(Vec<(ObjectId, Object)>),
    /// These objects were taken out of the map.
    Removed(Vec<(ObjectId, Object)>),
    /// These objects changed: each with how it was before and after.
    Modified(Vec<(ObjectId, Object, Object)>),
    /// A world setting changed: its value before and after (`None`: not
    /// set).
    World {
        /// The setting.
        key: String,
        /// Its value before.
        before: Option<String>,
        /// Its value after.
        after: Option<String>,
    },
    /// Several changes, in order. Objects they insert join the selection
    /// if `select_added`, else the selection is left alone.
    Several {
        /// The changes.
        parts: Vec<Change>,
        /// Whether inserted objects join the selection.
        select_added: bool,
    },
}

impl Change {
    /// The change that undoes this one.
    pub(crate) fn inverse(self) -> Self {
        match self {
            Self::Inserted(objects) => Self::Removed(objects),
            Self::Removed(objects) => Self::Inserted(objects),
            Self::Modified(objects) => Self::Modified(
                objects
                    .into_iter()
                    .map(|(id, before, after)| (id, after, before))
                    .collect(),
            ),
            Self::Several {
                parts,
                select_added,
            } => Self::Several {
                parts: parts.into_iter().rev().map(Self::inverse).collect(),
                select_added,
            },
            Self::World { key, before, after } => Self::World {
                key,
                before: after,
                after: before,
            },
        }
    }

    /// True if the change leaves everything as it was (a drag that came back
    /// to where it started).
    pub(crate) fn is_noop(&self) -> bool {
        match self {
            Self::Modified(objects) => objects.iter().all(|(_, before, after)| before == after),
            Self::Inserted(objects) | Self::Removed(objects) => objects.is_empty(),
            Self::Several { parts, .. } => parts.iter().all(Self::is_noop),
            Self::World { before, after, .. } => before == after,
        }
    }

    /// This change followed by `later`, as one change, when both modify
    /// objects (or both set the same world setting). Used to make a whole
    /// drag one undo step.
    pub(crate) fn merged_with(&self, later: &Change) -> Option<Change> {
        if let (
            Self::World {
                key, before: first, ..
            },
            Self::World {
                key: later_key,
                after: last,
                ..
            },
        ) = (self, later)
        {
            return (key == later_key).then(|| Self::World {
                key: key.clone(),
                before: first.clone(),
                after: last.clone(),
            });
        }
        let (Self::Modified(first), Self::Modified(second)) = (self, later) else {
            return None;
        };
        // Each object keeps how it was before the first change and how it
        // is after the last; one changed by only one of them keeps that
        // change (a sculpt stroke can run onto further brushes).
        let mut merged = first.clone();
        for (id, before, after) in second {
            match merged.iter_mut().find(|(m, ..)| m == id) {
                Some(entry) => entry.2 = after.clone(),
                None => merged.push((*id, before.clone(), after.clone())),
            }
        }
        Some(Self::Modified(merged))
    }
}
