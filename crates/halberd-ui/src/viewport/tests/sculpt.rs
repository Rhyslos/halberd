//! The Sculpt tool in the viewport: its key, its toolbar row, and
//! dragging over a displacement.

use super::*;
use halberd_doc::{BrushObject, Command, Displacement, ObjectId, ordered_corners};
use halberd_geom::Brush;

/// A wide floor under the default camera whose top is a flat
/// displacement; its id.
fn terrain(h: &mut Harness<'_, State>) -> ObjectId {
    let brush = Brush::cuboid(Aabb::from_corners(
        Vec3::new(-2048.0, -2048.0, -16.0),
        Vec3::new(2048.0, 2048.0, 0.0),
    ))
    .unwrap();
    let doc = &mut h.state_mut().doc;
    doc.execute(Command::AddBrushes(vec![brush.clone()]))
        .unwrap();
    let id = *doc.selection().iter().next().unwrap();
    let top = brush
        .faces()
        .iter()
        .position(|f| f.plane().normal.z > 0.9)
        .unwrap();
    let shape = &brush.faces()[top];
    let corners = ordered_corners(shape.vertices(), Vec3::Z, shape.vertices()[0]).unwrap();
    let disp = Displacement::flat(4, corners[0], Vec3::Z);
    let mut faces = doc.get(id).unwrap().as_brush().unwrap().faces().to_vec();
    faces[top] = faces[top].with_displacement(Some(&disp));
    doc.execute(Command::ShapeEdit {
        label: "Make displacement".into(),
        changed: vec![(id, BrushObject::with_faces(brush, faces, Vec::new()))],
        added: vec![],
        removed: vec![],
        select_added: false,
    })
    .unwrap();
    doc.end_step();
    doc.clear_selection();
    id
}

fn highest(h: &Harness<'_, State>, id: ObjectId) -> f32 {
    let b = h.state().doc.get(id).unwrap().as_brush().unwrap();
    let disp = b.faces().iter().find_map(|f| f.displacement()).unwrap();
    disp.distances.iter().copied().fold(f32::MIN, f32::max)
}

#[test]
fn g_picks_sculpt_and_shows_its_settings() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    assert!(h.query_by_label(SCULPT_RADIUS_NAME).is_none());
    h.key_press(Key::G);
    h.run();
    assert_eq!(h.state().panel.tool(), Tool::Sculpt);
    assert!(h.query_by_label(SCULPT_RADIUS_NAME).is_some());
    assert!(h.query_by_label(SCULPT_STRENGTH_NAME).is_some());
    h.get_by_label("Paint blend").click();
    h.run();
    assert_eq!(
        h.state().panel.tools().sculpt_settings().mode,
        halberd_tools::SculptMode::Paint
    );
    h.key_press(Key::Q);
    h.run();
    assert_eq!(h.state().panel.tool(), Tool::Select);
}

#[test]
fn dragging_over_a_displacement_raises_it_in_one_undo_step() {
    let mut h = harness(ViewportOptions::default());
    let id = terrain(&mut h);
    h.state_mut().panel.set_tool(Tool::Sculpt);
    let at = egui::pos2(400.0, 300.0);
    h.hover_at(at);
    h.run();
    assert!(h.state().panel.tools().sculpt_at().is_some(), "over it");
    press(&h, PointerButton::Primary, at, true);
    // Held still, the tool keeps asking for frames: step through a few.
    for _ in 0..6 {
        h.step();
    }
    press(&h, PointerButton::Primary, at, false);
    h.run();
    assert!(highest(&h, id) > 0.0, "raised");
    assert_eq!(h.state().doc.undo_label(), Some("Sculpt"));
    h.state_mut().doc.undo();
    assert_eq!(highest(&h, id), 0.0, "one undo takes the stroke back");
    assert_eq!(h.state().doc.undo_label(), Some("Make displacement"));
}
