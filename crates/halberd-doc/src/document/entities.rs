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
        let old = self.model_boxes.insert(name.to_ascii_lowercase(), local);
        if old != Some(local) {
            self.model_box_revision += 1;
        }
    }

    /// Goes up whenever a model's box is learnt or changes, so anything
    /// worked out from props' sizes (the map's size) can be redone.
    pub fn model_box_revision(&self) -> u64 {
        self.model_box_revision
    }

    /// Model `name`'s box in its own space, if it has been read.
    pub fn model_box(&self, name: &str) -> Option<Aabb> {
        self.model_boxes.get(&name.to_ascii_lowercase()).copied()
    }

    /// Records the box entities of class `classname` are shown as, around
    /// their origin (from the game's entity definitions). Not an edit.
    pub fn set_class_box(&mut self, classname: &str, local: Aabb) {
        let key = format!("class:{}", classname.to_ascii_lowercase());
        if self.model_boxes.insert(key, local) != Some(local) {
            self.model_box_revision += 1;
        }
    }

    /// The box a point entity fills: its model's (turned with it) when the
    /// model has been read, else its class's from the entity definitions,
    /// else the small marker.
    pub fn entity_box(&self, entity: &crate::EntityObject) -> Option<Aabb> {
        if let Some(model) = entity.model().and_then(|m| self.model_box(m)) {
            return entity.placed_box(Some(model));
        }
        let class = format!("class:{}", entity.classname.to_ascii_lowercase());
        match (self.model_boxes.get(&class), entity.origin) {
            (Some(local), Some(origin)) => {
                Some(Aabb::from_corners(origin + local.min, origin + local.max))
            }
            _ => entity.marker(),
        }
    }

    /// The box around an object itself: a prop fills its model's box.
    pub(super) fn object_box(&self, object: &Object) -> Option<Aabb> {
        match object {
            Object::Entity(e) => self.entity_box(e),
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

#[cfg(test)]
mod tests {
    use crate::{Command, Document, EntityObject};
    use glam::Vec3;
    use halberd_geom::Aabb;

    #[test]
    fn a_prop_is_picked_and_framed_by_its_models_box_once_known() {
        let mut doc = Document::new();
        let prop = EntityObject::point(
            "prop_static",
            Vec3::new(0.0, 0.0, 0.0),
            &[("model", "models/lamppost.mdl")],
        );
        let command = Command::AddEntity(prop);
        assert_eq!(command.describe(), "Place prop");
        doc.execute(command).unwrap();
        let id = doc.objects().next().unwrap().0;
        // Aiming at the top of a tall lamp post misses the small marker…
        let from = Vec3::new(0.0, -500.0, 150.0);
        assert_eq!(doc.pick(from, Vec3::Y), None);
        // …until the model's box is known (which is not an edit).
        let revision = doc.revision();
        doc.set_model_box(
            "MODELS/LAMPPOST.MDL",
            Aabb::from_corners(Vec3::new(-4.0, -4.0, 0.0), Vec3::new(4.0, 4.0, 176.0)),
        );
        assert_eq!(doc.pick(from, Vec3::Y).map(|(hit, _)| hit), Some(id));
        assert_eq!(doc.bounds_of(id).unwrap().max.z, 176.0);
        assert_eq!(doc.revision(), revision);
        assert_eq!(doc.model_box_revision(), 1);
        let light = Command::AddEntity(EntityObject::point("light", Vec3::ZERO, &[]));
        assert_eq!(light.describe(), "Add light");
    }

    #[test]
    fn a_class_box_from_the_definitions_is_used_without_a_model() {
        let mut doc = Document::new();
        doc.execute(Command::AddEntity(EntityObject::point(
            "npc_citizen",
            Vec3::new(100.0, 0.0, 0.0),
            &[],
        )))
        .unwrap();
        let id = *doc.selection().iter().next().unwrap();
        let before = doc.model_box_revision();
        doc.set_class_box(
            "NPC_Citizen",
            Aabb::from_corners(Vec3::new(-16.0, -16.0, 0.0), Vec3::new(16.0, 16.0, 72.0)),
        );
        assert!(doc.model_box_revision() > before);
        let b = doc.bounds_of(id).unwrap();
        assert_eq!(b.min, Vec3::new(84.0, -16.0, 0.0));
        assert_eq!(b.max, Vec3::new(116.0, 16.0, 72.0));
    }
}
