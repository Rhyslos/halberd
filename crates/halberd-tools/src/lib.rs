//! Interaction tools: select, gizmo, shape tools, radial menus.
//!
//! Turns mouse and keyboard input into commands: selection, the W/R/S/T gizmo, shape editing tools and radial menus.
//!
//! Status: skeleton. See this crate's README for what it must never do.

/// The name of this crate, used in logs and diagnostics.
pub const CRATE_NAME: &str = "halberd-tools";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
