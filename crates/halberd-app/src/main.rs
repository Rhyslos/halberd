//! The Halberd program: starts the editor and wires the crates together.
//!
//! Status: early skeleton. It loads settings, finds Garry's Mod and reports
//! what it found; the window and frame loop arrive in the Phase 0
//! "Window and docking panels" milestone.

mod cli;
mod startup;

use std::io::{BufRead, IsTerminal, Write};
use std::process::ExitCode;

/// The full product name shown in the title bar and about screen.
const PRODUCT_NAME: &str = "Halberd Map Editor";

/// Builds the one-line version banner, e.g. "Halberd Map Editor 0.0.1".
fn version_banner() -> String {
    format!("{PRODUCT_NAME} {}", env!("CARGO_PKG_VERSION"))
}

/// Builds the full message to print.
///
/// When `wait_for_enter` is true, the message ends with a prompt, because the
/// program was most likely started by double-clicking and its window would
/// otherwise close before the text can be read.
fn compose_message(body: &[String], wait_for_enter: bool) -> String {
    let mut message = format!("{}\n\n", version_banner());
    for line in body {
        message.push_str(line);
        message.push('\n');
    }
    message.push_str("\nThe editor window is not built yet. See the roadmap in README.md.\n");
    if wait_for_enter {
        message.push_str("\nPress Enter to close this window.\n");
    }
    message
}

fn main() -> ExitCode {
    // An interactive input means a person is watching: keep the window open
    // until they press Enter. Scripts and pipes are not kept waiting.
    let interactive = std::io::stdin().is_terminal();

    let (body, code) = match cli::parse(std::env::args_os().skip(1)) {
        Ok(options) if options.help => (vec![cli::HELP.to_string()], ExitCode::SUCCESS),
        Ok(options) => (startup::run(&options), ExitCode::SUCCESS),
        Err(problem) => (
            vec![format!("Problem: {problem}"), cli::HELP.to_string()],
            ExitCode::FAILURE,
        ),
    };

    let mut out = std::io::stdout().lock();
    if out
        .write_all(compose_message(&body, interactive).as_bytes())
        .is_err()
        || out.flush().is_err()
    {
        return ExitCode::FAILURE;
    }
    drop(out);

    if interactive {
        let mut line = String::new();
        // Any outcome (Enter, closed input, error) simply ends the program.
        let _ = std::io::stdin().lock().read_line(&mut line);
    }
    code
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

    #[test]
    fn interactive_message_asks_for_enter() {
        let message = compose_message(&["line one".to_string()], true);
        assert!(message.starts_with(&version_banner()));
        assert!(message.contains("line one\n"));
        assert!(message.ends_with("Press Enter to close this window.\n"));
    }

    #[test]
    fn non_interactive_message_does_not_ask_for_enter() {
        let message = compose_message(&[], false);
        assert!(message.starts_with(&version_banner()));
        assert!(!message.contains("Press Enter"));
    }
}
