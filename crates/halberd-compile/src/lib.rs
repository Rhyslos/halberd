//! Run the map compilers and report their progress.
//!
//! A [`CompileJob`] names the map file, Garry's Mod's folders and its
//! compile tools; [`CompileJob::steps`] checks them and lists the tools to
//! run for its [`Profile`]. [`CompileRunner`] runs them in the background,
//! one process at a time (never through a shell), passing their output on
//! as [`CompileEvent`]s, and copies the compiled map into the game's
//! `maps` folder. [`Launcher`] then starts Garry's Mod on it.

mod error;
mod job;
mod launch;
mod log;
mod run;

pub use error::CompileError;
pub use job::{CompileJob, Profile, Step};
pub use launch::{GMOD_APP_ID, Launcher};
pub use log::{LineKind, classify, is_leak};
pub use run::{CompileEvent, CompileRunner, Compiled};

/// The name of this crate, used in logs and diagnostics.
pub const CRATE_NAME: &str = "halberd-compile";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
