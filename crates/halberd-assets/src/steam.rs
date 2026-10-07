//! Finding Garry's Mod: first from settings, then through Steam.

use crate::gmod::{FoundBy, GMOD_APP_ID, GmodInstall, InvalidGmodDir, inspect_gmod_dir};
use std::fmt;
use std::path::{Path, PathBuf};

/// A GMod install that was found, plus notes worth showing the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    /// The install that was found and checked.
    pub install: GmodInstall,
    /// Plain-language notes, such as "the saved folder no longer exists,
    /// so Steam was asked instead". Empty when nothing unusual happened.
    pub notes: Vec<String>,
}

/// Why no GMod install could be found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectionError {
    /// No Steam installation was found on this computer.
    SteamNotFound,
    /// Steam was found, but none of its libraries has GMod installed.
    NotInstalled {
        /// The Steam installations that were searched.
        searched: Vec<PathBuf>,
    },
    /// Steam lists GMod, but its folder is not a valid install.
    Broken(InvalidGmodDir),
}

impl fmt::Display for DetectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SteamNotFound => write!(
                f,
                "Steam was not found on this computer. Choose the Garry's Mod folder manually."
            ),
            Self::NotInstalled { searched } => {
                write!(f, "Garry's Mod is not installed in any Steam library")?;
                if !searched.is_empty() {
                    let list: Vec<String> =
                        searched.iter().map(|p| p.display().to_string()).collect();
                    write!(f, " (searched {})", list.join(", "))?;
                }
                write!(
                    f,
                    ". Install it through Steam, or choose its folder manually."
                )
            }
            Self::Broken(problem) => write!(f, "Steam lists Garry's Mod, but {problem}"),
        }
    }
}

impl std::error::Error for DetectionError {}

/// Finds Garry's Mod on this computer.
///
/// 1. If `saved` is given and is a valid install, it is used.
/// 2. Otherwise every Steam installation found on the system is searched.
///    If `saved` was invalid, a note explains why Steam was asked instead.
pub fn detect_gmod(saved: Option<&Path>) -> Result<Detection, DetectionError> {
    let steam_dirs = || {
        steamlocate::locate_all().map(|all| {
            all.iter()
                .map(|s| s.path().to_path_buf())
                .collect::<Vec<_>>()
        })
    };
    detect_with(saved, || steam_dirs().unwrap_or_default())
}

/// Like [`detect_gmod`], but searches only the given Steam folders instead of
/// asking the system where Steam is. Used by tests and by a future
/// "choose Steam folder" option.
pub fn detect_gmod_in(
    saved: Option<&Path>,
    steam_dirs: &[PathBuf],
) -> Result<Detection, DetectionError> {
    detect_with(saved, || steam_dirs.to_vec())
}

fn detect_with(
    saved: Option<&Path>,
    steam_dirs: impl FnOnce() -> Vec<PathBuf>,
) -> Result<Detection, DetectionError> {
    let mut notes = Vec::new();
    if let Some(path) = saved {
        match inspect_gmod_dir(path, FoundBy::Settings) {
            Ok(install) => return Ok(Detection { install, notes }),
            Err(problem) => notes.push(format!(
                "The saved Garry's Mod folder is no longer usable ({problem}); asking Steam instead."
            )),
        }
    }

    let dirs = steam_dirs();
    if dirs.is_empty() {
        return Err(DetectionError::SteamNotFound);
    }
    let mut broken = None;
    for steam_dir in &dirs {
        match gmod_in_steam_dir(steam_dir) {
            Some(Ok(install)) => return Ok(Detection { install, notes }),
            Some(Err(problem)) => broken = broken.or(Some(problem)),
            None => {}
        }
    }
    Err(match broken {
        Some(problem) => DetectionError::Broken(problem),
        None => DetectionError::NotInstalled { searched: dirs },
    })
}

