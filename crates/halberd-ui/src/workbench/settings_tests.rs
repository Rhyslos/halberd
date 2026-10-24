//! Tests for the settings in the workbench: keys from the settings file,
//! the Settings window, and the first-launch WASD question.

use super::file_tests::harness_with_actions;
use super::tests::info;
use super::*;
use crate::console::LogLevel;
use crate::{Command, SETTINGS_TITLE, key_button_name};
use egui_kittest::kittest::Queryable;
use halberd_config::Settings;
use halberd_tools::GizmoMode;

fn answered() -> Settings {
    let mut settings = Settings::default();
    settings.editor.wasd_movement = Some(true);
    settings
}

#[test]
fn keys_changed_in_the_settings_file_are_used() {
    let mut settings = answered();
    settings.keys.insert("file.save".into(), "F5".into());
    settings.keys.insert("gizmo.move".into(), "G".into());
    let wb = Workbench::new(info(), None).with_settings(settings);
    let mut h = harness_with_actions(wb);
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::S);
    h.run();
    assert!(h.state().1.is_empty(), "Ctrl+S no longer saves");
    h.key_press(Key::F5);
    h.run();
    assert_eq!(h.state().1, [WorkbenchAction::Save { then: None }]);
    h.key_press(Key::W);
    h.run();
    assert_eq!(h.state().0.viewport().tools().gizmo_mode(), None);
    h.key_press(Key::G);
    h.run();
    assert_eq!(
        h.state().0.viewport().tools().gizmo_mode(),
        Some(GizmoMode::Move)
    );
    // The toolbar and the File menu show the new keys.
    assert!(h.query_by_label("Move G").is_some());
}

#[test]
fn settings_come_back_as_they_were_with_changes() {
    let mut settings = answered();
    settings.keys.insert("edit.undo".into(), "Ctrl+U".into());
    settings.camera.fly_speed = 1200;
    let mut wb = Workbench::new(info(), None).with_settings(settings.clone());
    assert_eq!(wb.viewport().controller().fly_speed(), 1200.0);
    assert_eq!(wb.settings(), settings);
    wb.set_length_unit(halberd_config::LengthUnit::Feet);
    assert_eq!(
        wb.settings().editor.length_unit,
        halberd_config::LengthUnit::Feet
    );
}

#[test]
fn bad_keys_in_the_settings_file_are_explained() {
    let mut settings = answered();
    settings
        .keys
        .insert("file.save".into(), "Ctrl+Banana".into());
    let wb = Workbench::new(info(), None).with_settings(settings);
    assert!(
        wb.console()
            .entries()
            .any(|e| { e.level == LogLevel::Warning && e.message.contains("Ctrl+Banana") })
    );
    assert_eq!(
        wb.viewport().bindings().key(Command::Save),
        Command::Save.default_key()
    );
}

#[test]
fn the_settings_window_opens_from_the_menu_bar_and_changes_keys() {
    let wb = Workbench::new(info(), None).with_settings(answered());
    let mut h = harness_with_actions(wb);
    h.run();
    h.get_by_label(SETTINGS_TITLE).click();
    h.run();
    assert!(h.state().0.settings_open());
    h.get_by_label(&key_button_name(Command::Save)).click();
    h.run();
    // While waiting for the key, Ctrl+S doesn't save: it becomes the key.
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, Key::K);
    h.run();
    assert!(h.state().1.is_empty());
    let save = h.state().0.settings().keys.get("file.save").cloned();
    assert_eq!(save.as_deref(), Some("Ctrl+Alt+K"));
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, Key::K);
    h.run();
    assert_eq!(h.state().1, [WorkbenchAction::Save { then: None }]);
}

#[test]
fn turning_wasd_off_in_settings_hides_the_fly_camera() {
    let mut wb = Workbench::new(info(), None).with_settings(answered());
    assert!(
        wb.viewport()
            .controller()
            .available_modes()
            .contains(&halberd_tools::CameraMode::Fly)
    );
    let mut settings = wb.settings();
    settings.editor.wasd_movement = Some(false);
    wb = wb.with_settings(settings);
    assert!(
        !wb.viewport()
            .controller()
            .available_modes()
            .contains(&halberd_tools::CameraMode::Fly)
    );
}

#[test]
fn first_launch_asks_about_wasd_once() {
    let wb = Workbench::new(info(), None).with_settings(Settings::default());
    let mut h = harness_with_actions(wb);
    h.run();
    assert!(h.query_by_label(WASD_QUESTION_TITLE).is_some());
    h.get_by_label(WASD_NO_LABEL).click();
    h.run();
    assert!(h.query_by_label(WASD_QUESTION_TITLE).is_none());
    assert_eq!(h.state().0.settings().editor.wasd_movement, Some(false));
    assert!(
        !h.state()
            .0
            .viewport()
            .controller()
            .available_modes()
            .contains(&halberd_tools::CameraMode::Fly)
    );
    // Already answered: not asked again.
    let wb = Workbench::new(info(), None).with_settings(answered());
    let mut h = harness_with_actions(wb);
    h.run();
    assert!(h.query_by_label(WASD_QUESTION_TITLE).is_none());
}

#[test]
fn the_keybinds_viewer_shows_changed_keys() {
    let mut settings = answered();
    settings.keys.insert("file.save".into(), "F5".into());
    let wb = Workbench::new(info(), None).with_settings(settings);
    let mut wb = wb;
    wb.keybinds.search = "f5".into();
    assert!(wb.keybinds.found().iter().any(|b| b.action == "Save"));
    wb.keybinds.search = "ctrl+s".into();
    assert!(!wb.keybinds.found().iter().any(|b| b.action == "Save"));
}
