//! Loading and saving the settings file safely.

use crate::settings::{FORMAT_VERSION, Settings};
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Settings files larger than this are treated as damaged, not read.
/// A real settings file is a few kilobytes.
pub const MAX_SETTINGS_FILE_BYTES: u64 = 1024 * 1024;

/// Folder name used inside the system's configuration folder.
const APP_FOLDER: &str = "Halberd";
/// File name of the settings file.
const FILE_NAME: &str = "settings.toml";
/// First lines of every saved settings file.
const HEADER: &str = "# Halberd Map Editor settings.\n\
# Safe to edit by hand while Halberd is closed. Delete this file to reset everything.\n\n";

/// Reads and writes one settings file.
#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

/// The result of loading settings. Loading always produces usable settings.
#[derive(Debug, Clone)]
pub struct LoadOutcome {
    /// The settings to use, already within safe limits.
    pub settings: Settings,
    /// What happened while loading.
    pub status: LoadStatus,
    /// Plain-language notes for the console: corrected values, recovered
    /// files and similar. Empty when everything was normal.
    pub notes: Vec<String>,
}

/// What happened while loading settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadStatus {
    /// The file was read normally.
    Loaded,
    /// No file existed yet (first launch); defaults are used.
    NotFound,
    /// The file could not be understood. It was renamed to `backup`
    /// (when possible) and defaults are used.
    Recovered {
        /// Where the damaged file was moved, if moving it worked.
        backup: Option<PathBuf>,
    },
    /// The file could not be read (for example, no permission); defaults are used.
    Unreadable,
    /// The file was written by a newer Halberd. Its values are used, but it
    /// must not be overwritten, or settings this version does not know about
    /// would be lost. [`SettingsStore::save`] refuses in this case.
    NewerFormat {
        /// The layout version found in the file.
        found: u32,
    },
}

