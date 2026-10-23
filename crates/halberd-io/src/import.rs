//! VMF → document.

use crate::planes::plane_from_points;

/// Name of the placeholder kept in an entity's file data where one of its
/// shown brushes was, so saving writes each brush back in its place. Not a
/// name a map file can contain (it has spaces and is never written).
pub(crate) const SOLID_SLOT: &str = "halberd: solid here";
use crate::export::{OUTLINE_KEY, SHAPE_KEY};
use glam::Vec2;
use glam::Vec3;
use halberd_doc::{
    BrushObject, Document, EntityObject, FaceInfo, MapFileData, MapObject, ShapeObject,
};
use halberd_geom::Brush;
use halberd_vmf::{Block, Entry, Vmf, parse_plane, parse_vec3};

/// A map read from a VMF, with notes for the user about anything that
/// could not be shown.
#[derive(Debug)]
pub struct Imported {
    /// The map.
    pub document: Document,
    /// Plain-language notes, such as brushes kept but not shown.
    pub notes: Vec<String>,
}

/// Turns a parsed VMF into a document.
///
/// World brushes and entities (with their brushes) become objects, in
/// file order. A brush whose shape cannot be worked out (damaged
/// planes) is kept unchanged in the file data, so it is written back, but
/// it is not shown; the notes say how many.
pub fn document_from_vmf(vmf: &Vmf) -> Result<Imported, halberd_doc::DocError> {
    let mut data = MapFileData {
        line_ending: vmf.line_ending,
        ..MapFileData::default()
    };
    let mut objects = Vec::new();
    let mut hidden_brushes = 0;
    let mut seen_world = false;
    for entry in &vmf.entries {
        match entry {
            Entry::Block(block) if !seen_world && block.name.eq_ignore_ascii_case("world") => {
                seen_world = true;
                let mut in_solids = false;
                for child in &block.entries {
                    match child {
                        Entry::Block(b) if b.name.eq_ignore_ascii_case("solid") => {
                            in_solids = true;
                            match brush_from_solid(b) {
                                Some(brush) => objects.push(MapObject::Brush(brush)),
                                None => {
                                    hidden_brushes += 1;
                                    data.world_trailer.push(child.clone());
                                }
                            }
                        }
                        _ if in_solids => data.world_trailer.push(child.clone()),
                        _ => data.world_header.push(child.clone()),
                    }
                }
            }
            Entry::Block(block) if block.name.eq_ignore_ascii_case("entity") => {
                let (entity, brushes, unshown) = entity_from_block(block);
                hidden_brushes += unshown;
                objects.push(MapObject::Entity(entity, brushes));
            }
            other if seen_world => data.footer.push(other.clone()),
            other => data.header.push(other.clone()),
        }
    }
    let objects = regroup_shapes(objects);
    let document = Document::from_map(objects, data)?;
    let mut notes = Vec::new();
    if hidden_brushes > 0 {
        notes.push(format!(
            "{hidden_brushes} brush{} had a shape Halberd could not work out (broken, or with extra sides). \
             {} kept unchanged and will be saved, but not shown.",
            if hidden_brushes == 1 { "" } else { "es" },
            if hidden_brushes == 1 {
                "It is"
            } else {
                "They are"
            },
        ));
    }
    Ok(Imported { document, notes })
}

