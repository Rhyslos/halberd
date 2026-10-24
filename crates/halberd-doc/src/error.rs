//! Why an edit could not be made.

use crate::ObjectId;
use std::fmt;

/// Why an edit could not be made. Nothing changes when one is returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocError {
    /// The command would change nothing (for example, deleting an empty
    /// selection).
    NothingToDo,
    /// The command names an object that is not in the map.
    UnknownObject(ObjectId),
    /// The map would hold more than [`crate::MAX_OBJECTS`] objects.
    TooManyObjects,
    /// The command changes a brush's shape, but the object is not a world
    /// brush.
    NotABrush(ObjectId),
    /// The object is not an entity, so nothing can join it.
    NotAnEntity(ObjectId),
    /// An entity would have more than [`crate::MAX_CONNECTIONS`] outputs.
    TooManyConnections,
}

impl fmt::Display for DocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NothingToDo => f.write_str("there is nothing to change"),
            Self::UnknownObject(id) => write!(f, "object {id} is not in the map"),
            Self::TooManyObjects => f.write_str("the map has reached its object limit"),
            Self::NotABrush(id) => write!(f, "object {id} is not a brush"),
            Self::NotAnEntity(id) => write!(f, "object {id} is not an entity"),
            Self::TooManyConnections => write!(
                f,
                "an entity can have at most {} outputs",
                crate::MAX_CONNECTIONS
            ),
        }
    }
}

impl std::error::Error for DocError {}
