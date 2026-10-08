//! Tests for the workbench, run through the real interface without a window.

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
    wb.dock = DockState::new(Panel::ALL.to_vec());
    wb.reset_layout();
    assert_eq!(wb.layout().main_surface().num_tabs(), 6);
    assert_eq!(
        wb.layout()
            .main_surface()
            .iter()
            .filter(|n| n.is_leaf())
            .count(),
        5,
        "each panel back in its own area, with Layers beside Scene"
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
    let (viewport, library) = (rect(Panel::Viewport), rect(Panel::Library));
    let (scene, properties, console) = (
        rect(Panel::Scene),
        rect(Panel::Properties),
        rect(Panel::Console),
    );

    // The viewport is the main area: much wider than the library column.
    assert!(
        viewport.width() > 3.0 * library.width(),
        "{viewport:?} vs {library:?}"
    );
    // Library left of the viewport; Scene and Properties right of both.
    assert!(library.center().x < viewport.center().x);
    assert!(scene.left() >= viewport.right() && properties.left() >= viewport.right());
    // Scene above Properties; Console below the viewport.
    assert!(scene.center().y < properties.center().y);
    assert!(console.top() >= viewport.bottom());
    // Console spans under both Library and the viewport.
    assert!(console.left() <= library.left() && console.right() >= viewport.right() - 1.0);
}

#[test]
fn broken_saved_layout_is_replaced_and_noted() {
    let wb = Workbench::new(info(), Some(DockState::new(vec![Panel::Console])));
    assert!(crate::is_complete(wb.layout()));
    assert_eq!(wb.console_lines().len(), 1);
}

#[test]
fn layers_is_a_tab_beside_scene() {
    let mut harness = harness_for(Workbench::new(info(), None));
    harness.run();
    let scene = harness.state().layout().find_tab(&Panel::Scene).unwrap();
    let layers = harness.state().layout().find_tab(&Panel::Layers).unwrap();
    assert_eq!(scene.node_path(), layers.node_path());
    // Scene shows first; clicking the Layers tab shows the Layers placeholder.
    assert!(harness.query_by_label("No map open").is_some());
    assert!(harness.query_by_label("No layers yet").is_none());
    harness.get_by_label("Layers").click();
    harness.run();
    assert!(harness.query_by_label("No layers yet").is_some());
}
