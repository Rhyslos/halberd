//! The File menu: new, open, save and quit, the "save changes?" question,
//! and message boxes for problems. Files themselves are opened and saved by
//! the program around the workbench, which the workbench asks through
//! [`WorkbenchAction`]s.

use super::{FileIntent, QUIT_SHORTCUT, Workbench, WorkbenchAction};
use egui::{Id, Key, KeyboardShortcut, Modifiers, RichText, Ui};
use halberd_doc::Document;
use std::path::{Path, PathBuf};

/// File → New (Ctrl+N).
pub const NEW_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
/// File → Open (Ctrl+O).
pub const OPEN_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
/// File → Save (Ctrl+S).
pub const SAVE_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
/// File → Save As (Ctrl+Shift+S).
pub const SAVE_AS_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(
    Modifiers {
        shift: true,
        ..Modifiers::COMMAND
    },
    Key::S,
);
/// Button in the "save changes?" box that saves first.
pub const SAVE_BUTTON: &str = "Save";
/// Button in the "save changes?" box that carries on without saving.
pub const DONT_SAVE_BUTTON: &str = "Don't save (N)";
/// Button that closes a box and changes nothing.
pub const CANCEL_BUTTON: &str = "Cancel";

/// True if `key` went down this frame on its own: not a key-repeat, and with
/// no Ctrl, Shift, Alt or Command held.
fn plain_press(input: &egui::InputState, key: Key) -> bool {
    input.events.iter().any(|e| {
        matches!(
            e,
            egui::Event::Key { key: k, pressed: true, repeat: false, modifiers, .. }
                if *k == key && modifiers.is_none()
        )
    })
}

impl Workbench {
    /// The file the map was opened from or last saved to.
    pub fn file_path(&self) -> Option<&Path> {
        self.file_path.as_deref()
    }

    /// Records where the map now lives (after Save As).
    pub fn set_file_path(&mut self, path: PathBuf) {
        self.file_path = Some(path);
    }

    /// True if the map has changes that are not saved.
    pub fn is_modified(&self) -> bool {
        self.doc.is_modified()
    }

    /// Replaces the open map, for example with one just opened from `path`.
    /// Nothing is selected, the tools start fresh, and the camera frames the
    /// whole map.
    pub fn set_document(&mut self, doc: Document, path: Option<PathBuf>) {
        let bounds = doc
            .objects()
            .map(|(_, o)| o.bounds())
            .reduce(halberd_geom::Aabb::union);
        if let Some(bounds) = bounds {
            self.viewport.frame(bounds);
        }
        self.doc = doc;
        self.file_path = path;
        self.viewport.set_tool(halberd_tools::Tool::Select);
        self.viewport.tools_mut().set_gizmo_mode(None);
    }

    /// Starts a new, empty map.
    pub fn new_document(&mut self) {
        self.set_document(Document::new(), None);
        self.push_console("New map.");
    }

