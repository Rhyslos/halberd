//! Editing a brush by its corners, edges and faces.
//!
//! A brush's corners are listed once each ([`Brush::points`]); edges and
//! faces refer to them by number. Moving some corners gives a new set of
//! points, and the new brush is the smallest convex solid around them (its
//! convex hull), so the result is always a valid brush: a face whose
//! corners no longer lie flat is split in two, and a corner pushed inside
//! the shape disappears, as when Hammer merges vertices.

use crate::{Brush, GeomError, MAX_FACES, Plane};
use glam::{DVec3, Vec3};

/// Corners closer than this count as the same corner, in units.
const WELD: f32 = 1e-2;
/// Points closer than this to a plane count as on it, in units. More than
/// the 0.01 a corner may be moved onto a whole unit, so a face whose
/// corners were rounded still counts as flat.
const FLAT: f64 = 2e-2;

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
        let points = self.points();
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
        let mut edges = Vec::new();
        for corners in self.face_points() {
            for (i, &a) in corners.iter().enumerate() {
                let b = corners[(i + 1) % corners.len()];
                let edge = (a.min(b), a.max(b));
                if a != b && !edges.contains(&edge) {
                    edges.push(edge);
                }
            }
        }
        edges
    }

    /// The brush with some corners moved: `moves` gives new positions for
    /// places in [`Brush::points`] (places out of range are ignored).
    ///
    /// The new brush is the convex hull of the moved points. Each of its
    /// faces' [`crate::Face::source`] is the old face it grew from (the one
    /// sharing the most corners), so materials follow; the returned list
    /// says, for each new face, whether it is that old face kept whole.
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
        let old_faces = self.face_points();

        // A face whose corners all moved the same way (or not at all) only
        // slid, so it keeps its exact plane, shifted: untouched faces stay
        // exactly as they were, however their corners were rounded.
        let kept: Vec<Option<Plane>> = self
            .faces()
            .iter()
            .zip(&old_faces)
            .map(|(face, corners)| {
                let offset = points[*corners.first()?] - original[corners[0]];
                corners
                    .iter()
                    .all(|&c| (points[c] - original[c]).distance(offset) < 1e-4)
                    .then(|| Plane {
                        normal: face.plane().normal,
                        distance: face.plane().distance + face.plane().normal.dot(offset),
                    })
            })
            .collect();
        let wide: Vec<DVec3> = points.iter().map(|p| p.as_dvec3()).collect();
        let groups = hull_planes(&wide, &old_faces, &kept)?;
        if groups.len() > MAX_FACES {
            return Err(GeomError::TooManyFaces);
        }

        // Match each side of the hull with the old face it shares most
        // corners with (ties go to the one facing most nearly the same way).
        let old_normals: Vec<DVec3> = self
            .faces()
            .iter()
            .map(|f| f.plane().normal.as_dvec3())
            .collect();
        let matches: Vec<usize> = groups
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
            .collect();
        let same_corners: Vec<bool> = groups
            .iter()
            .zip(&matches)
            .map(|(group, &old)| {
                let mut on = group.on.clone();
                let mut corners = old_faces[old].clone();
                on.sort_unstable();
                corners.sort_unstable();
                on == corners
            })
            .collect();
        let planes: Vec<Plane> = groups
            .iter()
            .map(|group| match group.kept_from {
                Some(old) => kept[old].unwrap_or_else(|| self.faces()[old].plane()),
                None => Plane {
                    normal: group.normal.as_vec3(),
                    distance: group.distance as f32,
                },
            })
            .collect();
        let mut brush = Brush::from_planes(&planes)?;
        let fates = brush
            .faces()
            .iter()
            .map(|face| {
                let g = face.source();
                let alone = matches.iter().filter(|&&m| m == matches[g]).count() == 1;
                FaceFate {
                    whole: alone && same_corners[g],
                }
            })
            .collect();
        let matches: Vec<usize> = brush.faces().iter().map(|f| matches[f.source()]).collect();
        brush.set_sources(&matches);
        Ok((brush, fates))
    }
}

/// One flat side of a convex hull: its plane and the points lying on it.
struct HullPlane {
    normal: DVec3,
    distance: f64,
    on: Vec<usize>,
    /// The old face this side is, unchanged but perhaps slid, if any.
    kept_from: Option<usize>,
}

