//! Tests for the full entity list and the keyvalue editor.

use super::tests::{harness_for, info};
use super::*;
use crate::panels::{
    ADD_KEY_LABEL, CLASS_FIELD_NAME, MOVE_TO_WORLD_LABEL, NEW_KEY_NAME, NEW_VALUE_NAME,
    value_field_name,
};
use egui_kittest::kittest::Queryable;
use glam::Vec3;
use halberd_doc::{EntityObject, ObjectId};
use halberd_geom::{Aabb, Brush};
use halberd_tools::Tool;

const DEFS: &str = r#"
@BaseClass = Targetname [ targetname(target_source) : "Name" : : "What others call it." ]
@PointClass base(Targetname) size(-8 -8 0, 8 8 40) = item_crate : "A crate. Break it."
[
    health(integer) : "Health" : 25 : "How much damage it takes."
    material(choices) : "Material" : 1 =
    [
        0 : "Glass"
        1 : "Wood"
    ]
    spawnflags(flags) = [ 1 : "Start asleep" : 1  2 : "Glow" : 0 ]
    input Break(void) : "Breaks it."
    output OnBreak(void) : "When it breaks."
]
@SolidClass base(Targetname) = func_wall : "A wall that can be named." []
@SolidClass base(Targetname) = func_brush : "A brush." [ _lightHDR(color255) : "HDR colour" : "-1 -1 -1 1" ]
@SolidClass = worldspawn : "The world." []
"#;

fn with_defs() -> Workbench {
    let mut wb = Workbench::new(info(), None);
    wb.set_entity_definitions(halberd_fgd::parse(DEFS, "test.fgd"), "test.fgd");
    wb
}

fn add_crate(wb: &mut Workbench, settings: &[(&str, &str)]) -> ObjectId {
    wb.document_mut()
        .execute(Command::AddEntity(EntityObject::point(
            "item_crate",
            Vec3::new(64.0, 0.0, 0.0),
            settings,
        )))
        .unwrap();
    wb.document().objects().last().unwrap().0
}

fn entity(wb: &Workbench, id: ObjectId) -> EntityObject {
    wb.document().get(id).unwrap().as_entity().unwrap().clone()
}

#[test]
fn the_list_searches_and_places_with_defaults_and_size() {
    let mut h = harness_for(with_defs());
    h.run();
    assert!(
        h.state()
            .console()
            .entries()
            .any(|e| e.message.starts_with("Entity definitions: 3 entities"))
    );
    h.get_by_label(ENTITIES_TAB).click();
    h.run();
    h.get_by_label(ENTITY_SEARCH_NAME).click();
    h.run();
    h.event(egui::Event::Text("crate".into()));
    h.run();
    assert!(h.query_by_label("func_wall  (brush)").is_none());
    h.get_by_label("item_crate").click();
    h.run();
    let tools = h.state().viewport().tools();
    assert_eq!(tools.tool(), Tool::Place);
    let placing = tools.placing().unwrap();
    assert_eq!(placing.classname, "item_crate");
    assert_eq!(placing.local.max.z, 40.0);
    assert!(placing.settings.contains(&("health".into(), "25".into())));
    assert!(
        placing
            .settings
            .contains(&("spawnflags".into(), "1".into()))
    );
}

#[test]
fn point_entities_are_shown_at_their_defined_size() {
    let mut wb = with_defs();
    let id = add_crate(&mut wb, &[]);
    assert_eq!(wb.document().bounds_of(id).unwrap().max.z, 40.0);
    // Another map gets the sizes too.
    wb.new_document();
    let id = add_crate(&mut wb, &[]);
    assert_eq!(wb.document().bounds_of(id).unwrap().max.z, 40.0);
}

