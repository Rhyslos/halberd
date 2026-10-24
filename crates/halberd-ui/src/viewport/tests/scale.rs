//! The player figure's clearance check and following the pointer.

use super::*;
use halberd_doc::Command;
use halberd_geom::Brush;
use halberd_tools::Fit;

fn cuboid(min: Vec3, max: Vec3) -> Brush {
    Brush::cuboid(Aabb::from_corners(min, max)).unwrap()
}

fn last_view(h: &Harness<'_, State>) -> ViewportView {
    h.state().renderer.views.last().unwrap().clone()
}

#[test]
fn under_a_low_ceiling_the_figure_says_crouch_only() {
    let mut h = harness(ViewportOptions::default());
    // A floor, a crate on it (selected), and a ceiling 48 units up.
    h.state_mut()
        .doc
        .execute(Command::AddBrushes(vec![
            cuboid(
                Vec3::new(-512.0, -512.0, -16.0),
                Vec3::new(512.0, 512.0, 0.0),
            ),
            cuboid(
                Vec3::new(-512.0, -512.0, 48.0),
                Vec3::new(512.0, 512.0, 64.0),
            ),
            cuboid(Vec3::ZERO, Vec3::splat(16.0)),
        ]))
        .unwrap();
    let crate_id = h.state().doc.objects().last().unwrap().0;
    h.state_mut().doc.set_selection([crate_id]);
    h.run();
    let view = last_view(&h);
    assert_eq!(view.player.unwrap().min.z, 0.0);
    assert_eq!(view.player_fit, Some(Fit::CrouchOnly));
    let clearance = h.state().panel.player_clearance().unwrap();
    assert_eq!(clearance.headroom, Some(12.0));
}

#[test]
fn the_figure_can_follow_the_pointer_onto_what_is_under_it() {
    let mut h = harness(ViewportOptions::default());
    h.state_mut()
        .doc
        .execute(Command::AddBrushes(vec![cuboid(
            Vec3::new(-2048.0, -2048.0, -16.0),
            Vec3::new(2048.0, 2048.0, 32.0),
        )]))
        .unwrap();
    h.state_mut().doc.clear_selection();
    h.state_mut().panel.set_player_follows(true);
    assert!(h.state().panel.player_follows());
    h.hover_at(egui::pos2(400.0, 300.0));
    h.run();
    let view = last_view(&h);
    let player = view.player.unwrap();
    assert!(
        (player.min.z - 32.0).abs() < 0.01,
        "on top of the slab: {player:?}"
    );
    assert_eq!(view.player_fit, Some(Fit::Stands));
    // Turned off, it goes back beside the selection (the origin).
    h.state_mut().panel.set_player_follows(false);
    h.run();
    assert_eq!(last_view(&h).player.unwrap().min.z, 0.0);
}