/// The flat sides of the convex hull of `points`, found by adding points
/// one at a time to a growing hull of triangles, then joining triangles
/// that lie in the same plane. A triangle among the corners of an old face
/// that only slid (`kept`, by old face, with each face's corners in
/// `old_faces`) belongs to that face and its exact plane.
fn hull_planes(
    points: &[DVec3],
    old_faces: &[Vec<usize>],
    kept: &[Option<Plane>],
) -> Result<Vec<HullPlane>, GeomError> {
    let triangles = convex_hull(points)?;
    // A slid face still bounds the hull only if no point is in front of it
    // (a moved corner elsewhere may now stick out past it).
    let still_outside: Vec<bool> = kept
        .iter()
        .map(|plane| {
            plane.is_some_and(|plane| {
                let (normal, distance) = plane.to_f64();
                points.iter().all(|p| normal.dot(*p) - distance < FLAT)
            })
        })
        .collect();
    let mut planes: Vec<HullPlane> = Vec::new();
    for t in triangles {
        let kept_face = (0..old_faces.len())
            .find(|&f| still_outside[f] && t.iter().all(|c| old_faces[f].contains(c)));
        if let Some(face) = kept_face {
            if let Some(plane) =
                kept[face].filter(|_| !planes.iter().any(|p| p.kept_from == Some(face)))
            {
                let (normal, distance) = plane.to_f64();
                let on = (0..points.len())
                    .filter(|&i| (normal.dot(points[i]) - distance).abs() < FLAT)
                    .collect();
                planes.push(HullPlane {
                    normal,
                    distance,
                    on,
                    kept_from: Some(face),
                });
            }
            continue;
        }
        let [p, q, r] = t.map(|i| points[i]);
        let Some(normal) = (q - p).cross(r - p).try_normalize() else {
            continue; // A sliver with no area.
        };
        let distance = normal.dot(p);
        let flat_with = |plane: &HullPlane| {
            plane.normal.dot(normal) > 0.0
                && [p, q, r]
                    .iter()
                    .all(|x| (plane.normal.dot(*x) - plane.distance).abs() < FLAT)
        };
        if planes.iter().any(flat_with) {
            continue;
        }
        let on = (0..points.len())
            .filter(|&i| (normal.dot(points[i]) - distance).abs() < FLAT)
            .collect();
        planes.push(HullPlane {
            normal,
            distance,
            on,
            kept_from: None,
        });
    }
    Ok(planes)
}

/// One triangle of a growing hull.
struct Triangle {
    corners: [usize; 3],
    normal: DVec3,
    distance: f64,
    /// Still part of the hull.
    alive: bool,
    /// Waiting points in front of this triangle (each in one list only).
    outside: Vec<usize>,
}

impl Triangle {
    fn new(points: &[DVec3], corners: [usize; 3]) -> Self {
        let [p, q, r] = corners.map(|i| points[i]);
        let normal = (q - p).cross(r - p).normalize_or_zero();
        Self {
            corners,
            normal,
            distance: normal.dot(p),
            alive: true,
            outside: Vec::new(),
        }
    }

    fn height(&self, point: DVec3) -> f64 {
        self.normal.dot(point) - self.distance
    }
}

