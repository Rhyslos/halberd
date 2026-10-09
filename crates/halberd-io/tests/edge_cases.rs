//! Edge cases found in review: sides that add nothing to a shape, ids in
//! hidden places, faces far from the middle of the map, and backups.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use glam::Vec3;
use halberd_doc::{Command, Document, TextEncoding};
use halberd_geom::{Aabb, Brush};
use halberd_io::{BACKUP_EXTENSION, open_map, open_map_text, save_map, vmf_from_document};
use halberd_vmf::{Block, Entry, Vmf};

const SAMPLE: &str = include_str!("../../halberd-vmf/tests/data/sample.vmf");

fn open(text: &str) -> Document {
    open_map_text(text, TextEncoding::Utf8).unwrap().document
}

fn saved_text(doc: &Document) -> String {
    vmf_from_document(doc).to_text()
}

/// Every side id and every solid/entity id in `text`, hidden ones included.
fn ids_by_kind(text: &str) -> (Vec<u64>, Vec<u64>) {
    fn walk(block: &Block, sides: &mut Vec<u64>, others: &mut Vec<u64>) {
        if let Some(id) = halberd_vmf::id_of(block) {
            match block.name.to_ascii_lowercase().as_str() {
                "side" => sides.push(id),
                "solid" | "entity" | "world" => others.push(id),
                _ => {}
            }
        }
        for e in &block.entries {
            if let Entry::Block(b) = e {
                walk(b, sides, others);
            }
        }
    }
    let vmf = Vmf::parse(text).unwrap();
    let (mut sides, mut others) = (Vec::new(), Vec::new());
    for e in &vmf.entries {
        if let Entry::Block(b) = e {
            walk(b, &mut sides, &mut others);
        }
    }
    (sides, others)
}

fn assert_unique(ids: &[u64]) {
    let unique: std::collections::BTreeSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "no id is used twice: {ids:?}");
}

#[test]
fn a_solid_with_a_repeated_side_is_kept_whole() {
    // Regression: a side repeating another one was dropped from the shape,
    // and saving lost it (with its id and material).
    let extra = "\t\tside\r\n\t\t{\r\n\t\t\t\"id\" \"77\"\r\n\t\t\t\"plane\" \"(0 64 64) (64 64 64) (64 0 64)\"\r\n\t\t\t\"material\" \"DUPLICATE\"\r\n\t\t}\r\n";
    let marker = "\t\t\"id\" \"9\"\r\n";
    let text = SAMPLE.replacen(marker, &format!("{marker}{extra}"), 1);
    assert_ne!(text, SAMPLE);
    let opened = open_map_text(&text, TextEncoding::Utf8).unwrap();
    assert_eq!(opened.notes.len(), 1, "{:?}", opened.notes);
    let saved = saved_text(&opened.document);
    assert!(saved.contains("DUPLICATE"), "the repeated side is saved");
    assert!(saved.contains("\"id\" \"77\""));
}

#[test]
fn new_ids_never_clash_with_hidden_objects() {
    // Regression: entities hidden by a visgroup (in a top-level `hidden`
    // block) were not counted, so new brushes reused their ids.
    let hidden = "hidden\r\n{\r\n\tentity\r\n\t{\r\n\t\t\"id\" \"500\"\r\n\t\t\"classname\" \"func_detail\"\r\n\t\tsolid\r\n\t\t{\r\n\t\t\t\"id\" \"501\"\r\n\t\t\tside\r\n\t\t\t{\r\n\t\t\t\t\"id\" \"600\"\r\n\t\t\t}\r\n\t\t}\r\n\t}\r\n}\r\n";
    let at = SAMPLE.find("cameras").unwrap();
    let text = format!("{}{hidden}{}", &SAMPLE[..at], &SAMPLE[at..]);
    let mut doc = open(&text);
    assert_eq!(saved_text(&doc), text, "kept as read");
    let brush = Brush::cuboid(Aabb::from_corners(Vec3::splat(500.0), Vec3::splat(564.0))).unwrap();
    doc.execute(Command::AddBrushes(vec![brush])).unwrap();
    let (sides, others) = ids_by_kind(&saved_text(&doc));
    assert_unique(&sides);
    assert_unique(&others);
    assert!(sides.iter().any(|&id| id > 600), "new sides after 600");
}

#[test]
fn absurd_ids_do_not_crash_or_wrap() {
    // Regression: an id of u64::MAX made the next id overflow.
    let text = SAMPLE.replacen("\"id\" \"30\"", "\"id\" \"18446744073709551615\"", 1);
    assert_ne!(text, SAMPLE);
    let mut doc = open(&text);
    let brush = Brush::cuboid(Aabb::from_corners(Vec3::ZERO, Vec3::splat(64.0))).unwrap();
    doc.execute(Command::AddBrushes(vec![brush])).unwrap();
    let (sides, others) = ids_by_kind(&saved_text(&doc));
    let new_solid = others
        .iter()
        .copied()
        .filter(|&id| id < 1000)
        .max()
        .unwrap();
    assert!(new_solid > 23, "a usable id after the real ones");
    assert!(sides.iter().all(|&id| id < 1000));
}

