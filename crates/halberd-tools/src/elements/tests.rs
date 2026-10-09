//! Tests for picking corners, edges and faces, and moving them with the
//! gizmo, driven through the tool controller like the interface does.

use super::*;
use crate::{Axis, GizmoMode, Handle, Tool, ToolAction, ToolController, ToolInput};
use halberd_doc::Command;

const SIZE: Vec2 = Vec2::new(1600.0, 900.0);

/// A camera above and to the side, so every corner can be seen.
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

fn tools(mode: SelectMode) -> ToolController {
    let mut t = ToolController::new(16.0);
    t.set_select_mode(mode);
    t
}

fn input(cursor: Vec2) -> ToolInput {
    ToolInput {
        size: SIZE,
        cursor: Some(cursor),
        ..ToolInput::default()
    }
}

fn screen(camera: &Camera, p: Vec3) -> Vec2 {
    camera.project(p, SIZE).unwrap()
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
        Some(ToolAction::ToggleSelected(id)) => doc.toggle_selected(id),
        Some(other) => panic!("unexpected action {other:?}"),
        None => {}
    }
}

/// Runs one frame of tool input and applies what it asks.
fn step(t: &mut ToolController, camera: &Camera, doc: &mut Document, input: &ToolInput) {
    let action = t.update(input, camera, doc);
    apply(doc, action);
}

/// Hovers, then clicks at `at` (with Ctrl if `additive`).
fn click(t: &mut ToolController, camera: &Camera, doc: &mut Document, at: Vec2, additive: bool) {
    let hover = input(at);
    step(t, camera, doc, &hover);
    let press = ToolInput {
        pressed: true,
        held: true,
        additive,
        ..input(at)
    };
    step(t, camera, doc, &press);
    let release = ToolInput {
        released: true,
        additive,
        ..input(at)
    };
    step(t, camera, doc, &release);
}

/// The brush in the map.
fn the_brush(doc: &Document) -> Brush {
    doc.objects()
        .find_map(|(_, o)| o.as_brush().map(|b| b.brush().clone()))
        .unwrap()
}

/// Grabs `handle` and drags it by the screen distance of `world` units
/// from the gizmo's middle, applying every edit. Returns false if the
/// handle could not be grabbed.
fn drag_handle(
    t: &mut ToolController,
    camera: &Camera,
    doc: &mut Document,
    handle: Handle,
    world: Vec3,
) -> bool {
    doc.end_step();
    let shapes = t.gizmo_shapes(camera, SIZE, doc);
    let Some(shape) = shapes.iter().find(|s| s.handle == handle) else {
        return false;
    };
    let from = if shape.filled {
        shape.points.iter().copied().sum::<Vec2>() / shape.points.len() as f32
    } else {
        (shape.points[0] + shape.points[1]) * 0.5
    };
    let pivot = t.picked_bounds().unwrap().center();
    let delta = screen(camera, pivot + world) - screen(camera, pivot);
    let press = ToolInput {
        pressed: true,
        held: true,
        ..input(from)
    };
    step(t, camera, doc, &press);
    for k in 1..=4 {
        let moving = ToolInput {
            held: true,
            ..input(from + delta * (k as f32 / 4.0))
        };
        step(t, camera, doc, &moving);
    }
    let release = ToolInput {
        released: true,
        ..input(from + delta)
    };
    step(t, camera, doc, &release);
    doc.end_step();
    true
}

#[test]
fn modes_have_names_keys_and_kinds() {
    let keys: Vec<&str> = SelectMode::ALL.iter().map(|m| m.key()).collect();
    assert_eq!(keys, ["1", "2", "3", "4"]);
    assert_eq!(SelectMode::Object.element(), None);
    assert_eq!(SelectMode::Vertex.element(), Some(ElementKind::Vertex));
    assert_eq!(SelectMode::Face.label(), "Face");
    assert!(SelectMode::ALL.iter().all(|m| !m.description().is_empty()));
    assert_eq!(SelectMode::default(), SelectMode::Object);
}

