//! Compile and play: writes the map as it is now to a work folder,
//! compiles it with Garry's Mod's tools in the background
//! (`halberd-compile`), reports progress to the workbench, and starts the
//! game on the map when it's done. A leak check runs the same way with
//! only the first compiler, and installs nothing.

use eframe::egui;
use halberd_assets::{FoundBy, GmodInstall};
use halberd_compile::{CompileEvent, CompileJob, CompileRunner, Launcher, LineKind, Profile};
use halberd_ui::{CompileLine, Workbench};
use std::path::{Path, PathBuf};

/// A compile and what to do when it finishes.
pub(crate) struct CompileSession {
    runner: CompileRunner,
    launcher: Option<Launcher>,
}

/// The map's name for compiling: its file name, made safe for the game's
/// `map` command (letters, digits and underscores, lower case).
fn compile_name(file: Option<&Path>) -> String {
    let stem = file
        .and_then(Path::file_stem)
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "untitled".into());
    let safe: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if safe.trim_matches('_').is_empty() {
        "untitled".into()
    } else {
        safe
    }
}

/// Folder (inside Garry's Mod's game folder) holding each Halberd's
/// compile work.
const WORK_FOLDER: &str = "halberd_compile";
/// Work folders left by earlier sessions are removed after this long.
const OLD_WORK: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 3600);

/// Where this Halberd writes maps for compiling: a folder of its own
/// inside Garry's Mod's game folder (the user's own file is never touched;
/// two Halberds never share files; and the compilers, which can't read
/// every path, get a plain one).
fn work_dir(game_dir: &Path) -> PathBuf {
    game_dir
        .join(WORK_FOLDER)
        .join(std::process::id().to_string())
}

/// Removes other sessions' work folders that are a week old.
fn tidy_old_work(game_dir: &Path, ours: &Path) {
    let Ok(entries) = std::fs::read_dir(game_dir.join(WORK_FOLDER)) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > OLD_WORK);
        if old && path != ours && path.is_dir() {
            std::fs::remove_dir_all(&path).ok();
        }
    }
}

/// Starts compiling the workbench's map (`Profile::Quick`) or checking it
/// for leaks (`Profile::LeakCheck`), or says why it can't.
pub(crate) fn start(
    workbench: &mut Workbench,
    install: Option<&GmodInstall>,
    profile: Profile,
    ctx: &egui::Context,
) -> Option<CompileSession> {
    let name = compile_name(workbench.file_path());
    match profile {
        Profile::LeakCheck => workbench.leak_check_started(&name),
        _ => workbench.compile_started(&name),
    }
    let Some(install) = install else {
        workbench.compile_finished(Err(
            "Garry's Mod wasn't found, so its compile tools can't be used. Start Halberd \
             with --gmod-dir to point at it."
                .into(),
        ));
        return None;
    };
    let dir = work_dir(&install.game_dir);
    let vmf = dir.join(format!("{name}.vmf"));
    let job = CompileJob {
        vmf: vmf.clone(),
        root: install.root.clone(),
        game_dir: install.game_dir.clone(),
        vbsp: install.tools.vbsp.clone(),
        vvis: install.tools.vvis.clone(),
        vrad: install.tools.vrad.clone(),
        profile,
    };
    // Check the tools before writing anything.
    if let Err(why) = job.steps() {
        workbench.compile_finished(Err(why.to_string()));
        return None;
    }
    tidy_old_work(&install.game_dir, &dir);
    let text = halberd_io::vmf_from_document(workbench.document()).to_text();
    let written = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&vmf, text));
    if let Err(e) = written {
        workbench.compile_finished(Err(format!(
            "the map could not be written for compiling ({}): {e}",
            vmf.display()
        )));
        return None;
    }
    let steam_dir = match &install.found_by {
        FoundBy::Steam { steam_dir } => Some(steam_dir.as_path()),
        _ => None,
    };
    let launcher = Launcher::find(steam_dir, &install.root);
    let wake = ctx.clone();
    Some(CompileSession {
        runner: CompileRunner::start(job, move || wake.request_repaint()),
        launcher,
    })
}

impl CompileSession {
    /// Asks the compile to stop.
    pub(crate) fn cancel(&self) {
        self.runner.cancel();
    }

    /// Passes on what the compile reported. Returns false once it is over.
    pub(crate) fn poll(&mut self, workbench: &mut Workbench) -> bool {
        for event in self.runner.poll() {
            match event {
                CompileEvent::Step {
                    index,
                    total,
                    tool,
                    label,
                } => {
                    workbench.compile_step(index, total, &format!("{label} ({tool})"));
                }
                CompileEvent::Line(text, kind) => {
                    let kind = match kind {
                        LineKind::Info => CompileLine::Info,
                        LineKind::Warning => CompileLine::Warning,
                        LineKind::Error => CompileLine::Error,
                    };
                    workbench.compile_line(&text, kind);
                }
                CompileEvent::LeakPath(points) => workbench.set_leak_path(&points),
                CompileEvent::Finished(Ok(compiled)) if compiled.installed.is_none() => {
                    workbench.compile_finished(Ok("No leaks found: the map is sealed.".into()));
                }
                CompileEvent::Finished(Ok(compiled)) => {
                    let note = match &self.launcher {
                        Some(launcher) => match launcher.launch(&compiled.map_name) {
                            Ok(()) => format!(
                                "Starting Garry's Mod on {0}… (If it was already open, type \
                                 \"map {0}\" in its console.)",
                                compiled.map_name
                            ),
                            Err(why) => format!(
                                "The map is in Garry's Mod's maps folder, but the game could \
                                 not be started ({why}); type \"map {}\" in its console.",
                                compiled.map_name
                            ),
                        },
                        None => format!(
                            "The map is in Garry's Mod's maps folder; start the game and type \
                             \"map {}\" in its console.",
                            compiled.map_name
                        ),
                    };
                    workbench.compile_finished(Ok(note));
                }
                CompileEvent::Finished(Err(why)) => {
                    workbench.compile_finished(Err(why.to_string()))
                }
            }
        }
        !self.runner.is_finished()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_names_are_safe_for_the_games_map_command() {
        assert_eq!(
            compile_name(Some(Path::new("/maps/My Map (v2).vmf"))),
            "my_map__v2_"
        );
        assert_eq!(
            compile_name(Some(Path::new("gm_construct.vmf"))),
            "gm_construct"
        );
        assert_eq!(compile_name(None), "untitled");
        assert_eq!(compile_name(Some(Path::new("ÄÖ.vmf"))), "untitled");
    }
}
