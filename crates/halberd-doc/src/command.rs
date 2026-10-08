//! Edits to the map.

use crate::{Object, ObjectId};
use halberd_geom::Brush;

/// An edit to the map. Every change goes through one, so it can be undone.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Adds brushes. They become the selection.
    AddBrushes(Vec<Brush>),
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
    },
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
}

impl Command {
    /// A short description for the Edit menu and the Console, such as
    /// "Create box" or "Delete 3 objects".
    pub fn describe(&self) -> String {
        match self {
            Self::AddBrushes(brushes) if brushes.len() == 1 => "Create brush".into(),
            Self::AddBrushes(brushes) => format!("Create {} brushes", brushes.len()),
            Self::ReplaceBrush { .. } => "Change brush".into(),
            Self::TransformBrushes { kind, brushes } => {
                let verb = match kind {
                    TransformKind::Move => "Move",
                    TransformKind::Rotate => "Rotate",
                    TransformKind::Scale => "Scale",
                };
                if brushes.len() == 1 {
                    format!("{verb} brush")
                } else {
                    format!("{verb} {} brushes", brushes.len())
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
        }
    }

    /// True if the change leaves everything as it was (a drag that came back
    /// to where it started).
    pub(crate) fn is_noop(&self) -> bool {
        match self {
            Self::Modified(objects) => objects.iter().all(|(_, before, after)| before == after),
            Self::Inserted(objects) | Self::Removed(objects) => objects.is_empty(),
        }
    }

    /// This change followed by `later`, as one change, when both modify the
    /// same objects. Used to make a whole drag one undo step.
    pub(crate) fn merged_with(&self, later: &Change) -> Option<Change> {
        let (Self::Modified(first), Self::Modified(second)) = (self, later) else {
            return None;
        };
        let same_objects =
            first.len() == second.len() && first.iter().zip(second).all(|(a, b)| a.0 == b.0);
        same_objects.then(|| {
            Self::Modified(
                first
                    .iter()
                    .zip(second)
                    .map(|((id, before, _), (_, _, after))| (*id, before.clone(), after.clone()))
                    .collect(),
            )
        })
    }
}
