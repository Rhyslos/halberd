//! Read Source models (MDL, VVD, VTX) into plain meshes.
//!
//! Props are Source models split over MDL, VVD and VTX files. This crate decodes them into plain vertex and index lists that the renderer can upload.
//!
//! Status: skeleton. See this crate's README for what it must never do.

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
