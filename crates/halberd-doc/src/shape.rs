//! Floor-plan shapes: solids drawn as an outline of any shape (dents
//! included) and stood up to a height, kept whole while editing and split
//! into convex brushes, as Source needs, only when the map is saved.

use crate::FaceInfo;
use glam::{Vec2, Vec3};
use halberd_geom::{Aabb, Brush, GeomError, prism_brushes};

/// A solid standing on a floor plan: its outline (seen from above,
/// counter-clockwise, at least three corners), from height `bottom` up to
/// `top`. Its convex pieces are worked out when it is made, so it can be
/// drawn, picked and saved without working them out again.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeObject {
    outline: Vec<Vec2>,
    bottom: f32,
    top: f32,
    /// The look of every face (material and texture alignment).
    pub look: FaceInfo,
    pieces: Vec<Brush>,
    /// The ids its pieces had in the file it was read from, one per
    /// piece (empty for a new shape), so saving keeps them: overlays and
    /// decals refer to faces by id.
    file_ids: Vec<PieceIds>,
}

/// The ids a shape's piece was saved with: the solid's, and each side's in
/// the order of the piece's faces.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PieceIds {
    /// The solid's id.
    pub solid: u64,
    /// Each face's side id, in face order.
    pub sides: Vec<u64>,
}

impl ShapeObject {
    /// The solid on `outline` from `bottom` to `top`, or why it can't be
    /// made (the outline crosses itself, encloses nothing, or the solid is
    /// too thin somewhere).
    pub fn new(outline: &[Vec2], bottom: f32, top: f32, look: FaceInfo) -> Result<Self, GeomError> {
        let outline = halberd_geom::clean_outline(outline)?;
        let pieces = prism_brushes(&outline, bottom, top)?;
        Ok(Self {
            outline,
            bottom,
            top,
            look,
            pieces,
            file_ids: Vec::new(),
        })
    }

    /// The same shape, remembering the ids its pieces were read with.
    /// Ids that don't fit the pieces (a different number of pieces or
    /// faces) are not kept.
    #[must_use]
    pub fn with_file_ids(mut self, ids: Vec<PieceIds>) -> Self {
        self.file_ids = if fits(&self.pieces, &ids) {
            ids
        } else {
            Vec::new()
        };
        self
    }

    /// The ids its pieces were read with, one per piece, or none.
    pub fn file_ids(&self) -> &[PieceIds] {
        &self.file_ids
    }

    /// A copy of this shape, without the saved ids (a copy is a new object
    /// and must not share them).
    #[must_use]
    pub fn copied(&self) -> Self {
        let mut copy = self.clone();
        copy.file_ids.clear();
        copy
    }

    /// The outline, counter-clockwise seen from above.
    pub fn outline(&self) -> &[Vec2] {
        &self.outline
    }

    /// Height of the floor.
    pub fn bottom(&self) -> f32 {
        self.bottom
    }

    /// Height of the top.
    pub fn top(&self) -> f32 {
        self.top
    }

    /// The convex brushes the solid is saved as.
    pub fn pieces(&self) -> &[Brush] {
        &self.pieces
    }

    /// The box around the solid.
    pub fn bounds(&self) -> Aabb {
        let low = self.outline.iter().copied().fold(Vec2::MAX, Vec2::min);
        let high = self.outline.iter().copied().fold(Vec2::MIN, Vec2::max);
        Aabb::from_corners(low.extend(self.bottom), high.extend(self.top))
    }

    /// Distance along a ray to where it enters the solid, if it does.
    pub fn ray_hit(&self, origin: Vec3, direction: Vec3) -> Option<f32> {
        self.pieces
            .iter()
            .filter_map(|p| p.ray_hit(origin, direction))
            .min_by(f32::total_cmp)
    }

    /// The same solid with every point passed through `f` (which must keep
    /// up up: moves, turns about a vertical line, and stretches), or why
    /// the result can't be made.
    pub fn mapped(&self, f: impl Fn(Vec3) -> Vec3) -> Result<Self, GeomError> {
        let low = f(Vec3::new(0.0, 0.0, self.bottom));
        let high = f(Vec3::new(0.0, 0.0, self.top));
        let outline: Vec<Vec2> = self
            .outline
            .iter()
            .map(|p| f(p.extend(self.bottom)).truncate().round())
            .collect();
        let (bottom, top) = (low.z.min(high.z).round(), low.z.max(high.z).round());
        // Moved pieces keep their ids, as moved brushes do, while the
        // shape still splits the same way.
        Ok(Self::new(&outline, bottom, top, self.look.clone())?
            .with_file_ids(self.file_ids.clone()))
    }
}

/// True if `ids` has one entry per piece with one side id per face.
fn fits(pieces: &[Brush], ids: &[PieceIds]) -> bool {
    ids.len() == pieces.len()
        && pieces
            .iter()
            .zip(ids)
            .all(|(p, i)| p.faces().len() == i.sides.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l_shape() -> ShapeObject {
        let outline = [
            Vec2::new(0.0, 0.0),
            Vec2::new(128.0, 0.0),
            Vec2::new(128.0, 64.0),
            Vec2::new(64.0, 64.0),
            Vec2::new(64.0, 128.0),
            Vec2::new(0.0, 128.0),
        ];
        ShapeObject::new(&outline, 0.0, 96.0, FaceInfo::default()).unwrap()
    }

    #[test]
    fn an_l_shape_is_two_pieces_inside_its_box() {
        let l = l_shape();
        assert_eq!(l.pieces().len(), 2);
        assert_eq!(
            l.bounds(),
            Aabb::from_corners(Vec3::ZERO, Vec3::new(128.0, 128.0, 96.0))
        );
        let down = l.ray_hit(Vec3::new(32.0, 32.0, 500.0), Vec3::NEG_Z);
        assert_eq!(down, Some(404.0));
        let in_the_dent = l.ray_hit(Vec3::new(100.0, 100.0, 500.0), Vec3::NEG_Z);
        assert_eq!(in_the_dent, None);
    }

    #[test]
    fn moving_and_turning_keep_it_whole() {
        let l = l_shape();
        let moved = l.mapped(|p| p + Vec3::new(16.0, 0.0, 32.0)).unwrap();
        assert_eq!(moved.bottom(), 32.0);
        assert_eq!(moved.bounds().min.x, 16.0);
        let turned = l.mapped(|p| Vec3::new(-p.y, p.x, p.z)).unwrap();
        assert_eq!(turned.pieces().len(), 2);
        assert_eq!(turned.bounds().min.x, -128.0);
        assert!(
            l.mapped(|p| p * Vec3::new(1.0, 1.0, 0.0)).is_err(),
            "flattened"
        );
    }
}
