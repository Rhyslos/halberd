//! Tests for changeable key bindings.

use super::*;

fn key(modifiers: Modifiers, key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(modifiers, key)
}

#[test]
fn command_names_are_unique_and_found_again() {
    let mut seen = std::collections::HashSet::new();
    for command in Command::ALL {
        assert!(seen.insert(command.id()), "{} twice", command.id());
        assert_eq!(Command::from_id(command.id()), Some(*command));
    }
    assert_eq!(Command::from_id("file.nonsense"), None);
}

#[test]
fn default_keys_never_clash_and_are_allowed() {
    let bindings = Bindings::default();
    for command in Command::ALL {
        assert!(
            bindings.clashes(*command).is_empty(),
            "{:?} clashes with {:?}",
            command,
            bindings.clashes(*command)
        );
        if let Some(k) = command.default_key() {
            assert_eq!(check_binding(*command, k), Ok(()), "{command:?}");
        }
    }
}

#[test]
fn key_text_round_trips_for_every_key() {
    for k in Key::ALL {
        for modifiers in [NONE, CTRL, CTRL_SHIFT, Modifiers::ALT] {
            let chord = Some(key(modifiers, *k));
            let text = chord_text(chord);
            assert_eq!(parse_chord(&text), Ok(chord), "{text}");
        }
    }
    assert_eq!(parse_chord(""), Ok(None));
    assert_eq!(chord_text(None), "");
}

#[test]
fn key_text_is_forgiving_about_case_and_names() {
    assert_eq!(
        parse_chord("cmd + shift + s"),
        Ok(Some(key(CTRL_SHIFT, Key::S)))
    );
    assert_eq!(parse_chord("f9"), Ok(Some(key(NONE, Key::F9))));
    assert_eq!(parse_chord("Ctrl++"), Ok(Some(key(CTRL, Key::Plus))));
    assert_eq!(parse_chord("Ctrl"), Err(NotAKey), "no key");
    assert_eq!(parse_chord("A+B"), Err(NotAKey), "two keys");
    assert_eq!(parse_chord("Hyper+Q"), Err(NotAKey));
    assert_eq!(parse_chord(&"Shift+".repeat(100)), Err(NotAKey), "too long");
}

#[test]
fn changes_load_from_and_save_to_the_settings_file() {
    let mut saved = BTreeMap::new();
    saved.insert("file.save".to_string(), "Ctrl+Alt+S".to_string());
    saved.insert("tool.select".to_string(), String::new());
    let (bindings, notes) = Bindings::from_settings(&saved);
    assert!(notes.is_empty(), "{notes:?}");
    let ctrl_alt = Modifiers { alt: true, ..CTRL };
    assert_eq!(bindings.key(Command::Save), Some(key(ctrl_alt, Key::S)));
    assert_eq!(bindings.key(Command::SelectTool), None);
    assert_eq!(bindings.key(Command::Open), Command::Open.default_key());
    assert_eq!(bindings.to_settings(), saved);
}

#[test]
fn choosing_the_default_again_removes_the_change() {
    let mut bindings = Bindings::default();
    bindings.set(Command::Extrude, Some(key(NONE, Key::G)));
    assert!(bindings.is_changed(Command::Extrude));
    bindings.set(Command::Extrude, Command::Extrude.default_key());
    assert!(!bindings.is_changed(Command::Extrude));
    assert!(bindings.to_settings().is_empty());
    bindings.set(Command::Bridge, None);
    bindings.reset_all();
    assert_eq!(bindings, Bindings::default());
}

#[test]
fn bad_entries_are_explained_and_left_out() {
    let mut saved = BTreeMap::new();
    saved.insert("tool.teleport".to_string(), "G".to_string());
    saved.insert("file.open".to_string(), "Ctrl+Nonsense".to_string());
    saved.insert("edit.undo".to_string(), "Escape".to_string());
    saved.insert("fly.up".to_string(), "Ctrl+E".to_string());
    let (bindings, notes) = Bindings::from_settings(&saved);
    assert_eq!(notes.len(), 4, "{notes:?}");
    assert!(notes.iter().any(|n| n.contains("tool.teleport")));
    assert!(notes.iter().any(|n| n.contains("kept for cancelling")));
    assert!(notes.iter().any(|n| n.contains("without Ctrl")));
    for command in Command::ALL {
        assert_eq!(bindings.key(*command), command.default_key(), "{command:?}");
    }
}

#[test]
fn too_many_entries_are_capped() {
    let saved: BTreeMap<String, String> = (0..MAX_KEY_OVERRIDES + 10)
        .map(|i| (format!("made.up.{i:04}"), "G".to_string()))
        .collect();
    let (_, notes) = Bindings::from_settings(&saved);
    assert_eq!(notes.len(), MAX_KEY_OVERRIDES + 1);
    assert!(notes.last().is_some_and(|n| n.contains("more than")));
}

