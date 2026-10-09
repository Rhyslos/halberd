//! Tests for the viewport panel, driven with simulated mouse and keyboard input.

use super::*;
use egui::{Event, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

struct Recording {
    views: Vec<ViewportView>,
}

impl ViewportRenderer for Recording {
    fn render(&mut self, view: &ViewportView, _doc: &Document) -> Result<egui::TextureId, String> {
        self.views.push(view.clone());
        Err("recording only".into())
    }
}

struct State {
    panel: ViewportPanel,
    renderer: Recording,
    doc: Document,
    notes: Vec<String>,
}

fn harness(options: ViewportOptions) -> Harness<'static, State> {
    Harness::builder().with_size([800.0, 600.0]).build_ui_state(
        |ui, s: &mut State| {
            let notes = s.panel.show(ui, &mut s.renderer, &mut s.doc);
            s.notes.extend(notes);
        },
        State {
            panel: ViewportPanel::new(None, options),
            renderer: Recording { views: Vec::new() },
            doc: Document::new(),
            notes: Vec::new(),
        },
    )
}

/// A left-button click (press and release) at `at`.
fn left_click(h: &mut Harness<'_, State>, at: egui::Pos2) {
    h.hover_at(at);
    press(h, PointerButton::Primary, at, true);
    h.run();
    press(h, PointerButton::Primary, at, false);
    h.run();
}

/// A left-button drag from `from` to `to`.
fn left_drag(h: &mut Harness<'_, State>, from: egui::Pos2, to: egui::Pos2) {
    h.hover_at(from);
    press(h, PointerButton::Primary, from, true);
    h.run();
    h.hover_at(to);
    h.run();
    press(h, PointerButton::Primary, to, false);
    h.run();
}

fn press(h: &Harness<'_, State>, button: PointerButton, at: egui::Pos2, pressed: bool) {
    h.event(Event::PointerButton {
        pos: at,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    });
}

#[test]
fn renders_at_the_panel_size_in_pixels() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    let view = h.state().renderer.views.last().unwrap().clone();
    assert!(
        view.size_px[0] >= 780 && view.size_px[1] >= 580,
        "{:?}",
        view.size_px
    );
    assert_eq!(view.grid_size, 16.0);
    assert!(view.view_projection.is_finite());
}

#[test]
fn missing_renderer_shows_a_reason_instead_of_crashing() {
    let mut h = Harness::builder().with_size([400.0, 300.0]).build_ui_state(
        |ui, (panel, doc): &mut (ViewportPanel, Document)| {
            panel.show(ui, &mut NoRenderer::default(), doc);
        },
        (
            ViewportPanel::new(None, ViewportOptions::default()),
            Document::new(),
        ),
    );
    h.run();
    assert!(h.query_by_label(VIEWPORT_LABEL).is_some());
}

#[test]
fn right_drag_orbits_the_camera() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    let yaw = h.state().panel.controller().camera().yaw;
    let start = egui::pos2(400.0, 500.0);
    h.hover_at(start);
    press(&h, PointerButton::Secondary, start, true);
    h.run();
    h.hover_at(start + vec2(60.0, 0.0));
    h.run();
    assert!(
        h.state().panel.controller().active_pivot().is_some(),
        "pivot marker shown"
    );
    press(&h, PointerButton::Secondary, start + vec2(60.0, 0.0), false);
    h.run();
    assert!(
        h.state().panel.controller().camera().yaw < yaw,
        "dragging right turns right"
    );
    assert!(
        h.state().panel.controller().active_pivot().is_none(),
        "marker gone"
    );
}

#[test]
fn middle_drag_pans_the_camera() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    let before = h.state().panel.controller().camera().position;
    let start = egui::pos2(400.0, 300.0);
    h.hover_at(start);
    press(&h, PointerButton::Middle, start, true);
    h.run();
    h.hover_at(start + vec2(-40.0, 25.0));
    h.run();
    press(&h, PointerButton::Middle, start + vec2(-40.0, 25.0), false);
    h.run();
    let after = h.state().panel.controller().camera().position;
    assert!(before.distance(after) > 1.0);
}

