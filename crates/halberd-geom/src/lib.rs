//! Geometry: planes, convex shapes, booleans, convex splitting.
//!
//! All the maths of shapes: planes, convex brushes, shape operations (extrude, bevel, split, bridge), booleans, and splitting editable shapes into the convex brushes Source requires.
//!
//! Status: skeleton. See this crate's README for what it must never do.

/// The name of this crate, used in logs and diagnostics.
pub const CRATE_NAME: &str = "halberd-geom";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
