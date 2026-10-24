//! Tests for the Library's Entities tab and editing basic entities in
//! Properties.

use super::tests::{harness_for, info};
use super::*;
use crate::panels::{BRIGHTNESS_NAME, COLOUR_NAME, CONE_NAME, FACING_NAME, PITCH_NAME};
use egui_kittest::kittest::Queryable;
use glam::Vec3;
use halberd_doc::EntityObject;
use halberd_tools::Tool;

fn add(wb: &mut Workbench, class: &str, settings: &[(&str, &str)]) -> halberd_doc::ObjectId {
    wb.document_mut()
        .execute(Command::AddEntity(EntityObject::point(
            class,
            Vec3::ZERO,
            settings,
        )))
        .unwrap();
    wb.document().objects().last().unwrap().0
}

#[test]
fn picking_a_basic_entity_starts_placing_it_even_without_the_game() {
    let mut h = harness_for(Workbench::new(info(), None));
    h.run();
    h.get_by_label(ENTITIES_TAB).click();
    h.run();
    h.get_by_label("Player spawn  (info_player_start)").click();
    h.run();
    let tools = h.state().viewport().tools();
    assert_eq!(tools.tool(), Tool::Place);
    let placing = tools.placing().unwrap();
    assert_eq!(placing.classname, "info_player_start");
    assert_eq!(placing.local.max.z, 72.0, "as tall as a player");
    h.get_by_label("Spotlight  (light_spot)").click();
    h.run();
    let placing = h.state().viewport().tools().placing().unwrap().clone();
    assert!(placing.settings.contains(&("pitch".into(), "-90".into())));
}

#[test]
fn a_map_has_one_sun() {
    let mut wb = Workbench::new(info(), None);
    let sun = add(&mut wb, "light_environment", &[]);
    wb.document_mut().clear_selection();
    let mut h = harness_for(wb);
    h.run();
    h.get_by_label(ENTITIES_TAB).click();
    h.run();
    h.get_by_label("Sun  (light_environment)").click();
    h.run();
    assert_eq!(h.state().viewport().tools().tool(), Tool::Select);
    assert!(h.state().document().selection().contains(&sun));
    assert!(h.query_by_label_contains("already has a sun").is_some());
}

#[test]
fn choosing_a_sky_changes_the_map_and_can_be_undone() {
    let mut wb = Workbench::new(info(), None);
    wb.set_sky_names(vec!["sky_borealis01".into(), DEFAULT_SKY.into()]);
    let mut h = harness_for(wb);
    h.run();
    h.get_by_label(ENTITIES_TAB).click();
    h.run();
    h.get_by_label(SKY_PICKER_NAME).click();
    h.run();
    h.get_by_label("sky_borealis01").click();
    h.run();
    assert_eq!(
        h.state().document().world_setting("skyname"),
        Some("sky_borealis01")
    );
    h.state_mut().undo();
    assert_eq!(h.state().document().world_setting("skyname"), None);
}

#[test]
fn a_selected_light_shows_its_editable_settings() {
    let mut wb = Workbench::new(info(), None);
    add(
        &mut wb,
        "light_spot",
        &[("_light", "255 128 0 400"), ("_cone", "30")],
    );
    let mut h = harness_for(wb);
    h.run();
    for field in [
        BRIGHTNESS_NAME,
        COLOUR_NAME,
        CONE_NAME,
        PITCH_NAME,
        FACING_NAME,
    ] {
        assert!(h.query_by_label(field).is_some(), "missing {field}");
    }
    // Typing a brightness changes the light, as one undo step.
    h.get_by_label(BRIGHTNESS_NAME).click();
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.get_by_label(BRIGHTNESS_NAME).type_text("750");
    h.key_press(egui::Key::Enter);
    h.run();
    h.run();
    let light = h
        .state()
        .document()
        .objects()
        .next()
        .unwrap()
        .1
        .as_entity()
        .unwrap()
        .clone();
    assert_eq!(light.setting("_light"), Some("255 128 0 750"));
    assert_eq!(
        h.state_mut().document_mut().undo().as_deref(),
        Some("Change entity settings")
    );
}

#[test]
fn other_entities_get_the_plain_keyvalue_editor() {
    let mut wb = Workbench::new(info(), None);
    add(&mut wb, "prop_static", &[("model", "models/a.mdl")]);
    let mut h = harness_for(wb);
    h.run();
    assert!(h.query_by_label(BRIGHTNESS_NAME).is_none());
    assert!(h.query_by_label(FACING_NAME).is_none());
    assert!(
        h.query_by_label(&crate::panels::value_field_name("model"))
            .is_some()
    );
}

#[test]
fn a_plain_light_has_no_facing() {
    let mut wb = Workbench::new(info(), None);
    add(&mut wb, "light", &[]);
    let mut h = harness_for(wb);
    h.run();
    assert!(h.query_by_label(BRIGHTNESS_NAME).is_some());
    assert!(h.query_by_label(FACING_NAME).is_none());
}

#[test]
fn selecting_an_entity_never_changes_it() {
    // Regression: values outside the fields' ranges were clamped and saved
    // just by drawing Properties.
    for (class, settings) in [
        (
            "light_spot",
            vec![("_cone", "180"), ("_inner_cone", "180"), ("pitch", "-90")],
        ),
        ("info_player_start", vec![("angles", "0 359.6 0")]),
        ("light", vec![("_light", "255 255 255 200000")]),
        ("info_player_start", vec![("angles", "12.5 -30 0")]),
    ] {
        let mut wb = Workbench::new(info(), None);
        add(&mut wb, class, &settings);
        wb.document_mut().mark_saved();
        let mut h = harness_for(wb);
        h.run();
        h.run();
        let now: Vec<(String, String)> = h
            .state()
            .document()
            .objects()
            .next()
            .unwrap()
            .1
            .as_entity()
            .unwrap()
            .settings()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        assert!(
            !h.state().document().is_modified(),
            "{class} {settings:?} changed: {now:?} {:?}",
            h.state().document().undo_label()
        );
    }
}

#[test]
fn a_smaller_cone_takes_the_inner_cone_with_it() {
    let mut wb = Workbench::new(info(), None);
    add(
        &mut wb,
        "light_spot",
        &[("_cone", "45"), ("_inner_cone", "30"), ("pitch", "-90")],
    );
    let mut h = harness_for(wb);
    h.run();
    super::tests::type_into(&mut h, CONE_NAME, "20");
    let spot = h
        .state()
        .document()
        .objects()
        .next()
        .unwrap()
        .1
        .as_entity()
        .unwrap()
        .clone();
    assert_eq!(spot.setting("_cone"), Some("20"));
    assert_eq!(spot.setting("_inner_cone"), Some("20"));
    super::tests::type_into(&mut h, PITCH_NAME, "-60");
    let spot = h
        .state()
        .document()
        .objects()
        .next()
        .unwrap()
        .1
        .as_entity()
        .unwrap()
        .clone();
    assert_eq!(spot.setting("pitch"), Some("-60"));
    assert_eq!(spot.setting("angles"), Some("-60 0 0"));
}
