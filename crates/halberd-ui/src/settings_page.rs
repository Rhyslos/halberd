//! The Settings window: key bindings, the camera (WASD, speeds) and the
//! memory budget. Changes apply at once and are saved by the program.

use crate::bindings::{BindingGroup, Bindings, Command, check_binding};
use egui::{Color32, Key, KeyboardShortcut, Modifiers, RichText, Ui};
use halberd_config::{Budgets, CameraSettings, Settings};

/// The menu bar button and the window's title.
pub const SETTINGS_TITLE: &str = "Settings";
/// Screen-reader name of the key filter box (and for tests).
pub const KEY_FILTER_NAME: &str = "Filter keys";
/// The button that puts every key back.
pub const RESET_KEYS_LABEL: &str = "Reset all keys";
/// The WASD checkbox.
pub const WASD_LABEL: &str = "Fly with W, A, S, D while holding the right mouse button";
/// Height of the key list, in points.
const KEY_LIST_HEIGHT: f32 = 420.0;
/// Colour of a warning that two commands share a key.
const CLASH_COLOR: Color32 = Color32::from_rgb(255, 170, 60);
/// Colour of a key that was refused.
const REFUSED_COLOR: Color32 = Color32::from_rgb(255, 110, 100);

/// The parts of the Settings window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    /// Key bindings.
    #[default]
    Keys,
    /// WASD and camera speeds.
    Camera,
    /// The memory budget.
    Performance,
}

impl SettingsTab {
    /// Every tab, in order.
    pub const ALL: [SettingsTab; 3] = [Self::Keys, Self::Camera, Self::Performance];

    /// The tab's name.
    pub fn label(self) -> &'static str {
        match self {
            Self::Keys => "Keys",
            Self::Camera => "Camera",
            Self::Performance => "Performance",
        }
    }
}

/// The Settings window's own state.
#[derive(Debug, Default)]
pub struct SettingsPage {
    /// The tab showing.
    pub tab: SettingsTab,
    /// Text that narrows the key list.
    pub filter: String,
    /// The command waiting for its new key.
    capturing: Option<Command>,
    /// Why the last key pressed was refused.
    refused: Option<String>,
}

/// The name of a command's key button (for tests and screen readers).
pub fn key_button_name(command: Command) -> String {
    format!("Key for {}", command.label())
}

impl SettingsPage {
    /// True while waiting for a key: other keys must not act meanwhile.
    pub fn is_capturing(&self) -> bool {
        self.capturing.is_some()
    }

    /// Stops waiting for a key (the window closed).
    pub fn stop_capturing(&mut self) {
        self.capturing = None;
    }

    /// Draws the window's contents, changing `settings` and `bindings`.
    pub fn show(&mut self, ui: &mut Ui, settings: &mut Settings, bindings: &mut Bindings) {
        self.take_captured_key(ui, bindings);
        ui.horizontal(|ui| {
            for tab in SettingsTab::ALL {
                if ui.selectable_label(self.tab == tab, tab.label()).clicked() {
                    self.tab = tab;
                    self.capturing = None;
                }
            }
        });
        ui.separator();
        match self.tab {
            SettingsTab::Keys => self.keys_tab(ui, bindings),
            SettingsTab::Camera => camera_tab(ui, settings),
            SettingsTab::Performance => performance_tab(ui, settings),
        }
    }

    /// While waiting for a key: the first key pressed (with its modifiers)
    /// becomes the command's key, unless refused; Escape cancels. Every
    /// key press this frame is taken, so nothing else acts on it.
    fn take_captured_key(&mut self, ui: &Ui, bindings: &mut Bindings) {
        let Some(command) = self.capturing else {
            return;
        };
        let pressed = ui.input_mut(|i| {
            let found = i.events.iter().find_map(captured_key).or_else(|| {
                // Ctrl+Shift+C and X arrive as Copy and Cut; with other
                // modifiers (Shift+Delete on Windows) they aren't keys.
                if !halberd_modifiers(i.modifiers).matches_exact(CTRL_SHIFT) {
                    return None;
                }
                let key = if i.events.contains(&egui::Event::Copy) {
                    Key::C
                } else if i.events.contains(&egui::Event::Cut) {
                    Key::X
                } else {
                    return None;
                };
                Some(KeyboardShortcut::new(halberd_modifiers(i.modifiers), key))
            });
            i.events.retain(|e| {
                !matches!(
                    e,
                    egui::Event::Key { .. } | egui::Event::Copy | egui::Event::Cut
                )
            });
            found
        });
        let Some(key) = pressed else {
            return;
        };
        self.capturing = None;
        if key.logical_key == Key::Escape && key.modifiers.is_none() {
            self.refused = None;
            return;
        }
        match check_binding(command, key) {
            Ok(()) => {
                bindings.set(command, Some(key));
                self.refused = None;
            }
            Err(why) => self.refused = Some(format!("{}: {why}", command.label())),
        }
    }

