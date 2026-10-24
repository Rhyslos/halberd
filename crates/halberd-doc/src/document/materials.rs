//! Putting materials on faces.

use super::{Change, DocError, Document};
use crate::{Object, ObjectId};
use std::collections::BTreeMap;

impl Document {
    /// Checks a [`crate::Command::SetMaterial`]: a brush entity's faces are
    /// its brushes'; objects whose faces already have the material are
    /// left alone.
    pub(super) fn prepare_set_material(
        &self,
        material: &str,
        faces: Vec<(ObjectId, Option<Vec<usize>>)>,
    ) -> Result<Change, DocError> {
        if material.trim().is_empty() {
            return Err(DocError::NothingToDo);
        }
        // Brushes of selected entities, with all their faces.
        let mut wanted: BTreeMap<ObjectId, Option<Vec<usize>>> = BTreeMap::new();
        for (id, picked) in faces {
            let object = self.objects.get(&id).ok_or(DocError::UnknownObject(id))?;
            if matches!(object, Object::Entity(_)) {
                for brush in self.brushes_of(id) {
                    wanted.insert(brush, None);
                }
            } else {
                wanted.insert(id, picked);
            }
        }
        let mut modified = Vec::new();
        for (id, picked) in wanted {
            let before = self.objects.get(&id).ok_or(DocError::UnknownObject(id))?;
            let mut after = before.clone();
            let changed = match &mut after {
                Object::Brush(brush) => match &picked {
                    None => brush.set_material(None, material),
                    Some(list) => list
                        .iter()
                        .fold(false, |any, &i| brush.set_material(Some(i), material) | any),
                },
                Object::Shape(shape) => {
                    let changed = shape.look.material != material;
                    shape.look.material = material.to_string();
                    changed
                }
                Object::Entity(_) => false,
            };
            if changed {
                modified.push((id, before.clone(), after));
            }
        }
        if modified.is_empty() {
            return Err(DocError::NothingToDo);
        }
        Ok(Change::Modified(modified))
    }
}

#[cfg(test)]
mod tests {
    use crate::{BrushObject, Command, Document, EntityObject, FaceInfo, MapObject, Object};
    use glam::{Vec2, Vec3};
    use halberd_geom::{Aabb, Brush};

    fn cube(x: f32) -> Brush {
        Brush::cuboid(Aabb::from_corners(Vec3::splat(x), Vec3::splat(x + 64.0))).unwrap()
    }

    fn materials(doc: &Document) -> Vec<Vec<String>> {
        doc.objects()
            .filter_map(|(_, o)| o.as_brush())
            .map(|b| b.faces().iter().map(|f| f.material.clone()).collect())
            .collect()
    }

    #[test]
    fn a_material_goes_on_whole_brushes_or_picked_faces_and_undoes() {
        let mut doc = Document::new();
        doc.execute(Command::AddBrushes(vec![cube(0.0), cube(100.0)]))
            .unwrap();
        let ids: Vec<_> = doc.objects().map(|(id, _)| id).collect();
        doc.end_step();
        doc.execute(Command::SetMaterial {
            material: "BRICK/BRICKWALL001A".into(),
            faces: vec![(ids[0], None), (ids[1], Some(vec![2]))],
        })
        .unwrap();
        assert_eq!(doc.undo_label(), Some("Apply material"));
        let after = materials(&doc);
        assert!(after[0].iter().all(|m| m == "BRICK/BRICKWALL001A"));
        assert_eq!(
            after[1]
                .iter()
                .filter(|m| *m == "BRICK/BRICKWALL001A")
                .count(),
            1
        );
        assert_eq!(after[1][2], "BRICK/BRICKWALL001A");
        doc.undo();
        assert!(
            materials(&doc)
                .iter()
                .flatten()
                .all(|m| m == FaceInfo::default().material.as_str())
        );
        // Putting on what is already there changes nothing.
        doc.redo();
        doc.end_step();
        let again = doc.execute(Command::SetMaterial {
            material: "BRICK/BRICKWALL001A".into(),
            faces: vec![(ids[0], None)],
        });
        assert!(again.is_err());
    }

    #[test]
    fn entities_pass_it_to_their_brushes_and_shapes_take_it_as_their_look() {
        let entity = MapObject::Entity(
            EntityObject {
                classname: "func_detail".into(),
                origin: None,
                file_data: Vec::new(),
            },
            vec![BrushObject::new(cube(0.0)), BrushObject::new(cube(100.0))],
        );
        let mut doc = Document::from_map(vec![entity], Default::default()).unwrap();
        let detail = doc
            .objects()
            .find(|(_, o)| matches!(o, Object::Entity(_)))
            .unwrap()
            .0;
        let outline = [Vec2::ZERO, Vec2::new(64.0, 0.0), Vec2::new(0.0, 64.0)];
        let shape = crate::ShapeObject::new(&outline, 0.0, 32.0, FaceInfo::default()).unwrap();
        doc.execute(Command::AddShape(shape)).unwrap();
        let shape_id = doc
            .objects()
            .find(|(_, o)| o.as_shape().is_some())
            .unwrap()
            .0;
        doc.execute(Command::SetMaterial {
            material: "CONCRETE/CONCRETEFLOOR001A".into(),
            faces: vec![(detail, None), (shape_id, None)],
        })
        .unwrap();
        assert!(
            materials(&doc)
                .iter()
                .flatten()
                .all(|m| m == "CONCRETE/CONCRETEFLOOR001A")
        );
        assert_eq!(
            doc.get(shape_id).unwrap().as_shape().unwrap().look.material,
            "CONCRETE/CONCRETEFLOOR001A"
        );
    }

    #[test]
    fn new_brushes_can_carry_a_material() {
        let mut doc = Document::new();
        let mut brush = BrushObject::new(cube(0.0));
        brush.set_material(None, "DEV/DEV_MEASUREWALL01A");
        doc.execute(Command::AddBrushObjects(vec![brush])).unwrap();
        assert_eq!(doc.undo_label(), Some("Create brush"));
        assert!(
            materials(&doc)[0]
                .iter()
                .all(|m| m == "DEV/DEV_MEASUREWALL01A")
        );
        assert_eq!(doc.selection().len(), 1);
    }
}
