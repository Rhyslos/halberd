//! Every key Halberd responds to, in one list, for the Keybinds viewer.
//!
//! The list describes the keys the code reads; tests check that the menu
//! shortcuts in it match the ones the code uses, so the viewer cannot
//! drift from what the keys really do.

use crate::{Bindings, Command};
use KeyCap::Shift;
use egui::{Key, KeyboardShortcut, Modifiers};
use std::borrow::Cow;

/// A key on the keyboard as the viewer draws it: an ordinary key, or one
/// of the modifier keys (which egui does not count as keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCap {
    /// An ordinary key.
    Key(Key),
    /// Either Shift key.
    Shift,
    /// Either Ctrl key (Cmd on macOS).
    Ctrl,
    /// Either Alt key (Option on macOS).
    Alt,
}

impl KeyCap {
    /// The key's name, as written on it and in searches ("Ctrl", "S", "Esc").
    pub fn name(self) -> &'static str {
        match self {
            Self::Shift => "Shift",
            Self::Ctrl => {
                if cfg!(target_os = "macos") {
                    "Cmd"
                } else {
                    "Ctrl"
                }
            }
            Self::Alt => {
                if cfg!(target_os = "macos") {
                    "Option"
                } else {
                    "Alt"
                }
            }
            Self::Key(key) => match key {
                Key::Escape => "Esc",
                Key::Backspace => "Backspace",
                Key::Delete => "Delete",
                Key::Enter => "Enter",
                Key::Space => "Space",
                other => other.symbol_or_name(),
            },
        }
    }
}

/// When a key does what it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyContext {
    /// Anywhere in the window. (Undo, redo and Delete wait while typing in
    /// a text box, which has its own.)
    Anywhere,
    /// In the 3D viewport.
    Viewport,
    /// While drawing a shape (left button held with the Draw tool).
    Drawing,
    /// While the Draw tool's shape list is open.
    ShapeList,
    /// While holding the right mouse button in the viewport, any camera.
    RightMouse,
    /// In the Fly camera, while holding the right mouse button.
    Flying,
    /// While the "Save changes?" question is showing.
    SaveQuestion,
    /// While a problem is shown in a message box.
    MessageBox,
}

impl KeyContext {
    /// Plain-language name, shown next to each action.
    pub fn label(self) -> &'static str {
        match self {
            Self::Anywhere => "Anywhere",
            Self::Viewport => "Viewport",
            Self::Drawing => "While drawing",
            Self::ShapeList => "Shape list open",
            Self::RightMouse => "Holding right mouse",
            Self::Flying => "Fly camera, holding right mouse",
            Self::SaveQuestion => "\"Save changes?\" showing",
            Self::MessageBox => "Message box showing",
        }
    }
}

/// One thing a key (or key combination) does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keybind {
    /// The keys held together, modifiers first.
    pub keys: Cow<'static, [KeyCap]>,
    /// The command whose key this is, when Settings can change it.
    pub command: Option<Command>,
    /// What happens, in plain words.
    pub action: &'static str,
    /// When it applies.
    pub context: KeyContext,
}

impl Keybind {
    /// The combination as text, such as "Ctrl + Shift + S".
    pub fn chord(&self) -> String {
        self.keys
            .iter()
            .map(|k| k.name())
            .collect::<Vec<_>>()
            .join(" + ")
    }

    /// The combination as an egui shortcut, if it is one ordinary key with
    /// modifiers.
    pub fn shortcut(&self) -> Option<KeyboardShortcut> {
        let mut modifiers = Modifiers::NONE;
        let mut key = None;
        for cap in self.keys.iter() {
            match cap {
                KeyCap::Shift => modifiers.shift = true,
                KeyCap::Ctrl => modifiers |= Modifiers::COMMAND,
                KeyCap::Alt => modifiers.alt = true,
                KeyCap::Key(k) if key.is_none() => key = Some(*k),
                KeyCap::Key(_) => return None,
            }
        }
        Some(KeyboardShortcut::new(modifiers, key?))
    }