#[test]
fn scrolling_zooms() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    let before = h.state().panel.controller().camera().position.length();
    h.hover_at(egui::pos2(400.0, 300.0));
    h.event(Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, 120.0),
        modifiers: Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    });
    for _ in 0..30 {
        h.step();
    }
    let after = h.state().panel.controller().camera().position.length();
    assert!(
        after < before,
        "scrolling up moves closer: {before} -> {after}"
    );
}

#[test]
fn fly_mode_moves_with_right_mouse_and_w() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    h.get_by_label("Fly").click();
    h.run();
    let start = h.state().panel.controller().camera().position;
    let forward = h.state().panel.controller().camera().forward();
    let centre = egui::pos2(400.0, 300.0);
    h.hover_at(centre);
    press(&h, PointerButton::Secondary, centre, true);
    h.key_down(Key::W);
    // Flying keeps requesting frames, so step instead of run.
    for _ in 0..20 {
        h.step();
    }
    h.key_up(Key::W);
    press(&h, PointerButton::Secondary, centre, false);
    h.step();
    let moved = h.state().panel.controller().camera().position - start;
    assert!(moved.length() > 1.0, "camera should have flown forward");
    assert!(
        moved.normalize().dot(forward) > 0.99,
        "moved along the view direction"
    );
}

#[test]
fn w_alone_does_not_fly() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    h.get_by_label("Fly").click();
    h.run();
    let start = h.state().panel.controller().camera().position;
    h.hover_at(egui::pos2(400.0, 300.0));
    h.key_down(Key::W);
    for _ in 0..10 {
        h.step();
    }
    h.key_up(Key::W);
    h.step();
    assert_eq!(h.state().panel.controller().camera().position, start);
}

#[test]
fn mode_switcher_changes_mode() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    h.get_by_label("Fly").click();
    h.run();
    assert_eq!(h.state().panel.controller().mode(), CameraMode::Fly);
    h.get_by_label("Orbit").click();
    h.run();
    assert_eq!(h.state().panel.controller().mode(), CameraMode::Orbit);
}

#[test]
fn fly_button_is_hidden_when_wasd_is_off() {
    let mut h = harness(ViewportOptions {
        wasd_enabled: false,
        grid_size: 16.0,
    });
    h.run();
    assert!(h.query_by_label("Default").is_some());
    assert!(h.query_by_label("Fly").is_none());
}

#[test]
fn clicking_the_switcher_does_not_move_the_camera() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    let before = *h.state().panel.controller().camera();
    h.get_by_label("Orbit").click();
    h.run();
    assert_eq!(*h.state().panel.controller().camera(), before);
}

#[test]
fn saved_camera_is_restored() {
    let mut state = CameraState::default();
    state.camera.position = Vec3::new(10.0, 20.0, 30.0);
    state.mode = CameraMode::Orbit;
    let panel = ViewportPanel::new(Some(state), ViewportOptions::default());
    assert_eq!(panel.camera_state(), state);
}

#[test]
fn box_tool_draws_a_brush_by_dragging() {
    let mut h = harness(ViewportOptions::default());
    h.state_mut().panel.set_tool(Tool::Box);
    h.run();
    left_drag(&mut h, egui::pos2(300.0, 420.0), egui::pos2(480.0, 470.0));
    let doc = &h.state().doc;
    assert_eq!(doc.len(), 1, "one box drawn");
    assert_eq!(doc.selection().len(), 1, "and selected");
    let size = doc.selection_bounds().unwrap().size();
    assert_eq!(size.z, halberd_tools::DEFAULT_BOX_HEIGHT);
    assert_eq!(size.x % 16.0, 0.0, "snapped to the 16-unit grid");
    assert!(h.state().notes.is_empty());
}

