//! The Halberd program: starts the editor and wires the crates together.
//!
//! Status: skeleton. It reports its version and exits; the window and
//! frame loop arrive in the Phase 0 "Window, docking panels" milestone.

use std::io::Write;
use std::process::ExitCode;

/// The full product name shown in the title bar and about screen.
const PRODUCT_NAME: &str = "Halberd Map Editor";

/// Builds the one-line version banner, e.g. "Halberd Map Editor 0.0.1".
fn version_banner() -> String {
    format!("{PRODUCT_NAME} {}", env!("CARGO_PKG_VERSION"))
}

fn main() -> ExitCode {
    let mut out = std::io::stdout().lock();
    let message = format!(
        "{}\nThe editor window is not built yet. See the roadmap in README.md.\n",
        version_banner()
    );
    match out.write_all(message.as_bytes()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_contains_name_and_version() {
        let banner = version_banner();
        assert!(banner.starts_with("Halberd Map Editor "));
        assert!(banner.ends_with(env!("CARGO_PKG_VERSION")));
    }
}
