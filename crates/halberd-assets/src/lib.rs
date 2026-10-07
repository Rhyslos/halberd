//! Find, mount, index and cache game content.
//!
//! Finds the Garry's Mod install, mounts base, mounted-game and Workshop
//! content, indexes what is available, and keeps decoded assets in caches
//! that respect the memory budget.
//!
//! Implemented so far: finding the Garry's Mod install ([`detect_gmod`]),
//! either from a folder the user chose or through Steam, and checking it
//! really is a GMod install with the compile tools Halberd needs.

mod gmod;
mod steam;

pub use gmod::{
    CompileTool, CompileTools, FoundBy, GMOD_APP_ID, GmodInstall, InvalidGmodDir, inspect_gmod_dir,
};
pub use steam::{Detection, DetectionError, detect_gmod, detect_gmod_in};
