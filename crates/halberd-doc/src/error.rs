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
}

impl fmt::Display for DocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NothingToDo => f.write_str("there is nothing to change"),
            Self::UnknownObject(id) => write!(f, "object {id} is not in the map"),
            Self::TooManyObjects => f.write_str("the map has reached its object limit"),
        }
    }
}

impl std::error::Error for DocError {}
