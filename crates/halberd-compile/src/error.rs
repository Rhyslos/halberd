//! Why a compile could not start or did not finish.

use std::fmt;

/// Why a compile could not start or did not finish, in plain words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    /// A compile tool was not found (named).
    MissingTool(&'static str),
    /// A tool is outside Garry's Mod's folder, so it won't be run.
    ToolOutsideGame(&'static str),
    /// A tool could not be started (named, with the reason).
    CouldNotStart(&'static str, String),
    /// A tool stopped with an error (named, with its last error line).
    ToolFailed(&'static str, String),
    /// The map has a leak: a hole to the void outside it.
    Leaked,
    /// The compiled map could not be put in the game's maps folder.
    CouldNotCopy(String),
    /// A map of the same name, not made by Halberd, is already in the
    /// game's maps folder (named).
    WouldReplace(String),
    /// The user cancelled.
    Cancelled,
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTool(tool) => write!(
                f,
                "{tool} wasn't found in Garry's Mod's bin folder. In Steam, right-click \
                 Garry's Mod, choose Properties → Installed Files → Verify integrity. (GMod \
                 for Linux and macOS has no compile tools; they come with the Windows \
                 version.)"
            ),
            Self::ToolOutsideGame(tool) => write!(
                f,
                "{tool} is outside Garry's Mod's folder, so Halberd won't run it"
            ),
            Self::CouldNotStart(tool, why) => write!(f, "{tool} could not be started: {why}"),
            Self::ToolFailed(tool, why) if why.is_empty() => {
                write!(f, "{tool} stopped with an error")
            }
            Self::ToolFailed(tool, why) => write!(f, "{tool} stopped with an error: {why}"),
            Self::Leaked => f.write_str(
                "the map leaks: somewhere there is a gap to the void outside it, so the \
                 compiler can't tell inside from outside. Seal the map with brushes (a box \
                 around everything works) and compile again.",
            ),
            Self::CouldNotCopy(why) => {
                write!(
                    f,
                    "the compiled map could not be copied into Garry's Mod: {why}"
                )
            }
            Self::WouldReplace(name) => write!(
                f,
                "Garry's Mod already has a map called {name} that Halberd didn't make, and it \
                 won't be replaced. Save your map under another name and compile again."
            ),
            Self::Cancelled => f.write_str("the compile was cancelled"),
        }
    }
}

impl std::error::Error for CompileError {}
