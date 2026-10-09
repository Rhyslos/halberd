//! Tests for the workbench, run through the real interface without a window.

use super::*;
use crate::viewport::NoRenderer;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

pub(super) fn info() -> AppInfo {
    AppInfo {
        name: "Halberd Map Editor".into(),
        version: "9.9.9".into(),
    }
}

pub(super) fn harness_for(workbench: Workbench) -> Harness<'static, Workbench> {
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
fn menu_bar_has_file_edit_view_help() {
    let mut harness = harness_for(Workbench::new(info(), None));
    harness.run();
    for menu in ["File", "Edit", "View", "Help"] {
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
    assert!(harness.query_by_label("No brushes yet").is_some());
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
    assert!(harness.query_by_label("No brushes yet").is_some());
    assert!(harness.query_by_label("No layers yet").is_none());
    harness.get_by_label("Layers").click();
    harness.run();
    assert!(harness.query_by_label("No layers yet").is_some());
}

pub(super) fn workbench_with_boxes(count: usize) -> Workbench {
    let mut wb = Workbench::new(info(), None);
    let brushes = (0..count)
        .map(|i| {
            let x = i as f32 * 100.0;
            halberd_geom::Brush::cuboid(halberd_geom::Aabb::from_corners(
                glam::Vec3::new(x, 0.0, 0.0),
                glam::Vec3::new(x + 64.0, 64.0, 64.0),
            ))
            .unwrap()
        })
        .collect();
    wb.document_mut()
        .execute(halberd_doc::Command::AddBrushes(brushes))
        .unwrap();
    wb
}

#[test]
fn delete_undo_and_redo_from_the_keyboard() {
    let mut harness = harness_for(workbench_with_boxes(1));
    harness.run();
    harness.key_press(Key::Delete);
    harness.run();
    assert!(harness.state().document().is_empty());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert_eq!(harness.state().document().len(), 1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Y);
    harness.run();
    assert!(harness.state().document().is_empty());
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    harness.run();
    assert!(
        harness.state().document().is_empty(),
        "nothing more to redo"
    );
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    harness.run();
    assert!(harness.state().document().is_empty(), "Ctrl+Shift+Z redoes");
}

#[test]
fn edit_menu_names_what_undo_would_reverse() {
    let mut harness = harness_for(workbench_with_boxes(2));
    harness.run();
    harness.get_by_label("Edit").click();
    harness.run();
    harness
        .get_by_label_contains("Undo Create 2 brushes")
        .click();
    harness.run();
    assert!(harness.state().document().is_empty());
    harness.get_by_label("Edit").click();
    harness.run();
    assert!(
        harness
            .query_by_label_contains("Redo Create 2 brushes")
            .is_some()
    );
}

#[test]
fn scene_panel_lists_objects_and_selects_them() {
    let mut wb = workbench_with_boxes(2);
    wb.document_mut().clear_selection();
    let mut harness = harness_for(wb);
    harness.run();
    let first = harness.state().document().objects().next().unwrap().0;
    harness.get_by_label(&format!("Brush {first}")).click();
    harness.run();
    let selected: Vec<_> = harness
        .state()
        .document()
        .selection()
        .iter()
        .copied()
        .collect();
    assert_eq!(selected, [first]);
}

#[test]
fn properties_panel_summarises_the_selection() {
    let mut harness = harness_for(workbench_with_boxes(1));
    harness.run();
    assert!(harness.query_by_label("1 object selected").is_some());
    for field in crate::panels::BOX_FIELD_NAMES {
        assert!(harness.query_by_label(field).is_some(), "missing {field}");
    }
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.query_by_label("Nothing selected").is_some());
}

#[test]
fn backspace_also_deletes() {
    let mut harness = harness_for(workbench_with_boxes(1));
    harness.run();
    harness.key_press(Key::Backspace);
    harness.run();
    assert!(harness.state().document().is_empty());
}

#[test]
fn escape_that_closes_a_menu_keeps_the_selection() {
    // Regression: Escape closed the Edit menu and also deselected.
    let mut harness = harness_for(workbench_with_boxes(1));
    harness.run();
    harness.get_by_label("Edit").click();
    harness.run();
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().document().selection().len(), 1);
}

/// Types `text` into the Properties number field named `field`.
pub(super) fn type_into(harness: &mut Harness<'static, Workbench>, field: &str, text: &str) {
    harness.get_by_label(field).click();
    harness.run();
    harness.get_by_label(field).type_text(text);
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
}

fn only_bounds(wb: &Workbench) -> halberd_geom::Aabb {
    wb.document().objects().next().unwrap().1.bounds().unwrap()
}

#[test]
fn typing_a_height_resizes_the_box_and_can_be_undone() {
    let mut harness = harness_for(workbench_with_boxes(1));
    harness.run();
    type_into(&mut harness, "Box height", "200");
    let bounds = only_bounds(harness.state());
    assert_eq!(bounds.size().z, 200.0);
    assert_eq!(bounds.min.z, 0.0, "the lowest corner stays put");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert_eq!(only_bounds(harness.state()).size().z, 64.0);
}

