//! Find, mount, index and cache game content.
//!
//! Finds the GMod install, mounts base, mounted-game and Workshop content, indexes what is available, and keeps decoded assets in caches that respect the memory budget.
//!
//! Status: skeleton. See this crate's README for what it must never do.

/// The name of this crate, used in logs and diagnostics.
pub const CRATE_NAME: &str = "halberd-assets";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
