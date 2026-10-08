//! Tests for maps read from files: saved state, entities and face data.

use super::tests::{cube_at, ids, only_id};
use super::*;

#[test]
fn a_map_from_a_file_counts_as_saved_until_edited() {
    let objects = vec![
        Object::Brush(crate::BrushObject::new(cube_at(0.0))),
        Object::Entity(crate::EntityObject {
            classname: "light".into(),
            origin: Some(Vec3::new(0.0, 0.0, 100.0)),
            solids: Vec::new(),
            file_data: Vec::new(),
        }),
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
    let entity = Object::Entity(crate::EntityObject {
        classname: "info_player_start".into(),
        origin: Some(Vec3::ZERO),
        solids: Vec::new(),
        file_data: Vec::new(),
    });
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