#[test]
fn in_metres_typed_values_are_converted_to_whole_units() {
    let mut wb = workbench_with_boxes(1);
    wb.set_length_unit(halberd_config::LengthUnit::Metres);
    let mut harness = harness_for(wb);
    harness.run();
    type_into(&mut harness, "Box width", "2");
    // 2 m is 78.74 units; brushes keep whole units.
    assert_eq!(only_bounds(harness.state()).size().x, 79.0);
    type_into(&mut harness, "Box position Z", "1");
    assert_eq!(only_bounds(harness.state()).min.z, 39.0);
}

#[test]
fn several_objects_show_a_summary_in_the_chosen_unit() {
    let mut wb = workbench_with_boxes(2);
    wb.set_length_unit(halberd_config::LengthUnit::Metres);
    let mut harness = harness_for(wb);
    harness.run();
    assert!(harness.query_by_label("2 objects selected").is_some());
    assert!(
        harness
            .query_by_label("Size: 4.17 m × 1.63 m × 1.63 m")
            .is_some()
    );
    assert!(
        harness.query_by_label("Width").is_none(),
        "no fields for several"
    );
}

#[test]
fn view_menu_sets_the_unit_and_the_player_figure() {
    let mut harness = harness_for(Workbench::new(info(), None));
    harness.run();
    harness.get_by_label("View").click();
    harness.run();
    harness.get_by_label("Metres").click();
    harness.run();
    assert_eq!(
        harness.state().length_unit(),
        halberd_config::LengthUnit::Metres
    );
    assert!(harness.state().show_player());
    harness.get_by_label("View").click();
    harness.run();
    harness.get_by_label(PLAYER_TOGGLE_LABEL).click();
    harness.run();
    assert!(!harness.state().show_player());
}

#[test]
fn an_unknown_unit_is_shown_as_hammer_units() {
    let wb = Workbench::new(info(), None).with_display(halberd_config::LengthUnit::Unknown, true);
    assert_eq!(wb.length_unit(), halberd_config::LengthUnit::Units);
}

#[test]
fn a_gizmo_drag_in_the_window_is_one_undo_step() {
    // The workbench ends the undo step whenever no drag is in progress; a
    // gizmo drag across many frames must still be one step.
    use halberd_tools::{Axis, GizmoMode, Handle};
    let mut wb = workbench_with_boxes(1);
    wb.viewport_mut()
        .tools_mut()
        .set_gizmo_mode(Some(GizmoMode::Move));
    let mut harness = harness_for(wb);
    harness.run();
    let rect = harness.get_by_label(crate::VIEWPORT_LABEL).rect();
    let grab = {
        let wb = harness.state();
        let camera = *wb.viewport().controller().camera();
        let size = glam::Vec2::new(rect.width(), rect.height());
        let shapes = wb
            .viewport()
            .tools()
            .gizmo_shapes(&camera, size, wb.document());
        let arrow = shapes
            .iter()
            .find(|s| s.handle == Handle::MoveAxis(Axis::Y) && !s.filled)
            .unwrap();
        let p = (arrow.points[0] + arrow.points[1]) * 0.5;
        rect.min + egui::vec2(p.x, p.y)
    };
    let before = only_bounds(harness.state());
    harness.hover_at(grab);
    harness.event(egui::Event::PointerButton {
        pos: grab,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    harness.run();
    for step in 1..=6 {
        harness.hover_at(grab + egui::vec2(-step as f32 * 10.0, -step as f32 * 6.0));
        harness.run();
    }
    harness.event(egui::Event::PointerButton {
        pos: grab + egui::vec2(-60.0, -36.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    harness.run();
    assert_ne!(only_bounds(harness.state()), before, "it moved");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert_eq!(
        only_bounds(harness.state()),
        before,
        "one Ctrl+Z undoes it all"
    );
}

#[test]
fn delete_and_undo_wait_while_a_box_is_being_drawn() {
    // Regression: Delete or Ctrl+Z mid-drag pulled the map from under it.
    let mut harness = harness_for(workbench_with_boxes(1));
    harness
        .state_mut()
        .viewport_mut()
        .set_tool(halberd_tools::Tool::Box);
    harness.run();
    let rect = harness.get_by_label(crate::VIEWPORT_LABEL).rect();
    let at = rect.center() + egui::vec2(0.0, 150.0);
    harness.hover_at(at);
    harness.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    harness.run();
    harness.hover_at(at + egui::vec2(80.0, 20.0));
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    harness.key_press(Key::Delete);
    harness.run();
    assert_eq!(
        harness.state().document().len(),
        1,
        "nothing undone or deleted"
    );
    harness.event(egui::Event::PointerButton {
        pos: at + egui::vec2(80.0, 20.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    harness.run();
    assert_eq!(harness.state().document().len(), 2, "the box was drawn");
}

#[test]
fn delete_waits_in_vertex_edge_and_face_modes() {
    let mut harness = harness_for(workbench_with_boxes(1));
    harness.run();
    harness.key_press(Key::Num2);
    harness.run();
    harness.key_press(Key::Delete);
    harness.run();
    assert_eq!(harness.state().document().len(), 1, "the brush is kept");
    let warned = harness
        .state()
        .console()
        .entries()
        .any(|e| e.message.contains("Deleting corners, edges or faces"));
    assert!(warned, "the Console says why");
    harness.key_press(Key::Num1);
    harness.run();
    harness.key_press(Key::Delete);
    harness.run();
    assert!(harness.state().document().is_empty(), "Object mode deletes");
}
