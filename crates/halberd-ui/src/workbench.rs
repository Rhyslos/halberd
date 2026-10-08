//! The editor window's contents: menu bar, docked panels and the About box.
//! The workbench also owns the open map, and carries out the Edit menu.

use crate::layout::{default_layout, restore_or_default};
use crate::panel::Panel;
use crate::panels::{
    about_contents, console_contents, placeholder, properties_contents, scene_contents,
};
use crate::viewport::{ViewportOptions, ViewportPanel, ViewportRenderer};
use egui::{Align2, Id, Key, KeyboardShortcut, Modifiers, Ui, WidgetText};
use egui_dock::{DockArea, DockState, Style, TabViewer};
use halberd_config::LengthUnit;
use halberd_doc::{Command, Document};

/// Facts about the program shown in the window and the About box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppInfo {
    /// Product name, such as "Halberd Map Editor".
    pub name: String,
    /// Version, such as "0.0.1".
    pub version: String,
}

/// The View menu's switch for the player figure.
pub const PLAYER_TOGGLE_LABEL: &str = "Show player for scale";

/// Keyboard shortcut for File → Quit (Ctrl+Q, or Cmd+Q on macOS).
pub const QUIT_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Q);
/// Edit → Undo (Ctrl+Z).
pub const UNDO_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
/// Edit → Redo (Ctrl+Y).
pub const REDO_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
/// Also Redo (Ctrl+Shift+Z), as in many programs.
const REDO_ALT_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(
    Modifiers {
        shift: true,
        ..Modifiers::COMMAND
    },
    Key::Z,
);
/// Edit → Delete (the Delete key).
pub const DELETE_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::Delete);
/// Also Delete: Backspace, which Mac keyboards label "delete".
const DELETE_ALT_SHORTCUT: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::NONE, Key::Backspace);

/// Something the user asked for that the program around the workbench
/// must carry out (the workbench never touches files or the window itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkbenchAction {
    /// Close the window: the user chose Quit, with nothing unsaved (or
    /// chose not to save).
    Quit,
    /// Show an Open window and load the chosen map with
    /// [`Workbench::set_document`].
    Open,
    /// Save the map to its file (or ask where, if it has none), then carry
    /// on with `then`.
    Save {
        /// What the user was doing when asked to save first.
        then: Option<FileIntent>,
    },
    /// Ask where to save the map, save it there, then carry on with `then`.
    SaveAs {
        /// What the user was doing when asked to save first.
        then: Option<FileIntent>,
    },
}

/// Something that would close the current map, and so may first need its
/// unsaved changes saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileIntent {
    /// Start a new, empty map.
    New,
    /// Open another map.
    Open,
    /// Close Halberd.
    Quit,
}

/// The editor window's contents.
pub struct Workbench {
    info: AppInfo,
    dock: DockState<Panel>,
    console: Vec<String>,
    about_open: bool,
    viewport: ViewportPanel,
    doc: Document,
    length_unit: LengthUnit,
    show_player: bool,
    /// The file the map was opened from or last saved to.
    file_path: Option<std::path::PathBuf>,
    /// What the user wants to do once they have answered "save changes?".
    asking_to_save: Option<FileIntent>,
    /// A problem to show in a message box.
    error: Option<String>,
}

impl Workbench {
    /// A workbench using a saved panel layout if it is usable, otherwise
    /// the standard one.
    pub fn new(info: AppInfo, saved_layout: Option<DockState<Panel>>) -> Self {
        let (dock, note) = restore_or_default(saved_layout);
        let mut workbench = Self {
            info,
            dock,
            console: Vec::new(),
            about_open: false,
            viewport: ViewportPanel::new(None, ViewportOptions::default()),
            doc: Document::new(),
            length_unit: LengthUnit::Units,
            show_player: true,
            file_path: None,
            asking_to_save: None,
            error: None,
        };
        if let Some(note) = note {
            workbench.push_console(note);
        }
        workbench
    }

    /// Replaces the viewport, for example with one restored from the last
    /// session and configured from settings.
    pub fn with_viewport(mut self, viewport: ViewportPanel) -> Self {
        self.viewport = viewport;
        self
    }

    /// Sets the display preferences from settings: the unit lengths are
    /// shown in, and whether the player figure is shown.
    pub fn with_display(mut self, length_unit: LengthUnit, show_player: bool) -> Self {
        self.set_length_unit(length_unit);
        self.show_player = show_player;
        self
    }

    /// The unit lengths are shown and typed in.
    pub fn length_unit(&self) -> LengthUnit {
        self.length_unit
    }

    /// Changes the unit lengths are shown and typed in.
    pub fn set_length_unit(&mut self, unit: LengthUnit) {
        self.length_unit = if unit == LengthUnit::Unknown {
            LengthUnit::Units
        } else {
            unit
        };
    }

    /// Whether the player figure is shown for scale.
    pub fn show_player(&self) -> bool {
        self.show_player
    }

