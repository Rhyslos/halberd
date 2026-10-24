//! Tests for editing outputs and the connection view.

use super::tests::{harness_for, info};
use super::*;
use crate::ConnectionView;
use crate::panels::{ADD_OUTPUT_LABEL, connection_field_name};
use egui_kittest::kittest::Queryable;
use glam::Vec3;
use halberd_doc::{Connection, EntityObject, ObjectId};

const DEFS: &str = r#"
@BaseClass = Targetname [ targetname(target_source) : "Name" ]
@PointClass base(Targetname) = logic_relay : "Relay"
[
    input Trigger(void) : "Fire."
    output OnTrigger(void) : "Fired."
]
@PointClass base(Targetname) = light_dynamic : "A light"
[
    input TurnOn(void) : "On."
    input TurnOff(void) : "Off."
]
"#;

fn add(wb: &mut Workbench, class: &str, name: &str, at: f32) -> ObjectId {
    wb.document_mut()
        .execute(Command::AddEntity(EntityObject::point(
            class,
            Vec3::new(at, 0.0, 0.0),
            &[("targetname", name)],
        )))
        .unwrap();
    wb.document().objects().last().unwrap().0
}

fn connections(wb: &Workbench, id: ObjectId) -> Vec<Connection> {
    wb.document()
        .get(id)
        .unwrap()
        .as_entity()
        .unwrap()
        .connections()
}

/// A relay and a lamp, with the relay selected.
fn setup() -> (Workbench, ObjectId, ObjectId) {
    let mut wb = Workbench::new(info(), None);
    wb.set_entity_definitions(halberd_fgd::parse(DEFS, "t.fgd"), "t.fgd");
    let lamp = add(&mut wb, "light_dynamic", "lamp", 128.0);
    let relay = add(&mut wb, "logic_relay", "relay", 0.0);
    (wb, relay, lamp)
}

#[test]
fn an_output_is_added_pointed_at_a_target_and_removed() {
    let (wb, relay, _) = setup();
    let mut h = harness_for(wb);
    h.run();
    h.get_by_label(ADD_OUTPUT_LABEL).click();
    h.run();
    let added = connections(h.state(), relay);
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].output, "OnTrigger", "the class's first output");
    assert!(h.query_by_label("No target yet.").is_some());
    // Typing a target that doesn't exist warns.
    h.get_by_label(&connection_field_name(0, "target")).click();
    h.run();
    h.event(egui::Event::Text("lmap".into()));
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(connections(h.state(), relay)[0].target, "lmap");
    assert!(h.query_by_label("No entity is named \"lmap\".").is_some());
    h.get_by_label(&connection_field_name(0, "remove")).click();
    h.run();
    assert!(connections(h.state(), relay).is_empty());
    assert_eq!(
        h.state_mut().document_mut().undo().as_deref(),
        Some("Change outputs")
    );
}

#[test]
fn the_targets_inputs_are_offered_and_wrong_ones_warned() {
    let (mut wb, relay, _) = setup();
    wb.document_mut()
        .execute(Command::SetConnections {
            id: relay,
            connections: vec![Connection::new("OnTrigger", "lamp", "Explode")],
        })
        .unwrap();
    let mut h = harness_for(wb);
    h.run();
    assert!(
        h.query_by_label("A light_dynamic has no input \"Explode\".")
            .is_some()
    );
    h.get_by_label(&connection_field_name(0, "input")).click();
    h.run();
    h.get_by_label("TurnOn").click();
    h.run();
    assert_eq!(connections(h.state(), relay)[0].input, "TurnOn");
    assert!(h.query_by_label_contains("has no input").is_none());
    // Only once and the delay.
    h.get_by_label("Only once").click();
    h.run();
    assert!(connections(h.state(), relay)[0].only_once());
}

#[test]
fn what_reaches_an_entity_is_listed_and_selects_the_source() {
    let (mut wb, relay, lamp) = setup();
    wb.document_mut()
        .execute(Command::SetConnections {
            id: relay,
            connections: vec![Connection::new("OnTrigger", "lamp", "TurnOn")],
        })
        .unwrap();
    wb.document_mut().set_selection([lamp]);
    let mut h = harness_for(wb);
    h.run();
    assert!(h.query_by_label("Connected to this (1)").is_some());
    h.get_by_label("relay: OnTrigger -> TurnOn").click();
    h.run();
    assert!(h.state().document().selection().contains(&relay));
}

#[test]
fn the_views_draw_connections_of_the_selection_or_all() {
    let (mut wb, relay, lamp) = setup();
    wb.document_mut()
        .execute(Command::SetConnections {
            id: relay,
            connections: vec![Connection::new("OnTrigger", "lamp", "TurnOn")],
        })
        .unwrap();
    let mut h = harness_for(wb);
    h.run();
    assert_eq!(h.state().viewport().connection_lines(), 1, "relay selected");
    h.state_mut().document_mut().clear_selection();
    h.run();
    assert_eq!(h.state().viewport().connection_lines(), 0);
    h.state_mut()
        .viewport_mut()
        .set_connection_view(ConnectionView::All);
    h.run();
    assert_eq!(h.state().viewport().connection_lines(), 1);
    h.state_mut().document_mut().set_selection([lamp]);
    h.state_mut()
        .viewport_mut()
        .set_connection_view(ConnectionView::Off);
    h.run();
    assert_eq!(h.state().viewport().connection_lines(), 0);
}
