//! The interface panels.
//!
//! The egui panels: Browsers, Scene, Properties, Console and Settings, docked around the viewports.
//!
//! Status: skeleton. See this crate's README for what it must never do.

/// The name of this crate, used in logs and diagnostics.
pub const CRATE_NAME: &str = "halberd-ui";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
