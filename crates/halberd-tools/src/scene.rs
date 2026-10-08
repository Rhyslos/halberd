//! What the camera and tools can ask about an open map.

use crate::camera::Ray;
use crate::controller::{GroundPlane, MAX_PICK_DISTANCE, SceneQuery};
use glam::Vec3;
use halberd_doc::{Document, ObjectId};

/// An open map as the camera sees it: its objects, and the grid plane
/// (Z = 0) wherever there is nothing else.
#[derive(Debug, Clone, Copy)]
pub struct DocumentScene<'a> {
    doc: &'a Document,
}

impl<'a> DocumentScene<'a> {
    /// The scene of `doc`.
    pub fn new(doc: &'a Document) -> Self {
        Self { doc }
    }

    /// The nearest object the ray hits within [`MAX_PICK_DISTANCE`], with
    /// the point where it hits.
    pub fn pick_object(&self, ray: &Ray) -> Option<(ObjectId, Vec3)> {
        let (id, distance) = self.doc.pick(ray.origin, ray.direction)?;
        (distance <= MAX_PICK_DISTANCE).then(|| (id, ray.at(distance)))
    }
}

impl SceneQuery for DocumentScene<'_> {
    fn pick(&self, ray: &Ray) -> Option<Vec3> {
        let object = self.pick_object(ray).map(|(_, point)| point);
        let ground = GroundPlane.pick(ray);
        match (object, ground) {
            (Some(o), Some(g)) => Some(if o.distance(ray.origin) <= g.distance(ray.origin) {
                o
            } else {
                g
            }),
            (o, g) => o.or(g),
        }
    }

    fn selection_center(&self) -> Option<Vec3> {
        self.doc.selection_bounds().map(|b| b.center())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halberd_doc::Command;
    use halberd_geom::{Aabb, Brush};

    fn doc_with_box(min: Vec3, max: Vec3) -> Document {
        let mut doc = Document::new();
        let brush = Brush::cuboid(Aabb::from_corners(min, max)).unwrap();
        doc.execute(Command::AddBrushes(vec![brush])).unwrap();
        doc
    }

    fn down_from(x: f32, y: f32) -> Ray {
        Ray {
            origin: Vec3::new(x, y, 1000.0),
            direction: Vec3::NEG_Z,
        }
    }

    #[test]
    fn picks_the_top_of_a_box_before_the_grid() {
        let doc = doc_with_box(Vec3::ZERO, Vec3::splat(64.0));
        let scene = DocumentScene::new(&doc);
        assert_eq!(
            scene.pick(&down_from(32.0, 32.0)),
            Some(Vec3::new(32.0, 32.0, 64.0))
        );
        assert_eq!(
            scene.pick(&down_from(100.0, 32.0)),
            Some(Vec3::new(100.0, 32.0, 0.0))
        );
        assert!(scene.pick_object(&down_from(32.0, 32.0)).is_some());
        assert!(scene.pick_object(&down_from(100.0, 32.0)).is_none());
    }

    #[test]
    fn grid_wins_over_a_box_under_the_floor() {
        let doc = doc_with_box(Vec3::new(0.0, 0.0, -128.0), Vec3::new(64.0, 64.0, -64.0));
        let scene = DocumentScene::new(&doc);
        assert_eq!(
            scene.pick(&down_from(32.0, 32.0)),
            Some(Vec3::new(32.0, 32.0, 0.0))
        );
    }

    #[test]
    fn selection_center_is_the_middle_of_the_selection() {
        let mut doc = doc_with_box(Vec3::ZERO, Vec3::new(64.0, 128.0, 32.0));
        assert_eq!(
            DocumentScene::new(&doc).selection_center(),
            Some(Vec3::new(32.0, 64.0, 16.0))
        );
        doc.clear_selection();
        assert_eq!(DocumentScene::new(&doc).selection_center(), None);
    }
}
