//! Floor plan shapes changed outside Halberd: whatever was changed there is
//! kept, and old marks never confuse later saves.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use glam::{Vec2, Vec3};
use halberd_doc::{Command, Document, FaceInfo, Object, ShapeObject, TextEncoding};
use halberd_geom::{Aabb, Brush};
use halberd_io::{open_map_text, vmf_from_document};
use halberd_vmf::{Entry, Vmf};

fn open(text: &str) -> Document {
    open_map_text(text, TextEncoding::Utf8).unwrap().document
}

fn save(doc: &Document) -> String {
    vmf_from_document(doc).to_text()
}

fn shape(outline: &[(f32, f32)]) -> ShapeObject {
    let outline: Vec<Vec2> = outline.iter().map(|&(x, y)| Vec2::new(x, y)).collect();
    ShapeObject::new(&outline, 0.0, 64.0, FaceInfo::default()).unwrap()
}

/// An L (two pieces) and, at 512, a 64-unit square (one piece).
fn l_and_square() -> Document {
    let mut doc = Document::new();
    let l = shape(&[
        (0.0, 0.0),
        (128.0, 0.0),
        (128.0, 64.0),
        (64.0, 64.0),
        (64.0, 128.0),
        (0.0, 128.0),
    ]);
    let square = shape(&[(512.0, 0.0), (576.0, 0.0), (576.0, 64.0), (512.0, 64.0)]);
    doc.execute(Command::AddShape(l)).unwrap();
    doc.execute(Command::AddShape(square)).unwrap();
    doc
}

fn count(doc: &Document) -> (usize, usize) {
    let shapes = doc
        .objects()
        .filter(|(_, o)| o.as_shape().is_some())
        .count();
    let brushes = doc
        .objects()
        .filter(|(_, o)| matches!(o, Object::Brush(_)))
        .count();
    (shapes, brushes)
}

/// Edits the world's solids of the saved text.
fn edit_solids(text: &str, f: impl FnOnce(&mut Vec<Entry>)) -> String {
    let mut vmf = Vmf::parse(text).unwrap();
    f(&mut vmf.world_mut().unwrap().entries);
    vmf.to_text()
}

fn solid_indices(entries: &[Entry]) -> Vec<usize> {
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, Entry::Block(b) if b.name == "solid"))
        .map(|(i, _)| i)
        .collect()
}

#[test]
fn a_deleted_piece_leaves_plain_brushes_that_never_capture_another_shape() {
    // Regression: the L's leftover piece kept its mark "1"; on the next
    // save the square became shape "1" too, and both turned to brushes.
    let text = save(&l_and_square());
    let edited = edit_solids(&text, |entries| {
        let first = solid_indices(entries)[0];
        entries.remove(first);
    });
    let doc = open(&edited);
    assert_eq!(
        count(&doc),
        (1, 1),
        "the square is a shape, the piece a brush"
    );
    let resaved = save(&doc);
    assert_eq!(
        resaved.matches("halberd_shape").count(),
        1,
        "only the square is marked"
    );
    let again = open(&resaved);
    assert_eq!(count(&again), (1, 1));
}

#[test]
fn a_reshaped_piece_with_the_same_box_stays_a_brush_and_keeps_its_shape() {
    let mut doc = Document::new();
    doc.execute(Command::AddShape(shape(&[
        (0.0, 0.0),
        (64.0, 0.0),
        (64.0, 64.0),
        (0.0, 64.0),
    ])))
    .unwrap();
    let text = save(&doc);
    // Lean the front face back 16 units at the top: same box, same sides.
    let slanted = "(64 0 0) (0 0 0) (64 16 64)";
    let edited = text.replacen("(64 0 0) (0 0 0) (64 0 64)", slanted, 1);
    assert_ne!(edited, text, "the test changed the face");
    let doc = open(&edited);
    assert_eq!(count(&doc), (0, 1));
    assert!(save(&doc).contains(slanted), "Hammer's edit is kept");
}

