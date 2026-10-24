//! Key bindings that can be changed in Settings: every command a key
//! starts, its default key, and the keys chosen instead.
//!
//! The settings file keeps only the changes, as text ("Ctrl+Shift+S",
//! "F9", or "" for no key), by each command's stable name, so a new
//! Halberd can add commands or change defaults without losing them.
//!
//! Some keys stay fixed because they mean the same everywhere: Escape
//! (cancel, close), Enter, Backspace while drawing a floor plan, the arrow
//! keys in the shape list, Shift and Alt while drawing, and Ctrl+C, X and V
//! (the window system keeps those for copy, cut and paste).

use egui::{Key, KeyboardShortcut, Modifiers};
use std::collections::{BTreeMap, HashMap};

/// Most changed bindings read from the settings file.
pub const MAX_KEY_OVERRIDES: usize = 256;
/// Longest key text read from the settings file.
const MAX_CHORD_TEXT: usize = 48;

/// When a command's key works, which decides which keys may not be shared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingGroup {
    /// Anywhere in the window.
    Anywhere,
    /// In the viewport (not while drawing).
    Viewport,
    /// While drawing a shape.
    Drawing,
    /// Flying (Fly camera, right mouse held): plain keys only, since Shift
    /// flies faster.
    Flying,
}

impl BindingGroup {
    /// Heading in the Settings page.
    pub fn label(self) -> &'static str {
        match self {
            Self::Anywhere => "Anywhere",
            Self::Viewport => "In the viewport",
            Self::Drawing => "While drawing a shape",
            Self::Flying => "Flying (Fly camera, holding right mouse)",
        }
    }

    /// True if a key used in both groups would do two things at once.
    fn overlaps(self, other: Self) -> bool {
        use BindingGroup::{Anywhere, Drawing, Flying, Viewport};
        match (self, other) {
            // Flying keys are held down while the right button is; they
            // live beside the viewport's keys by design (W flies, Shift+W
            // picks Move), and only clash with each other.
            (Flying, Flying) => true,
            (Flying, _) | (_, Flying) => false,
            (Anywhere, _) | (_, Anywhere) => true,
            (Viewport, Viewport) | (Drawing, Drawing) => true,
            // While drawing, the viewport's keys wait.
            (Viewport, Drawing) | (Drawing, Viewport) => false,
        }
    }
}

macro_rules! commands {
    ($($variant:ident, $id:literal, $label:literal, $group:ident, $mods:expr, $key:expr;)*) => {
        /// Something a key starts, which can be given another key.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Command {
            $(
                #[doc = $label]
                $variant,
            )*
        }

        impl Command {
            /// Every command, in the order the Settings page lists them.
            pub const ALL: &'static [Command] = &[$(Command::$variant),*];

            /// The name the settings file uses (never changes).
            pub fn id(self) -> &'static str {
                match self { $(Command::$variant => $id),* }
            }

            /// What it does, in plain words.
            pub fn label(self) -> &'static str {
                match self { $(Command::$variant => $label),* }
            }

            /// When its key works.
            pub fn group(self) -> BindingGroup {
                match self { $(Command::$variant => BindingGroup::$group),* }
            }

            /// Its key unless changed.
            pub fn default_key(self) -> Option<KeyboardShortcut> {
                match self { $(Command::$variant => Some(KeyboardShortcut::new($mods, $key))),* }
            }
        }
    };
}

/// No modifiers.
const NONE: Modifiers = Modifiers::NONE;
/// Ctrl (Cmd on macOS).
const CTRL: Modifiers = Modifiers::COMMAND;
/// Ctrl and Shift (Cmd and Shift on macOS).
const CTRL_SHIFT: Modifiers = Modifiers {
    shift: true,
    ..Modifiers::COMMAND
};

