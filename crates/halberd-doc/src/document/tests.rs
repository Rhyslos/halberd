//! Tests for the document, its commands and undo.

use super::*;
use crate::MAX_UNDO_STEPS;
use halberd_geom::Brush;

pub(super) fn cube_at(x: f32) -> Brush {
    Brush::cuboid(Aabb::from_corners(
        Vec3::new(x, 0.0, 0.0),
        Vec3::new(x + 64.0, 64.0, 64.0),
    ))
    .unwrap()
}

pub(super) fn ids(doc: &Document) -> Vec<ObjectId> {
    doc.objects().map(|(id, _)| id).collect()
}

#[test]
fn adding_brushes_gives_new_ids_and_selects_them() {
    let mut doc = Document::new();
    assert!(doc.is_empty());
    doc.execute(Command::AddBrushes(vec![cube_at(0.0)]))
        .unwrap();
    doc.execute(Command::AddBrushes(vec![cube_at(100.0), cube_at(200.0)]))
        .unwrap();
    assert_eq!(doc.len(), 3);
    let all = ids(&doc);
    assert!(all.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(doc.selection().len(), 2, "the latest brushes are selected");
    assert!(!doc.is_selected(all[0]));
}

#[test]
fn undo_and_redo_bring_back_the_same_objects() {
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![cube_at(0.0)]))
        .unwrap();
    let id = ids(&doc)[0];
    let original = doc.get(id).cloned();

    doc.execute(Command::Remove(vec![id])).unwrap();
    assert!(doc.is_empty());
    assert_eq!(doc.undo_label(), Some("Delete 1 object"));

    assert_eq!(doc.undo().as_deref(), Some("Delete 1 object"));
    assert_eq!(doc.get(id).cloned(), original, "same id, same shape");
    assert!(doc.is_selected(id), "what undo brings back is selected");

    assert_eq!(doc.undo().as_deref(), Some("Create brush"));
    assert!(doc.is_empty());
    assert_eq!(doc.undo(), None, "nothing left to undo");

    assert_eq!(doc.redo().as_deref(), Some("Create brush"));
    assert_eq!(ids(&doc), [id]);
    assert_eq!(doc.redo().as_deref(), Some("Delete 1 object"));
    assert!(doc.is_empty());
    assert_eq!(doc.redo(), None);
}

#[test]
fn a_new_edit_clears_redo() {
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![cube_at(0.0)]))
        .unwrap();
    doc.undo();
    assert_eq!(doc.redo_label(), Some("Create brush"));
    doc.execute(Command::AddBrushes(vec![cube_at(100.0)]))
        .unwrap();
    assert_eq!(doc.redo_label(), None);
}

#[test]
fn ids_are_never_reused() {
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![cube_at(0.0)]))
        .unwrap();
    let first = ids(&doc)[0];
    doc.undo();
    doc.execute(Command::AddBrushes(vec![cube_at(0.0)]))
        .unwrap();
    assert_ne!(ids(&doc)[0], first);
}

#[test]
fn failed_edits_change_nothing() {
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![cube_at(0.0)]))
        .unwrap();
    let real = ids(&doc)[0];
    let (revision, label) = (doc.revision(), doc.undo_label().map(String::from));

    assert_eq!(
        doc.execute(Command::AddBrushes(vec![])),
        Err(DocError::NothingToDo)
    );
    assert_eq!(
        doc.execute(Command::Remove(vec![])),
        Err(DocError::NothingToDo)
    );
    let missing = ObjectId(999);
    assert_eq!(
        doc.execute(Command::Remove(vec![real, missing])),
        Err(DocError::UnknownObject(missing))
    );
    assert_eq!(doc.len(), 1, "the real object was not removed either");
    assert_eq!(doc.revision(), revision);
    assert_eq!(doc.undo_label().map(String::from), label);
    assert_eq!(missing.to_string(), "#999");
    assert!(
        DocError::UnknownObject(missing)
            .to_string()
            .contains("#999")
    );
}

