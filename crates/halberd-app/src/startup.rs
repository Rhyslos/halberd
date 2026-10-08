//! What happens when Halberd starts: load settings, find Garry's Mod,
//! remember what was found, and report it in plain words.

use crate::cli::Options;
use halberd_assets::{
    CompileTool, Detection, FoundBy, GmodInstall, detect_gmod, detect_gmod_in, inspect_gmod_dir,
};
use halberd_config::{LoadStatus, Settings, SettingsStore};

/// What startup produced.
pub(crate) struct Startup {
    /// Plain-language report lines.
    pub(crate) report: Vec<String>,
    /// The settings in effect (loaded, corrected and possibly updated).
    pub(crate) settings: Settings,
}

/// Runs the startup steps.
pub(crate) fn run(options: &Options) -> Startup {
    let mut report = Vec::new();

    let store = match &options.settings_file {
        Some(path) => Some(SettingsStore::new(path)),
        None => match SettingsStore::standard() {
            Ok(store) => Some(store),
            Err(err) => {
                report.push(format!(
                    "Settings: {err}. Using defaults; nothing will be saved."
                ));
                None
            }
        },
    };

    let (mut settings, mut must_save) = match &store {
        Some(store) => {
            let loaded = store.load();
            report.push(format!(
                "Settings: {} ({})",
                store.path().display(),
                describe_status(&loaded.status)
            ));
            report.extend(loaded.notes.iter().map(|n| format!("  Note: {n}")));
            let first_launch = loaded.status == LoadStatus::NotFound;
            (loaded.settings, first_launch)
        }
        None => (Settings::default(), false),
    };

    let install = find_gmod(options, &settings, &mut report);
    if let Some(install) = &install {
        report.extend(describe_install(install));
        if settings.game.gmod_dir.as_deref() != Some(install.root.as_path()) {
            settings.game.gmod_dir = Some(install.root.clone());
            must_save = true;
        }
    }

    if must_save && let Some(store) = &store {
        match store.save(&settings) {
            Ok(()) => report.push("Settings saved.".to_string()),
            Err(err) => report.push(format!("Settings were not saved: {err}.")),
        }
    }
    Startup { report, settings }
}

/// Finds GMod: a folder given on the command line first, then the saved
/// folder, then Steam.
fn find_gmod(
    options: &Options,
    settings: &Settings,
    report: &mut Vec<String>,
) -> Option<GmodInstall> {
    if let Some(chosen) = &options.gmod_dir {
        match inspect_gmod_dir(chosen, FoundBy::UserChoice) {
            Ok(install) => return Some(install),
            Err(problem) => report.push(format!(
                "Garry's Mod: the chosen folder can't be used: {problem}. Trying the saved folder and Steam instead."
            )),
        }
    }

    let saved = settings.game.gmod_dir.as_deref();
    let result = match &options.steam_dir {
        Some(steam_dir) => detect_gmod_in(saved, std::slice::from_ref(steam_dir)),
        None => detect_gmod(saved),
    };
    match result {
        Ok(Detection { install, notes }) => {
            report.extend(notes.iter().map(|n| format!("  Note: {n}")));
            Some(install)
        }
        Err(err) => {
            report.push(format!("Garry's Mod: not found. {err}"));
            report.push(
                "  Tip: start Halberd with --gmod-dir \"<your GarrysMod folder>\" to set it."
                    .to_string(),
            );
            None
        }
    }
}

fn describe_status(status: &LoadStatus) -> String {
    match status {
        LoadStatus::Loaded => "loaded".to_string(),
        LoadStatus::NotFound => "first launch, using defaults".to_string(),
        LoadStatus::Recovered { .. } => "damaged file set aside, using defaults".to_string(),
        LoadStatus::Unreadable => "could not be read, using defaults".to_string(),
        LoadStatus::NewerFormat { found } => format!("from a newer Halberd, format {found}"),
    }
}

/// Plain-language lines describing a found install.
pub(crate) fn describe_install(install: &GmodInstall) -> Vec<String> {
    let how = match &install.found_by {
        FoundBy::Settings => "from saved settings".to_string(),
        FoundBy::Steam { steam_dir } => format!("through Steam at {}", steam_dir.display()),
        FoundBy::UserChoice => "from the folder you chose".to_string(),
    };
    let mut lines = vec![
        format!("Garry's Mod: found {how}"),
        format!("  Folder:        {}", install.root.display()),
        match &install.workshop_dir {
            Some(dir) => format!("  Workshop:      {}", dir.display()),
            None => "  Workshop:      no subscribed content folder found yet".to_string(),
        },
    ];
    let tools: Vec<String> = CompileTool::ALL
        .iter()
        .map(|t| {
            let state = if install.tools.get(*t).is_some() {
                "found"
            } else {
                "MISSING"
            };
            format!("{} {state}", t.name())
        })
        .collect();
    lines.push(format!("  Compile tools: {}", tools.join(", ")));
    if !install.tools.missing().is_empty() {
        lines.push(
            "  Note: maps can't be compiled until every tool is present. On Windows, try Steam's \
             'Verify integrity of game files'. GMod for Linux and macOS does not include them."
                .to_string(),
        );
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use halberd_assets::CompileTools;
    use std::path::PathBuf;

    fn install(tools: CompileTools, workshop: Option<&str>) -> GmodInstall {
        GmodInstall {
            root: PathBuf::from("/games/GarrysMod"),
            game_dir: PathBuf::from("/games/GarrysMod/garrysmod"),
            tools,
            workshop_dir: workshop.map(PathBuf::from),
            found_by: FoundBy::Steam {
                steam_dir: PathBuf::from("/games/Steam"),
            },
        }
    }

    #[test]
    fn complete_install_reads_cleanly() {
        let all = CompileTools {
            vbsp: Some("a".into()),
            vvis: Some("b".into()),
            vrad: Some("c".into()),
            bspzip: Some("d".into()),
        };
        let lines = describe_install(&install(all, Some("/w/4000")));
        assert!(lines[0].starts_with("Garry's Mod: found through Steam"));
        assert!(
            lines
                .iter()
                .any(|l| l.contains("vbsp found, vvis found, vrad found, bspzip found"))
        );
        assert!(lines.iter().any(|l| l.contains("/w/4000")));
        assert!(!lines.iter().any(|l| l.contains("Note")));
    }

    #[test]
    fn missing_tools_get_a_note() {
        let lines = describe_install(&install(CompileTools::default(), None));
        assert!(lines.iter().any(|l| l.contains("vbsp MISSING")));
        assert!(lines.iter().any(|l| l.contains("Verify integrity")));
        assert!(
            lines
                .iter()
                .any(|l| l.contains("no subscribed content folder"))
        );
    }
}