/// Looks for GMod in one Steam installation's libraries.
/// `None` means GMod is not listed there at all.
fn gmod_in_steam_dir(steam_dir: &Path) -> Option<Result<GmodInstall, InvalidGmodDir>> {
    let steam = steamlocate::SteamDir::from_dir(steam_dir).ok()?;
    let (app, library) = steam.find_app(GMOD_APP_ID).ok()??;
    let folder = library.resolve_app_dir(&app);
    Some(inspect_gmod_dir(
        &folder,
        FoundBy::Steam {
            steam_dir: steam_dir.to_path_buf(),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gmod::test_support::fake_gmod;
    use std::fs;

    /// Builds a fake Steam installation whose libraries are `libraries`
    /// (the Steam folder itself is always library 0).
    fn fake_steam(steam: &Path, libraries: &[&Path]) {
        let steamapps = steam.join("steamapps");
        fs::create_dir_all(&steamapps).unwrap();
        let mut vdf = String::from("\"libraryfolders\"\n{\n");
        let all: Vec<&Path> = std::iter::once(steam)
            .chain(libraries.iter().copied())
            .collect();
        for (i, lib) in all.iter().enumerate() {
            fs::create_dir_all(lib.join("steamapps")).unwrap();
            let path = lib.display().to_string().replace('\\', "\\\\");
            vdf.push_str(&format!(
                "\t\"{i}\"\n\t{{\n\t\t\"path\"\t\t\"{path}\"\n\t\t\"apps\"\n\t\t{{\n\t\t}}\n\t}}\n"
            ));
        }
        vdf.push_str("}\n");
        fs::write(steamapps.join("libraryfolders.vdf"), vdf).unwrap();
    }

    /// Registers GMod in `library` with Steam's app manifest, installed in
    /// `steamapps/common/GarrysMod`, and returns that folder.
    fn install_gmod_in(library: &Path, tools: &[&str]) -> PathBuf {
        let manifest = "\"AppState\"\n{\n\t\"appid\"\t\t\"4000\"\n\t\"name\"\t\t\"Garry's Mod\"\n\
                        \t\"installdir\"\t\t\"GarrysMod\"\n}\n";
        fs::write(
            library.join("steamapps").join("appmanifest_4000.acf"),
            manifest,
        )
        .unwrap();
        fake_gmod(
            &library.join("steamapps").join("common").join("GarrysMod"),
            tools,
        )
    }

    #[test]
    fn saved_folder_is_used_when_valid() {
        let dir = tempfile::tempdir().unwrap();
        let root = fake_gmod(&dir.path().join("GarrysMod"), &["vbsp"]);
        let found = detect_gmod_in(Some(&root), &[]).unwrap();
        assert_eq!(found.install.root, root);
        assert_eq!(found.install.found_by, FoundBy::Settings);
        assert!(found.notes.is_empty());
    }

    #[test]
    fn finds_gmod_in_the_main_steam_library() {
        let dir = tempfile::tempdir().unwrap();
        let steam = dir.path().join("Steam");
        fake_steam(&steam, &[]);
        let root = install_gmod_in(&steam, &["vbsp", "vvis", "vrad", "bspzip"]);

        let found = detect_gmod_in(None, std::slice::from_ref(&steam)).unwrap();
        assert_eq!(found.install.root, root);
        assert_eq!(found.install.found_by, FoundBy::Steam { steam_dir: steam });
        assert!(found.install.tools.missing().is_empty());
    }

    #[test]
    fn finds_gmod_in_a_second_library() {
        let dir = tempfile::tempdir().unwrap();
        let steam = dir.path().join("Steam");
        let library = dir.path().join("D_SteamLibrary");
        fake_steam(&steam, &[&library]);
        let root = install_gmod_in(&library, &[]);
        let workshop = library
            .join("steamapps")
            .join("workshop")
            .join("content")
            .join("4000");
        fs::create_dir_all(&workshop).unwrap();

        let found = detect_gmod_in(None, &[steam]).unwrap();
        assert_eq!(found.install.root, root);
        assert_eq!(found.install.workshop_dir, Some(workshop));
    }

    #[test]
    fn invalid_saved_folder_falls_back_to_steam_with_a_note() {
        let dir = tempfile::tempdir().unwrap();
        let steam = dir.path().join("Steam");
        fake_steam(&steam, &[]);
        let root = install_gmod_in(&steam, &[]);
        let moved_away = dir.path().join("OldGarrysMod");

        let found = detect_gmod_in(Some(&moved_away), &[steam]).unwrap();
        assert_eq!(found.install.root, root);
        assert_eq!(found.notes.len(), 1);
        assert!(found.notes[0].contains("asking Steam instead"));
    }

    #[test]
    fn no_steam_is_reported() {
        assert_eq!(
            detect_gmod_in(None, &[]),
            Err(DetectionError::SteamNotFound)
        );
    }

    #[test]
    fn steam_without_gmod_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let steam = dir.path().join("Steam");
        fake_steam(&steam, &[]);
        let err = detect_gmod_in(None, std::slice::from_ref(&steam)).unwrap_err();
        assert_eq!(
            err,
            DetectionError::NotInstalled {
                searched: vec![steam]
            }
        );
        assert!(err.to_string().contains("not installed"));
    }

    #[test]
    fn listed_but_broken_install_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let steam = dir.path().join("Steam");
        fake_steam(&steam, &[]);
        let root = install_gmod_in(&steam, &[]);
        fs::remove_file(root.join("garrysmod").join("gameinfo.txt")).unwrap();
        let err = detect_gmod_in(None, &[steam]).unwrap_err();
        assert!(matches!(
            err,
            DetectionError::Broken(InvalidGmodDir::NoGameInfo(_))
        ));
    }

    #[test]
    fn second_steam_install_is_searched_when_first_lacks_gmod() {
        let dir = tempfile::tempdir().unwrap();
        let empty_steam = dir.path().join("FlatpakSteam");
        let real_steam = dir.path().join("Steam");
        fake_steam(&empty_steam, &[]);
        fake_steam(&real_steam, &[]);
        let root = install_gmod_in(&real_steam, &[]);
        let found = detect_gmod_in(None, &[empty_steam, real_steam]).unwrap();
        assert_eq!(found.install.root, root);
    }

    #[test]
    fn garbage_steam_files_do_not_crash() {
        let dir = tempfile::tempdir().unwrap();
        let steam = dir.path().join("Steam");
        let steamapps = steam.join("steamapps");
        fs::create_dir_all(&steamapps).unwrap();
        fs::write(
            steamapps.join("libraryfolders.vdf"),
            [0xff, 0x00, b'{', b'"'],
        )
        .unwrap();
        fs::write(
            steamapps.join("appmanifest_4000.acf"),
            "\"AppState\" { broken",
        )
        .unwrap();
        assert!(detect_gmod_in(None, &[steam]).is_err());
    }

    #[test]
    fn a_folder_that_is_not_steam_is_harmless() {
        let dir = tempfile::tempdir().unwrap();
        let err = detect_gmod_in(None, &[dir.path().join("does-not-exist")]).unwrap_err();
        assert!(matches!(err, DetectionError::NotInstalled { .. }));
    }
}
