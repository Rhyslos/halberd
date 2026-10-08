//! Tests for the transform gizmo, driven through the tool controller like
//! the interface does, with the resulting edits applied to a real map.

use super::*;
use crate::{Tool, ToolController, ToolInput};
use halberd_doc::{Command, ObjectId};
use halberd_geom::Brush;

const SIZE: Vec2 = Vec2::new(1600.0, 900.0);

/// A camera above and to the side, so every handle can be seen.
fn angled() -> Camera {
    Camera::looking_at(Vec3::new(500.0, -700.0, 600.0), Vec3::new(64.0, 32.0, 64.0))
}

/// A map with one 128 × 64 × 128 box at the origin, selected.
fn one_box() -> Document {
    let mut doc = Document::new();
    let brush = Brush::cuboid(Aabb::from_corners(
        Vec3::ZERO,
        Vec3::new(128.0, 64.0, 128.0),
    ))
    .unwrap();
    doc.execute(Command::AddBrushes(vec![brush])).unwrap();
    doc
}

fn tools(mode: GizmoMode) -> ToolController {
    let mut t = ToolController::new(16.0);
    t.set_gizmo_mode(Some(mode));
    t
}

fn input(cursor: Vec2) -> ToolInput {
    ToolInput {
        size: SIZE,
        cursor: Some(cursor),
        ..ToolInput::default()
    }
}

/// Carries out a tool action on the map, as the interface does.
fn apply(doc: &mut Document, action: Option<ToolAction>) {
    match action {
        Some(ToolAction::ExecuteMerging(command, key)) => {
            doc.execute_merging(command, key).ok();
        }
        Some(ToolAction::CancelMerging(key)) => {
            doc.discard_step(key);
        }
        Some(ToolAction::Select(id)) => doc.set_selection(id),
        Some(other) => panic!("unexpected action {other:?}"),
        None => {}
    }
}

/// A point to grab `handle` by: the middle of its first line, or the
/// middle of its first filled shape.
fn grab_point(t: &ToolController, camera: &Camera, doc: &Document, handle: Handle) -> Vec2 {
    let shapes = t.gizmo_shapes(camera, SIZE, doc);
    let shape = shapes
        .iter()
        .find(|s| s.handle == handle)
        .unwrap_or_else(|| panic!("no shape for {handle:?}"));
    if shape.filled {
        shape.points.iter().copied().sum::<Vec2>() / shape.points.len() as f32
    } else if matches!(handle, Handle::Rotate(_)) {
        shape.points[6]
    } else {
        (shape.points[0] + shape.points[1]) * 0.5
    }
}

/// Presses at `from`, moves through `path` and releases, applying every
/// edit to the map. Returns the last edit's key, if any.
fn drag(
    t: &mut ToolController,
    camera: &Camera,
    doc: &mut Document,
    from: Vec2,
    path: &[Vec2],
) -> Option<u64> {
    doc.end_step();
    let press = ToolInput {
        pressed: true,
        held: true,
        ..input(from)
    };
    let action = t.update(&press, camera, doc);
    assert_eq!(action, None, "grabbing a handle changes nothing yet");
    assert!(t.is_busy(), "the press grabbed a handle");
    let mut key = None;
    for &to in path {
        let moving = ToolInput {
            held: true,
            ..input(to)
        };
        let action = t.update(&moving, camera, doc);
        if let Some(ToolAction::ExecuteMerging(_, k)) = &action {
            key = Some(*k);
        }
        apply(doc, action);
    }
    let end = *path.last().unwrap_or(&from);
    let release = ToolInput {
        released: true,
        ..input(end)
    };
    apply(doc, t.update(&release, camera, doc));
    assert!(!t.is_busy());
    doc.end_step();
    key
}

fn bounds(doc: &Document) -> Aabb {
    doc.objects().next().unwrap().1.bounds().unwrap()
}

/// Screen position of a world point.
fn screen(camera: &Camera, p: Vec3) -> Vec2 {
    camera.project(p, SIZE).unwrap()
}

