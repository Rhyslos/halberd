//! Tests for the Keybinds window.

use super::*;
use crate::keymap::KEYBINDS;

#[test]
fn every_bound_key_is_on_the_drawn_keyboard() {
    let drawn = drawn_keys();
    for bind in KEYBINDS {
        for key in bind.keys.iter() {
            assert!(drawn.contains(key), "{} ({})", key.name(), bind.action);
        }
    }
}

#[test]
fn the_search_finds_the_keys_to_light() {
    let viewer = KeybindsViewer {
        search: "undo".into(),
        ..KeybindsViewer::default()
    };
    let found = viewer.found();
    assert_eq!(found.len(), 1);
    assert!(found[0].keys.contains(&KeyCap::Key(Key::Z)));
    assert!(found[0].keys.contains(&KeyCap::Ctrl));
}
