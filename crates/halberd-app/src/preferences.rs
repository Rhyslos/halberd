//! Saves settings changed in the editor (the Settings window, the View
//! menu, the first-launch question) back to the settings file, so they are
//! kept next time.

use halberd_config::{Settings, SettingsStore};

/// The settings in effect and where to save them.
pub(crate) struct PreferenceSaver {
    store: Option<SettingsStore>,
    settings: Settings,
}

impl PreferenceSaver {
    /// Starts from the settings loaded at startup. Without a store (the
    /// settings folder could not be found), changes are kept only for this
    /// session.
    pub(crate) fn new(settings: Settings, store: Option<SettingsStore>) -> Self {
        Self { store, settings }
    }

    /// The settings in effect.
    #[cfg(test)]
    pub(crate) fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Records the editor's settings, saving the file if they changed.
    /// Returns a message for the Console if saving failed (once per change,
    /// not every frame).
    pub(crate) fn update(&mut self, current: &Settings) -> Option<String> {
        if *current == self.settings {
            return None;
        }
        self.settings = current.clone();
        let store = self.store.as_ref()?;
        store
            .save(&self.settings)
            .err()
            .map(|err| format!("Settings were not saved: {err}."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halberd_config::LengthUnit;

    fn changed() -> Settings {
        let mut s = Settings::default();
        s.editor.length_unit = LengthUnit::Metres;
        s.editor.show_player_scale = false;
        s.editor.wasd_movement = Some(false);
        s.camera.fly_speed = 1200;
        s.keys.insert("file.save".into(), "F5".into());
        s
    }

    #[test]
    fn changed_settings_are_saved_and_load_next_time() {
        let folder = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(folder.path().join("settings.toml"));
        let mut saver = PreferenceSaver::new(Settings::default(), Some(store.clone()));
        assert_eq!(saver.update(&Settings::default()), None, "nothing changed");
        assert!(!store.path().exists(), "unchanged settings are not written");
        assert_eq!(saver.update(&changed()), None);
        let loaded = store.load().settings;
        assert_eq!(loaded, changed());
        assert_eq!(saver.settings(), &changed());
    }

    #[test]
    fn without_a_settings_folder_changes_last_the_session() {
        let mut saver = PreferenceSaver::new(Settings::default(), None);
        assert_eq!(saver.update(&changed()), None);
        assert_eq!(saver.settings(), &changed());
    }

    #[test]
    fn a_failed_save_is_reported_once() {
        let folder = tempfile::tempdir().unwrap();
        // A folder where the file should be: saving must fail.
        let blocked = folder.path().join("settings.toml");
        std::fs::create_dir(&blocked).unwrap();
        let mut saver =
            PreferenceSaver::new(Settings::default(), Some(SettingsStore::new(blocked)));
        let note = saver.update(&changed()).unwrap();
        assert!(note.starts_with("Settings were not saved"), "{note}");
        assert_eq!(saver.update(&changed()), None, "not again every frame");
    }
}