    fn keys_tab(&mut self, ui: &mut Ui, bindings: &mut Bindings) {
        ui.horizontal(|ui| {
            ui.label("Filter");
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text("an action, like \"save\" or \"gizmo\"")
                    .desired_width(240.0),
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, KEY_FILTER_NAME)
            });
            if ui
                .button(RESET_KEYS_LABEL)
                .on_hover_text("Every command back to its usual key")
                .clicked()
            {
                bindings.reset_all();
                self.capturing = None;
                self.refused = None;
            }
        });
        match self.capturing {
            Some(command) => {
                ui.label(
                    RichText::new(format!(
                        "Press the new key for \"{}\" (with Ctrl, Shift or Alt if you like). \
                         Esc cancels.",
                        command.label()
                    ))
                    .strong(),
                );
            }
            None => {
                ui.label(RichText::new("Click a key to change it.").weak().small());
            }
        }
        if let Some(why) = &self.refused {
            ui.label(RichText::new(why).color(REFUSED_COLOR));
        }
        let filter = self.filter.trim().to_lowercase();
        egui::ScrollArea::vertical()
            .max_height(KEY_LIST_HEIGHT)
            .min_scrolled_height(KEY_LIST_HEIGHT)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // One grid for every group, so the columns line up.
                egui::Grid::new("halberd_keys")
                    .num_columns(4)
                    .spacing([10.0, 3.0])
                    .show(ui, |ui| {
                        for group in [
                            BindingGroup::Anywhere,
                            BindingGroup::Viewport,
                            BindingGroup::Drawing,
                            BindingGroup::Flying,
                        ] {
                            self.key_group(ui, bindings, group, &filter);
                        }
                    });
            });
    }

    fn key_group(
        &mut self,
        ui: &mut Ui,
        bindings: &mut Bindings,
        group: BindingGroup,
        filter: &str,
    ) {
        let commands: Vec<Command> = Command::ALL
            .iter()
            .copied()
            .filter(|c| c.group() == group)
            .filter(|c| filter.is_empty() || c.label().to_lowercase().contains(filter))
            .collect();
        if commands.is_empty() {
            return;
        }
        ui.label(RichText::new(group.label()).strong());
        ui.end_row();
        for command in commands {
            self.key_row(ui, bindings, command);
            ui.end_row();
        }
    }

    fn key_row(&mut self, ui: &mut Ui, bindings: &mut Bindings, command: Command) {
        ui.label(command.label());
        let waiting = self.capturing == Some(command);
        let text = match (waiting, bindings.key(command)) {
            (true, _) => "Press a key…".to_string(),
            (false, Some(key)) => ui.ctx().format_shortcut(&key),
            (false, None) => "(none)".to_string(),
        };
        let name = key_button_name(command);
        let button = ui
            .add(egui::Button::new(RichText::new(text).monospace()).selected(waiting))
            .on_hover_text("Click, then press the new key");
        button.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
        if button.clicked() {
            self.capturing = (!waiting).then_some(command);
            self.refused = None;
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(bindings.key(command).is_some(), egui::Button::new("Clear"))
                .on_hover_text("No key for this")
                .clicked()
            {
                bindings.set(command, None);
            }
            if ui
                .add_enabled(bindings.is_changed(command), egui::Button::new("Reset"))
                .on_hover_text(match command.default_key() {
                    Some(key) => format!("Back to {}", ui.ctx().format_shortcut(&key)),
                    None => "Back to no key".to_string(),
                })
                .clicked()
            {
                bindings.set(command, command.default_key());
            }
        });
        let clashes = bindings.clashes(command);
        if clashes.is_empty() {
            ui.label("");
        } else {
            let names: Vec<&str> = clashes.iter().map(|c| c.label()).collect();
            ui.label(RichText::new(format!("Also: {}", names.join(", "))).color(CLASH_COLOR))
                .on_hover_text(
                    "These share the key where both work, so pressing it does both. Give one \
                     of them another key.",
                );
        }
    }
}