#[test]
fn removing_the_same_id_twice_in_one_command_is_fine() {
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![cube_at(0.0)]))
        .unwrap();
    let id = ids(&doc)[0];
    let twice = Command::Remove(vec![id, id]);
    assert_eq!(twice.describe(), "Delete 1 object");
    doc.execute(twice).unwrap();
    assert!(doc.is_empty());
    doc.undo();
    assert_eq!(doc.len(), 1);
}

#[test]
fn selection_changes_are_not_edits() {
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![cube_at(0.0), cube_at(100.0)]))
        .unwrap();
    let [a, b] = [ids(&doc)[0], ids(&doc)[1]];
    let revision = doc.revision();
    let before = doc.selection_revision();

    doc.set_selection([a]);
    assert!(doc.is_selected(a) && !doc.is_selected(b));
    doc.toggle_selected(b);
    assert_eq!(doc.selection().len(), 2);
    doc.toggle_selected(a);
    assert_eq!(doc.selection().iter().copied().collect::<Vec<_>>(), [b]);
    doc.clear_selection();
    assert!(doc.selection().is_empty());
    doc.set_selection([ObjectId(12345)]);
    assert!(doc.selection().is_empty(), "unknown ids are ignored");
    doc.toggle_selected(ObjectId(12345));
    assert!(doc.selection().is_empty());

    assert!(doc.selection_revision() > before);
    assert_eq!(doc.revision(), revision, "the map itself did not change");
    assert_eq!(doc.undo_label(), Some("Create 2 brushes"));
}

#[test]
fn selection_bounds_cover_every_selected_object() {
    let mut doc = Document::new();
    assert_eq!(doc.selection_bounds(), None);
    doc.execute(Command::AddBrushes(vec![cube_at(0.0), cube_at(200.0)]))
        .unwrap();
    let bounds = doc.selection_bounds().unwrap();
    assert_eq!(bounds.min, Vec3::ZERO);
    assert_eq!(bounds.max, Vec3::new(264.0, 64.0, 64.0));
}

#[test]
fn picking_finds_the_nearest_object() {
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![cube_at(0.0), cube_at(100.0)]))
        .unwrap();
    let [near, far] = [ids(&doc)[0], ids(&doc)[1]];
    // From inside the first box, the ray passes through it to the second.
    let inside = doc.pick(Vec3::new(32.0, 32.0, 32.0), Vec3::X);
    assert_eq!(inside.map(|p| p.0), Some(far));
    let from_left = doc.pick(Vec3::new(-100.0, 32.0, 32.0), Vec3::X);
    assert_eq!(from_left, Some((near, 100.0)));
    let from_right = doc.pick(Vec3::new(400.0, 32.0, 32.0), Vec3::NEG_X);
    assert_eq!(from_right.map(|p| p.0), Some(far));
    assert_eq!(doc.pick(Vec3::new(0.0, 0.0, 500.0), Vec3::Z), None);
}

#[test]
fn history_forgets_the_oldest_steps_beyond_the_limit() {
    let mut doc = Document::new();
    for i in 0..(MAX_UNDO_STEPS + 5) {
        doc.execute(Command::AddBrushes(vec![cube_at(i as f32)]))
            .unwrap();
    }
    let mut undone = 0;
    while doc.undo().is_some() {
        undone += 1;
    }
    assert_eq!(undone, MAX_UNDO_STEPS);
    assert_eq!(doc.len(), 5, "the forgotten steps stay done");
}

