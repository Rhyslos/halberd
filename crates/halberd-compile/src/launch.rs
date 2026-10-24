//! Starting Garry's Mod on a map.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Garry's Mod's Steam app number.
pub const GMOD_APP_ID: &str = "4000";

/// How to start the game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launcher {
    /// Through Steam (its program), which starts Garry's Mod.
    Steam(PathBuf),
    /// Garry's Mod's own program.
    Game(PathBuf),
}

impl Launcher {
    /// The best way to start the game: Steam's program in `steam_dir`, if
    /// found; else the game's own program in its install folder `root`.
    pub fn find(steam_dir: Option<&Path>, root: &Path) -> Option<Self> {
        let steam = steam_dir.and_then(|dir| {
            ["steam.exe", "steam.sh", "steam"]
                .iter()
                .map(|name| dir.join(name))
                .find(|p| p.is_file())
        });
        if let Some(steam) = steam {
            return Some(Self::Steam(steam));
        }
        ["gmod.exe", "hl2.exe", "hl2.sh", "hl2_osx"]
            .iter()
            .map(|name| root.join(name))
            .find(|p| p.is_file())
            .map(Self::Game)
    }

    /// The program and arguments that load `map`.
    pub fn command(&self, map: &str) -> (PathBuf, Vec<String>) {
        match self {
            Self::Steam(program) => (
                program.clone(),
                vec![
                    "-applaunch".into(),
                    GMOD_APP_ID.into(),
                    "+map".into(),
                    map.into(),
                ],
            ),
            Self::Game(program) => (
                program.clone(),
                vec![
                    "-steam".into(),
                    "-game".into(),
                    "garrysmod".into(),
                    "+map".into(),
                    map.into(),
                ],
            ),
        }
    }

    /// Starts the game on `map` (it runs on its own; Halberd doesn't wait).
    pub fn launch(&self, map: &str) -> Result<(), String> {
        let (program, args) = self.command(map);
        let mut command = Command::new(&program);
        command
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(dir) = program.parent() {
            command.current_dir(dir);
        }
        command
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("{} could not be started: {e}", program.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_is_preferred_and_loads_the_map() {
        let dir = std::env::temp_dir().join(format!("halberd-launch-{}", std::process::id()));
        let steam = dir.join("steam");
        let game = dir.join("game");
        std::fs::create_dir_all(&steam).unwrap();
        std::fs::create_dir_all(&game).unwrap();
        std::fs::write(game.join("gmod.exe"), b"").unwrap();
        let only_game = Launcher::find(Some(&steam), &game).unwrap();
        assert_eq!(only_game, Launcher::Game(game.join("gmod.exe")));
        let (_, args) = only_game.command("my_map");
        assert_eq!(args, ["-steam", "-game", "garrysmod", "+map", "my_map"]);
        std::fs::write(steam.join("steam.exe"), b"").unwrap();
        let via_steam = Launcher::find(Some(&steam), &game).unwrap();
        assert_eq!(via_steam, Launcher::Steam(steam.join("steam.exe")));
        assert_eq!(
            via_steam.command("m").1,
            ["-applaunch", "4000", "+map", "m"]
        );
        assert_eq!(Launcher::find(None, &dir), None);
        std::fs::remove_dir_all(&dir).ok();
    }
}
