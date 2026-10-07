//! Run the map compilers and report their progress.
//!
//! Compile profiles (geometry only, quick test, final), running vbsp, vvis and vrad as separate processes, incremental change detection, and parsing their logs for the console.
//!
//! Status: skeleton. See this crate's README for what it must never do.

/// The name of this crate, used in logs and diagnostics.
pub const CRATE_NAME: &str = "halberd-compile";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
