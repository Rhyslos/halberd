//! Checking that a folder is a Garry's Mod install, and what it contains.

use std::fmt;
use std::path::{Path, PathBuf};

/// Steam's ID for Garry's Mod.
pub const GMOD_APP_ID: u32 = 4000;

/// The game folder inside a GMod install.
const GAME_FOLDER: &str = "garrysmod";
/// The file that identifies a Source game folder.
const GAMEINFO: &str = "gameinfo.txt";

/// A Garry's Mod install that has been checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GmodInstall {
    /// The install folder, containing `garrysmod` and `bin`
    /// (for example `...\steamapps\common\GarrysMod`).
    pub root: PathBuf,
    /// The game folder: `root/garrysmod`.
    pub game_dir: PathBuf,
    /// Which compile tools were found.
    pub tools: CompileTools,
    /// Where Steam keeps subscribed Workshop content for GMod, if the
    /// folder exists.
    pub workshop_dir: Option<PathBuf>,
    /// How the install was found.
    pub found_by: FoundBy,
}

/// How a GMod install was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoundBy {
    /// From the folder saved in Halberd's settings.
    Settings,
    /// By asking Steam, in the given Steam folder.
    Steam {
        /// The Steam installation that has GMod in one of its libraries.
        steam_dir: PathBuf,
    },
    /// From a folder the user just chose.
    UserChoice,
}

/// The Valve tools Halberd runs to compile a map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileTool {
    /// Builds the map's geometry.
    Vbsp,
    /// Works out visibility.
    Vvis,
    /// Bakes lighting.
    Vrad,
    /// Packs custom content into the map.
    Bspzip,
}

impl CompileTool {
    /// All tools, in the order a compile uses them.
    pub const ALL: [CompileTool; 4] = [Self::Vbsp, Self::Vvis, Self::Vrad, Self::Bspzip];

    /// The tool's file name without extension, such as `vbsp`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Vbsp => "vbsp",
            Self::Vvis => "vvis",
            Self::Vrad => "vrad",
            Self::Bspzip => "bspzip",
        }
    }
}

/// Where each compile tool was found, if anywhere.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompileTools {
    /// Path to vbsp, if found.
    pub vbsp: Option<PathBuf>,
    /// Path to vvis, if found.
    pub vvis: Option<PathBuf>,
    /// Path to vrad, if found.
    pub vrad: Option<PathBuf>,
    /// Path to bspzip, if found.
    pub bspzip: Option<PathBuf>,
}

impl CompileTools {
    /// Looks for each tool in the install's `bin` folder. The 64-bit
    /// versions (`bin/win64`) are preferred over the 32-bit ones (`bin`).
    pub fn find_in(root: &Path) -> Self {
        let find = |tool: CompileTool| {
            let name = tool.name();
            [
                root.join("bin").join("win64").join(format!("{name}.exe")),
                root.join("bin").join(format!("{name}.exe")),
                root.join("bin").join("linux64").join(name),
                root.join("bin").join(name),
            ]
            .into_iter()
            .find(|candidate| candidate.is_file())
        };
        Self {
            vbsp: find(CompileTool::Vbsp),
            vvis: find(CompileTool::Vvis),
            vrad: find(CompileTool::Vrad),
            bspzip: find(CompileTool::Bspzip),
        }
    }

    /// Where `tool` was found, if anywhere.
    pub fn get(&self, tool: CompileTool) -> Option<&Path> {
        match tool {
            CompileTool::Vbsp => self.vbsp.as_deref(),
            CompileTool::Vvis => self.vvis.as_deref(),
            CompileTool::Vrad => self.vrad.as_deref(),
            CompileTool::Bspzip => self.bspzip.as_deref(),
        }
    }

    /// The tools that were not found.
    pub fn missing(&self) -> Vec<CompileTool> {
        CompileTool::ALL
            .into_iter()
            .filter(|t| self.get(*t).is_none())
            .collect()
    }
}

/// Why a folder is not a usable Garry's Mod install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidGmodDir {
    /// The folder does not exist (or is a file).
    NotAFolder(PathBuf),
    /// The folder has no `garrysmod` game folder inside.
    NoGameFolder(PathBuf),
    /// The `garrysmod` folder has no `gameinfo.txt`.
    NoGameInfo(PathBuf),
}

