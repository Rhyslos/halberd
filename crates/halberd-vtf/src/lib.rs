//! Read VTF textures and VMT materials.
//!
//! Materials are VMT text files pointing at VTF textures. [`Vtf::parse`]
//! reads a texture's header and finds its images; [`Vtf::decode`] turns one
//! of its sizes into plain RGBA pixels. [`Material::parse`] reads a VMT's
//! shader and parameters, such as its base texture.
//!
//! Everything here treats its input as hostile: sizes are checked before
//! anything is allocated, and bad files give plain-language errors.

mod decode;
mod error;
mod vmt;
mod vtf;

pub use error::VtfError;
pub use vmt::Material;
pub use vtf::{ImageFormat, MAX_DIMENSION, RgbaImage, Vtf};

/// The name of this crate, used in logs and diagnostics.
pub const CRATE_NAME: &str = "halberd-vtf";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
