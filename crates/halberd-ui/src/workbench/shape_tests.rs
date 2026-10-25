//! Tests for floor plan shapes in the workbench.

use super::tests::{harness_for, info};
use super::*;
use egui::Key;
use halberd_doc::{FaceInfo, ShapeObject};

fn workbench_with_shape() -> Workbench {
    let mut wb = Workbench::new(info(), None);
    let outline = [
        glam::Vec2::new(0.0, 0.0),
        glam::Vec2::new(128.0, 0.0),
        glam::Vec2::new(128.0, 64.0),
        glam::Vec2::new(64.0, 64.0),
        glam::Vec2::new(64.0, 128.0),
        glam::Vec2::new(0.0, 128.0),
    ];
    let shape = ShapeObject::new(&outline, 0.0, 64.0, FaceInfo::default()).unwrap();
    wb.document_mut().execute(Command::AddShape(shape)).unwrap();
    wb
}

#[test]
fn delete_removes_a_selected_shape_even_in_face_mode() {
    // Regression: in Vertex, Edge or Face mode Delete waited for picked
    // parts, which a shape doesn't have, so nothing could delete it.
    let mut harness = harness_for(workbench_with_shape());
    harness.run();
    harness.key_press(Key::Num4);
    harness.run();
    harness.key_press(Key::Delete);
    harness.run();
    assert!(harness.state().document().is_empty());
}

#[test]
fn ctrl_shift_b_makes_brushes_from_the_selected_shape() {
    let mut harness = harness_for(workbench_with_shape());
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::B);
    harness.run();
    let doc = harness.state().document();
    assert_eq!(doc.len(), 2, "the L's two pieces");
    assert!(doc.objects().all(|(_, o)| o.as_brush().is_some()));
}
