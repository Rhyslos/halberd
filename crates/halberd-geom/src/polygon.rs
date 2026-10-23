//! Floor plans: flat outlines of any shape, dents included, and the convex
//! brushes that fill them.
//!
//! An outline is split into as few convex parts as an easy method allows
//! (Hertel and Mehlhorn's): cut it into triangles by clipping off "ears",
//! then remove every cut whose two sides together are still convex. That
//! gives at most four times the fewest possible parts, and in practice
//! about the same number a person would choose.

use crate::{Brush, GeomError, Plane};
use glam::{DVec2, Vec2, Vec3};

/// Most corners an outline may have.
pub const MAX_OUTLINE_POINTS: usize = 256;

/// Tolerance for corners counting as on a line, in square units of area.
const STRAIGHT: f64 = 1e-6;

/// The outline cleaned up and wound counter-clockwise (seen from above):
/// repeated corners and corners on a straight line removed. Fails if what
/// is left encloses no area or crosses itself.
pub fn clean_outline(outline: &[Vec2]) -> Result<Vec<Vec2>, GeomError> {
    if outline.iter().any(|p| !p.is_finite()) {
        return Err(GeomError::NotFinite);
    }
    if outline.len() > MAX_OUTLINE_POINTS {
        return Err(GeomError::TooManyFaces);
    }
    let mut points: Vec<DVec2> = Vec::with_capacity(outline.len());
    for p in outline {
        let p = p.as_dvec2();
        if points.last().is_none_or(|q| q.distance(p) > 1e-6) {
            points.push(p);
        }
    }
    while points.len() > 1 && points[0].distance(points[points.len() - 1]) <= 1e-6 {
        points.pop();
    }
    // Drop corners on a straight line, until none are left.
    loop {
        let n = points.len();
        if n < 3 {
            return Err(GeomError::NotClosed);
        }
        let straight = (0..n).find(|&i| {
            let (a, b, c) = (points[(i + n - 1) % n], points[i], points[(i + 1) % n]);
            cross(b - a, c - b).abs() <= STRAIGHT * (b - a).length().max(1.0)
        });
        match straight {
            Some(i) => {
                points.remove(i);
            }
            None => break,
        }
    }
    if crosses_itself(&points) {
        return Err(GeomError::CrossesItself);
    }
    if signed_area(&points) < 0.0 {
        points.reverse();
    }
    if signed_area(&points) < 1.0 {
        return Err(GeomError::TooSmall);
    }
    Ok(points.into_iter().map(|p| p.as_vec2()).collect())
}

/// The outline split into convex parts, each counter-clockwise.
pub fn convex_parts(outline: &[Vec2]) -> Result<Vec<Vec<Vec2>>, GeomError> {
    let outline = clean_outline(outline)?;
    let points: Vec<DVec2> = outline.iter().map(|p| p.as_dvec2()).collect();
    let mut parts: Vec<Vec<usize>> = triangulate(&points)?
        .into_iter()
        .map(|t| t.to_vec())
        .collect();
    // Remove cuts while the two sides together stay convex.
    let mut merged = true;
    while merged {
        merged = false;
        'search: for i in 0..parts.len() {
            for j in i + 1..parts.len() {
                if let Some(joined) = join(&parts[i], &parts[j])
                    && is_convex(&points, &joined)
                {
                    parts[i] = joined;
                    parts.remove(j);
                    merged = true;
                    break 'search;
                }
            }
        }
    }
    Ok(parts
        .into_iter()
        .map(|part| part.into_iter().map(|i| outline[i]).collect())
        .collect())
}

/// Brushes filling the outline from height `bottom` up to `top`, one per
/// convex part.
pub fn prism_brushes(outline: &[Vec2], bottom: f32, top: f32) -> Result<Vec<Brush>, GeomError> {
    if !(bottom.is_finite() && top.is_finite()) {
        return Err(GeomError::NotFinite);
    }
    if top - bottom < crate::MIN_SIZE {
        return Err(GeomError::TooSmall);
    }
    convex_parts(outline)?
        .iter()
        .map(|part| {
            let mut planes = vec![
                Plane {
                    normal: Vec3::NEG_Z,
                    distance: -bottom,
                },
                Plane {
                    normal: Vec3::Z,
                    distance: top,
                },
            ];
            for (i, a) in part.iter().enumerate() {
                let b = part[(i + 1) % part.len()];
                // Counter-clockwise from above: outside is to the right.
                let along = b - *a;
                let out = Vec3::new(along.y, -along.x, 0.0);
                planes.push(Plane::new(out, a.extend(bottom)).ok_or(GeomError::NotFinite)?);
            }
            Brush::from_planes(&planes)
        })
        .collect()
}