/// World brushes Halberd saved as the pieces of a floor-plan shape, put
/// back together as that shape, where they still are exactly its pieces
/// (if a piece was changed or deleted elsewhere, they stay brushes).
fn regroup_shapes(objects: Vec<MapObject>) -> Vec<MapObject> {
    let mark = |object: &MapObject| -> Option<(String, String)> {
        let MapObject::Brush(brush) = object else {
            return None;
        };
        brush.file_data.iter().find_map(|entry| match entry {
            Entry::Block(editor) if editor.name.eq_ignore_ascii_case("editor") => Some((
                editor.get(SHAPE_KEY)?.to_string(),
                editor.get(OUTLINE_KEY)?.to_string(),
            )),
            _ => None,
        })
    };
    let marks: Vec<Option<(String, String)>> = objects.iter().map(mark).collect();
    let mut rebuilt: std::collections::BTreeMap<String, Option<ShapeObject>> = Default::default();
    for (object, mark) in objects.iter().zip(&marks) {
        let (Some((number, outline)), MapObject::Brush(brush)) = (mark, object) else {
            continue;
        };
        rebuilt.entry(number.clone()).or_insert_with(|| {
            // Only the material: each piece's texture alignment suits its
            // own face, and the pieces get world-aligned faces again when
            // saved.
            let look = FaceInfo {
                material: brush
                    .faces()
                    .first()
                    .map(|f| f.material.clone())
                    .unwrap_or_default(),
                ..FaceInfo::default()
            };
            shape_from_outline(outline, look)
        });
    }
    // A shape is kept only if its pieces are all there, unchanged.
    for (number, shape) in &mut rebuilt {
        let Some(found) = shape else { continue };
        let pieces: Vec<&Brush> = objects
            .iter()
            .zip(&marks)
            .filter(|(_, m)| m.as_ref().is_some_and(|(n, _)| n == number))
            .filter_map(|(o, _)| match o {
                MapObject::Brush(b) => Some(b.brush()),
                _ => None,
            })
            .collect();
        let same = pieces.len() == found.pieces().len()
            && pieces.iter().all(|p| {
                found
                    .pieces()
                    .iter()
                    .any(|q| q.bounds() == p.bounds() && q.faces().len() == p.faces().len())
            });
        if !same {
            *shape = None;
        }
    }
    let valid: std::collections::BTreeSet<String> = rebuilt
        .iter()
        .filter(|(_, s)| s.is_some())
        .map(|(n, _)| n.clone())
        .collect();
    let mut out = Vec::with_capacity(objects.len());
    for (object, mark) in objects.into_iter().zip(marks) {
        match mark {
            // The first piece stands for the shape; the rest go.
            Some((number, _)) if valid.contains(&number) => {
                if let Some(shape) = rebuilt.get_mut(&number).and_then(Option::take) {
                    out.push(MapObject::Shape(shape));
                }
            }
            _ => out.push(object),
        }
    }
    out
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

/// A brush from a `solid` block, or `None` if its planes do not make a
/// valid shape.
fn brush_from_solid(solid: &Block) -> Option<BrushObject> {
    let mut planes = Vec::new();
    let mut faces = Vec::new();
    let mut file_data = Vec::new();
    for entry in &solid.entries {
        match entry {
            Entry::Block(side) if side.name.eq_ignore_ascii_case("side") => {
                let text = side.get("plane")?;
                planes.push(plane_from_points(parse_plane(text).ok()?)?);
                faces.push(FaceInfo {
                    material: side.get("material").unwrap_or_default().to_string(),
                    file_data: side.entries.clone(),
                    file_plane: Some(text.to_string()),
                    loaded_plane: None,
                });
            }
            other => file_data.push(other.clone()),
        }
    }
    let brush = Brush::from_planes(&planes).ok()?;
    // A side that adds nothing to the shape (a repeat, or one that misses
    // the solid) would have no face to hang on, and saving would drop it
    // with its id and data. Such solids are kept untouched instead.
    if brush.faces().len() != planes.len() {
        return None;
    }
    for face in brush.faces() {
        if let Some(info) = faces.get_mut(face.source()) {
            info.loaded_plane = Some(face.plane());
        }
    }
    Some(BrushObject::from_parts(brush, &faces, file_data))
}

/// An entity from an `entity` block, its brushes, and how many of its
/// brushes could not be shown (they stay in its file data, in place).
fn entity_from_block(block: &Block) -> (EntityObject, Vec<BrushObject>, usize) {
    let mut solids = Vec::new();
    let mut file_data = Vec::new();
    let mut unshown = 0;
    for entry in &block.entries {
        match entry {
            Entry::Block(b) if b.name.eq_ignore_ascii_case("solid") => match brush_from_solid(b) {
                Some(brush) => {
                    // Marks the place, so saving puts it back there.
                    file_data.push(Entry::Block(Block::new(SOLID_SLOT)));
                    solids.push(brush);
                }
                None => {
                    unshown += 1;
                    file_data.push(entry.clone());
                }
            },
            other => file_data.push(other.clone()),
        }
    }
    let origin = block
        .get("origin")
        .and_then(parse_vec3)
        .map(|v| Vec3::new(v[0] as f32, v[1] as f32, v[2] as f32));
    let entity = EntityObject {
        classname: block.get("classname").unwrap_or_default().to_string(),
        origin,
        file_data,
    };
    (entity, solids, unshown)
}