/// The convex hull of `points` as triangles wound counter-clockwise from
/// outside (Quickhull). The point standing highest above the hull is added
/// first, and the triangles it sees are found as one connected patch, so
/// rounding near flat spots cannot tear the hull. Points within [`FLAT`] of
/// the hull are left out.
fn convex_hull(points: &[DVec3]) -> Result<Vec<[usize; 3]>, GeomError> {
    use std::collections::HashMap;

    let first = start_tetrahedron(points).ok_or(GeomError::NotClosed)?;
    let [a, b, c, d] = first;
    let start = if Triangle::new(points, [a, b, c]).height(points[d]) < 0.0 {
        [[a, b, c], [a, d, b], [b, d, c], [c, d, a]]
    } else {
        [[a, c, b], [a, b, d], [b, c, d], [c, a, d]]
    };
    let mut triangles: Vec<Triangle> = start.iter().map(|&t| Triangle::new(points, t)).collect();
    // Each directed edge, to the triangle it belongs to.
    let mut edges: HashMap<(usize, usize), usize> = HashMap::new();
    for (t, tri) in triangles.iter().enumerate() {
        for i in 0..3 {
            edges.insert((tri.corners[i], tri.corners[(i + 1) % 3]), t);
        }
    }
    // Gives each point to the triangle (among `candidates`) it is highest
    // above, or drops it if it is inside.
    let assign = |triangles: &mut Vec<Triangle>, candidates: &[usize], point: usize| {
        let best = candidates
            .iter()
            .map(|&t| (t, triangles[t].height(points[point])))
            .max_by(|x, y| x.1.total_cmp(&y.1));
        if let Some((t, h)) = best
            && h > FLAT
        {
            triangles[t].outside.push(point);
        }
    };
    let all: Vec<usize> = (0..triangles.len()).collect();
    for point in (0..points.len()).filter(|i| !first.contains(i)) {
        assign(&mut triangles, &all, point);
    }

    loop {
        // The highest point above any triangle.
        let mut best: Option<(usize, usize, f64)> = None;
        for (t, tri) in triangles.iter().enumerate().filter(|(_, tri)| tri.alive) {
            for &p in &tri.outside {
                let h = tri.height(points[p]);
                if best.is_none_or(|(_, _, top)| h > top) {
                    best = Some((t, p, h));
                }
            }
        }
        let Some((start, index, _)) = best else {
            break;
        };
        let point = points[index];

        // The patch of triangles the point sees, grown from `start`.
        let mut seen = vec![start];
        let mut grow = vec![start];
        while let Some(t) = grow.pop() {
            for i in 0..3 {
                let (from, to) = (triangles[t].corners[i], triangles[t].corners[(i + 1) % 3]);
                if let Some(&o) = edges.get(&(to, from))
                    && !seen.contains(&o)
                    && triangles[o].height(point) > FLAT
                {
                    seen.push(o);
                    grow.push(o);
                }
            }
        }
        // The rim of the patch: each edge becomes a new triangle up to the
        // point.
        let mut rim = Vec::new();
        for &t in &seen {
            for i in 0..3 {
                let (from, to) = (triangles[t].corners[i], triangles[t].corners[(i + 1) % 3]);
                if edges.get(&(to, from)).is_none_or(|o| !seen.contains(o)) {
                    rim.push((from, to));
                }
            }
        }
        let mut orphans = Vec::new();
        for &t in &seen {
            triangles[t].alive = false;
            orphans.append(&mut triangles[t].outside);
            for i in 0..3 {
                let (from, to) = (triangles[t].corners[i], triangles[t].corners[(i + 1) % 3]);
                edges.remove(&(from, to));
            }
        }
        let mut added = Vec::with_capacity(rim.len());
        for (from, to) in rim {
            let t = triangles.len();
            triangles.push(Triangle::new(points, [from, to, index]));
            for (x, y) in [(from, to), (to, index), (index, from)] {
                edges.insert((x, y), t);
            }
            added.push(t);
        }
        for orphan in orphans.into_iter().filter(|&p| p != index) {
            assign(&mut triangles, &added, orphan);
        }
        if triangles.len() > MAX_FACES * 256 {
            return Err(GeomError::TooManyFaces);
        }
    }
    Ok(triangles
        .into_iter()
        .filter(|t| t.alive)
        .map(|t| t.corners)
        .collect())
}

/// Four points that are not all in one plane, spread far apart, or `None`
/// if every point lies in one plane.
fn start_tetrahedron(points: &[DVec3]) -> Option<[usize; 4]> {
    let farthest = |score: &dyn Fn(DVec3) -> f64| {
        (0..points.len()).max_by(|&i, &j| score(points[i]).total_cmp(&score(points[j])))
    };
    let a = 0;
    let b = farthest(&|p| p.distance_squared(points[a]))?;
    let along = (points[b] - points[a]).try_normalize()?;
    let off_line = |p: DVec3| {
        let v = p - points[a];
        (v - along * v.dot(along)).length()
    };
    let c = farthest(&off_line)?;
    if off_line(points[c]) < FLAT {
        return None;
    }
    let normal = (points[b] - points[a])
        .cross(points[c] - points[a])
        .try_normalize()?;
    let off_plane = |p: DVec3| normal.dot(p - points[a]).abs();
    let d = farthest(&off_plane)?;
    (off_plane(points[d]) >= FLAT).then_some([a, b, c, d])
}

#[cfg(test)]
mod tests;
