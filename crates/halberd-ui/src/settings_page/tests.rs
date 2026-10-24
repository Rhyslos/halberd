//! Tests for the Settings window.

use super::*;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

struct State {
    page: SettingsPage,
    settings: Settings,
    bindings: Bindings,
}

fn harness() -> Harness<'static, State> {
    Harness::builder().with_size([800.0, 700.0]).build_ui_state(
        |ui, s: &mut State| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                s.page.show(ui, &mut s.settings, &mut s.bindings);
            });
        },
        State {
            page: SettingsPage::default(),
            settings: Settings::default(),
            bindings: Bindings::default(),
        },
    )
}

fn shortcut(modifiers: Modifiers, key: Key) -> Option<KeyboardShortcut> {
    Some(KeyboardShortcut::new(modifiers, key))
}

#[test]
fn clicking_a_key_then_pressing_one_changes_it() {
    let mut h = harness();
    h.state_mut().page.filter = "extrude".into();
    h.run();
    h.get_by_label(&key_button_name(Command::Extrude)).click();
    h.run();
    assert!(h.state().page.is_capturing());
    assert!(h.query_by_label_contains("Press the new key").is_some());
    h.key_press(Key::G);
    h.run();
    assert!(!h.state().page.is_capturing());
    assert_eq!(
        h.state().bindings.key(Command::Extrude),
        shortcut(Modifiers::NONE, Key::G)
    );
    // With modifiers too. Regression: the real window sends a press of
    // Ctrl itself first, which must not become the key.
    h.state_mut().page.filter = String::new();
    h.run();
    h.get_by_label(&key_button_name(Command::Save)).click();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::ControlLeft);
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, Key::S);
    h.run();
    assert_eq!(
        h.state().bindings.key(Command::Save),
        shortcut(Modifiers::COMMAND | Modifiers::ALT, Key::S)
    );
}

#[test]
fn escape_cancels_and_reserved_keys_are_refused() {
    let mut h = harness();
    h.run();
    h.get_by_label(&key_button_name(Command::Undo)).click();
    h.run();
    h.key_press(Key::Escape);
    h.run();
    assert!(!h.state().page.is_capturing());
    assert_eq!(h.state().bindings, Bindings::default());
    h.get_by_label(&key_button_name(Command::Undo)).click();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(h.state().bindings, Bindings::default());
    assert!(h.query_by_label_contains("kept for finishing").is_some());
    h.state_mut().page.filter = "fly up".into();
    h.run();
    h.get_by_label(&key_button_name(Command::FlyUp)).click();
    h.run();
    h.key_press_modifiers(Modifiers::SHIFT, Key::E);
    h.run();
    assert!(h.query_by_label_contains("without Ctrl").is_some());
    assert_eq!(h.state().bindings, Bindings::default());
}

#[test]
fn ctrl_shift_c_arriving_as_copy_is_taken() {
    // The window system sends Ctrl+Shift+C as Copy, with no key.
    let mut h = harness();
    h.state_mut().page.capturing = Some(Command::Intersect);
    h.run();
    h.event(egui::Event::ModifiersChanged(
        Modifiers::COMMAND | Modifiers::SHIFT,
    ));
    h.event(egui::Event::Copy);
    h.run();
    assert_eq!(
        h.state().bindings.key(Command::Intersect),
        shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::C)
    );
    // Plain Ctrl+C stays copy: still waiting for a key.
    h.state_mut().page.capturing = Some(Command::Union);
    h.event(egui::Event::ModifiersChanged(Modifiers::COMMAND));
    h.event(egui::Event::Copy);
    h.run();
    assert!(h.state().page.is_capturing());
    assert!(!h.state().bindings.is_changed(Command::Union));
}

#[test]
fn shared_keys_are_pointed_out_and_everything_can_be_reset() {
    let mut h = harness();
    h.state_mut()
        .bindings
        .set(Command::Bridge, shortcut(Modifiers::NONE, Key::E));
    h.run();
    assert!(h.query_by_label("Also: Bridge").is_some());
    assert!(h.query_by_label("Also: Extrude faces").is_some());
    h.get_by_label(RESET_KEYS_LABEL).click();
    h.run();
    assert_eq!(h.state().bindings, Bindings::default());
    assert!(h.query_by_label("Also: Bridge").is_none());
}

#[test]
fn clear_and_reset_one_key() {
    let mut h = harness();
    h.state_mut()
        .bindings
        .set(Command::Split, shortcut(Modifiers::NONE, Key::G));
    h.state_mut().page.filter = "split".into();
    h.run();
    h.get_by_label("Reset").click();
    h.run();
    assert!(!h.state().bindings.is_changed(Command::Split));
    h.get_by_label("Clear").click();
    h.run();
    assert_eq!(h.state().bindings.key(Command::Split), None);
}

#[test]
fn the_filter_narrows_the_list() {
    let mut h = harness();
    h.state_mut().page.filter = "gizmo".into();
    h.run();
    assert!(
        h.query_by_label(&key_button_name(Command::MoveGizmo))
            .is_some()
    );
    assert!(h.query_by_label(&key_button_name(Command::Save)).is_none());
}

#[test]
fn the_camera_tab_sets_wasd_and_speeds() {
    let mut h = harness();
    h.run();
    h.get_by_label("Camera").click();
    h.run();
    h.get_by_label(WASD_LABEL).click();
    h.run();
    assert_eq!(h.state().settings.editor.wasd_movement, Some(false));
    h.state_mut().settings.camera.turn_speed_percent = 200;
    h.run();
    h.get_by_label("Reset camera speeds").click();
    h.run();
    assert_eq!(h.state().settings.camera, CameraSettings::default());
}

#[test]
fn the_performance_tab_says_what_the_budget_keeps() {
    let mut h = harness();
    h.state_mut().page.tab = SettingsTab::Performance;
    h.state_mut().settings.performance.memory_budget_mb = 1024;
    h.run();
    assert!(
        h.query_by_label_contains("Keeps up to 300 pictures")
            .is_some()
    );
    assert!(h.query_by_label_contains("128 material textures").is_some());
    assert!(
        h.query_by_label_contains("next time Halberd starts")
            .is_some()
    );
}
