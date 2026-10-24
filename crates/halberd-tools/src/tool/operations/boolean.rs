//! Union, Subtract and Intersect: boolean operations on brushes.
//!
//! Brushes must be convex, so results that are not are made of several
//! convex brushes, as Hammer's Carve does. Faces keep their look: a piece's
//! face from a brush keeps that brush's face data (its saved id only if the
//! face is unchanged and on the brush's first piece), and a face made by a
//! cutter copies the look of that cutter's face.

use super::{ShapeOpOutcome, brush_object, counted};
use crate::elements::editable_brushes;
use halberd_doc::{AddedBrush, BrushObject, Command, Document, FaceInfo, Object, ObjectId};
use halberd_geom::{Brush, Subtracted};

/// A convex piece being cut, with the face data of each of its faces and,
/// for faces still on the original brush, which face that was.
#[derive(Debug, Clone)]
struct Piece {
    brush: Brush,
    infos: Vec<FaceInfo>,
    original: Vec<Option<usize>>,
}

impl Piece {
    fn whole(object: &BrushObject) -> Self {
        Self {
            brush: object.brush().clone(),
            infos: object.faces().to_vec(),
            original: (0..object.brush().faces().len()).map(Some).collect(),
        }
    }

    /// A new piece from `brush`, whose face sources are this piece's faces
    /// or, one past them, faces of `cutter`.
    fn derive(&self, brush: Brush, cutter: &BrushObject) -> Self {
        let own = self.brush.faces().len();
        let mut infos = Vec::new();
        let mut original = Vec::new();
        for face in brush.faces() {
            let s = face.source();
            if s < own {
                infos.push(self.infos[s].clone());
                original.push(self.original[s]);
            } else {
                let info = cutter.faces().get(s - own).map(FaceInfo::copied);
                infos.push(info.unwrap_or_default());
                original.push(None);
            }
        }
        Self {
            brush,
            infos,
            original,
        }
    }

    /// This piece minus `cutter`: `None` if untouched, else the pieces
    /// left. Adds to `lost` the space dropped as slivers too thin to be
    /// brushes (what is neither left nor inside the cutter).
    fn minus(&self, cutter: &BrushObject, lost: &mut f32) -> Option<Vec<Piece>> {
        match self.brush.subtract(cutter.brush()) {
            Subtracted::Untouched => None,
            Subtracted::Gone => Some(Vec::new()),
            Subtracted::Pieces(parts) => {
                let inside = self
                    .brush
                    .intersection(cutter.brush())
                    .map_or(0.0, |b| b.volume());
                let left: f32 = parts.iter().map(Brush::volume).sum();
                *lost += (self.brush.volume() - left - inside).max(0.0);
                Some(parts.into_iter().map(|b| self.derive(b, cutter)).collect())
            }
        }
    }

    /// As a brush object replacing `object` (keeping ids of unchanged faces)
    /// or, `as_copy`, as a new brush beside it.
    fn into_object(self, object: &BrushObject, as_copy: bool) -> BrushObject {
        let old = object.brush().faces();
        let infos = self
            .brush
            .faces()
            .iter()
            .zip(self.infos)
            .zip(self.original)
            .map(|((face, info), original)| {
                let same = original.and_then(|i| old.get(i)).is_some_and(|before| {
                    before.vertices().len() == face.vertices().len()
                        && face
                            .vertices()
                            .iter()
                            .all(|v| before.vertices().iter().any(|w| w.distance(*v) < 0.01))
                });
                if same && !as_copy {
                    info
                } else {
                    info.copied()
                }
            })
            .collect();
        let data = if as_copy {
            object.copied_data()
        } else {
            object.file_data.clone()
        };
        BrushObject::with_faces(self.brush, infos, data)
    }
}

/// Cuts `object` by each of `cutters` in turn. `None` if none touches it.
/// Space dropped as slivers is added to `lost`.
fn carve(object: &BrushObject, cutters: &[&BrushObject], lost: &mut f32) -> Option<Vec<Piece>> {
    let bounds = object.brush().bounds();
    let near = |c: &&&BrushObject| {
        let b = c.brush().bounds();
        (0..3).all(|i| bounds.min[i] < b.max[i] && b.min[i] < bounds.max[i])
    };
    if !cutters.iter().any(|c| near(&c)) {
        return None;
    }
    let mut pieces = vec![Piece::whole(object)];
    let mut touched = false;
    for cutter in cutters {
        let mut next = Vec::new();
        for piece in pieces {
            match piece.minus(cutter, lost) {
                None => next.push(piece),
                Some(parts) => {
                    touched = true;
                    next.extend(parts);
                }
            }
        }
        pieces = next;
    }
    touched.then_some(pieces)
}

/// What became of brushes cut into pieces, as the parts of a shape edit:
/// each brush's first piece replaces it, the rest join its entity, and a
/// brush with no pieces left goes.
#[derive(Default)]
struct Parts {
    changed: Vec<(ObjectId, BrushObject)>,
    added: Vec<AddedBrush>,
    removed: Vec<ObjectId>,
    /// Space dropped as slivers too thin to be brushes.
    lost: f32,
}

