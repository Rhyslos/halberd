//! The gizmo moving and turning floor plan shapes.

use super::*;
use glam::Vec2 as Flat;
use halberd_doc::{FaceInfo, ShapeObject};

/// A map with one L-shaped floor plan inside 0..128 × 0..64 × 0..128,
/// selected.
fn one_shape() -> Document {
    let outline = [
        Flat::new(0.0, 0.0),
        Flat::new(128.0, 0.0),
        Flat::new(128.0, 32.0),
        Flat::new(64.0, 32.0),
        Flat::new(64.0, 64.0),
        Flat::new(0.0, 64.0),
    ];
    let shape = ShapeObject::new(&outline, 0.0, 128.0, FaceInfo::default()).unwrap();
    let mut doc = Document::new();
    doc.execute(Command::AddShape(shape)).unwrap();
    doc
}

fn shape(doc: &Document) -> &ShapeObject {
    doc.objects().find_map(|(_, o)| o.as_shape()).unwrap()
}

#[test]
fn dragging_an_arrow_moves_a_shape() {
    let camera = angled();
    let mut doc = one_shape();
    let mut t = tools(GizmoMode::Move);
    let from = grab_point(&t, &camera, &doc, Handle::MoveAxis(Axis::X));
    let step = screen(&camera, Vec3::new(164.0, 32.0, 64.0))
        - screen(&camera, Vec3::new(64.0, 32.0, 64.0));
    drag(
        &mut t,
        &camera,
        &mut doc,
        from,
        &[from + step * 0.5, from + step],
    );
    assert_eq!(shape(&doc).bounds().min, Vec3::new(96.0, 0.0, 0.0));
    assert_eq!(shape(&doc).outline()[0], Flat::new(96.0, 0.0));
    assert_eq!(doc.undo().as_deref(), Some("Move shape"));
    assert_eq!(shape(&doc).bounds().min, Vec3::ZERO);
}

#[test]
fn a_shape_turns_about_the_upright_axis_only() {
    let camera = angled();
    for (axis, turns) in [(Axis::Z, true), (Axis::X, false)] {
        let mut doc = one_shape();
        let before = shape(&doc).clone();
        let mut t = tools(GizmoMode::Rotate);
        let from = grab_point(&t, &camera, &doc, Handle::Rotate(axis));
        let pivot = screen(&camera, Vec3::new(64.0, 32.0, 64.0));
        let arm = from - pivot;
        let path: Vec<Vec2> = (1..=9)
            .map(|i| {
                let a = -(i as f32) * 10f32.to_radians();
                pivot
                    + Vec2::new(
                        arm.x * a.cos() - arm.y * a.sin(),
                        arm.x * a.sin() + arm.y * a.cos(),
                    )
            })
            .collect();
        drag(&mut t, &camera, &mut doc, from, &path);
        let after = shape(&doc);
        assert_eq!(after.pieces().len(), 2, "{axis:?}: still an L");
        assert_eq!(*after != before, turns, "{axis:?}");
        assert_eq!(
            (after.bottom(), after.top()),
            (0.0, 128.0),
            "{axis:?}: stays upright"
        );
    }
}
