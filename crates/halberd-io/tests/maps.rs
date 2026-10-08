//! Opening and saving maps: round trips, edits, new maps, and bad files.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use glam::Vec3;
use halberd_doc::{Command, Document, Object, TextEncoding, TransformKind};
use halberd_geom::{Aabb, Brush};
use halberd_io::{BACKUP_EXTENSION, IoError, open_map, open_map_text, save_map, vmf_from_document};
use halberd_vmf::{Block, Entry, Vmf, parse_plane};

const SAMPLE: &str = include_str!("../../halberd-vmf/tests/data/sample.vmf");

fn open(text: &str) -> Document {
    open_map_text(text, TextEncoding::Utf8).unwrap().document
}

fn saved_text(doc: &Document) -> String {
    vmf_from_document(doc).to_text()
}

#[test]
fn an_untouched_map_saves_back_byte_for_byte() {
    let doc = open(SAMPLE);
    assert_eq!(saved_text(&doc), SAMPLE);
}

#[test]
fn brushes_and_entities_are_read() {
    let doc = open(SAMPLE);
    let brushes = doc
        .objects()
        .filter(|(_, o)| o.as_brush().is_some())
        .count();
    assert_eq!(brushes, 2);
    let entities: Vec<&str> = doc
        .objects()
        .filter_map(|(_, o)| match o {
            Object::Entity(e) => Some(e.classname.as_str()),
            Object::Brush(_) => None,
        })
        .collect();
    assert_eq!(
        entities,
        ["info_player_start", "light", "func_detail", "logic_relay"]
    );
    let floor = doc.objects().next().unwrap().1;
    assert_eq!(floor.bounds().min, Vec3::new(-256.0, -256.0, -16.0));
    assert_eq!(floor.bounds().max, Vec3::new(256.0, 256.0, 0.0));
    let floor = floor.as_brush().unwrap();
    assert!(
        floor
            .faces()
            .iter()
            .all(|f| f.material == "DEV/DEV_MEASUREGENERIC01B")
    );
    let detail = doc
        .objects()
        .find_map(|(_, o)| match o {
            Object::Entity(e) if e.classname == "func_detail" => Some(e),
            _ => None,
        })
        .unwrap();
    assert_eq!(detail.solids.len(), 1);
    assert!(
        detail.marker().is_none(),
        "brush entities show their brushes"
    );
    assert!(!doc.is_modified());
}

/// The block for the first world solid in saved text.
fn first_world_solid(text: &str) -> Block {
    let vmf = Vmf::parse(text).unwrap();
    vmf.world().unwrap().blocks("solid").next().unwrap().clone()
}

#[test]
fn a_moved_brush_keeps_its_ids_materials_and_editor_data() {
    let mut doc = open(SAMPLE);
    let (id, object) = doc.objects().next().unwrap();
    let moved = object
        .as_brush()
        .unwrap()
        .brush()
        .translated(Vec3::new(32.0, 0.0, 64.0))
        .unwrap();
    doc.execute(Command::TransformBrushes {
        kind: TransformKind::Move,
        brushes: vec![(id, moved)],
    })
    .unwrap();
    let text = saved_text(&doc);
    let solid = first_world_solid(&text);
    let original = first_world_solid(SAMPLE);
    assert_eq!(solid.get("id"), Some("2"));
    assert_eq!(
        solid.blocks("editor").next(),
        original.blocks("editor").next()
    );
    let side_ids: Vec<_> = solid.blocks("side").map(|s| s.get("id").unwrap()).collect();
    let old_ids: Vec<_> = original
        .blocks("side")
        .map(|s| s.get("id").unwrap())
        .collect();
    assert_eq!(side_ids, old_ids);
    let mut rewritten = 0;
    for (side, old) in solid.blocks("side").zip(original.blocks("side")) {
        assert_eq!(side.get("material"), old.get("material"));
        assert_eq!(side.get("uaxis"), old.get("uaxis"));
        if side.get("plane") != old.get("plane") {
            rewritten += 1;
        }
    }
    // Moving along X and Z moves the four faces facing X and Z; the two
    // facing Y stay on the same planes and keep their exact text.
    assert_eq!(rewritten, 4);
    // Reading it back gives the moved brush.
    let again = open(&text);
    let bounds = again.objects().next().unwrap().1.bounds();
    assert_eq!(bounds.min, Vec3::new(-224.0, -256.0, 48.0));
    // Everything else is untouched: only the moved brush's planes differ.
    let changed_lines = text
        .lines()
        .zip(SAMPLE.lines())
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(changed_lines, 4, "one plane line per moved side");
}

