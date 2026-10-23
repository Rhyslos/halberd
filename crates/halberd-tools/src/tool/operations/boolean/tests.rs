//! Tests for Union, Subtract and Intersect, run as the interface runs them.

use super::super::ShapeOp;
use crate::camera::Camera;
use crate::{SelectMode, ToolController};
use glam::Vec3;
use halberd_doc::{BrushObject, Command, Document, EntityObject, MapObject};
use halberd_geom::{Aabb, Brush};

fn camera() -> Camera {
    Camera::looking_at(Vec3::new(500.0, -700.0, 600.0), Vec3::new(64.0, 32.0, 64.0))
}

fn block(min: Vec3, max: Vec3) -> Brush {
    Brush::cuboid(Aabb::from_corners(min, max)).unwrap()
}

fn tools() -> ToolController {
    let mut t = ToolController::new(16.0);
    t.set_select_mode(SelectMode::Object);
    t
}

/// A map with `others` unselected and `chosen` selected.
fn map(others: Vec<Brush>, chosen: Vec<Brush>) -> Document {
    let mut doc = Document::new();
    if !others.is_empty() {
        doc.execute(Command::AddBrushes(others)).unwrap();
    }
    if chosen.is_empty() {
        doc.clear_selection();
    } else {
        doc.execute(Command::AddBrushes(chosen)).unwrap();
    }
    doc.end_step();
    doc
}

fn brushes(doc: &Document) -> Vec<&BrushObject> {
    doc.objects().filter_map(|(_, o)| o.as_brush()).collect()
}

fn volume(doc: &Document) -> f32 {
    brushes(doc).iter().map(|b| b.brush().volume()).sum()
}

#[test]
fn subtract_carves_a_doorway_and_keeps_the_cutter() {
    let wall = block(Vec3::ZERO, Vec3::new(256.0, 16.0, 128.0));
    let door = block(Vec3::new(96.0, -8.0, 0.0), Vec3::new(160.0, 24.0, 96.0));
    let mut doc = map(vec![wall.clone()], vec![door.clone()]);
    let note = tools()
        .apply_shape_op(ShapeOp::Subtract, &mut doc, &camera())
        .unwrap();
    assert!(note.unwrap().contains("press Delete"));
    let all = brushes(&doc);
    assert_eq!(all.len(), 4, "three wall pieces and the door");
    let wall_left: f32 = all
        .iter()
        .filter(|b| b.brush() != &door)
        .map(|b| b.brush().volume())
        .sum();
    assert!((wall_left - (wall.volume() - 64.0 * 16.0 * 96.0)).abs() < 1.0);
    for b in &all {
        assert_eq!(b.faces().len(), b.brush().faces().len());
    }
    assert_eq!(doc.selection().len(), 1, "the door stays selected");
    assert_eq!(doc.undo().as_deref(), Some("Subtract from brush"));
    assert_eq!(brushes(&doc).len(), 2);
}

#[test]
fn subtract_with_nothing_underneath_is_refused() {
    let mut doc = map(
        vec![block(Vec3::ZERO, Vec3::splat(64.0))],
        vec![block(Vec3::splat(200.0), Vec3::splat(264.0))],
    );
    let refused = tools()
        .apply_shape_op(ShapeOp::Subtract, &mut doc, &camera())
        .unwrap_err();
    assert!(refused.contains("don't overlap"), "{refused}");
}

#[test]
fn subtract_cuts_brushes_inside_entities_and_keeps_them_there() {
    let detail = MapObject::Entity(
        EntityObject {
            classname: "func_detail".into(),
            origin: None,
            file_data: Vec::new(),
        },
        vec![BrushObject::new(block(
            Vec3::ZERO,
            Vec3::new(256.0, 16.0, 128.0),
        ))],
    );
    let mut doc = Document::from_map(vec![detail], Default::default()).unwrap();
    let entity = doc.objects().next().unwrap().0;
    doc.execute(Command::AddBrushes(vec![block(
        Vec3::new(96.0, -8.0, 0.0),
        Vec3::new(160.0, 24.0, 96.0),
    )]))
    .unwrap();
    tools()
        .apply_shape_op(ShapeOp::Subtract, &mut doc, &camera())
        .unwrap();
    assert_eq!(doc.brushes_of(entity).count(), 3);
}

#[test]
fn intersect_keeps_the_common_part() {
    let mut doc = map(
        Vec::new(),
        vec![
            block(Vec3::ZERO, Vec3::splat(64.0)),
            block(Vec3::splat(32.0), Vec3::splat(96.0)),
        ],
    );
    tools()
        .apply_shape_op(ShapeOp::Intersect, &mut doc, &camera())
        .unwrap();
    let all = brushes(&doc);
    assert_eq!(all.len(), 1);
    assert_eq!(
        all[0].brush().bounds(),
        Aabb::from_corners(Vec3::splat(32.0), Vec3::splat(64.0))
    );
    assert_eq!(doc.undo_label(), Some("Intersect 2 brushes"));
    let mut apart = map(
        Vec::new(),
        vec![
            block(Vec3::ZERO, Vec3::splat(64.0)),
            block(Vec3::splat(100.0), Vec3::splat(164.0)),
        ],
    );
    assert!(
        tools()
            .apply_shape_op(ShapeOp::Intersect, &mut apart, &camera())
            .is_err()
    );
}

#[test]
fn union_removes_overlap_and_merges_when_it_can() {
    // An L: two pieces, no overlap, same space as before.
    let a = block(Vec3::ZERO, Vec3::new(128.0, 32.0, 32.0));
    let b = block(Vec3::new(96.0, 0.0, 0.0), Vec3::new(128.0, 128.0, 32.0));
    let mut doc = map(Vec::new(), vec![a.clone(), b.clone()]);
    let overlap = a.intersection(&b).unwrap().volume();
    tools()
        .apply_shape_op(ShapeOp::Union, &mut doc, &camera())
        .unwrap();
    let all = brushes(&doc);
    assert!(all.len() >= 2);
    assert!((volume(&doc) - (a.volume() + b.volume() - overlap)).abs() < 1.0);
    for (i, x) in all.iter().enumerate() {
        for y in &all[i + 1..] {
            assert!(!x.brush().overlaps(y.brush()), "pieces don't overlap");
        }
    }
    assert_eq!(doc.undo_label(), Some("Union of 2 brushes"));
    // Two overlapping boxes in a row make one box.
    let mut row = map(
        Vec::new(),
        vec![
            block(Vec3::ZERO, Vec3::splat(64.0)),
            block(Vec3::new(32.0, 0.0, 0.0), Vec3::new(128.0, 64.0, 64.0)),
        ],
    );
    tools()
        .apply_shape_op(ShapeOp::Union, &mut row, &camera())
        .unwrap();
    let one = brushes(&row);
    assert_eq!(one.len(), 1);
    assert_eq!(
        one[0].brush().bounds(),
        Aabb::from_corners(Vec3::ZERO, Vec3::new(128.0, 64.0, 64.0))
    );
}

#[test]
fn every_boolean_waits_for_a_selection() {
    let doc = map(vec![block(Vec3::ZERO, Vec3::splat(64.0))], Vec::new());
    let mut doc = doc;
    doc.clear_selection();
    let t = tools();
    for op in [ShapeOp::Union, ShapeOp::Subtract, ShapeOp::Intersect] {
        assert!(t.shape_op_ready(op, &doc).is_err(), "{op:?}");
    }
}
