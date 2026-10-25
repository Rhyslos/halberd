//! Drawing floor plans in the viewport: clicks, Enter and Backspace.

use super::*;

/// Four corners on the ground in front of the default camera.
const CORNERS: [egui::Pos2; 4] = [
    egui::pos2(300.0, 420.0),
    egui::pos2(500.0, 420.0),
    egui::pos2(500.0, 500.0),
    egui::pos2(300.0, 500.0),
];

fn plan_harness() -> Harness<'static, State> {
    let mut h = harness(ViewportOptions::default());
    h.state_mut().panel.set_tool(Tool::Box);
    h.state_mut()
        .panel
        .tools_mut()
        .set_shape(halberd_geom::Shape::FloorPlan);
    h.run();
    h
}

fn shapes(h: &Harness<'_, State>) -> usize {
    h.state()
        .doc
        .objects()
        .filter(|(_, o)| o.as_shape().is_some())
        .count()
}

#[test]
fn clicking_corners_then_enter_makes_a_shape() {
    let mut h = plan_harness();
    for at in CORNERS {
        left_click(&mut h, at);
    }
    let preview = h.state().panel.tools().plan_preview().unwrap();
    assert_eq!(preview.corners.len(), 4);
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(shapes(&h), 1, "notes: {:?}", h.state().notes);
    assert!(!h.state().panel.tools().is_drawing_plan());
}

#[test]
fn backspace_takes_back_a_corner_and_deletes_nothing() {
    let mut h = plan_harness();
    // A shape already there and selected must survive Backspace.
    for at in CORNERS {
        left_click(&mut h, at);
    }
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(shapes(&h), 1);
    for at in &CORNERS[..3] {
        left_click(&mut h, *at + egui::vec2(0.0, -150.0));
    }
    h.key_press(Key::Backspace);
    h.run();
    let preview = h.state().panel.tools().plan_preview().unwrap();
    assert_eq!(preview.corners.len(), 2);
    assert_eq!(shapes(&h), 1, "the selected shape is still there");
    h.key_press(Key::Escape);
    h.run();
    assert!(!h.state().panel.tools().is_drawing_plan());
}

#[test]
fn a_click_down_and_up_within_one_frame_still_places_a_corner() {
    // Regression: a quick click (down and up before the next frame) was
    // never seen by the tools, so corners went missing.
    let mut h = plan_harness();
    for at in CORNERS {
        h.hover_at(at);
        press(&h, PointerButton::Primary, at, true);
        press(&h, PointerButton::Primary, at, false);
        h.run();
    }
    let preview = h.state().panel.tools().plan_preview().unwrap();
    assert_eq!(preview.corners.len(), 4);
}