#[test]
fn no_gizmo_without_a_mode_a_selection_or_the_select_tool() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = ToolController::new(16.0);
    assert!(t.gizmo_shapes(&camera, SIZE, &doc).is_empty(), "no mode");
    t.set_gizmo_mode(Some(GizmoMode::Move));
    assert!(!t.gizmo_shapes(&camera, SIZE, &doc).is_empty());
    t.set_tool(Tool::Box);
    assert!(t.gizmo_shapes(&camera, SIZE, &doc).is_empty(), "Box tool");
    assert_eq!(t.gizmo_mode(), None, "the Box tool turns the gizmo off");
    t.set_gizmo_mode(Some(GizmoMode::Move));
    assert_eq!(t.tool(), Tool::Select, "picking a mode picks Select");
    doc.clear_selection();
    assert!(
        t.gizmo_shapes(&camera, SIZE, &doc).is_empty(),
        "nothing selected"
    );
}

#[test]
fn mode_keys_are_toggles() {
    let mut t = ToolController::new(16.0);
    t.toggle_gizmo_mode(GizmoMode::Rotate);
    assert_eq!(t.gizmo_mode(), Some(GizmoMode::Rotate));
    t.toggle_gizmo_mode(GizmoMode::Scale);
    assert_eq!(t.gizmo_mode(), Some(GizmoMode::Scale));
    t.toggle_gizmo_mode(GizmoMode::Scale);
    assert_eq!(t.gizmo_mode(), None, "the same key again: plain selection");
}

#[test]
fn each_mode_shows_its_handles() {
    let (camera, doc) = (angled(), one_box());
    let handles = |mode| {
        let shapes = tools(mode).gizmo_shapes(&camera, SIZE, &doc);
        let mut list: Vec<Handle> = shapes.iter().map(|s| s.handle).collect();
        list.dedup();
        list
    };
    let moving = handles(GizmoMode::Move);
    for axis in Axis::ALL {
        assert!(moving.contains(&Handle::MoveAxis(axis)));
        assert!(moving.contains(&Handle::MovePlane(axis)));
    }
    let rotating = handles(GizmoMode::Rotate);
    assert_eq!(rotating.len(), 3);
    assert!(rotating.iter().all(|h| matches!(h, Handle::Rotate(_))));
    let scaling = handles(GizmoMode::Scale);
    assert!(scaling.contains(&Handle::ScaleUniform));
    assert!(scaling.contains(&Handle::ScaleAxis(Axis::Z)));
    assert!(!scaling.iter().any(|h| matches!(h, Handle::MoveAxis(_))));
    let all = handles(GizmoMode::All);
    assert!(all.contains(&Handle::MoveAxis(Axis::X)));
    assert!(all.contains(&Handle::Rotate(Axis::Y)));
    assert!(all.contains(&Handle::ScaleAxis(Axis::Z)));
}

#[test]
fn the_gizmo_looks_the_same_size_near_and_far() {
    let doc = one_box();
    let t = tools(GizmoMode::Move);
    let arrow_length = |camera: Camera| {
        let shapes = t.gizmo_shapes(&camera, SIZE, &doc);
        let line = shapes
            .iter()
            .find(|s| s.handle == Handle::MoveAxis(Axis::X) && !s.filled)
            .unwrap();
        line.points[0].distance(line.points[1])
    };
    let centre = Vec3::new(64.0, 32.0, 64.0);
    let near = arrow_length(Camera::looking_at(
        centre + Vec3::new(200.0, -300.0, 200.0),
        centre,
    ));
    let far = arrow_length(Camera::looking_at(
        centre + Vec3::new(2000.0, -3000.0, 2000.0),
        centre,
    ));
    assert!((near - far).abs() < near * 0.15, "{near} vs {far}");
}

#[test]
fn an_arrow_pointing_at_the_camera_is_hidden() {
    let doc = one_box();
    let above = Camera::looking_at(Vec3::new(64.0, 31.0, 2000.0), Vec3::new(64.0, 32.0, 64.0));
    let shapes = tools(GizmoMode::Move).gizmo_shapes(&above, SIZE, &doc);
    assert!(!shapes.iter().any(|s| s.handle == Handle::MoveAxis(Axis::Z)));
    assert!(
        shapes
            .iter()
            .any(|s| s.handle == Handle::MovePlane(Axis::Z))
    );
}

