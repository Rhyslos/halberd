//! Tests for the shape operations, carried out on a real map as the
//! interface does.

use super::*;
use crate::ToolInput;
use glam::Vec2;
use halberd_doc::{EntityObject, MapObject};
use halberd_geom::Aabb;

const SIZE: Vec2 = Vec2::new(1600.0, 900.0);

fn angled() -> Camera {
    Camera::looking_at(Vec3::new(500.0, -700.0, 600.0), Vec3::new(64.0, 32.0, 64.0))
}

/// Straight down from above.
fn above() -> Camera {
    Camera::looking_at(Vec3::new(64.0, 31.0, 900.0), Vec3::new(64.0, 32.0, 0.0))
}

fn block(min: Vec3, max: Vec3) -> Brush {
    Brush::cuboid(Aabb::from_corners(min, max)).unwrap()
}

/// A map with these brushes, all selected.
fn map(brushes: Vec<Brush>) -> Document {
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(brushes)).unwrap();
    doc.end_step();
    doc
}

fn one_box() -> Document {
    map(vec![block(Vec3::ZERO, Vec3::new(128.0, 64.0, 128.0))])
}

fn tools(mode: SelectMode) -> ToolController {
    let mut t = ToolController::new(16.0);
    t.set_select_mode(mode);
    t
}

fn first_id(doc: &Document) -> ObjectId {
    *doc.selection().iter().next().unwrap()
}

fn brush_of(doc: &Document, id: ObjectId) -> &BrushObject {
    brush_object(doc, id).unwrap()
}

/// Picks parts of the first selected brush.
fn pick(t: &mut ToolController, doc: &Document, shapes: Vec<ElementShape>) {
    let id = first_id(doc);
    t.elements.items = shapes
        .into_iter()
        .map(|shape| PickedElement { brush: id, shape })
        .collect();
}

fn top_face(doc: &Document) -> ElementShape {
    let b = brush_of(doc, first_id(doc)).brush();
    let top = b.faces().iter().find(|f| f.plane().normal.z > 0.9).unwrap();
    ElementShape::Face(top.vertices().to_vec())
}

/// Runs `op` as the interface does. Returns its note.
fn run(
    t: &mut ToolController,
    doc: &mut Document,
    camera: &Camera,
    op: ShapeOp,
) -> Result<Option<String>, String> {
    t.apply_shape_op(op, doc, camera)
}

fn brushes(doc: &Document) -> Vec<&BrushObject> {
    doc.objects().filter_map(|(_, o)| o.as_brush()).collect()
}

#[test]
fn each_operation_says_what_it_needs_first() {
    let doc = one_box();
    let t = tools(SelectMode::Object);
    for op in ShapeOp::MENU {
        let ready = t.shape_op_ready(op, &doc);
        match op {
            ShapeOp::Mirror(_) | ShapeOp::MirrorAcrossView => assert!(ready.is_ok()),
            _ => assert!(ready.unwrap_err().ends_with('.'), "{op:?}"),
        }
        assert!(!op.label().is_empty() && !op.description().is_empty());
    }
    let t = tools(SelectMode::Edge);
    assert!(t.shape_op_ready(ShapeOp::DeleteParts, &doc).is_err());
}

#[test]
fn extruding_the_top_face_adds_a_brush_with_its_far_face_picked() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(SelectMode::Face);
    let top = top_face(&doc);
    pick(&mut t, &doc, vec![top]);
    run(&mut t, &mut doc, &camera, ShapeOp::Extrude).unwrap();
    assert_eq!(doc.len(), 2);
    let new = brush_of(&doc, first_id(&doc));
    assert_eq!(
        new.brush().bounds(),
        Aabb::from_corners(Vec3::new(0.0, 0.0, 128.0), Vec3::new(128.0, 64.0, 144.0))
    );
    assert_eq!(t.picked_elements().len(), 1);
    match &t.picked_elements()[0].shape {
        ElementShape::Face(corners) => assert!(corners.iter().all(|c| c.z == 144.0)),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        t.gizmo_mode(),
        Some(GizmoMode::Move),
        "ready to pull further"
    );
    assert_eq!(doc.undo().as_deref(), Some("Extrude face"));
    assert_eq!(doc.len(), 1);
}

