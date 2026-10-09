//! Tests for the Keybinds window, run through the real interface.

use super::tests::{harness_for, info};
use super::*;
use crate::{KEYBINDS_SEARCH_NAME, KEYBINDS_TITLE};
use egui_kittest::kittest::Queryable;

#[test]
fn the_keybinds_button_sits_between_view_and_help_and_opens_the_window() {
    let mut h = harness_for(Workbench::new(info(), None));
    h.run();
    let x = |label: &str| h.get_by_label(label).rect().min.x;
    assert!(x("View") < x(KEYBINDS_TITLE) && x(KEYBINDS_TITLE) < x("Help"));
    assert!(h.query_by_label("Key S").is_none());
    h.get_by_label(KEYBINDS_TITLE).click();
    h.run();
    assert!(h.query_by_label("Key S").is_some(), "the keyboard is drawn");
    assert!(h.query_by_label("Key Space").is_some());
    // The menu bar button (the highest of the two "Keybinds", the other
    // being the window's title) closes it again.
    h.get_all_by_label(KEYBINDS_TITLE)
        .min_by(|a, b| a.rect().min.y.total_cmp(&b.rect().min.y))
        .unwrap()
        .click();
    h.run();
    assert!(h.query_by_label("Key S").is_none());
}

#[test]
fn clicking_a_key_lists_what_it_does() {
    let mut h = harness_for(Workbench::new(info(), None));
    h.run();
    h.get_by_label(KEYBINDS_TITLE).click();
    h.run();
    h.get_by_label("Key B").click();
    h.run();
    assert!(
        h.query_by_label("Draw tool; again: open or close the shape list")
            .is_some()
    );
    h.get_by_label("Key R").click();
    h.run();
    assert!(h.query_by_label("Rotate gizmo (again: off)").is_some());
    assert!(
        h.query_by_label("Turn the shape a quarter turn clockwise")
            .is_some(),
        "and what it does while drawing"
    );
}

#[test]
fn searching_lists_the_matching_keys_without_moving_the_keyboard() {
    let mut h = harness_for(Workbench::new(info(), None));
    h.run();
    h.get_by_label(KEYBINDS_TITLE).click();
    h.run();
    let esc = h.get_by_label("Key Esc").rect();
    h.get_by_label(KEYBINDS_SEARCH_NAME).click();
    h.run();
    h.get_by_label(KEYBINDS_SEARCH_NAME).type_text("fly");
    h.run();
    assert!(h.query_by_label("Fly forward").is_some());
    assert!(h.query_by_label("Fly faster").is_some());
    assert!(h.query_by_label("Save").is_none(), "only what matches");
    // Regression: a long result list grew the window off the top of the
    // screen, and clicking a key made the keyboard jump.
    assert_eq!(
        h.get_by_label("Key Esc").rect(),
        esc,
        "the keyboard stays put"
    );
    assert!(esc.min.y >= 0.0);
}

#[test]
fn keys_typed_in_the_search_box_stay_there() {
    // Typing tool keys, Delete, Ctrl+Z or Escape in the search box must not
    // change the map, the selection or the tools. Regression: the Escape
    // that leaves the box also deselected.
    let mut wb = super::tests::workbench_with_boxes(1);
    wb.document_mut().end_step();
    let mut h = harness_for(wb);
    h.run();
    h.get_by_label(KEYBINDS_TITLE).click();
    h.run();
    h.get_by_label(KEYBINDS_SEARCH_NAME).click();
    h.run();
    h.get_by_label(KEYBINDS_SEARCH_NAME).type_text("wqbrst");
    h.run();
    for key in [egui::Key::Delete, egui::Key::Backspace] {
        h.key_press(key);
        h.run();
    }
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::W);
    h.run();
    h.key_press(egui::Key::Escape);
    h.run();
    let wb = h.state();
    assert_eq!(wb.document().len(), 1);
    assert_eq!(wb.document().selection().len(), 1, "still selected");
    assert_eq!(wb.viewport().tools().gizmo_mode(), None);
    assert_eq!(wb.viewport().tool(), halberd_tools::Tool::Select);
    assert!(!wb.viewport().tools().inside_entities());
}