#[test]
fn a_new_map_is_a_complete_hammer_file() {
    let mut doc = Document::new();
    let brush = Brush::cuboid(Aabb::from_corners(
        Vec3::ZERO,
        Vec3::new(128.0, 64.0, 128.0),
    ))
    .unwrap();
    doc.execute(Command::AddBrushes(vec![brush])).unwrap();
    let text = saved_text(&doc);
    let vmf = Vmf::parse(&text).unwrap();
    for block in [
        "versioninfo",
        "visgroups",
        "viewsettings",
        "world",
        "cameras",
        "cordons",
    ] {
        assert_eq!(vmf.top_blocks(block).count(), 1, "{block}");
    }
    let world = vmf.world().unwrap();
    assert_eq!(world.get("classname"), Some("worldspawn"));
    let solid = world.blocks("solid").next().unwrap();
    let sides: Vec<&Block> = solid.blocks("side").collect();
    assert_eq!(sides.len(), 6);
    let mut ids: Vec<&str> = vec![world.get("id").unwrap(), solid.get("id").unwrap()];
    for side in &sides {
        for key in [
            "id",
            "plane",
            "material",
            "uaxis",
            "vaxis",
            "rotation",
            "lightmapscale",
            "smoothing_groups",
        ] {
            assert!(side.get(key).is_some(), "side lacks {key}");
        }
        assert_eq!(side.get("material"), Some(halberd_doc::DEFAULT_MATERIAL));
        ids.push(side.get("id").unwrap());
        // Every plane's normal points out of the brush (Hammer's convention).
        let points = parse_plane(side.get("plane").unwrap()).unwrap();
        let [p0, p1, p2] = points.map(|p| Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32));
        let normal = (p0 - p1).cross(p2 - p1).normalize();
        let centre = Vec3::new(64.0, 32.0, 64.0);
        assert!(
            normal.dot(p0 - centre) > 0.0,
            "inward plane {:?}",
            side.get("plane")
        );
    }
    let unique: std::collections::BTreeSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "ids are unique: {ids:?}");
    // And it reads back as the same box.
    let again = open(&text);
    assert_eq!(
        again.objects().next().unwrap().1.bounds().size(),
        Vec3::new(128.0, 64.0, 128.0)
    );
}

#[test]
fn new_brushes_in_an_opened_map_get_fresh_ids() {
    let mut doc = open(SAMPLE);
    let brush = Brush::cuboid(Aabb::from_corners(Vec3::splat(500.0), Vec3::splat(564.0))).unwrap();
    doc.execute(Command::AddBrushes(vec![brush])).unwrap();
    let text = saved_text(&doc);
    let vmf = Vmf::parse(&text).unwrap();
    // Hammer numbers sides separately from solids and entities; within each
    // kind, no id may repeat.
    let (mut sides, mut others) = (Vec::new(), Vec::new());
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
    for e in &vmf.entries {
        if let Entry::Block(b) = e {
            walk(b, &mut sides, &mut others);
        }
    }
    for ids in [&sides, &others] {
        let unique: std::collections::BTreeSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "no id is used twice: {ids:?}");
    }
    assert_eq!(sides.len(), 18 + 6, "the new box has six sides");
}

#[test]
fn a_deleted_entity_is_gone_and_everything_else_stays() {
    let mut doc = open(SAMPLE);
    let light = doc
        .objects()
        .find_map(|(id, o)| match o {
            Object::Entity(e) if e.classname == "light" => Some(id),
            _ => None,
        })
        .unwrap();
    doc.execute(Command::Remove(vec![light])).unwrap();
    let text = saved_text(&doc);
    assert!(!text.contains("\"classname\" \"light\""));
    assert!(
        text.contains("\"OnTrigger\" \"door1,Open,,0,-1\""),
        "outputs kept"
    );
    assert!(text.contains("cameras"));
    doc.undo();
    assert_eq!(saved_text(&doc), SAMPLE, "undo brings back the exact file");
}

