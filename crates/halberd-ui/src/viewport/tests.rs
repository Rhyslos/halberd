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
        self.views.push(*view);
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
    let view = *h.state().renderer.views.last().unwrap();
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
fn b_switches_between_select_and_box() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    assert_eq!(h.state().panel.tool(), Tool::Select);
    h.key_press(Key::B);
    h.run();
    assert_eq!(h.state().panel.tool(), Tool::Box);
    h.key_press(Key::B);
    h.run();
    assert_eq!(h.state().panel.tool(), Tool::Select);
}

#[test]
fn tool_switcher_changes_tool_without_drawing() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    h.get_by_label("Box").click();
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