#[test]
fn clicking_corners_picks_them_and_ctrl_click_adds() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(SelectMode::Vertex);
    let corner = Vec3::new(128.0, 0.0, 128.0);
    click(&mut t, &camera, &mut doc, screen(&camera, corner), false);
    assert_eq!(t.picked_elements().len(), 1);
    assert_eq!(t.picked_elements()[0].shape, ElementShape::Vertex(corner));
    let other = Vec3::new(0.0, 0.0, 128.0);
    click(&mut t, &camera, &mut doc, screen(&camera, other), true);
    assert_eq!(t.picked_elements().len(), 2, "Ctrl+click adds");
    click(&mut t, &camera, &mut doc, screen(&camera, other), true);
    assert_eq!(
        t.picked_elements().len(),
        1,
        "Ctrl+click again takes it out"
    );
    // Clicking empty sky lets go of the corners but keeps the brush.
    click(&mut t, &camera, &mut doc, Vec2::new(5.0, 5.0), false);
    assert!(t.picked_elements().is_empty());
    assert_eq!(doc.selection().len(), 1);
}

#[test]
fn edges_and_faces_are_picked_too() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(SelectMode::Edge);
    let (a, b) = (Vec3::new(128.0, 0.0, 128.0), Vec3::new(128.0, 0.0, 0.0));
    let middle = (screen(&camera, a) + screen(&camera, b)) * 0.5;
    click(&mut t, &camera, &mut doc, middle, false);
    match &t.picked_elements()[0].shape {
        ElementShape::Edge(ends) => {
            assert!(ends.contains(&a) && ends.contains(&b));
        }
        other => panic!("picked {other:?}"),
    }
    t.set_select_mode(SelectMode::Face);
    assert!(t.picked_elements().is_empty(), "a new mode starts afresh");
    let top_middle = screen(&camera, Vec3::new(64.0, 32.0, 128.0));
    click(&mut t, &camera, &mut doc, top_middle, false);
    match &t.picked_elements()[0].shape {
        ElementShape::Face(corners) => {
            assert_eq!(corners.len(), 4);
            assert!(corners.iter().all(|c| c.z == 128.0), "the top face");
        }
        other => panic!("picked {other:?}"),
    }
}

#[test]
fn with_nothing_selected_a_click_selects_the_brush() {
    let camera = angled();
    let mut doc = one_box();
    doc.clear_selection();
    let mut t = tools(SelectMode::Vertex);
    let corner = screen(&camera, Vec3::new(128.0, 0.0, 128.0));
    // No corners to pick yet; the click lands on the brush and selects it.
    click(
        &mut t,
        &camera,
        &mut doc,
        corner - Vec2::new(0.0, -20.0),
        false,
    );
    assert_eq!(doc.selection().len(), 1);
    assert!(t.picked_elements().is_empty());
    click(&mut t, &camera, &mut doc, corner, false);
    assert_eq!(
        t.picked_elements().len(),
        1,
        "now its corners can be picked"
    );
}

#[test]
fn the_overlay_shows_what_can_be_picked() {
    let camera = angled();
    let doc = one_box();
    let t = tools(SelectMode::Vertex);
    let overlay = t.element_overlay(&camera, SIZE, &doc).unwrap();
    assert_eq!(overlay.candidates.len(), 8);
    let t = tools(SelectMode::Edge);
    assert_eq!(
        t.element_overlay(&camera, SIZE, &doc)
            .unwrap()
            .candidates
            .len(),
        12
    );
    let t = tools(SelectMode::Object);
    assert!(t.element_overlay(&camera, SIZE, &doc).is_none());
    let mut t = tools(SelectMode::Vertex);
    t.set_tool(Tool::Box);
    assert!(
        t.element_overlay(&camera, SIZE, &doc).is_none(),
        "Draw tool"
    );
}