#[test]
fn the_box_preview_is_sent_to_the_renderer_while_dragging() {
    let mut h = harness(ViewportOptions::default());
    h.state_mut().panel.set_tool(Tool::Box);
    h.run();
    let (from, to) = (egui::pos2(300.0, 420.0), egui::pos2(480.0, 470.0));
    h.hover_at(from);
    press(&h, PointerButton::Primary, from, true);
    h.run();
    h.hover_at(to);
    h.run();
    assert!(h.state().renderer.views.last().unwrap().preview.is_some());
    press(&h, PointerButton::Primary, to, false);
    h.run();
    assert!(h.state().renderer.views.last().unwrap().preview.is_none());
}

#[test]
fn clicking_selects_and_escape_or_empty_space_deselects() {
    let mut h = harness(ViewportOptions::default());
    h.state_mut().panel.set_tool(Tool::Box);
    h.run();
    let (from, to) = (egui::pos2(300.0, 420.0), egui::pos2(480.0, 470.0));
    left_drag(&mut h, from, to);
    h.state_mut().panel.set_tool(Tool::Select);

    // Sky: nothing there, so the selection is cleared.
    left_click(&mut h, egui::pos2(400.0, 40.0));
    assert!(h.state().doc.selection().is_empty());
    // The middle of the drawn footprint is inside the box.
    left_click(&mut h, egui::pos2(390.0, 445.0));
    assert_eq!(h.state().doc.selection().len(), 1);
    h.key_press(Key::Escape);
    h.run();
    assert!(h.state().doc.selection().is_empty());
}

#[test]
fn b_picks_draw_then_opens_the_shape_list_and_q_goes_back_to_select() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    assert_eq!(h.state().panel.tool(), Tool::Select);
    h.key_press(Key::B);
    h.run();
    assert_eq!(h.state().panel.tool(), Tool::Box);
    assert!(h.query_by_label("Cylinder").is_none(), "list closed");
    // B again opens the list; arrows move, Enter picks.
    h.key_press(Key::B);
    h.run();
    assert!(h.query_by_label("Cylinder").is_some(), "list open");
    for _ in 0..2 {
        h.key_press(Key::ArrowDown);
        h.run();
    }
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(
        h.state().panel.tools().shape(),
        halberd_geom::Shape::Cylinder
    );
    assert!(
        h.query_by_label("Cylinder").is_none(),
        "closed after picking"
    );
    assert_eq!(h.state().panel.tool(), Tool::Box);
    // Up wraps round to the last shape; Space picks too.
    h.key_press(Key::B);
    h.run();
    for _ in 0..3 {
        h.key_press(Key::ArrowUp);
        h.run();
    }
    h.key_press(Key::Space);
    h.run();
    assert_eq!(h.state().panel.tools().shape(), halberd_geom::Shape::Stairs);
    // Q is Select.
    h.key_press(Key::Q);
    h.run();
    assert_eq!(h.state().panel.tool(), Tool::Select);
}

#[test]
fn escape_closes_the_shape_list_but_keeps_the_selection() {
    let mut h = harness(ViewportOptions::default());
    let brush =
        halberd_geom::Brush::cuboid(Aabb::from_corners(Vec3::ZERO, Vec3::splat(64.0))).unwrap();
    h.state_mut()
        .doc
        .execute(halberd_doc::Command::AddBrushes(vec![brush]))
        .unwrap();
    h.state_mut().panel.set_tool(Tool::Box);
    h.run();
    h.key_press(Key::B);
    h.run();
    assert!(h.query_by_label("Wedge").is_some());
    h.key_press(Key::Escape);
    h.run();
    assert!(h.query_by_label("Wedge").is_none());
    assert_eq!(h.state().doc.selection().len(), 1, "still selected");
    assert_eq!(h.state().panel.tools().shape(), halberd_geom::Shape::Box);
}