/// The key a key-press event gives, with the modifiers in Halberd's form
/// (Ctrl and Cmd both as "Ctrl"). Ctrl+Shift+C and X arrive as Copy and
/// Cut, with no key.
fn captured_key(event: &egui::Event) -> Option<KeyboardShortcut> {
    match event {
        // Ctrl, Shift and Alt send presses of their own: wait for the key
        // they are held with.
        egui::Event::Key {
            key,
            pressed: true,
            repeat: false,
            modifiers,
            ..
        } if !crate::bindings::is_modifier_key(*key) => {
            Some(KeyboardShortcut::new(halberd_modifiers(*modifiers), *key))
        }
        _ => None,
    }
}

/// Ctrl and Shift.
const CTRL_SHIFT: Modifiers = Modifiers {
    shift: true,
    ..Modifiers::COMMAND
};

/// `m` as Halberd stores modifiers: Ctrl (or Cmd) as the command key.
fn halberd_modifiers(m: Modifiers) -> Modifiers {
    Modifiers {
        alt: m.alt,
        shift: m.shift,
        command: m.command || m.ctrl || m.mac_cmd,
        ..Modifiers::NONE
    }
}

fn camera_tab(ui: &mut Ui, settings: &mut Settings) {
    let mut wasd = settings.editor.wasd_movement.unwrap_or(true);
    if ui
        .checkbox(&mut wasd, WASD_LABEL)
        .on_hover_text(
            "Turns the Fly camera on: hold right mouse to look around and fly with W, A, S, D, \
             Space and C, like in a game. Off: right-drag only orbits.",
        )
        .changed()
    {
        settings.editor.wasd_movement = Some(wasd);
    }
    ui.add_space(6.0);
    let camera = &mut settings.camera;
    camera_speeds(ui, camera);
    ui.add_space(6.0);
    if ui
        .add_enabled(
            *camera != CameraSettings::default(),
            egui::Button::new("Reset camera speeds"),
        )
        .clicked()
    {
        *camera = CameraSettings::default();
    }
}

/// Flying, turning and zooming speeds.
fn camera_speeds(ui: &mut Ui, camera: &mut CameraSettings) {
    egui::Grid::new("halberd_camera_settings")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label("Flying speed");
            ui.add(
                egui::DragValue::new(&mut camera.fly_speed)
                    .range(CameraSettings::MIN_FLY_SPEED..=CameraSettings::MAX_FLY_SPEED)
                    .speed(8.0)
                    .suffix(" units/s"),
            )
            .on_hover_text(
                "How fast the Fly camera moves when Halberd starts (a player sprints at about \
                 400). The mouse wheel changes it while flying.",
            );
            ui.end_row();
            ui.label("Turning speed");
            ui.add(
                egui::Slider::new(
                    &mut camera.turn_speed_percent,
                    CameraSettings::MIN_PERCENT..=CameraSettings::MAX_PERCENT,
                )
                .suffix(" %"),
            )
            .on_hover_text("How far the 3D camera turns as you drag with the right button");
            ui.end_row();
            ui.label("Wheel zoom speed");
            ui.add(
                egui::Slider::new(
                    &mut camera.zoom_speed_percent,
                    CameraSettings::MIN_PERCENT..=CameraSettings::MAX_PERCENT,
                )
                .suffix(" %"),
            )
            .on_hover_text("How far each turn of the mouse wheel zooms, in every view");
            ui.end_row();
        });
}

fn performance_tab(ui: &mut Ui, settings: &mut Settings) {
    ui.horizontal(|ui| {
        ui.label("Memory budget");
        ui.add(
            egui::Slider::new(
                &mut settings.performance.memory_budget_mb,
                Settings::MIN_MEMORY_MB..=Settings::MAX_MEMORY_MB,
            )
            .logarithmic(true)
            .suffix(" MB"),
        )
        .on_hover_text("How much memory Halberd may use for pictures and textures");
    });
    let budgets = Budgets::for_memory(settings.performance.memory_budget_mb);
    ui.label(format!(
        "Keeps up to {} pictures in each of the material and prop browsers, and {} material \
         textures on the graphics card for the Textured view.",
        budgets.browser_pictures, budgets.material_textures
    ));
    ui.label(
        RichText::new("Takes effect the next time Halberd starts.")
            .weak()
            .small(),
    );
}

#[cfg(test)]
mod tests;
