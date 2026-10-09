//! Convex brushes: solids made of planes, as Source and Hammer store them.
//!
//! A brush is the space behind all of its planes. Each plane becomes a face
//! whose polygon is found by starting with a huge square on the plane and
//! cutting away everything in front of the other planes. The cutting runs
//! in double precision so corners land exactly on whole units.

use crate::{Aabb, GeomError, MAX_COORD, Plane};
use glam::{DVec3, Vec3};

/// Most faces a brush may have. Hammer's own limit is similar; anything
/// beyond it is almost certainly a damaged file.
pub const MAX_FACES: usize = 128;
/// Thinnest a brush may be on any axis, in units.
pub const MIN_SIZE: f32 = 1.0;

/// Half the width of the square each face starts as before cutting: far
/// beyond the world, so only a shape that is not closed keeps its corners.
const HUGE: f64 = 1_048_576.0;
/// Points closer than this to a cutting plane count as on it.
const EPSILON: f64 = 1e-3;
/// Corners within this of a whole unit are moved onto it.
const SNAP: f64 = 1e-2;

mod edit;
pub use edit::FaceFate;

/// One flat side of a brush.
#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    plane: Plane,
    vertices: Vec<Vec3>,
    source: usize,
}

impl Face {
    /// The plane the face lies on; its normal points out of the brush.
    pub fn plane(&self) -> Plane {
        self.plane
    }

    /// The corners of the face, counter-clockwise seen from outside.
    pub fn vertices(&self) -> &[Vec3] {
        &self.vertices
    }

    /// Which of the planes given to [`Brush::from_planes`] this face came
    /// from (its index in that list). Data kept per face elsewhere, such as
    /// a texture, follows the face through moves, turns and rebuilds this
    /// way: [`Brush::transformed`] passes the faces in order, so a face's
    /// source is then its index among the old faces.
    pub fn source(&self) -> usize {
        self.source
    }
}

/// A convex solid: the space behind all of its faces' planes.
#[derive(Debug, Clone, PartialEq)]
pub struct Brush {
    faces: Vec<Face>,
    bounds: Aabb,
}

impl Brush {
    /// A box filling `bounds`.
    pub fn cuboid(bounds: Aabb) -> Result<Self, GeomError> {
        if !(bounds.min.is_finite() && bounds.max.is_finite()) {
            return Err(GeomError::NotFinite);
        }
        if bounds.size().min_element() < MIN_SIZE {
            return Err(GeomError::TooSmall);
        }
        let (lo, hi) = (bounds.min, bounds.max);
        let planes = [
            (Vec3::X, hi),
            (Vec3::NEG_X, lo),
            (Vec3::Y, hi),
            (Vec3::NEG_Y, lo),
            (Vec3::Z, hi),
            (Vec3::NEG_Z, lo),
        ]
        .map(|(normal, point)| Plane {
            normal,
            distance: normal.dot(point),
        });
        Self::from_planes(&planes)
    }

    /// The solid behind every plane. Duplicate planes are ignored, and so are
    /// planes that do not touch the solid.
    pub fn from_planes(planes: &[Plane]) -> Result<Self, GeomError> {
        if planes.len() > MAX_FACES {
            return Err(GeomError::TooManyFaces);
        }
        let finite = |p: &Plane| p.normal.is_finite() && p.distance.is_finite();
        if !planes.iter().all(finite) {
            return Err(GeomError::NotFinite);
        }
        let mut unique: Vec<(Plane, usize)> = Vec::with_capacity(planes.len());
        for (index, plane) in planes.iter().enumerate() {
            // Any normal length is accepted; the plane is scaled so its
            // normal is unit length, which the cutting below relies on.
            let length = plane.normal.length();
            if length < 1e-6 {
                return Err(GeomError::NotFinite);
            }
            let plane = Plane {
                normal: plane.normal / length,
                distance: plane.distance / length,
            };
            if !unique.iter().any(|(u, _)| u.same_as(&plane)) {
                unique.push((plane, index));
            }
        }

        let mut faces = Vec::with_capacity(unique.len());
        for (i, (plane, source)) in unique.iter().enumerate() {
            let mut polygon = huge_square(*plane);
            for (j, (other, _)) in unique.iter().enumerate() {
                if i != j {
                    polygon = clip(&polygon, *other);
                    if polygon.len() < 3 {
                        break;
                    }
                }
            }
            if polygon.len() >= 3 {
                faces.push(Face {
                    plane: *plane,
                    vertices: polygon.into_iter().map(snap).collect(),
                    source: *source,
                });
            }
        }
        if faces.len() < 4 {
            return Err(GeomError::NotClosed);
        }
        let bounds = Aabb::from_points(faces.iter().flat_map(|f| f.vertices.iter().copied()))
            .ok_or(GeomError::NotClosed)?;
        let reach = bounds.min.abs().max(bounds.max.abs()).max_element();
        if f64::from(reach) > HUGE * 0.5 {
            return Err(GeomError::NotClosed);
        }
        if reach > MAX_COORD {
            return Err(GeomError::TooLarge);
        }
        if bounds.size().min_element() < MIN_SIZE {
            return Err(GeomError::TooSmall);
        }
        Ok(Self { faces, bounds })
    }

