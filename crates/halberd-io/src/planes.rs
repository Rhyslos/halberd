//! Planes in Hammer's three-point form.
//!
//! A VMF side's plane is three points `p0 p1 p2`. Its normal, pointing out
//! of the brush, is `(p0 - p1) × (p2 - p1)`, the convention Valve's
//! compile tools use.

use glam::DVec3;
use halberd_geom::{Face, Plane};

/// The plane through three points, or `None` if they are in a line.
pub(crate) fn plane_from_points(points: [[f64; 3]; 3]) -> Option<Plane> {
    let [p0, p1, p2] = points.map(DVec3::from_array);
    let normal = (p0 - p1).cross(p2 - p1).try_normalize()?;
    let plane = Plane {
        normal: normal.as_vec3(),
        distance: normal.dot(p0) as f32,
    };
    (plane.normal.is_finite() && plane.distance.is_finite()).then_some(plane)
}

/// Three corners of a face, in the order that gives its outward normal in
/// Hammer's convention. The corners chosen make the largest triangle, so
/// the plane is as exact as the corners allow.
pub(crate) fn points_of_face(face: &Face) -> [[f64; 3]; 3] {
    let v: Vec<DVec3> = face.vertices().iter().map(|p| p.as_dvec3()).collect();
    let mut best = (0.0, 1, 2);
    for i in 1..v.len() {
        for j in i + 1..v.len() {
            let area = (v[i] - v[0]).cross(v[j] - v[0]).length();
            if area > best.0 {
                best = (area, i, j);
            }
        }
    }
    let (_, i, j) = best;
    // Corners run counter-clockwise seen from outside, so (v[i] - v[0]) ×
    // (v[j] - v[0]) points out. Hammer's normal is (p0 - p1) × (p2 - p1),
    // so p1 is the shared corner.
    [v[i], v[0], v[j]].map(|p| p.to_array())
}

/// True if two planes are the same within rounding.
pub(crate) fn same_plane(a: &Plane, b: &Plane) -> bool {
    a.normal.dot(b.normal) > 1.0 - 1e-6 && (a.distance - b.distance).abs() < 1e-3
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    use halberd_geom::{Aabb, Brush};

    #[test]
    fn hammer_s_top_face_points_up() {
        // The top of Hammer's default 128-unit box, as Hammer writes it.
        let top = plane_from_points([[-64.0, 64.0, 64.0], [64.0, 64.0, 64.0], [64.0, -64.0, 64.0]])
            .unwrap();
        assert_eq!(top.normal, Vec3::Z);
        assert_eq!(top.distance, 64.0);
    }

    #[test]
    fn face_points_give_back_the_face_plane() {
        let brush =
            Brush::cuboid(Aabb::from_corners(Vec3::ZERO, Vec3::new(64.0, 32.0, 16.0))).unwrap();
        let turned = brush
            .rotated(Vec3::new(1.0, 2.0, 3.0), 0.4, Vec3::ZERO)
            .unwrap();
        for face in brush.faces().iter().chain(turned.faces()) {
            let back = plane_from_points(points_of_face(face)).unwrap();
            assert!(
                same_plane(&back, &face.plane()),
                "{back:?} vs {:?}",
                face.plane()
            );
        }
    }

    #[test]
    fn points_in_a_line_make_no_plane() {
        assert!(plane_from_points([[0.0; 3], [1.0, 1.0, 1.0], [2.0, 2.0, 2.0]]).is_none());
    }
}
