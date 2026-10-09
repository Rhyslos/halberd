//! Editing a brush by its corners, edges and faces.
//!
//! A brush's corners are listed once each ([`Brush::points`]); edges and
//! faces refer to them by number. Moving some corners gives a new set of
//! points, and the new brush is the smallest convex solid around them (its
//! convex hull), so the result is always a valid brush: a face whose
//! corners no longer lie flat is split in two, and a corner pushed inside
//! the shape disappears, as when Hammer merges vertices.

mod hull;

use crate::{Brush, GeomError, MAX_FACES, Plane};
use glam::{DVec3, Vec3};
use hull::{FLAT, HullPlane, hull_planes};

/// Corners closer than this count as the same corner, in units.
const WELD: f32 = 1e-2;

/// A brush's corners (as [`Brush::points`]), the corners of each face (as
/// [`Brush::face_points`]) and its edges (as [`Brush::edges`]).
pub type Topology = (Vec<Vec3>, Vec<Vec<usize>>, Vec<(usize, usize)>);

/// What became of one face of the old brush after [`Brush::with_moved_points`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceFate {
    /// True if the face kept exactly its corners (it may have moved or
    /// tilted, but was not split, merged or given new corners). Data that
    /// only fits the old outline, such as a displacement, can then stay.
    pub whole: bool,
}

impl Brush {
    /// Every corner of the brush, each listed once. Edges and faces refer
    /// to corners by their place in this list, which stays the same for an
    /// unchanged brush.
    pub fn points(&self) -> Vec<Vec3> {
        let mut points: Vec<Vec3> = Vec::new();
        for face in self.faces() {
            for &v in face.vertices() {
                if !points.iter().any(|p| p.distance(v) < WELD) {
                    points.push(v);
                }
            }
        }
        points
    }

    /// The corners of each face, as places in [`Brush::points`], in the
    /// face's own order (counter-clockwise seen from outside).
    pub fn face_points(&self) -> Vec<Vec<usize>> {
        self.face_points_among(&self.points())
    }

    /// [`Brush::face_points`] with the brush's points already worked out.
    fn face_points_among(&self, points: &[Vec3]) -> Vec<Vec<usize>> {
        self.faces()
            .iter()
            .map(|face| {
                face.vertices()
                    .iter()
                    .filter_map(|v| points.iter().position(|p| p.distance(*v) < WELD))
                    .collect()
            })
            .collect()
    }

    /// Every edge once, as two places in [`Brush::points`], the lower first.
    pub fn edges(&self) -> Vec<(usize, usize)> {
        self.topology().2
    }

    /// The corners, the corners of each face and the edges together, for
    /// callers that need all three (each alone works out the corners anew).
    pub fn topology(&self) -> Topology {
        let points = self.points();
        let faces = self.face_points_among(&points);
        let mut edges = Vec::new();
        for corners in &faces {
            for (i, &a) in corners.iter().enumerate() {
                let b = corners[(i + 1) % corners.len()];
                let edge = (a.min(b), a.max(b));
                if a != b && !edges.contains(&edge) {
                    edges.push(edge);
                }
            }
        }
        (points, faces, edges)
    }

    /// The brush with some corners moved: `moves` gives new positions for
    /// places in [`Brush::points`] (places out of range are ignored).
    ///
    /// The new brush is the convex hull of the moved points. Each of its
    /// faces' [`crate::Face::source`] is the old face it grew from (the one
    /// sharing the most corners), so materials follow, and faces stay in
    /// the old faces' order; the returned list says, for each new face,
    /// whether it is that old face kept whole. Moving nothing (or every
    /// corner to where it already is) gives back the very same brush.
    ///
    /// Fails if the points no longer enclose a solid (all in one plane),
    /// or with the usual limits on size and face count.
    pub fn with_moved_points(
        &self,
        moves: &[(usize, Vec3)],
    ) -> Result<(Brush, Vec<FaceFate>), GeomError> {
        let original = self.points();
        let mut points = original.clone();
        for &(index, to) in moves {
            if !to.is_finite() {
                return Err(GeomError::NotFinite);
            }
            if let Some(p) = points.get_mut(index) {
                *p = to;
            }
        }
        if points == original {
            let fates = vec![FaceFate { whole: true }; self.faces().len()];
            return Ok((self.clone(), fates));
        }
        let old_faces = self.face_points_among(&original);
        let kept = self.slid_planes(&old_faces, &original, &points);
        let wide: Vec<DVec3> = points.iter().map(|p| p.as_dvec3()).collect();
        let mut groups = hull_planes(&wide, &old_faces, &kept)?;
        let mut planes: Vec<Plane> = groups.iter().map(|g| g.plane(&kept)).collect();
        if planes.len() > MAX_FACES {
            prune(&mut planes, &mut groups);
        }
        if planes.len() > MAX_FACES {
            return Err(GeomError::TooManyFaces);
        }
        let matches = self.match_groups(&groups, &old_faces);
        let mut brush = Brush::from_planes(&planes)?;
        let fates = fates(&brush, &groups, &matches, &old_faces);

        // Each face names its old face, and faces keep the old order.
        let mut faces: Vec<_> = std::mem::take(&mut brush.faces)
            .into_iter()
            .zip(fates)
            .map(|(mut face, fate)| {
                face.source = matches[face.source];
                (face, fate)
            })
            .collect();
        faces.sort_by_key(|(face, _)| face.source);
        let (faces, fates) = faces.into_iter().unzip();
        brush.faces = faces;
        Ok((brush, fates))
    }

