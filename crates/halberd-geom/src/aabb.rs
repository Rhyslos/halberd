//! Axis-aligned boxes.

use glam::Vec3;

/// A box aligned with the world axes, given by its lowest and highest
/// corners.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    /// The corner with the lowest X, Y and Z.
    pub min: Vec3,
    /// The corner with the highest X, Y and Z.
    pub max: Vec3,
}

impl Aabb {
    /// The box spanning two opposite corners given in any order.
    pub fn from_corners(a: Vec3, b: Vec3) -> Self {
        Self {
            min: a.min(b),
            max: a.max(b),
        }
    }

    /// The smallest box containing every point, or `None` if there are none.
    pub fn from_points(points: impl IntoIterator<Item = Vec3>) -> Option<Self> {
        let mut points = points.into_iter();
        let first = points.next()?;
        Some(points.fold(Self::from_corners(first, first), |b, p| {
            Self::from_corners(b.min.min(p), b.max.max(p))
        }))
    }

    /// The smallest box containing both.
    pub fn union(self, other: Self) -> Self {
        Self::from_corners(self.min.min(other.min), self.max.max(other.max))
    }

    /// The middle of the box.
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// Width, depth and height.
    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    /// The eight corners: bottom four (Z = min) first, each four going round.
    pub fn corners(&self) -> [Vec3; 8] {
        let (a, b) = (self.min, self.max);
        [
            Vec3::new(a.x, a.y, a.z),
            Vec3::new(b.x, a.y, a.z),
            Vec3::new(b.x, b.y, a.z),
            Vec3::new(a.x, b.y, a.z),
            Vec3::new(a.x, a.y, b.z),
            Vec3::new(b.x, a.y, b.z),
            Vec3::new(b.x, b.y, b.z),
            Vec3::new(a.x, b.y, b.z),
        ]
    }

    /// The twelve edges, as pairs of corners.
    pub fn edges(&self) -> [[Vec3; 2]; 12] {
        let c = self.corners();
        [
            [c[0], c[1]],
            [c[1], c[2]],
            [c[2], c[3]],
            [c[3], c[0]],
            [c[4], c[5]],
            [c[5], c[6]],
            [c[6], c[7]],
            [c[7], c[4]],
            [c[0], c[4]],
            [c[1], c[5]],
            [c[2], c[6]],
            [c[3], c[7]],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_can_be_given_in_any_order() {
        let b = Aabb::from_corners(Vec3::new(10.0, -5.0, 3.0), Vec3::new(-2.0, 7.0, 1.0));
        assert_eq!(b.min, Vec3::new(-2.0, -5.0, 1.0));
        assert_eq!(b.max, Vec3::new(10.0, 7.0, 3.0));
        assert_eq!(b.size(), Vec3::new(12.0, 12.0, 2.0));
        assert_eq!(b.center(), Vec3::new(4.0, 1.0, 2.0));
    }

    #[test]
    fn bounds_of_points_and_union() {
        assert!(Aabb::from_points([]).is_none());
        let b = Aabb::from_points([Vec3::ONE, -Vec3::ONE, Vec3::new(5.0, 0.0, 0.0)]).unwrap();
        assert_eq!(b.max, Vec3::new(5.0, 1.0, 1.0));
        let u = b.union(Aabb::from_corners(Vec3::splat(10.0), Vec3::splat(11.0)));
        assert_eq!(u.min, -Vec3::ONE);
        assert_eq!(u.max, Vec3::splat(11.0));
    }

    #[test]
    fn edges_join_corners_one_axis_apart() {
        let b = Aabb::from_corners(Vec3::ZERO, Vec3::new(1.0, 2.0, 3.0));
        for [a, c] in b.edges() {
            let differs = (a - c).to_array().iter().filter(|d| **d != 0.0).count();
            assert_eq!(differs, 1);
        }
    }
}
