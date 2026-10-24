//! Floor-plan shapes in map files (see decision 0019).
//!
//! A shape is saved as its convex pieces, ordinary world brushes, each
//! marked in its `editor` block with the shape's number and outline. On
//! opening, a shape's pieces become the shape again only if nothing about
//! them was changed elsewhere: the same pieces (every face in place), one
//! material, Halberd's own texture alignment and editor settings. Anything
//! else (a piece moved, retextured, deleted, copied, put in a visgroup)
//! leaves them ordinary brushes, unmarked, so no edit made in another
//! editor is lost and old marks never get mixed up with new shapes.

use glam::Vec2;
use halberd_doc::{BrushObject, FaceInfo, MapObject, PieceIds, ShapeObject};
use halberd_vmf::{Block, Entry};
use std::collections::BTreeMap;

/// Key in a solid's `editor` block naming the shape it is a piece of (a
/// number, the same for every piece of one shape).
pub(crate) const SHAPE_KEY: &str = "halberd_shape";
/// Key holding the shape's outline, as `x y` pairs, then its bottom and
/// top heights after a `|`.
pub(crate) const OUTLINE_KEY: &str = "halberd_outline";
/// The editor colour of shape pieces (orange).
const COLOR: &str = "220 140 60";
/// Most outline corners worked through when opening a map: rebuilding a
/// shape takes time, and a damaged or hostile file must not hang Halberd.
/// Shapes past this many open as ordinary brushes.
const MAX_CORNERS_TO_REGROUP: usize = 100_000;

/// The `solid` blocks a shape is saved as: `solid` turns each piece into a
/// block (giving it fresh ids), then the ids the pieces were read with,
/// if any, are put back.
pub(crate) fn shape_blocks(
    shape: &ShapeObject,
    number: usize,
    mut solid: impl FnMut(&BrushObject) -> Block,
) -> Vec<Block> {
    let outline: Vec<String> = shape
        .outline()
        .iter()
        .map(|p| format!("{} {}", p.x, p.y))
        .collect();
    let mut editor = Block::new("editor");
    editor.push_pair("color", COLOR);
    editor.push_pair("visgroupshown", "1");
    editor.push_pair("visgroupautoshown", "1");
    editor.push_pair(SHAPE_KEY, number.to_string());
    editor.push_pair(
        OUTLINE_KEY,
        format!("{} | {} {}", outline.join(" "), shape.bottom(), shape.top()),
    );
    let look = FaceInfo {
        material: shape.look.material.clone(),
        ..FaceInfo::default()
    };
    shape
        .pieces()
        .iter()
        .enumerate()
        .map(|(i, piece)| {
            let faces = vec![look.clone(); piece.faces().len()];
            let object =
                BrushObject::with_faces(piece.clone(), faces, vec![Entry::Block(editor.clone())]);
            let mut block = solid(&object);
            if let Some(ids) = shape.file_ids().get(i) {
                set_ids(&mut block, ids);
            }
            block
        })
        .collect()
}

