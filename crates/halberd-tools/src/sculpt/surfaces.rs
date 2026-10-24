//! The map's displacement faces, read once and kept: each frame of the
//! Sculpt tool looks at them, and reading a displacement from its text
//! costs far more than using it. A brush is read again only when it
//! changed.

use glam::Vec3;
use halberd_doc::{BrushObject, Displacement, Document, ObjectId, ordered_corners};
use std::collections::HashMap;

/// Most displacement faces looked at (far more than a map has).
const MAX_DISP_FACES: usize = 20_000;

/// One displacement face in the map, ready to use.
#[derive(Debug, Clone)]
pub(crate) struct DispFace {
    /// Its brush and the face's place in it.
    pub(crate) brush: ObjectId,
    pub(crate) face: usize,
    pub(crate) disp: Displacement,
    /// The face's outward direction.
    pub(crate) normal: Vec3,
    /// Where each point is now, and where it would be on the flat face.
    pub(crate) points: Vec<Vec3>,
    pub(crate) base: Vec<Vec3>,
    /// The box around its points.
    pub(crate) min: Vec3,
    pub(crate) max: Vec3,
}

impl DispFace {
    fn read(brush: ObjectId, object: &BrushObject, face: usize) -> Option<Self> {
        let shape = object.brush().faces().get(face)?;
        let disp = object.faces().get(face)?.displacement()?;
        let normal = shape.plane().normal;
        let corners = ordered_corners(shape.vertices(), normal, disp.start)?;
        let points = disp.points(&corners)?;
        let base = disp.base_points(&corners)?;
        let min = points.iter().copied().fold(Vec3::INFINITY, Vec3::min);
        let max = points.iter().copied().fold(Vec3::NEG_INFINITY, Vec3::max);
        Some(Self {
            brush,
            face,
            disp,
            normal,
            points,
            base,
            min,
            max,
        })
    }

    /// True if a ball at `center` reaching `radius` may touch it.
    pub(crate) fn near(&self, center: Vec3, radius: f32) -> bool {
        let closest = center.clamp(self.min, self.max);
        closest.distance_squared(center) <= radius * radius
    }
}

/// True if any face of `object` has a displacement block.
fn has_displacement(object: &BrushObject) -> bool {
    object
        .faces()
        .iter()
        .any(halberd_doc::FaceInfo::has_dispinfo)
}

/// The displacement faces of the document they were read from.
#[derive(Debug, Clone, Default)]
pub(crate) struct DispCache {
    key: Option<(u64, u64)>,
    brushes: Vec<(ObjectId, BrushObject, Vec<DispFace>)>,
}

impl DispCache {
    /// Brings the faces up to date with `doc`.
    pub(crate) fn update(&mut self, doc: &Document) {
        let key = (doc.instance(), doc.revision());
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        let mut old: HashMap<ObjectId, (BrushObject, Vec<DispFace>)> =
            std::mem::take(&mut self.brushes)
                .into_iter()
                .map(|(id, object, faces)| (id, (object, faces)))
                .collect();
        let mut count = 0;
        for (id, object) in doc.objects() {
            let Some(brush) = object.as_brush().filter(|b| has_displacement(b)) else {
                continue;
            };
            let faces = match old.remove(&id) {
                Some((was, faces)) if was == *brush => faces,
                _ => (0..brush.faces().len())
                    .filter_map(|k| DispFace::read(id, brush, k))
                    .collect(),
            };
            count += faces.len();
            self.brushes.push((id, brush.clone(), faces));
            if count >= MAX_DISP_FACES {
                break;
            }
        }
    }

    /// Every displacement face.
    pub(crate) fn faces(&self) -> impl Iterator<Item = &DispFace> {
        self.brushes.iter().flat_map(|(_, _, faces)| faces)
    }

    /// True if brush `id` has displacements.
    pub(crate) fn has(&self, id: ObjectId) -> bool {
        self.brushes.iter().any(|(b, _, _)| *b == id)
    }
}
