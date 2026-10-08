//! Opening and saving maps.
//!
//! Converts between the editor's document and Hammer's VMF files:
//!
//! - [`document_from_vmf`]: VMF → document. World brushes and entities become
//!   objects; everything else in the file is kept as it was.
//! - [`vmf_from_document`]: document → VMF, putting everything kept back in its
//!   place. An untouched map is written back exactly as it was read.
//! - [`open_map`] and [`save_map`]: the same with files, with size limits
//!   on reading, and safe saving (write a temporary file, keep the old one
//!   as a `.vmx` backup as Hammer does, then swap).
//!
//! The `.halberd` project format comes later.

mod error;
mod export;
mod files;
mod import;
mod planes;

pub use error::IoError;
pub use export::vmf_from_document;
pub use files::{
    BACKUP_EXTENSION, MAX_MAP_FILE_BYTES, OpenedMap, open_map, open_map_text, save_map,
};
pub use import::{Imported, document_from_vmf};
