//! The editor window's contents: menu bar, docked panels and the About box.

use crate::layout::{default_layout, restore_or_default};
use crate::panel::Panel;
use crate::viewport::{ViewportOptions, ViewportPanel, ViewportRenderer};
use egui::{Align2, Id, Key, KeyboardShortcut, Modifiers, RichText, ScrollArea, Ui, WidgetText};
use egui_dock::{DockArea, DockState, Style, TabViewer};

/// Facts about the program shown in the window and the About box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppInfo {
    /// Product name, such as "Halberd Map Editor".
    pub name: String,
    /// Version, such as "0.0.1".
    pub version: String,
}

/// Keyboard shortcut for File → Quit (Ctrl+Q, or Cmd+Q on macOS).
pub const QUIT_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Q);

/// Something the user asked for that the program around the workbench
/// must carry out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkbenchAction {
    /// File → Quit was chosen.
    Quit,
}

/// The editor window's contents.
pub struct Workbench {
    info: AppInfo,
    dock: DockState<Panel>,
    console: Vec<String>,
    about_open: bool,
    viewport: ViewportPanel,
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

    /// The viewport panel.
    pub fn viewport(&self) -> &ViewportPanel {
        &self.viewport
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
        if ui.input_mut(|input| input.consume_shortcut(&QUIT_SHORTCUT)) {
            actions.push(WorkbenchAction::Quit);
        }
        egui::Panel::top("halberd_menu_bar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| self.menu_bar(ui, &mut actions));
        });

        let mut viewer = PanelViewer {
            console: &self.console,
            viewport: &mut self.viewport,
            renderer,
        };
        DockArea::new(&mut self.dock)
            .style(Style::from_egui(ui.style().as_ref()))
            .show_close_buttons(false)
            .show_add_buttons(false)
            .show_leaf_close_all_buttons(false)
            .show_inside(ui, &mut viewer);

        let mut about_open = self.about_open;
        egui::Window::new("About Halberd")
            .open(&mut about_open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| about_contents(ui, &self.info));
        self.about_open = about_open;

        actions
    }

    fn menu_bar(&mut self, ui: &mut Ui, actions: &mut Vec<WorkbenchAction>) {
        ui.menu_button("File", |ui| {
            let shortcut = ui.ctx().format_shortcut(&QUIT_SHORTCUT);
            if ui
                .add(egui::Button::new("Quit").shortcut_text(shortcut))
                .clicked()
            {
                actions.push(WorkbenchAction::Quit);
            }
        });
        ui.menu_button("View", |ui| {
            if ui.button("Reset panel layout").clicked() {
                self.reset_layout();
                self.push_console("Panel layout reset to the standard arrangement.");
            }
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
            Panel::Viewport => self.viewport.show(ui, self.renderer),
            Panel::Library => placeholder(
                ui,
                "Props, materials and entities",
                "The asset library arrives in Phase 1.",
            ),
            Panel::Scene => placeholder(ui, "No map open", "The scene tree arrives with brushes."),
            Panel::Layers => placeholder(
                ui,
                "No layers yet",
                "Layers group sections of a map so they can be hidden, locked and \
                 selected together. They are saved to Hammer as visgroups.",
            ),
            Panel::Properties => placeholder(
                ui,
                "Nothing selected",
                "Select something to see its properties.",
            ),
            Panel::Console => console_contents(ui, self.console),
        }
    }
}

fn placeholder(ui: &mut Ui, heading: &str, detail: &str) {
    ui.add_space(8.0);
    ui.label(RichText::new(heading).strong());
    ui.label(RichText::new(detail).weak());
}

fn console_contents(ui: &mut Ui, lines: &[String]) {
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for line in lines {
                ui.label(RichText::new(line).monospace());
            }
        });
}

fn about_contents(ui: &mut Ui, info: &AppInfo) {
    ui.heading(format!("{} {}", info.name, info.version));
    ui.label("A modern map editor for Garry's Mod.");
    ui.add_space(6.0);
    ui.label("Created by Rhyslos, built with AI assistance (Claude by Anthropic).");
    ui.label("Licensed under the Apache License 2.0.");
    ui.add_space(6.0);
    ui.label(
        RichText::new(
            "Not affiliated with Valve or Facepunch Studios. Garry's Mod is a trademark of \
             Facepunch Studios; Hammer, Source and Steam are trademarks of Valve Corporation.",
        )
        .weak()
        .small(),
    );
}

#[cfg(test)]
mod tests;