#[test]
fn extruding_a_brush_of_an_entity_keeps_the_new_brush_in_it() {
    let detail = MapObject::Entity(
        EntityObject {
            classname: "func_detail".into(),
            origin: None,
            file_data: Vec::new(),
        },
        vec![BrushObject::new(block(Vec3::ZERO, Vec3::splat(64.0)))],
    );
    let mut doc = Document::from_map(vec![detail], Default::default()).unwrap();
    let entity = doc.objects().next().unwrap().0;
    let brush = doc.brushes_of(entity).next().unwrap();
    doc.set_selection([brush]);
    let mut t = tools(SelectMode::Face);
    pick(&mut t, &doc, vec![top_face(&doc)]);
    run(&mut t, &mut doc, &angled(), ShapeOp::Extrude).unwrap();
    assert_eq!(doc.brushes_of(entity).count(), 2);
}

#[test]
fn bevelling_an_edge_slants_it() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(SelectMode::Edge);
    let edge = ElementShape::Edge([Vec3::new(128.0, 0.0, 128.0), Vec3::new(128.0, 64.0, 128.0)]);
    pick(&mut t, &doc, vec![edge]);
    run(&mut t, &mut doc, &camera, ShapeOp::Bevel).unwrap();
    let b = brush_of(&doc, first_id(&doc));
    assert_eq!(b.brush().faces().len(), 7);
    assert_eq!(b.faces().len(), 7);
    assert!(b.brush().points().contains(&Vec3::new(112.0, 0.0, 128.0)));
    assert_eq!(doc.undo_label(), Some("Bevel edge"));
    // Too wide for a short edge's faces.
    let mut thin = map(vec![block(Vec3::ZERO, Vec3::new(8.0, 64.0, 8.0))]);
    let edge = ElementShape::Edge([Vec3::new(8.0, 0.0, 8.0), Vec3::new(8.0, 64.0, 8.0)]);
    pick(&mut t, &thin, vec![edge]);
    let refused = run(&mut t, &mut thin, &camera, ShapeOp::Bevel).unwrap_err();
    assert!(refused.contains("wider than the faces"), "{refused}");
}

#[test]
fn bridging_two_facing_faces_fills_the_gap() {
    let camera = angled();
    let mut doc = map(vec![
        block(Vec3::ZERO, Vec3::splat(64.0)),
        block(Vec3::new(128.0, 0.0, 0.0), Vec3::new(192.0, 64.0, 64.0)),
    ]);
    let ids: Vec<ObjectId> = doc.selection().iter().copied().collect();
    let face = |id: ObjectId, x: f32| {
        let b = brush_of(&doc, id).brush();
        let f = b
            .faces()
            .iter()
            .find(|f| f.vertices().iter().all(|v| v.x == x))
            .unwrap();
        PickedElement {
            brush: id,
            shape: ElementShape::Face(f.vertices().to_vec()),
        }
    };
    let mut t = tools(SelectMode::Face);
    t.elements.items = vec![face(ids[0], 64.0), face(ids[1], 128.0)];
    run(&mut t, &mut doc, &camera, ShapeOp::Bridge).unwrap();
    assert_eq!(doc.len(), 3);
    let new = brush_of(&doc, first_id(&doc));
    assert_eq!(
        new.brush().bounds(),
        Aabb::from_corners(Vec3::new(64.0, 0.0, 0.0), Vec3::new(128.0, 64.0, 64.0))
    );
    assert_eq!(doc.undo_label(), Some("Bridge"));
    // Two corners span nothing.
    t.elements.items.clear();
    t.elements.items.push(PickedElement {
        brush: ids[0],
        shape: ElementShape::Vertex(Vec3::ZERO),
    });
    assert!(run(&mut t, &mut doc, &camera, ShapeOp::Bridge).is_err());
}

#[test]
fn splitting_along_two_corners_seen_from_above_makes_two_halves() {
    let camera = above();
    let mut doc = one_box();
    let mut t = tools(SelectMode::Vertex);
    pick(
        &mut t,
        &doc,
        vec![
            ElementShape::Vertex(Vec3::new(0.0, 0.0, 128.0)),
            ElementShape::Vertex(Vec3::new(128.0, 64.0, 128.0)),
        ],
    );
    run(&mut t, &mut doc, &camera, ShapeOp::Split).unwrap();
    let all = brushes(&doc);
    assert_eq!(all.len(), 2);
    for b in &all {
        // The view is a little off straight down, so the halves differ a
        // little.
        let half = 128.0 * 64.0 * 64.0;
        assert!((b.brush().volume() - half).abs() < half * 0.1);
        assert_eq!(b.faces().len(), b.brush().faces().len());
    }
    assert_eq!(doc.selection().len(), 2, "both halves selected");
    assert!(t.picked_elements().is_empty());
    assert_eq!(doc.undo().as_deref(), Some("Split brush"));
    assert_eq!(brushes(&doc).len(), 1);
}