#[test]
fn random_edits_undo_back_to_an_empty_map() {
    // Stress test: random adds, deletes, undos and redos, then undo
    // everything. The map must end exactly where it started, and the
    // revision must change with every edit.
    let mut seed: u64 = 0xA11C_E5ED_0000_0042;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut doc = Document::new();
    for _ in 0..3_000 {
        let revision = doc.revision();
        match next() % 4 {
            0 | 1 => {
                let x = (next() % 1000) as f32;
                doc.execute(Command::AddBrushes(vec![cube_at(x)])).unwrap();
                assert!(doc.revision() > revision);
            }
            2 => {
                let all = ids(&doc);
                if !all.is_empty() {
                    let pick = all[(next() as usize) % all.len()];
                    doc.execute(Command::Remove(vec![pick])).unwrap();
                }
            }
            _ => {
                if next() % 2 == 0 {
                    doc.undo();
                } else {
                    doc.redo();
                }
            }
        }
        assert!(doc.selection().iter().all(|id| doc.get(*id).is_some()));
    }
    // The history keeps at most MAX_UNDO_STEPS, so start counting from a
    // map rebuilt by undoing everything that is still remembered.
    while doc.undo().is_some() {}
    let base = ids(&doc);
    while doc.redo().is_some() {}
    while doc.undo().is_some() {}
    assert_eq!(ids(&doc), base, "undo all, redo all, undo all is stable");
}

#[test]
fn every_document_and_copy_has_its_own_instance() {
    let a = Document::new();
    let b = Document::new();
    let c = a.clone();
    assert_ne!(a.instance(), b.instance());
    assert_ne!(a.instance(), c.instance());
    assert_eq!(a.revision(), c.revision());
}

pub(super) fn only_id(doc: &Document) -> ObjectId {
    ids(doc)[0]
}

fn bounds_of(doc: &Document, id: ObjectId) -> Aabb {
    doc.bounds_of(id).unwrap()
}

#[test]
fn replacing_a_brush_can_be_undone_and_keeps_the_selection() {
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![cube_at(0.0)]))
        .unwrap();
    let id = only_id(&doc);
    let selection_before = doc.selection().clone();
    doc.execute(Command::ReplaceBrush {
        id,
        brush: cube_at(500.0),
    })
    .unwrap();
    assert_eq!(bounds_of(&doc, id).min.x, 500.0);
    assert_eq!(doc.selection(), &selection_before);
    assert_eq!(doc.undo().as_deref(), Some("Change brush"));
    assert_eq!(bounds_of(&doc, id).min.x, 0.0);
    doc.redo();
    assert_eq!(bounds_of(&doc, id).min.x, 500.0);
}

#[test]
fn replacing_with_the_same_shape_or_a_missing_brush_is_refused() {
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![cube_at(0.0)]))
        .unwrap();
    let id = only_id(&doc);
    assert_eq!(
        doc.execute(Command::ReplaceBrush {
            id,
            brush: cube_at(0.0)
        }),
        Err(DocError::NothingToDo)
    );
    let missing = ObjectId(77);
    assert_eq!(
        doc.execute(Command::ReplaceBrush {
            id: missing,
            brush: cube_at(0.0)
        }),
        Err(DocError::UnknownObject(missing))
    );
}

#[test]
fn a_drag_of_many_small_edits_is_one_undo_step() {
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![cube_at(0.0)]))
        .unwrap();
    let id = only_id(&doc);
    doc.end_step();
    for x in 1..=20 {
        doc.execute_merging(
            Command::ReplaceBrush {
                id,
                brush: cube_at(x as f32 * 10.0),
            },
            7,
        )
        .unwrap();
    }
    doc.end_step();
    // A second drag is its own step.
    doc.execute_merging(
        Command::ReplaceBrush {
            id,
            brush: cube_at(1000.0),
        },
        7,
    )
    .unwrap();
    doc.undo();
    assert_eq!(bounds_of(&doc, id).min.x, 200.0, "second drag undone");
    doc.undo();
    assert_eq!(
        bounds_of(&doc, id).min.x,
        0.0,
        "whole first drag undone at once"
    );
    assert_eq!(doc.undo_label(), Some("Create brush"));
    doc.redo();
    assert_eq!(bounds_of(&doc, id).min.x, 200.0);
}

#[test]
fn different_merge_keys_are_separate_steps() {
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![cube_at(0.0)]))
        .unwrap();
    let id = only_id(&doc);
    let replace = |x: f32| Command::ReplaceBrush {
        id,
        brush: cube_at(x),
    };
    doc.execute_merging(replace(10.0), 1).unwrap();
    doc.execute_merging(replace(20.0), 2).unwrap();
    doc.undo();
    assert_eq!(bounds_of(&doc, id).min.x, 10.0);
}

