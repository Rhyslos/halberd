//! Import and export VMF, save and load Halberd projects.
//!
//! Converts between the document and files: VMF import and Export to Hammer, and the `.halberd` project format with versioned migrations.
//!
//! Status: skeleton. See this crate's README for what it must never do.

/// The name of this crate, used in logs and diagnostics.
pub const CRATE_NAME: &str = "halberd-io";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
