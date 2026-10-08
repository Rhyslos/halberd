//! Document → VMF.

use crate::planes::{plane_from_points, points_of_face, same_plane};
use halberd_doc::{BrushObject, Document, Object};
use halberd_geom::Face;
use halberd_vmf::{Block, Entry, Vmf, format_plane, id_of, parse_plane};

/// Turns a document into a VMF, ready to write.
///
/// Everything kept from the file the map came from goes back in its place;
/// a map that was not changed comes out exactly as it was read. Brushes,
/// faces and entities without an id (new ones) get fresh ids above any in
/// use. New maps get the blocks Hammer expects (`versioninfo`, `world`
/// settings, `cameras`, `cordons`).
pub fn vmf_from_document(doc: &Document) -> Vmf {
    let data = doc.file_data();
    let mut ids = IdSource::new(doc);
    let mut entries = if data.header.is_empty() && data.world_header.is_empty() {
        default_header()
    } else {
        data.header.clone()
    };

    let mut world = Block::new("world");
    world.entries = if data.world_header.is_empty() {
        default_world_settings()
    } else {
        data.world_header.clone()
    };
    for (_, object) in doc.objects() {
        if let Object::Brush(brush) = object {
            world.push_block(solid_block(brush, &mut ids));
        }
    }
    world.entries.extend(data.world_trailer.iter().cloned());
    entries.push(Entry::Block(world));

    for (_, object) in doc.objects() {
        if let Object::Entity(entity) = object {
            let mut block = Block::new("entity");
            block.entries = entity.file_data.clone();
            if block.get("id").is_none() {
                block
                    .entries
                    .insert(0, Entry::Pair("id".into(), ids.next().to_string()));
            }
            let solids = entity
                .solids
                .iter()
                .map(|s| Entry::Block(solid_block(s, &mut ids)))
                .collect();
            insert_before_editor(&mut block.entries, solids);
            entries.push(Entry::Block(block));
        }
    }
    if data.footer.is_empty() && data.header.is_empty() && data.world_header.is_empty() {
        entries.extend(default_footer());
    } else {
        entries.extend(data.footer.iter().cloned());
    }
    Vmf {
        entries,
        line_ending: data.line_ending,
    }
}

/// A brush as a `solid` block: its kept entries, with its sides placed
/// before the `editor` block, as Hammer writes them.
fn solid_block(brush: &BrushObject, ids: &mut IdSource) -> Block {
    let mut block = Block::new("solid");
    block.entries = brush.file_data.clone();
    if block.get("id").is_none() {
        block
            .entries
            .insert(0, Entry::Pair("id".into(), ids.next().to_string()));
    }
    let sides = brush
        .brush()
        .faces()
        .iter()
        .zip(brush.faces())
        .map(|(face, info)| Entry::Block(side_block(face, info, ids)))
        .collect();
    insert_before_editor(&mut block.entries, sides);
    block
}

/// A face as a `side` block. A face read from a file keeps all its entries
/// in order, with its plane (unless the face moved) and material updated;
/// a new face gets the entries Hammer writes, aligned to the world.
fn side_block(face: &Face, info: &halberd_doc::FaceInfo, ids: &mut IdSource) -> Block {
    // Exactly the plane it was read with: untouched, so its text is kept
    // even far from the middle of the map, where rounding is coarse. Face
    // data made some other way is compared within rounding.
    let unchanged = match info.loaded_plane {
        Some(loaded) => loaded == face.plane(),
        None => info
            .file_plane
            .as_deref()
            .and_then(|text| plane_from_points(parse_plane(text).ok()?))
            .is_some_and(|old| same_plane(&old, &face.plane())),
    };
    let plane = match (&info.file_plane, unchanged) {
        (Some(text), true) => text.clone(),
        _ => format_plane(points_of_face(face)),
    };
    let mut side = Block::new("side");
    if info.file_data.is_empty() {
        let (u, v) = world_axes(face);
        side.push_pair("id", ids.next().to_string());
        side.push_pair("plane", plane);
        side.push_pair("material", info.material.clone());
        side.push_pair("uaxis", format!("[{u} 0] 0.25"));
        side.push_pair("vaxis", format!("[{v} 0] 0.25"));
        side.push_pair("rotation", "0");
        side.push_pair("lightmapscale", "16");
        side.push_pair("smoothing_groups", "0");
        return side;
    }
    side.entries = info.file_data.clone();
    for entry in &mut side.entries {
        if let Entry::Pair(key, value) = entry {
            if key.eq_ignore_ascii_case("plane") {
                value.clone_from(&plane);
            } else if key.eq_ignore_ascii_case("material") {
                value.clone_from(&info.material);
            }
        }
    }
    if side.get("id").is_none() {
        side.entries
            .insert(0, Entry::Pair("id".into(), ids.next().to_string()));
    }
    side
}

