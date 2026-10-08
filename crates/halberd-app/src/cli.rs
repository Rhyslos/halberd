//! Command-line options.

use std::ffi::OsString;
use std::path::PathBuf;

/// Options given when starting Halberd.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Options {
    /// Use this Garry's Mod folder and remember it.
    pub(crate) gmod_dir: Option<PathBuf>,
    /// Search this Steam folder instead of the system's.
    pub(crate) steam_dir: Option<PathBuf>,
    /// Use this settings file instead of the standard one.
    pub(crate) settings_file: Option<PathBuf>,
    /// Print the startup report and exit, without opening the window.
    pub(crate) report_only: bool,
    /// Show help and exit.
    pub(crate) help: bool,
}

/// The help text shown by `--help`.
pub(crate) const HELP: &str = "\
Usage: halberd [options]

Options:
  --gmod-dir <folder>       Use this Garry's Mod folder and remember it
  --steam-dir <folder>      Look for Garry's Mod in this Steam folder
  --settings <file>         Use this settings file instead of the standard one
  --report-only             Print what Halberd found and exit, without a window
  --help                    Show this help
";

/// Parses command-line arguments (without the program name).
/// Returns a plain-language message for anything it does not understand.
pub(crate) fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value_for = |flag: &str| {
            args.next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("{flag} needs a folder or file after it"))
        };
        match arg.to_str() {
            Some("--gmod-dir") => options.gmod_dir = Some(value_for("--gmod-dir")?),
            Some("--steam-dir") => options.steam_dir = Some(value_for("--steam-dir")?),
            Some("--settings") => options.settings_file = Some(value_for("--settings")?),
            Some("--report-only") => options.report_only = true,
            Some("--help" | "-h" | "/?") => options.help = true,
            _ => return Err(format!("unknown option: {}", arg.to_string_lossy())),
        }
    }
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn no_arguments_gives_defaults() {
        assert_eq!(parse(args(&[])), Ok(Options::default()));
    }

    #[test]
    fn all_options_are_read() {
        let parsed = parse(args(&[
            "--gmod-dir",
            "G",
            "--steam-dir",
            "S",
            "--settings",
            "F",
            "--report-only",
            "--help",
        ]))
        .unwrap();
        assert!(parsed.report_only);
        assert_eq!(parsed.gmod_dir, Some(PathBuf::from("G")));
        assert_eq!(parsed.steam_dir, Some(PathBuf::from("S")));
        assert_eq!(parsed.settings_file, Some(PathBuf::from("F")));
        assert!(parsed.help);
    }

    #[test]
    fn paths_with_spaces_are_kept_whole() {
        let parsed = parse(args(&["--gmod-dir", r"C:\Program Files (x86)\Steam"])).unwrap();
        assert_eq!(
            parsed.gmod_dir,
            Some(PathBuf::from(r"C:\Program Files (x86)\Steam"))
        );
    }

    #[test]
    fn missing_value_is_explained() {
        assert_eq!(
            parse(args(&["--gmod-dir"])),
            Err("--gmod-dir needs a folder or file after it".to_string())
        );
    }

    #[test]
    fn unknown_option_is_explained() {
        assert_eq!(
            parse(args(&["--fast"])),
            Err("unknown option: --fast".to_string())
        );
    }

    #[test]
    fn windows_help_spelling_works() {
        assert!(parse(args(&["/?"])).unwrap().help);
    }
}
