//! Tests for the File menu, the "save changes?" box, message boxes and
//! maps opened from files, run through the real interface.

use super::tests::{harness_for, info, type_into, workbench_with_boxes};
use super::*;
use crate::viewport::NoRenderer;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A harness that collects the actions the workbench asks for.
fn harness_with_actions(wb: Workbench) -> Harness<'static, (Workbench, Vec<WorkbenchAction>)> {
    Harness::builder()
        .with_size([1280.0, 800.0])
        .build_ui_state(
            |ui, (wb, actions): &mut (Workbench, Vec<WorkbenchAction>)| {
                let new = wb.show(ui, &mut NoRenderer::default());
                actions.extend(new);
            },
            (wb, Vec::new()),
        )
}

#[test]
fn file_shortcuts_ask_for_saving_and_opening() {
    let mut h = harness_with_actions(Workbench::new(info(), None));
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::S);
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::S);
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::O);
    h.run();
    assert_eq!(
        h.state().1,
        [
            WorkbenchAction::Save { then: None },
            WorkbenchAction::SaveAs { then: None },
            WorkbenchAction::Open,
        ],
        "nothing unsaved, so Open needs no question"
    );
}

#[test]
fn unsaved_changes_are_asked_about_before_a_new_map() {
    let mut h = harness_with_actions(workbench_with_boxes(1));
    h.run();
    assert!(h.state().0.window_title().starts_with("Untitled •"));
    h.key_press_modifiers(Modifiers::COMMAND, Key::N);
    h.run();
    assert!(h.state().0.is_asking_to_save());
    assert!(
        h.query_by_label_contains("Save changes to Untitled?")
            .is_some()
    );
    // Cancel: nothing happens.
    h.get_by_label(CANCEL_BUTTON).click();
    h.run();
    assert!(!h.state().0.is_asking_to_save());
    assert_eq!(h.state().0.document().len(), 1);
    // Don't save: a new, empty map.
    h.key_press_modifiers(Modifiers::COMMAND, Key::N);
    h.run();
    h.get_by_label(DONT_SAVE_BUTTON).click();
    h.run();
    assert!(h.state().0.document().is_empty());
    assert!(!h.state().0.is_modified());
    assert!(h.state().1.is_empty(), "New needs nothing from the program");
}

#[test]
fn choosing_save_saves_first_then_carries_on() {
    let mut h = harness_with_actions(workbench_with_boxes(1));
    h.run();
    h.get_by_label("File").click();
    h.run();
    h.get_by_label_contains("Quit").click();
    h.run();
    assert!(h.state().1.is_empty(), "asked first, nothing yet");
    h.get_by_label(SAVE_BUTTON).click();
    h.run();
    assert_eq!(
        h.state().1,
        [WorkbenchAction::Save {
            then: Some(FileIntent::Quit)
        }]
    );
}

#[test]
fn quitting_without_changes_or_without_saving() {
    let mut h = harness_with_actions(Workbench::new(info(), None));
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Q);
    h.run();
    assert_eq!(h.state().1, [WorkbenchAction::Quit]);

    let mut h = harness_with_actions(workbench_with_boxes(1));
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Q);
    h.run();
    h.get_by_label(DONT_SAVE_BUTTON).click();
    h.run();
    assert_eq!(h.state().1, [WorkbenchAction::Quit]);
}

#[test]
fn problems_show_in_a_message_box_until_ok() {
    let mut wb = Workbench::new(info(), None);
    wb.show_error("map.vmf could not be opened: the file is damaged");
    let mut h = harness_for(wb);
    h.run();
    assert!(h.query_by_label("Something went wrong").is_some());
    assert!(
        h.state()
            .console_lines()
            .iter()
            .any(|l| l.contains("damaged"))
    );
    h.get_by_label("OK").click();
    h.run();
    assert!(h.state().error().is_none());
}

/// A workbench showing a map with a light and a func_detail.
fn workbench_with_entities() -> Workbench {
    use halberd_doc::{BrushObject, EntityObject, Object};
    let light = Object::Entity(EntityObject {
        classname: "light".into(),
        origin: Some(glam::Vec3::new(0.0, 0.0, 64.0)),
        solids: Vec::new(),
        file_data: vec![
            halberd_vmf_pair("classname", "light"),
            halberd_vmf_pair("_light", "255 240 200 300"),
        ],
    });
    let cube = halberd_geom::Brush::cuboid(halberd_geom::Aabb::from_corners(
        glam::Vec3::ZERO,
        glam::Vec3::splat(64.0),
    ))
    .unwrap();
    let detail = Object::Entity(EntityObject {
        classname: "func_detail".into(),
        origin: None,
        solids: vec![BrushObject::new(cube)],
        file_data: Vec::new(),
    });
    let doc = halberd_doc::Document::from_map(vec![light, detail], Default::default()).unwrap();
    let mut wb = Workbench::new(info(), None);
    wb.set_document(doc, Some("demo.vmf".into()));
    wb
}

fn halberd_vmf_pair(k: &str, v: &str) -> halberd_kv::Entry {
    halberd_kv::Entry::Pair(k.into(), v.into())
}

