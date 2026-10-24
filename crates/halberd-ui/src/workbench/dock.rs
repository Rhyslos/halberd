//! The docked panels: which contents each tab shows.

use super::*;

/// Draws each panel's contents.
pub(super) struct PanelViewer<'a> {
    pub(super) console: &'a mut Console,
    pub(super) viewport: &'a mut ViewportPanel,
    pub(super) renderer: &'a mut dyn ViewportRenderer,
    pub(super) doc: &'a mut Document,
    /// Messages for the Console, added after the panels are drawn.
    pub(super) notes: &'a mut Vec<String>,
    pub(super) length_unit: LengthUnit,
    pub(super) library: &'a mut Library,
    pub(super) library_action: &'a mut Option<LibraryAction>,
    pub(super) stats: &'a mut crate::panels::map_summary::StatsCache,
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
                // Brushes, or brush entities standing for theirs.
                self.library.can_tie = self.doc.selection().iter().any(|id| {
                    matches!(self.doc.get(*id), Some(halberd_doc::Object::Brush(_)))
                        || self.doc.has_brushes(*id)
                });
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
            Panel::Properties => {
                let defs = self.library.defs.clone();
                let notes = properties_contents(
                    ui,
                    self.doc,
                    self.length_unit,
                    self.stats,
                    defs.as_deref(),
                );
                self.notes.extend(notes);
            }
            Panel::Console => self.console.show(ui),
        }
    }
}
