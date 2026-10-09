//! Geometry: planes, convex shapes, booleans, convex splitting.
//!
//! All the maths of shapes. Implemented so far:
//!
//! - [`Plane`]: an infinite flat surface with an outward-facing side.
//! - [`Aabb`]: an axis-aligned box, used for bounds and for drawing boxes.
//! - [`Brush`]: a convex solid made of planes, the way Source and Hammer
//!   store brushes, with its face polygons worked out from the planes. It
//!   can be moved, rotated and scaled ([`Brush::transformed`]).
//! - [`build_shape`]: ready-made shapes ([`Shape`]: box, wedge, cylinder,
//!   cone, sphere, arch, stairs) fitted into a box, with corners on whole
//!   units.
//!
//! Coordinates follow Hammer: Z is up, units are Hammer units. Shapes are
//! never allowed beyond [`MAX_COORD`] units from the origin.
//!
//! Shape operations (extrude, bevel, split, bridge), booleans and convex
//! splitting arrive in Phase 1.

mod aabb;
mod brush;
mod error;
mod plane;
mod primitive;
mod transform;

pub use aabb::Aabb;
pub use brush::{Brush, Face, MAX_FACES, MIN_SIZE};
pub use error::GeomError;
pub use plane::Plane;
pub use primitive::{
    Heading, MAX_SHAPE_BRUSHES, SIDES_RANGE, SPHERE_SIDES_RANGE, Shape, ShapeSettings, build_shape,
};

/// Farthest any point of a shape may be from the origin on any axis, in
/// units. Matches the editor's world limit; a Source map itself ends at
/// ±16384.
pub const MAX_COORD: f32 = 131_072.0;
