//! VMF → document.

use crate::planes::plane_from_points;
use glam::Vec3;
use halberd_doc::{BrushObject, Document, EntityObject, FaceInfo, MapFileData, Object};
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
/// World brushes become [`Object::Brush`] and entities [`Object::Entity`],
/// in file order. A brush whose shape cannot be worked out (damaged
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
                                Some(brush) => objects.push(Object::Brush(brush)),
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
                let (entity, unshown) = entity_from_block(block);
                hidden_brushes += unshown;
                objects.push(Object::Entity(entity));
            }
            other if seen_world => data.footer.push(other.clone()),
            other => data.header.push(other.clone()),
        }
    }
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

/// An entity from an `entity` block, and how many of its brushes could not
/// be shown (they stay in its file data, in place).
fn entity_from_block(block: &Block) -> (EntityObject, usize) {
    let mut solids = Vec::new();
    let mut file_data = Vec::new();
    let mut unshown = 0;
    for entry in &block.entries {
        match entry {
            Entry::Block(b) if b.name.eq_ignore_ascii_case("solid") => match brush_from_solid(b) {
                Some(brush) => solids.push(brush),
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
        solids,
        file_data,
    };
    (entity, unshown)
}