#[test]
fn dragging_an_arrow_moves_along_it_in_grid_steps() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(GizmoMode::Move);
    let from = grab_point(&t, &camera, &doc, Handle::MoveAxis(Axis::X));
    // Move the pointer the screen distance of 100 units along X.
    let step = screen(&camera, Vec3::new(164.0, 32.0, 64.0))
        - screen(&camera, Vec3::new(64.0, 32.0, 64.0));
    drag(
        &mut t,
        &camera,
        &mut doc,
        from,
        &[from + step * 0.5, from + step],
    );
    let b = bounds(&doc);
    assert_eq!(b.min, Vec3::new(96.0, 0.0, 0.0), "100 units snaps to 96");
    assert_eq!(b.size(), Vec3::new(128.0, 64.0, 128.0));
    assert_eq!(doc.undo().as_deref(), Some("Move brush"));
    assert_eq!(bounds(&doc).min, Vec3::ZERO, "the whole drag is one step");
}

#[test]
fn dragging_a_square_moves_across_its_plane() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(GizmoMode::Move);
    let from = grab_point(&t, &camera, &doc, Handle::MovePlane(Axis::Z));
    // The world point under the grab, on the square's plane (Z = 64).
    let ray = camera.ray_through(from, SIZE);
    let grabbed = ray.at(ray.hit_horizontal_plane(64.0).unwrap());
    let to = screen(&camera, grabbed + Vec3::new(-64.0, 128.0, 0.0));
    drag(&mut t, &camera, &mut doc, from, &[to]);
    let b = bounds(&doc);
    assert_eq!(b.min, Vec3::new(-64.0, 128.0, 0.0), "moved in X and Y only");
}

#[test]
fn dragging_a_ring_rotates_in_fifteen_degree_steps() {
    // Two boxes either side of the middle, so the direction of the turn
    // shows: a quarter turn counter-clockwise seen from above (Z points at
    // the camera) takes the box on the +X side to the +Y side.
    let camera = Camera::looking_at(Vec3::new(30.0, -60.0, 1500.0), Vec3::ZERO);
    let mut doc = Document::new();
    let cube = |x: f32| {
        Brush::cuboid(Aabb::from_corners(
            Vec3::new(x - 16.0, -16.0, 0.0),
            Vec3::new(x + 16.0, 16.0, 32.0),
        ))
        .unwrap()
    };
    doc.execute(Command::AddBrushes(vec![cube(100.0), cube(-100.0)]))
        .unwrap();
    let right = doc.objects().next().unwrap().0;
    let mut t = tools(GizmoMode::Rotate);
    let from = grab_point(&t, &camera, &doc, Handle::Rotate(Axis::Z));
    let pivot = screen(&camera, Vec3::new(0.0, 0.0, 16.0));
    let arm = from - pivot;
    // A quarter turn counter-clockwise on screen (y points down), in steps.
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
    let moved = doc.get(right).unwrap().bounds().unwrap().center();
    assert!(
        (moved - Vec3::new(0.0, 100.0, 16.0)).length() < 0.5,
        "{moved}"
    );
    assert_eq!(doc.undo().as_deref(), Some("Rotate 2 brushes"));
}

#[test]
fn small_turns_snap_to_nothing() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(GizmoMode::Rotate);
    let from = grab_point(&t, &camera, &doc, Handle::Rotate(Axis::Z));
    drag(
        &mut t,
        &camera,
        &mut doc,
        from,
        &[from + Vec2::new(1.0, 1.0)],
    );
    assert_eq!(
        doc.undo_label(),
        Some("Create brush"),
        "under 7.5°: no edit"
    );
}

