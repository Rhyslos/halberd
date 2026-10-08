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
}

impl Command {
    /// A short description for the Edit menu and the Console, such as
    /// "Create box" or "Delete 3 objects".
    pub fn describe(&self) -> String {
        match self {
            Self::AddBrushes(brushes) if brushes.len() == 1 => "Create brush".into(),
            Self::AddBrushes(brushes) => format!("Create {} brushes", brushes.len()),
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
}

impl Change {
    /// The change that undoes this one.
    pub(crate) fn inverse(self) -> Self {
        match self {
            Self::Inserted(objects) => Self::Removed(objects),
            Self::Removed(objects) => Self::Inserted(objects),
        }
    }
}