    /// The faces, each with its polygon.
    pub fn faces(&self) -> &[Face] {
        &self.faces
    }

    /// The smallest axis-aligned box around the brush.
    pub fn bounds(&self) -> Aabb {
        self.bounds
    }

    /// The middle of the brush's bounds.
    pub fn center(&self) -> Vec3 {
        self.bounds.center()
    }

    /// True if the brush is a box with sides facing along the world axes,
    /// so its shape is fully described by its bounds.
    pub fn is_axis_aligned_box(&self) -> bool {
        self.faces.len() == 6
            && self.faces.iter().all(|f| {
                let n = f.plane.normal.abs();
                n.max_element() > 1.0 - 1e-6
            })
    }

    /// The same brush moved by `offset`.
    pub fn translated(&self, offset: Vec3) -> Result<Self, GeomError> {
        if !offset.is_finite() {
            return Err(GeomError::NotFinite);
        }
        let planes: Vec<Plane> = self
            .faces
            .iter()
            .map(|f| Plane {
                normal: f.plane.normal,
                distance: f.plane.distance + f.plane.normal.dot(offset),
            })
            .collect();
        Self::from_planes(&planes)
    }

    /// Sets each face's [`Face::source`], in face order.
    pub(crate) fn set_sources(&mut self, sources: &[usize]) {
        for (face, &source) in self.faces.iter_mut().zip(sources) {
            face.source = source;
        }
    }

    /// Distance along a ray (from `origin` along unit `direction`) to where
    /// it enters the brush, or `None` if it misses. A ray starting inside
    /// the brush does not hit it, so a camera inside a large brush can
    /// still pick what is in front of it (as in Hammer).
    pub fn ray_hit(&self, origin: Vec3, direction: Vec3) -> Option<f32> {
        if !(origin.is_finite() && direction.is_finite()) || direction == Vec3::ZERO {
            return None;
        }
        if self
            .faces
            .iter()
            .all(|f| f.plane.signed_distance(origin) <= 0.0)
        {
            return None;
        }
        let mut enter = 0.0_f32;
        let mut exit = f32::INFINITY;
        for face in &self.faces {
            let plane = face.plane;
            let along = plane.normal.dot(direction);
            let height = plane.signed_distance(origin);
            if along.abs() < 1e-9 {
                if height > 0.0 {
                    return None;
                }
                continue;
            }
            let t = -height / along;
            if along < 0.0 {
                enter = enter.max(t);
            } else {
                exit = exit.min(t);
            }
            if enter > exit {
                return None;
            }
        }
        (exit >= 0.0 && enter.is_finite()).then_some(enter)
    }
}

/// A square on `plane`, far larger than the world, counter-clockwise seen
/// from the side the normal points to.
fn huge_square(plane: Plane) -> Vec<DVec3> {
    let (normal, distance) = plane.to_f64();
    let normal = normal.normalize();
    let helper = if normal.z.abs() < 0.9 {
        DVec3::Z
    } else {
        DVec3::X
    };
    let u = normal.cross(helper).normalize() * HUGE;
    let v = normal.cross(u);
    let centre = normal * distance;
    vec![
        centre - u - v,
        centre + u - v,
        centre + u + v,
        centre - u + v,
    ]
}

/// Keeps the part of `polygon` behind `plane` (Sutherland–Hodgman).
fn clip(polygon: &[DVec3], plane: Plane) -> Vec<DVec3> {
    let (normal, distance) = plane.to_f64();
    let height = |p: DVec3| normal.dot(p) - distance;
    let mut out = Vec::with_capacity(polygon.len() + 1);
    for (i, &current) in polygon.iter().enumerate() {
        let next = polygon[(i + 1) % polygon.len()];
        let (h_current, h_next) = (height(current), height(next));
        if h_current <= EPSILON {
            out.push(current);
        }
        let crosses = (h_current < -EPSILON && h_next > EPSILON)
            || (h_current > EPSILON && h_next < -EPSILON);
        if crosses {
            let t = h_current / (h_current - h_next);
            out.push(current + (next - current) * t);
        }
    }
    out.dedup_by(|a, b| a.distance_squared(*b) < EPSILON * EPSILON);
    if out.len() > 1 && out[0].distance_squared(out[out.len() - 1]) < EPSILON * EPSILON {
        out.pop();
    }
    out
}