#[test]
fn the_gizmo_waits_for_a_pick() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(SelectMode::Face);
    t.set_gizmo_mode(Some(GizmoMode::Move));
    assert_eq!(
        t.select_mode(),
        SelectMode::Face,
        "the gizmo keeps the mode"
    );
    assert!(t.gizmo_shapes(&camera, SIZE, &doc).is_empty());
    click(
        &mut t,
        &camera,
        &mut doc,
        screen(&camera, Vec3::new(64.0, 32.0, 128.0)),
        false,
    );
    assert!(!t.gizmo_shapes(&camera, SIZE, &doc).is_empty());
}

#[test]
fn moving_the_top_face_up_makes_the_brush_taller_in_one_step() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(SelectMode::Face);
    t.set_gizmo_mode(Some(GizmoMode::Move));
    click(
        &mut t,
        &camera,
        &mut doc,
        screen(&camera, Vec3::new(64.0, 32.0, 128.0)),
        false,
    );
    assert!(drag_handle(
        &mut t,
        &camera,
        &mut doc,
        Handle::MoveAxis(Axis::Z),
        Vec3::new(0.0, 0.0, 64.0)
    ));
    let b = the_brush(&doc).bounds();
    assert_eq!(b.max.z, 192.0);
    assert_eq!(b.min, Vec3::ZERO);
    // The picked face moved with it.
    match &t.picked_elements()[0].shape {
        ElementShape::Face(corners) => assert!(corners.iter().all(|c| c.z == 192.0)),
        other => panic!("picked {other:?}"),
    }
    assert_eq!(doc.undo().as_deref(), Some("Move face"));
    assert_eq!(the_brush(&doc).bounds().max.z, 128.0, "one step");
}

#[test]
fn moving_a_corner_up_folds_the_top() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(SelectMode::Vertex);
    t.set_gizmo_mode(Some(GizmoMode::Move));
    let corner = Vec3::new(128.0, 0.0, 128.0);
    click(&mut t, &camera, &mut doc, screen(&camera, corner), false);
    assert!(drag_handle(
        &mut t,
        &camera,
        &mut doc,
        Handle::MoveAxis(Axis::Z),
        Vec3::new(0.0, 0.0, 32.0)
    ));
    let brush = the_brush(&doc);
    assert_eq!(brush.faces().len(), 7, "the top folds in two");
    assert!(brush.points().contains(&Vec3::new(128.0, 0.0, 160.0)));
    assert_eq!(
        t.picked_elements()[0].shape,
        ElementShape::Vertex(Vec3::new(128.0, 0.0, 160.0)),
        "the corner stays picked"
    );
    assert_eq!(doc.undo_label(), Some("Move vertex"));
    // Undo puts the corner back; the pick no longer matches and drops.
    doc.undo();
    step(&mut t, &camera, &mut doc, &input(Vec2::new(5.0, 5.0)));
    assert!(t.picked_elements().is_empty());
}

#[test]
fn moving_an_edge_down_makes_a_ramp() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(SelectMode::Edge);
    t.set_gizmo_mode(Some(GizmoMode::Move));
    let (a, b) = (Vec3::new(128.0, 0.0, 128.0), Vec3::new(128.0, 64.0, 128.0));
    click(
        &mut t,
        &camera,
        &mut doc,
        (screen(&camera, a) + screen(&camera, b)) * 0.5,
        false,
    );
    assert!(drag_handle(
        &mut t,
        &camera,
        &mut doc,
        Handle::MoveAxis(Axis::Z),
        Vec3::new(0.0, 0.0, -112.0)
    ));
    let brush = the_brush(&doc);
    assert_eq!(brush.bounds().max.z, 128.0);
    assert!(
        brush.faces().iter().any(|f| {
            let n = f.plane().normal;
            n.x > 0.3 && n.z > 0.3
        }),
        "a sloped top"
    );
}