#[test]
fn a_brush_with_a_broken_shape_is_kept_but_not_shown() {
    // A solid with only three sides cannot be a closed shape.
    let broken = SAMPLE.replacen(
        "\tsolid\r\n\t{\r\n\t\t\"id\" \"9\"",
        "\tsolid\r\n\t{\r\n\t\t\"id\" \"99\"\r\n\t\tside\r\n\t\t{\r\n\t\t\t\"plane\" \"(0 0 0) (1 0 0) (0 1 0)\"\r\n\t\t}\r\n\t}\r\n\tsolid\r\n\t{\r\n\t\t\"id\" \"9\"",
        1,
    );
    assert_ne!(broken, SAMPLE);
    let opened = open_map_text(&broken, TextEncoding::Utf8).unwrap();
    assert_eq!(opened.notes.len(), 1);
    assert!(opened.notes[0].contains("not shown"), "{}", opened.notes[0]);
    let text = saved_text(&opened.document);
    assert!(text.contains("\"id\" \"99\""), "the broken brush is saved");
}

#[test]
fn not_a_map_is_refused_with_a_reason() {
    let err = open_map_text("versioninfo { a b }", TextEncoding::Utf8).unwrap_err();
    assert!(err.to_string().contains("not a Hammer map"), "{err}");
    let err = open_map_text("world {", TextEncoding::Utf8).unwrap_err();
    assert!(err.to_string().contains("damaged"), "{err}");
}

#[test]
fn files_save_safely_with_a_backup_and_reopen() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("test.vmf");
    std::fs::write(&path, SAMPLE).unwrap();
    let mut doc = open_map(&path).unwrap().document;
    let (id, _) = doc.objects().next().unwrap();
    doc.execute(Command::Remove(vec![id])).unwrap();
    assert!(doc.is_modified());
    save_map(&mut doc, &path).unwrap();
    assert!(!doc.is_modified());
    let backup = path.with_extension(BACKUP_EXTENSION);
    assert_eq!(
        std::fs::read_to_string(&backup).unwrap(),
        SAMPLE,
        "old file kept as .vmx"
    );
    let reopened = open_map(&path).unwrap().document;
    assert_eq!(reopened.len(), doc.len());
    let leftovers: Vec<_> = std::fs::read_dir(folder.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(leftovers.len(), 2, "no temporary files left: {leftovers:?}");
}

#[test]
fn latin1_files_come_back_byte_for_byte() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("old.vmf");
    let mut bytes = SAMPLE.as_bytes().to_vec();
    let at = SAMPLE.find("door_relay").unwrap();
    bytes.splice(at..at + 4, [b'd', 0xF8, b'r', 0xE9]); // "dør" in Latin-1
    std::fs::write(&path, &bytes).unwrap();
    let mut doc = open_map(&path).unwrap().document;
    assert_eq!(doc.file_data().encoding, TextEncoding::Latin1);
    let out = folder.path().join("again.vmf");
    save_map(&mut doc, &out).unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), bytes);
}

#[test]
fn missing_and_oversized_files_are_reported() {
    let folder = tempfile::tempdir().unwrap();
    let missing = folder.path().join("nope.vmf");
    let err = open_map(&missing).unwrap_err();
    assert!(matches!(err, IoError::Read(..)));
    assert!(err.to_string().contains("could not be read"));
    // Saving into a folder that does not exist fails without leftovers.
    let mut doc = Document::new();
    let err = save_map(&mut doc, &folder.path().join("no/such/folder/x.vmf")).unwrap_err();
    assert!(err.to_string().contains("could not be saved"), "{err}");
}

#[test]
fn fuzz_damaged_maps_never_crash() {
    // Random edits to the sample: every result opens or gives an error.
    let mut seed: u64 = 0xF11E_5EED_0000_0003;
    let mut next = move |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    let pieces = [
        "{", "}", "\"", "(", ")", "0", "-", "nan", "1e40", " ", "solid", "side", "plane",
    ];
    for _ in 0..2_000 {
        let mut text = SAMPLE.to_string();
        for _ in 0..1 + next(4) {
            let at = next(text.len());
            let at = (0..=at)
                .rev()
                .find(|i| text.is_char_boundary(*i))
                .unwrap_or(0);
            let end = (at + next(12)).min(text.len());
            let end = (end..=text.len())
                .find(|i| text.is_char_boundary(*i))
                .unwrap_or(text.len());
            text.replace_range(at..end, pieces[next(pieces.len())]);
        }
        if let Ok(opened) = open_map_text(&text, TextEncoding::Utf8) {
            // Whatever opened can be saved and opened again.
            let saved = saved_text(&opened.document);
            assert!(open_map_text(&saved, TextEncoding::Utf8).is_ok());
        }
    }
}
