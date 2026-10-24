//! The gizmo moving and turning point entities (props) with brushes.

use super::*;
use halberd_doc::EntityObject;

#[test]
fn a_quarter_turn_carries_a_prop_round_and_turns_it() {
    let camera = Camera::looking_at(Vec3::new(30.0, -60.0, 1500.0), Vec3::ZERO);
    let mut doc = Document::new();
    let cube = Brush::cuboid(Aabb::from_corners(
        Vec3::new(-108.0, -8.0, 8.0),
        Vec3::new(-92.0, 8.0, 24.0),
    ))
    .unwrap();
    doc.execute(Command::AddBrushes(vec![cube])).unwrap();
    let brush = doc.objects().next().unwrap().0;
    doc.execute(Command::AddEntity(EntityObject::point(
        "prop_static",
        Vec3::new(100.0, 0.0, 16.0),
        &[("model", "models/props_c17/oildrum001.mdl")],
    )))
    .unwrap();
    let prop = doc.objects().last().unwrap().0;
    doc.set_selection([brush, prop].into_iter().collect::<Vec<_>>());
    let mut t = tools(GizmoMode::Rotate);
    let from = grab_point(&t, &camera, &doc, Handle::Rotate(Axis::Z));
    let pivot = screen(&camera, Vec3::new(0.0, 0.0, 16.0));
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
    let moved = doc.get(prop).unwrap().as_entity().unwrap();
    let origin = moved.origin.unwrap();
    assert!(
        (origin - Vec3::new(0.0, 100.0, 16.0)).length() < 0.5,
        "{origin}"
    );
    assert!((moved.angles() - Vec3::new(0.0, 90.0, 0.0)).length() < 0.5);
    assert_eq!(doc.undo().as_deref(), Some("Rotate 2 objects"));
    let back = doc.get(prop).unwrap().as_entity().unwrap();
    assert_eq!(back.origin, Some(Vec3::new(100.0, 0.0, 16.0)));
}

#[test]
fn a_prop_alone_moves_with_the_gizmo() {
    let camera = angled();
    let mut doc = Document::new();
    doc.execute(Command::AddEntity(EntityObject::point(
        "prop_static",
        Vec3::new(64.0, 32.0, 64.0),
        &[],
    )))
    .unwrap();
    let mut t = tools(GizmoMode::Move);
    let from = grab_point(&t, &camera, &doc, Handle::MoveAxis(Axis::X));
    let to = screen(&camera, Vec3::new(64.0 + 128.0, 32.0, 64.0));
    drag(
        &mut t,
        &camera,
        &mut doc,
        from,
        &[
            (from + to) * 0.5,
            from + (to - screen(&camera, Vec3::new(64.0, 32.0, 64.0))),
        ],
    );
    let entity = doc.objects().next().unwrap().1.as_entity().unwrap();
    let origin = entity.origin.unwrap();
    assert!(origin.x > 64.0 + 100.0 && origin.y == 32.0, "{origin}");
    assert_eq!(
        entity.setting("origin"),
        Some(format!("{} 32 64", origin.x).as_str())
    );
    assert_eq!(doc.undo().as_deref(), Some("Move entity"));
}
