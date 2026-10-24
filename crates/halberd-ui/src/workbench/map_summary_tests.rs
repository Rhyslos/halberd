//! Tests for the map summary in Properties (nothing selected).

use super::tests::{harness_for, info};
use super::*;
use crate::panels::map_summary::{MAP_HEADING, SELECT_OUTSIDE_LABEL};
use egui_kittest::kittest::Queryable;
use glam::Vec3;
use halberd_geom::{Aabb, Brush};

fn cube(at: f32, size: f32) -> Brush {
    Brush::cuboid(Aabb::from_corners(Vec3::splat(at), Vec3::splat(at + size))).unwrap()
}

#[test]
fn with_nothing_selected_properties_shows_the_maps_size_and_limits() {
    let mut wb = Workbench::new(info(), None);
    wb.document_mut()
        .execute(Command::AddBrushes(vec![
            cube(0.0, 64.0),
            cube(16_300.0, 256.0),
        ]))
        .unwrap();
    wb.document_mut().clear_selection();
    let mut h = harness_for(wb);
    h.run();
    assert!(h.query_by_label(MAP_HEADING).is_some());
    assert!(
        h.query_by_label_contains("Size: 16556 × 16556 × 16556")
            .is_some()
    );
    assert!(h.query_by_label("Brushes: 2 of 8192").is_some());
    assert!(h.query_by_label("Brush sides: 12 of 65536").is_some());
    // One brush reaches past the edge; the button selects it.
    assert!(
        h.query_by_label_contains("1 object reaches past the map's edge")
            .is_some()
    );
    h.get_by_label(SELECT_OUTSIDE_LABEL).click();
    h.run();
    let selected: Vec<_> = h.state().document().selection().iter().copied().collect();
    assert_eq!(selected.len(), 1);
    let far = h.state().document().bounds_of(selected[0]).unwrap();
    assert_eq!(far.max.x, 16_556.0);
}

#[test]
fn an_empty_map_says_so() {
    let mut h = harness_for(Workbench::new(info(), None));
    h.run();
    assert!(h.query_by_label("Empty").is_some());
    assert!(h.query_by_label("Map entities: 1 of 8192").is_some());
}
