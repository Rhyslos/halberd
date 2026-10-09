//! Tests for the left-mouse tools.

use super::*;
use glam::Vec3;

const SIZE: Vec2 = Vec2::new(1600.0, 900.0);

/// A camera straight above the origin looking down, so screen positions
/// map simply onto the grid.
fn top_down() -> Camera {
    Camera::looking_at(Vec3::new(0.0, -1.0, 2000.0), Vec3::ZERO)
}

fn input(cursor: Vec2) -> ToolInput {
    ToolInput {
        size: SIZE,
        cursor: Some(cursor),
        ..ToolInput::default()
    }
}

/// Where a world point appears on screen.
fn screen(camera: &Camera, p: Vec3) -> Vec2 {
    camera.project(p, SIZE).unwrap()
}

/// Presses at `from`, drags to `to` and releases. Returns the last action.
fn drag(
    tools: &mut ToolController,
    camera: &Camera,
    doc: &Document,
    from: Vec2,
    to: Vec2,
) -> Option<ToolAction> {
    let press = ToolInput {
        pressed: true,
        held: true,
        ..input(from)
    };
    assert_eq!(tools.update(&press, camera, doc), None);
    let moving = ToolInput {
        held: true,
        ..input(to)
    };
    assert_eq!(tools.update(&moving, camera, doc), None);
    let release = ToolInput {
        released: true,
        ..input(to)
    };
    tools.update(&release, camera, doc)
}

fn added_brush(action: Option<ToolAction>) -> Brush {
    match action {
        Some(ToolAction::Execute(Command::AddBrushes(mut brushes))) => brushes.remove(0),
        other => panic!("expected a new brush, got {other:?}"),
    }
}

#[test]
fn dragging_draws_a_box_snapped_to_the_grid() {
    let (camera, doc) = (top_down(), Document::new());
    let mut tools = ToolController::new(16.0);
    tools.set_tool(Tool::Box);
    let from = screen(&camera, Vec3::new(3.0, 5.0, 0.0));
    let to = screen(&camera, Vec3::new(130.0, 60.0, 0.0));
    let brush = added_brush(drag(&mut tools, &camera, &doc, from, to));
    assert_eq!(brush.bounds().min, Vec3::new(0.0, 0.0, 0.0));
    assert_eq!(
        brush.bounds().max,
        Vec3::new(128.0, 64.0, DEFAULT_BOX_HEIGHT)
    );
    assert!(tools.preview().is_none(), "the preview ends with the drag");
}

#[test]
fn the_preview_follows_the_drag() {
    let (camera, doc) = (top_down(), Document::new());
    let mut tools = ToolController::new(16.0);
    tools.set_tool(Tool::Box);
    let press = ToolInput {
        pressed: true,
        held: true,
        ..input(screen(&camera, Vec3::ZERO))
    };
    tools.update(&press, &camera, &doc);
    let moving = ToolInput {
        held: true,
        ..input(screen(&camera, Vec3::new(-64.0, 32.0, 0.0)))
    };
    tools.update(&moving, &camera, &doc);
    let preview = tools.preview().unwrap();
    assert_eq!(preview.min, Vec3::new(-64.0, 0.0, 0.0));
    assert_eq!(preview.max, Vec3::new(0.0, 32.0, DEFAULT_BOX_HEIGHT));
    assert!(tools.is_busy());
}

#[test]
fn boxes_drawn_on_a_box_stand_on_top_of_it() {
    let camera = top_down();
    let mut doc = Document::new();
    let floor = Brush::cuboid(Aabb::from_corners(
        Vec3::new(-256.0, -256.0, 0.0),
        Vec3::new(256.0, 256.0, 64.0),
    ))
    .unwrap();
    doc.execute(Command::AddBrushes(vec![floor])).unwrap();
    let mut tools = ToolController::new(16.0);
    tools.set_tool(Tool::Box);
    let from = screen(&camera, Vec3::new(0.0, 0.0, 64.0));
    let to = screen(&camera, Vec3::new(64.0, 64.0, 64.0));
    let brush = added_brush(drag(&mut tools, &camera, &doc, from, to));
    assert_eq!(brush.bounds().min.z, 64.0);
    assert_eq!(brush.bounds().max.z, 64.0 + DEFAULT_BOX_HEIGHT);
}

