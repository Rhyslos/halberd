//! The arrangement of panels: the default, and checks on saved arrangements.

use crate::panel::Panel;
use egui_dock::{DockState, NodeIndex};

/// Share of the window width given to everything left of Scene/Properties.
const MAIN_AREA_WIDTH: f32 = 0.8;
/// Share of the right column given to Scene (Properties gets the rest).
const SCENE_HEIGHT: f32 = 0.5;
/// Share of the main area's height given to the viewport row (Console gets the rest).
const VIEWPORT_ROW_HEIGHT: f32 = 0.75;
/// Share of the viewport row given to the viewport (Browsers gets the rest).
const VIEWPORT_WIDTH: f32 = 0.8;

/// The standard arrangement from the feature spec:
///
/// ```text
/// ┌──────────┬────────────────────┬────────────┐
/// │ Browsers │      Viewport      │   Scene    │
/// │          │                    ├────────────┤
/// ├──────────┴────────────────────┤ Properties │
/// │            Console            │            │
/// └───────────────────────────────┴────────────┘
/// ```
pub fn default_layout() -> DockState<Panel> {
    let mut dock = DockState::new(vec![Panel::Viewport]);
    let tree = dock.main_surface_mut();
    let [main_area, right_column] =
        tree.split_right(NodeIndex::root(), MAIN_AREA_WIDTH, vec![Panel::Scene]);
    tree.split_below(right_column, SCENE_HEIGHT, vec![Panel::Properties]);
    let [viewport_row, _console] =
        tree.split_below(main_area, VIEWPORT_ROW_HEIGHT, vec![Panel::Console]);
    // egui_dock gives `fraction` to whichever side ends up left or top.
    // Browsers is the new node and lands on the left, so it gets the
    // fraction, not the viewport.
    tree.split_left(viewport_row, 1.0 - VIEWPORT_WIDTH, vec![Panel::Browsers]);
    dock
}

/// True if every panel appears exactly once.
///
/// A saved layout could be damaged or come from a version with different
/// panels; such layouts are replaced by the default.
pub fn is_complete(dock: &DockState<Panel>) -> bool {
    let mut seen = Vec::new();
    for (_, panel) in dock.iter_all_tabs() {
        if seen.contains(panel) {
            return false;
        }
        seen.push(*panel);
    }
    Panel::ALL.iter().all(|p| seen.contains(p))
}

/// Uses a saved layout if it is complete, otherwise the default.
/// Returns a note for the console when the saved layout was not usable.
pub fn restore_or_default(saved: Option<DockState<Panel>>) -> (DockState<Panel>, Option<String>) {
    match saved {
        None => (default_layout(), None),
        Some(dock) if is_complete(&dock) => (dock, None),
        Some(_) => (
            default_layout(),
            Some("The saved panel layout was incomplete, so the standard layout is used.".into()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Which panel shares a leaf (tab group) with which.
    fn leaves(dock: &DockState<Panel>) -> Vec<Vec<Panel>> {
        dock.main_surface()
            .iter()
            .filter_map(|node| node.tabs().map(|tabs| tabs.to_vec()))
            .collect()
    }

    #[test]
    fn default_layout_has_every_panel_once() {
        assert!(is_complete(&default_layout()));
    }

    #[test]
    fn default_layout_gives_each_panel_its_own_area() {
        let mut groups = leaves(&default_layout());
        groups.sort_by_key(|g| g.first().map(|p| p.title()));
        assert_eq!(
            groups,
            vec![
                vec![Panel::Browsers],
                vec![Panel::Console],
                vec![Panel::Properties],
                vec![Panel::Scene],
                vec![Panel::Viewport],
            ]
        );
    }

    #[test]
    fn default_layout_has_no_floating_windows() {
        assert_eq!(default_layout().iter_surfaces().count(), 1);
    }

    #[test]
    fn missing_panel_is_incomplete() {
        let dock = DockState::new(vec![Panel::Viewport, Panel::Scene]);
        assert!(!is_complete(&dock));
    }

    #[test]
    fn duplicate_panel_is_incomplete() {
        let mut dock = default_layout();
        dock.main_surface_mut()
            .split_right(NodeIndex::root(), 0.5, vec![Panel::Console]);
        assert!(!is_complete(&dock));
    }

    #[test]
    fn restore_keeps_a_complete_saved_layout() {
        // A user's own arrangement: two groups instead of five, but every
        // panel still present exactly once.
        let mut saved = DockState::new(vec![Panel::Viewport, Panel::Console]);
        saved.main_surface_mut().split_left(
            NodeIndex::root(),
            0.8,
            vec![Panel::Browsers, Panel::Scene, Panel::Properties],
        );
        let (dock, note) = restore_or_default(Some(saved));
        assert!(note.is_none());
        assert_eq!(leaves(&dock).len(), 2);
    }

    #[test]
    fn restore_replaces_a_broken_layout_with_a_note() {
        let broken = DockState::new(vec![Panel::Console]);
        let (dock, note) = restore_or_default(Some(broken));
        assert!(is_complete(&dock));
        assert!(note.is_some_and(|n| n.contains("standard layout")));
    }

    #[test]
    fn restore_without_a_saved_layout_is_silent() {
        let (dock, note) = restore_or_default(None);
        assert!(is_complete(&dock));
        assert!(note.is_none());
    }

    #[test]
    fn layout_survives_saving_and_loading() {
        // eframe stores the layout as RON text between sessions.
        let original = default_layout();
        let text = ron::to_string(&original).unwrap();
        let loaded: DockState<Panel> = ron::from_str(&text).unwrap();
        assert!(is_complete(&loaded));
        assert_eq!(leaves(&loaded), leaves(&original));
    }

    #[test]
    fn damaged_saved_text_does_not_load() {
        assert!(ron::from_str::<DockState<Panel>>("(surfaces: [garbage").is_err());
    }
}