#[test]
fn clipping_through_three_corners_keeps_the_bigger_part() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(SelectMode::Vertex);
    // Three corners round the top front right corner: slices it off.
    pick(
        &mut t,
        &doc,
        vec![
            ElementShape::Vertex(Vec3::new(0.0, 0.0, 128.0)),
            ElementShape::Vertex(Vec3::new(128.0, 64.0, 128.0)),
            ElementShape::Vertex(Vec3::new(128.0, 0.0, 0.0)),
        ],
    );
    run(&mut t, &mut doc, &camera, ShapeOp::Clip).unwrap();
    let all = brushes(&doc);
    assert_eq!(all.len(), 1);
    let whole = 128.0 * 64.0 * 128.0;
    assert!((all[0].brush().volume() - whole * 5.0 / 6.0).abs() < 2.0);
    assert_eq!(doc.undo_label(), Some("Clip brush"));
}

#[test]
fn a_cut_along_a_face_cuts_the_other_brushes() {
    let camera = angled();
    // A thin floor, and a post standing through it.
    let mut doc = map(vec![
        block(Vec3::new(-64.0, -64.0, 0.0), Vec3::new(64.0, 64.0, 16.0)),
        block(Vec3::new(-8.0, -8.0, -64.0), Vec3::new(8.0, 8.0, 64.0)),
    ]);
    let floor = *doc.selection().iter().next().unwrap();
    let top = {
        let b = brush_of(&doc, floor).brush();
        let f = b.faces().iter().find(|f| f.plane().normal.z > 0.9).unwrap();
        ElementShape::Face(f.vertices().to_vec())
    };
    let mut t = tools(SelectMode::Face);
    t.elements.items = vec![PickedElement {
        brush: floor,
        shape: top,
    }];
    run(&mut t, &mut doc, &camera, ShapeOp::Clip).unwrap();
    let post = brushes(&doc)
        .into_iter()
        .find(|b| b.brush().bounds().size().x == 16.0)
        .unwrap()
        .brush()
        .bounds();
    assert_eq!(post.max.z, 16.0, "the post was trimmed at the floor's top");
    assert_eq!(post.min.z, -64.0, "keeping its bigger part");
}

#[test]
fn a_cut_that_misses_is_refused() {
    let mut doc = one_box();
    let mut t = tools(SelectMode::Vertex);
    pick(&mut t, &doc, vec![ElementShape::Vertex(Vec3::ZERO)]);
    assert!(
        run(&mut t, &mut doc, &above(), ShapeOp::Split).is_err(),
        "one corner"
    );
    // A face's own plane only touches its brush.
    let mut faces = tools(SelectMode::Face);
    pick(&mut faces, &doc, vec![top_face(&doc)]);
    let refused = run(&mut faces, &mut doc, &above(), ShapeOp::Split).unwrap_err();
    assert!(refused.contains("doesn't pass through"), "{refused}");
}

#[test]
fn joining_touching_boxes_makes_one_and_gaps_are_mentioned() {
    let camera = angled();
    let mut doc = map(vec![
        block(Vec3::ZERO, Vec3::splat(64.0)),
        block(Vec3::new(64.0, 0.0, 0.0), Vec3::new(128.0, 64.0, 64.0)),
    ]);
    let mut t = tools(SelectMode::Object);
    let note = run(&mut t, &mut doc, &camera, ShapeOp::Join).unwrap();
    assert_eq!(note, None);
    let all = brushes(&doc);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].brush().faces().len(), 6);
    assert_eq!(doc.selection().len(), 1);
    assert_eq!(doc.undo().as_deref(), Some("Join 2 brushes"));
    assert_eq!(brushes(&doc).len(), 2);
    let mut apart = map(vec![
        block(Vec3::ZERO, Vec3::splat(64.0)),
        block(Vec3::new(128.0, 0.0, 0.0), Vec3::new(192.0, 64.0, 128.0)),
    ]);
    let note = run(&mut t, &mut apart, &camera, ShapeOp::Join).unwrap();
    assert!(note.unwrap().contains("space between"));
}