#[test]
fn dragging_a_cube_stretches_one_side() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(GizmoMode::Scale);
    let from = grab_point(&t, &camera, &doc, Handle::ScaleAxis(Axis::X));
    let centre = Vec3::new(64.0, 32.0, 64.0);
    let step = screen(&camera, centre + Vec3::new(64.0, 0.0, 0.0)) - screen(&camera, centre);
    drag(&mut t, &camera, &mut doc, from, &[from + step]);
    let b = bounds(&doc);
    assert_eq!(b.min, Vec3::ZERO, "the far side stays put");
    assert_eq!(b.size(), Vec3::new(192.0, 64.0, 128.0));
    assert_eq!(doc.undo().as_deref(), Some("Scale brush"));
}

#[test]
fn shrinking_stops_at_one_grid_square() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(GizmoMode::Scale);
    let from = grab_point(&t, &camera, &doc, Handle::ScaleAxis(Axis::Z));
    let centre = Vec3::new(64.0, 32.0, 64.0);
    let step = screen(&camera, centre - Vec3::new(0.0, 0.0, 600.0)) - screen(&camera, centre);
    drag(&mut t, &camera, &mut doc, from, &[from + step]);
    assert_eq!(bounds(&doc).size().z, 16.0);
    assert_eq!(bounds(&doc).min.z, 0.0);
}

#[test]
fn the_middle_cube_resizes_evenly_from_the_floor() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(GizmoMode::Scale);
    let from = grab_point(&t, &camera, &doc, Handle::ScaleUniform);
    drag(
        &mut t,
        &camera,
        &mut doc,
        from,
        &[from + Vec2::new(100.0, 0.0)],
    );
    let b = bounds(&doc);
    assert_eq!(b.size(), Vec3::new(256.0, 128.0, 256.0), "doubled");
    assert_eq!(b.min.z, 0.0, "still standing on the floor");
    assert_eq!(b.center().truncate(), glam::Vec2::new(64.0, 32.0));
}

#[test]
fn escape_cancels_a_drag_and_leaves_no_trace() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(GizmoMode::Move);
    doc.end_step();
    let from = grab_point(&t, &camera, &doc, Handle::MoveAxis(Axis::Y));
    let press = ToolInput {
        pressed: true,
        held: true,
        ..input(from)
    };
    let action = t.update(&press, &camera, &doc);
    apply(&mut doc, action);
    let moving = ToolInput {
        held: true,
        ..input(from + Vec2::new(-150.0, -60.0))
    };
    let action = t.update(&moving, &camera, &doc);
    apply(&mut doc, action);
    assert_ne!(bounds(&doc).min, Vec3::ZERO, "it moved");
    let escape = ToolInput {
        held: true,
        cancel: true,
        ..input(from + Vec2::new(-150.0, -60.0))
    };
    let action = t.update(&escape, &camera, &doc);
    assert!(matches!(action, Some(ToolAction::CancelMerging(_))));
    apply(&mut doc, action);
    assert_eq!(bounds(&doc).min, Vec3::ZERO, "back where it started");
    assert_eq!(doc.undo_label(), Some("Create brush"));
    assert!(!t.is_busy());
}

#[test]
fn clicking_a_handle_keeps_the_selection_and_clicking_away_still_selects() {
    let camera = angled();
    let mut doc = one_box();
    let id: ObjectId = doc.objects().next().unwrap().0;
    let mut t = tools(GizmoMode::Move);
    let on_handle = grab_point(&t, &camera, &doc, Handle::MoveAxis(Axis::X));
    drag(&mut t, &camera, &mut doc, on_handle, &[]);
    assert!(
        doc.is_selected(id),
        "grabbing and letting go selects nothing new"
    );
    // Far away in the sky: the select tool clears the selection.
    let sky = Vec2::new(5.0, 5.0);
    let press = ToolInput {
        pressed: true,
        held: true,
        ..input(sky)
    };
    let action = t.update(&press, &camera, &doc);
    apply(&mut doc, action);
    let release = ToolInput {
        released: true,
        ..input(sky)
    };
    let action = t.update(&release, &camera, &doc);
    apply(&mut doc, action);
    assert!(doc.selection().is_empty());
}

