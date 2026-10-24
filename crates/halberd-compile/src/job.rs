//! What to compile and how: the map file, Garry's Mod's folders and tools,
//! and the steps of a compile profile.

use crate::CompileError;
use std::path::{Path, PathBuf};

/// How thoroughly to compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Profile {
    /// Quick test: full geometry, fast visibility and fast lighting.
    #[default]
    Quick,
}

/// One compile tool run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The tool's name (`vbsp`).
    pub tool: &'static str,
    /// What it does, for people ("Building geometry").
    pub label: &'static str,
    /// The program to run.
    pub program: PathBuf,
    /// Its arguments.
    pub args: Vec<String>,
}

/// A compile to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileJob {
    /// The map file to compile (a `.vmf`); the compiled `.bsp` is written
    /// next to it.
    pub vmf: PathBuf,
    /// Garry's Mod's install folder (tools must be inside it).
    pub root: PathBuf,
    /// The game folder (`root/garrysmod`); the map is copied into its
    /// `maps` folder.
    pub game_dir: PathBuf,
    /// vbsp, if found.
    pub vbsp: Option<PathBuf>,
    /// vvis, if found.
    pub vvis: Option<PathBuf>,
    /// vrad, if found.
    pub vrad: Option<PathBuf>,
    /// How thoroughly.
    pub profile: Profile,
}

impl CompileJob {
    /// The map's name: the file name without `.vmf`.
    pub fn map_name(&self) -> String {
        self.vmf
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "map".into())
    }

    /// The compiled map, next to the map file.
    pub fn bsp(&self) -> PathBuf {
        self.vmf.with_extension("bsp")
    }

    /// Where the compiled map goes for the game to load it.
    pub fn installed_bsp(&self) -> PathBuf {
        self.game_dir
            .join("maps")
            .join(format!("{}.bsp", self.map_name()))
    }

    /// The tools to run, in order, checked: each must exist and be inside
    /// Garry's Mod's folder.
    pub fn steps(&self) -> Result<Vec<Step>, CompileError> {
        // Tools run in the map's folder, so every path is made absolute.
        let game = absolute(&self.game_dir).to_string_lossy().into_owned();
        let map = absolute(&self.vmf.with_extension(""))
            .to_string_lossy()
            .into_owned();
        let step = |tool: &'static str,
                    label: &'static str,
                    path: &Option<PathBuf>,
                    options: &[&str]|
         -> Result<Step, CompileError> {
            let program = absolute(path.as_deref().ok_or(CompileError::MissingTool(tool))?);
            if !inside(&program, &self.root) {
                return Err(CompileError::ToolOutsideGame(tool));
            }
            let mut args: Vec<String> = options.iter().map(|o| o.to_string()).collect();
            args.extend(["-game".to_string(), game.clone(), map.clone()]);
            Ok(Step {
                tool,
                label,
                program,
                args,
            })
        };
        match self.profile {
            Profile::Quick => Ok(vec![
                step("vbsp", "Building geometry", &self.vbsp, &[])?,
                step(
                    "vvis",
                    "Working out visibility (fast)",
                    &self.vvis,
                    &["-fast"],
                )?,
                step("vrad", "Lighting (fast)", &self.vrad, &["-fast", "-ldr"])?,
            ]),
        }
    }
}

/// `path` made absolute (from the current folder), as far as possible.
fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// True if `path` is inside `root` (after following links, when they can
/// be followed).
fn inside(path: &Path, root: &Path) -> bool {
    let real = |p: &Path| p.canonicalize().unwrap_or_else(|_| absolute(p));
    real(path).starts_with(real(root))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(root: &Path) -> CompileJob {
        CompileJob {
            vmf: PathBuf::from("/maps/work/my_map.vmf"),
            root: root.to_path_buf(),
            game_dir: root.join("garrysmod"),
            vbsp: Some(root.join("bin/vbsp.exe")),
            vvis: Some(root.join("bin/vvis.exe")),
            vrad: Some(root.join("bin/vrad.exe")),
            profile: Profile::Quick,
        }
    }

    #[test]
    fn a_quick_compile_runs_the_three_tools_with_fast_options() {
        let root = Path::new("/games/GarrysMod");
        let j = job(root);
        assert_eq!(j.map_name(), "my_map");
        assert_eq!(j.installed_bsp(), root.join("garrysmod/maps/my_map.bsp"));
        let steps = j.steps().unwrap();
        let tools: Vec<&str> = steps.iter().map(|s| s.tool).collect();
        assert_eq!(tools, ["vbsp", "vvis", "vrad"]);
        let game = root.join("garrysmod").to_string_lossy().into_owned();
        if cfg!(unix) {
            assert_eq!(steps[0].args, ["-game", game.as_str(), "/maps/work/my_map"]);
        }
        assert_eq!(steps[1].args[0], "-fast");
        assert_eq!(&steps[2].args[..2], ["-fast", "-ldr"]);
    }

    #[test]
    fn missing_tools_and_tools_from_elsewhere_are_refused() {
        let root = Path::new("/games/GarrysMod");
        let mut j = job(root);
        j.vrad = None;
        assert_eq!(j.steps(), Err(CompileError::MissingTool("vrad")));
        let mut j = job(root);
        j.vbsp = Some(PathBuf::from("/somewhere/else/vbsp.exe"));
        assert_eq!(j.steps(), Err(CompileError::ToolOutsideGame("vbsp")));
        let msg = CompileError::MissingTool("vrad").to_string();
        assert!(msg.contains("Verify integrity"), "{msg}");
    }
}