commands! {
    New, "file.new", "New map", Anywhere, CTRL, Key::N;
    Open, "file.open", "Open a map", Anywhere, CTRL, Key::O;
    Save, "file.save", "Save", Anywhere, CTRL, Key::S;
    SaveAs, "file.save_as", "Save as…", Anywhere, CTRL_SHIFT, Key::S;
    Quit, "file.quit", "Quit", Anywhere, CTRL, Key::Q;
    Compile, "file.compile", "Compile and play", Anywhere, NONE, Key::F9;
    Undo, "edit.undo", "Undo", Anywhere, CTRL, Key::Z;
    Redo, "edit.redo", "Redo", Anywhere, CTRL, Key::Y;
    Delete, "edit.delete", "Delete the selection", Anywhere, NONE, Key::Delete;
    SelectTool, "tool.select", "Select tool", Viewport, NONE, Key::Q;
    DrawTool, "tool.draw", "Draw tool (again: the shape list)", Viewport, NONE, Key::B;
    MoveGizmo, "gizmo.move", "Move gizmo", Viewport, NONE, Key::W;
    RotateGizmo, "gizmo.rotate", "Rotate gizmo", Viewport, NONE, Key::R;
    ScaleGizmo, "gizmo.scale", "Scale gizmo", Viewport, NONE, Key::S;
    AllGizmos, "gizmo.all", "All gizmos at once", Viewport, NONE, Key::T;
    InsideEntities, "pick.inside_entities", "Pick brushes inside entities", Viewport, CTRL, Key::W;
    ObjectMode, "pick.objects", "Object mode", Viewport, NONE, Key::Num1;
    VertexMode, "pick.vertices", "Vertex mode", Viewport, NONE, Key::Num2;
    EdgeMode, "pick.edges", "Edge mode", Viewport, NONE, Key::Num3;
    FaceMode, "pick.faces", "Face mode", Viewport, NONE, Key::Num4;
    Extrude, "shape.extrude", "Extrude faces", Viewport, NONE, Key::E;
    Bevel, "shape.bevel", "Bevel edges", Viewport, CTRL, Key::B;
    Bridge, "shape.bridge", "Bridge", Viewport, NONE, Key::F;
    Split, "shape.split", "Split", Viewport, NONE, Key::K;
    Clip, "shape.clip", "Clip", Viewport, NONE, Key::X;
    Join, "shape.join", "Join brushes", Viewport, CTRL, Key::J;
    MergeCorners, "shape.merge_corners", "Merge corners", Viewport, NONE, Key::M;
    Mirror, "shape.mirror", "Mirror left to right", Viewport, CTRL, Key::M;
    Union, "shape.union", "Union", Viewport, CTRL_SHIFT, Key::U;
    Subtract, "shape.subtract", "Subtract", Viewport, CTRL_SHIFT, Key::C;
    Intersect, "shape.intersect", "Intersect", Viewport, CTRL_SHIFT, Key::I;
    MakeBrushes, "shape.make_brushes", "Make brushes from shape", Viewport, CTRL_SHIFT, Key::B;
    TurnShape, "draw.turn", "Turn the shape a quarter turn", Drawing, NONE, Key::R;
    FlyForward, "fly.forward", "Fly forward", Flying, NONE, Key::W;
    FlyBack, "fly.back", "Fly back", Flying, NONE, Key::S;
    FlyLeft, "fly.left", "Fly left", Flying, NONE, Key::A;
    FlyRight, "fly.right", "Fly right", Flying, NONE, Key::D;
    FlyUp, "fly.up", "Fly up", Flying, NONE, Key::Space;
    FlyDown, "fly.down", "Fly down", Flying, NONE, Key::C;
}

impl Command {
    /// The command the settings file calls `id`.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.id() == id)
    }
}

/// Why a key can't be given to a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingRefused {
    /// The key is kept for something fixed (Escape, Enter…).
    Reserved(&'static str),
    /// Flying keys can't use Ctrl, Shift or Alt.
    NeedsPlainKey,
}

impl std::fmt::Display for BindingRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reserved(why) => write!(f, "That key is kept for {why}."),
            Self::NeedsPlainKey => f.write_str(
                "Flying keys must be a single key without Ctrl, Shift or Alt (Shift flies \
                 faster).",
            ),
        }
    }
}

/// True for Shift, Ctrl, Alt and the Windows (Cmd) key themselves, which
/// are held with other keys, not used alone.
pub fn is_modifier_key(key: Key) -> bool {
    matches!(
        key,
        Key::ShiftLeft
            | Key::ShiftRight
            | Key::ControlLeft
            | Key::ControlRight
            | Key::AltLeft
            | Key::AltRight
            | Key::SuperLeft
            | Key::SuperRight
    )
}