#[test]
fn hovering_a_handle_highlights_it() {
    let camera = angled();
    let doc = one_box();
    let mut t = tools(GizmoMode::Move);
    let at = grab_point(&t, &camera, &doc, Handle::MoveAxis(Axis::Z));
    t.update(&input(at), &camera, &doc);
    let shapes = t.gizmo_shapes(&camera, SIZE, &doc);
    for shape in shapes {
        assert_eq!(shape.highlighted, shape.handle == Handle::MoveAxis(Axis::Z));
    }
}

#[test]
fn random_drags_never_break_the_map() {
    // Stress test: random grabs and pointer paths in every mode. Every edit
    // the gizmo asks for must be accepted by the map, and every brush must
    // stay a valid solid.
    let mut seed: u64 = 0x61A0_5EED_0000_0001;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % 20_001) as f32 / 10_000.0 - 1.0
    };
    let camera = angled();
    let mut doc = one_box();
    for round in 0..300 {
        let mode = GizmoMode::ALL[round % 4];
        let mut t = tools(mode);
        let shapes = t.gizmo_shapes(&camera, SIZE, &doc);
        if shapes.is_empty() {
            continue;
        }
        let pick = &shapes[((next() + 1.0) * 0.5 * (shapes.len() - 1) as f32) as usize];
        let from = pick.points[0];
        let path: Vec<Vec2> = (0..8)
            .map(|_| from + Vec2::new(next(), next()) * 400.0)
            .collect();
        doc.end_step();
        let press = ToolInput {
            pressed: true,
            held: true,
            ..input(from)
        };
        let action = t.update(&press, &camera, &doc);
        apply(&mut doc, action);
        for to in path {
            let moving = ToolInput {
                held: true,
                ..input(to)
            };
            match t.update(&moving, &camera, &doc) {
                Some(ToolAction::ExecuteMerging(command, key)) => {
                    match doc.execute_merging(command, key) {
                        Ok(()) | Err(halberd_doc::DocError::NothingToDo) => {}
                        Err(e) => panic!("the map refused a gizmo edit: {e}"),
                    }
                }
                other => apply(&mut doc, other),
            }
        }
        let release = ToolInput {
            released: true,
            ..input(from)
        };
        let action = t.update(&release, &camera, &doc);
        apply(&mut doc, action);
        for (_, object) in doc.objects() {
            let brush = object.as_brush().unwrap().brush();
            assert!(brush.faces().len() >= 4);
            assert!(brush.bounds().min.is_finite());
        }
        // Keep the box in view: undo every few rounds.
        if round % 5 == 4 {
            while doc.undo_label() != Some("Create brush") {
                doc.undo();
            }
        }
    }
}

/// Grabs `handle` (pressing at its grab point) and moves the pointer by
/// `by`, applying the edits. Leaves the drag in progress.
fn start_drag(
    t: &mut ToolController,
    camera: &Camera,
    doc: &mut Document,
    handle: Handle,
    by: Vec2,
) -> Vec2 {
    doc.end_step();
    let from = grab_point(t, camera, doc, handle);
    let press = ToolInput {
        pressed: true,
        held: true,
        ..input(from)
    };
    let action = t.update(&press, camera, doc);
    apply(doc, action);
    let moving = ToolInput {
        held: true,
        ..input(from + by)
    };
    let action = t.update(&moving, camera, doc);
    apply(doc, action);
    from + by
}

#[test]
fn switching_tools_mid_drag_is_refused() {
    // Regression: B during a gizmo drag left a stuck drag that blocked the
    // gizmo keys and Escape.
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(GizmoMode::Move);
    let at = start_drag(
        &mut t,
        &camera,
        &mut doc,
        Handle::MoveAxis(Axis::X),
        Vec2::new(80.0, 0.0),
    );
    t.set_tool(Tool::Box);
    assert_eq!(t.tool(), Tool::Select, "the drag finishes first");
    let release = ToolInput {
        released: true,
        ..input(at)
    };
    let action = t.update(&release, &camera, &doc);
    apply(&mut doc, action);
    assert!(!t.is_busy());
    t.set_tool(Tool::Box);
    assert_eq!(t.tool(), Tool::Box);
}

