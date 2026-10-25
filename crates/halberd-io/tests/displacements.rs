//! Displacements through saving and opening: made in Halberd, written
//! as Hammer writes them, read back the same, and moved with their brush.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use glam::Vec3;
use halberd_doc::{
    BrushObject, Command, Displacement, Document, ObjectId, TextEncoding, ordered_corners,
};
use halberd_geom::{Aabb, Brush};
use halberd_io::{open_map_text, vmf_from_document};

/// A map with one box whose top is a power 2 displacement, raised and
/// blended at a few points.
fn terrain() -> (Document, ObjectId) {
    let brush = Brush::cuboid(Aabb::from_corners(
        Vec3::new(0.0, 0.0, -32.0),
        Vec3::new(512.0, 256.0, 0.0),
    ))
    .unwrap();
    let mut doc = Document::new();
    doc.execute(Command::AddBrushes(vec![brush.clone()]))
        .unwrap();
    let id = *doc.selection().iter().next().unwrap();
    let top = brush
        .faces()
        .iter()
        .position(|f| f.plane().normal.z > 0.9)
        .unwrap();
    let shape = &brush.faces()[top];
    let corners = ordered_corners(shape.vertices(), Vec3::Z, shape.vertices()[0]).unwrap();
    let mut disp = Displacement::flat(2, corners[0], Vec3::Z);
    disp.raise(12, 48.5, Vec3::Z);
    disp.raise(3, -8.0, Vec3::Z);
    disp.set_alpha(24, 200.0);
    let object = doc.get(id).unwrap().as_brush().unwrap();
    let mut faces = object.faces().to_vec();
    faces[top] = faces[top].with_displacement(Some(&disp));
    doc.execute(Command::ShapeEdit {
        label: "Make displacement".into(),
        changed: vec![(id, BrushObject::with_faces(brush, faces, Vec::new()))],
        added: vec![],
        removed: vec![],
        select_added: false,
    })
    .unwrap();
    (doc, id)
}

fn displacements(doc: &Document) -> Vec<Displacement> {
    doc.objects()
        .filter_map(|(_, o)| o.as_brush())
        .flat_map(|b| b.faces().iter().filter_map(|f| f.displacement()))
        .collect()
}

fn reopen(doc: &Document) -> (String, Document) {
    let text = vmf_from_document(doc).to_text();
    let back = open_map_text(&text, TextEncoding::Utf8).unwrap().document;
    (text, back)
}

#[test]
fn a_displacement_is_saved_as_hammer_writes_it_and_read_back() {
    let (doc, _) = terrain();
    let (text, back) = reopen(&doc);
    for part in [
        "dispinfo",
        "\"power\" \"2\"",
        "\"startposition\"",
        "normals",
        "distances",
        "offset_normals",
        "alphas",
    ] {
        assert!(text.contains(part), "{part} is written");
    }
    assert_eq!(displacements(&back), displacements(&doc));
    // Saving again changes nothing.
    assert_eq!(reopen(&back).0, text);
}

#[test]
fn moving_the_brush_moves_the_saved_start_corner() {
    let (mut doc, id) = terrain();
    let before = displacements(&doc)[0].clone();
    let brush = doc.get(id).unwrap().as_brush().unwrap().brush();
    let offset = Vec3::new(1024.0, -512.0, 64.0);
    let moved = brush.translated(offset).unwrap();
    doc.execute(Command::ReplaceBrush { id, brush: moved })
        .unwrap();
    let (_, back) = reopen(&doc);
    let after = displacements(&back)[0].clone();
    assert_eq!(after.start, before.start + offset);
    assert_eq!(after.distances, before.distances);
    assert_eq!(after.alphas, before.alphas);
}
