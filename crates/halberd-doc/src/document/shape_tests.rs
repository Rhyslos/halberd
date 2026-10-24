//! Tests for floor plan shapes and shape edits in the document: adding,
//! moving, merging, undo.

use super::*;
use crate::{FaceInfo, ShapeObject, TransformKind};
use glam::{Vec2, Vec3};

fn l_shape() -> ShapeObject {
    let outline = [
        Vec2::new(0.0, 0.0),
        Vec2::new(256.0, 0.0),
        Vec2::new(256.0, 128.0),
        Vec2::new(128.0, 128.0),
        Vec2::new(128.0, 256.0),
        Vec2::new(0.0, 256.0),
    ];
    ShapeObject::new(&outline, 0.0, 64.0, FaceInfo::default()).unwrap()
}

fn only_shape(doc: &Document) -> (ObjectId, &ShapeObject) {
    let mut shapes = doc
        .objects()
        .filter_map(|(id, o)| Some((id, o.as_shape()?)));
    let found = shapes.next().unwrap();
    assert!(shapes.next().is_none());
    found
}

#[test]
fn adding_a_shape_selects_it_and_undo_takes_it_away() {
    let mut doc = Document::new();
    doc.execute(Command::AddShape(l_shape())).unwrap();
    let (id, _) = only_shape(&doc);
    assert!(doc.selection().contains(&id));
    assert_eq!(doc.undo_label(), Some("Create floor plan shape"));
    doc.undo();
    assert_eq!(doc.objects().count(), 0);
    doc.redo();
    only_shape(&doc);
}

#[test]
fn a_moved_shape_moves_its_pieces_and_undo_puts_it_back() {
    let mut doc = Document::new();
    doc.execute(Command::AddShape(l_shape())).unwrap();
    let (id, shape) = only_shape(&doc);
    let moved = shape.mapped(|p| p + Vec3::new(32.0, 0.0, 16.0)).unwrap();
    doc.end_step();
    doc.execute(Command::TransformBrushes {
        kind: TransformKind::Move,
        brushes: Vec::new(),
        shapes: vec![(id, moved)],
        entities: Vec::new(),
    })
    .unwrap();
    assert_eq!(doc.undo_label(), Some("Move shape"));
    let (_, shape) = only_shape(&doc);
    assert_eq!(shape.outline()[0], Vec2::new(32.0, 0.0));
    assert_eq!((shape.bottom(), shape.top()), (16.0, 80.0));
    assert_eq!(shape.bounds().min, Vec3::new(32.0, 0.0, 16.0));
    doc.undo();
    let (_, shape) = only_shape(&doc);
    assert_eq!(shape.outline()[0], Vec2::ZERO);
}

#[test]
fn a_shape_is_not_a_brush_to_brush_edits() {
    let mut doc = Document::new();
    doc.execute(Command::AddShape(l_shape())).unwrap();
    let (id, shape) = only_shape(&doc);
    let result = doc.execute(Command::TransformBrushes {
        kind: TransformKind::Move,
        brushes: vec![(id, shape.pieces()[0].clone())],
        shapes: Vec::new(),
        entities: Vec::new(),
    });
    assert!(result.is_err(), "a shape can't be replaced by a brush");
    only_shape(&doc);
}

#[test]
fn shape_edits_of_one_drag_merge_even_onto_more_brushes() {
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![
        super::tests::cube_at(0.0),
        super::tests::cube_at(100.0),
    ]))
    .unwrap();
    doc.end_step();
    let all = super::tests::ids(&doc);
    let painted = |doc: &Document, id: ObjectId, material: &str| {
        let mut b = doc.get(id).unwrap().as_brush().unwrap().clone();
        b.set_material(None, material);
        Command::ShapeEdit {
            label: "Sculpt".into(),
            changed: vec![(id, b)],
            added: vec![],
            removed: vec![],
            select_added: false,
        }
    };
    doc.execute_merging(painted(&doc, all[0], "A"), 7).unwrap();
    doc.execute_merging(painted(&doc, all[1], "B"), 7).unwrap();
    doc.execute_merging(painted(&doc, all[0], "C"), 7).unwrap();
    doc.end_step();
    assert_eq!(doc.undo().as_deref(), Some("Sculpt"));
    for id in &all {
        let b = doc.get(*id).unwrap().as_brush().unwrap();
        assert!(
            b.faces()
                .iter()
                .all(|f| f.material == FaceInfo::default().material)
        );
    }
    assert_eq!(doc.undo_label(), Some("Create 2 brushes"));
}