#[test]
fn merging_two_corners_and_deleting_parts() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(SelectMode::Vertex);
    pick(
        &mut t,
        &doc,
        vec![
            ElementShape::Vertex(Vec3::new(128.0, 0.0, 128.0)),
            ElementShape::Vertex(Vec3::new(128.0, 64.0, 128.0)),
        ],
    );
    run(&mut t, &mut doc, &camera, ShapeOp::MergeCorners).unwrap();
    let b = brush_of(&doc, first_id(&doc)).brush();
    assert_eq!(b.points().len(), 7);
    assert!(b.points().contains(&Vec3::new(128.0, 32.0, 128.0)));
    assert_eq!(
        t.picked_elements()[0].shape,
        ElementShape::Vertex(Vec3::new(128.0, 32.0, 128.0))
    );
    assert_eq!(doc.undo_label(), Some("Merge 2 vertices"));
    run(&mut t, &mut doc, &camera, ShapeOp::DeleteParts).unwrap();
    assert_eq!(brush_of(&doc, first_id(&doc)).brush().points().len(), 6);
    assert_eq!(doc.undo_label(), Some("Delete vertex"));
    // A box's face can't go: the box would be open.
    let mut boxed = one_box();
    let mut faces = tools(SelectMode::Face);
    pick(&mut faces, &boxed, vec![top_face(&boxed)]);
    let refused = run(&mut faces, &mut boxed, &camera, ShapeOp::DeleteParts).unwrap_err();
    assert!(refused.contains("open"), "{refused}");
}

#[test]
fn mirroring_flips_the_selection_about_its_middle() {
    let mut doc = map(vec![
        block(Vec3::ZERO, Vec3::splat(64.0)),
        block(Vec3::new(64.0, 0.0, 0.0), Vec3::new(96.0, 64.0, 128.0)),
    ]);
    let tall_x = |doc: &Document| {
        brushes(doc)
            .into_iter()
            .find(|b| b.brush().bounds().size().z == 128.0)
            .unwrap()
            .brush()
            .bounds()
            .min
            .x
    };
    let mut t = tools(SelectMode::Object);
    run(&mut t, &mut doc, &angled(), ShapeOp::Mirror(Axis::X)).unwrap();
    assert_eq!(tall_x(&doc), 0.0, "the tall box swapped sides");
    assert_eq!(doc.undo_label(), Some("Mirror 2 brushes"));
    // From above with the default camera's right along X, the same again.
    run(&mut t, &mut doc, &above(), ShapeOp::MirrorAcrossView).unwrap();
    assert_eq!(tall_x(&doc), 64.0);
}

#[test]
fn dragging_a_box_picks_every_corner_inside_it() {
    let camera = angled();
    let doc = one_box();
    let mut t = tools(SelectMode::Vertex);
    let screen = |p: Vec3| camera.project(p, SIZE).unwrap();
    // A box around the two right-hand top corners.
    let a = screen(Vec3::new(128.0, 0.0, 128.0));
    let b = screen(Vec3::new(128.0, 64.0, 128.0));
    let (from, to) = (a.min(b) - Vec2::splat(6.0), a.max(b) + Vec2::splat(6.0));
    let at = |p: Vec2| ToolInput {
        size: SIZE,
        cursor: Some(p),
        ..ToolInput::default()
    };
    let press = ToolInput {
        pressed: true,
        held: true,
        ..at(from)
    };
    t.update(&press, &camera, &doc);
    t.update(
        &ToolInput {
            held: true,
            ..at(to)
        },
        &camera,
        &doc,
    );
    assert!(t.marquee().is_some(), "the box is drawn");
    t.update(
        &ToolInput {
            released: true,
            ..at(to)
        },
        &camera,
        &doc,
    );
    assert!(t.marquee().is_none());
    let picked: Vec<&ElementShape> = t.picked_elements().iter().map(|p| &p.shape).collect();
    assert!(picked.contains(&&ElementShape::Vertex(Vec3::new(128.0, 0.0, 128.0))));
    assert!(picked.contains(&&ElementShape::Vertex(Vec3::new(128.0, 64.0, 128.0))));
    assert!(picked.len() <= 3, "{picked:?}");
}
