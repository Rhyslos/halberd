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
    assert_eq!(
        found("vertex"),
        ["Vertex mode: pick corners of the selected brushes"]
    );
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