impl fmt::Display for InvalidGmodDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAFolder(p) => write!(f, "{} does not exist or is not a folder", p.display()),
            Self::NoGameFolder(p) => write!(
                f,
                "{} has no 'garrysmod' folder inside; choose the GarrysMod install folder",
                p.display()
            ),
            Self::NoGameInfo(p) => write!(
                f,
                "{} has a 'garrysmod' folder but no gameinfo.txt; the install may be damaged \
                 (try Steam's 'Verify integrity of game files')",
                p.display()
            ),
        }
    }
}

impl std::error::Error for InvalidGmodDir {}

/// Checks that `dir` is a Garry's Mod install and reports what it contains.
///
/// Accepts either the install folder (`GarrysMod`) or the game folder
/// inside it (`GarrysMod/garrysmod`), since people pick either one.
pub fn inspect_gmod_dir(dir: &Path, found_by: FoundBy) -> Result<GmodInstall, InvalidGmodDir> {
    if !dir.is_dir() {
        return Err(InvalidGmodDir::NotAFolder(dir.to_path_buf()));
    }
    let root = match dir.file_name() {
        Some(name) if name.eq_ignore_ascii_case(GAME_FOLDER) && dir.join(GAMEINFO).is_file() => {
            dir.parent().unwrap_or(dir).to_path_buf()
        }
        _ => dir.to_path_buf(),
    };
    let game_dir = root.join(GAME_FOLDER);
    if !game_dir.is_dir() {
        return Err(InvalidGmodDir::NoGameFolder(root));
    }
    if !game_dir.join(GAMEINFO).is_file() {
        return Err(InvalidGmodDir::NoGameInfo(root));
    }
    let tools = CompileTools::find_in(&root);
    let workshop_dir = workshop_dir_for(&root);
    Ok(GmodInstall {
        root,
        game_dir,
        tools,
        workshop_dir,
        found_by,
    })
}