#[test]
fn r_while_drawing_turns_the_shape_and_gizmo_keys_wait() {
    let mut h = harness(ViewportOptions::default());
    h.state_mut().panel.set_tool(Tool::Box);
    h.run();
    let (from, to) = (egui::pos2(300.0, 420.0), egui::pos2(480.0, 470.0));
    h.hover_at(from);
    press(&h, PointerButton::Primary, from, true);
    h.run();
    h.hover_at(to);
    h.run();
    h.key_press(Key::R);
    h.run();
    h.key_press(Key::W);
    h.run();
    assert_eq!(h.state().panel.tool(), Tool::Box, "still drawing");
    assert!(h.state().panel.tools().is_drawing());
    assert_eq!(h.state().panel.tools().gizmo_mode(), None);
    press(&h, PointerButton::Primary, to, false);
    h.run();
    assert_eq!(h.state().doc.len(), 1, "the drawing finished");
}

#[test]
fn tool_switcher_changes_tool_without_drawing() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    h.get_by_label("Draw").click();
    h.run();
    assert_eq!(h.state().panel.tool(), Tool::Box);
    assert!(
        h.state().doc.is_empty(),
        "clicking the button draws nothing"
    );
    h.get_by_label("Select").click();
    h.run();
    assert_eq!(h.state().panel.tool(), Tool::Select);
}

#[test]
fn a_left_press_while_orbiting_does_not_draw_or_select() {
    // Regression: a left press during a right-drag reached the tool.
    let mut h = harness(ViewportOptions::default());
    h.state_mut().panel.set_tool(Tool::Box);
    h.run();
    let at = egui::pos2(300.0, 420.0);
    h.hover_at(at);
    press(&h, PointerButton::Secondary, at, true);
    h.run();
    press(&h, PointerButton::Primary, at, true);
    h.run();
    h.hover_at(egui::pos2(480.0, 470.0));
    h.run();
    press(&h, PointerButton::Primary, egui::pos2(480.0, 470.0), false);
    h.run();
    press(
        &h,
        PointerButton::Secondary,
        egui::pos2(480.0, 470.0),
        false,
    );
    h.run();
    assert!(h.state().doc.is_empty(), "no box drawn");
}

#[test]
fn the_box_tool_shows_a_height_field_in_the_chosen_unit() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    assert!(
        h.query_by_label("Height").is_none(),
        "only for the Draw tool"
    );
    h.state_mut().panel.set_tool(Tool::Box);
    h.state_mut()
        .panel
        .set_display(halberd_config::LengthUnit::Metres, true);
    h.run();
    assert!(h.query_by_label("Height").is_some());
    let field = h.get_by_label(draw_options::NEW_BOX_HEIGHT_NAME);
    let value = field.value();
    assert!(
        value.as_deref().is_some_and(|v| v.contains("3.25")),
        "128 units in metres: {value:?}"
    );
}

#[test]
fn the_player_figure_stands_beside_the_selection_or_the_box_being_drawn() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    let player = h.state().renderer.views.last().unwrap().player.unwrap();
    assert_eq!(player.min.z, 0.0, "at the origin with nothing selected");
    assert_eq!(player.size(), Vec3::new(32.0, 32.0, 72.0));

    h.state_mut().panel.set_tool(Tool::Box);
    h.run();
    left_drag(&mut h, egui::pos2(300.0, 420.0), egui::pos2(480.0, 470.0));
    let selected = h.state().doc.selection_bounds().unwrap();
    let player = h.state().renderer.views.last().unwrap().player.unwrap();
    let outside = player.min.x > selected.max.x
        || player.max.x < selected.min.x
        || player.min.y > selected.max.y
        || player.max.y < selected.min.y;
    assert!(outside, "beside the new box, not inside it");
    assert_eq!(player.min.z, selected.min.z, "on its floor");
    let camera = h.state().panel.controller().camera().position;
    let figure_distance = player.center().truncate().distance(camera.truncate());
    let box_distance = selected.center().truncate().distance(camera.truncate());
    assert!(
        figure_distance < box_distance,
        "on the side facing the camera"
    );

    h.state_mut()
        .panel
        .set_display(halberd_config::LengthUnit::Units, false);
    h.run();
    assert!(h.state().renderer.views.last().unwrap().player.is_none());
}