/// Whether `key` may be given to `command`.
pub fn check_binding(command: Command, key: KeyboardShortcut) -> Result<(), BindingRefused> {
    let plain = key.modifiers.is_none();
    if is_modifier_key(key.logical_key) {
        return Err(BindingRefused::Reserved(
            "holding with other keys (choose an ordinary key)",
        ));
    }
    match key.logical_key {
        Key::Escape => return Err(BindingRefused::Reserved("cancelling and closing")),
        Key::Enter if plain => return Err(BindingRefused::Reserved("finishing and confirming")),
        Key::Backspace if plain => {
            return Err(BindingRefused::Reserved(
                "deleting and taking back a corner",
            ));
        }
        Key::ArrowUp | Key::ArrowDown if plain => {
            return Err(BindingRefused::Reserved("moving through lists"));
        }
        Key::C | Key::X | Key::V if key.modifiers.matches_exact(CTRL) => {
            return Err(BindingRefused::Reserved("copy, cut and paste"));
        }
        _ => {}
    }
    if command.group() == BindingGroup::Flying && !plain {
        return Err(BindingRefused::NeedsPlainKey);
    }
    Ok(())
}

/// The keys in effect: each command's default unless changed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Bindings {
    /// Changed commands: their key, or `None` for no key.
    changed: HashMap<Command, Option<KeyboardShortcut>>,
}

impl Bindings {
    /// The keys from the settings file's changes (command name → key
    /// text), with plain-language notes for anything that couldn't be used.
    pub fn from_settings(saved: &BTreeMap<String, String>) -> (Self, Vec<String>) {
        let mut bindings = Self::default();
        let mut notes = Vec::new();
        for (id, text) in saved.iter().take(MAX_KEY_OVERRIDES) {
            let Some(command) = Command::from_id(id) else {
                notes.push(format!(
                    "The settings file changes the key of \"{}\", which this Halberd doesn't \
                     have; it was left out.",
                    shorten(id)
                ));
                continue;
            };
            match parse_chord(text) {
                Ok(key) => match key.map(|k| check_binding(command, k)) {
                    Some(Err(why)) => notes.push(format!(
                        "\"{}\" can't be the key for {}: {why} Its usual key is used.",
                        shorten(text),
                        command.label()
                    )),
                    _ => bindings.set(command, key),
                },
                Err(NotAKey) => notes.push(format!(
                    "\"{}\" in the settings file isn't a key Halberd knows; {} keeps its usual \
                     key.",
                    shorten(text),
                    command.label()
                )),
            }
        }
        if saved.len() > MAX_KEY_OVERRIDES {
            notes.push(format!(
                "The settings file changes more than {MAX_KEY_OVERRIDES} keys; the rest were \
                 left out."
            ));
        }
        (bindings, notes)
    }

    /// The changes, for the settings file.
    pub fn to_settings(&self) -> BTreeMap<String, String> {
        self.changed
            .iter()
            .map(|(command, key)| (command.id().to_string(), chord_text(*key)))
            .collect()
    }

    /// `command`'s key, or `None` if it has none.
    pub fn key(&self, command: Command) -> Option<KeyboardShortcut> {
        match self.changed.get(&command) {
            Some(key) => *key,
            None => command.default_key(),
        }
    }

    /// Gives `command` the key `key` (`None`: no key). Choosing its default
    /// key removes the change.
    pub fn set(&mut self, command: Command, key: Option<KeyboardShortcut>) {
        if key == command.default_key() {
            self.changed.remove(&command);
        } else {
            self.changed.insert(command, key);
        }
    }

    /// True if `command` has a key other than its default.
    pub fn is_changed(&self, command: Command) -> bool {
        self.changed.contains_key(&command)
    }

    /// Every command back to its default key.
    pub fn reset_all(&mut self) {
        self.changed.clear();
    }

