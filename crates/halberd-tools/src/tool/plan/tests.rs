//! Tests for drawing floor plans and turning them into brushes.

use super::super::{Tool, ToolAction, ToolController, ToolInput};
use crate::{Camera, ShapeOp};
use glam::{Vec2, Vec3};
use halberd_doc::{Command, Document, FaceInfo, Object, ShapeObject};
use halberd_geom::Shape;

const SIZE: Vec2 = Vec2::new(1600.0, 900.0);

fn top_down() -> Camera {
    Camera::looking_at(Vec3::new(0.0, -1.0, 2000.0), Vec3::ZERO)
}

fn plan_tools() -> ToolController {
    let mut tools = ToolController::new(16.0);
    tools.set_tool(Tool::Box);
    tools.set_shape(Shape::FloorPlan);
    tools.set_box_height(64.0);
    tools
}

/// Moves the pointer to `p` and clicks there. Returns the action.
fn click(
    tools: &mut ToolController,
    camera: &Camera,
    doc: &Document,
    p: Vec3,
) -> Option<ToolAction> {
    let cursor = camera.project(p, SIZE).unwrap();
    let hover = ToolInput {
        size: SIZE,
        cursor: Some(cursor),
        ..ToolInput::default()
    };
    tools.update(&hover, camera, doc);
    let press = ToolInput {
        pressed: true,
        held: true,
        ..hover
    };
    let action = tools.update(&press, camera, doc);
    let release = ToolInput {
        released: true,
        ..hover
    };
    action.or(tools.update(&release, camera, doc))
}

/// An L-shaped outline, 256 by 256 with a 128 by 128 bite.
const L: [Vec3; 6] = [
    Vec3::new(0.0, 0.0, 0.0),
    Vec3::new(256.0, 0.0, 0.0),
    Vec3::new(256.0, 128.0, 0.0),
    Vec3::new(128.0, 128.0, 0.0),
    Vec3::new(128.0, 256.0, 0.0),
    Vec3::new(0.0, 256.0, 0.0),
];

fn added_shape(action: Option<ToolAction>) -> ShapeObject {
    match action {
        Some(ToolAction::Execute(Command::AddShape(shape))) => shape,
        other => panic!("expected a new shape, got {other:?}"),
    }
}

#[test]
fn clicking_the_first_corner_again_closes_an_l_shape() {
    let (camera, doc) = (top_down(), Document::new());
    let mut tools = plan_tools();
    for corner in L {
        assert_eq!(click(&mut tools, &camera, &doc, corner), None);
    }
    assert!(tools.is_drawing_plan());
    assert!(tools.is_busy(), "undo and delete wait while drawing");
    let preview = tools.plan_preview().unwrap();
    assert_eq!(preview.corners.len(), 6);
    let shape = added_shape(click(&mut tools, &camera, &doc, L[0]));
    assert!(!tools.is_drawing_plan());
    assert_eq!(shape.outline().len(), 6);
    assert_eq!((shape.bottom(), shape.top()), (0.0, 64.0));
    assert_eq!(shape.pieces().len(), 2, "an L is two convex pieces");
}

#[test]
fn enter_finishes_and_backspace_takes_a_corner_back() {
    let (camera, doc) = (top_down(), Document::new());
    let mut tools = plan_tools();
    for corner in &L[..4] {
        click(&mut tools, &camera, &doc, *corner);
    }
    assert!(tools.undo_plan_corner());
    assert_eq!(tools.plan_preview().unwrap().corners.len(), 3);
    let shape = added_shape(tools.finish_plan());
    assert_eq!(shape.pieces().len(), 1, "a triangle is one piece");
    assert_eq!(tools.finish_plan(), None, "nothing left to finish");
}

#[test]
fn backspacing_every_corner_stops_drawing() {
    let (camera, doc) = (top_down(), Document::new());
    let mut tools = plan_tools();
    click(&mut tools, &camera, &doc, L[0]);
    assert!(tools.undo_plan_corner());
    assert!(!tools.is_drawing_plan());
    assert!(!tools.undo_plan_corner(), "no plan to take from");
}

#[test]
fn too_few_corners_or_a_crossing_outline_is_refused() {
    let (camera, doc) = (top_down(), Document::new());
    let mut tools = plan_tools();
    click(&mut tools, &camera, &doc, L[0]);
    click(&mut tools, &camera, &doc, L[1]);
    match tools.finish_plan() {
        Some(ToolAction::Refused(why)) => assert!(why.contains("three corners"), "{why}"),
        other => panic!("expected a refusal, got {other:?}"),
    }
    // A bow tie.
    for p in [
        L[0],
        L[1],
        Vec3::new(0.0, 256.0, 0.0),
        Vec3::new(256.0, 256.0, 0.0),
    ] {
        click(&mut tools, &camera, &doc, p);
    }
    match tools.finish_plan() {
        Some(ToolAction::Refused(why)) => assert!(why.contains("crosses itself"), "{why}"),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn escape_and_changing_shape_drop_the_plan() {
    let (camera, doc) = (top_down(), Document::new());
    let mut tools = plan_tools();
    click(&mut tools, &camera, &doc, L[0]);
    let cancel = ToolInput {
        size: SIZE,
        cancel: true,
        ..ToolInput::default()
    };
    tools.update(&cancel, &camera, &doc);
    assert!(!tools.is_drawing_plan());
    click(&mut tools, &camera, &doc, L[0]);
    tools.set_shape(Shape::Box);
    assert!(!tools.is_drawing_plan());
}

#[test]
fn make_brushes_swaps_a_shape_for_its_pieces_and_undo_brings_it_back() {
    let mut doc = Document::new();
    let outline: Vec<Vec2> = L.iter().map(|p| p.truncate()).collect();
    let look = FaceInfo {
        material: "DEV/DEV_MEASUREWALL01A".into(),
        ..FaceInfo::default()
    };
    let shape = ShapeObject::new(&outline, 0.0, 64.0, look).unwrap();
    doc.execute(Command::AddShape(shape)).unwrap();
    let mut tools = ToolController::new(16.0);
    let camera = top_down();
    tools
        .apply_shape_op(ShapeOp::MakeBrushes, &mut doc, &camera)
        .unwrap();
    let objects: Vec<&Object> = doc.objects().map(|(_, o)| o).collect();
    assert_eq!(objects.len(), 2);
    assert!(objects.iter().all(|o| o.as_brush().is_some()));
    assert_eq!(doc.selection().len(), 2, "the new brushes are selected");
    let brush = objects[0].as_brush().unwrap();
    assert!(
        brush
            .faces()
            .iter()
            .all(|f| f.material == "DEV/DEV_MEASUREWALL01A")
    );
    doc.undo();
    let objects: Vec<&Object> = doc.objects().map(|(_, o)| o).collect();
    assert_eq!(objects.len(), 1);
    assert!(objects[0].as_shape().is_some());
}

#[test]
fn make_brushes_needs_a_shape_selected() {
    let doc = Document::new();
    let tools = ToolController::new(16.0);
    let why = tools
        .shape_op_ready(ShapeOp::MakeBrushes, &doc)
        .unwrap_err();
    assert!(why.contains("floor plan shape"), "{why}");
}
