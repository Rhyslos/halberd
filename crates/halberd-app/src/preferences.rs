//! Saves display preferences changed in the editor (View menu) back to the
//! settings file, so they are kept next time.

use halberd_config::{LengthUnit, Settings, SettingsStore};

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

    /// Records the current display preferences, saving the settings file if
    /// they changed. Returns a message for the Console if saving failed.
    pub(crate) fn update(&mut self, length_unit: LengthUnit, show_player: bool) -> Option<String> {
        let editor = &mut self.settings.editor;
        if editor.length_unit == length_unit && editor.show_player_scale == show_player {
            return None;
        }
        editor.length_unit = length_unit;
        editor.show_player_scale = show_player;
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

    #[test]
    fn changed_preferences_are_saved_and_load_next_time() {
        let folder = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(folder.path().join("settings.toml"));
        let mut saver = PreferenceSaver::new(Settings::default(), Some(store.clone()));
        assert_eq!(
            saver.update(LengthUnit::Units, true),
            None,
            "nothing changed"
        );
        assert!(!store.path().exists(), "unchanged settings are not written");
        assert_eq!(saver.update(LengthUnit::Metres, false), None);
        let loaded = store.load().settings;
        assert_eq!(loaded.editor.length_unit, LengthUnit::Metres);
        assert!(!loaded.editor.show_player_scale);
        assert_eq!(saver.settings().editor.length_unit, LengthUnit::Metres);
    }

    #[test]
    fn without_a_settings_folder_changes_last_the_session() {
        let mut saver = PreferenceSaver::new(Settings::default(), None);
        assert_eq!(saver.update(LengthUnit::Metres, true), None);
        assert_eq!(saver.settings().editor.length_unit, LengthUnit::Metres);
    }

    #[test]
    fn a_failed_save_is_reported() {
        let folder = tempfile::tempdir().unwrap();
        // A folder where the file should be: saving must fail.
        let blocked = folder.path().join("settings.toml");
        std::fs::create_dir(&blocked).unwrap();
        let mut saver =
            PreferenceSaver::new(Settings::default(), Some(SettingsStore::new(blocked)));
        let note = saver.update(LengthUnit::Metres, true).unwrap();
        assert!(note.starts_with("Settings were not saved"), "{note}");
    }
}
