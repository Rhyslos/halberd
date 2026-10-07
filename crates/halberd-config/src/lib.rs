//! Settings, key bindings and memory budgets.
//!
//! Halberd's settings live in one human-readable TOML file. This crate
//! defines what is in it ([`Settings`]), keeps every value within safe
//! limits ([`Settings::sanitized`]), and loads and saves it safely
//! ([`SettingsStore`]):
//!
//! - Loading never fails. A missing file gives defaults; a corrupt file is
//!   set aside as a backup and defaults are used, with a note explaining it.
//! - Saving never leaves a half-written file: it writes a temporary file
//!   first and swaps it in, so a crash mid-save keeps the old settings.
//! - A file written by a newer Halberd is read, but never overwritten, so
//!   settings this version does not know about are not lost.
//!
//! # Example
//!
//! ```
//! use halberd_config::{Settings, SettingsStore};
//!
//! # let dir = tempfile::tempdir().unwrap();
//! let store = SettingsStore::new(dir.path().join("settings.toml"));
//! let loaded = store.load();
//! let mut settings = loaded.settings;
//! settings.editor.grid_size = 32;
//! store.save(&settings).unwrap();
//! assert_eq!(store.load().settings.editor.grid_size, 32);
//! ```

mod settings;
mod store;

pub use settings::{
    Adjustment, EditorSettings, FORMAT_VERSION, GameSettings, PerformanceSettings, Settings,
};
pub use store::{ConfigError, LoadOutcome, LoadStatus, MAX_SETTINGS_FILE_BYTES, SettingsStore};
