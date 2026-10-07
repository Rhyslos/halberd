//! The Halberd program: starts the editor and wires the crates together.
//!
//! Startup loads settings and finds Garry's Mod (see `startup`), then opens
//! the editor window (see `window`). With `--report-only` it prints what it
//! found and exits instead, which is useful for troubleshooting and tests.

mod cli;
mod startup;
mod window;

use halberd_ui::AppInfo;
use std::io::{BufRead, IsTerminal, Write};
use std::process::ExitCode;

/// The full product name shown in the title bar and About box.
const PRODUCT_NAME: &str = "Halberd Map Editor";

/// Builds the one-line version banner, e.g. "Halberd Map Editor 0.0.1".
fn version_banner() -> String {
    format!("{PRODUCT_NAME} {}", env!("CARGO_PKG_VERSION"))
}

/// Builds a text message: the banner, then the given lines.
///
/// When `wait_for_enter` is true, the message ends with a prompt, because the
/// program was most likely started by double-clicking and its terminal
/// window would otherwise close before the text can be read.
fn compose_message(body: &[String], wait_for_enter: bool) -> String {
    let mut message = format!("{}\n\n", version_banner());
    for line in body {
        message.push_str(line);
        message.push('\n');
    }
    if wait_for_enter {
        message.push_str("\nPress Enter to close this window.\n");
    }
    message
}

/// Prints a message to the terminal. Waits for Enter first if asked.
fn print_message(body: &[String], wait_for_enter: bool) -> bool {
    let mut out = std::io::stdout().lock();
    let written = out
        .write_all(compose_message(body, wait_for_enter).as_bytes())
        .is_ok()
        && out.flush().is_ok();
    drop(out);
    if wait_for_enter {
        let mut line = String::new();
        // Any outcome (Enter, closed input, error) simply continues.
        let _ = std::io::stdin().lock().read_line(&mut line);
    }
    written
}

/// Prints a message and returns `code`, or failure if printing failed.
fn finish(body: &[String], interactive: bool, code: ExitCode) -> ExitCode {
    if print_message(body, interactive) {
        code
    } else {
        ExitCode::FAILURE
    }
}

fn main() -> ExitCode {
    // An interactive input means a person is watching: keep text on screen
    // until they press Enter. Scripts and pipes are not kept waiting.
    let interactive = std::io::stdin().is_terminal();

    let options = match cli::parse(std::env::args_os().skip(1)) {
        Ok(options) => options,
        Err(problem) => {
            let body = [format!("Problem: {problem}"), cli::HELP.to_string()];
            return finish(&body, interactive, ExitCode::FAILURE);
        }
    };
    if options.help {
        return finish(&[cli::HELP.to_string()], interactive, ExitCode::SUCCESS);
    }

    let report = startup::run(&options);
    if options.report_only {
        return finish(&report, interactive, ExitCode::SUCCESS);
    }

    // Also show the report in the terminal behind the editor window, where
    // it stays readable even if the window fails to open.
    print_message(&report, false);

    let info = AppInfo {
        name: PRODUCT_NAME.to_string(),
        version: env!("CARGO_PKG_VERSION").into(),
    };
    match window::run(info, report) {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            let body = [
                format!("The editor window could not be opened: {reason}"),
                "Halberd needs a graphics driver with Vulkan, DirectX 12 or Metal support. \
                 Updating your graphics driver usually fixes this."
                    .to_string(),
            ];
            finish(&body, interactive, ExitCode::FAILURE)
        }
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
