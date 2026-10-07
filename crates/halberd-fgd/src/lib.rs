//! Read FGD entity definition files.
//!
//! FGD files list every entity a game supports, with its keyvalues, inputs and outputs. This crate parses them so the editor can offer GMod's real entity list.
//!
//! Status: skeleton. See this crate's README for what it must never do.

/// The name of this crate, used in logs and diagnostics.
pub const CRATE_NAME: &str = "halberd-fgd";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