#[test]
fn transforming_several_brushes_is_one_named_step() {
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![cube_at(0.0), cube_at(100.0)]))
        .unwrap();
    let [a, b] = [ids(&doc)[0], ids(&doc)[1]];
    let moved = |id: ObjectId, dx: f32| {
        let brush = doc.get(id).unwrap().as_brush().unwrap().brush().clone();
        (id, brush.translated(Vec3::new(dx, 0.0, 0.0)).unwrap())
    };
    let command = Command::TransformBrushes {
        kind: crate::TransformKind::Move,
        brushes: vec![moved(a, 16.0), moved(b, 16.0)],
    };
    assert_eq!(command.describe(), "Move 2 brushes");
    doc.execute(command).unwrap();
    assert_eq!(bounds_of(&doc, a).min.x, 16.0);
    assert_eq!(bounds_of(&doc, b).min.x, 116.0);
    assert_eq!(doc.undo().as_deref(), Some("Move 2 brushes"));
    assert_eq!(bounds_of(&doc, a).min.x, 0.0);
    assert_eq!(bounds_of(&doc, b).min.x, 100.0);
}

#[test]
fn a_transform_that_changes_nothing_is_refused() {
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![cube_at(0.0)]))
        .unwrap();
    let id = only_id(&doc);
    let same = doc.get(id).unwrap().as_brush().unwrap().brush().clone();
    let command = Command::TransformBrushes {
        kind: crate::TransformKind::Rotate,
        brushes: vec![(id, same)],
    };
    assert_eq!(command.describe(), "Rotate brush");
    assert_eq!(doc.execute(command), Err(DocError::NothingToDo));
}

#[test]
fn a_cancelled_drag_is_reversed_and_forgotten() {
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![cube_at(0.0)]))
        .unwrap();
    let id = only_id(&doc);
    doc.end_step();
    for x in [8.0, 16.0, 24.0] {
        doc.execute_merging(
            Command::TransformBrushes {
                kind: crate::TransformKind::Move,
                brushes: vec![(id, cube_at(x))],
            },
            42,
        )
        .unwrap();
    }
    assert!(doc.discard_step(42));
    assert_eq!(bounds_of(&doc, id).min.x, 0.0, "back where it started");
    assert_eq!(
        doc.undo_label(),
        Some("Create brush"),
        "the drag left no step"
    );
    assert_eq!(doc.redo_label(), None, "and cannot be redone");
    assert!(!doc.discard_step(42), "nothing left to discard");
}

#[test]
fn a_finished_step_is_never_discarded() {
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![cube_at(0.0)]))
        .unwrap();
    let id = only_id(&doc);
    doc.end_step();
    doc.execute_merging(
        Command::TransformBrushes {
            kind: crate::TransformKind::Move,
            brushes: vec![(id, cube_at(8.0))],
        },
        5,
    )
    .unwrap();
    doc.end_step();
    assert!(!doc.discard_step(5));
    assert!(!doc.discard_step(6), "other keys are left alone too");
    assert_eq!(bounds_of(&doc, id).min.x, 8.0);
}

#[test]
fn a_drag_that_comes_back_to_the_start_leaves_no_undo_step() {
    // Regression: dragging out and back left a "Move brush" step that did
    // nothing when undone.
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![cube_at(0.0)]))
        .unwrap();
    let id = only_id(&doc);
    doc.end_step();
    for x in [16.0, 32.0, 0.0] {
        doc.execute_merging(
            Command::TransformBrushes {
                kind: crate::TransformKind::Move,
                brushes: vec![(id, cube_at(x))],
            },
            9,
        )
        .ok();
    }
    assert_eq!(doc.undo_label(), Some("Create brush"));
    // The same drag can carry on afterwards, as a fresh step.
    doc.execute_merging(
        Command::TransformBrushes {
            kind: crate::TransformKind::Move,
            brushes: vec![(id, cube_at(48.0))],
        },
        9,
    )
    .unwrap();
    doc.undo();
    assert_eq!(bounds_of(&doc, id).min.x, 0.0);
}