#[test]
fn brushes_are_tied_to_a_brush_entity_and_moved_back() {
    let mut wb = with_defs();
    let cube = Brush::cuboid(Aabb::from_corners(Vec3::ZERO, Vec3::splat(64.0))).unwrap();
    wb.document_mut()
        .execute(Command::AddBrushes(vec![cube]))
        .unwrap();
    let brush = wb.document().objects().last().unwrap().0;
    let mut h = harness_for(wb);
    h.run();
    h.get_by_label(ENTITIES_TAB).click();
    h.run();
    h.get_by_label("func_wall  (brush)").click();
    h.run();
    let owner = h
        .state()
        .document()
        .get(brush)
        .unwrap()
        .as_brush()
        .unwrap()
        .entity()
        .expect("tied");
    assert!(h.state().document().selection().contains(&owner));
    assert_eq!(entity(h.state(), owner).classname, "func_wall");
    // Clicking another brush class turns it into that and keeps it
    // selected (regression: the selection emptied).
    h.get_by_label("func_brush  (brush)").click();
    h.run();
    let new_owner = h
        .state()
        .document()
        .get(brush)
        .unwrap()
        .as_brush()
        .unwrap()
        .entity()
        .unwrap();
    assert!(h.state().document().selection().contains(&new_owner));
    assert!(h.state().document().get(owner).is_none(), "replaced");
    // An HDR colour of -1s is text, not a black colour button.
    assert!(h.query_by_label(&value_field_name("_lightHDR")).is_some());
    h.get_by_label(MOVE_TO_WORLD_LABEL).click();
    h.run();
    assert!(h.state().document().get(new_owner).is_none());
    assert!(
        h.state().document().selection().contains(&brush),
        "the brushes stay selected"
    );
    let brush = h.state().document().get(brush).unwrap().as_brush().unwrap();
    assert_eq!(brush.entity(), None);
    // The world itself is never offered.
    assert!(h.query_by_label("worldspawn  (brush)").is_none());
}

#[test]
fn brush_entities_wait_for_a_selection() {
    let mut h = harness_for(with_defs());
    h.run();
    h.get_by_label(ENTITIES_TAB).click();
    h.run();
    h.get_by_label("func_wall  (brush)").click();
    h.run();
    assert!(h.state().document().is_empty());
}

#[test]
fn keys_are_edited_by_type_and_text_goes_in_on_enter() {
    let mut wb = with_defs();
    let id = add_crate(&mut wb, &[("health", "25")]);
    let mut h = harness_for(wb);
    h.run();
    // The class's keys are listed with their labels; inputs and outputs too.
    assert!(h.query_by_label("Health").is_some());
    assert!(h.query_by_label("Start asleep").is_some());
    assert!(h.query_by_label("Inputs it takes (1)").is_some());
    // Flags: ticking Glow adds its bit.
    h.get_by_label("Glow").click();
    h.run();
    assert_eq!(entity(h.state(), id).setting("spawnflags"), Some("2"));
    // Text: typing changes nothing until Enter.
    h.get_by_label(&value_field_name("targetname")).click();
    h.run();
    h.event(egui::Event::Text("crate1".into()));
    h.run();
    assert_eq!(entity(h.state(), id).setting("targetname"), None);
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(entity(h.state(), id).setting("targetname"), Some("crate1"));
    // Escape puts the text back.
    h.get_by_label(&value_field_name("health")).click();
    h.run();
    h.event(egui::Event::Text("0".into()));
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(entity(h.state(), id).setting("health"), Some("25"));
}

#[test]
fn keys_are_added_removed_and_the_class_changed() {
    let mut wb = with_defs();
    let id = add_crate(&mut wb, &[("health", "25"), ("custom", "x")]);
    let mut h = harness_for(wb);
    h.run();
    // A key the definitions don't know is listed as text.
    assert!(h.query_by_label(&value_field_name("custom")).is_some());
    h.get_by_label(NEW_KEY_NAME).click();
    h.run();
    h.event(egui::Event::Text("speed".into()));
    h.get_by_label(NEW_VALUE_NAME).click();
    h.run();
    h.event(egui::Event::Text("200".into()));
    h.run();
    h.get_by_label(ADD_KEY_LABEL).click();
    h.run();
    assert_eq!(entity(h.state(), id).setting("speed"), Some("200"));
    h.get_by_label(CLASS_FIELD_NAME).click();
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.event(egui::Event::Text("item_box".into()));
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(entity(h.state(), id).classname, "item_box");
    // A point entity can't become a brush entity.
    h.get_by_label(CLASS_FIELD_NAME).click();
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.event(egui::Event::Text("func_wall".into()));
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(entity(h.state(), id).classname, "item_box");
    assert!(h.state().console().entries().any(|e| {
        e.message
            .starts_with("The class can't be changed to func_wall")
    }));
    // The class and position can't be added as keys.
    h.get_by_label(NEW_KEY_NAME).click();
    h.run();
    h.event(egui::Event::Text("origin".into()));
    h.get_by_label(ADD_KEY_LABEL).click();
    h.run();
    assert!(
        h.state()
            .console()
            .entries()
            .any(|e| e.message.contains("can't be added as a key"))
    );
    assert!(
        h.query_by_label_contains("doesn't have this class")
            .is_some()
    );
}
