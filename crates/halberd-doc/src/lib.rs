//! The map document: objects, selection, commands and undo.
//!
//! A [`Document`] is the single source of truth for an open map. Every
//! object has an [`ObjectId`] that never changes and is never reused. The
//! map changes only through a [`Command`] passed to [`Document::execute`],
//! which records how to reverse it, so every edit can be undone and redone.
//!
//! The selection lives in the document too, but is not part of the map:
//! selecting is not an edit and is not undone (as in Hammer). Undoing an
//! edit does select what it brings back.
//!
//! So far the only objects are brushes; props and entities follow.

mod command;
mod document;
mod error;
mod history;
mod object;

pub use command::{Command, TransformKind};
pub use document::{Document, MAX_OBJECTS};
pub use error::DocError;
pub use history::MAX_UNDO_STEPS;
pub use object::{Object, ObjectId};
