//! Tests for maps read from files: saved state, entities and face data.

use super::tests::{cube_at, ids, only_id};
use super::*;

#[test]
fn a_map_from_a_file_counts_as_saved_until_edited() {
    let objects = vec![
        MapObject::Brush(crate::BrushObject::new(cube_at(0.0))),
        MapObject::Entity(
            crate::EntityObject {
                classname: "light".into(),
                origin: Some(Vec3::new(0.0, 0.0, 100.0)),
                file_data: Vec::new(),
            },
            Vec::new(),
        ),
    ];
    let data = crate::MapFileData {
        world_header: vec![halberd_kv::Entry::Pair(
            "skyname".into(),
            "sky_day01_01".into(),
        )],
        ..Default::default()
    };
    let mut doc = Document::from_map(objects, data.clone()).unwrap();
    assert_eq!(doc.len(), 2);
    assert_eq!(doc.file_data(), &data);
    assert!(!doc.is_modified());
    assert_eq!(doc.undo_label(), None);
    assert!(doc.selection().is_empty());
    let id = ids(&doc)[0];
    doc.set_selection([id]);
    assert!(!doc.is_modified(), "selecting is not an edit");
    doc.execute(Command::Remove(vec![id])).unwrap();
    assert!(doc.is_modified());
    doc.mark_saved();
    assert!(!doc.is_modified());
}

#[test]
fn entities_cannot_be_reshaped_like_brushes() {
    let entity = MapObject::Entity(
        crate::EntityObject {
            classname: "info_player_start".into(),
            origin: Some(Vec3::ZERO),
            file_data: Vec::new(),
        },
        Vec::new(),
    );
    let mut doc = Document::from_map(vec![entity], Default::default()).unwrap();
    let id = ids(&doc)[0];
    assert_eq!(
        doc.execute(Command::ReplaceBrush {
            id,
            brush: cube_at(0.0)
        }),
        Err(DocError::NotABrush(id))
    );
    assert!(DocError::NotABrush(id).to_string().contains("not a brush"));
}

#[test]
fn reshaping_keeps_materials() {
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![cube_at(0.0)]))
        .unwrap();
    let id = only_id(&doc);
    doc.execute(Command::ReplaceBrush {
        id,
        brush: cube_at(64.0),
    })
    .unwrap();
    let brush = doc.get(id).unwrap().as_brush().unwrap();
    assert!(
        brush
            .faces()
            .iter()
            .all(|f| f.material == crate::DEFAULT_MATERIAL)
    );
    assert_eq!(brush.faces().len(), 6);
}

/// A map with a world brush, then a func_detail (with an origin, to check
/// it is ignored) owning two brushes.
fn map_with_detail() -> (Document, [ObjectId; 4]) {
    let detail = MapObject::Entity(
        crate::EntityObject {
            classname: "func_detail".into(),
            origin: Some(Vec3::new(500.0, 500.0, 500.0)),
            file_data: Vec::new(),
        },
        vec![
            crate::BrushObject::new(cube_at(100.0)),
            crate::BrushObject::new(cube_at(200.0)),
        ],
    );
    let world = MapObject::Brush(crate::BrushObject::new(cube_at(0.0)));
    let doc = Document::from_map(vec![world, detail], Default::default()).unwrap();
    let all = ids(&doc);
    (doc, [all[0], all[1], all[2], all[3]])
}

#[test]
fn a_brush_entitys_brushes_are_objects_that_name_it() {
    let (doc, [world, detail, a, b]) = map_with_detail();
    assert_eq!(doc.len(), 4);
    assert_eq!(doc.brushes_of(detail).collect::<Vec<_>>(), [a, b]);
    assert!(doc.has_brushes(detail));
    assert!(!doc.has_brushes(world));
    let brush = doc.get(a).unwrap().as_brush().unwrap();
    assert_eq!(brush.entity(), Some(detail));
    // Clicking a brush picks the entity, unless picking inside entities.
    assert_eq!(doc.selectable(a, false), detail);
    assert_eq!(doc.selectable(a, true), a);
    assert_eq!(doc.selectable(world, false), world);
    // The entity's box is its brushes', not its origin's.
    let bounds = doc.bounds_of(detail).unwrap();
    assert_eq!(bounds.min.x, 100.0);
    assert_eq!(bounds.max.x, 264.0);
}