impl Parts {
    fn take(&mut self, id: ObjectId, object: &BrushObject, pieces: Vec<Piece>) {
        let mut pieces = pieces.into_iter();
        match pieces.next() {
            None => self.removed.push(id),
            Some(first) => {
                self.changed.push((id, first.into_object(object, false)));
                for piece in pieces {
                    self.added
                        .push((piece.into_object(object, true), object.entity()));
                }
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.removed.is_empty()
    }

    /// A note for the Console if slivers were dropped.
    fn sliver_note(&self) -> Option<String> {
        (self.lost >= 1.0).then(|| {
            format!(
                "About {:.0} cubic units were left out: slivers thinner than a unit can't be \
                 brushes. If that leaves a gap, snap the cutting brush to the grid and try again.",
                self.lost
            )
        })
    }

    fn into_command(self, label: String, select_added: bool) -> Command {
        Command::ShapeEdit {
            label,
            changed: self.changed,
            added: self.added,
            removed: self.removed,
            select_added,
        }
    }
}

/// The selected brushes, with their objects.
fn selected(doc: &Document) -> Vec<(ObjectId, &BrushObject)> {
    editable_brushes(doc)
        .iter()
        .filter_map(|(id, _)| Some((*id, brush_object(doc, *id)?)))
        .collect()
}

/// Subtract: the selected brushes cut their space out of every other brush
/// they overlap. They stay, selected, so they can be moved or deleted.
pub(super) fn subtract(doc: &Document) -> Result<ShapeOpOutcome, String> {
    let cutters = selected(doc);
    let cutting: Vec<&BrushObject> = cutters.iter().map(|(_, b)| *b).collect();
    let mut parts = Parts::default();
    let mut cut = 0;
    for (id, object) in doc.objects() {
        let Object::Brush(object) = object else {
            continue;
        };
        if cutters.iter().any(|(c, _)| *c == id) {
            continue;
        }
        if let Some(pieces) = carve(object, &cutting, &mut parts.lost) {
            cut += 1;
            parts.take(id, object, pieces);
        }
    }
    if parts.is_empty() {
        return Err("Nothing was cut: the selected brushes don't overlap any other brush.".into());
    }
    let label = format!("Subtract from {}", counted(cut, "one brush", "brushes"));
    let slivers = parts.sliver_note();
    // The cut pieces don't join the selection (not even on redo), so
    // Delete removes only the cutters.
    let mut outcome = ShapeOpOutcome::new(parts.into_command(label, false));
    outcome.select = super::SelectAfter::Same;
    let cutters_note = format!(
        "Cut out of {}. The cutting {} stayed: press Delete to remove {}.",
        counted(cut, "one brush", "brushes"),
        if cutters.len() == 1 {
            "brush"
        } else {
            "brushes"
        },
        if cutters.len() == 1 { "it" } else { "them" },
    );
    outcome.note = Some(match slivers {
        Some(s) => format!("{cutters_note} {s}"),
        None => cutters_note,
    });
    Ok(outcome)
}

/// Union: the selected brushes fill the space they cover together without
/// overlapping: each is cut by those before it. If that space has no dents,
/// it becomes one brush, as Join would make it.
pub(super) fn union(doc: &Document) -> Result<ShapeOpOutcome, String> {
    let brushes = selected(doc);
    let mut parts = Parts::default();
    let mut kept_volume = 0.0;
    for (i, (id, object)) in brushes.iter().enumerate() {
        let earlier: Vec<&BrushObject> = brushes[..i].iter().map(|(_, b)| *b).collect();
        match carve(object, &earlier, &mut parts.lost) {
            None => kept_volume += object.brush().volume(),
            Some(pieces) => {
                kept_volume += pieces.iter().map(|p| p.brush.volume()).sum::<f32>();
                parts.take(*id, object, pieces);
            }
        }
    }
    // Space without dents: one brush.
    let points: Vec<glam::Vec3> = brushes
        .iter()
        .flat_map(|(_, b)| b.brush().points())
        .collect();
    // Join would fill no gaps: its hull is no bigger than the space the
    // brushes cover, and has no side they don't have.
    if let Ok(hull) = Brush::hull(&points)
        && hull.volume() <= kept_volume + 1.0
        && let Ok(mut outcome) = super::join(doc)
        && outcome.note.is_none()
    {
        if let Command::ShapeEdit { label, .. } = &mut outcome.command {
            *label = format!("Union of {} brushes", brushes.len());
        }
        return Ok(outcome);
    }
    if parts.is_empty() {
        return Err("Nothing changed: the selected brushes don't overlap.".into());
    }
    let label = format!("Union of {} brushes", brushes.len());
    let note = parts.sliver_note();
    let mut outcome = ShapeOpOutcome::new(parts.into_command(label, true));
    outcome.note = note;
    Ok(outcome)
}

/// Intersect: only the space all the selected brushes share is kept, as
/// one brush (a world brush if any of them is one).
pub(super) fn intersect(doc: &Document) -> Result<ShapeOpOutcome, String> {
    let brushes = selected(doc);
    let keep_at = brushes
        .iter()
        .position(|(_, b)| b.entity().is_none())
        .unwrap_or(0);
    let (keep, kept) = *brushes.get(keep_at).ok_or("Nothing to intersect.")?;
    let mut common = Piece::whole(kept);
    for (id, other) in &brushes {
        if *id == keep {
            continue;
        }
        let shared = common
            .brush
            .intersection(other.brush())
            .ok_or("Nothing was kept: the selected brushes don't all overlap.")?;
        common = common.derive(shared, other);
    }
    let removed = brushes
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| *id != keep)
        .collect();
    let command = Command::ShapeEdit {
        label: format!("Intersect {} brushes", brushes.len()),
        changed: vec![(keep, common.into_object(kept, false))],
        added: Vec::new(),
        removed,
        select_added: true,
    };
    Ok(ShapeOpOutcome::new(command))
}

#[cfg(test)]
mod tests;