#[test]
fn escape_cancels_a_part_drag() {
    let camera = angled();
    let mut doc = one_box();
    let before = the_brush(&doc);
    let mut t = tools(SelectMode::Face);
    t.set_gizmo_mode(Some(GizmoMode::Move));
    let top = screen(&camera, Vec3::new(64.0, 32.0, 128.0));
    click(&mut t, &camera, &mut doc, top, false);
    let shapes = t.gizmo_shapes(&camera, SIZE, &doc);
    let arrow = shapes
        .iter()
        .find(|s| s.handle == Handle::MoveAxis(Axis::Z) && !s.filled)
        .unwrap();
    let from = (arrow.points[0] + arrow.points[1]) * 0.5;
    doc.end_step();
    let press = ToolInput {
        pressed: true,
        held: true,
        ..input(from)
    };
    step(&mut t, &camera, &mut doc, &press);
    let moving = ToolInput {
        held: true,
        ..input(from - Vec2::new(0.0, 80.0))
    };
    step(&mut t, &camera, &mut doc, &moving);
    assert_ne!(the_brush(&doc), before);
    let cancel = ToolInput {
        held: true,
        cancel: true,
        ..input(from - Vec2::new(0.0, 80.0))
    };
    step(&mut t, &camera, &mut doc, &cancel);
    assert_eq!(the_brush(&doc), before);
    assert_eq!(doc.undo_label(), Some("Create brush"));
}

#[test]
fn a_single_corner_cannot_be_scaled_or_turned() {
    let camera = angled();
    let mut doc = one_box();
    let before = the_brush(&doc);
    let mut t = tools(SelectMode::Vertex);
    t.set_gizmo_mode(Some(GizmoMode::Scale));
    click(
        &mut t,
        &camera,
        &mut doc,
        screen(&camera, Vec3::new(128.0, 0.0, 128.0)),
        false,
    );
    drag_handle(
        &mut t,
        &camera,
        &mut doc,
        Handle::ScaleAxis(Axis::X),
        Vec3::new(64.0, 0.0, 0.0),
    );
    assert_eq!(the_brush(&doc), before);
}

#[test]
fn turning_the_top_face_twists_the_brush() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(SelectMode::Face);
    t.set_gizmo_mode(Some(GizmoMode::Rotate));
    click(
        &mut t,
        &camera,
        &mut doc,
        screen(&camera, Vec3::new(64.0, 32.0, 128.0)),
        false,
    );
    let shapes = t.gizmo_shapes(&camera, SIZE, &doc);
    let ring = shapes
        .iter()
        .find(|s| s.handle == Handle::Rotate(Axis::Z))
        .unwrap();
    let centre = screen(&camera, t.picked_bounds().unwrap().center());
    let (from, to) = (ring.points[6], ring.points[12]);
    doc.end_step();
    let press = ToolInput {
        pressed: true,
        held: true,
        ..input(from)
    };
    step(&mut t, &camera, &mut doc, &press);
    for k in 1..=6 {
        let p = from.lerp(to, k as f32 / 6.0);
        // Keep the pointer on the ring's side of the centre.
        let p = centre + (p - centre).normalize() * (from - centre).length();
        let moving = ToolInput {
            held: true,
            ..input(p)
        };
        step(&mut t, &camera, &mut doc, &moving);
    }
    step(
        &mut t,
        &camera,
        &mut doc,
        &ToolInput {
            released: true,
            ..input(to)
        },
    );
    let brush = the_brush(&doc);
    assert!(
        brush.faces().len() > 6,
        "the sides fold: {}",
        brush.faces().len()
    );
    assert_eq!(doc.undo_label(), Some("Rotate face"));
}