#[test]
fn untouched_faces_far_from_the_middle_keep_their_text() {
    // Regression: faces of turned brushes near the edge of the map were
    // rewritten on every save, because rounding there is coarser than the
    // "unchanged" check allowed.
    let mut doc = Document::new();
    let brushes: Vec<Brush> = (0..40)
        .map(|i| {
            let corner = Vec3::new(15000.0, -15000.0, 15000.0) + Vec3::splat(i as f32 * 3.0);
            Brush::cuboid(Aabb::from_corners(
                corner,
                corner + Vec3::new(64.0, 48.0, 32.0),
            ))
            .unwrap()
            .rotated(
                Vec3::new(1.0, 2.0, 3.0 + i as f32),
                0.3 + i as f32 * 0.1,
                corner,
            )
            .unwrap()
        })
        .collect();
    doc.execute(Command::AddBrushes(brushes)).unwrap();
    let first = saved_text(&doc);
    let second = saved_text(&open(&first));
    assert_eq!(second, first, "an untouched map saves back the same");
}

#[test]
fn saving_a_vmx_file_over_itself_keeps_the_map() {
    // Regression: the backup of map.vmx is map.vmx itself; copying a file
    // onto itself could empty it.
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("map.vmx");
    std::fs::write(&path, SAMPLE).unwrap();
    let mut doc = open_map(&path).unwrap().document;
    let (id, _) = doc.objects().next().unwrap();
    doc.execute(Command::Remove(vec![id])).unwrap();
    let notes = save_map(&mut doc, &path).unwrap();
    assert!(notes.is_empty(), "{notes:?}");
    let reopened = open_map(&path).unwrap().document;
    assert_eq!(reopened.len(), doc.len());
}

#[test]
fn a_blocked_backup_does_not_stop_saving() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("map.vmf");
    std::fs::write(&path, SAMPLE).unwrap();
    // A folder where the backup would go: it cannot be written.
    std::fs::create_dir(path.with_extension(BACKUP_EXTENSION)).unwrap();
    let mut doc = open_map(&path).unwrap().document;
    let (id, _) = doc.objects().next().unwrap();
    doc.execute(Command::Remove(vec![id])).unwrap();
    let notes = save_map(&mut doc, &path).unwrap();
    assert_eq!(notes.len(), 1);
    assert!(notes[0].contains("No backup"), "{}", notes[0]);
    assert!(!doc.is_modified());
    assert_eq!(open_map(&path).unwrap().document.len(), doc.len());
}

#[test]
fn a_byte_order_mark_is_kept() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("marked.vmf");
    let bytes = [b"\xEF\xBB\xBF".as_slice(), SAMPLE.as_bytes()].concat();
    std::fs::write(&path, &bytes).unwrap();
    let mut doc = open_map(&path).unwrap().document;
    assert_eq!(doc.file_data().encoding, TextEncoding::Utf8WithMark);
    let out = folder.path().join("again.vmf");
    save_map(&mut doc, &out).unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), bytes);
}

/// The sample with a broken solid (id 77) inside the func_detail, after its
/// shown solid (id 23).
fn sample_with_broken_entity_solid() -> String {
    let after = SAMPLE.find("\"id\" \"23\"").unwrap();
    let at = after + SAMPLE[after..].find("\r\n\teditor\r\n").unwrap() + 2;
    let broken = "\tsolid\r\n\t{\r\n\t\t\"id\" \"77\"\r\n\t\tside\r\n\t\t{\r\n\t\t\t\"plane\" \"(0 0 0) (1 0 0) (0 1 0)\"\r\n\t\t}\r\n\t}\r\n";
    format!("{}{broken}{}", &SAMPLE[..at], &SAMPLE[at..])
}

#[test]
fn entity_brushes_go_back_in_their_places_around_unshown_ones() {
    // Regression: shown brushes were all put just before `editor`, so an
    // unshown brush after them moved in front.
    let text = sample_with_broken_entity_solid();
    let doc = open(&text);
    assert_eq!(saved_text(&doc), text);
}

#[test]
fn deleting_an_entitys_shown_brushes_keeps_its_unshown_ones() {
    let text = sample_with_broken_entity_solid();
    let mut doc = open(&text);
    let (detail, _) = doc
        .objects()
        .find(|(_, o)| matches!(o, halberd_doc::Object::Entity(e) if e.classname == "func_detail"))
        .unwrap();
    let brushes: Vec<_> = doc.brushes_of(detail).collect();
    doc.execute(Command::Remove(brushes)).unwrap();
    assert!(doc.get(detail).is_some(), "the entity stays");
    let saved = saved_text(&doc);
    assert!(
        saved.contains("\"id\" \"77\""),
        "the unshown brush is saved"
    );
    assert!(!saved.contains("\"id\" \"23\""));
}
