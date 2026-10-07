//! What Halberd's settings contain, their defaults and their safe limits.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Version of the settings file layout this build writes.
///
/// Raise it when a field is renamed or changes meaning, and add a migration.
/// Adding a new field with a default does not need a new version.
pub const FORMAT_VERSION: u32 = 1;

/// All of Halberd's settings.
///
/// Every field has a default, so a settings file only needs the values
/// someone changed, and a file from an older Halberd still loads.
/// Unknown fields are ignored, so a file from a newer Halberd also loads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Layout version of the file these settings came from.
    pub format_version: u32,
    /// Where the game is installed.
    pub game: GameSettings,
    /// How much of the computer Halberd may use.
    pub performance: PerformanceSettings,
    /// How the editor behaves.
    pub editor: EditorSettings,
}

/// Where the game is installed.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GameSettings {
    /// The Garry's Mod folder (the one containing `garrysmod` and `bin`).
    /// `None` means "find it through Steam".
    pub gmod_dir: Option<PathBuf>,
}

/// How much of the computer Halberd may use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PerformanceSettings {
    /// Memory budget in megabytes. Caches shrink to stay under it.
    pub memory_budget_mb: u32,
    /// Threads for background work. 0 means "choose automatically".
    pub worker_threads: u32,
}

/// How the editor behaves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorSettings {
    /// Minutes between autosaves. 0 turns autosave off.
    pub autosave_minutes: u32,
    /// Grid size in Hammer units. Always a power of two.
    pub grid_size: u32,
    /// Whether WASD moves the camera while right mouse is held.
    /// `None` means the user has not been asked yet (first launch).
    pub wasd_movement: Option<bool>,
}

/// One value that was out of range and has been corrected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adjustment {
    /// The setting, as written in the file, such as `editor.grid_size`.
    pub setting: &'static str,
    /// The value found in the file.
    pub found: String,
    /// The value used instead.
    pub used: String,
}

impl Settings {
    /// Smallest allowed memory budget, in megabytes.
    pub const MIN_MEMORY_MB: u32 = 512;
    /// Largest allowed memory budget, in megabytes (256 GB).
    pub const MAX_MEMORY_MB: u32 = 262_144;
    /// Largest allowed number of worker threads.
    pub const MAX_WORKER_THREADS: u32 = 256;
    /// Longest allowed autosave interval, in minutes.
    pub const MAX_AUTOSAVE_MINUTES: u32 = 120;
    /// Smallest allowed grid size, in Hammer units.
    pub const MIN_GRID: u32 = 1;
    /// Largest allowed grid size, in Hammer units.
    pub const MAX_GRID: u32 = 4096;

    /// Returns these settings with every value brought within its limits,
    /// plus a list of what had to change.
    ///
    /// Values are clamped to the nearest allowed value. A grid size that is
    /// not a power of two is rounded to the nearest power of two.
    pub fn sanitized(mut self) -> (Self, Vec<Adjustment>) {
        let mut changes = Vec::new();
        let mut fix = |setting: &'static str, value: &mut u32, allowed: u32| {
            if *value != allowed {
                changes.push(Adjustment {
                    setting,
                    found: value.to_string(),
                    used: allowed.to_string(),
                });
                *value = allowed;
            }
        };

        let memory = self.performance.memory_budget_mb;
        fix(
            "performance.memory_budget_mb",
            &mut self.performance.memory_budget_mb,
            memory.clamp(Self::MIN_MEMORY_MB, Self::MAX_MEMORY_MB),
        );
        let threads = self.performance.worker_threads;
        fix(
            "performance.worker_threads",
            &mut self.performance.worker_threads,
            threads.min(Self::MAX_WORKER_THREADS),
        );
        let autosave = self.editor.autosave_minutes;
        fix(
            "editor.autosave_minutes",
            &mut self.editor.autosave_minutes,
            autosave.min(Self::MAX_AUTOSAVE_MINUTES),
        );
        let grid = self.editor.grid_size;
        fix(
            "editor.grid_size",
            &mut self.editor.grid_size,
            nearest_grid(grid),
        );

