//! Several views: layouts, what each view shows, and which view gets the
//! mouse.

use super::*;
use halberd_tools::{Side, SideCamera};

/// The middle of each view of the Four layout in the 800 × 600 harness.
const QUARTERS: [egui::Pos2; 4] = [
    egui::pos2(200.0, 150.0),
    egui::pos2(600.0, 150.0),
    egui::pos2(200.0, 450.0),
    egui::pos2(600.0, 450.0),
];

fn four_views() -> Harness<'static, State> {
    let mut h = harness(ViewportOptions::default());
    h.state_mut().panel.set_layout(ViewLayout::Four);
    h.run();
    h
}

fn last_views(h: &Harness<'_, State>, count: usize) -> Vec<ViewportView> {
    let views = &h.state().renderer.views;
    views[views.len() - count..].to_vec()
}

#[test]
fn four_views_are_hammers_and_each_has_its_own_image_and_grid() {
    let h = four_views();
    let views = last_views(&h, 4);
    assert_eq!(views.iter().map(|v| v.id).collect::<Vec<_>>(), [0, 1, 2, 3]);
    assert_eq!(
        views.iter().map(|v| v.grid_axis).collect::<Vec<_>>(),
        [2, 2, 0, 1],
        "ground, Top, Front, Right"
    );
    assert_eq!(
        views.iter().map(|v| v.grid_fade).collect::<Vec<_>>(),
        [true, false, false, false]
    );
    // Each image is about a quarter of the panel.
    assert!(
        views
            .iter()
            .all(|v| v.size_px[0] < 420 && v.size_px[1] < 320)
    );
    assert!(h.query_by_label("View 4").is_some());
}

#[test]
fn the_views_menu_picks_a_layout() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    h.get_by_label_contains(LAYOUT_PICKER_LABEL).click();
    h.run();
    h.get_by_label("Six").click();
    h.run();
    assert_eq!(h.state().panel.layout(), ViewLayout::Six);
    h.run();
    assert_eq!(last_views(&h, 6).last().unwrap().id, 5);
}

#[test]
fn a_views_name_is_a_menu_of_what_it_shows() {
    let mut h = harness(ViewportOptions::default());
    h.run();
    h.get_by_label_contains("Perspective").click();
    h.run();
    h.get_by_label("Bottom (X/Y)").click();
    h.run();
    assert_eq!(h.state().panel.view(0), PaneView::Side(Side::Bottom));
    h.run();
    let view = last_views(&h, 1).remove(0);
    assert_eq!((view.grid_axis, view.grid_fade), (2, false));
    assert!(
        view.grid_offset > 1000.0,
        "Bottom's grid lies above the map"
    );
}

#[test]
fn dragging_with_the_right_button_moves_only_the_view_under_the_pointer() {
    let mut h = four_views();
    let camera_before = *h.state().panel.controller().camera();
    let top_before = h.state().panel.view_setup().sides[1];
    let (from, to) = (QUARTERS[1], QUARTERS[1] + egui::vec2(40.0, 0.0));
    h.hover_at(from);
    h.run();
    press(&h, PointerButton::Secondary, from, true);
    h.run();
    h.hover_at(to);
    h.run();
    press(&h, PointerButton::Secondary, to, false);
    h.run();
    let top_after = h.state().panel.view_setup().sides[1];
    assert!(
        top_after.center.x < top_before.center.x - 1.0,
        "the Top view moved"
    );
    assert_eq!(*h.state().panel.controller().camera(), camera_before);
}

#[test]
fn shapes_are_drawn_in_the_top_view_but_not_yet_in_side_views() {
    let mut h = four_views();
    h.state_mut().panel.set_tool(Tool::Box);
    h.run();
    left_drag(&mut h, QUARTERS[2], QUARTERS[2] + egui::vec2(60.0, 30.0));
    assert!(h.state().doc.is_empty(), "nothing drawn in Front");
    assert!(
        h.state().notes.iter().any(|n| n.contains("Front (Y/Z)")),
        "{:?}",
        h.state().notes
    );
    left_drag(
        &mut h,
        QUARTERS[1] - egui::vec2(40.0, 30.0),
        QUARTERS[1] + egui::vec2(40.0, 30.0),
    );
    assert_eq!(h.state().doc.len(), 1, "a box drawn in Top");
    let bounds = h.state().doc.objects().next().unwrap().1.bounds().unwrap();
    assert_eq!((bounds.min.z, bounds.max.z), (0.0, 128.0));
    assert!(bounds.size().x > 32.0 && bounds.size().y > 32.0);
}

#[test]
fn a_drag_stays_with_the_view_it_began_in() {
    let mut h = four_views();
    h.state_mut().panel.set_tool(Tool::Box);
    h.run();
    h.hover_at(QUARTERS[1]);
    h.run();
    press(&h, PointerButton::Primary, QUARTERS[1], true);
    h.run();
    h.hover_at(QUARTERS[0]);
    h.run();
    assert_eq!(h.state().panel.active_view(), 1, "still the Top view");
    press(&h, PointerButton::Primary, QUARTERS[0], false);
    h.run();
    h.hover_at(QUARTERS[0] + egui::vec2(1.0, 0.0));
    h.run();
    assert_eq!(
        h.state().panel.active_view(),
        0,
        "the pointer's view once let go"
    );
}

#[test]
fn the_views_come_back_as_saved_and_bad_values_are_repaired() {
    let mut h = four_views();
    h.state_mut().panel.set_view(3, PaneView::Side(Side::Left));
    let setup = h.state().panel.view_setup();
    let restored =
        ViewportPanel::new(None, ViewportOptions::default()).with_view_setup(setup.clone());
    assert_eq!(restored.view_setup(), setup);
    let mut broken = setup;
    broken.sides[1] = SideCamera {
        side: Side::Back,
        center: glam::Vec3::NAN,
        height: -5.0,
    };
    broken.views.truncate(2);
    let repaired = ViewportPanel::new(None, ViewportOptions::default()).with_view_setup(broken);
    let side = repaired.view_setup().sides[1];
    assert_eq!(side.side, Side::Top, "follows what the view shows");
    assert!(side.center.is_finite() && side.height > 0.0);
    assert_eq!(
        repaired.view(3),
        PaneView::Side(Side::Right),
        "missing views keep defaults"
    );
}

#[test]
fn a_click_in_a_side_view_selects_what_is_under_it() {
    let mut h = four_views();
    let cube = halberd_geom::Brush::cuboid(halberd_geom::Aabb::from_corners(
        glam::Vec3::splat(-128.0),
        glam::Vec3::splat(128.0),
    ))
    .unwrap();
    h.state_mut()
        .doc
        .execute(halberd_doc::Command::AddBrushes(vec![cube]))
        .unwrap();
    h.state_mut().doc.clear_selection();
    h.run();
    // The Front view shows the origin in its middle.
    left_click(&mut h, QUARTERS[2]);
    assert_eq!(h.state().doc.selection().len(), 1, "picked in Front");
    left_click(&mut h, QUARTERS[2] + egui::vec2(150.0, 0.0));
    assert!(h.state().doc.selection().is_empty(), "nothing there");
    left_click(&mut h, QUARTERS[3]);
    assert_eq!(h.state().doc.selection().len(), 1, "picked in Right");
}
