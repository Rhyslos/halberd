//! Entities in the document: the boxes props fill, from their models.

use super::Document;
use crate::Object;
use glam::Vec3;
use halberd_geom::Aabb;

impl Document {
    /// Records model `name`'s box in its own space, once the model has been
    /// read, so props showing it are picked and framed by their real size.
    /// Not an edit: nothing to undo, and the map is not changed.
    pub fn set_model_box(&mut self, name: &str, local: Aabb) {
        self.model_boxes.insert(name.to_ascii_lowercase(), local);
    }

    /// Model `name`'s box in its own space, if it has been read.
    pub fn model_box(&self, name: &str) -> Option<Aabb> {
        self.model_boxes.get(&name.to_ascii_lowercase()).copied()
    }

    /// The box around an object itself: a prop fills its model's box.
    pub(super) fn object_box(&self, object: &Object) -> Option<Aabb> {
        match object {
            Object::Entity(e) => e.placed_box(e.model().and_then(|m| self.model_box(m))),
            other => other.bounds(),
        }
    }

    /// Distance along a ray to where it hits an object itself.
    pub(super) fn object_hit(&self, object: &Object, origin: Vec3, direction: Vec3) -> Option<f32> {
        match object {
            Object::Entity(_) => halberd_geom::Brush::cuboid(self.object_box(object)?)
                .ok()?
                .ray_hit(origin, direction),
            other => other.ray_hit(origin, direction),
        }
    }
}
