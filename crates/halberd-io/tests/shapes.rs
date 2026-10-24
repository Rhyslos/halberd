//! Floor plan shapes: saved as convex world brushes, read back as shapes.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use glam::Vec2;
use halberd_doc::{Command, Document, FaceInfo, Object, ShapeObject, TextEncoding};
use halberd_io::{open_map_text, vmf_from_document};
use halberd_vmf::Vmf;

fn open(text: &str) -> Document {
    open_map_text(text, TextEncoding::Utf8).unwrap().document
}

/// An L, 256 across with a 128 bite, standing at `x`.
fn l_shape(x: f32) -> ShapeObject {
    let outline: Vec<Vec2> = [
        (0.0, 0.0),
        (256.0, 0.0),
        (256.0, 128.0),
        (128.0, 128.0),
        (128.0, 256.0),
        (0.0, 256.0),
    ]
    .iter()
    .map(|(a, b)| Vec2::new(a + x, *b))
    .collect();
    let look = FaceInfo {
        material: "DEV/DEV_MEASUREGENERIC01B".into(),
        ..FaceInfo::default()
    };
    ShapeObject::new(&outline, 0.0, 128.0, look).unwrap()
}

fn shapes(doc: &Document) -> Vec<&ShapeObject> {
    doc.objects().filter_map(|(_, o)| o.as_shape()).collect()
}

fn doc_with(list: Vec<ShapeObject>) -> Document {
    let mut doc = Document::new();
    for shape in list {
        doc.execute(Command::AddShape(shape)).unwrap();
    }
    doc
}

#[test]
fn a_shape_saves_as_convex_world_brushes_with_unique_ids() {
    let doc = doc_with(vec![l_shape(0.0), l_shape(512.0)]);
    let text = vmf_from_document(&doc).to_text();
    let vmf = Vmf::parse(&text).unwrap();
    let solids: Vec<_> = vmf.world().unwrap().blocks("solid").collect();
    assert_eq!(solids.len(), 4, "two pieces for each L");
    let mut solid_ids = Vec::new();
    let mut side_ids = Vec::new();
    for solid in &solids {
        solid_ids.push(solid.get("id").unwrap().to_string());
        side_ids.extend(
            solid
                .blocks("side")
                .map(|s| s.get("id").unwrap().to_string()),
        );
        let side = solid.blocks("side").next().unwrap();
        assert_eq!(side.get("material"), Some("DEV/DEV_MEASUREGENERIC01B"));
    }
    for ids in [&solid_ids, &side_ids] {
        let unique: std::collections::BTreeSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "no id is used twice: {ids:?}");
    }
}

#[test]
fn a_saved_shape_opens_as_the_same_shape() {
    let doc = doc_with(vec![l_shape(0.0), l_shape(512.0)]);
    let text = vmf_from_document(&doc).to_text();
    let again = open(&text);
    let found = shapes(&again);
    assert_eq!(found.len(), 2, "each L comes back whole");
    assert_eq!(again.objects().count(), 2, "no loose pieces left over");
    let originals = shapes(&doc);
    for shape in &found {
        assert!(originals.iter().any(|o| o.outline() == shape.outline()));
        assert_eq!((shape.bottom(), shape.top()), (0.0, 128.0));
        assert_eq!(shape.look.material, "DEV/DEV_MEASUREGENERIC01B");
    }
    // And saving again gives the same file.
    assert_eq!(vmf_from_document(&again).to_text(), text);
}

#[test]
fn a_piece_moved_in_another_editor_comes_back_as_plain_brushes() {
    let doc = doc_with(vec![l_shape(0.0)]);
    let text = vmf_from_document(&doc).to_text();
    // Hammer moved one piece's top up 16 units, so it no longer matches
    // the outline.
    let moved = text.replacen("(0 0 128)", "(0 0 144)", 1);
    assert_ne!(moved, text, "the test moved something");
    let again = open(&moved);
    assert!(shapes(&again).is_empty(), "the shape no longer matches");
    let brushes = again
        .objects()
        .filter(|(_, o)| matches!(o, Object::Brush(_)))
        .count();
    assert_eq!(brushes, 2, "its pieces stay, as brushes");
}

#[test]
fn a_broken_outline_note_leaves_plain_brushes() {
    let doc = doc_with(vec![l_shape(0.0)]);
    let text = vmf_from_document(&doc).to_text();
    let start = text.find("\"halberd_outline\" \"").unwrap() + "\"halberd_outline\" \"".len();
    let end = start + text[start..].find('"').unwrap();
    for broken in [
        "",
        "nonsense",
        "0 0 1 | 0 64",
        "0 0 256 0 | 64 0",
        "1e40 0 0 1 1 1 | 0 64",
    ] {
        let edited = format!("{}{}{}", &text[..start], broken, &text[end..]);
        let again = open(&edited);
        assert!(shapes(&again).is_empty(), "{broken:?}");
        assert_eq!(again.objects().count(), 2, "{broken:?}");
    }
}
