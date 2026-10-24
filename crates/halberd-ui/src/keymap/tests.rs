//! Tests for the list of key bindings.

use super::*;
use crate::workbench::{
    NEW_SHORTCUT, OPEN_SHORTCUT, QUIT_SHORTCUT, REDO_SHORTCUT, SAVE_AS_SHORTCUT, SAVE_SHORTCUT,
    UNDO_SHORTCUT,
};

#[test]
fn the_menu_shortcuts_the_code_uses_are_listed() {
    let listed: Vec<KeyboardShortcut> = KEYBINDS.iter().filter_map(Keybind::shortcut).collect();
    for shortcut in [
        NEW_SHORTCUT,
        OPEN_SHORTCUT,
        SAVE_SHORTCUT,
        SAVE_AS_SHORTCUT,
        QUIT_SHORTCUT,
        UNDO_SHORTCUT,
        REDO_SHORTCUT,
    ] {
        assert!(listed.contains(&shortcut), "{shortcut:?} is in the list");
    }
    for (op, command) in crate::viewport::SHAPE_OP_COMMANDS {
        let shortcut = command.default_key().unwrap();
        assert!(listed.contains(&shortcut), "{op:?}'s key is in the list");
    }
}

#[test]
fn every_command_is_listed_with_its_default_keys() {
    for command in Command::ALL {
        let listed: Vec<&Keybind> = KEYBINDS
            .iter()
            .filter(|b| b.command == Some(*command) && b.context != KeyContext::RightMouse)
            .collect();
        assert!(!listed.is_empty(), "{command:?} is listed");
        let default = command.default_key().map(crate::bindings::caps);
        for bind in listed {
            assert_eq!(Some(bind.keys.to_vec()), default, "{command:?}");
        }
    }
}

#[test]
fn changed_keys_show_in_the_list_and_cleared_ones_go() {
    let mut bindings = Bindings::default();
    bindings.set(
        Command::Save,
        Some(KeyboardShortcut::new(Modifiers::NONE, Key::F5)),
    );
    bindings.set(Command::Quit, None);
    // Move picked with H: no longer a flying key, so no Shift line.
    bindings.set(
        Command::MoveGizmo,
        Some(KeyboardShortcut::new(Modifiers::NONE, Key::H)),
    );
    let binds = current_binds(&bindings);
    let save = binds
        .iter()
        .find(|b| b.command == Some(Command::Save))
        .unwrap();
    assert_eq!(save.keys.to_vec(), [KeyCap::Key(Key::F5)]);
    assert!(!binds.iter().any(|b| b.command == Some(Command::Quit)));
    let shift_lines: Vec<_> = binds
        .iter()
        .filter(|b| b.context == KeyContext::RightMouse)
        .collect();
    assert_eq!(shift_lines.len(), 1, "only Scale (S) still needs Shift");
    assert_eq!(binds_with(&binds, KeyCap::Key(Key::H)).count(), 1);
}

#[test]
fn chords_read_naturally() {
    let save_as = KEYBINDS.iter().find(|b| b.action == "Save as…").unwrap();
    let ctrl = KeyCap::Ctrl.name();
    assert_eq!(save_as.chord(), format!("{ctrl} + Shift + S"));
}

#[test]
fn searching_finds_actions_contexts_and_keys() {
    let found = |q: &str| -> Vec<&str> {
        KEYBINDS
            .iter()
            .filter(|b| b.matches(q))
            .map(|b| b.action)
            .collect()
    };
    assert_eq!(found("SAVE AS"), ["Save as…"]);
    assert!(found("save").contains(&"Save"));
    assert_eq!(found("ctrl+s"), ["Save"], "exact combination");
    assert_eq!(found("cmd + shift + s"), ["Save as…"], "Cmd means Ctrl");
    assert!(found("while draw").contains(&"Cancel the shape"));
    assert!(found("vertex").contains(&"Vertex mode: pick corners of the selected brushes"));
    assert!(found("").is_empty() && found("   ").is_empty());
    assert!(found("teleport").is_empty());
}

#[test]
fn a_single_letter_asks_about_that_key_only() {
    // Regression: "w" also matched every action or situation containing a
    // w ("Anywhere", "Draw…"), lighting up most of the keyboard.
    for letter in ["w", "q", "e", "n", "shift", "esc"] {
        for bind in KEYBINDS.iter().filter(|b| b.matches(letter)) {
            assert!(
                bind.keys
                    .iter()
                    .any(|k| k.name().eq_ignore_ascii_case(letter)),
                "{letter:?} found {:?} ({})",
                bind.action,
                bind.chord()
            );
        }
    }
    let w: Vec<_> = KEYBINDS.iter().filter(|b| b.matches("w")).collect();
    assert!(w.iter().any(|b| b.action.starts_with("Move gizmo")));
    assert!(w.iter().any(|b| b.action == "Fly forward"));
}

#[test]
fn no_combination_means_two_things_at_once() {
    // The same keys may do different things in different situations, but
    // never two things in the same one.
    for (i, a) in KEYBINDS.iter().enumerate() {
        for b in &KEYBINDS[i + 1..] {
            if a.context == b.context {
                assert!(
                    a.keys != b.keys,
                    "{} does both {:?} and {:?}",
                    a.chord(),
                    a.action,
                    b.action
                );
            }
        }
    }
}