/// Draws a box with the Draw tool, then switches to Select with the gizmo
/// in `mode`. The new box is selected.
fn box_with_gizmo(mode: halberd_tools::GizmoMode) -> Harness<'static, State> {
    let mut h = harness(ViewportOptions::default());
    h.state_mut().panel.set_tool(Tool::Box);
    h.run();
    left_drag(&mut h, egui::pos2(300.0, 420.0), egui::pos2(480.0, 470.0));
    h.state_mut().panel.tools_mut().set_gizmo_mode(Some(mode));
    h.run();
    h
}

/// Where a gizmo handle is on screen, in harness coordinates.
fn handle_on_screen(h: &Harness<'_, State>, handle: halberd_tools::Handle) -> egui::Pos2 {
    let rect = h.get_by_label(VIEWPORT_LABEL).rect();
    let state = h.state();
    let camera = *state.panel.controller().camera();
    let size = Vec2::new(rect.width(), rect.height());
    let shapes = state.panel.tools().gizmo_shapes(&camera, size, &state.doc);
    let shape = shapes
        .iter()
        .find(|s| s.handle == handle && !s.filled)
        .or_else(|| shapes.iter().find(|s| s.handle == handle))
        .expect("the handle is drawn");
    let p = if shape.filled {
        shape.points.iter().copied().sum::<Vec2>() / shape.points.len() as f32
    } else {
        (shape.points[0] + shape.points[1]) * 0.5
    };
    rect.min + egui::vec2(p.x, p.y)
}

#[test]
fn w_r_s_t_pick_gizmo_modes_and_the_same_key_turns_it_off() {
    use halberd_tools::GizmoMode;
    let mut h = harness(ViewportOptions::default());
    h.run();
    for (key, mode) in [
        (Key::W, GizmoMode::Move),
        (Key::R, GizmoMode::Rotate),
        (Key::S, GizmoMode::Scale),
        (Key::T, GizmoMode::All),
    ] {
        h.key_press(key);
        h.run();
        assert_eq!(h.state().panel.tools().gizmo_mode(), Some(mode));
    }
    h.key_press(Key::T);
    h.run();
    assert_eq!(h.state().panel.tools().gizmo_mode(), None);
}

#[test]
fn while_flying_shift_w_picks_move_but_w_alone_does_not() {
    use halberd_tools::GizmoMode;
    let mut h = harness(ViewportOptions::default());
    h.state_mut().panel.set_tool(Tool::Select);
    h.run();
    let at = egui::pos2(400.0, 300.0);
    h.hover_at(at);
    press(&h, PointerButton::Secondary, at, true);
    h.run();
    h.key_press(Key::W);
    h.run();
    assert_eq!(h.state().panel.tools().gizmo_mode(), None, "W flies");
    h.key_press_modifiers(Modifiers::SHIFT, Key::W);
    h.run();
    assert_eq!(h.state().panel.tools().gizmo_mode(), Some(GizmoMode::Move));
    h.key_press(Key::R);
    h.run();
    assert_eq!(
        h.state().panel.tools().gizmo_mode(),
        Some(GizmoMode::Rotate)
    );
    press(&h, PointerButton::Secondary, at, false);
    h.run();
}

#[test]
fn ctrl_s_is_not_a_gizmo_key() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::S);
    h.run();
    assert_eq!(h.state().panel.tools().gizmo_mode(), None, "kept for Save");
}

#[test]
fn the_toolbar_buttons_pick_gizmo_modes() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    h.get_by_label("Rotate R").click();
    h.run();
    assert_eq!(
        h.state().panel.tools().gizmo_mode(),
        Some(halberd_tools::GizmoMode::Rotate)
    );
}

