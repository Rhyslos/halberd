//! The editor window's contents: menu bar, docked panels and the About box.
//! The workbench also owns the open map, and carries out the Edit menu.

use crate::console::Console;
use crate::keybinds::{KEYBINDS_TITLE, KeybindsViewer};
use crate::layout::{default_layout, restore_or_default};
use crate::panel::Panel;
use crate::panels::{about_contents, placeholder, properties_contents, scene_contents};
use crate::viewport::{ViewportOptions, ViewportPanel, ViewportRenderer};
pub use compile::{
    COMPILE_LABEL, COMPILE_SHORTCUT, CompileLine, LEAK_CHECK_LABEL, SHOW_LEAK_LABEL,
    STOP_COMPILE_LABEL,
};
use egui::{Align2, Id, Key, KeyboardShortcut, Modifiers, Ui, WidgetText};
use egui_dock::{DockArea, DockState, Style, TabViewer};
pub use entity_list::{DEFAULT_SKY, ENTITIES_TAB, SKY_PICKER_NAME, sky_names};
use halberd_config::LengthUnit;
use halberd_doc::{Command, Document};
use halberd_tools::SelectMode;
pub use library::{ContentStatus, MATERIALS_TAB, PROPS_TAB};
use library::{Library, LibraryAction};

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
/// The View menu's switch that makes the player figure follow the pointer.
pub const PLAYER_FOLLOW_LABEL: &str = "Player follows the pointer";

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
    /// Compile the map (as it is now) and start Garry's Mod on it; report
    /// with [`Workbench::compile_started`] and the other `compile_`
    /// calls.
    Compile,
    /// Check the map for leaks (only the first compiler runs; nothing is
    /// installed or played); report as for [`WorkbenchAction::Compile`],
    /// starting with [`Workbench::leak_check_started`].
    CheckLeaks,
    /// Stop the compile that is running.
    CancelCompile,
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
    console: Console,
    about_open: bool,
    /// The Keybinds window is showing.
    keybinds_open: bool,
    keybinds: KeybindsViewer,
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
    /// The Library panel: the material and prop browsers.
    library: Library,
    /// The compile running, if any.
    compiling: Option<compile::CompileProgress>,
    /// The map's size and limits, kept while the map doesn't change.
    stats: crate::panels::map_summary::StatsCache,
    /// The settings and the Settings window.
    settings: settings::SettingsState,
}

impl Workbench {
    /// A workbench using a saved panel layout if it is usable, otherwise
    /// the standard one.
    pub fn new(info: AppInfo, saved_layout: Option<DockState<Panel>>) -> Self {
        let (dock, note) = restore_or_default(saved_layout);
        let mut workbench = Self {
            info,
            dock,
            console: Console::new(),
            about_open: false,
            keybinds_open: false,
            keybinds: KeybindsViewer::default(),
            viewport: ViewportPanel::new(None, ViewportOptions::default()),
            doc: Document::new(),
            length_unit: LengthUnit::Units,
            show_player: true,
            file_path: None,
            asking_to_save: None,
            error: None,
            library: Library::default(),
            compiling: None,
            stats: Default::default(),
            settings: Default::default(),
        };
        if let Some(note) = note {
            workbench.push_warning(note);
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
        // In Vertex, Edge or Face mode, Delete takes away the picked parts,
        // never the whole brushes.
        if self.delete_waits() {
            let notes = self
                .viewport
                .run_shape_op(halberd_tools::ShapeOp::DeleteParts, &mut self.doc);
            for note in notes {
                self.push_warning(note);
            }
            return;
        }
        let selected: Vec<_> = self.doc.selection().iter().copied().collect();
        if !selected.is_empty()
            && let Err(e) = self.doc.execute(Command::Remove(selected))
        {
            self.push_warning(format!("Could not delete: {e}."));
        }
    }

    /// True if Delete is for picked parts, not whole objects: the Select
    /// tool is picking corners, edges or faces of selected brushes.
    fn delete_waits(&self) -> bool {
        let tools = self.viewport.tools();
        // Floor plan shapes have no parts to pick: a selection of only
        // shapes is deleted whole.
        let selection = self.doc.selection();
        tools.tool() == halberd_tools::Tool::Select
            && tools.select_mode() != SelectMode::Object
            && !selection.is_empty()
            && !selection
                .iter()
                .all(|id| self.doc.get(*id).is_some_and(|o| o.as_shape().is_some()))
    }

    /// The current panel arrangement, for saving between sessions.
    pub fn layout(&self) -> &DockState<Panel> {
        &self.dock
    }

    /// Puts every panel back where the standard layout has it.
    pub fn reset_layout(&mut self) {
        self.dock = default_layout();
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
        // While Settings waits for a new key, keys do nothing else.
        let capturing = self.capturing_key();
        if !capturing {
            self.file_shortcuts(ui, &mut actions);
            self.compile_shortcut(ui, &mut actions);
            self.edit_shortcuts(ui);
        }
        // Between drags, the next edit starts a new undo step; during a drag
        // (of a size field, say) its many small edits stay one step.
        if !ui.ctx().egui_is_using_pointer() {
            self.doc.end_step();
        }
        self.viewport
            .note_popup_open(egui::Popup::is_any_open(ui.ctx()));
        self.viewport
            .block_keys(self.is_asking_to_save() || self.error.is_some() || capturing);
        self.viewport
            .set_display(self.length_unit, self.show_player);
        egui::Panel::top("halberd_menu_bar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| self.menu_bar(ui, &mut actions));
        });

        let mut notes = Vec::new();
        let mut library_action = None;
        let mut viewer = PanelViewer {
            console: &mut self.console,
            viewport: &mut self.viewport,
            renderer,
            doc: &mut self.doc,
            notes: &mut notes,
            length_unit: self.length_unit,
            library: &mut self.library,
            library_action: &mut library_action,
            stats: &mut self.stats,
        };
        DockArea::new(&mut self.dock)
            .style(Style::from_egui(ui.style().as_ref()))
            .show_close_buttons(false)
            .show_add_buttons(false)
            .show_leaf_close_all_buttons(false)
            .show_inside(ui, &mut viewer);
        for note in notes {
            self.push_warning(note);
        }
        if let Some(action) = library_action {
            self.library_action(action);
        }

        let mut about_open = self.about_open;
        egui::Window::new("About Halberd")
            .open(&mut about_open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| about_contents(ui, &self.info));
        self.about_open = about_open;
        let mut keybinds_open = self.keybinds_open;
        let keybinds = &mut self.keybinds;
        // Placed by its top edge, so it grows downwards, never off the top.
        let screen = ui.ctx().content_rect();
        egui::Window::new(KEYBINDS_TITLE)
            .open(&mut keybinds_open)
            .pivot(Align2::CENTER_TOP)
            .default_pos(egui::pos2(screen.center().x, screen.top() + 60.0))
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| keybinds.show(ui));
        self.keybinds_open = keybinds_open;
        self.settings_windows(ui);
        self.dialogs(ui, &mut actions);
        self.viewport
            .note_text_focus(ui.ctx().egui_wants_keyboard_input());
        // The Console tab's count changed after the tabs were drawn.
        if self.console.take_badge_change() {
            ui.ctx().request_repaint();
        }