#[test]
fn a_click_makes_no_box() {
    let (camera, doc) = (top_down(), Document::new());
    let mut tools = ToolController::new(16.0);
    tools.set_tool(Tool::Box);
    let at = screen(&camera, Vec3::new(32.0, 32.0, 0.0));
    assert_eq!(drag(&mut tools, &camera, &doc, at, at), None);
}

#[test]
fn a_drag_along_a_line_makes_a_wall_one_grid_square_thick() {
    let (camera, doc) = (top_down(), Document::new());
    let mut tools = ToolController::new(16.0);
    tools.set_tool(Tool::Box);
    let from = screen(&camera, Vec3::new(32.0, 32.0, 0.0));
    let along = screen(&camera, Vec3::new(200.0, 34.0, 0.0));
    let wall = added_brush(drag(&mut tools, &camera, &doc, from, along));
    assert_eq!(wall.bounds().min, Vec3::new(32.0, 32.0, 0.0));
    assert_eq!(
        wall.bounds().max,
        Vec3::new(208.0, 48.0, DEFAULT_BOX_HEIGHT)
    );
}

#[test]
fn escape_cancels_a_box() {
    let (camera, doc) = (top_down(), Document::new());
    let mut tools = ToolController::new(16.0);
    tools.set_tool(Tool::Box);
    let press = ToolInput {
        pressed: true,
        held: true,
        ..input(screen(&camera, Vec3::ZERO))
    };
    tools.update(&press, &camera, &doc);
    let cancel = ToolInput {
        held: true,
        cancel: true,
        ..input(screen(&camera, Vec3::splat(100.0)))
    };
    assert_eq!(tools.update(&cancel, &camera, &doc), None);
    assert!(!tools.is_busy());
    let release = ToolInput {
        released: true,
        ..input(screen(&camera, Vec3::splat(100.0)))
    };
    assert_eq!(tools.update(&release, &camera, &doc), None);
}