        (self, changes)
    }
}

/// Clamps a grid size to the allowed range and rounds it to the nearest
/// power of two (ties round down, to the finer grid).
fn nearest_grid(value: u32) -> u32 {
    let clamped = value.clamp(Settings::MIN_GRID, Settings::MAX_GRID);
    let lower = 1u32 << (31 - clamped.leading_zeros());
    let upper = lower.saturating_mul(2).min(Settings::MAX_GRID);
    if clamped - lower <= upper.saturating_sub(clamped) {
        lower
    } else {
        upper
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            game: GameSettings::default(),
            performance: PerformanceSettings::default(),
            editor: EditorSettings::default(),
        }
    }
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            memory_budget_mb: 2048,
            worker_threads: 0,
        }
    }
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            autosave_minutes: 5,
            grid_size: 16,
            wasd_movement: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_technical_design() {
        let s = Settings::default();
        assert_eq!(s.format_version, FORMAT_VERSION);
        assert_eq!(s.game.gmod_dir, None);
        assert_eq!(s.performance.memory_budget_mb, 2048);
        assert_eq!(s.performance.worker_threads, 0);
        assert_eq!(s.editor.autosave_minutes, 5);
        assert_eq!(s.editor.grid_size, 16);
        assert_eq!(s.editor.wasd_movement, None);
    }

    #[test]
    fn defaults_need_no_adjustment() {
        let (s, changes) = Settings::default().sanitized();
        assert!(changes.is_empty());
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn out_of_range_values_are_clamped_and_reported() {
        let mut s = Settings::default();
        s.performance.memory_budget_mb = 1;
        s.performance.worker_threads = 10_000;
        s.editor.autosave_minutes = 9_999;
        s.editor.grid_size = 1_000_000;
        let (s, changes) = s.sanitized();
        assert_eq!(s.performance.memory_budget_mb, Settings::MIN_MEMORY_MB);
        assert_eq!(s.performance.worker_threads, Settings::MAX_WORKER_THREADS);
        assert_eq!(s.editor.autosave_minutes, Settings::MAX_AUTOSAVE_MINUTES);
        assert_eq!(s.editor.grid_size, Settings::MAX_GRID);
        assert_eq!(changes.len(), 4);
        assert!(
            changes.iter().any(|c| c.setting == "editor.grid_size"
                && c.found == "1000000"
                && c.used == "4096")
        );
    }

    #[test]
    fn huge_memory_budget_is_capped() {
        let mut s = Settings::default();
        s.performance.memory_budget_mb = u32::MAX;
        assert_eq!(
            s.sanitized().0.performance.memory_budget_mb,
            Settings::MAX_MEMORY_MB
        );
    }

    #[test]
    fn grid_sizes_round_to_powers_of_two() {
        assert_eq!(nearest_grid(0), 1);
        assert_eq!(nearest_grid(1), 1);
        assert_eq!(nearest_grid(3), 2); // tie between 2 and 4: finer grid wins
        assert_eq!(nearest_grid(5), 4);
        assert_eq!(nearest_grid(7), 8);
        assert_eq!(nearest_grid(16), 16);
        assert_eq!(nearest_grid(20), 16);
        assert_eq!(nearest_grid(25), 32);
        assert_eq!(nearest_grid(4096), 4096);
        assert_eq!(nearest_grid(u32::MAX), 4096);
    }

    #[test]
    fn every_grid_result_is_a_power_of_two_in_range() {
        for value in (0..5000).chain([u32::MAX - 1, u32::MAX]) {
            let g = nearest_grid(value);
            assert!(g.is_power_of_two(), "{value} gave {g}");
            assert!((Settings::MIN_GRID..=Settings::MAX_GRID).contains(&g));
        }
    }
}