/// Hammer's world-aligned texture axes for a face, as `x y z` text:
/// textures run along the two world axes the face is most nearly flat to.
fn world_axes(face: &Face) -> (&'static str, &'static str) {
    let n = face.plane().normal.abs();
    if n.z >= n.x && n.z >= n.y {
        ("1 0 0", "0 -1 0")
    } else if n.x >= n.y {
        ("0 1 0", "0 0 -1")
    } else {
        ("1 0 0", "0 0 -1")
    }
}

/// Puts `blocks` just before the first `editor` block in `entries`, or at
/// the end if there is none.
fn insert_before_editor(entries: &mut Vec<Entry>, blocks: Vec<Entry>) {
    let at = entries
        .iter()
        .position(|e| matches!(e, Entry::Block(b) if b.name.eq_ignore_ascii_case("editor")))
        .unwrap_or(entries.len());
    entries.splice(at..at, blocks);
}

/// Hands out ids above every id already in the map, so new brushes, faces
/// and entities never clash with kept ones (overlays and decals refer to
/// faces by id).
struct IdSource {
    next: u64,
}

impl IdSource {
    fn new(doc: &Document) -> Self {
        let mut highest = 1; // The world is id 1.
        // Every id anywhere in what is kept: objects' own entries and every
        // block inside them (hidden brushes, `hidden` wrappers), and the
        // blocks around the world (hidden entities live in the footer).
        // Ids beyond what Hammer can store are ignored rather than followed.
        let mut see = |entries: &[Entry]| {
            for entry in entries {
                match entry {
                    Entry::Pair(k, v) if k.eq_ignore_ascii_case("id") => {
                        if let Ok(id) = v.trim().parse::<u64>() {
                            highest = highest.max(usable(id));
                        }
                    }
                    Entry::Block(b) => walk_ids(b, &mut |id| highest = highest.max(usable(id))),
                    Entry::Pair(..) => {}
                }
            }
        };
        let see_brush = |b: &BrushObject, see: &mut dyn FnMut(&[Entry])| {
            see(&b.file_data);
            for face in b.faces() {
                see(&face.file_data);
            }
        };
        for (_, object) in doc.objects() {
            match object {
                Object::Brush(b) => see_brush(b, &mut see),
                Object::Entity(e) => {
                    see(&e.file_data);
                    for s in &e.solids {
                        see_brush(s, &mut see);
                    }
                }
            }
        }
        let data = doc.file_data();
        for part in [
            &data.header,
            &data.world_header,
            &data.world_trailer,
            &data.footer,
        ] {
            see(part);
        }
        Self {
            next: highest.saturating_add(1),
        }
    }

    fn next(&mut self) -> u64 {
        let id = self.next;
        self.next += 1;
        id
    }
}

/// The highest id Hammer can store; larger ones are not real ids.
const MAX_ID: u64 = i32::MAX as u64;

/// `id`, or 0 if it is too large to be a real id.
fn usable(id: u64) -> u64 {
    if id > MAX_ID { 0 } else { id }
}

fn walk_ids(block: &Block, found: &mut dyn FnMut(u64)) {
    if let Some(id) = id_of(block) {
        found(id);
    }
    for entry in &block.entries {
        if let Entry::Block(b) = entry {
            walk_ids(b, found);
        }
    }
}

fn pairs(list: &[(&str, &str)]) -> Vec<Entry> {
    list.iter()
        .map(|(k, v)| Entry::Pair((*k).into(), (*v).into()))
        .collect()
}

fn default_header() -> Vec<Entry> {
    let mut version = Block::new("versioninfo");
    version.entries = pairs(&[
        ("editorversion", "400"),
        ("editorbuild", "8864"),
        ("mapversion", "1"),
        ("formatversion", "100"),
        ("prefab", "0"),
    ]);
    let mut view = Block::new("viewsettings");
    view.entries = pairs(&[
        ("bSnapToGrid", "1"),
        ("bShowGrid", "1"),
        ("bShowLogicalGrid", "0"),
        ("nGridSpacing", "16"),
        ("bShow3DGrid", "0"),
    ]);
    vec![
        Entry::Block(version),
        Entry::Block(Block::new("visgroups")),
        Entry::Block(view),
    ]
}

fn default_world_settings() -> Vec<Entry> {
    pairs(&[
        ("id", "1"),
        ("mapversion", "1"),
        ("classname", "worldspawn"),
        ("detailmaterial", "detail/detailsprites"),
        ("detailvbsp", "detail.vbsp"),
        ("maxpropscreenwidth", "-1"),
        ("skyname", "sky_day01_01"),
    ])
}

fn default_footer() -> Vec<Entry> {
    let mut cameras = Block::new("cameras");
    cameras.push_pair("activecamera", "-1");
    let mut cordons = Block::new("cordons");
    cordons.push_pair("active", "0");
    vec![Entry::Block(cameras), Entry::Block(cordons)]
}
