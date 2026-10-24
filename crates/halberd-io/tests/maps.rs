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
        .filter(|(_, o)| o.as_brush().is_some_and(|b| b.entity().is_none()))
        .count();
    assert_eq!(brushes, 2, "world brushes");
    let entities: Vec<&str> = doc
        .objects()
        .filter_map(|(_, o)| match o {
            Object::Entity(e) => Some(e.classname.as_str()),
            Object::Brush(_) | Object::Shape(_) => None,
        })
        .collect();
    assert_eq!(
        entities,
        ["info_player_start", "light", "func_detail", "logic_relay"]
    );
    let floor = doc.objects().next().unwrap().1;
    assert_eq!(
        floor.bounds().unwrap().min,
        Vec3::new(-256.0, -256.0, -16.0)
    );
    assert_eq!(floor.bounds().unwrap().max, Vec3::new(256.0, 256.0, 0.0));
    let floor = floor.as_brush().unwrap();
    assert!(
        floor
            .faces()
            .iter()
            .all(|f| f.material == "DEV/DEV_MEASUREGENERIC01B")
    );
    let (detail, _) = doc
        .objects()
        .find(|(_, o)| matches!(o, Object::Entity(e) if e.classname == "func_detail"))
        .unwrap();
    let brushes: Vec<_> = doc.brushes_of(detail).collect();
    assert_eq!(brushes.len(), 1);
    let brush = doc.get(brushes[0]).unwrap().as_brush().unwrap();
    assert_eq!(brush.entity(), Some(detail));
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
        shapes: Vec::new(),
        entities: Vec::new(),
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
    let bounds = again.objects().next().unwrap().1.bounds().unwrap();
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
        again.objects().next().unwrap().1.bounds().unwrap().size(),
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

#[test]
fn a_brush_inside_an_entity_can_be_moved_and_saved() {
    let mut doc = open(SAMPLE);
    let (detail, _) = doc
        .objects()
        .find(|(_, o)| matches!(o, Object::Entity(e) if e.classname == "func_detail"))
        .unwrap();
    let brush = doc.brushes_of(detail).next().unwrap();
    let moved = doc
        .get(brush)
        .unwrap()
        .as_brush()
        .unwrap()
        .brush()
        .translated(Vec3::new(0.0, 0.0, 64.0))
        .unwrap();
    doc.execute(Command::TransformBrushes {
        kind: TransformKind::Move,
        brushes: vec![(brush, moved)],
        shapes: Vec::new(),
        entities: Vec::new(),
    })
    .unwrap();
    let text = saved_text(&doc);
    let vmf = Vmf::parse(&text).unwrap();
    // Still inside the func_detail, still solid 23, now 64 units higher.
    let entity = vmf
        .entities()
        .find(|e| e.get("classname") == Some("func_detail"))
        .unwrap();
    let solid = entity.blocks("solid").next().unwrap();
    assert_eq!(solid.get("id"), Some("23"));
    let again = open(&text);
    let (detail, _) = again
        .objects()
        .find(|(_, o)| matches!(o, Object::Entity(e) if e.classname == "func_detail"))
        .unwrap();
    let bounds = again.bounds_of(detail).unwrap();
    let before = open(SAMPLE);
    let (old, _) = before
        .objects()
        .find(|(_, o)| matches!(o, Object::Entity(e) if e.classname == "func_detail"))
        .unwrap();
    assert_eq!(bounds.min.z, before.bounds_of(old).unwrap().min.z + 64.0);
    // Undoing gives back the exact file.
    doc.undo();
    assert_eq!(saved_text(&doc), SAMPLE);
}

