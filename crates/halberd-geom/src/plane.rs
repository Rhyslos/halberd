//! Infinite planes.

use glam::{DVec3, Vec3};

/// An infinite flat surface: the points `p` where `normal · p == distance`.
/// The normal is unit length and points out of the solid the plane bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    /// Unit normal, pointing outwards.
    pub normal: Vec3,
    /// Distance from the origin along the normal.
    pub distance: f32,
}

impl Plane {
    /// The plane through `point` facing `normal`. Returns `None` if the
    /// normal has no length or anything is not finite.
    pub fn new(normal: Vec3, point: Vec3) -> Option<Self> {
        let normal = normal.try_normalize()?;
        let distance = normal.dot(point);
        (point.is_finite() && distance.is_finite()).then_some(Self { normal, distance })
    }

    /// How far `point` is in front of the plane (negative: behind, inside
    /// the solid).
    pub fn signed_distance(&self, point: Vec3) -> f32 {
        self.normal.dot(point) - self.distance
    }

    /// The plane in double precision, for clipping.
    pub(crate) fn to_f64(self) -> (DVec3, f64) {
        (self.normal.as_dvec3(), f64::from(self.distance))
    }

    /// True if both planes face the same way within a tiny tolerance and are
    /// at the same place.
    pub(crate) fn same_as(&self, other: &Plane) -> bool {
        self.normal.dot(other.normal) > 1.0 - 1e-6 && (self.distance - other.distance).abs() < 1e-3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_distance_is_positive_in_front() {
        let floor_top = Plane::new(Vec3::Z, Vec3::new(5.0, 5.0, 64.0)).unwrap();
        assert_eq!(floor_top.distance, 64.0);
        assert_eq!(floor_top.signed_distance(Vec3::new(0.0, 0.0, 100.0)), 36.0);
        assert_eq!(floor_top.signed_distance(Vec3::ZERO), -64.0);
    }

    #[test]
    fn normals_are_made_unit_length() {
        let p = Plane::new(Vec3::new(0.0, 3.0, 4.0), Vec3::ZERO).unwrap();
        assert!((p.normal.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn bad_input_gives_none() {
        assert!(Plane::new(Vec3::ZERO, Vec3::ZERO).is_none());
        assert!(Plane::new(Vec3::Z, Vec3::splat(f32::NAN)).is_none());
        assert!(Plane::new(Vec3::splat(f32::INFINITY), Vec3::ZERO).is_none());
    }
}