#[test]
fn dragging_the_move_arrow_moves_the_box_in_one_undo_step() {
    use halberd_tools::{Axis, GizmoMode, Handle};
    let mut h = box_with_gizmo(GizmoMode::Move);
    let before = h.state().doc.selection_bounds().unwrap();
    let grab = handle_on_screen(&h, Handle::MoveAxis(Axis::X));
    h.hover_at(grab);
    press(&h, PointerButton::Primary, grab, true);
    h.run();
    for step in 1..=5 {
        h.hover_at(grab + egui::vec2(step as f32 * 12.0, 0.0));
        h.run();
    }
    press(
        &h,
        PointerButton::Primary,
        grab + egui::vec2(60.0, 0.0),
        false,
    );
    h.run();
    let after = h.state().doc.selection_bounds().unwrap();
    assert_ne!(after.min.x, before.min.x, "moved along X");
    assert_eq!(after.min.y, before.min.y);
    assert_eq!(after.min.z, before.min.z);
    assert_eq!(after.size(), before.size());
    assert_eq!(h.state().doc.selection().len(), 1, "still selected");
    assert_eq!(h.state_mut().doc.undo().as_deref(), Some("Move brush"));
    assert_eq!(
        h.state().doc.selection_bounds().unwrap(),
        before,
        "one step"
    );
}

#[test]
fn escape_cancels_a_gizmo_drag() {
    use halberd_tools::{Axis, GizmoMode, Handle};
    let mut h = box_with_gizmo(GizmoMode::Scale);
    let before = h.state().doc.selection_bounds().unwrap();
    let grab = handle_on_screen(&h, Handle::ScaleAxis(Axis::Z));
    h.hover_at(grab);
    press(&h, PointerButton::Primary, grab, true);
    h.run();
    h.hover_at(grab + egui::vec2(0.0, -80.0));
    h.run();
    assert_ne!(h.state().doc.selection_bounds().unwrap(), before);
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(h.state().doc.selection_bounds().unwrap(), before);
    assert_eq!(
        h.state().doc.selection().len(),
        1,
        "Escape kept the selection"
    );
    press(
        &h,
        PointerButton::Primary,
        grab + egui::vec2(0.0, -80.0),
        false,
    );
    h.run();
    assert_eq!(h.state().doc.selection_bounds().unwrap(), before);
    assert_eq!(h.state().doc.undo_label(), Some("Create brush"));
}