        actions
    }
}

/// Draws each panel's contents.
struct PanelViewer<'a> {
    console: &'a mut Console,
    viewport: &'a mut ViewportPanel,
    renderer: &'a mut dyn ViewportRenderer,
    doc: &'a mut Document,
    /// Messages for the Console, added after the panels are drawn.
    notes: &'a mut Vec<String>,
    length_unit: LengthUnit,
    library: &'a mut Library,
    library_action: &'a mut Option<LibraryAction>,
    stats: &'a mut crate::panels::map_summary::StatsCache,
}

impl TabViewer for PanelViewer<'_> {
    type Tab = Panel;

    fn id(&mut self, tab: &mut Panel) -> Id {
        Id::new(("halberd_panel", *tab))
    }

    fn title(&mut self, tab: &mut Panel) -> WidgetText {
        if *tab == Panel::Console {
            // With a count of problems that arrived while it was hidden.
            return self.console.tab_text();
        }
        tab.title().into()
    }

    fn is_closeable(&self, _tab: &Panel) -> bool {
        false
    }

    fn on_tab_button(&mut self, tab: &mut Panel, response: &egui::Response) {
        // Name the tab for screen readers (and UI tests); egui_dock paints
        // the title without labelling the button.
        let title = match tab {
            Panel::Console => self.console.tab_title().0,
            _ => tab.title().to_string(),
        };
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &title));
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
            Panel::Library => {
                let can_apply = !self.doc.selection().is_empty();
                let sky = self.doc.world_setting("skyname").map(str::to_string);
                self.library
                    .show(ui, can_apply, sky.as_deref(), self.library_action);
            }
            Panel::Scene => scene_contents(ui, self.doc),
            Panel::Layers => placeholder(
                ui,
                "No layers yet",
                "Layers group sections of a map so they can be hidden, locked and \
                 selected together. They are saved to Hammer as visgroups.",
            ),
            Panel::Properties => properties_contents(ui, self.doc, self.length_unit, self.stats),
            Panel::Console => self.console.show(ui),
        }
    }
}

mod compile;
mod entity_list;
mod files;
mod library;
mod menus;
mod messages;
mod settings;

pub use files::{
    CANCEL_BUTTON, DONT_SAVE_BUTTON, NEW_SHORTCUT, OPEN_SHORTCUT, SAVE_AS_SHORTCUT, SAVE_BUTTON,
    SAVE_SHORTCUT,
};
pub use settings::{WASD_NO_LABEL, WASD_QUESTION_TITLE, WASD_YES_LABEL};

#[cfg(test)]
mod basic_entity_tests;
#[cfg(test)]
mod compile_tests;
#[cfg(test)]
mod console_tests;
#[cfg(test)]
mod entity_tests;
#[cfg(test)]
mod file_tests;
#[cfg(test)]
mod keybinds_tests;
#[cfg(test)]
mod library_tests;
#[cfg(test)]
mod map_summary_tests;
#[cfg(test)]
mod settings_tests;
#[cfg(test)]
mod shape_tests;
#[cfg(test)]
mod tests;
