//! Parse and write VMF map files losslessly.
//!
//! VMF is the text map format Hammer saves. This crate reads a VMF into plain data (solids, sides, entities, groups, visgroups) and writes it back without losing anything, which is what makes Export to Hammer and import possible.
//!
//! Status: skeleton. See this crate's README for what it must never do.

/// The name of this crate, used in logs and diagnostics.
pub const CRATE_NAME: &str = "halberd-vmf";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
