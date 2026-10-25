//! Boolean operations on convex brushes: subtracting one from another and
//! finding their common part.
//!
//! A convex brush minus another is usually not convex, so the result of a
//! subtraction is a list of convex pieces. They are cut one face of the
//! cutter at a time: the part of the brush in front of each face lies
//! outside the cutter and is kept as a piece, and what lies behind goes on
//! to the next face; what is left at the end is inside the cutter and is
//! dropped. Pieces thinner than a unit are dropped too, as Hammer does.
//!
//! Faces keep track of where they came from through their sources: a
//! piece's face from this brush keeps its source, and a face made by the
//! cutter's face `k` gets source `faces + k` (one past this brush's faces,
//! counting on), so its look can be copied from that face of the cutter.

use crate::{Brush, Plane};

/// What subtracting a cutter left of a brush.
#[derive(Debug, Clone, PartialEq)]
pub enum Subtracted {
    /// The cutter does not reach into the brush: it is unchanged.
    Untouched,
    /// The brush lies wholly inside the cutter: nothing is left.
    Gone,
    /// What is left, in convex pieces (at least one).
    Pieces(Vec<Brush>),
}

impl Brush {
    /// This brush minus `cutter`, as convex pieces (see the module notes for
    /// the faces' sources).
    pub fn subtract(&self, cutter: &Brush) -> Subtracted {
        if !self.overlaps(cutter) {
            return Subtracted::Untouched;
        }
        let own: Vec<Plane> = self.faces().iter().map(|f| f.plane()).collect();
        let count = own.len();
        let mut inside = own.clone();
        let mut inside_sources: Vec<usize> = (0..count).collect();
        let mut pieces = Vec::new();
        for (k, face) in cutter.faces().iter().enumerate() {
            let plane = face.plane();
            let flipped = Plane {
                normal: -plane.normal,
                distance: -plane.distance,
            };
            // In front of this face of the cutter: outside it, so kept.
            let mut outside = inside.clone();
            outside.push(flipped);
            let mut outside_sources = inside_sources.clone();
            outside_sources.push(count + k);
            if let Some(piece) = build(&outside, &outside_sources) {
                pieces.push(piece);
            }
            inside.push(plane);
            inside_sources.push(count + k);
        }
        if pieces.is_empty() {
            Subtracted::Gone
        } else {
            Subtracted::Pieces(pieces)
        }
    }

    /// The part this brush and `other` have in common, or `None` if they do
    /// not overlap by at least a unit. Faces from this brush keep their
    /// sources; faces from `other`'s face `k` get source `faces + k`.
    pub fn intersection(&self, other: &Brush) -> Option<Brush> {
        let count = self.faces().len();
        let planes: Vec<Plane> = self
            .faces()
            .iter()
            .chain(other.faces())
            .map(|f| f.plane())
            .collect();
        let sources: Vec<usize> = (0..count + other.faces().len()).collect();
        build(&planes, &sources)
    }

    /// True if the two brushes share some space (more than touching).
    pub fn overlaps(&self, other: &Brush) -> bool {
        let (a, b) = (self.bounds(), other.bounds());
        let apart = (0..3).any(|i| a.max[i] <= b.min[i] + 1e-3 || b.max[i] <= a.min[i] + 1e-3);
        !apart && self.intersection(other).is_some()
    }
}

/// The brush behind `planes`, with each face's source looked up in
/// `sources` (by the plane it came from), or `None` if they make no brush.
fn build(planes: &[Plane], sources: &[usize]) -> Option<Brush> {
    let mut brush = Brush::from_any_planes(planes).ok()?;
    if brush.faces().len() > crate::MAX_FACES {
        return None;
    }
    for face in &mut brush.faces {
        face.source = sources.get(face.source).copied().unwrap_or(face.source);
    }
    Some(brush)
}

#[cfg(test)]
mod tests;
