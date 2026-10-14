//! The shape operations in the interface: their keys, the Shape menu, and
//! carrying them out.

use super::ViewportPanel;
use super::tools::first_press;
use egui::{Key, KeyboardShortcut, Modifiers, Ui};
use halberd_doc::Document;
use halberd_tools::{Axis, ShapeOp};

/// The key for each shape operation that has one.
pub const SHAPE_OP_KEYS: [(ShapeOp, KeyboardShortcut); 8] = [
    (
        ShapeOp::Extrude,
        KeyboardShortcut::new(Modifiers::NONE, Key::E),
    ),
    (
        ShapeOp::Bevel,
        KeyboardShortcut::new(Modifiers::COMMAND, Key::B),
    ),
    (
        ShapeOp::Bridge,
        KeyboardShortcut::new(Modifiers::NONE, Key::F),
    ),
    (
        ShapeOp::Split,
        KeyboardShortcut::new(Modifiers::NONE, Key::K),
    ),
    (
        ShapeOp::Clip,
        KeyboardShortcut::new(Modifiers::NONE, Key::X),
    ),
    (
        ShapeOp::Join,
        KeyboardShortcut::new(Modifiers::COMMAND, Key::J),
    ),
    (
        ShapeOp::MergeCorners,
        KeyboardShortcut::new(Modifiers::NONE, Key::M),
    ),
    (
        ShapeOp::MirrorAcrossView,
        KeyboardShortcut::new(Modifiers::COMMAND, Key::M),
    ),
];

/// The key for `op`, if it has one.
fn shape_op_key(op: ShapeOp) -> Option<KeyboardShortcut> {
    SHAPE_OP_KEYS
        .iter()
        .find(|(o, _)| *o == op)
        .map(|(_, key)| *key)
}

impl ViewportPanel {
    /// Carries out `op` on `doc` as one undo step. Returns messages for
    /// the Console: why it could not be done, or something worth knowing.
    pub fn run_shape_op(&mut self, op: ShapeOp, doc: &mut Document) -> Vec<String> {
        let camera = *self.controller.camera();
        match self.tools.apply_shape_op(op, doc, &camera) {
            Ok(note) => note.into_iter().collect(),
            Err(reason) => vec![reason],
        }
    }

    /// Whether `op` can run now, or what to do first.
    pub fn shape_op_ready(&self, op: ShapeOp, doc: &Document) -> Result<(), String> {
        self.tools.shape_op_ready(op, doc)
    }

    /// The shape operation keys pressed this frame, carried out.
    pub(super) fn shape_op_keys(&mut self, ui: &Ui, doc: &mut Document) -> Vec<String> {
        let pressed: Vec<ShapeOp> = ui.input(|i| {
            SHAPE_OP_KEYS
                .iter()
                .filter(|(_, key)| {
                    first_press(i, key.logical_key, |m| {
                        if key.modifiers.command {
                            m.command_only()
                        } else {
                            m.is_none()
                        }
                    })
                })
                .map(|(op, _)| *op)
                .collect()
        });
        pressed
            .into_iter()
            .flat_map(|op| self.run_shape_op(op, doc))
            .collect()
    }
}

/// The Shape menu: every operation, greyed out with the reason when it
/// can't run yet. Returns messages for the Console.
pub(crate) fn shape_menu(
    ui: &mut Ui,
    viewport: &mut ViewportPanel,
    doc: &mut Document,
) -> Vec<String> {
    let mut notes = Vec::new();
    for op in ShapeOp::MENU {
        if matches!(op, ShapeOp::MirrorAcrossView | ShapeOp::Mirror(Axis::X)) {
            ui.separator();
        }
        let mut button = egui::Button::new(op.label());
        if let Some(key) = shape_op_key(op) {
            button = button.shortcut_text(ui.ctx().format_shortcut(&key));
        }
        let ready = viewport.shape_op_ready(op, doc);
        let response = ui.add_enabled(ready.is_ok(), button);
        let response = match &ready {
            Ok(()) => response.on_hover_text(op.description()),
            Err(why) => response.on_disabled_hover_text(format!("{}. {why}", op.description())),
        };
        if response.clicked() {
            notes.extend(viewport.run_shape_op(op, doc));
        }
    }
    notes
}
