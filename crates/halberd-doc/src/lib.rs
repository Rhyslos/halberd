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
//! Objects are brushes ([`BrushObject`], with each face's material) and
//! entities ([`EntityObject`]). A brush entity's brushes (a `func_detail`'s,
//! say) are objects of their own that name their entity, so they can be
//! selected and edited one by one, as with Hammer's "Ignore groups". What a map
//! file holds that the editor does not use yet is kept in
//! [`MapFileData`] and on each object, so saving loses nothing.

mod command;
mod document;
mod error;
mod history;
mod map_data;
mod object;

pub use command::{Command, ElementKind, TransformKind};
pub use document::{Document, MAX_OBJECTS};
pub use error::DocError;
pub use history::MAX_UNDO_STEPS;
pub use map_data::{MapFileData, TextEncoding};
pub use object::{
    BrushObject, DEFAULT_MATERIAL, EntityObject, FaceInfo, MapObject, Object, ObjectId,
    POINT_ENTITY_HALF_SIZE,
};