#[test]
fn a_held_gizmo_key_does_not_flicker() {
    // Regression: key repeats from a held W toggled Move on and off.
    let mut h = harness(ViewportOptions::default());
    h.run();
    // Hold W down: one press, then the repeats a held key sends.
    for repeat in [false, true, true, true, true, true] {
        h.event(egui::Event::Key {
            key: Key::W,
            physical_key: None,
            pressed: true,
            repeat,
            modifiers: Modifiers::NONE,
        });
        h.run();
    }
    h.event(egui::Event::Key {
        key: Key::W,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    h.run();
    assert_eq!(
        h.state().panel.tools().gizmo_mode(),
        Some(halberd_tools::GizmoMode::Move)
    );
}

#[test]
fn the_shape_picker_and_its_settings() {
    let mut h = harness(ViewportOptions::default());
    h.state_mut().panel.set_tool(Tool::Box);
    h.run();
    assert!(
        h.query_by_label(draw_options::SIDES_NAME).is_none(),
        "a box has none"
    );
    h.get_by_label(draw_options::SHAPE_PICKER_NAME).click();
    h.run();
    h.get_by_label("Cylinder").click();
    h.run();
    assert_eq!(
        h.state().panel.tools().shape(),
        halberd_geom::Shape::Cylinder
    );
    assert!(h.query_by_label(draw_options::SIDES_NAME).is_some());
    h.state_mut()
        .panel
        .tools_mut()
        .set_shape(halberd_geom::Shape::Arch);
    h.run();
    assert!(
        h.query_by_label(draw_options::ARCH_THICKNESS_NAME)
            .is_some()
    );
    h.state_mut()
        .panel
        .tools_mut()
        .set_shape(halberd_geom::Shape::Stairs);
    h.run();
    assert!(h.query_by_label(draw_options::STEP_HEIGHT_NAME).is_some());
    assert!(h.query_by_label(draw_options::SIDES_NAME).is_none());
}

#[test]
fn drawing_a_cylinder_previews_and_makes_a_cylinder() {
    let mut h = harness(ViewportOptions::default());
    h.state_mut().panel.set_tool(Tool::Box);
    h.state_mut()
        .panel
        .tools_mut()
        .set_shape(halberd_geom::Shape::Cylinder);
    h.run();
    let (from, to) = (egui::pos2(300.0, 420.0), egui::pos2(480.0, 470.0));
    h.hover_at(from);
    press(&h, PointerButton::Primary, from, true);
    h.run();
    h.hover_at(to);
    h.run();
    let view = h.state().renderer.views.last().unwrap().clone();
    assert_eq!(view.preview_shape.len(), 1, "the cylinder is outlined");
    assert_eq!(view.preview_shape[0].faces().len(), 8 + 2);
    press(&h, PointerButton::Primary, to, false);
    h.run();
    let doc = &h.state().doc;
    assert_eq!(doc.len(), 1);
    let brush = doc.objects().next().unwrap().1.as_brush().unwrap();
    assert_eq!(brush.brush().faces().len(), 8 + 2);
    assert!(
        h.state()
            .renderer
            .views
            .last()
            .unwrap()
            .preview_shape
            .is_empty(),
        "no outline after the drag"
    );
}

#[test]
fn typing_a_length_in_metres_is_not_rewritten_while_typing() {
    // Regression: each keystroke was stored rounded to whole units, and
    // the field then rewrote the text being typed ("1.5" became "0.5.99").
    let mut h = harness(ViewportOptions::default());
    h.state_mut().panel.set_tool(Tool::Box);
    h.state_mut()
        .panel
        .tools_mut()
        .set_shape(halberd_geom::Shape::Arch);
    h.state_mut()
        .panel
        .set_display(halberd_config::LengthUnit::Metres, true);
    h.run();
    h.get_by_label(draw_options::ARCH_THICKNESS_NAME).click();
    h.run();
    h.get_by_label(draw_options::ARCH_THICKNESS_NAME)
        .type_text("1.5");
    h.run();
    h.key_press(Key::Enter);
    h.run();
    // 1.5 m is 59.06 units, stored as 59.
    assert_eq!(
        h.state().panel.tools().shape_settings().arch_thickness,
        59.0
    );
}

#[test]
fn a_shape_that_cannot_be_made_says_why_while_dragging() {
    let mut h = harness(ViewportOptions::default());
    h.state_mut().panel.set_tool(Tool::Box);
    h.state_mut()
        .panel
        .tools_mut()
        .set_shape(halberd_geom::Shape::Stairs);
    h.state_mut().panel.tools_mut().set_box_height(4096.0);
    h.run();
    let (from, to) = (egui::pos2(300.0, 420.0), egui::pos2(480.0, 470.0));
    h.hover_at(from);
    press(&h, PointerButton::Primary, from, true);
    h.run();
    h.hover_at(to);
    h.run();
    assert!(h.query_by_label_contains("more than 256 pieces").is_some());
    press(&h, PointerButton::Primary, to, false);
    h.run();
    assert!(h.state().doc.is_empty());
    assert!(h.query_by_label_contains("more than 256 pieces").is_none());
}

/// Where a world point is on screen, in harness coordinates.
fn world_on_screen(h: &Harness<'_, State>, p: Vec3) -> egui::Pos2 {
    let rect = h.get_by_label(VIEWPORT_LABEL).rect();
    let camera = *h.state().panel.controller().camera();
    let at = camera
        .project(p, Vec2::new(rect.width(), rect.height()))
        .expect("in front of the camera");
    rect.min + egui::vec2(at.x, at.y)
}

/// The top corner of the selected box nearest the camera.
fn near_top_corner(h: &Harness<'_, State>) -> Vec3 {
    let state = h.state();
    let b = state.doc.selection_bounds().unwrap();
    let eye = state.panel.controller().camera().position;
    [
        Vec3::new(b.min.x, b.min.y, b.max.z),
        Vec3::new(b.max.x, b.min.y, b.max.z),
        Vec3::new(b.min.x, b.max.y, b.max.z),
        Vec3::new(b.max.x, b.max.y, b.max.z),
    ]
    .into_iter()
    .min_by(|p, q| p.distance(eye).total_cmp(&q.distance(eye)))
    .unwrap()
}

#[test]
fn keys_1_to_4_and_the_toolbar_pick_what_clicks_pick() {
    use halberd_tools::SelectMode;
    let mut h = harness(ViewportOptions::default());
    h.run();
    for (key, mode) in [
        (Key::Num2, SelectMode::Vertex),
        (Key::Num3, SelectMode::Edge),
        (Key::Num4, SelectMode::Face),
        (Key::Num1, SelectMode::Object),
    ] {
        h.key_press(key);
        h.run();
        assert_eq!(h.state().panel.tools().select_mode(), mode);
    }
    h.get_by_label("Vertex 2").click();
    h.run();
    assert_eq!(h.state().panel.tools().select_mode(), SelectMode::Vertex);
    // Ctrl+2 is not a mode key.
    h.key_press_modifiers(Modifiers::COMMAND, Key::Num4);
    h.run();
    assert_eq!(h.state().panel.tools().select_mode(), SelectMode::Vertex);
}

#[test]
fn escape_lets_go_of_picked_corners_first_then_deselects() {
    use halberd_tools::SelectMode;
    let mut h = box_with_gizmo(halberd_tools::GizmoMode::Move);
    h.key_press(Key::Num2);
    h.run();
    let corner = near_top_corner(&h);
    let at = world_on_screen(&h, corner);
    left_click(&mut h, at);
    assert_eq!(h.state().panel.tools().picked_elements().len(), 1);
    assert_eq!(h.state().panel.tools().select_mode(), SelectMode::Vertex);
    h.key_press(Key::Escape);
    h.run();
    assert!(h.state().panel.tools().picked_elements().is_empty());
    assert_eq!(h.state().doc.selection().len(), 1, "the box stays selected");
    h.key_press(Key::Escape);
    h.run();
    assert!(h.state().doc.selection().is_empty());
}

#[test]
fn dragging_a_picked_corner_reshapes_the_box_in_one_undo_step() {
    use halberd_tools::{Axis, GizmoMode, Handle};
    let mut h = box_with_gizmo(GizmoMode::Move);
    h.key_press(Key::Num2);
    h.run();
    let corner = near_top_corner(&h);
    let at = world_on_screen(&h, corner);
    left_click(&mut h, at);
    let grab = handle_on_screen(&h, Handle::MoveAxis(Axis::Z));
    h.hover_at(grab);
    press(&h, PointerButton::Primary, grab, true);
    h.run();
    for step in 1..=5 {
        h.hover_at(grab + egui::vec2(0.0, -step as f32 * 12.0));
        h.run();
    }
    press(
        &h,
        PointerButton::Primary,
        grab + egui::vec2(0.0, -60.0),
        false,
    );
    h.run();
    let state = h.state();
    let id = *state.doc.selection().iter().next().unwrap();
    let brush = state
        .doc
        .get(id)
        .unwrap()
        .as_brush()
        .unwrap()
        .brush()
        .clone();
    assert_eq!(
        brush.faces().len(),
        7,
        "the top folds where the corner rose"
    );
    assert!(brush.bounds().max.z > corner.z);
    assert_eq!(
        state.panel.tools().picked_elements().len(),
        1,
        "still picked"
    );
    assert_eq!(h.state_mut().doc.undo().as_deref(), Some("Move vertex"));
    let state = h.state();
    let brush = state
        .doc
        .get(id)
        .unwrap()
        .as_brush()
        .unwrap()
        .brush()
        .clone();
    assert_eq!(brush.faces().len(), 6, "one step");
}