    /// For each old face whose corners all moved the same way (or not at
    /// all), its exact plane, shifted: such a face only slid, and keeping
    /// its plane keeps untouched faces exactly as they were, however their
    /// corners were rounded.
    fn slid_planes(
        &self,
        old_faces: &[Vec<usize>],
        before: &[Vec3],
        after: &[Vec3],
    ) -> Vec<Option<Plane>> {
        self.faces()
            .iter()
            .zip(old_faces)
            .map(|(face, corners)| {
                let offset = after[*corners.first()?] - before[corners[0]];
                corners
                    .iter()
                    .all(|&c| (after[c] - before[c]).distance(offset) < 1e-4)
                    .then(|| Plane {
                        normal: face.plane().normal,
                        distance: face.plane().distance + face.plane().normal.dot(offset),
                    })
            })
            .collect()
    }

    /// The old face each side of the hull grew from: the one sharing the
    /// most corners (ties go to the one facing most nearly the same way).
    fn match_groups(&self, groups: &[HullPlane], old_faces: &[Vec<usize>]) -> Vec<usize> {
        let old_normals: Vec<DVec3> = self
            .faces()
            .iter()
            .map(|f| f.plane().normal.as_dvec3())
            .collect();
        groups
            .iter()
            .map(|group| {
                if let Some(old) = group.kept_from {
                    return old;
                }
                let shared =
                    |i: usize| old_faces[i].iter().filter(|c| group.on.contains(c)).count();
                (0..old_faces.len())
                    .max_by(|&a, &b| {
                        shared(a).cmp(&shared(b)).then(
                            old_normals[a]
                                .dot(group.normal)
                                .total_cmp(&old_normals[b].dot(group.normal)),
                        )
                    })
                    .unwrap_or(0)
            })
            .collect()
    }
}

/// What became of each old face, for the faces of `brush` (whose sources
/// are places in `groups`): whole if it is the only face grown from its old
/// face and has exactly the old face's corners.
fn fates(
    brush: &Brush,
    groups: &[HullPlane],
    matches: &[usize],
    old_faces: &[Vec<usize>],
) -> Vec<FaceFate> {
    let grown_from: Vec<usize> = brush.faces().iter().map(|f| matches[f.source()]).collect();
    brush
        .faces()
        .iter()
        .zip(&grown_from)
        .map(|(face, &old)| {
            let mut on = groups[face.source()].on.clone();
            let mut corners = old_faces[old].clone();
            on.sort_unstable();
            corners.sort_unstable();
            let alone = grown_from.iter().filter(|&&m| m == old).count() == 1;
            FaceFate {
                whole: alone && on == corners,
            }
        })
        .collect()
}

/// Drops sides of the hull that cut away almost nothing, until at most
/// [`MAX_FACES`] are left. Rounded corners (a many-sided sphere's, say) are
/// not exactly convex, so the hull can find slivers along them that a
/// brush has no need for. Only newly found sides are dropped, smallest
/// first, and only if the rest already keeps within [`FLAT`] of them.
fn prune(planes: &mut Vec<Plane>, groups: &mut Vec<HullPlane>) {
    let mut candidates: Vec<usize> = (0..groups.len())
        .filter(|&g| groups[g].kept_from.is_none())
        .collect();
    candidates.sort_by_key(|&g| groups[g].on.len());
    let mut dropped = vec![false; planes.len()];
    let mut left = planes.len();
    for g in candidates {
        if left <= MAX_FACES {
            break;
        }
        let others: Vec<Plane> = (0..planes.len())
            .filter(|&i| i != g && !dropped[i])
            .map(|i| planes[i])
            .collect();
        // The rest must keep within every side dropped so far, too.
        let checked: Vec<(DVec3, f64)> = (0..planes.len())
            .filter(|&i| i == g || dropped[i])
            .map(|i| planes[i].to_f64())
            .collect();
        let redundant = Brush::from_any_planes(&others).is_ok_and(|rest| {
            rest.faces().iter().flat_map(|f| f.vertices()).all(|v| {
                checked
                    .iter()
                    .all(|(normal, distance)| normal.dot(v.as_dvec3()) - distance < FLAT)
            })
        });
        if redundant {
            dropped[g] = true;
            left -= 1;
        }
    }
    let mut keep = dropped.iter().map(|d| !d);
    let mut keep_group = keep.clone();
    planes.retain(|_| keep.next().unwrap_or(true));
    groups.retain(|_| keep_group.next().unwrap_or(true));
}

#[cfg(test)]
mod tests;