#[test]
fn clashes_count_only_where_both_keys_work() {
    let mut bindings = Bindings::default();
    // G for Extrude and for Bridge: both in the viewport.
    bindings.set(Command::Extrude, Some(key(NONE, Key::G)));
    bindings.set(Command::Bridge, Some(key(NONE, Key::G)));
    assert_eq!(bindings.clashes(Command::Extrude), [Command::Bridge]);
    // Ctrl+S for Save (anywhere) clashes with a viewport command on it.
    bindings.set(Command::Split, Some(key(CTRL, Key::S)));
    assert_eq!(bindings.clashes(Command::Split), [Command::Save]);
    // Flying keys live beside the viewport's: W flies and picks Move.
    assert!(bindings.clashes(Command::FlyForward).is_empty());
    // While drawing, the viewport's keys wait: R turns, R rotates.
    assert!(bindings.clashes(Command::TurnShape).is_empty());
    assert_eq!(
        bindings.command_for(BindingGroup::Viewport, key(NONE, Key::R)),
        Some(Command::RotateGizmo)
    );
}

#[test]
fn reserved_keys_are_refused() {
    let refuse = |c, k| check_binding(c, k).is_err();
    assert!(refuse(Command::Save, key(NONE, Key::Escape)));
    assert!(refuse(Command::Save, key(CTRL, Key::Escape)));
    assert!(refuse(Command::Save, key(NONE, Key::Enter)));
    assert!(refuse(Command::Save, key(CTRL, Key::C)));
    assert!(refuse(Command::Save, key(CTRL, Key::V)));
    assert!(refuse(Command::FlyUp, key(Modifiers::SHIFT, Key::E)));
    assert!(refuse(Command::Save, key(NONE, Key::ControlLeft)));
    assert!(refuse(Command::Save, key(CTRL, Key::ShiftRight)));
    assert!(!refuse(Command::Save, key(CTRL, Key::Enter)));
    assert!(!refuse(Command::Subtract, key(CTRL_SHIFT, Key::C)));
}

#[test]
fn fuzz_odd_key_text_never_crashes() {
    let mut seed: u64 = 0xB1D5;
    let alphabet = b"CtrlShiftAlt+ F19aZ-=.Esc\x01";
    for _ in 0..5000 {
        let text: String = (0..12)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                alphabet[(seed % alphabet.len() as u64) as usize] as char
            })
            .collect();
        if let Ok(chord) = parse_chord(&text) {
            assert_eq!(parse_chord(&chord_text(chord)), Ok(chord), "{text:?}");
        }
    }
}

#[test]
fn changes_for_commands_this_halberd_lacks_are_kept() {
    // Regression: an older Halberd dropped a newer one's key changes when
    // it saved.
    let mut saved = BTreeMap::new();
    saved.insert("future.command".to_string(), "Ctrl+G".to_string());
    saved.insert("file.save".to_string(), "F5".to_string());
    let (bindings, notes) = Bindings::from_settings(&saved);
    assert!(notes[0].contains("kept"));
    assert_eq!(bindings.to_settings(), saved);
}

#[test]
fn clipboard_keys_are_refused_unless_they_can_be_told_apart() {
    let ctrl_alt = Modifiers { alt: true, ..CTRL };
    let refuse = |k| check_binding(Command::Save, k).is_err();
    assert!(refuse(key(ctrl_alt, Key::C)));
    assert!(refuse(key(ctrl_alt, Key::X)));
    assert!(refuse(key(CTRL_SHIFT, Key::V)));
    assert!(!refuse(key(CTRL_SHIFT, Key::C)));
    assert!(!refuse(key(CTRL_SHIFT, Key::X)));
    assert!(!refuse(key(Modifiers::SHIFT, Key::C)));
}

#[test]
fn double_actions_while_drawing_and_flying_are_pointed_out() {
    let mut bindings = Bindings::default();
    // Turning a shape with Q also cancels it and picks Select.
    bindings.set(Command::TurnShape, Some(key(NONE, Key::Q)));
    assert_eq!(bindings.clashes(Command::TurnShape), [Command::SelectTool]);
    // Flying up with E also extrudes.
    bindings.set(Command::FlyUp, Some(key(NONE, Key::E)));
    assert_eq!(bindings.clashes(Command::FlyUp), [Command::Extrude]);
    // But W flies and (with Shift) picks Move, by design.
    assert!(bindings.clashes(Command::FlyForward).is_empty());
}

#[test]
fn plain_keys_wait_while_typing_but_ctrl_and_function_keys_dont() {
    assert!(!usable_while_typing(key(NONE, Key::N)));
    assert!(!usable_while_typing(key(Modifiers::SHIFT, Key::N)));
    assert!(usable_while_typing(key(CTRL, Key::N)));
    assert!(usable_while_typing(key(NONE, Key::F9)));
    assert!(!usable_while_typing(key(NONE, Key::F)));
}