    /// True if `query` (any letter case) finds this binding:
    ///
    /// - a key's name ("s", "shift", "esc") finds the bindings using that
    ///   key, and only those, so "w" answers "what does W do?";
    /// - a combination ("ctrl+s", spaces ignored) finds exactly it;
    /// - other words find actions and situations where words start with
    ///   them ("save", "while draw").
    ///
    /// "ctrl", "cmd", "control" and "command" all mean the Ctrl key (Cmd on
    /// macOS), and "alt" and "option" the Alt key.
    pub fn matches(&self, query: &str) -> bool {
        let query = normalise(query);
        if query.is_empty() {
            return false;
        }
        let squeezed: String = query.chars().filter(|c| !c.is_whitespace()).collect();
        if is_key_name(&squeezed) {
            return self.keys.iter().any(|k| normalise(k.name()) == squeezed);
        }
        if squeezed.contains('+') {
            let chord: String = normalise(&self.chord())
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            return chord == squeezed;
        }
        let text = format!("{} {}", self.action, self.context.label()).to_lowercase();
        starts_words(&text, &query)
    }
}

/// `text` in lower case, with every name for Ctrl and Alt made the same.
fn normalise(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .split('+')
        .map(|part| match part.trim() {
            "ctrl" | "cmd" | "control" | "command" => "ctrl".to_string(),
            "alt" | "option" | "opt" => "alt".to_string(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// True if `query` is the name of a key on the drawn keyboard (any single
/// character counts, so typing a letter asks about that key).
fn is_key_name(query: &str) -> bool {
    query.chars().count() == 1
        || crate::drawn_keys()
            .iter()
            .chain(KEYBINDS.iter().flat_map(|b| b.keys.iter()))
            .any(|k| normalise(k.name()) == query)
}

/// True if the words of `query` start consecutive words of `text`.
fn starts_words(text: &str, query: &str) -> bool {
    let words: Vec<&str> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let wanted: Vec<&str> = query.split_whitespace().collect();
    (0..words.len()).any(|start| {
        wanted.len() <= words.len() - start
            && wanted
                .iter()
                .zip(&words[start..])
                .all(|(want, word)| word.starts_with(want))
    })
}

mod list;
pub use list::KEYBINDS;

/// Every key binding with the keys in effect: changed keys in place of the
/// defaults, and commands without a key left out. While holding right
/// mouse, a gizmo whose plain key also flies is picked with Shift; that
/// line is shown only while it applies.
pub fn current_binds(bindings: &Bindings) -> Vec<Keybind> {
    KEYBINDS
        .iter()
        .filter_map(|bind| {
            let Some(command) = bind.command else {
                return Some(bind.clone());
            };
            let key = bindings.key(command)?;
            let keys = if bind.context == KeyContext::RightMouse {
                let flies = bindings.key_down_commands().any(|f| {
                    bindings
                        .key(f)
                        .is_some_and(|b| b.logical_key == key.logical_key)
                });
                if !key.modifiers.is_none() || !flies {
                    return None;
                }
                vec![Shift, KeyCap::Key(key.logical_key)]
            } else {
                crate::bindings::caps(key)
            };
            Some(Keybind {
                keys: Cow::Owned(keys),
                ..bind.clone()
            })
        })
        .collect()
}

/// What the mouse does, for the viewer's list under the keyboard.
pub fn mouse_binds() -> [(&'static str, String); 8] {
    [
        (
            "Left click",
            format!(
                "Select, or pick a corner, edge or face in modes 2 to 4 ({}+click adds or removes)",
                KeyCap::Ctrl.name()
            ),
        ),
        (
            "Left drag",
            "Draw a shape (Draw tool), drag a gizmo handle, or in modes 2 to 4 draw a box \
             that picks every part inside it"
                .into(),
        ),
        (
            "Right drag",
            "Look around or orbit, depending on the camera mode".into(),
        ),
        ("Middle drag", "Pan".into()),
        ("Wheel", "Zoom".into()),
        (
            "Wheel",
            "Fly speed (Fly camera, holding right mouse)".into(),
        ),
        (
            "Right or middle drag",
            "In a side view (Top, Front, Right…): move the view".into(),
        ),
        (
            "Wheel",
            "In a side view: zoom in and out around the pointer".into(),
        ),
    ]
}

/// The bindings in `binds` that involve `cap`.
pub fn binds_with(binds: &[Keybind], cap: KeyCap) -> impl Iterator<Item = &Keybind> {
    binds.iter().filter(move |b| b.keys.contains(&cap))
}

#[cfg(test)]
mod tests;
