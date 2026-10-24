//! The workbench's settings: what the program loaded, the Settings window,
//! the first-launch question about WASD, and putting changes into effect
//! (keys, camera). The program saves [`Workbench::settings`] when it
//! changes.

use super::Workbench;
use crate::bindings::Bindings;
use crate::settings_page::{SETTINGS_TITLE, SettingsPage};
use egui::{Align2, RichText, Ui};
use halberd_config::Settings;

/// The first-launch question's window title.
pub const WASD_QUESTION_TITLE: &str = "Camera keys";
/// The first-launch question's "yes" button.
pub const WASD_YES_LABEL: &str = "Yes, fly with WASD";
/// The first-launch question's "no" button.
pub const WASD_NO_LABEL: &str = "No, only orbit";

/// The settings and the Settings window's state.
#[derive(Debug, Default)]
pub(super) struct SettingsState {
    /// The settings in effect (the keys and display choices live in the
    /// viewport and the workbench, and are merged in by `settings`).
    config: Settings,
    page: SettingsPage,
    open: bool,
    /// Ask about WASD (first launch: the settings had no answer).
    ask_wasd: bool,
    /// The flying speed last put into the cameras.
    applied_fly_speed: Option<u32>,
}

impl Workbench {
    /// Takes the settings loaded at startup: keys, camera feel, WASD (asked
    /// about on first launch), units and the player figure. Keys the file
    /// gets wrong are explained in the Console.
    #[must_use]
    pub fn with_settings(mut self, settings: Settings) -> Self {
        let (bindings, notes) = Bindings::from_settings(&settings.keys);
        for note in notes {
            self.push_warning(note);
        }
        self.set_length_unit(settings.editor.length_unit);
        self.show_player = settings.editor.show_player_scale;
        self.settings.ask_wasd = settings.editor.wasd_movement.is_none();
        self.settings.config = settings;
        self.settings.applied_fly_speed = None;
        self.apply_bindings(bindings);
        self.apply_settings();
        self
    }

    /// The settings as they are now, to save.
    pub fn settings(&self) -> Settings {
        let mut settings = self.settings.config.clone();
        settings.editor.length_unit = self.length_unit;
        settings.editor.show_player_scale = self.show_player;
        settings.keys = self.viewport.bindings().to_settings();
        settings
    }

    /// Whether the Settings window is showing.
    pub fn settings_open(&self) -> bool {
        self.settings.open
    }

    /// Shows or hides the Settings window.
    pub fn set_settings_open(&mut self, open: bool) {
        self.settings.open = open;
        if !open {
            self.settings.page.stop_capturing();
        }
    }

    /// True while the Settings window waits for a new key: every other
    /// key waits too.
    pub(super) fn capturing_key(&self) -> bool {
        self.settings.open && self.settings.page.is_capturing()
    }

    /// Uses `bindings` everywhere keys are read or shown.
    fn apply_bindings(&mut self, bindings: Bindings) {
        self.keybinds.set_bindings(&bindings);
        self.viewport.set_bindings(bindings);
    }

    /// Puts the camera settings into effect.
    fn apply_settings(&mut self) {
        let config = &self.settings.config;
        let camera = &config.camera;
        self.viewport
            .set_camera_feel(camera.turn_factor(), camera.zoom_factor());
        // Not asked yet counts as on (as it was before Settings existed).
        self.viewport
            .set_wasd_enabled(config.editor.wasd_movement.unwrap_or(true));
        if self.settings.applied_fly_speed != Some(camera.fly_speed) {
            self.viewport.set_fly_speed(camera.fly_speed as f32);
            self.settings.applied_fly_speed = Some(camera.fly_speed);
        }
    }

    /// The Settings window and the first-launch question, when showing.
    pub(super) fn settings_windows(&mut self, ui: &Ui) {
        self.wasd_question(ui);
        if !self.settings.open {
            return;
        }
        let mut open = true;
        let mut config = self.settings.config.clone();
        let mut bindings = self.viewport.bindings().clone();
        let page = &mut self.settings.page;
        let screen = ui.ctx().content_rect();
        egui::Window::new(SETTINGS_TITLE)
            .open(&mut open)
            .pivot(Align2::CENTER_TOP)
            .default_pos(egui::pos2(screen.center().x, screen.top() + 60.0))
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.set_width(640.0);
                page.show(ui, &mut config, &mut bindings);
            });
        if bindings != *self.viewport.bindings() {
            self.apply_bindings(bindings);
        }
        if config != self.settings.config {
            if config.editor.wasd_movement.is_some() {
                self.settings.ask_wasd = false;
            }
            self.settings.config = config;
            self.apply_settings();
        }
        if !open {
            self.set_settings_open(false);
        }
    }

    /// On first launch: does the user want WASD flying? A small window in
    /// the corner, so it never covers the map.
    fn wasd_question(&mut self, ui: &Ui) {
        if !self.settings.ask_wasd {
            return;
        }
        let mut answer = None;
        egui::Window::new(WASD_QUESTION_TITLE)
            .anchor(Align2::RIGHT_BOTTOM, [-12.0, -12.0])
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.set_max_width(300.0);
                ui.label(
                    "Hold the right mouse button and use W, A, S, D to fly through the map, \
                     as in a game?",
                );
                ui.label(
                    RichText::new("You can change this later in Settings, under Camera.")
                        .weak()
                        .small(),
                );
                ui.horizontal(|ui| {
                    if ui.button(RichText::new(WASD_YES_LABEL).strong()).clicked() {
                        answer = Some(true);
                    }
                    if ui.button(WASD_NO_LABEL).clicked() {
                        answer = Some(false);
                    }
                });
            });
        if let Some(yes) = answer {
            self.settings.ask_wasd = false;
            self.settings.config.editor.wasd_movement = Some(yes);
            self.apply_settings();
            self.push_console(if yes {
                "Fly with WASD: on (change it in Settings, under Camera)."
            } else {
                "Fly with WASD: off (change it in Settings, under Camera)."
            });
        }
    }
}
