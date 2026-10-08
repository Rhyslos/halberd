//! Tests for brush entities in the interface: picking their brushes one
//! by one, the Scene list, Properties, and deleting.

use super::tests::{harness_for, info};
use super::*;
use crate::INSIDE_ENTITIES_LABEL;
use egui_kittest::kittest::Queryable;
use halberd_doc::{BrushObject, EntityObject, MapObject, ObjectId};

/// A func_detail with three boxes, and a world box.
fn workbench_with_detail() -> (Workbench, ObjectId, Vec<ObjectId>) {
    let cube = |x: f32| {
        halberd_geom::Brush::cuboid(halberd_geom::Aabb::from_corners(
            glam::Vec3::new(x, 0.0, 0.0),
            glam::Vec3::new(x + 64.0, 64.0, 64.0),
        ))
        .unwrap()
    };
    let detail = MapObject::Entity(
        EntityObject {
            classname: "func_detail".into(),
            origin: None,
            file_data: Vec::new(),
        },
        (0..3)
            .map(|i| BrushObject::new(cube(i as f32 * 100.0)))
            .collect(),
    );
    let world = MapObject::Brush(BrushObject::new(cube(-200.0)));
    let doc = halberd_doc::Document::from_map(vec![world, detail], Default::default()).unwrap();
    let entity = doc.objects().nth(1).unwrap().0;
    let brushes = doc.brushes_of(entity).collect();
    let mut wb = Workbench::new(info(), None);
    wb.set_document(doc, Some("detail.vmf".into()));
    (wb, entity, brushes)
}

#[test]
fn ctrl_w_and_the_toolbar_button_switch_picking_inside_entities() {
    let (wb, ..) = workbench_with_detail();
    let mut h = harness_for(wb);
    h.run();
    let inside =
        |h: &egui_kittest::Harness<'_, Workbench>| h.state().viewport().tools().inside_entities();
    assert!(!inside(&h));
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::W);
    h.run();
    assert!(inside(&h));
    h.get_by_label_contains(INSIDE_ENTITIES_LABEL).click();
    h.run();
    assert!(!inside(&h));
    // Plain W is still the Move gizmo, not this.
    h.key_press(egui::Key::W);
    h.run();
    assert!(!inside(&h));
}

#[test]
fn scene_folds_an_entitys_brushes_under_it() {
    let (wb, entity, brushes) = workbench_with_detail();
    let mut h = harness_for(wb);
    h.run();
    let row = format!("func_detail {entity} (3 brushes)");
    assert!(h.query_by_label(&row).is_some());
    let brush_row = format!("Brush {}", brushes[1]);
    assert!(h.query_by_label(&brush_row).is_none(), "folded at first");
    h.get_by_label(&format!("Unfold func_detail {entity}"))
        .click();
    h.run();
    // Clicking a brush in the list selects that brush alone.
    h.get_by_label(&brush_row).click();
    h.run();
    let selected: Vec<_> = h.state().document().selection().iter().copied().collect();
    assert_eq!(selected, [brushes[1]]);
    // Properties says what it belongs to, and it has size fields.
    assert!(
        h.query_by_label(&format!("Part of func_detail {entity}"))
            .is_some()
    );
    assert!(h.query_by_label("Box width").is_some());
    // Clicking the entity's line selects the whole entity.
    h.get_by_label(&row).click();
    h.run();
    let selected: Vec<_> = h.state().document().selection().iter().copied().collect();
    assert_eq!(selected, [entity]);
    assert!(h.query_by_label("3 brushes").is_some());
}

#[test]
fn a_selected_brush_inside_an_entity_unfolds_its_list() {
    let (mut wb, _, brushes) = workbench_with_detail();
    wb.document_mut().set_selection([brushes[2]]);
    let mut h = harness_for(wb);
    h.run();
    assert!(h.query_by_label(&format!("Brush {}", brushes[2])).is_some());
}

#[test]
fn resizing_and_deleting_a_brush_inside_an_entity() {
    let (mut wb, entity, brushes) = workbench_with_detail();
    wb.document_mut().set_selection([brushes[0]]);
    let mut h = harness_for(wb);
    h.run();
    h.get_by_label("Box height").click();
    h.run();
    h.get_by_label("Box height").type_text("128");
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
    let doc = h.state().document();
    let brush = doc.get(brushes[0]).unwrap().as_brush().unwrap();
    assert_eq!(brush.brush().bounds().size().z, 128.0);
    assert_eq!(brush.entity(), Some(entity), "still in the func_detail");
    // Delete takes only that brush.
    h.key_press(egui::Key::Delete);
    h.run();
    let doc = h.state().document();
    assert_eq!(doc.brushes_of(entity).count(), 2);
    assert!(doc.get(entity).is_some());
}

#[test]
fn ctrl_w_is_not_a_plain_w_even_when_ctrl_is_let_go_quickly() {
    // Regression: with Ctrl released before the frame read the key (as in
    // the real window), Ctrl+W also switched on the Move gizmo, and B or W
    // checked the modifiers held at the end of the frame, not at the press.
    let (wb, ..) = workbench_with_detail();
    let mut h = harness_for(wb);
    h.run();
    h.event(egui::Event::Key {
        key: egui::Key::W,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    });
    h.run();
    let tools = h.state().viewport().tools();
    assert!(tools.inside_entities());
    assert_eq!(tools.gizmo_mode(), None, "not the Move gizmo");
}

#[test]
fn a_list_opened_by_a_selection_can_still_be_folded() {
    // Regression: while one of its brushes was selected, the fold arrow did
    // nothing and then left the list stuck open.
    let (mut wb, entity, brushes) = workbench_with_detail();
    wb.document_mut().set_selection([brushes[0]]);
    let mut h = harness_for(wb);
    h.run();
    let brush_row = format!("Brush {}", brushes[0]);
    assert!(h.query_by_label(&brush_row).is_some());
    h.get_by_label(&format!("Fold func_detail {entity}"))
        .click();
    h.run();
    assert!(h.query_by_label(&brush_row).is_none(), "folded");
    h.get_by_label(&format!("Unfold func_detail {entity}"))
        .click();
    h.run();
    h.state_mut().document_mut().clear_selection();
    h.run();
    assert!(
        h.query_by_label(&brush_row).is_some(),
        "opened by hand stays open"
    );
}

#[test]
fn an_entity_without_a_box_still_shows_its_details() {
    // Regression: an entity with no origin and no shown brushes made
    // Properties say "Nothing selected".
    let entity = MapObject::Entity(
        EntityObject {
            classname: "func_brush".into(),
            origin: None,
            file_data: Vec::new(),
        },
        Vec::new(),
    );
    let doc = halberd_doc::Document::from_map(vec![entity], Default::default()).unwrap();
    let id = doc.objects().next().unwrap().0;
    let mut wb = Workbench::new(info(), None);
    wb.set_document(doc, None);
    wb.document_mut().set_selection([id]);
    let mut h = harness_for(wb);
    h.run();
    assert!(h.query_by_label("Nothing selected").is_none());
    assert!(
        h.query_by_label("Editing entities comes in Phase 2.")
            .is_some()
    );
}