    /// Shows a problem in a message box (and the Console).
    pub fn show_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.push_console(message.clone());
        self.error = Some(message);
    }

    /// The message box text, if one is showing.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// True while the "save changes?" box is showing.
    pub fn is_asking_to_save(&self) -> bool {
        self.asking_to_save.is_some()
    }

    /// The window title: the map's file name, a dot if it has unsaved
    /// changes, and the program's name.
    pub fn window_title(&self) -> String {
        let dot = if self.is_modified() { " •" } else { "" };
        format!(
            "{}{dot} — {} {}",
            self.map_name(),
            self.info.name,
            self.info.version
        )
    }

    fn map_name(&self) -> String {
        self.file_path
            .as_deref()
            .and_then(Path::file_name)
            .map_or_else(
                || "Untitled".to_string(),
                |n| n.to_string_lossy().into_owned(),
            )
    }

    /// Starts something that would close the current map. With unsaved
    /// changes, asks first ("save changes?") and returns nothing; the answer
    /// produces the action later. Otherwise returns the action to carry out
    /// now (or does it, for New).
    pub fn request(&mut self, intent: FileIntent) -> Option<WorkbenchAction> {
        if self.is_modified() {
            self.asking_to_save = Some(intent);
            return None;
        }
        self.carry_out(intent)
    }

    /// Does `intent` now, without asking: New is done here; Open and Quit
    /// need the program around the workbench.
    pub fn carry_out(&mut self, intent: FileIntent) -> Option<WorkbenchAction> {
        match intent {
            FileIntent::New => {
                self.new_document();
                None
            }
            FileIntent::Open => Some(WorkbenchAction::Open),
            FileIntent::Quit => Some(WorkbenchAction::Quit),
        }
    }

    pub(super) fn file_shortcuts(&mut self, ui: &Ui, actions: &mut Vec<WorkbenchAction>) {
        // Mid-drag, files wait until the drag ends.
        if self.viewport.tools().is_busy() || self.is_asking_to_save() {
            return;
        }
        let consume = |s: &KeyboardShortcut| ui.input_mut(|i| i.consume_shortcut(s));
        // Ctrl+Shift+S first: Ctrl+S would also match it.
        if consume(&SAVE_AS_SHORTCUT) {
            actions.push(WorkbenchAction::SaveAs { then: None });
        } else if consume(&SAVE_SHORTCUT) {
            actions.push(WorkbenchAction::Save { then: None });
        }
        let intent = if consume(&QUIT_SHORTCUT) {
            Some(FileIntent::Quit)
        } else if consume(&OPEN_SHORTCUT) {
            Some(FileIntent::Open)
        } else if consume(&NEW_SHORTCUT) {
            Some(FileIntent::New)
        } else {
            None
        };
        actions.extend(intent.and_then(|i| self.request(i)));
    }

    pub(super) fn file_menu(&mut self, ui: &mut Ui, actions: &mut Vec<WorkbenchAction>) {
        let item = |ui: &mut Ui, text: &str, shortcut: &KeyboardShortcut| {
            let keys = ui.ctx().format_shortcut(shortcut);
            ui.add(egui::Button::new(text).shortcut_text(keys))
                .clicked()
        };
        if item(ui, "New", &NEW_SHORTCUT) {
            actions.extend(self.request(FileIntent::New));
        }
        if item(ui, "Open…", &OPEN_SHORTCUT) {
            actions.extend(self.request(FileIntent::Open));
        }
        ui.separator();
        if item(ui, "Save", &SAVE_SHORTCUT) {
            actions.push(WorkbenchAction::Save { then: None });
        }
        if item(ui, "Save As…", &SAVE_AS_SHORTCUT) {
            actions.push(WorkbenchAction::SaveAs { then: None });
        }
        ui.separator();
        if item(ui, "Quit", &QUIT_SHORTCUT) {
            actions.extend(self.request(FileIntent::Quit));
        }
    }

    /// The "save changes?" box and the message box, when showing.
    pub(super) fn dialogs(&mut self, ui: &Ui, actions: &mut Vec<WorkbenchAction>) {
        if let Some(intent) = self.asking_to_save {
            let name = self.map_name();
            let mut answer = None;
            let modal = egui::Modal::new(Id::new("halberd_save_changes")).show(ui.ctx(), |ui| {
                ui.set_width(360.0);
                ui.heading(format!("Save changes to {name}?"));
                ui.label("Your changes will be lost if you don't save them.");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new(SAVE_BUTTON).strong()).clicked() {
                        answer = Some(Some(true));
                    }
                    if ui.button(DONT_SAVE_BUTTON).clicked() {
                        answer = Some(Some(false));
                    }
                    if ui.button(CANCEL_BUTTON).clicked() {
                        answer = Some(None);
                    }
                });
            });
            // Keys, as in most programs: Enter saves, N carries on without
            // saving, Escape (which also closes the box) cancels.
            // Only fresh presses with no Ctrl, Shift or Alt count: holding
            // Ctrl+N a moment too long must not answer "don't save".
            if answer.is_none() {
                let (enter, n) = ui.input(|i| (plain_press(i, Key::Enter), plain_press(i, Key::N)));
                if enter {
                    answer = Some(Some(true));
                } else if n {
                    answer = Some(Some(false));
                }
            }
            if modal.should_close() && answer.is_none() {
                answer = Some(None);
            }
            match answer {
                Some(Some(true)) => {
                    self.asking_to_save = None;
                    actions.push(WorkbenchAction::Save { then: Some(intent) });
                }
                Some(Some(false)) => {
                    self.asking_to_save = None;
                    actions.extend(self.carry_out(intent));
                }
                Some(None) => self.asking_to_save = None,
                None => {}
            }
        }
        if let Some(message) = self.error.clone() {
            let modal = egui::Modal::new(Id::new("halberd_error")).show(ui.ctx(), |ui| {
                ui.set_width(420.0);
                ui.heading("Something went wrong");
                ui.label(message);
                ui.add_space(8.0);
                ui.button("OK").clicked()
            });
            if modal.inner || modal.should_close() {
                self.error = None;
            }
        }
    }
}
