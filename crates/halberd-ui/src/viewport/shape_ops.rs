//! The shape operations in the interface: their keys, the Shape menu, and
//! carrying them out.

use super::ViewportPanel;
use crate::Command;
use crate::bindings::pressed;
use egui::Ui;
use halberd_doc::Document;
use halberd_tools::ShapeOp;

/// The command (and so the key) of each shape operation that has one.
pub const SHAPE_OP_COMMANDS: [(ShapeOp, Command); 12] = [
    (ShapeOp::Extrude, Command::Extrude),
    (ShapeOp::Bevel, Command::Bevel),
    (ShapeOp::Bridge, Command::Bridge),
    (ShapeOp::Split, Command::Split),
    (ShapeOp::Clip, Command::Clip),
    (ShapeOp::Join, Command::Join),
    (ShapeOp::MergeCorners, Command::MergeCorners),
    (ShapeOp::MirrorAcrossView, Command::Mirror),
    (ShapeOp::Union, Command::Union),
    (ShapeOp::Subtract, Command::Subtract),
    (ShapeOp::Intersect, Command::Intersect),
    (ShapeOp::MakeBrushes, Command::MakeBrushes),
];

/// The command of `op`, if it has one.
fn shape_op_command(op: ShapeOp) -> Option<Command> {
    SHAPE_OP_COMMANDS
        .iter()
        .find(|(o, _)| *o == op)
        .map(|(_, command)| *command)
}

impl ViewportPanel {
    /// Carries out `op` on `doc` as one undo step. Returns messages for
    /// the Console: why it could not be done, or something worth knowing.
    pub fn run_shape_op(&mut self, op: ShapeOp, doc: &mut Document) -> Vec<String> {
        let camera = self.active_camera();
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
        let bindings = &self.bindings;
        let ops: Vec<ShapeOp> = ui.input(|i| {
            SHAPE_OP_COMMANDS
                .iter()
                .filter_map(|(op, command)| Some((*op, bindings.key(*command)?)))
                .filter(|(_, key)| pressed(i, *key))
                .map(|(op, _)| op)
                .collect::<Vec<_>>()
        });
        ops.into_iter()
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
        if matches!(
            op,
            ShapeOp::MirrorAcrossView | ShapeOp::Union | ShapeOp::MakeBrushes
        ) {
            ui.separator();
        }
        let mut button = egui::Button::new(op.label());
        if let Some(key) = shape_op_command(op).and_then(|c| viewport.bindings().key(c)) {
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