    /// The other commands that share `command`'s key where both work.
    pub fn clashes(&self, command: Command) -> Vec<Command> {
        let Some(key) = self.key(command) else {
            return Vec::new();
        };
        Command::ALL
            .iter()
            .copied()
            .filter(|other| {
                *other != command
                    && other.group().overlaps(command.group())
                    && self.key(*other) == Some(key)
            })
            .collect()
    }

    /// The flying commands (their keys are held, not pressed).
    pub fn key_down_commands(&self) -> impl Iterator<Item = Command> + '_ {
        Command::ALL
            .iter()
            .copied()
            .filter(|c| c.group() == BindingGroup::Flying)
    }

    /// The command whose key is `key` in `group`, if any.
    pub fn command_for(&self, group: BindingGroup, key: KeyboardShortcut) -> Option<Command> {
        Command::ALL
            .iter()
            .copied()
            .find(|c| c.group() == group && self.key(*c) == Some(key))
    }
}

/// True if `key` went down this frame (not a repeat) with exactly its
/// modifiers.
pub(crate) fn pressed(input: &egui::InputState, key: KeyboardShortcut) -> bool {
    input.events.iter().any(|e| {
        matches!(
            e,
            egui::Event::Key { key: k, pressed: true, repeat: false, modifiers, .. }
                if *k == key.logical_key && modifiers.matches_exact(key.modifiers)
        )
    })
}

/// True if `key` was pressed this frame with exactly its modifiers
/// (repeats count, as holding Ctrl+Z keeps undoing); the presses are taken
/// out so nothing else acts on them.
pub(crate) fn consume(input: &mut egui::InputState, key: KeyboardShortcut) -> bool {
    let before = input.events.len();
    input.events.retain(|e| {
        !matches!(
            e,
            egui::Event::Key { key: k, pressed: true, modifiers, .. }
                if *k == key.logical_key && modifiers.matches_exact(key.modifiers)
        )
    });
    input.events.len() != before
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

/// Text that may be long or odd, cut short for a message.
fn shorten(text: &str) -> String {
    let short: String = text.chars().take(MAX_CHORD_TEXT).collect();
    if short.len() < text.len() {
        format!("{short}…")
    } else {
        short
    }
}

/// Key text that isn't a key Halberd knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotAKey;

/// A key as the settings file writes it: "Ctrl+Shift+S", "F9", or "" for
/// no key.
pub fn chord_text(key: Option<KeyboardShortcut>) -> String {
    let Some(key) = key else {
        return String::new();
    };
    let mut parts = Vec::new();
    if key.modifiers.command || key.modifiers.ctrl {
        parts.push("Ctrl");
    }
    if key.modifiers.shift {
        parts.push("Shift");
    }
    if key.modifiers.alt {
        parts.push("Alt");
    }
    parts.push(key.logical_key.name());
    parts.join("+")
}

/// Reads key text from the settings file ("Ctrl+Shift+S"; "Cmd" means
/// Ctrl; "" is no key). Any letter case.
pub fn parse_chord(text: &str) -> Result<Option<KeyboardShortcut>, NotAKey> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    if text.len() > MAX_CHORD_TEXT {
        return Err(NotAKey);
    }
    let mut modifiers = Modifiers::NONE;
    let mut key = None;
    // "Ctrl++" names the Plus key.
    let parts: Vec<&str> = match text.strip_suffix("++") {
        Some(rest) => rest.split('+').chain(std::iter::once("Plus")).collect(),
        None => text.split('+').collect(),
    };
    for part in parts {
        let part = part.trim();
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "cmd" | "command" => modifiers |= Modifiers::COMMAND,
            "shift" => modifiers.shift = true,
            "alt" | "option" => modifiers.alt = true,
            _ if key.is_none() => key = Some(key_from_name(part).ok_or(NotAKey)?),
            _ => return Err(NotAKey),
        }
    }
    Ok(Some(KeyboardShortcut::new(modifiers, key.ok_or(NotAKey)?)))
}

/// A key by its name, in any letter case.
fn key_from_name(name: &str) -> Option<Key> {
    Key::from_name(name).or_else(|| {
        Key::ALL
            .iter()
            .copied()
            .find(|k| k.name().eq_ignore_ascii_case(name))
    })
}

#[cfg(test)]
mod tests;