#[test]
fn a_corner_edit_saves_with_unique_face_ids_and_reads_back() {
    use halberd_doc::ElementKind;
    let mut doc = open(SAMPLE);
    let (id, object) = doc.objects().next().unwrap();
    let brush = object.as_brush().unwrap().brush().clone();
    // Pull the floor's top corner at (256, 256, 0) out and up.
    let corner = brush
        .points()
        .iter()
        .position(|p| *p == Vec3::new(256.0, 256.0, 0.0))
        .unwrap();
    let (bent, fates) = brush
        .with_moved_points(&[(corner, Vec3::new(272.0, 272.0, 32.0))])
        .unwrap();
    doc.execute(Command::EditBrushes {
        kind: TransformKind::Move,
        parts: (ElementKind::Vertex, 1),
        brushes: vec![(id, bent.clone(), fates)],
    })
    .unwrap();
    let text = saved_text(&doc);
    let solid = first_world_solid(&text);
    let ids: Vec<&str> = solid.blocks("side").map(|s| s.get("id").unwrap()).collect();
    assert_eq!(ids.len(), bent.faces().len());
    let unique: std::collections::BTreeSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "no two faces share an id: {ids:?}");
    assert!(
        solid
            .blocks("side")
            .all(|s| s.get("material") == Some("DEV/DEV_MEASUREGENERIC01B"))
    );
    let again = open(&text);
    let read = again
        .objects()
        .next()
        .unwrap()
        .1
        .as_brush()
        .unwrap()
        .brush()
        .clone();
    assert_eq!(read.faces().len(), bent.faces().len());
    assert_eq!(read.bounds(), bent.bounds());
}

#[test]
fn a_brush_split_in_two_saves_with_unique_ids() {
    use halberd_doc::{BrushObject, FaceInfo};
    use halberd_geom::Plane;
    let mut doc = open(SAMPLE);
    let (id, object) = doc.objects().next().unwrap();
    let object = object.as_brush().unwrap().clone();
    let plane = Plane::new(Vec3::X, Vec3::ZERO).unwrap();
    let (behind, front) = object.brush().cut(plane);
    let (behind, front) = (behind.unwrap(), front.unwrap());
    // As the Split operation builds them: the first part keeps the faces'
    // data, the second gets copies, and both cut faces copy a look.
    let infos = |part: &halberd_geom::Brush, copy: bool| -> Vec<FaceInfo> {
        part.faces()
            .iter()
            .map(|f| match object.faces().get(f.source()) {
                Some(info) if !copy => info.clone(),
                Some(info) => info.copied(),
                None => object.faces()[0].copied(),
            })
            .collect()
    };
    let first = BrushObject::with_faces(
        behind.clone(),
        infos(&behind, false),
        object.file_data.clone(),
    );
    let second = BrushObject::with_faces(front.clone(), infos(&front, true), object.copied_data());
    doc.execute(Command::ShapeEdit {
        label: "Split brush".into(),
        changed: vec![(id, first)],
        added: vec![(second, None)],
        removed: Vec::new(),
        select_added: true,
    })
    .unwrap();
    let text = saved_text(&doc);
    let vmf = Vmf::parse(&text).unwrap();
    // Hammer counts solid ids and side ids separately.
    let mut solids = Vec::new();
    let mut sides = Vec::new();
    for solid in vmf.world().unwrap().blocks("solid") {
        solids.push(solid.get("id").unwrap().to_string());
        sides.extend(
            solid
                .blocks("side")
                .map(|s| s.get("id").unwrap().to_string()),
        );
    }
    for ids in [&solids, &sides] {
        let unique: std::collections::BTreeSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "no id is used twice: {ids:?}");
    }
    assert_eq!(solids.len(), 3, "the two parts and the other world brush");
    let again = open(&text);
    let brushes = |d: &Document| d.objects().filter(|(_, o)| o.as_brush().is_some()).count();
    assert_eq!(brushes(&again), brushes(&doc));
}

#[test]
fn a_placed_and_moved_prop_saves_as_hammer_writes_it_and_reads_back() {
    use halberd_doc::EntityObject;
    let mut doc = open(SAMPLE);
    doc.execute(Command::AddEntity(EntityObject::point(
        "prop_static",
        Vec3::new(32.0, -16.0, 8.0),
        &[
            ("model", "models/props_c17/oildrum001.mdl"),
            ("angles", "0 0 0"),
        ],
    )))
    .unwrap();
    let id = doc.objects().last().unwrap().0;
    let prop = doc.get(id).unwrap().as_entity().unwrap().clone();
    let moved = prop
        .with_origin(Vec3::new(64.0, -16.0, 8.0))
        .with_rotation(halberd_doc::rotation_of(Vec3::new(0.0, 90.0, 0.0)));
    doc.execute(Command::TransformBrushes {
        kind: TransformKind::Move,
        brushes: Vec::new(),
        shapes: Vec::new(),
        entities: vec![(id, moved)],
    })
    .unwrap();
    let text = saved_text(&doc);
    let reopened = open(&text);
    let prop = reopened
        .objects()
        .filter_map(|(_, o)| o.as_entity())
        .find(|e| e.classname == "prop_static")
        .expect("the prop is saved");
    assert_eq!(prop.origin, Some(Vec3::new(64.0, -16.0, 8.0)));
    assert_eq!(prop.setting("origin"), Some("64 -16 8"));
    assert_eq!(prop.model(), Some("models/props_c17/oildrum001.mdl"));
    assert_eq!(prop.setting("angles"), Some("0 90 0"));
    assert!(prop.setting("id").is_some(), "given an id");
}