#[test]
fn a_brush_entity_is_picked_by_its_brushes_not_its_origin() {
    let (doc, [_, _, a, _]) = map_with_detail();
    let down = Vec3::NEG_Z;
    assert_eq!(doc.pick(Vec3::new(132.0, 32.0, 1000.0), down).unwrap().0, a);
    assert_eq!(doc.pick(Vec3::new(500.0, 500.0, 1000.0), down), None);
}

#[test]
fn a_selected_brush_entity_shows_its_brushes_selected() {
    let (mut doc, [world, detail, a, b]) = map_with_detail();
    doc.set_selection([detail]);
    assert!(doc.is_shown_selected(a) && doc.is_shown_selected(b));
    assert!(!doc.is_shown_selected(world));
    assert_eq!(doc.selection_bounds(), doc.bounds_of(detail));
}

#[test]
fn deleting_a_brush_entity_takes_its_brushes_and_undo_brings_all_back() {
    let (mut doc, [world, detail, a, b]) = map_with_detail();
    doc.execute(Command::Remove(vec![detail])).unwrap();
    assert_eq!(ids(&doc), [world]);
    assert!(!doc.has_brushes(detail));
    doc.undo();
    assert_eq!(ids(&doc), [world, detail, a, b]);
    assert_eq!(doc.brushes_of(detail).collect::<Vec<_>>(), [a, b]);
    assert_eq!(
        doc.selection().iter().copied().collect::<Vec<_>>(),
        [detail],
        "the entity comes back selected, its brushes through it"
    );
}

#[test]
fn deleting_one_brush_of_an_entity_keeps_the_rest() {
    let (mut doc, [world, detail, a, b]) = map_with_detail();
    doc.execute(Command::Remove(vec![a])).unwrap();
    assert_eq!(ids(&doc), [world, detail, b]);
    assert_eq!(doc.brushes_of(detail).collect::<Vec<_>>(), [b]);
    doc.undo();
    assert_eq!(doc.brushes_of(detail).collect::<Vec<_>>(), [a, b]);
    assert_eq!(doc.selection().iter().copied().collect::<Vec<_>>(), [a]);
}

#[test]
fn deleting_an_entitys_last_brushes_deletes_the_entity_too() {
    let (mut doc, [world, detail, a, b]) = map_with_detail();
    doc.execute(Command::Remove(vec![a, b])).unwrap();
    assert_eq!(ids(&doc), [world], "no brush entity without brushes");
    doc.undo();
    assert_eq!(ids(&doc), [world, detail, a, b]);
}

#[test]
fn an_entity_brush_moves_like_any_brush_and_stays_in_its_entity() {
    let (mut doc, [_, detail, a, _]) = map_with_detail();
    doc.execute(Command::TransformBrushes {
        kind: crate::TransformKind::Move,
        brushes: vec![(a, cube_at(132.0))],
    })
    .unwrap();
    let brush = doc.get(a).unwrap().as_brush().unwrap();
    assert_eq!(brush.entity(), Some(detail));
    assert_eq!(brush.brush().bounds().min.x, 132.0);
    assert_eq!(doc.brushes_of(detail).count(), 2);
}

#[test]
fn deleting_an_entity_names_everything_that_goes() {
    let (mut doc, [_, detail, ..]) = map_with_detail();
    doc.execute(Command::Remove(vec![detail])).unwrap();
    assert_eq!(doc.undo_label(), Some("Delete 3 objects"));
}

#[test]
fn ctrl_clicking_a_brush_of_a_selected_entity_takes_just_it_out() {
    let (mut doc, [_, detail, a, b]) = map_with_detail();
    doc.set_selection([detail]);
    doc.toggle_selected(a);
    assert_eq!(doc.selection().iter().copied().collect::<Vec<_>>(), [b]);
    assert!(!doc.is_shown_selected(a));
}

#[test]
fn an_entity_keeping_unshown_brushes_stays_when_its_shown_ones_go() {
    // Regression: deleting the brushes Halberd shows deleted the entity,
    // and with it the brushes it could not show, which were to be saved.
    let kept = halberd_kv::Entry::Block(halberd_kv::Block::new("solid"));
    let detail = MapObject::Entity(
        crate::EntityObject {
            classname: "func_detail".into(),
            origin: None,
            file_data: vec![kept],
        },
        vec![crate::BrushObject::new(cube_at(0.0))],
    );
    let mut doc = Document::from_map(vec![detail], Default::default()).unwrap();
    let [entity, brush] = [ids(&doc)[0], ids(&doc)[1]];
    doc.execute(Command::Remove(vec![brush])).unwrap();
    assert_eq!(ids(&doc), [entity]);
}