    /// Shows or hides the player figure.
    pub fn set_show_player(&mut self, show: bool) {
        self.show_player = show;
    }

    /// The viewport panel.
    pub fn viewport(&self) -> &ViewportPanel {
        &self.viewport
    }

    /// The viewport panel, for changing its tool.
    pub fn viewport_mut(&mut self) -> &mut ViewportPanel {
        &mut self.viewport
    }

    /// The open map.
    pub fn document(&self) -> &Document {
        &self.doc
    }

    /// The open map, for changing it (edits through [`Document::execute`]).
    pub fn document_mut(&mut self) -> &mut Document {
        &mut self.doc
    }

    /// Edit → Undo.
    pub fn undo(&mut self) {
        self.doc.undo();
    }

    /// Edit → Redo.
    pub fn redo(&mut self) {
        self.doc.redo();
    }

    /// Edit → Delete: removes the selected objects (an undoable edit).
    pub fn delete_selection(&mut self) {
        let selected: Vec<_> = self.doc.selection().iter().copied().collect();
        if !selected.is_empty()
            && let Err(e) = self.doc.execute(Command::Remove(selected))
        {
            self.push_console(format!("Could not delete: {e}."));
        }
    }

    /// The current panel arrangement, for saving between sessions.
    pub fn layout(&self) -> &DockState<Panel> {
        &self.dock
    }

    /// Puts every panel back where the standard layout has it.
    pub fn reset_layout(&mut self) {
        self.dock = default_layout();
    }

    /// Adds a line to the Console panel.
    pub fn push_console(&mut self, line: impl Into<String>) {
        self.console.push(line.into());
    }

    /// The lines in the Console panel, oldest first.
    pub fn console_lines(&self) -> &[String] {
        &self.console
    }

    /// Whether the About box is showing.
    pub fn about_open(&self) -> bool {
        self.about_open
    }

    /// Draws the whole window and returns what the user asked for.
    /// `renderer` draws the 3D viewport.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        renderer: &mut dyn ViewportRenderer,
    ) -> Vec<WorkbenchAction> {
        let mut actions = Vec::new();
        self.file_shortcuts(ui, &mut actions);
        self.edit_shortcuts(ui);
        // Between drags, the next edit starts a new undo step; during a drag
        // (of a size field, say) its many small edits stay one step.
        if !ui.ctx().egui_is_using_pointer() {
            self.doc.end_step();
        }
        self.viewport
            .note_popup_open(egui::Popup::is_any_open(ui.ctx()));
        self.viewport
            .block_keys(self.is_asking_to_save() || self.error.is_some());
        self.viewport
            .set_display(self.length_unit, self.show_player);
        egui::Panel::top("halberd_menu_bar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| self.menu_bar(ui, &mut actions));
        });

        let mut notes = Vec::new();
        let mut viewer = PanelViewer {
            console: &self.console,
            viewport: &mut self.viewport,
            renderer,
            doc: &mut self.doc,
            notes: &mut notes,
            length_unit: self.length_unit,
        };
        DockArea::new(&mut self.dock)
            .style(Style::from_egui(ui.style().as_ref()))
            .show_close_buttons(false)
            .show_add_buttons(false)
            .show_leaf_close_all_buttons(false)
            .show_inside(ui, &mut viewer);
        self.console.extend(notes);

        let mut about_open = self.about_open;
        egui::Window::new("About Halberd")
            .open(&mut about_open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| about_contents(ui, &self.info));
        self.about_open = about_open;
        self.dialogs(ui, &mut actions);

        actions
    }

    /// Undo, redo and delete from the keyboard, unless a text box is in use.
    fn edit_shortcuts(&mut self, ui: &Ui) {
        // Mid-drag (a box being drawn, a gizmo handle held), undo and delete
        // would pull the map from under the drag; they wait until it ends.
        // While a box ("save changes?", a problem) is showing, the map
        // behind it stays as it is.
        if ui.ctx().egui_wants_keyboard_input()
            || self.viewport.tools().is_busy()
            || self.is_asking_to_save()
            || self.error.is_some()
        {
            return;
        }
        let consume = |shortcut: &KeyboardShortcut| ui.input_mut(|i| i.consume_shortcut(shortcut));
        // Ctrl+Shift+Z first: Ctrl+Z would also match it.
        if consume(&REDO_ALT_SHORTCUT) || consume(&REDO_SHORTCUT) {
            self.redo();
        } else if consume(&UNDO_SHORTCUT) {
            self.undo();
        }
        if consume(&DELETE_SHORTCUT) || consume(&DELETE_ALT_SHORTCUT) {
            self.delete_selection();
        }
    }

    fn edit_menu(&mut self, ui: &mut Ui) {
        let shortcut = |s: &KeyboardShortcut| ui.ctx().format_shortcut(s);
        let undo = match self.doc.undo_label() {
            Some(label) => format!("Undo {label}"),
            None => "Undo".to_string(),
        };
        let redo = match self.doc.redo_label() {
            Some(label) => format!("Redo {label}"),
            None => "Redo".to_string(),
        };
        let (undo_keys, redo_keys, delete_keys) = (
            shortcut(&UNDO_SHORTCUT),
            shortcut(&REDO_SHORTCUT),
            shortcut(&DELETE_SHORTCUT),
        );
        let can_undo = self.doc.undo_label().is_some();
        let can_redo = self.doc.redo_label().is_some();
        let can_delete = !self.doc.selection().is_empty();
        if ui
            .add_enabled(can_undo, egui::Button::new(undo).shortcut_text(undo_keys))
            .clicked()
        {
            self.undo();
        }
        if ui
            .add_enabled(can_redo, egui::Button::new(redo).shortcut_text(redo_keys))
            .clicked()
        {
            self.redo();
        }
        ui.separator();
        if ui
            .add_enabled(
                can_delete,
                egui::Button::new("Delete").shortcut_text(delete_keys),
            )
            .clicked()
        {
            self.delete_selection();
        }
    }

    fn menu_bar(&mut self, ui: &mut Ui, actions: &mut Vec<WorkbenchAction>) {
        ui.menu_button("File", |ui| self.file_menu(ui, actions));
        ui.menu_button("Edit", |ui| self.edit_menu(ui));
        ui.menu_button("View", |ui| {
            if ui.button("Reset panel layout").clicked() {
                self.reset_layout();
                self.push_console("Panel layout reset to the standard arrangement.");
            }
            ui.separator();
            ui.label("Show lengths in");
            for unit in LengthUnit::CHOICES {
                if ui
                    .radio(self.length_unit == unit, unit.label())
                    .on_hover_text("Maps are always saved in Hammer units; 1 unit is 2.54 cm")
                    .clicked()
                {
                    self.set_length_unit(unit);
                }
            }
            ui.separator();
            ui.checkbox(&mut self.show_player, PLAYER_TOGGLE_LABEL)
                .on_hover_text("A 72-unit (1.83 m) player figure next to what you are working on");
        });
        ui.menu_button("Help", |ui| {
            if ui.button("About Halberd").clicked() {
                self.about_open = true;
            }
        });
    }
}

