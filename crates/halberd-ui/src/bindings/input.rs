//! Reading the keys in effect from egui's input.

use super::CTRL_SHIFT;
use egui::{Event, InputState, Key, KeyboardShortcut};

/// The Copy or Cut event the window system sends instead of `key`, if it
/// does: Ctrl+Shift+C arrives as Copy and Ctrl+Shift+X as Cut, with no key.
fn clipboard_event(key: KeyboardShortcut) -> Option<Event> {
    if !key.modifiers.matches_exact(CTRL_SHIFT) {
        return None;
    }
    match key.logical_key {
        Key::C => Some(Event::Copy),
        Key::X => Some(Event::Cut),
        _ => None,
    }
}

/// True if this frame's input holds `key` sent as a Copy or Cut.
fn arrived_as_clipboard(input: &InputState, key: KeyboardShortcut) -> bool {
    clipboard_event(key)
        .is_some_and(|e| input.modifiers.matches_exact(key.modifiers) && input.events.contains(&e))
}

/// True if `key` went down this frame (not a repeat) with exactly its
/// modifiers.
pub(crate) fn pressed(input: &InputState, key: KeyboardShortcut) -> bool {
    arrived_as_clipboard(input, key)
        || input.events.iter().any(|e| {
            matches!(
                e,
                Event::Key { key: k, pressed: true, repeat: false, modifiers, .. }
                    if *k == key.logical_key && modifiers.matches_exact(key.modifiers)
            )
        })
}

/// True if `key` was pressed this frame with exactly its modifiers
/// (repeats count, as holding Ctrl+Z keeps undoing); the presses are taken
/// out so nothing else acts on them.
pub(crate) fn consume(input: &mut InputState, key: KeyboardShortcut) -> bool {
    let clipboard = arrived_as_clipboard(input, key)
        .then(|| clipboard_event(key))
        .flatten();
    let before = input.events.len();
    input.events.retain(|e| {
        let is_key = matches!(
            e,
            Event::Key { key: k, pressed: true, modifiers, .. }
                if *k == key.logical_key && modifiers.matches_exact(key.modifiers)
        );
        !is_key && clipboard.as_ref() != Some(e)
    });
    input.events.len() != before
}

/// True if `key` may act while a text box has the keyboard: it uses Ctrl
/// or Alt, or is a function key. A plain letter there is typing.
pub(crate) fn usable_while_typing(key: KeyboardShortcut) -> bool {
    let m = key.modifiers;
    m.command || m.ctrl || m.alt || is_function_key(key.logical_key)
}

/// F1 to F35.
fn is_function_key(key: Key) -> bool {
    let name = key.name();
    name.len() > 1 && name.starts_with('F') && name[1..].chars().all(|c| c.is_ascii_digit())
}

/// The keys of `key` as the keyboard viewer draws them, modifiers first.
pub(crate) fn caps(key: KeyboardShortcut) -> Vec<crate::KeyCap> {
    use crate::KeyCap;
    let mut caps = Vec::new();
    if key.modifiers.command || key.modifiers.ctrl {
        caps.push(KeyCap::Ctrl);
    }
    if key.modifiers.shift {
        caps.push(KeyCap::Shift);
    }
    if key.modifiers.alt {
        caps.push(KeyCap::Alt);
    }
    caps.push(KeyCap::Key(key.logical_key));
    caps
}