#[test]
fn a_drag_survives_the_camera_passing_the_selection() {
    // Regression: a frame with the selection behind the camera cancelled
    // the whole drag.
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(GizmoMode::Move);
    let at = start_drag(
        &mut t,
        &camera,
        &mut doc,
        Handle::MoveAxis(Axis::X),
        Vec2::new(80.0, 0.0),
    );
    let moved = bounds(&doc);
    assert_ne!(moved.min, Vec3::ZERO);
    let behind = Camera::looking_at(Vec3::new(64.0, 300.0, 64.0), Vec3::new(64.0, 600.0, 64.0));
    let held = ToolInput {
        held: true,
        ..input(at)
    };
    let action = t.update(&held, &behind, &doc);
    assert_eq!(action, None, "the drag waits");
    assert!(t.is_busy());
    assert_eq!(bounds(&doc), moved, "and keeps what was done");
}

#[test]
fn a_handle_that_cannot_be_grabbed_leaves_the_click_to_selection() {
    // Looking almost along a move square's plane, the ray never meets it;
    // the click must still select what is under the pointer.
    let mut doc = one_box();
    let id = doc.objects().next().unwrap().0;
    doc.clear_selection();
    let camera = angled();
    let mut t = tools(GizmoMode::Move);
    let on_box = screen(&camera, Vec3::new(64.0, 0.0, 64.0));
    let press = ToolInput {
        pressed: true,
        held: true,
        ..input(on_box)
    };
    let action = t.update(&press, &camera, &doc);
    apply(&mut doc, action);
    let release = ToolInput {
        released: true,
        ..input(on_box)
    };
    let action = t.update(&release, &camera, &doc);
    apply(&mut doc, action);
    assert!(doc.is_selected(id));
}

#[test]
fn rings_partly_behind_the_camera_are_not_drawn() {
    // Regression: points behind the camera were dropped and the rest joined
    // with a false line across the screen.
    let doc = one_box();
    let t = tools(GizmoMode::Rotate);
    let centre = Vec3::new(64.0, 32.0, 64.0);
    let close = Camera::looking_at(centre + Vec3::new(0.0, -3.0, 0.5), centre);
    for shape in t.gizmo_shapes(&close, Vec2::new(1600.0, 40.0), &doc) {
        assert_eq!(shape.points.len(), 49, "a ring is whole or not there");
    }
}

#[test]
fn entities_alone_get_no_gizmo() {
    use halberd_doc::{EntityObject, MapObject};
    let camera = angled();
    let light = MapObject::Entity(
        EntityObject {
            classname: "light".into(),
            origin: Some(Vec3::new(64.0, 32.0, 64.0)),
            file_data: Vec::new(),
        },
        Vec::new(),
    );
    let mut doc = Document::from_map(vec![light], Default::default()).unwrap();
    let id = doc.objects().next().unwrap().0;
    doc.set_selection([id]);
    let t = tools(GizmoMode::Move);
    assert!(t.gizmo_shapes(&camera, SIZE, &doc).is_empty());
}

#[test]
fn a_brush_picked_inside_an_entity_gets_the_gizmo_but_the_entity_does_not() {
    use halberd_doc::{BrushObject, EntityObject, MapObject};
    let camera = angled();
    let cube = Brush::cuboid(Aabb::from_corners(Vec3::ZERO, Vec3::splat(64.0))).unwrap();
    let detail = MapObject::Entity(
        EntityObject {
            classname: "func_detail".into(),
            origin: None,
            file_data: Vec::new(),
        },
        vec![BrushObject::new(cube)],
    );
    let mut doc = Document::from_map(vec![detail], Default::default()).unwrap();
    let entity = doc.objects().next().unwrap().0;
    let t = tools(GizmoMode::Move);
    doc.set_selection([entity]);
    assert!(
        t.gizmo_shapes(&camera, SIZE, &doc).is_empty(),
        "entities can't be moved yet"
    );
    let brush = doc.brushes_of(entity).next().unwrap();
    doc.set_selection([brush]);
    assert!(!t.gizmo_shapes(&camera, SIZE, &doc).is_empty());
}