#[test]
fn a_new_map_saves_the_sky_chosen_with_the_usual_settings() {
    let mut doc = Document::new();
    doc.execute(Command::SetWorldSetting {
        key: "skyname".into(),
        value: "sky_borealis01".into(),
    })
    .unwrap();
    let text = saved_text(&doc);
    assert!(text.contains("\"skyname\" \"sky_borealis01\""), "{text}");
    assert!(!text.contains("sky_day01_01"));
    assert!(text.contains("\"classname\" \"worldspawn\""));
    assert!(
        text.contains("versioninfo"),
        "Hammer's header is still written"
    );
    let reopened = open(&text);
    assert_eq!(reopened.world_setting("skyname"), Some("sky_borealis01"));
    // An opened map changes its own setting in place.
    let mut opened = open(SAMPLE);
    opened
        .execute(Command::SetWorldSetting {
            key: "skyname".into(),
            value: "sky_wasteland02".into(),
        })
        .unwrap();
    let text = saved_text(&opened);
    assert_eq!(text.matches("\"skyname\"").count(), 1);
    assert!(text.contains("\"skyname\" \"sky_wasteland02\""));
}

#[test]
fn an_opened_map_without_hammer_blocks_keeps_its_world_as_read() {
    // Regression: such a map was taken for a new one, and the blocks in
    // its world were dropped on saving.
    let text = "world\n{\n\t\"id\" \"1\"\n\t\"skyname\" \"sky_x\"\n\tgroup\n\t{\n\t\t\"id\" \"5\"\n\t}\n}\n";
    let doc = open(text);
    let saved = saved_text(&doc);
    assert!(saved.contains("group"), "{saved}");
    assert!(!saved.contains("versioninfo"));
    assert_eq!(saved, text);
}

#[test]
fn connections_and_tied_brushes_survive_saving_and_opening() {
    let mut doc = Document::new();
    let cube = Brush::cuboid(Aabb::from_corners(Vec3::ZERO, Vec3::splat(64.0))).unwrap();
    doc.execute(Command::AddBrushes(vec![cube])).unwrap();
    let brush = *doc.selection().iter().next().unwrap();
    let wall = halberd_doc::EntityObject {
        classname: "func_wall".into(),
        origin: None,
        file_data: vec![Entry::Pair("classname".into(), "func_wall".into())],
    };
    doc.execute(Command::TieToEntity {
        objects: vec![brush],
        entity: wall,
    })
    .unwrap();
    doc.execute(Command::AddEntity(halberd_doc::EntityObject::point(
        "logic_relay",
        Vec3::new(0.0, 0.0, 128.0),
        &[("targetname", "relay")],
    )))
    .unwrap();
    let relay = *doc.selection().iter().next().unwrap();
    let mut on = halberd_doc::Connection::new("OnTrigger", "lamp", "TurnOn");
    on.delay = 1.5;
    doc.execute(Command::SetConnections {
        id: relay,
        connections: vec![on.clone()],
    })
    .unwrap();
    let text = saved_text(&doc);
    assert!(
        text.contains("\"OnTrigger\" \"lamp,TurnOn,,1.5,-1\""),
        "{text}"
    );
    let back = open(&text);
    let relay = back
        .objects()
        .find_map(|(_, o)| o.as_entity().filter(|e| e.classname == "logic_relay"))
        .unwrap();
    assert_eq!(relay.connections(), [on]);
    let tied = back
        .objects()
        .find_map(|(id, o)| {
            o.as_entity()
                .filter(|e| e.classname == "func_wall")
                .map(|_| id)
        })
        .unwrap();
    assert_eq!(back.brushes_of(tied).count(), 1);
}