/// Why settings could not be saved or located.
#[derive(Debug)]
pub enum ConfigError {
    /// The system has no configuration folder Halberd can use.
    NoConfigFolder,
    /// A file or folder could not be written.
    Io {
        /// The path involved.
        path: PathBuf,
        /// The underlying error.
        source: io::Error,
    },
    /// The settings could not be turned into text (for example, a folder
    /// path that is not valid Unicode).
    Serialize(String),
    /// The existing file was written by a newer Halberd and is protected.
    NewerFormatProtected {
        /// The layout version found in the file.
        found: u32,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoConfigFolder => write!(f, "this system has no settings folder Halberd can use"),
            Self::Io { path, source } => write!(f, "could not write {}: {source}", path.display()),
            Self::Serialize(reason) => write!(f, "could not save settings: {reason}"),
            Self::NewerFormatProtected { found } => write!(
                f,
                "the settings file is from a newer Halberd (format {found}, this version uses \
                 {FORMAT_VERSION}); it was left unchanged to avoid losing settings"
            ),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl SettingsStore {
    /// A store for the settings file at `path`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// A store for the standard location: `Halberd/settings.toml` inside the
    /// system's configuration folder (`%APPDATA%` on Windows, `~/.config` on
    /// Linux, `~/Library/Application Support` on macOS).
    pub fn standard() -> Result<Self, ConfigError> {
        let base = config_folder(|name| std::env::var_os(name), std::env::consts::OS)
            .ok_or(ConfigError::NoConfigFolder)?;
        Ok(Self::new(base.join(APP_FOLDER).join(FILE_NAME)))
    }

    /// The settings file this store reads and writes.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads settings. Never fails: problems fall back to defaults and are
    /// explained in [`LoadOutcome::notes`].
    pub fn load(&self) -> LoadOutcome {
        let text = match read_limited(&self.path) {
            Ok(Some(text)) => text,
            Ok(None) => return outcome(Settings::default(), LoadStatus::NotFound, Vec::new()),
            Err(ReadProblem::TooLarge) => {
                return self.recover("the file is larger than a settings file can be".into());
            }
            Err(ReadProblem::NotText) => {
                return self.recover("the file is not plain text".into());
            }
            Err(ReadProblem::Io(err)) => {
                let note = format!(
                    "Could not read {} ({err}); using default settings for this session.",
                    self.path.display()
                );
                return outcome(Settings::default(), LoadStatus::Unreadable, vec![note]);
            }
        };

        let parsed: Settings = match toml::from_str(&text) {
            Ok(settings) => settings,
            Err(err) => return self.recover(first_line(&err.to_string())),
        };

        let found_version = parsed.format_version;
        let (settings, adjustments) = parsed.sanitized();
        let mut notes: Vec<String> = adjustments
            .iter()
            .map(|a| {
                format!(
                    "Setting {} was {}, which is out of range; using {}.",
                    a.setting, a.found, a.used
                )
            })
            .collect();

        let status = if found_version > FORMAT_VERSION {
            notes.push(format!(
                "Settings were saved by a newer Halberd (format {found_version}). They are used \
                 as far as this version understands them, and the file will not be overwritten."
            ));
            LoadStatus::NewerFormat {
                found: found_version,
            }
        } else {
            LoadStatus::Loaded
        };
        let mut settings = settings;
        settings.format_version = FORMAT_VERSION;
        outcome(settings, status, notes)
    }

    /// Saves settings, replacing the file only once the new one is fully
    /// written. Refuses to overwrite a file from a newer Halberd.
    pub fn save(&self, settings: &Settings) -> Result<(), ConfigError> {
        if let Some(found) = self.existing_newer_format() {
            return Err(ConfigError::NewerFormatProtected { found });
        }
        let mut to_save = settings.clone();
        to_save.format_version = FORMAT_VERSION;
        let body =
            toml::to_string_pretty(&to_save).map_err(|e| ConfigError::Serialize(e.to_string()))?;
        let text = format!("{HEADER}{body}");
        write_atomically(&self.path, text.as_bytes())
    }

    /// Returns the format version of the file on disk if it is newer than
    /// this build understands.
    fn existing_newer_format(&self) -> Option<u32> {
        #[derive(serde::Deserialize)]
        struct VersionOnly {
            #[serde(default)]
            format_version: u32,
        }
        let text = read_limited(&self.path).ok().flatten()?;
        let version = toml::from_str::<VersionOnly>(&text).ok()?.format_version;
        (version > FORMAT_VERSION).then_some(version)
    }

    /// Moves a damaged file aside and falls back to defaults.
    fn recover(&self, reason: String) -> LoadOutcome {
        let backup = free_backup_path(&self.path);
        let moved = backup.and_then(|b| fs::rename(&self.path, &b).ok().map(|()| b));
        let note = match &moved {
            Some(b) => format!(
                "The settings file could not be understood ({reason}). It was kept as {} and \
                 default settings are used.",
                b.display()
            ),
            None => format!(
                "The settings file could not be understood ({reason}) and could not be moved \
                 aside. Default settings are used for this session."
            ),
        };
        outcome(
            Settings::default(),
            LoadStatus::Recovered { backup: moved },
            vec![note],
        )
    }
}

/// The system's folder for per-user settings, following each platform's
/// convention. `env` looks up an environment variable; `os` is
/// `std::env::consts::OS`. Only absolute paths are accepted.
fn config_folder(env: impl Fn(&str) -> Option<std::ffi::OsString>, os: &str) -> Option<PathBuf> {
    let absolute = |name: &str| env(name).map(PathBuf::from).filter(|p| p.is_absolute());
    match os {
        "windows" => absolute("APPDATA"),
        "macos" => absolute("HOME").map(|home| home.join("Library").join("Application Support")),
        _ => absolute("XDG_CONFIG_HOME").or_else(|| absolute("HOME").map(|h| h.join(".config"))),
    }
}

fn outcome(settings: Settings, status: LoadStatus, notes: Vec<String>) -> LoadOutcome {
    LoadOutcome {
        settings,
        status,
        notes,
    }
}

enum ReadProblem {
    TooLarge,
    NotText,
    Io(io::Error),
}

/// Reads a file of at most [`MAX_SETTINGS_FILE_BYTES`]. `Ok(None)` means it does not exist.
fn read_limited(path: &Path) -> Result<Option<String>, ReadProblem> {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(ReadProblem::Io(e)),
    };
    let mut bytes = Vec::new();
    file.take(MAX_SETTINGS_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(ReadProblem::Io)?;
    if bytes.len() as u64 > MAX_SETTINGS_FILE_BYTES {
        return Err(ReadProblem::TooLarge);
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| ReadProblem::NotText)
}

/// Finds an unused name like `settings.toml.damaged-1` next to `path`.
fn free_backup_path(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    (1..=100)
        .map(|n| path.with_file_name(format!("{name}.damaged-{n}")))
        .find(|candidate| !candidate.exists())
}

/// Writes to a temporary file in the same folder, flushes it to disk, then
/// renames it over `path`. A crash at any point leaves either the old file
/// or the new one, never a mix.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), ConfigError> {
    let io_err = |p: &Path| {
        let p = p.to_path_buf();
        move |source| ConfigError::Io { path: p, source }
    };
    let folder = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(folder).map_err(io_err(folder))?;

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = folder.join(format!(".{name}.saving-{}", std::process::id()));

    let result = (|| {
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)
    })();