/// For an install at `<library>/steamapps/common/GarrysMod`, Workshop content
/// lives in `<library>/steamapps/workshop/content/4000`. Returns it if it exists.
fn workshop_dir_for(root: &Path) -> Option<PathBuf> {
    let common = root.parent()?;
    let steamapps = common.parent()?;
    let named = |p: &Path, name: &str| p.file_name().is_some_and(|n| n.eq_ignore_ascii_case(name));
    if !named(common, "common") || !named(steamapps, "steamapps") {
        return None;
    }
    let dir = steamapps
        .join("workshop")
        .join("content")
        .join(GMOD_APP_ID.to_string());
    dir.is_dir().then_some(dir)
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::fs;
    use std::path::{Path, PathBuf};

    /// Builds a fake GMod install at `root` with the given compile tools
    /// under `bin/win64`.
    pub(crate) fn fake_gmod(root: &Path, tools: &[&str]) -> PathBuf {
        let game = root.join("garrysmod");
        fs::create_dir_all(&game).unwrap();
        fs::write(
            game.join("gameinfo.txt"),
            "\"GameInfo\" { game \"Garry's Mod\" }",
        )
        .unwrap();
        let bin = root.join("bin").join("win64");
        fs::create_dir_all(&bin).unwrap();
        for tool in tools {
            fs::write(bin.join(format!("{tool}.exe")), b"MZ").unwrap();
        }
        root.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::fake_gmod;
    use super::*;
    use std::fs;

    #[test]
    fn accepts_a_complete_install() {
        let dir = tempfile::tempdir().unwrap();
        let root = fake_gmod(
            &dir.path().join("GarrysMod"),
            &["vbsp", "vvis", "vrad", "bspzip"],
        );
        let install = inspect_gmod_dir(&root, FoundBy::Settings).unwrap();
        assert_eq!(install.root, root);
        assert_eq!(install.game_dir, root.join("garrysmod"));
        assert!(install.tools.missing().is_empty());
        assert_eq!(install.found_by, FoundBy::Settings);
    }

    #[test]
    fn accepts_the_inner_garrysmod_folder() {
        let dir = tempfile::tempdir().unwrap();
        let root = fake_gmod(&dir.path().join("GarrysMod"), &[]);
        let install = inspect_gmod_dir(&root.join("garrysmod"), FoundBy::UserChoice).unwrap();
        assert_eq!(install.root, root);
    }

    #[test]
    fn reports_missing_tools() {
        let dir = tempfile::tempdir().unwrap();
        let root = fake_gmod(&dir.path().join("GarrysMod"), &["vbsp", "vrad"]);
        let install = inspect_gmod_dir(&root, FoundBy::Settings).unwrap();
        assert_eq!(
            install.tools.missing(),
            [CompileTool::Vvis, CompileTool::Bspzip]
        );
        assert!(install.tools.get(CompileTool::Vbsp).is_some());
    }

    #[test]
    fn prefers_64_bit_tools() {
        let dir = tempfile::tempdir().unwrap();
        let root = fake_gmod(&dir.path().join("GarrysMod"), &["vbsp"]);
        fs::write(root.join("bin").join("vbsp.exe"), b"MZ").unwrap();
        let tools = CompileTools::find_in(&root);
        assert_eq!(
            tools.vbsp,
            Some(root.join("bin").join("win64").join("vbsp.exe"))
        );
    }

    #[test]
    fn falls_back_to_32_bit_tools() {
        let dir = tempfile::tempdir().unwrap();
        let root = fake_gmod(&dir.path().join("GarrysMod"), &[]);
        fs::write(root.join("bin").join("vvis.exe"), b"MZ").unwrap();
        assert_eq!(
            CompileTools::find_in(&root).vvis,
            Some(root.join("bin").join("vvis.exe"))
        );
    }

    #[test]
    fn a_folder_named_like_a_tool_is_not_a_tool() {
        let dir = tempfile::tempdir().unwrap();
        let root = fake_gmod(&dir.path().join("GarrysMod"), &[]);
        fs::create_dir_all(root.join("bin").join("win64").join("vbsp.exe")).unwrap();
        assert_eq!(CompileTools::find_in(&root).vbsp, None);
    }

    #[test]
    fn rejects_a_missing_folder() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        assert_eq!(
            inspect_gmod_dir(&missing, FoundBy::Settings),
            Err(InvalidGmodDir::NotAFolder(missing))
        );
    }

    #[test]
    fn rejects_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("hl2.exe");
        fs::write(&file, b"MZ").unwrap();
        assert!(matches!(
            inspect_gmod_dir(&file, FoundBy::Settings),
            Err(InvalidGmodDir::NotAFolder(_))
        ));
    }

    #[test]
    fn rejects_a_folder_without_garrysmod() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            inspect_gmod_dir(dir.path(), FoundBy::Settings),
            Err(InvalidGmodDir::NoGameFolder(_))
        ));
    }

    #[test]
    fn rejects_a_damaged_install() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("garrysmod")).unwrap();
        let err = inspect_gmod_dir(dir.path(), FoundBy::Settings).unwrap_err();
        assert!(matches!(err, InvalidGmodDir::NoGameInfo(_)));
        assert!(err.to_string().contains("Verify integrity"));
    }

    #[test]
    fn finds_the_workshop_folder_in_a_steam_library() {
        let dir = tempfile::tempdir().unwrap();
        let steamapps = dir.path().join("SteamLibrary").join("steamapps");
        let root = fake_gmod(&steamapps.join("common").join("GarrysMod"), &[]);
        let workshop = steamapps.join("workshop").join("content").join("4000");
        fs::create_dir_all(&workshop).unwrap();
        let install = inspect_gmod_dir(&root, FoundBy::Settings).unwrap();
        assert_eq!(install.workshop_dir, Some(workshop));
    }

    #[test]
    fn no_workshop_folder_outside_a_steam_library() {
        let dir = tempfile::tempdir().unwrap();
        let root = fake_gmod(&dir.path().join("GarrysMod"), &[]);
        assert_eq!(
            inspect_gmod_dir(&root, FoundBy::Settings)
                .unwrap()
                .workshop_dir,
            None
        );
    }

    #[test]
    fn tool_names_match_valve_files() {
        let names: Vec<_> = CompileTool::ALL.iter().map(|t| t.name()).collect();
        assert_eq!(names, ["vbsp", "vvis", "vrad", "bspzip"]);
    }
}