#[test]
fn entities_are_listed_and_their_settings_shown() {
    let mut h = harness_for(workbench_with_entities());
    h.run();
    assert_eq!(
        h.state().window_title(),
        "demo.vmf — Halberd Map Editor 9.9.9"
    );
    let light = h.state().document().objects().next().unwrap().0;
    h.get_by_label(&format!("light {light}")).click();
    h.run();
    assert!(
        h.query_by_label("Editing entities comes in Phase 2.")
            .is_some()
    );
    assert!(h.query_by_label("255 240 200 300").is_some());
    assert!(
        h.query_by_label("Box width").is_none(),
        "no size fields for entities"
    );
}

#[test]
fn the_save_question_answers_to_keys() {
    let mut h = harness_with_actions(workbench_with_boxes(1));
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Q);
    h.run();
    h.key_press(Key::Escape);
    h.run();
    assert!(!h.state().0.is_asking_to_save(), "Escape cancels");
    assert!(h.state().1.is_empty());
    h.key_press_modifiers(Modifiers::COMMAND, Key::Q);
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(
        h.state().1,
        [WorkbenchAction::Save {
            then: Some(FileIntent::Quit)
        }]
    );
    h.state_mut().1.clear();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Q);
    h.run();
    h.key_press(Key::N);
    h.run();
    assert_eq!(h.state().1, [WorkbenchAction::Quit]);
}

#[test]
fn holding_ctrl_n_does_not_answer_the_save_question() {
    // Regression: the key-repeats of a held Ctrl+N counted as "N = don't
    // save" and threw the map away.
    let mut h = harness_with_actions(workbench_with_boxes(1));
    h.run();
    h.key_down_modifiers(Modifiers::COMMAND, Key::N);
    h.run();
    assert!(h.state().0.is_asking_to_save());
    let repeat = |h: &mut Harness<'static, _>, modifiers| {
        h.event(egui::Event::Key {
            key: Key::N,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers,
        });
        h.run();
    };
    for _ in 0..3 {
        repeat(&mut h, Modifiers::COMMAND);
    }
    // Ctrl let go first, N still held.
    repeat(&mut h, Modifiers::NONE);
    h.key_up(Key::N);
    h.run();
    // Fresh presses with Ctrl or Shift held don't answer either.
    h.key_press_modifiers(Modifiers::COMMAND, Key::N);
    h.run();
    h.key_press_modifiers(Modifiers::SHIFT, Key::N);
    h.run();
    assert!(h.state().0.is_asking_to_save(), "still asking");
    assert_eq!(h.state().0.document().len(), 1, "the map is kept");
    // A plain N does.
    h.key_press(Key::N);
    h.run();
    assert!(h.state().0.document().is_empty());
}

#[test]
fn the_map_behind_a_box_is_left_alone() {
    // Regression: Delete, Ctrl+Z and tool keys acted on the map while the
    // "save changes?" box or a message box was showing.
    let mut h = harness_with_actions(workbench_with_boxes(1));
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::N);
    h.run();
    h.key_press(Key::Delete);
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    h.key_press(Key::B);
    h.run();
    assert_eq!(h.state().0.document().len(), 1);
    assert_eq!(h.state().0.viewport().tool(), halberd_tools::Tool::Select);
    // Escape closes the box but keeps the selection.
    h.key_press(Key::Escape);
    h.run();
    assert!(!h.state().0.is_asking_to_save());
    assert_eq!(h.state().0.document().selection().len(), 1);

    h.state_mut().0.show_error("could not save");
    h.run();
    h.key_press(Key::Delete);
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(h.state().0.document().len(), 1);
    assert!(h.state().0.error().is_some());
}

#[test]
fn resizing_a_box_keeps_each_face_material_on_its_side() {
    // Regression: the size fields built a fresh cuboid, whose faces come in
    // a different order from a box opened from Hammer, so materials (and
    // face ids, displacements…) swapped sides.
    use halberd_doc::{BrushObject, FaceInfo, Object};
    use halberd_geom::Plane;
    let sides = [
        ("TOP", glam::Vec3::Z, 64.0),
        ("BOTTOM", glam::Vec3::NEG_Z, 0.0),
        ("WEST", glam::Vec3::NEG_X, 0.0),
        ("EAST", glam::Vec3::X, 64.0),
        ("NORTH", glam::Vec3::Y, 64.0),
        ("SOUTH", glam::Vec3::NEG_Y, 0.0),
    ];
    let planes: Vec<Plane> = sides
        .iter()
        .map(|&(_, normal, distance)| Plane { normal, distance })
        .collect();
    let infos: Vec<FaceInfo> = sides
        .iter()
        .map(|&(name, ..)| FaceInfo {
            material: name.into(),
            ..FaceInfo::default()
        })
        .collect();
    let brush = halberd_geom::Brush::from_planes(&planes).unwrap();
    let object = Object::Brush(BrushObject::from_parts(brush, &infos, Vec::new()));
    let doc = halberd_doc::Document::from_map(vec![object], Default::default()).unwrap();
    let id = doc.objects().next().unwrap().0;
    let mut wb = Workbench::new(info(), None);
    wb.set_document(doc, None);
    wb.document_mut().set_selection([id]);
    let mut harness = harness_for(wb);
    harness.run();
    type_into(&mut harness, "Box width", "100");
    type_into(&mut harness, "Box position Z", "32");
    let doc = harness.state().document();
    let brush = doc.objects().next().unwrap().1.as_brush().unwrap();
    assert_eq!(brush.brush().bounds().size().x, 100.0);
    for (face, info) in brush.brush().faces().iter().zip(brush.faces()) {
        let expected = sides.iter().find(|s| s.1 == face.plane().normal).unwrap().0;
        assert_eq!(info.material, expected);
    }
}