/// Draws each panel's contents.
struct PanelViewer<'a> {
    console: &'a [String],
    viewport: &'a mut ViewportPanel,
    renderer: &'a mut dyn ViewportRenderer,
    doc: &'a mut Document,
    /// Messages for the Console, added after the panels are drawn.
    notes: &'a mut Vec<String>,
    length_unit: LengthUnit,
}

impl TabViewer for PanelViewer<'_> {
    type Tab = Panel;

    fn id(&mut self, tab: &mut Panel) -> Id {
        Id::new(("halberd_panel", *tab))
    }

    fn title(&mut self, tab: &mut Panel) -> WidgetText {
        tab.title().into()
    }

    fn is_closeable(&self, _tab: &Panel) -> bool {
        false
    }

    fn on_tab_button(&mut self, tab: &mut Panel, response: &egui::Response) {
        // Name the tab for screen readers (and UI tests); egui_dock paints
        // the title without labelling the button.
        let title = tab.title();
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, title));
    }

    fn allowed_in_windows(&self, _tab: &mut Panel) -> bool {
        // Panels dock inside the main window only, for now: a floating
        // window could be closed and take its panel with it.
        false
    }

    fn clear_background(&self, tab: &Panel) -> bool {
        // The viewport paints its own background.
        *tab != Panel::Viewport
    }

    fn scroll_bars(&self, tab: &Panel) -> [bool; 2] {
        // The viewport fills its panel exactly and uses the wheel for zoom.
        if *tab == Panel::Viewport {
            [false, false]
        } else {
            [true, true]
        }
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Panel) {
        match tab {
            Panel::Viewport => {
                let notes = self.viewport.show(ui, self.renderer, self.doc);
                self.notes.extend(notes);
            }
            Panel::Library => placeholder(
                ui,
                "Props, materials and entities",
                "The asset library arrives in Phase 1.",
            ),
            Panel::Scene => scene_contents(ui, self.doc),
            Panel::Layers => placeholder(
                ui,
                "No layers yet",
                "Layers group sections of a map so they can be hidden, locked and \
                 selected together. They are saved to Hammer as visgroups.",
            ),
            Panel::Properties => properties_contents(ui, self.doc, self.length_unit),
            Panel::Console => console_contents(ui, self.console),
        }
    }
}

mod files;

pub use files::{
    CANCEL_BUTTON, DONT_SAVE_BUTTON, NEW_SHORTCUT, OPEN_SHORTCUT, SAVE_AS_SHORTCUT, SAVE_BUTTON,
    SAVE_SHORTCUT,
};

#[cfg(test)]
mod file_tests;
#[cfg(test)]
mod tests;
