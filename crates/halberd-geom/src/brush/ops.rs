//! Shape operations on single brushes: extruding a face, bevelling edges,
//! cutting with a plane, mirroring, taking a face away, and volume.
//!
//! Every result is a convex brush, as Source needs. Operations that add
//! faces give each new face a source one past the old faces (the
//! interface gives it fresh data); old faces keep their sources.

use super::edit::WELD;
use crate::{Brush, FaceFate, GeomError, Plane};
use glam::{DMat4, DVec3, Vec3};

impl Brush {
    /// How much space the brush fills, in cubic units.
    pub fn volume(&self) -> f32 {
        // Each face is the base of a cone from the origin; their signed
        // volumes add up to the brush's.
        let total: f64 = self
            .faces()
            .iter()
            .map(|face| {
                let v = face.vertices();
                let area = (1..v.len().saturating_sub(1))
                    .map(|i| (v[i] - v[0]).as_dvec3().cross((v[i + 1] - v[0]).as_dvec3()))
                    .sum::<DVec3>()
                    .length()
                    * 0.5;
                area * f64::from(face.plane().distance)
            })
            .sum();
        (total / 3.0).abs() as f32
    }

    /// A new brush standing on face `face`: its outline pushed straight out
    /// by `depth` units, as a solid (the face extruded). Its faces' sources
    /// are all `face`, so they can take that face's material.
    pub fn extruded_face(&self, face: usize, depth: f32) -> Result<Brush, GeomError> {
        let base = self.faces().get(face).ok_or(GeomError::NotClosed)?;
        if !(depth.is_finite() && depth >= crate::MIN_SIZE) {
            return Err(GeomError::TooSmall);
        }
        let normal = base.plane().normal;
        let outline = base.vertices();
        let mut planes = vec![
            Plane {
                normal: -normal,
                distance: -base.plane().distance,
            },
            Plane {
                normal,
                distance: base.plane().distance + depth,
            },
        ];
        // One side per edge of the outline, facing out of it (the outline
        // runs counter-clockwise seen from outside).
        for (i, &a) in outline.iter().enumerate() {
            let b = outline[(i + 1) % outline.len()];
            if let Some(plane) = Plane::new((b - a).cross(normal), a) {
                planes.push(plane);
            }
        }
        let mut brush = Brush::from_planes(&planes)?;
        for f in &mut brush.faces {
            f.source = face;
        }
        Ok(brush)
    }

    /// The brush with edges bevelled: each edge (two places in
    /// [`Brush::points`]) cut off by a flat face that starts `leg` units
    /// back along each of the two faces meeting there. Each new face's
    /// source is the first face beside its edge; old faces keep theirs.
    /// The fates say which old faces kept exactly their corners.
    ///
    /// Fails with [`GeomError::BevelTooWide`] if a cut would reach another
    /// corner, and with the usual limits otherwise. Pairs that are not
    /// edges are ignored.
    pub fn bevelled(
        &self,
        edges: &[(usize, usize)],
        leg: f32,
    ) -> Result<(Brush, Vec<FaceFate>), GeomError> {
        if !(leg.is_finite() && leg > 0.0) {
            return Err(GeomError::TooSmall);
        }
        let (points, faces, all_edges) = self.topology();
        let mut planes: Vec<Plane> = self.faces().iter().map(|f| f.plane()).collect();
        let mut new_sources = Vec::new();
        for &(a, b) in edges {
            let edge = (a.min(b), a.max(b));
            if !all_edges.contains(&edge) {
                continue;
            }
            let beside: Vec<usize> = (0..faces.len())
                .filter(|&f| faces[f].contains(&a) && faces[f].contains(&b))
                .collect();
            let [f1, f2] = beside[..] else {
                continue;
            };
            let (pa, pb) = (points[a], points[b]);
            let plane = bevel_plane(self, [f1, f2], pa, pb, leg)?;
            // The cut must leave every other corner where it is.
            let reaches = points
                .iter()
                .enumerate()
                .any(|(i, p)| i != a && i != b && plane.signed_distance(*p) > -0.01);
            if reaches {
                return Err(GeomError::BevelTooWide);
            }
            planes.push(plane);
            new_sources.push(f1);
        }
        let old = self.faces().len();
        let mut brush = Brush::from_planes(&planes)?;
        let fates = self.fates_of(&brush, old);
        for f in &mut brush.faces {
            if f.source >= old {
                f.source = new_sources.get(f.source - old).copied().unwrap_or(0);
            }
        }
        Ok((brush, fates))
    }

