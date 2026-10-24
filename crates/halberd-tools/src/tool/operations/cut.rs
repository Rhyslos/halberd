//! Split and Clip: cutting the selected brushes along the picked parts.

use super::{ShapeOpOutcome, ToolController, brush_object, counted, picked_corners};
use crate::camera::Camera;
use crate::elements::{ElementShape, editable_brushes};
use glam::Vec3;
use halberd_doc::{BrushObject, Command, Document, FaceInfo};
use halberd_geom::{Brush, Plane};

impl ToolController {
    /// Cuts every selected brush the cutting plane passes through: into two
    /// brushes (`keep_both`, Split), or keeping only the bigger part
    /// (Clip). A split brush's second part joins the same entity.
    pub(super) fn cut(
        &self,
        doc: &Document,
        camera: &Camera,
        keep_both: bool,
    ) -> Result<ShapeOpOutcome, String> {
        let plane = self.cut_plane(doc, camera)?;
        let mut changed = Vec::new();
        let mut added = Vec::new();
        let mut too_many = false;
        for (id, brush) in editable_brushes(doc) {
            let (Some(behind), Some(front)) = brush.cut(plane) else {
                // It crosses, but a part would be over the face limit.
                let corners = brush.points();
                let reach = corners.iter().map(|p| plane.signed_distance(*p));
                let (low, high) =
                    reach.fold((f32::MAX, f32::MIN), |(l, h), d| (l.min(d), h.max(d)));
                too_many |=
                    low < -1.0 && high > 1.0 && brush.faces().len() >= halberd_geom::MAX_FACES;
                continue;
            };
            let object = brush_object(doc, id).ok_or("A selected brush is gone.")?;
            let flipped = -plane.normal;
            if keep_both {
                changed.push((id, piece(object, behind, plane.normal, true)));
                let other = piece(object, front, flipped, false);
                added.push((other, object.entity()));
            } else if behind.volume() >= front.volume() {
                changed.push((id, piece(object, behind, plane.normal, true)));
            } else {
                changed.push((id, piece(object, front, flipped, true)));
            }
        }
        if changed.is_empty() && too_many {
            return Err(
                "Nothing was cut: a part would have more than 128 faces, Hammer's limit.".into(),
            );
        }
        if changed.is_empty() {
            return Err("Nothing was cut: the cut doesn't pass through any selected brush.".into());
        }
        let verb = if keep_both { "Split" } else { "Clip" };
        let label = format!("{verb} {}", counted(changed.len(), "brush", "brushes"));
        // Split's new parts join the selection beside the old ones.
        Ok(ShapeOpOutcome::new(Command::ShapeEdit {
            label,
            changed,
            added,
            removed: Vec::new(),
            select_added: true,
        }))
    }

    /// The plane to cut along, from the picked parts:
    ///
    /// - a picked face: its plane;
    /// - three or more corners (corners, or the ends of edges) not in a
    ///   line: the plane through the first three;
    /// - two corners, or one edge: the plane through that line along the
    ///   direction the camera looks (from above, a straight down cut).
    fn cut_plane(&self, doc: &Document, camera: &Camera) -> Result<Plane, String> {
        let items = &self.elements.items;
        if let Some(item) = items
            .iter()
            .find(|i| matches!(i.shape, ElementShape::Face(_)))
        {
            let object = brush_object(doc, item.brush).ok_or("A picked brush is gone.")?;
            let face = item
                .face_index(object.brush())
                .ok_or("The picked face is gone.")?;
            return Ok(object.brush().faces()[face].plane());
        }
        let points = picked_corners(items);
        let [a, rest @ ..] = &points[..] else {
            return Err("Pick where to cut first.".into());
        };
        let triple = rest.iter().enumerate().find_map(|(i, b)| {
            rest[i + 1..].iter().find_map(|c| {
                let normal = (*b - *a).cross(*c - *a);
                (normal.length() > 1e-3).then_some(normal)
            })
        });
        let normal = match (triple, rest.first()) {
            (Some(normal), _) => normal,
            (None, Some(b)) => {
                let line = *b - *a;
                [camera.forward(), Vec3::Z, Vec3::X]
                    .into_iter()
                    .map(|d| line.cross(d))
                    .find(|n| n.length() > 1e-3 * line.length())
                    .ok_or("Pick corners that are apart.")?
            }
            (None, None) => {
                return Err(
                    "One corner isn't enough to cut along: pick two or three corners, an edge \
                     or a face."
                        .into(),
                );
            }
        };
        Plane::new(normal, *a).ok_or_else(|| "Pick corners that are apart.".into())
    }
}

/// One part of a cut brush as a brush object: old faces keep their data
/// (`keep_ids`: a face with exactly its old corners keeps its saved id;
/// otherwise faces get copies), and the cut face (facing `cut_normal`)
/// copies the look of the old face facing most nearly the same way.
fn piece(object: &BrushObject, part: Brush, cut_normal: Vec3, keep_ids: bool) -> BrushObject {
    let old = object.brush().faces();
    let cut_look = old
        .iter()
        .zip(object.faces())
        .max_by(|a, b| {
            a.0.plane()
                .normal
                .dot(cut_normal)
                .total_cmp(&b.0.plane().normal.dot(cut_normal))
        })
        .map(|(_, info)| info.copied())
        .unwrap_or_default();
    let infos: Vec<FaceInfo> = part
        .faces()
        .iter()
        .map(
            |face| match (old.get(face.source()), object.faces().get(face.source())) {
                (Some(before), Some(info)) => {
                    let same = before.vertices().len() == face.vertices().len()
                        && face
                            .vertices()
                            .iter()
                            .all(|v| before.vertices().iter().any(|w| w.distance(*v) < 0.01));
                    if keep_ids && same {
                        info.clone()
                    } else {
                        info.copied()
                    }
                }
                _ => cut_look.clone(),
            },
        )
        .collect();
    let data = if keep_ids {
        object.file_data.clone()
    } else {
        object.copied_data()
    };
    BrushObject::with_faces(part, infos, data)
}