#[test]
fn deselecting_the_brush_drops_its_parts() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(SelectMode::Vertex);
    click(
        &mut t,
        &camera,
        &mut doc,
        screen(&camera, Vec3::new(128.0, 0.0, 128.0)),
        false,
    );
    assert_eq!(t.picked_elements().len(), 1);
    doc.clear_selection();
    step(&mut t, &camera, &mut doc, &input(Vec2::new(5.0, 5.0)));
    assert!(t.picked_elements().is_empty());
    assert!(!t.clear_elements(), "nothing left to clear");
}

#[test]
fn clear_elements_lets_go_first() {
    let camera = angled();
    let mut doc = one_box();
    let mut t = tools(SelectMode::Vertex);
    click(
        &mut t,
        &camera,
        &mut doc,
        screen(&camera, Vec3::new(128.0, 0.0, 128.0)),
        false,
    );
    assert!(t.clear_elements());
    assert!(t.picked_elements().is_empty());
    assert_eq!(doc.selection().len(), 1, "the brush stays selected");
}

#[test]
fn brushes_of_a_selected_entity_can_be_edited() {
    use halberd_doc::{BrushObject, EntityObject, MapObject};
    let camera = angled();
    let cube = Brush::cuboid(Aabb::from_corners(
        Vec3::ZERO,
        Vec3::new(128.0, 64.0, 128.0),
    ))
    .unwrap();
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
    doc.set_selection([entity]);
    let mut t = tools(SelectMode::Face);
    t.set_gizmo_mode(Some(GizmoMode::Move));
    click(
        &mut t,
        &camera,
        &mut doc,
        screen(&camera, Vec3::new(64.0, 32.0, 128.0)),
        false,
    );
    assert_eq!(t.picked_elements().len(), 1);
    assert!(drag_handle(
        &mut t,
        &camera,
        &mut doc,
        Handle::MoveAxis(Axis::Z),
        Vec3::new(0.0, 0.0, 32.0)
    ));
    assert_eq!(the_brush(&doc).bounds().max.z, 160.0);
    let brush = doc.brushes_of(entity).next().unwrap();
    assert_eq!(
        doc.get(brush).unwrap().as_brush().unwrap().entity(),
        Some(entity),
        "still in the entity"
    );
}

#[test]
fn random_part_drags_never_break_the_map() {
    let camera = angled();
    let mut seed: u64 = 0xE1E7_0000_0000_0042;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % 10_001) as f32 / 10_000.0
    };
    let modes = [SelectMode::Vertex, SelectMode::Edge, SelectMode::Face];
    let gizmos = [GizmoMode::Move, GizmoMode::Rotate, GizmoMode::Scale];
    for round in 0..60 {
        let mut doc = one_box();
        let mut t = tools(modes[round % 3]);
        t.set_gizmo_mode(Some(gizmos[(round / 3) % 3]));
        for _ in 0..4 {
            let at = Vec2::new(next() * SIZE.x, next() * SIZE.y);
            click(&mut t, &camera, &mut doc, at, next() > 0.5);
        }
        let shapes = t.gizmo_shapes(&camera, SIZE, &doc);
        if let Some(shape) =
            shapes.get((next() * shapes.len() as f32) as usize % shapes.len().max(1))
        {
            let from = shape.points[0];
            doc.end_step();
            let press = ToolInput {
                pressed: true,
                held: true,
                ..input(from)
            };
            step(&mut t, &camera, &mut doc, &press);
            for _ in 0..5 {
                let to = from + Vec2::new(next() - 0.5, next() - 0.5) * 400.0;
                let moving = ToolInput {
                    held: true,
                    ..input(to)
                };
                step(&mut t, &camera, &mut doc, &moving);
            }
            let release = ToolInput {
                released: true,
                ..input(from)
            };
            step(&mut t, &camera, &mut doc, &release);
        }
        for (_, object) in doc.objects() {
            if let Some(b) = object.as_brush() {
                assert_eq!(b.faces().len(), b.brush().faces().len());
                assert!(b.brush().faces().len() >= 4);
            }
        }
    }
}
