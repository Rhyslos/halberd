//! The things a map is made of.

use glam::Vec3;
use halberd_geom::{Aabb, Brush};
use std::fmt;

/// A map object's identity. Stays the same for the object's whole life,
/// through undo and redo, and is never given to another object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId(pub(crate) u64);

impl ObjectId {
    /// The number behind the id.
    pub fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// One thing in the map.
#[derive(Debug, Clone, PartialEq)]
pub enum Object {
    /// A solid shape.
    Brush(Brush),
}

impl Object {
    /// What kind of object this is, for lists such as the Scene panel.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Brush(_) => "Brush",
        }
    }

    /// The smallest axis-aligned box around the object.
    pub fn bounds(&self) -> Aabb {
        match self {
            Self::Brush(brush) => brush.bounds(),
        }
    }

    /// Distance along a ray to where it hits the object, if it does.
    pub fn ray_hit(&self, origin: Vec3, direction: Vec3) -> Option<f32> {
        match self {
            Self::Brush(brush) => brush.ray_hit(origin, direction),
        }
    }
}