    /// The brush cut by `plane`: the part behind it and the part in front,
    /// each `None` if there is no such part (or it would be thinner than a
    /// unit). In each part the cut face's source is one past the old faces.
    pub fn cut(&self, plane: Plane) -> (Option<Brush>, Option<Brush>) {
        let side = |plane: Plane| {
            let mut planes: Vec<Plane> = self.faces().iter().map(|f| f.plane()).collect();
            planes.push(plane);
            let brush = Brush::from_any_planes(&planes).ok()?;
            let used = brush.faces().iter().any(|f| f.source == planes.len() - 1);
            (used && brush.faces().len() <= crate::MAX_FACES).then_some(brush)
        };
        let behind = side(plane);
        let front = side(Plane {
            normal: -plane.normal,
            distance: -plane.distance,
        });
        match (behind, front) {
            // A plane that only grazes the brush cuts nothing.
            (Some(b), Some(f)) => (Some(b), Some(f)),
            _ => {
                let whole = self.clone();
                if plane.signed_distance(self.center()) <= 0.0 {
                    (Some(whole), None)
                } else {
                    (None, Some(whole))
                }
            }
        }
    }

    /// The brush mirrored along world axis `axis` (0 X, 1 Y, 2 Z) about the
    /// position `at` on that axis. Faces keep their order.
    pub fn mirrored(&self, axis: usize, at: f32) -> Result<Brush, GeomError> {
        let mut scale = DVec3::ONE;
        if axis > 2 {
            return Err(GeomError::NotFinite);
        }
        scale[axis] = -1.0;
        let mut pivot = DVec3::ZERO;
        pivot[axis] = f64::from(at);
        let matrix = DMat4::from_translation(pivot)
            * DMat4::from_scale(scale)
            * DMat4::from_translation(-pivot);
        self.transformed(matrix)
    }

    /// The brush without some faces: the faces around them grow to close
    /// the gap. Fails with [`GeomError::WouldOpen`] if they cannot (a box's
    /// side, for one). Places out of range are ignored.
    pub fn without_faces(&self, remove: &[usize]) -> Result<(Brush, Vec<FaceFate>), GeomError> {
        let kept: Vec<usize> = (0..self.faces().len())
            .filter(|f| !remove.contains(f))
            .collect();
        let planes: Vec<Plane> = kept.iter().map(|&f| self.faces()[f].plane()).collect();
        let mut brush = match Brush::from_planes(&planes) {
            Ok(brush) => brush,
            Err(GeomError::NotClosed | GeomError::TooLarge) => return Err(GeomError::WouldOpen),
            Err(e) => return Err(e),
        };
        for f in &mut brush.faces {
            f.source = kept[f.source];
        }
        let fates = self.fates_of(&brush, self.faces().len());
        Ok((brush, fates))
    }

    /// For each face of `new` (whose sources are this brush's faces, or
    /// `old` and up for new faces), whether it is its old face with exactly
    /// the same corners and the only face from it.
    fn fates_of(&self, new: &Brush, old: usize) -> Vec<FaceFate> {
        let same = |a: &[Vec3], b: &[Vec3]| {
            a.len() == b.len() && a.iter().all(|p| b.iter().any(|q| p.distance(*q) < WELD))
        };
        new.faces()
            .iter()
            .map(|f| {
                let alone = new.faces().iter().filter(|g| g.source == f.source).count() == 1;
                let whole = f.source < old
                    && alone
                    && same(f.vertices(), self.faces()[f.source].vertices());
                FaceFate { whole }
            })
            .collect()
    }
}

/// The plane bevelling the edge from `a` to `b` between faces `beside`,
/// starting `leg` units back along each face.
fn bevel_plane(
    brush: &Brush,
    beside: [usize; 2],
    a: Vec3,
    b: Vec3,
    leg: f32,
) -> Result<Plane, GeomError> {
    let along = (b - a).normalize_or_zero();
    let [n1, n2] = beside.map(|f| brush.faces()[f].plane().normal);
    // Into each face, at right angles to the edge.
    let into = |n: Vec3, other: Vec3| {
        let t = n.cross(along).normalize_or_zero();
        if t.dot(other) > 0.0 { -t } else { t }
    };
    let (t1, t2) = (into(n1, n2), into(n2, n1));
    let normal = (n1 + n2).normalize_or_zero();
    let (p1, p2) = (a + t1 * leg, a + t2 * leg);
    if normal == Vec3::ZERO || (p1 - p2).length() < 1e-3 {
        return Err(GeomError::NotClosed);
    }
    Plane::new(normal, (p1 + p2) * 0.5).ok_or(GeomError::NotFinite)
}

#[cfg(test)]
mod tests;
