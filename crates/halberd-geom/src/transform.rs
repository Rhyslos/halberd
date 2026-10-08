//! Moving, rotating and scaling brushes.
//!
//! A brush is a set of planes, so transforming it means transforming each
//! plane: a point on the plane moves with the transform, and the normal is
//! transformed by the inverse transpose (which keeps it at right angles to
//! the surface when scaling unevenly). The face polygons are then rebuilt
//! from the new planes. All of this runs in double precision.

use crate::{Brush, GeomError, Plane};
use glam::{DMat3, DMat4, Vec3};

impl Brush {
    /// The brush transformed by `matrix` (any mix of moving, rotating and
    /// scaling). Fails if the matrix flattens the brush or is not finite.
    pub fn transformed(&self, matrix: DMat4) -> Result<Brush, GeomError> {
        if !matrix.is_finite() {
            return Err(GeomError::NotFinite);
        }
        let linear = DMat3::from_mat4(matrix);
        if linear.determinant().abs() < 1e-9 {
            return Err(GeomError::TooSmall);
        }
        let normal_matrix = linear.inverse().transpose();
        let planes: Vec<Plane> = self
            .faces()
            .iter()
            .map(|face| {
                let plane = face.plane();
                let normal = plane.normal.as_dvec3();
                let point = normal * f64::from(plane.distance);
                let moved_point = matrix.transform_point3(point);
                let moved_normal = (normal_matrix * normal).normalize_or_zero();
                Plane {
                    normal: moved_normal.as_vec3(),
                    distance: moved_normal.dot(moved_point) as f32,
                }
            })
            .collect();
        Brush::from_planes(&planes)
    }

    /// The brush turned by `angle` radians around the line through `pivot`
    /// along `axis` (counter-clockwise looking down the axis).
    pub fn rotated(&self, axis: Vec3, angle: f32, pivot: Vec3) -> Result<Brush, GeomError> {
        let axis = axis
            .as_dvec3()
            .try_normalize()
            .ok_or(GeomError::NotFinite)?;
        let pivot = pivot.as_dvec3();
        let matrix = DMat4::from_translation(pivot)
            * DMat4::from_axis_angle(axis, f64::from(angle))
            * DMat4::from_translation(-pivot);
        self.transformed(matrix)
    }

    /// The brush stretched by `factors` along the world axes, keeping
    /// `anchor` in place.
    pub fn scaled(&self, factors: Vec3, anchor: Vec3) -> Result<Brush, GeomError> {
        let anchor = anchor.as_dvec3();
        let matrix = DMat4::from_translation(anchor)
            * DMat4::from_scale(factors.as_dvec3())
            * DMat4::from_translation(-anchor);
        self.transformed(matrix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Aabb;
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

    fn box_brush(min: Vec3, max: Vec3) -> Brush {
        Brush::cuboid(Aabb::from_corners(min, max)).unwrap()
    }

    #[test]
    fn a_quarter_turn_swaps_width_and_depth_on_whole_units() {
        let b = box_brush(Vec3::ZERO, Vec3::new(128.0, 64.0, 32.0));
        let turned = b.rotated(Vec3::Z, FRAC_PI_2, b.center()).unwrap();
        assert_eq!(turned.bounds().size(), Vec3::new(64.0, 128.0, 32.0));
        assert_eq!(turned.center(), b.center());
        assert!(turned.is_axis_aligned_box());
        for face in turned.faces() {
            for v in face.vertices() {
                assert_eq!(*v, v.round(), "corner {v} on whole units");
            }
        }
    }

    #[test]
    fn turning_back_and_forth_returns_the_same_shape() {
        let b = box_brush(Vec3::new(-16.0, 0.0, 0.0), Vec3::new(48.0, 96.0, 64.0));
        let there = b
            .rotated(Vec3::new(1.0, 1.0, 0.0), 0.7, Vec3::splat(10.0))
            .unwrap();
        let back = there
            .rotated(Vec3::new(1.0, 1.0, 0.0), -0.7, Vec3::splat(10.0))
            .unwrap();
        let (a, c) = (b.bounds(), back.bounds());
        assert!((a.min - c.min).length() < 1e-2 && (a.max - c.max).length() < 1e-2);
    }

    #[test]
    fn an_eighth_turn_makes_an_eight_sided_footprint_box() {
        let b = box_brush(Vec3::ZERO, Vec3::splat(64.0));
        let turned = b.rotated(Vec3::Z, FRAC_PI_4, b.center()).unwrap();
        assert!(!turned.is_axis_aligned_box());
        assert_eq!(turned.faces().len(), 6);
        let width = turned.bounds().size().x;
        assert!((width - 64.0 * 2f32.sqrt()).abs() < 0.01, "{width}");
        // Faces still point outwards.
        for face in turned.faces() {
            let v = face.vertices();
            let winding = (v[1] - v[0]).cross(v[2] - v[0]);
            assert!(winding.dot(face.plane().normal) > 0.0);
        }
        // A half turn of a box is the same box.
        let half = b.rotated(Vec3::Z, PI, b.center()).unwrap();
        assert_eq!(half.bounds(), b.bounds());
    }

    #[test]
    fn scaling_keeps_the_anchor_in_place() {
        let b = box_brush(Vec3::ZERO, Vec3::new(64.0, 64.0, 128.0));
        let wider = b.scaled(Vec3::new(2.0, 1.0, 1.0), Vec3::ZERO).unwrap();
        assert_eq!(wider.bounds().min, Vec3::ZERO);
        assert_eq!(wider.bounds().max, Vec3::new(128.0, 64.0, 128.0));
        let around_centre = b.scaled(Vec3::splat(0.5), b.center()).unwrap();
        assert_eq!(around_centre.center(), b.center());
        assert_eq!(around_centre.bounds().size(), Vec3::new(32.0, 32.0, 64.0));
    }

    #[test]
    fn scaling_a_turned_brush_keeps_faces_flat() {
        // Uneven scaling of a rotated box: normals must stay at right
        // angles to the faces, so every corner lies on its plane.
        let b = box_brush(Vec3::ZERO, Vec3::splat(64.0));
        let turned = b.rotated(Vec3::Z, 0.5, b.center()).unwrap();
        let stretched = turned.scaled(Vec3::new(3.0, 1.0, 1.0), Vec3::ZERO).unwrap();
        for face in stretched.faces() {
            for v in face.vertices() {
                assert!(face.plane().signed_distance(*v).abs() < 0.01);
            }
        }
    }

    #[test]
    fn flattening_or_broken_transforms_are_refused() {
        let b = box_brush(Vec3::ZERO, Vec3::splat(64.0));
        assert_eq!(
            b.scaled(Vec3::new(0.0, 1.0, 1.0), Vec3::ZERO),
            Err(GeomError::TooSmall)
        );
        assert_eq!(b.scaled(Vec3::NAN, Vec3::ZERO), Err(GeomError::NotFinite));
        assert_eq!(
            b.rotated(Vec3::ZERO, 1.0, Vec3::ZERO),
            Err(GeomError::NotFinite)
        );
        assert!(
            b.scaled(Vec3::splat(5000.0), Vec3::ZERO).is_err(),
            "beyond the world"
        );
        assert!(
            b.scaled(Vec3::splat(0.001), Vec3::ZERO).is_err(),
            "too thin"
        );
    }
}