    result.map_err(|source| {
        let _ = fs::remove_file(&temp);
        ConfigError::Io {
            path: path.to_path_buf(),
            source,
        }
    })
}

/// The first line of a parser message, which is the readable part.
fn first_line(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or("unknown problem")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_store() -> (tempfile::TempDir, SettingsStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("settings.toml"));
        (dir, store)
    }

    #[test]
    fn missing_file_gives_defaults() {
        let (_dir, store) = temp_store();
        let out = store.load();
        assert_eq!(out.status, LoadStatus::NotFound);
        assert_eq!(out.settings, Settings::default());
        assert!(out.notes.is_empty());
    }

    #[test]
    fn save_then_load_round_trips() {
        let (_dir, store) = temp_store();
        let mut s = Settings::default();
        s.game.gmod_dir = Some(PathBuf::from("D:/SteamLibrary/steamapps/common/GarrysMod"));
        s.performance.memory_budget_mb = 4096;
        s.performance.worker_threads = 6;
        s.editor.autosave_minutes = 10;
        s.editor.grid_size = 64;
        s.editor.wasd_movement = Some(false);
        store.save(&s).unwrap();

        let out = store.load();
        assert_eq!(out.status, LoadStatus::Loaded);
        assert_eq!(out.settings, s);
        assert!(out.notes.is_empty());
    }

    #[test]
    fn saved_file_is_readable_text_with_a_header() {
        let (_dir, store) = temp_store();
        store.save(&Settings::default()).unwrap();
        let text = fs::read_to_string(store.path()).unwrap();
        assert!(text.starts_with("# Halberd Map Editor settings."));
        assert!(text.contains("[editor]"));
        assert!(text.contains("grid_size = 16"));
    }

    #[test]
    fn save_creates_missing_folders() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("a").join("b").join("settings.toml"));
        store.save(&Settings::default()).unwrap();
        assert!(store.path().is_file());
    }

    #[test]
    fn save_leaves_no_temporary_files() {
        let (dir, store) = temp_store();
        store.save(&Settings::default()).unwrap();
        store.save(&Settings::default()).unwrap();
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["settings.toml"]);
    }

    #[test]
    fn partial_file_keeps_defaults_for_missing_values() {
        let (_dir, store) = temp_store();
        fs::write(store.path(), "[editor]\ngrid_size = 8\n").unwrap();
        let out = store.load();
        assert_eq!(out.status, LoadStatus::Loaded);
        assert_eq!(out.settings.editor.grid_size, 8);
        assert_eq!(out.settings.editor.autosave_minutes, 5);
        assert_eq!(out.settings.performance.memory_budget_mb, 2048);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let (_dir, store) = temp_store();
        fs::write(
            store.path(),
            "future_thing = 3\n[editor]\nshiny = true\ngrid_size = 32\n",
        )
        .unwrap();
        let out = store.load();
        assert_eq!(out.status, LoadStatus::Loaded);
        assert_eq!(out.settings.editor.grid_size, 32);
    }

    #[test]
    fn out_of_range_values_are_corrected_with_notes() {
        let (_dir, store) = temp_store();
        fs::write(store.path(), "[performance]\nmemory_budget_mb = 3\n").unwrap();
        let out = store.load();
        assert_eq!(
            out.settings.performance.memory_budget_mb,
            Settings::MIN_MEMORY_MB
        );
        assert_eq!(out.notes.len(), 1);
        assert!(out.notes[0].contains("performance.memory_budget_mb"));
    }

    #[test]
    fn corrupt_file_is_backed_up_and_defaults_used() {
        let (dir, store) = temp_store();
        fs::write(store.path(), "this is [ not toml ===").unwrap();
        let out = store.load();
        let expected_backup = dir.path().join("settings.toml.damaged-1");
        assert_eq!(
            out.status,
            LoadStatus::Recovered {
                backup: Some(expected_backup.clone())
            }
        );
        assert_eq!(out.settings, Settings::default());
        assert!(expected_backup.is_file());
        assert!(!store.path().exists());
        assert!(out.notes[0].contains("could not be understood"));
    }

    #[test]
    fn repeated_corruption_uses_new_backup_names() {
        let (dir, store) = temp_store();
        fs::write(store.path(), "[[[").unwrap();
        store.load();
        fs::write(store.path(), "]]]").unwrap();
        store.load();
        assert!(dir.path().join("settings.toml.damaged-1").is_file());
        assert!(dir.path().join("settings.toml.damaged-2").is_file());
    }

    #[test]
    fn wrong_types_count_as_corrupt() {
        let (_dir, store) = temp_store();
        fs::write(store.path(), "[editor]\ngrid_size = \"big\"\n").unwrap();
        assert!(matches!(store.load().status, LoadStatus::Recovered { .. }));
    }

    #[test]
    fn negative_numbers_count_as_corrupt() {
        let (_dir, store) = temp_store();
        fs::write(store.path(), "[editor]\nautosave_minutes = -5\n").unwrap();
        assert!(matches!(store.load().status, LoadStatus::Recovered { .. }));
    }

    #[test]
    fn oversized_file_is_not_read() {
        let (_dir, store) = temp_store();
        let huge = "# padding\n".repeat((MAX_SETTINGS_FILE_BYTES as usize / 10) + 10);
        fs::write(store.path(), huge).unwrap();
        let out = store.load();
        assert!(matches!(out.status, LoadStatus::Recovered { .. }));
        assert!(out.notes[0].contains("larger than a settings file"));
    }

    #[test]
    fn binary_file_is_treated_as_corrupt() {
        let (_dir, store) = temp_store();
        fs::write(store.path(), [0xff, 0xfe, 0x00, 0x80, 0x81]).unwrap();
        let out = store.load();
        assert!(matches!(out.status, LoadStatus::Recovered { .. }));
        assert!(out.notes[0].contains("not plain text"));
    }

    #[test]
    fn empty_file_gives_defaults() {
        let (_dir, store) = temp_store();
        fs::write(store.path(), "").unwrap();
        let out = store.load();
        assert_eq!(out.status, LoadStatus::Loaded);
        assert_eq!(out.settings, Settings::default());
    }

    #[test]
    fn newer_format_is_read_but_never_overwritten() {
        let (_dir, store) = temp_store();
        let original = "format_version = 99\nnew_feature = \"keep me\"\n[editor]\ngrid_size = 32\n";
        fs::write(store.path(), original).unwrap();

        let out = store.load();
        assert_eq!(out.status, LoadStatus::NewerFormat { found: 99 });
        assert_eq!(out.settings.editor.grid_size, 32);
        assert!(out.notes.iter().any(|n| n.contains("newer Halberd")));

        let err = store.save(&out.settings).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::NewerFormatProtected { found: 99 }
        ));
        assert_eq!(fs::read_to_string(store.path()).unwrap(), original);
    }

    #[test]
    fn unicode_paths_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("Ærlig spill ✓").join("settings.toml"));
        let mut s = Settings::default();
        s.game.gmod_dir = Some(PathBuf::from("C:/Spill/Garry's Mod ÆØÅ"));
        store.save(&s).unwrap();
        assert_eq!(store.load().settings, s);
    }

    #[test]
    fn windows_style_paths_round_trip() {
        let (_dir, store) = temp_store();
        let mut s = Settings::default();
        s.game.gmod_dir = Some(PathBuf::from(
            r"C:\Program Files (x86)\Steam\steamapps\common\GarrysMod",
        ));
        store.save(&s).unwrap();
        assert_eq!(store.load().settings, s);
    }

    #[test]
    fn standard_location_ends_in_halberd_settings() {
        // CI machines always have a configuration folder.
        let store = SettingsStore::standard().unwrap();
        assert!(
            store
                .path()
                .ends_with(Path::new("Halberd").join("settings.toml"))
        );
    }

    fn env_from<'a>(
        pairs: &'a [(&'a str, &'a str)],
    ) -> impl Fn(&str) -> Option<std::ffi::OsString> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| (*v).into())
        }
    }

    #[test]
    fn windows_uses_appdata() {
        // "Absolute" depends on the system running the test, so use a path
        // in the style of the current system.
        let appdata = if cfg!(windows) {
            r"C:\Users\Rhys\AppData\Roaming"
        } else {
            "/c/Users/Rhys/AppData"
        };
        let pairs = [("APPDATA", appdata), ("HOME", "/ignored")];
        let env = env_from(&pairs);
        assert_eq!(config_folder(&env, "windows"), Some(PathBuf::from(appdata)));
    }

    #[test]
    fn macos_uses_application_support() {
        let env = env_from(&[("HOME", "/Users/rhys")]);
        assert_eq!(
            config_folder(&env, "macos"),
            Some(PathBuf::from("/Users/rhys/Library/Application Support"))
        );
    }

    #[test]
    fn linux_prefers_xdg_config_home() {
        let env = env_from(&[
            ("XDG_CONFIG_HOME", "/home/rhys/cfg"),
            ("HOME", "/home/rhys"),
        ]);
        assert_eq!(
            config_folder(&env, "linux"),
            Some(PathBuf::from("/home/rhys/cfg"))
        );
    }

    #[test]
    fn linux_falls_back_to_dot_config() {
        let env = env_from(&[("HOME", "/home/rhys")]);
        assert_eq!(
            config_folder(&env, "linux"),
            Some(PathBuf::from("/home/rhys/.config"))
        );
    }

    #[test]
    fn relative_or_missing_folders_are_rejected() {
        let relative = env_from(&[("XDG_CONFIG_HOME", "cfg"), ("HOME", "home")]);
        assert_eq!(config_folder(&relative, "linux"), None);
        assert_eq!(config_folder(env_from(&[]), "linux"), None);
        assert_eq!(config_folder(env_from(&[]), "windows"), None);
    }

    #[test]
    fn errors_read_as_plain_sentences() {
        let text = ConfigError::NewerFormatProtected { found: 7 }.to_string();
        assert!(text.contains("newer Halberd"));
        assert!(
            ConfigError::NoConfigFolder
                .to_string()
                .contains("settings folder")
        );
    }

    #[test]
    fn hostile_inputs_never_panic() {
        // A small built-in fuzz pass: random bytes and mutated valid files
        // must always produce usable settings. A real fuzz target comes with
        // the fuzzing setup milestone.
        let (_dir, store) = temp_store();
        let valid = "[editor]\ngrid_size = 16\nautosave_minutes = 5\n[game]\ngmod_dir = \"C:/x\"\n";
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for round in 0..500 {
            let mut bytes = valid.as_bytes().to_vec();
            if round % 2 == 0 {
                let len = (next() % 200) as usize;
                bytes = (0..len).map(|_| next() as u8).collect();
            } else {
                for _ in 0..(next() % 8) {
                    let i = (next() as usize) % bytes.len();
                    bytes[i] = next() as u8;
                }
            }
            fs::write(store.path(), &bytes).unwrap();
            let out = store.load();
            let (again, changes) = out.settings.clone().sanitized();
            assert!(
                changes.is_empty(),
                "loaded settings must already be within limits"
            );
            assert_eq!(again, out.settings);
            let _ = fs::remove_file(store.path());
            for entry in fs::read_dir(store.path().parent().unwrap()).unwrap() {
                let _ = fs::remove_file(entry.unwrap().path());
            }
        }
    }
}
