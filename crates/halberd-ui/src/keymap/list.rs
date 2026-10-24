//! The list of every key binding: what each key does, and when.

use super::{KeyCap, KeyContext, Keybind};
use crate::Command;
use KeyCap::{Alt, Ctrl, Shift};
use egui::Key;
use std::borrow::Cow;

const fn k(key: Key) -> KeyCap {
    KeyCap::Key(key)
}

/// Every key binding, grouped by when it applies.
pub const KEYBINDS: &[Keybind] = &[
    // Anywhere.
    cmd(
        Command::New,
        &[Ctrl, k(Key::N)],
        "New map",
        KeyContext::Anywhere,
    ),
    cmd(
        Command::Open,
        &[Ctrl, k(Key::O)],
        "Open a map",
        KeyContext::Anywhere,
    ),
    cmd(
        Command::Save,
        &[Ctrl, k(Key::S)],
        "Save",
        KeyContext::Anywhere,
    ),
    cmd(
        Command::SaveAs,
        &[Ctrl, Shift, k(Key::S)],
        "Save as…",
        KeyContext::Anywhere,
    ),
    cmd(
        Command::Quit,
        &[Ctrl, k(Key::Q)],
        "Quit",
        KeyContext::Anywhere,
    ),
    cmd(
        Command::Compile,
        &[k(Key::F9)],
        "Compile and play (quick test in Garry's Mod)",
        KeyContext::Anywhere,
    ),
    cmd(
        Command::Undo,
        &[Ctrl, k(Key::Z)],
        "Undo",
        KeyContext::Anywhere,
    ),
    cmd(
        Command::Redo,
        &[Ctrl, k(Key::Y)],
        "Redo",
        KeyContext::Anywhere,
    ),
    bind(&[Ctrl, Shift, k(Key::Z)], "Redo", KeyContext::Anywhere),
    cmd(
        Command::Delete,
        &[k(Key::Delete)],
        "Delete the selection",
        KeyContext::Anywhere,
    ),
    bind(
        &[k(Key::Backspace)],
        "Delete the selection",
        KeyContext::Anywhere,
    ),
    // Viewport.
    cmd(
        Command::SelectTool,
        &[k(Key::Q)],
        "Select tool",
        KeyContext::Viewport,
    ),
    cmd(
        Command::DrawTool,
        &[k(Key::B)],
        "Draw tool; again: open or close the shape list",
        KeyContext::Viewport,
    ),
    cmd(
        Command::MoveGizmo,
        &[k(Key::W)],
        "Move gizmo (again: off)",
        KeyContext::Viewport,
    ),
    cmd(
        Command::RotateGizmo,
        &[k(Key::R)],
        "Rotate gizmo (again: off)",
        KeyContext::Viewport,
    ),
    cmd(
        Command::ScaleGizmo,
        &[k(Key::S)],
        "Scale gizmo (again: off)",
        KeyContext::Viewport,
    ),
    cmd(
        Command::AllGizmos,
        &[k(Key::T)],
        "All gizmos at once (again: off)",
        KeyContext::Viewport,
    ),
    cmd(
        Command::InsideEntities,
        &[Ctrl, k(Key::W)],
        "Pick single brushes inside entities (on/off)",
        KeyContext::Viewport,
    ),
    cmd(
        Command::ObjectMode,
        &[k(Key::Num1)],
        "Object mode: clicks pick whole objects",
        KeyContext::Viewport,
    ),
    cmd(
        Command::VertexMode,
        &[k(Key::Num2)],
        "Vertex mode: pick corners of the selected brushes",
        KeyContext::Viewport,
    ),
    cmd(
        Command::EdgeMode,
        &[k(Key::Num3)],
        "Edge mode: pick edges of the selected brushes",
        KeyContext::Viewport,
    ),
    cmd(
        Command::FaceMode,
        &[k(Key::Num4)],
        "Face mode: pick faces of the selected brushes",
        KeyContext::Viewport,
    ),
    bind(
        &[k(Key::Escape)],
        "Let go of picked corners, edges or faces; again: deselect. Also cancels a drag",
        KeyContext::Viewport,
    ),
    // Shape operations.
    cmd(
        Command::Extrude,
        &[k(Key::E)],
        "Extrude: a new brush on each picked face",
        KeyContext::Viewport,
    ),
    cmd(
        Command::Bevel,
        &[Ctrl, k(Key::B)],
        "Bevel the picked edges",
        KeyContext::Viewport,
    ),
    cmd(
        Command::Bridge,
        &[k(Key::F)],
        "Bridge: fill the space spanned by the picked parts",
        KeyContext::Viewport,
    ),
    cmd(
        Command::Split,
        &[k(Key::K)],
        "Split the selected brushes along the picked parts",
        KeyContext::Viewport,
    ),
    cmd(
        Command::Clip,
        &[k(Key::X)],
        "Clip: cut along the picked parts, keep the bigger part",
        KeyContext::Viewport,
    ),
    cmd(
        Command::Join,
        &[Ctrl, k(Key::J)],
        "Join the selected brushes into one",
        KeyContext::Viewport,
    ),
    cmd(
        Command::MergeCorners,
        &[k(Key::M)],
        "Merge the picked corners into one",
        KeyContext::Viewport,
    ),
    cmd(
        Command::Mirror,
        &[Ctrl, k(Key::M)],
        "Mirror the selected brushes left to right",
        KeyContext::Viewport,
    ),
    cmd(
        Command::Union,
        &[Ctrl, Shift, k(Key::U)],
        "Union: the selected brushes fill their space together, without overlap",
        KeyContext::Viewport,
    ),
    cmd(
        Command::Subtract,
        &[Ctrl, Shift, k(Key::C)],
        "Subtract: cut the selected brushes' space out of the brushes they overlap",
        KeyContext::Viewport,
    ),
    cmd(
        Command::Intersect,
        &[Ctrl, Shift, k(Key::I)],
        "Intersect: keep only the space the selected brushes share",
        KeyContext::Viewport,
    ),
    cmd(
        Command::MakeBrushes,
        &[Ctrl, Shift, k(Key::B)],
        "Make ordinary brushes from the selected floor plan shapes",
        KeyContext::Viewport,
    ),
    bind(
        &[k(Key::Delete)],
        "In Vertex or Face mode: delete the picked corners or faces",
        KeyContext::Viewport,
    ),
    bind(
        &[k(Key::Backspace)],
        "In Vertex or Face mode: delete the picked corners or faces",
        KeyContext::Viewport,
    ),
    // While drawing.
    cmd(
        Command::TurnShape,
        &[k(Key::R)],
        "Turn the shape a quarter turn clockwise",
        KeyContext::Drawing,
    ),
    bind(
        &[Shift],
        "As wide, deep and tall as the longest side",
        KeyContext::Drawing,
    ),
    bind(
        &[Alt],
        "As wide, deep and tall, around where you started",
        KeyContext::Drawing,
    ),
    bind(
        &[k(Key::Enter)],
        "Floor plan: finish the outline",
        KeyContext::Drawing,
    ),
    bind(
        &[k(Key::Backspace)],
        "Floor plan: take back the last corner",
        KeyContext::Drawing,
    ),
    bind(&[k(Key::Escape)], "Cancel the shape", KeyContext::Drawing),
    bind(
        &[k(Key::Q)],
        "Cancel the shape and pick the Select tool",
        KeyContext::Drawing,
    ),
    // Shape list.
    bind(&[k(Key::ArrowUp)], "Previous shape", KeyContext::ShapeList),
    bind(&[k(Key::ArrowDown)], "Next shape", KeyContext::ShapeList),
    bind(&[k(Key::Enter)], "Pick the shape", KeyContext::ShapeList),
    bind(&[k(Key::Space)], "Pick the shape", KeyContext::ShapeList),
    bind(&[k(Key::Escape)], "Close the list", KeyContext::ShapeList),
    // Flying.
    cmd(
        Command::FlyForward,
        &[k(Key::W)],
        "Fly forward",
        KeyContext::Flying,
    ),
    cmd(
        Command::FlyBack,
        &[k(Key::S)],
        "Fly back",
        KeyContext::Flying,
    ),
    cmd(
        Command::FlyLeft,
        &[k(Key::A)],
        "Fly left",
        KeyContext::Flying,
    ),
    cmd(
        Command::FlyRight,
        &[k(Key::D)],
        "Fly right",
        KeyContext::Flying,
    ),
    cmd(
        Command::FlyUp,
        &[k(Key::Space)],
        "Fly up",
        KeyContext::Flying,
    ),
    cmd(
        Command::FlyDown,
        &[k(Key::C)],
        "Fly down",
        KeyContext::Flying,
    ),
    bind(&[Shift], "Fly faster", KeyContext::Flying),
    cmd(
        Command::MoveGizmo,
        &[Shift, k(Key::W)],
        "Move gizmo",
        KeyContext::RightMouse,
    ),
    cmd(
        Command::ScaleGizmo,
        &[Shift, k(Key::S)],
        "Scale gizmo",
        KeyContext::RightMouse,
    ),
    // Save question.
    bind(&[k(Key::Enter)], "Save", KeyContext::SaveQuestion),
    bind(&[k(Key::N)], "Don't save", KeyContext::SaveQuestion),
    bind(&[k(Key::Escape)], "Cancel", KeyContext::SaveQuestion),
    // Message box.
    bind(
        &[k(Key::Escape)],
        "Close the message",
        KeyContext::MessageBox,
    ),
];

const fn bind(keys: &'static [KeyCap], action: &'static str, context: KeyContext) -> Keybind {
    Keybind {
        keys: Cow::Borrowed(keys),
        command: None,
        action,
        context,
    }
}

/// A binding Settings can change; `keys` are `command`'s default keys.
const fn cmd(
    command: Command,
    keys: &'static [KeyCap],
    action: &'static str,
    context: KeyContext,
) -> Keybind {
    Keybind {
        keys: Cow::Borrowed(keys),
        command: Some(command),
        action,
        context,
    }
}