/// Puts `ids` into a solid block: its own id and its sides' in order.
fn set_ids(block: &mut Block, ids: &PieceIds) {
    let mut sides = ids.sides.iter();
    for entry in &mut block.entries {
        match entry {
            Entry::Pair(key, value) if key.eq_ignore_ascii_case("id") => {
                *value = ids.solid.to_string();
            }
            Entry::Block(side) if side.name.eq_ignore_ascii_case("side") => {
                if let Some(id) = sides.next() {
                    for pair in &mut side.entries {
                        if let Entry::Pair(key, value) = pair
                            && key.eq_ignore_ascii_case("id")
                        {
                            *value = id.to_string();
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// World brushes Halberd saved as shape pieces, put back together as
/// their shapes where nothing was changed (see the module notes). Returns
/// how many marked groups stayed brushes.
pub(crate) fn regroup(objects: Vec<MapObject>) -> (Vec<MapObject>, usize) {
    let marks: Vec<Option<(String, String)>> = objects.iter().map(mark).collect();
    let mut groups: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, m) in marks.iter().enumerate() {
        if let Some((number, _)) = m {
            groups.entry(number.as_str()).or_default().push(i);
        }
    }
    let mut budget = MAX_CORNERS_TO_REGROUP;
    // The shape standing in for each group's first piece.
    let mut rebuilt: BTreeMap<usize, Option<ShapeObject>> = BTreeMap::new();
    let mut failed = 0;
    let mut unmark = std::collections::BTreeSet::new();
    for members in groups.values() {
        let pieces: Vec<&BrushObject> = members
            .iter()
            .filter_map(|&i| match &objects[i] {
                MapObject::Brush(b) => Some(b),
                _ => None,
            })
            .collect();
        let outline = marks[members[0]].as_ref().map(|(_, o)| o.as_str());
        let corners = outline.map_or(0, |o| o.split_whitespace().count() / 2);
        let shape = if corners <= budget {
            budget -= corners;
            outline.and_then(|o| rebuild(o, &pieces))
        } else {
            None
        };
        match shape {
            Some(shape) => {
                rebuilt.insert(members[0], Some(shape));
                for &i in &members[1..] {
                    rebuilt.insert(i, None);
                }
            }
            None => {
                failed += 1;
                unmark.extend(members.iter().copied());
            }
        }
    }
    let mut out = Vec::with_capacity(objects.len());
    for (i, object) in objects.into_iter().enumerate() {
        match rebuilt.remove(&i) {
            Some(Some(shape)) => out.push(MapObject::Shape(shape)),
            Some(None) => {}
            None if unmark.contains(&i) => out.push(unmarked(object)),
            None => out.push(object),
        }
    }
    (out, failed)
}

/// A world brush's shape number and outline, if it is marked as a piece.
fn mark(object: &MapObject) -> Option<(String, String)> {
    let MapObject::Brush(brush) = object else {
        return None;
    };
    editor_of(brush).and_then(|editor| {
        Some((
            editor.get(SHAPE_KEY)?.to_string(),
            editor.get(OUTLINE_KEY)?.to_string(),
        ))
    })
}

fn editor_of(brush: &BrushObject) -> Option<&Block> {
    brush.file_data.iter().find_map(|entry| match entry {
        Entry::Block(b) if b.name.eq_ignore_ascii_case("editor") => Some(b),
        _ => None,
    })
}

/// The brush without Halberd's shape marks.
fn unmarked(object: MapObject) -> MapObject {
    let MapObject::Brush(mut brush) = object else {
        return object;
    };
    for entry in &mut brush.file_data {
        if let Entry::Block(editor) = entry
            && editor.name.eq_ignore_ascii_case("editor")
        {
            editor.entries.retain(|e| {
                !matches!(e, Entry::Pair(k, _)
                    if k.eq_ignore_ascii_case(SHAPE_KEY) || k.eq_ignore_ascii_case(OUTLINE_KEY))
            });
        }
    }
    MapObject::Brush(brush)
}

/// The shape `pieces` were saved from, with their ids, if they are still
/// exactly its pieces as Halberd wrote them.
fn rebuild(outline: &str, pieces: &[&BrushObject]) -> Option<ShapeObject> {
    let material = pieces.first()?.faces().first()?.material.clone();
    if !pieces.iter().all(|p| as_written(p, &material)) {
        return None;
    }
    let look = FaceInfo {
        material,
        ..FaceInfo::default()
    };
    let shape = shape_from_outline(outline, look)?;
    if shape.pieces().len() != pieces.len() {
        return None;
    }
    // Each piece of the shape must be one of the brushes, face for face.
    let mut used = vec![false; pieces.len()];
    let mut ids = Vec::with_capacity(pieces.len());
    for piece in shape.pieces() {
        let (j, sides) = pieces.iter().enumerate().find_map(|(j, found)| {
            if used[j] {
                return None;
            }
            Some((j, same_faces(piece, found)?))
        })?;
        used[j] = true;
        ids.push(PieceIds {
            solid: id_in(&pieces[j].file_data)?,
            sides,
        });
    }
    Some(shape.with_file_ids(ids))
}

/// If `found` has exactly the faces of `piece`, the side ids of `found` in
/// the order of `piece`'s faces.
fn same_faces(piece: &halberd_geom::Brush, found: &BrushObject) -> Option<Vec<u64>> {
    if found.brush().faces().len() != piece.faces().len() {
        return None;
    }
    piece
        .faces()
        .iter()
        .map(|face| {
            let plane = face.plane();
            let k = found.brush().faces().iter().position(|f| {
                let other = f.plane();
                other.normal.dot(plane.normal) > 0.99999
                    && (other.distance - plane.distance).abs() < 0.01
            })?;
            id_in(&found.faces().get(k)?.file_data)
        })
        .collect()
}

/// True if a piece is as Halberd writes one: only an id and Halberd's
/// editor block, and on every side `material`, world-aligned textures and
/// the standard settings.
fn as_written(piece: &BrushObject, material: &str) -> bool {
    if piece.entity().is_some() {
        return false;
    }
    let solid_ok = piece.file_data.iter().all(|entry| match entry {
        Entry::Pair(key, _) => key.eq_ignore_ascii_case("id"),
        Entry::Block(editor) if editor.name.eq_ignore_ascii_case("editor") => {
            editor.entries.iter().all(|e| match e {
                Entry::Pair(k, v) => match k.to_ascii_lowercase().as_str() {
                    "color" => v == COLOR,
                    "visgroupshown" | "visgroupautoshown" => v == "1",
                    k => k == SHAPE_KEY || k == OUTLINE_KEY,
                },
                Entry::Block(_) => false,
            })
        }
        Entry::Block(_) => false,
    });
    solid_ok
        && piece
            .brush()
            .faces()
            .iter()
            .zip(piece.faces())
            .all(|(face, info)| {
                let axes = halberd_doc::TextureAxes::world_aligned(face.plane().normal);
                let (u, v) = (axes.u.to_hammer(), axes.v.to_hammer());
                info.material == material
                    && info.file_data.iter().all(|e| match e {
                        Entry::Pair(k, value) => match k.to_ascii_lowercase().as_str() {
                            "id" | "plane" | "material" => true,
                            "uaxis" => *value == u,
                            "vaxis" => *value == v,
                            "rotation" | "smoothing_groups" => value == "0",
                            "lightmapscale" => value == "16",
                            _ => false,
                        },
                        Entry::Block(_) => false,
                    })
            })
}

/// The `id` among `entries`, if there is a usable one.
fn id_in(entries: &[Entry]) -> Option<u64> {
    entries.iter().find_map(|e| match e {
        Entry::Pair(k, v) if k.eq_ignore_ascii_case("id") => v.trim().parse().ok(),
        _ => None,
    })
}

/// A floor-plan shape from its saved outline (`x y` pairs, `|`, bottom and
/// top), or `None` if the text is damaged.
fn shape_from_outline(text: &str, look: FaceInfo) -> Option<ShapeObject> {
    let (corners, heights) = text.split_once('|')?;
    let numbers: Vec<f32> = corners
        .split_whitespace()
        .map(|n| n.parse().ok())
        .collect::<Option<_>>()?;
    if !numbers.len().is_multiple_of(2) {
        return None;
    }
    let outline: Vec<Vec2> = numbers.chunks(2).map(|c| Vec2::new(c[0], c[1])).collect();
    let heights: Vec<f32> = heights
        .split_whitespace()
        .map(|n| n.parse().ok())
        .collect::<Option<_>>()?;
    let [bottom, top] = heights[..] else {
        return None;
    };
    ShapeObject::new(&outline, bottom, top, look).ok()
}
