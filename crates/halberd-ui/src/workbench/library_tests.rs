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

const BARREL: &str = "models/props_c17/oildrum001.mdl";

fn barrel_box() -> Aabb {
    Aabb::from_corners(Vec3::new(-14.0, -14.0, 0.0), Vec3::new(14.0, 14.0, 46.0))
}

#[test]
fn picking_a_prop_starts_placing_it_once_its_model_is_read() {
    let mut wb = Workbench::new(info(), None);
    wb.materials_mut().set_names(vec![BRICK.to_string()]);
    wb.props_mut()
        .set_names(vec!["models/props_c17/oildrum001.mdl".to_string()]);
    let mut h = harness_for(wb);
    h.run();
    h.get_by_label(PROPS_TAB).click();
    h.run();
    assert!(h.query_by_label("1 of 1 props").is_some());
    h.get_by_label("models/props_c17/oildrum001.mdl").click();
    h.run();
    // Not read yet: nothing to place until it arrives.
    assert_eq!(
        h.state().viewport().tools().tool(),
        halberd_tools::Tool::Select
    );
    h.state_mut().add_model_info(BARREL, barrel_box(), true);
    h.run();
    assert_eq!(
        h.state().viewport().tools().tool(),
        halberd_tools::Tool::Place
    );
    assert!(h.query_by_label_contains(crate::PLACING_LABEL).is_some());
    // Picking it again, now known, starts placing at once.
    h.state_mut().viewport_mut().tools_mut().set_placing(None);
    // The tile (the picked name is shown above the grid too).
    h.query_all_by_label("models/props_c17/oildrum001.mdl")
        .last()
        .unwrap()
        .click();
    h.run();
    assert_eq!(
        h.state().viewport().tools().tool(),
        halberd_tools::Tool::Place
    );
    h.get_by_label("Stop").click();
    h.run();
    assert_eq!(
        h.state().viewport().tools().tool(),
        halberd_tools::Tool::Select
    );
}

#[test]
fn a_newly_opened_map_knows_the_boxes_of_models_already_read() {
    // Regression: model boxes lived only in the old document, so after
    // opening a map props were picked by a tiny marker box.
    let mut wb = Workbench::new(info(), None);
    wb.add_model_info(BARREL, barrel_box(), true);
    let mut doc = Document::new();
    doc.execute(Command::AddEntity(halberd_doc::EntityObject::point(
        "prop_static",
        Vec3::ZERO,
        &[("model", "models/props_c17/oildrum001.mdl")],
    )))
    .unwrap();
    wb.set_document(doc, None);
    let id = wb.document().objects().next().unwrap().0;
    assert_eq!(wb.document().bounds_of(id).unwrap().max.z, 46.0);
}

#[test]
fn a_picked_prop_that_cant_be_read_says_why() {
    let mut wb = Workbench::new(info(), None);
    wb.props_mut()
        .set_names(vec!["models/broken.mdl".to_string()]);
    let mut h = harness_for(wb);
    h.run();
    h.get_by_label(PROPS_TAB).click();
    h.run();
    h.get_by_label("models/broken.mdl").click();
    h.run();
    h.state_mut()
        .prop_failed("models/broken.mdl", "its vertices (.vvd file) are missing");
    h.run();
    assert!(h.query_by_label_contains("can't be placed").is_some());
    assert_eq!(
        h.state().viewport().tools().tool(),
        halberd_tools::Tool::Select
    );
}