fn cross(a: DVec2, b: DVec2) -> f64 {
    a.x * b.y - a.y * b.x
}

fn signed_area(points: &[DVec2]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| cross(points[i], points[(i + 1) % n]))
        .sum::<f64>()
        * 0.5
}

/// True if two edges that are not neighbours touch or cross.
fn crosses_itself(points: &[DVec2]) -> bool {
    let n = points.len();
    let edge = |i: usize| (points[i], points[(i + 1) % n]);
    for i in 0..n {
        for j in i + 1..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            let ((a, b), (c, d)) = (edge(i), edge(j));
            if segments_meet(a, b, c, d) {
                return true;
            }
        }
    }
    false
}

fn segments_meet(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> bool {
    let side = |p: DVec2, q: DVec2, r: DVec2| cross(q - p, r - p);
    let (d1, d2) = (side(c, d, a), side(c, d, b));
    let (d3, d4) = (side(a, b, c), side(a, b, d));
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    let on = |p: DVec2, q: DVec2, r: DVec2| {
        side(p, q, r).abs() < 1e-9
            && r.x >= p.x.min(q.x) - 1e-9
            && r.x <= p.x.max(q.x) + 1e-9
            && r.y >= p.y.min(q.y) - 1e-9
            && r.y <= p.y.max(q.y) + 1e-9
    };
    on(c, d, a) || on(c, d, b) || on(a, b, c) || on(a, b, d)
}

/// Triangles covering a simple counter-clockwise polygon, by clipping ears.
fn triangulate(points: &[DVec2]) -> Result<Vec<[usize; 3]>, GeomError> {
    let mut left: Vec<usize> = (0..points.len()).collect();
    let mut triangles = Vec::with_capacity(points.len());
    while left.len() > 3 {
        let n = left.len();
        let ear = (0..n).find(|&i| {
            let (a, b, c) = (left[(i + n - 1) % n], left[i], left[(i + 1) % n]);
            let (pa, pb, pc) = (points[a], points[b], points[c]);
            if cross(pb - pa, pc - pb) <= 0.0 {
                return false; // A dent, not an ear.
            }
            !left
                .iter()
                .filter(|&&k| k != a && k != b && k != c)
                .any(|&k| inside_triangle(points[k], pa, pb, pc))
        });
        let i = ear.ok_or(GeomError::NotClosed)?;
        triangles.push([left[(i + n - 1) % n], left[i], left[(i + 1) % n]]);
        left.remove(i);
    }
    triangles.push([left[0], left[1], left[2]]);
    Ok(triangles)
}

fn inside_triangle(p: DVec2, a: DVec2, b: DVec2, c: DVec2) -> bool {
    cross(b - a, p - a) >= 0.0 && cross(c - b, p - b) >= 0.0 && cross(a - c, p - c) >= 0.0
}

/// Two parts sharing an edge, joined into one along it (both
/// counter-clockwise), or `None` if they share no edge.
fn join(a: &[usize], b: &[usize]) -> Option<Vec<usize>> {
    let (na, nb) = (a.len(), b.len());
    for i in 0..na {
        let (p, q) = (a[i], a[(i + 1) % na]);
        // In `b` the shared edge runs the other way: q then p.
        if let Some(j) = (0..nb).find(|&j| b[j] == q && b[(j + 1) % nb] == p) {
            let mut out = Vec::with_capacity(na + nb - 2);
            // a from q round to p, then b's corners between p and q.
            for k in 0..na {
                out.push(a[(i + 1 + k) % na]);
            }
            for k in 2..nb {
                out.push(b[(j + k) % nb]);
            }
            return Some(out);
        }
    }
    None
}

fn is_convex(points: &[DVec2], part: &[usize]) -> bool {
    let n = part.len();
    (0..n).all(|i| {
        let (a, b, c) = (
            points[part[(i + n - 1) % n]],
            points[part[i]],
            points[part[(i + 1) % n]],
        );
        cross(b - a, c - b) >= -STRAIGHT
    })
}

#[cfg(test)]
mod tests;