/// Moves coordinates that are a hair away from a whole unit onto it.
fn snap(p: DVec3) -> Vec3 {
    let snap_one = |x: f64| {
        let r = x.round();
        if (x - r).abs() < SNAP { r } else { x }
    };
    DVec3::new(snap_one(p.x), snap_one(p.y), snap_one(p.z)).as_vec3()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(min: f32, max: f32) -> Brush {
        Brush::cuboid(Aabb::from_corners(Vec3::splat(min), Vec3::splat(max))).unwrap()
    }

    #[test]
    fn a_cuboid_has_six_square_faces_on_whole_units() {
        let b = Brush::cuboid(Aabb::from_corners(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(128.0, 64.0, 32.0),
        ))
        .unwrap();
        assert_eq!(b.faces().len(), 6);
        for face in b.faces() {
            assert_eq!(face.vertices().len(), 4);
            for v in face.vertices() {
                assert_eq!(*v, v.round(), "corner {v} should be exact");
                assert!(face.plane().signed_distance(*v).abs() < 1e-4);
            }
        }
        assert_eq!(b.bounds().max, Vec3::new(128.0, 64.0, 32.0));
        assert_eq!(b.center(), Vec3::new(64.0, 32.0, 16.0));
    }

    #[test]
    fn faces_wind_counter_clockwise_seen_from_outside() {
        for face in cube(-8.0, 8.0).faces() {
            let v = face.vertices();
            let winding = (v[1] - v[0]).cross(v[2] - v[0]);
            assert!(winding.dot(face.plane().normal) > 0.0);
        }
    }

    #[test]
    fn rays_hit_where_they_enter() {
        let b = cube(0.0, 64.0);
        let hit = b.ray_hit(Vec3::new(32.0, 32.0, 200.0), Vec3::NEG_Z);
        assert_eq!(hit, Some(136.0));
        assert_eq!(b.ray_hit(Vec3::new(32.0, 32.0, 200.0), Vec3::Z), None);
        assert_eq!(b.ray_hit(Vec3::new(100.0, 32.0, 200.0), Vec3::NEG_Z), None);
        assert_eq!(b.ray_hit(Vec3::splat(32.0), Vec3::X), None, "from inside");
        assert_eq!(b.ray_hit(Vec3::new(32.0, 32.0, 200.0), Vec3::NAN), None);
        assert_eq!(b.ray_hit(Vec3::NAN, Vec3::NEG_Z), None);
        assert_eq!(b.ray_hit(Vec3::new(32.0, 32.0, 200.0), Vec3::ZERO), None);
        // Grazing along a face from outside misses.
        assert_eq!(
            b.ray_hit(Vec3::new(-10.0, 70.0, 10.0), Vec3::X),
            None,
            "parallel and outside"
        );
    }

    #[test]
    fn a_wedge_from_planes() {
        // A cube with one corner cut off diagonally.
        let mut planes: Vec<Plane> = cube(0.0, 64.0).faces().iter().map(|f| f.plane()).collect();
        planes.push(Plane::new(Vec3::new(1.0, 0.0, 1.0), Vec3::new(64.0, 0.0, 32.0)).unwrap());
        let wedge = Brush::from_planes(&planes).unwrap();
        assert_eq!(wedge.faces().len(), 7);
        assert!(wedge.faces().iter().any(|f| f.vertices().len() == 5));
    }

    #[test]
    fn planes_with_unscaled_normals_give_the_same_brush() {
        // Regression: a +X plane given as normal (2, 0, 0), distance 128 is
        // still the plane x = 64, and must not produce an open brush.
        let mut planes: Vec<Plane> = cube(0.0, 64.0).faces().iter().map(|f| f.plane()).collect();
        for plane in &mut planes {
            plane.normal *= 2.0;
            plane.distance *= 2.0;
        }
        planes[0].normal *= 0.2;
        planes[0].distance *= 0.2;
        let brush = Brush::from_planes(&planes).unwrap();
        assert_eq!(brush, cube(0.0, 64.0));
    }

    #[test]
    fn boxes_are_recognised() {
        assert!(cube(0.0, 64.0).is_axis_aligned_box());
        let mut planes: Vec<Plane> = cube(0.0, 64.0).faces().iter().map(|f| f.plane()).collect();
        planes.push(Plane::new(Vec3::new(1.0, 0.0, 1.0), Vec3::new(64.0, 0.0, 32.0)).unwrap());
        assert!(!Brush::from_planes(&planes).unwrap().is_axis_aligned_box());
    }

    #[test]
    fn faces_remember_which_plane_they_came_from() {
        let cube_planes: Vec<Plane> = cube(0.0, 64.0).faces().iter().map(|f| f.plane()).collect();
        // An unused plane first, then the cube's planes with one repeated.
        let mut planes = vec![Plane::new(Vec3::Z, Vec3::splat(500.0)).unwrap()];
        planes.extend(cube_planes.iter().copied());
        planes.push(cube_planes[2]);
        let brush = Brush::from_planes(&planes).unwrap();
        for face in brush.faces() {
            assert_eq!(planes[face.source()], face.plane());
            assert!(face.source() >= 1 && face.source() <= 6, "first copy wins");
        }
        // Transforms keep face order: a face's source is its old index.
        let moved = brush.translated(Vec3::X).unwrap();
        for (i, face) in moved.faces().iter().enumerate() {
            assert_eq!(face.source(), i);
        }
    }

    #[test]
    fn duplicate_and_unused_planes_are_dropped() {
        let mut planes: Vec<Plane> = cube(0.0, 64.0).faces().iter().map(|f| f.plane()).collect();
        planes.push(planes[0]);
        planes.push(Plane::new(Vec3::Z, Vec3::splat(500.0)).unwrap());
        assert_eq!(Brush::from_planes(&planes).unwrap().faces().len(), 6);
    }

    #[test]
    fn moving_keeps_the_shape() {
        let moved = cube(0.0, 16.0)
            .translated(Vec3::new(32.0, -16.0, 8.0))
            .unwrap();
        assert_eq!(moved.bounds().min, Vec3::new(32.0, -16.0, 8.0));
        assert_eq!(moved.bounds().max, Vec3::new(48.0, 0.0, 24.0));
        assert!(cube(0.0, 16.0).translated(Vec3::NAN).is_err());
    }

    #[test]
    fn bad_shapes_are_refused_with_a_reason() {
        let flat = Aabb::from_corners(Vec3::ZERO, Vec3::new(64.0, 64.0, 0.5));
        assert_eq!(Brush::cuboid(flat), Err(GeomError::TooSmall));
        let nan = Aabb::from_corners(Vec3::ZERO, Vec3::new(f32::NAN, 1.0, 1.0));
        assert_eq!(Brush::cuboid(nan), Err(GeomError::NotFinite));
        let huge = Aabb::from_corners(Vec3::ZERO, Vec3::splat(200_000.0));
        assert_eq!(Brush::cuboid(huge), Err(GeomError::TooLarge));
        // Only three planes: not a closed solid.
        let open: Vec<Plane> = cube(0.0, 8.0).faces()[..3]
            .iter()
            .map(|f| f.plane())
            .collect();
        assert_eq!(Brush::from_planes(&open), Err(GeomError::NotClosed));
        // Two opposite half-spaces that do not overlap.
        let apart = [
            Plane::new(Vec3::Z, Vec3::ZERO).unwrap(),
            Plane::new(Vec3::NEG_Z, Vec3::new(0.0, 0.0, 10.0)).unwrap(),
        ];
        assert!(Brush::from_planes(&apart).is_err());
        let many = vec![Plane::new(Vec3::Z, Vec3::ZERO).unwrap(); MAX_FACES + 1];
        assert_eq!(Brush::from_planes(&many), Err(GeomError::TooManyFaces));
        let zero = [Plane {
            normal: Vec3::ZERO,
            distance: 0.0,
        }];
        assert_eq!(Brush::from_planes(&zero), Err(GeomError::NotFinite));
        assert!(!GeomError::NotClosed.to_string().is_empty());
    }

    #[test]
    fn random_planes_never_crash_and_valid_brushes_are_closed() {
        // Fuzz-style: random sets of planes either make a valid brush, whose
        // corners all lie behind every plane, or give an error.
        let mut seed: u64 = 0x5EED_1234_ABCD_0001;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % 20_001) as f32 / 10_000.0 - 1.0
        };
        let mut made = 0;
        for _ in 0..2_000 {
            let count = 4 + ((next() + 1.0) * 6.0) as usize;
            let planes: Vec<Plane> = (0..count)
                .filter_map(|_| {
                    let normal = Vec3::new(next(), next(), next());
                    Plane::new(normal, normal.normalize_or_zero() * (next() + 1.5) * 300.0)
                })
                .collect();
            if let Ok(brush) = Brush::from_planes(&planes) {
                made += 1;
                for face in brush.faces() {
                    for v in face.vertices() {
                        for p in &planes {
                            assert!(p.signed_distance(*v) < 0.05, "corner outside a plane");
                        }
                    }
                }
            }
        }
        assert!(made > 100, "the test should make plenty of brushes: {made}");
    }
}