#[test]
fn a_box_beyond_the_world_is_refused_with_a_reason() {
    let camera = Camera::looking_at(
        Vec3::new(130_000.0, 0.0, 300.0),
        Vec3::new(131_000.0, 0.0, 0.0),
    );
    let doc = Document::new();
    let mut tools = ToolController::new(256.0);
    tools.set_tool(Tool::Box);
    let from = screen(&camera, Vec3::new(130_500.0, -300.0, 0.0));
    let to = screen(&camera, Vec3::new(131_500.0, 300.0, 0.0));
    match drag(&mut tools, &camera, &doc, from, to) {
        Some(ToolAction::Refused(reason)) => assert!(reason.contains("edge of the world")),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn clicking_selects_what_is_under_the_pointer() {
    let camera = top_down();
    let mut doc = Document::new();
    let brush = Brush::cuboid(Aabb::from_corners(Vec3::ZERO, Vec3::splat(64.0))).unwrap();
    doc.execute(Command::AddBrushes(vec![brush])).unwrap();
    let id = doc.objects().next().unwrap().0;
    let mut tools = ToolController::new(16.0);

    let on_box = screen(&camera, Vec3::new(32.0, 32.0, 64.0));
    let off_box = screen(&camera, Vec3::new(300.0, 300.0, 0.0));
    assert_eq!(
        drag(&mut tools, &camera, &doc, on_box, on_box),
        Some(ToolAction::Select(Some(id)))
    );
    assert_eq!(
        drag(&mut tools, &camera, &doc, off_box, off_box),
        Some(ToolAction::Select(None)),
        "clicking empty space selects nothing"
    );
}

#[test]
fn ctrl_click_adds_or_removes() {
    let camera = top_down();
    let mut doc = Document::new();
    let brush = Brush::cuboid(Aabb::from_corners(Vec3::ZERO, Vec3::splat(64.0))).unwrap();
    doc.execute(Command::AddBrushes(vec![brush])).unwrap();
    let id = doc.objects().next().unwrap().0;
    let mut tools = ToolController::new(16.0);
    let click = |tools: &mut ToolController, at: Vec2| {
        let press = ToolInput {
            pressed: true,
            held: true,
            additive: true,
            ..input(at)
        };
        tools.update(&press, &camera, &doc);
        let release = ToolInput {
            released: true,
            additive: true,
            ..input(at)
        };
        tools.update(&release, &camera, &doc)
    };
    let on_box = screen(&camera, Vec3::new(32.0, 32.0, 64.0));
    assert_eq!(
        click(&mut tools, on_box),
        Some(ToolAction::ToggleSelected(id))
    );
    let empty = screen(&camera, Vec3::new(300.0, 300.0, 0.0));
    assert_eq!(click(&mut tools, empty), None, "keeps the selection");
}

#[test]
fn dragging_with_the_select_tool_is_not_a_click() {
    let (camera, doc) = (top_down(), Document::new());
    let mut tools = ToolController::new(16.0);
    let from = Vec2::new(100.0, 100.0);
    let to = from + Vec2::new(CLICK_SLOP + 1.0, 0.0);
    assert_eq!(drag(&mut tools, &camera, &doc, from, to), None);
}

#[test]
fn switching_tools_cancels_a_drag_and_bad_grid_sizes_are_repaired() {
    let (camera, doc) = (top_down(), Document::new());
    let mut tools = ToolController::new(f32::NAN);
    assert_eq!(tools.grid_size(), 16.0);
    tools.set_tool(Tool::Box);
    let press = ToolInput {
        pressed: true,
        held: true,
        ..input(screen(&camera, Vec3::ZERO))
    };
    tools.update(&press, &camera, &doc);
    assert!(tools.is_busy());
    tools.set_tool(Tool::Select);
    assert!(!tools.is_busy());
    assert_eq!(ToolController::new(0.0).grid_size(), 16.0);
    assert_eq!(ToolController::new(1e9).grid_size(), 4096.0);
}

#[test]
fn broken_input_never_panics() {
    let (camera, doc) = (top_down(), Document::new());
    let mut tools = ToolController::new(16.0);
    for tool in Tool::ALL {
        tools.set_tool(tool);
        let bad = ToolInput {
            size: Vec2::ZERO,
            cursor: Some(Vec2::splat(f32::NAN)),
            pressed: true,
            held: true,
            released: true,
            additive: true,
            cancel: false,
        };
        tools.update(&bad, &camera, &doc);
        let no_cursor = ToolInput {
            cursor: None,
            ..bad
        };
        tools.update(&no_cursor, &camera, &doc);
    }
}

#[test]
fn new_boxes_use_the_chosen_height() {
    let (camera, doc) = (top_down(), Document::new());
    let mut tools = ToolController::new(16.0);
    tools.set_tool(Tool::Box);
    tools.set_box_height(72.4);
    assert_eq!(tools.box_height(), 72.0, "whole units");
    let from = screen(&camera, Vec3::ZERO);
    let to = screen(&camera, Vec3::new(64.0, 64.0, 0.0));
    let brush = added_brush(drag(&mut tools, &camera, &doc, from, to));
    assert_eq!(brush.bounds().max.z, 72.0);
    tools.set_box_height(f32::NAN);
    assert_eq!(tools.box_height(), 72.0, "nonsense is ignored");
    tools.set_box_height(-5.0);
    assert_eq!(tools.box_height(), BOX_HEIGHT_RANGE.0);
    tools.set_box_height(1e9);
    assert_eq!(tools.box_height(), BOX_HEIGHT_RANGE.1);
}

#[test]
fn clicking_a_brush_entity_picks_it_whole_unless_picking_inside() {
    use halberd_doc::{BrushObject, EntityObject, MapObject};
    let camera = top_down();
    let cube = Brush::cuboid(Aabb::from_corners(Vec3::ZERO, Vec3::splat(64.0))).unwrap();
    let detail = MapObject::Entity(
        EntityObject {
            classname: "func_detail".into(),
            origin: None,
            file_data: Vec::new(),
        },
        vec![BrushObject::new(cube)],
    );
    let doc = Document::from_map(vec![detail], Default::default()).unwrap();
    let entity = doc.objects().next().unwrap().0;
    let brush = doc.brushes_of(entity).next().unwrap();
    let mut tools = ToolController::new(16.0);
    let on_box = screen(&camera, Vec3::new(32.0, 32.0, 64.0));
    assert!(!tools.inside_entities(), "off at first, as in Hammer");
    assert_eq!(
        drag(&mut tools, &camera, &doc, on_box, on_box),
        Some(ToolAction::Select(Some(entity)))
    );
    tools.set_inside_entities(true);
    assert_eq!(
        drag(&mut tools, &camera, &doc, on_box, on_box),
        Some(ToolAction::Select(Some(brush)))
    );
}

fn added_brushes(action: Option<ToolAction>) -> Vec<Brush> {
    match action {
        Some(ToolAction::Execute(Command::AddBrushes(brushes))) => brushes,
        other => panic!("expected new brushes, got {other:?}"),
    }
}

#[test]
fn stairs_climb_the_way_the_drag_went() {
    let (camera, doc) = (top_down(), Document::new());
    let mut tools = ToolController::new(16.0);
    tools.set_tool(Tool::Box);
    tools.set_shape(halberd_geom::Shape::Stairs);
    tools.set_box_height(64.0);
    let west = screen(&camera, Vec3::new(0.0, 0.0, 0.0));
    let east = screen(&camera, Vec3::new(256.0, 64.0, 0.0));
    let up_east = added_brushes(drag(&mut tools, &camera, &doc, west, east));
    assert_eq!(up_east.len(), 8, "64 units in 8-unit steps");
    let top = up_east.last().unwrap().bounds();
    assert_eq!(
        (top.max.x, top.max.z),
        (256.0, 64.0),
        "highest step at the end"
    );
    // Dragged the other way: highest step at the west end.
    let up_west = added_brushes(drag(&mut tools, &camera, &doc, east, west));
    let top = up_west.last().unwrap().bounds();
    assert_eq!((top.min.x, top.max.z), (0.0, 64.0));
}

#[test]
fn the_preview_shows_the_shape_being_drawn() {
    let (camera, doc) = (top_down(), Document::new());
    let mut tools = ToolController::new(16.0);
    tools.set_tool(Tool::Box);
    tools.set_shape(halberd_geom::Shape::Cylinder);
    tools.shape_settings_mut().sides = 12;
    let from = screen(&camera, Vec3::new(0.0, 0.0, 0.0));
    let to = screen(&camera, Vec3::new(256.0, 256.0, 0.0));
    let press = ToolInput {
        pressed: true,
        held: true,
        ..input(from)
    };
    tools.update(&press, &camera, &doc);
    let moving = ToolInput {
        held: true,
        ..input(to)
    };
    tools.update(&moving, &camera, &doc);
    let preview = tools.preview_brushes();
    assert_eq!(preview.len(), 1);
    assert_eq!(preview[0].faces().len(), 12 + 2);
}

#[test]
fn a_shape_too_small_for_its_sides_is_refused_with_a_reason() {
    let (camera, doc) = (top_down(), Document::new());
    let mut tools = ToolController::new(1.0);
    tools.set_tool(Tool::Box);
    tools.set_shape(halberd_geom::Shape::Stairs);
    tools.set_box_height(128.0);
    // Four units long cannot hold 16 steps.
    let from = screen(&camera, Vec3::new(0.0, 0.0, 0.0));
    let to = screen(&camera, Vec3::new(4.0, 2.0, 0.0));
    match drag(&mut tools, &camera, &doc, from, to) {
        Some(ToolAction::Refused(reason)) => {
            assert!(reason.contains("too small"), "{reason}");
            assert!(reason.contains("stairs"), "{reason}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}
