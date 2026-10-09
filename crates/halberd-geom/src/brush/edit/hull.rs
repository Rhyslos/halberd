//! The convex hull of a brush's moved corners, as flat sides.

use crate::{GeomError, MAX_FACES, Plane};
use glam::DVec3;

/// Points closer than this to a plane count as on it, in units. More than
/// the 0.01 a corner may be moved onto a whole unit, so a face whose
/// corners were rounded still counts as flat.
pub(super) const FLAT: f64 = 2e-2;

/// One flat side of a convex hull: its plane and the points lying on it.
pub(super) struct HullPlane {
    pub(super) normal: DVec3,
    pub(super) distance: f64,
    pub(super) on: Vec<usize>,
    /// The old face this side is, unchanged but perhaps slid, if any.
    pub(super) kept_from: Option<usize>,
}

impl HullPlane {
    /// The side's plane: a slid old face's exact plane (from `kept`, by
    /// old face), or the one found.
    pub(super) fn plane(&self, kept: &[Option<Plane>]) -> Plane {
        match self
            .kept_from
            .and_then(|old| kept.get(old).copied().flatten())
        {
            Some(plane) => plane,
            None => Plane {
                normal: self.normal.as_vec3(),
                distance: self.distance as f32,
            },
        }
    }
}

/// The flat sides of the convex hull of `points`, found by adding points
/// one at a time to a growing hull of triangles, then joining triangles
/// that lie in the same plane. A triangle among the corners of an old face
/// that only slid (`kept`, by old face, with each face's corners in
/// `old_faces`) belongs to that face and its exact plane.
pub(super) fn hull_planes(
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
        // Joined only when facing the same way too: a small triangle can lie
        // within FLAT of a neighbouring side a few degrees off, and joining
        // it there would put a corner off where any point is.
        let flat_with = |plane: &HullPlane| {
            plane.normal.dot(normal) > 1.0 - 1e-6
                && [p, q, r]
                    .iter()
                    .all(|x| (plane.normal.dot(*x) - plane.distance).abs() < FLAT)
        };
        if planes.iter().any(flat_with) {
            continue;
        }
        // A thin sliver's plane can lean off the surface, with points in
        // front of it. Turned about its longest edge until every point is
        // behind it, it bounds the hull again (leaving it out would let the
        // brush bulge there).
        let (normal, distance) = if points.iter().any(|x| normal.dot(*x) - distance > FLAT) {
            match supporting_plane(points, [p, q, r], normal) {
                Some(plane) => plane,
                None => continue,
            }
        } else {
            (normal, distance)
        };
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

/// The plane through the longest edge of triangle `corners` with every
/// point behind it, facing as nearly along `facing` as it can, or `None` if
/// the edge is not on the outside of the points.
fn supporting_plane(points: &[DVec3], corners: [DVec3; 3], facing: DVec3) -> Option<(DVec3, f64)> {
    use std::f64::consts::{FRAC_PI_2, PI, TAU};
    let [p, q, r] = corners;
    let (a, b) = [(p, q), (q, r), (r, p)]
        .into_iter()
        .max_by(|x, y| x.0.distance(x.1).total_cmp(&y.0.distance(y.1)))?;
    let along = (b - a).try_normalize()?;
    // A frame at right angles to the edge: `u` as near `facing` as can be.
    let u = (facing - along * facing.dot(along)).try_normalize()?;
    let w = along.cross(u);
    // Angles of every point seen from the edge, from `u`, in (0, 2π).
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for x in points {
        let v = *x - a;
        let (s, t) = (v.dot(u), v.dot(w));
        if s.hypot(t) < 1e-6 {
            continue;
        }
        let angle = t.atan2(s).rem_euclid(TAU);
        lo = lo.min(angle);
        hi = hi.max(angle);
    }
    // A normal at angle α has every point behind it when every point's
    // angle is within a quarter turn either side of α + π.
    let (first, last) = (hi - 3.0 * FRAC_PI_2, lo - FRAC_PI_2);
    if first > last || hi - lo > PI {
        return None;
    }
    let turn = 0.0_f64.clamp(first, last);
    let normal = (u * turn.cos() + w * turn.sin()).try_normalize()?;
    Some((normal, normal.dot(a)))
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
    let first = start_tetrahedron(points).ok_or(GeomError::NotClosed)?;
    let [a, b, c, d] = first;
    let start = if Triangle::new(points, [a, b, c]).height(points[d]) < 0.0 {
        [[a, b, c], [a, d, b], [b, d, c], [c, d, a]]
    } else {
        [[a, c, b], [a, b, d], [b, c, d], [c, a, d]]
    };
    let mut triangles: Vec<Triangle> = start.iter().map(|&t| Triangle::new(points, t)).collect();
    // Each directed edge, to the triangle it belongs to.
    let mut edges = EdgeMap::new();
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

        let seen = visible_patch(&triangles, &edges, start, point);
        let rim = rim_of(&triangles, &edges, &seen);
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

/// Each directed edge of the hull, to the triangle it belongs to.
type EdgeMap = std::collections::HashMap<(usize, usize), usize>;

/// The patch of triangles `point` stands more than [`FLAT`] above, grown
/// from triangle `start` across shared edges, so it is always one piece.
fn visible_patch(
    triangles: &[Triangle],
    edges: &EdgeMap,
    start: usize,
    point: DVec3,
) -> Vec<usize> {
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
    seen
}

/// The rim of a patch of triangles: its edges whose other side is not in
/// the patch, each running the patch's way round.
fn rim_of(triangles: &[Triangle], edges: &EdgeMap, patch: &[usize]) -> Vec<(usize, usize)> {
    let mut rim = Vec::new();
    for &t in patch {
        for i in 0..3 {
            let (from, to) = (triangles[t].corners[i], triangles[t].corners[(i + 1) % 3]);
            if edges.get(&(to, from)).is_none_or(|o| !patch.contains(o)) {
                rim.push((from, to));
            }
        }
    }
    rim
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
pub(super) fn convex_hull_for_tests(points: &[DVec3]) -> Vec<[usize; 3]> {
    convex_hull(points).unwrap_or_default()
}
