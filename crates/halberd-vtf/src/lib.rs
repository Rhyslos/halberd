//! Read VTF textures and VMT materials.
//!
//! Materials are VMT text files pointing at VTF textures. This crate reads both into plain data: texture pixels or compressed blocks, and material parameters.
//!
//! Status: skeleton. See this crate's README for what it must never do.

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