#[test]
fn a_retextured_face_is_kept() {
    let text = save(&l_and_square());
    let edited = edit_solids(&text, |entries| {
        let last = *solid_indices(entries).last().unwrap();
        let Entry::Block(solid) = &mut entries[last] else {
            unreachable!()
        };
        let side = solid
            .entries
            .iter_mut()
            .filter_map(|e| match e {
                Entry::Block(b) if b.name == "side" => Some(b),
                _ => None,
            })
            .last()
            .unwrap();
        for e in &mut side.entries {
            if let Entry::Pair(k, v) = e
                && k == "material"
            {
                *v = "BRICK/BRICKWALL001".into();
            }
        }
    });
    let doc = open(&edited);
    assert_eq!(
        count(&doc),
        (1, 1),
        "the L stays a shape, the square is a brush"
    );
    assert!(save(&doc).contains("BRICK/BRICKWALL001"));
}

#[test]
fn a_shape_copied_in_another_editor_opens_as_brushes() {
    let text = save(&l_and_square());
    let edited = edit_solids(&text, |entries| {
        let copies: Vec<Entry> = solid_indices(entries)
            .iter()
            .map(|&i| entries[i].clone())
            .collect();
        entries.extend(copies);
    });
    let opened = open_map_text(&edited, TextEncoding::Utf8).unwrap();
    assert_eq!(count(&opened.document), (0, 6));
    assert!(
        opened
            .notes
            .iter()
            .any(|n| n.contains("2 floor plan shapes"))
    );
    assert!(
        !save(&opened.document).contains("halberd_shape"),
        "marks removed"
    );
}

#[test]
fn ids_stay_the_same_from_save_to_save() {
    let text = save(&l_and_square());
    let mut doc = open(&text);
    // A brush added in between must not take the shapes' ids.
    let cube = Brush::cuboid(Aabb::from_corners(Vec3::splat(1000.0), Vec3::splat(1064.0))).unwrap();
    doc.execute(Command::AddBrushes(vec![cube])).unwrap();
    let resaved = save(&doc);
    let ids = |t: &str| -> Vec<String> {
        let vmf = Vmf::parse(t).unwrap();
        vmf.world()
            .unwrap()
            .blocks("solid")
            .filter(|s| s.blocks("editor").any(|e| e.get("halberd_shape").is_some()))
            .flat_map(|s| {
                std::iter::once(s.get("id").unwrap().to_string())
                    .chain(s.blocks("side").map(|d| d.get("id").unwrap().to_string()))
            })
            .collect()
    };
    let before = ids(&text);
    assert_eq!(
        before.len(),
        3 + 3 * 6,
        "three box-like pieces and their sides"
    );
    let mut after = ids(&resaved);
    let mut sorted_before = before.clone();
    sorted_before.sort();
    after.sort();
    assert_eq!(after, sorted_before);
    // And unique across the whole file.
    let vmf = Vmf::parse(&resaved).unwrap();
    let all: Vec<_> = vmf
        .world()
        .unwrap()
        .blocks("solid")
        .flat_map(|s| s.blocks("side").map(|d| d.get("id").unwrap().to_string()))
        .collect();
    let unique: std::collections::BTreeSet<_> = all.iter().collect();
    assert_eq!(unique.len(), all.len());
}

#[test]
fn fuzz_damaged_shape_maps_never_crash() {
    let text = save(&l_and_square());
    let mut seed: u64 = 0x5BA9_E000_0000_0007;
    let mut next = move |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    let pieces = [
        "{",
        "}",
        "\"",
        "|",
        "0",
        "-",
        "nan",
        "1e40",
        " ",
        "9",
        "halberd_shape",
        "2",
        "64 64",
    ];
    for _ in 0..1_000 {
        let mut damaged = text.clone();
        for _ in 0..1 + next(4) {
            let at = next(damaged.len());
            let end = (at + next(12)).min(damaged.len());
            damaged.replace_range(at..end, pieces[next(pieces.len())]);
        }
        if let Ok(opened) = open_map_text(&damaged, TextEncoding::Utf8) {
            let saved = save(&opened.document);
            let again = open_map_text(&saved, TextEncoding::Utf8).unwrap();
            assert_eq!(
                save(&again.document),
                saved,
                "a second save changes nothing"
            );
        }
    }
}
