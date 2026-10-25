//! Tests for the Library panel's material browser in the workbench.

use super::tests::{harness_for, info};
use super::*;
use crate::materials::APPLY_MATERIAL_LABEL;
use egui_kittest::kittest::Queryable;
use glam::Vec3;
use halberd_doc::Object;
use halberd_geom::{Aabb, Brush};

const BRICK: &str = "BRICK/BRICKWALL001A";

fn workbench_with_cube_and_materials() -> Workbench {
    let mut wb = Workbench::new(info(), None);
    let cube = Brush::cuboid(Aabb::from_corners(Vec3::ZERO, Vec3::splat(64.0))).unwrap();
    wb.document_mut()
        .execute(Command::AddBrushes(vec![cube]))
        .unwrap();
    wb.materials_mut().set_names(vec![
        BRICK.to_string(),
        "DEV/DEV_MEASUREWALL01A".to_string(),
    ]);
    wb
}

fn face_materials(doc: &Document) -> Vec<String> {
    doc.objects()
        .filter_map(|(_, o)| match o {
            Object::Brush(b) => Some(b.faces().iter().map(|f| f.material.clone())),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn applying_a_material_paints_the_selection_and_can_be_undone() {
    let mut h = harness_for(workbench_with_cube_and_materials());
    h.run();
    let before = face_materials(h.state().document());
    h.get_by_label(BRICK).click();
    h.run();
    assert_eq!(h.state().viewport().tools().material(), Some(BRICK));
    assert_eq!(
        face_materials(h.state().document()),
        before,
        "a click only picks"
    );
    h.get_by_label(APPLY_MATERIAL_LABEL).click();
    h.run();
    let after = face_materials(h.state().document());
    assert_eq!(after.len(), 6);
    assert!(after.iter().all(|m| m == BRICK), "{after:?}");
    h.state_mut().undo();
    assert_eq!(face_materials(h.state().document()), before);
}

#[test]
fn without_a_game_the_library_says_how_to_find_one() {
    let mut h = harness_for(Workbench::new(info(), None));
    h.run();
    assert!(h.query_by_label_contains("--gmod-dir").is_some());
    h.state_mut()
        .set_content_status(ContentStatus::Mounted("12 archives, 3 materials".into()));
    h.run();
    assert!(h.query_by_label_contains("12 archives").is_some());
}

#[test]
fn applying_with_nothing_selected_warns_instead() {
    let mut wb = workbench_with_cube_and_materials();
    wb.document_mut().clear_selection();
    wb.browser_action(crate::materials::BrowserAction::Apply(BRICK.into()));
    assert_eq!(wb.viewport().tools().material(), Some(BRICK));
    assert!(face_materials(wb.document()).iter().all(|m| m != BRICK));
}
