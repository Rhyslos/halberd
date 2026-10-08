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
            Panel::Browsers => placeholder(
                ui,
                "Props, materials and entities",
                "Asset browsing arrives in Phase 1.",
            ),
            Panel::Scene => placeholder(ui, "No map open", "The scene tree arrives with brushes."),
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
mod tests {
    use super::*;
    use crate::viewport::NoRenderer;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    fn info() -> AppInfo {
        AppInfo {
            name: "Halberd Map Editor".into(),
            version: "9.9.9".into(),
        }
    }

    fn harness_for(workbench: Workbench) -> Harness<'static, Workbench> {
        Harness::builder()
            .with_size([1280.0, 800.0])
            .build_ui_state(
                |ui, wb: &mut Workbench| {
                    wb.show(ui, &mut NoRenderer::default());
                },
                workbench,
            )
    }

    #[test]
    fn all_panel_tabs_are_visible() {
        let mut harness = harness_for(Workbench::new(info(), None));
        harness.run();
        for panel in Panel::ALL {
            assert!(
                harness.query_by_label(panel.title()).is_some(),
                "missing tab: {}",
                panel.title()
            );
        }
    }

    #[test]
    fn menu_bar_has_file_view_help() {
        let mut harness = harness_for(Workbench::new(info(), None));
        harness.run();
        for menu in ["File", "View", "Help"] {
            assert!(
                harness.query_by_label(menu).is_some(),
                "missing menu: {menu}"
            );
        }
    }

    #[test]
    fn console_shows_its_lines() {
        let mut wb = Workbench::new(info(), None);
        wb.push_console("Garry's Mod: found through Steam");
        let mut harness = harness_for(wb);
        harness.run();
        assert!(
            harness
                .query_by_label("Garry's Mod: found through Steam")
                .is_some()
        );
    }

    #[test]
    fn placeholders_explain_what_is_coming() {
        let mut harness = harness_for(Workbench::new(info(), None));
        harness.run();
        assert!(harness.query_by_label("No map open").is_some());
        assert!(harness.query_by_label("Nothing selected").is_some());
        assert!(harness.query_by_label(crate::VIEWPORT_LABEL).is_some());
    }

    #[test]
    fn about_box_opens_from_the_help_menu() {
        let mut harness = harness_for(Workbench::new(info(), None));
        harness.run();
        harness.get_by_label("Help").click();
        harness.run();
        harness.get_by_label("About Halberd").click();
        harness.run();
        assert!(harness.state().about_open());
        assert!(harness.query_by_label("Halberd Map Editor 9.9.9").is_some());
    }

    #[test]
    fn quit_is_reported_to_the_program() {
        let quit_requested = std::rc::Rc::new(std::cell::Cell::new(false));
        let flag = quit_requested.clone();
        let mut harness = Harness::builder()
            .with_size([1280.0, 800.0])
            .build_ui_state(
                move |ui, wb: &mut Workbench| {
                    if wb
                        .show(ui, &mut NoRenderer::default())
                        .contains(&WorkbenchAction::Quit)
                    {
                        flag.set(true);
                    }
                },
                Workbench::new(info(), None),
            );
        harness.run();
        harness.get_by_label("File").click();
        harness.run();
        harness.get_by_label_contains("Quit").click();
        harness.run();
        assert!(quit_requested.get());
    }

    #[test]
    fn ctrl_q_quits() {
        let quit_requested = std::rc::Rc::new(std::cell::Cell::new(false));
        let flag = quit_requested.clone();
        let mut harness = Harness::builder()
            .with_size([1280.0, 800.0])
            .build_ui_state(
                move |ui, wb: &mut Workbench| {
                    if wb
                        .show(ui, &mut NoRenderer::default())
                        .contains(&WorkbenchAction::Quit)
                    {
                        flag.set(true);
                    }
                },
                Workbench::new(info(), None),
            );
        harness.run();
        harness.key_press_modifiers(Modifiers::COMMAND, Key::Q);
        harness.run();
        assert!(quit_requested.get());
    }

    #[test]
    fn plain_q_does_not_quit() {
        let quit_requested = std::rc::Rc::new(std::cell::Cell::new(false));
        let flag = quit_requested.clone();
        let mut harness = Harness::builder()
            .with_size([1280.0, 800.0])
            .build_ui_state(
                move |ui, wb: &mut Workbench| {
                    if wb
                        .show(ui, &mut NoRenderer::default())
                        .contains(&WorkbenchAction::Quit)
                    {
                        flag.set(true);
                    }
                },
                Workbench::new(info(), None),
            );
        harness.run();
        // Q alone is reserved for radial menus later.
        harness.key_press(Key::Q);
        harness.run();
        assert!(!quit_requested.get());
    }

    #[test]
    fn reset_layout_restores_the_standard_arrangement() {
        let mut wb = Workbench::new(info(), None);
        wb.dock = DockState::new(vec![
            Panel::Viewport,
            Panel::Browsers,
            Panel::Scene,
            Panel::Properties,
            Panel::Console,
        ]);
        wb.reset_layout();
        assert_eq!(wb.layout().main_surface().num_tabs(), 5);
        assert_eq!(
            wb.layout()
                .main_surface()
                .iter()
                .filter(|n| n.is_leaf())
                .count(),
            5,
            "each panel back in its own area"
        );
    }

    /// Where each panel was drawn, after running the harness.
    fn drawn_rects(harness: &Harness<'_, Workbench>) -> Vec<(Panel, egui::Rect)> {
        harness
            .state()
            .layout()
            .main_surface()
            .iter()
            .filter_map(|node| {
                let rect = node.rect()?;
                let panel = *node.tabs()?.first()?;
                Some((panel, rect))
            })
            .collect()
    }

    #[test]
    fn default_layout_is_drawn_as_the_spec_shows() {
        let mut harness = harness_for(Workbench::new(info(), None));
        harness.run();
        let rects = drawn_rects(&harness);
        let rect = |p: Panel| {
            rects
                .iter()
                .find(|(q, _)| *q == p)
                .map(|(_, r)| *r)
                .unwrap()
        };
        let (viewport, browsers) = (rect(Panel::Viewport), rect(Panel::Browsers));
        let (scene, properties, console) = (
            rect(Panel::Scene),
            rect(Panel::Properties),
            rect(Panel::Console),
        );

        // The viewport is the main area: much wider than the browser column.
        assert!(
            viewport.width() > 3.0 * browsers.width(),
            "{viewport:?} vs {browsers:?}"
        );
        // Browsers left of the viewport; Scene and Properties right of both.
        assert!(browsers.center().x < viewport.center().x);
        assert!(scene.left() >= viewport.right() && properties.left() >= viewport.right());
        // Scene above Properties; Console below the viewport.
        assert!(scene.center().y < properties.center().y);
        assert!(console.top() >= viewport.bottom());
        // Console spans under both Browsers and the viewport.
        assert!(console.left() <= browsers.left() && console.right() >= viewport.right() - 1.0);
    }

    #[test]
    fn broken_saved_layout_is_replaced_and_noted() {
        let wb = Workbench::new(info(), Some(DockState::new(vec![Panel::Console])));
        assert!(crate::is_complete(wb.layout()));
        assert_eq!(wb.console_lines().len(), 1);
    }
}
