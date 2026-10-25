//! Read Source models (MDL, VVD, VTX) into plain meshes.
//!
//! A prop is three files: the `.mdl` (name, materials, skins, how meshes
//! are laid out), the `.vvd` (vertices) and the `.vtx` (`.dx90.vtx`,
//! triangles). [`Model::parse`] reads all three into [`Mesh`]es of plain
//! vertices and indices that the renderer can upload, with the materials
//! they use and the model's bounds.
//!
//! Everything here treats its input as hostile: counts and offsets are
//! checked before anything is read or allocated, and bad files give
//! plain-language errors ([`MdlError`]).

mod bytes;
mod error;
pub mod example;
mod mdl;
mod model;
mod vtx;
mod vvd;

pub use error::MdlError;
pub use model::{Mesh, Model, Vertex};

/// Most vertices one model may have (Source allows 65,536 per body part).
pub const MAX_VERTICES: usize = 1 << 20;

/// The name of this crate, used in logs and diagnostics.
pub const CRATE_NAME: &str = "halberd-mdl";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
