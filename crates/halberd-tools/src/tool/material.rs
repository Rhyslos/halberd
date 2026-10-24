//! The material new brushes get, and putting a material on what is
//! selected or picked.

use super::{SelectMode, ToolController};
use crate::elements::editable_brushes;
use halberd_doc::{BrushObject, Command, Document, FaceInfo, Object, ObjectId};
use halberd_geom::Brush;

impl ToolController {
    /// The material new brushes and shapes get (`None`: Halberd's default,
    /// `DEV/DEV_MEASUREGENERIC01B`).
    pub fn material(&self) -> Option<&str> {
        self.material.as_deref()
    }

    /// Sets the material new brushes and shapes get.
    pub fn set_material(&mut self, material: Option<String>) {
        self.material = material.filter(|m| !m.trim().is_empty());
    }

    /// The face data new faces get.
    pub(super) fn new_look(&self) -> FaceInfo {
        match &self.material {
            Some(material) => FaceInfo {
                material: material.clone(),
                ..FaceInfo::default()
            },
            None => FaceInfo::default(),
        }
    }

    /// The edit that adds `brushes` with the current material.
    pub(super) fn new_brushes(&self, brushes: Vec<Brush>) -> Command {
        match &self.material {
            None => Command::AddBrushes(brushes),
            Some(material) => Command::AddBrushObjects(
                brushes
                    .into_iter()
                    .map(|brush| {
                        let mut object = BrushObject::new(brush);
                        object.set_material(None, material);
                        object
                    })
                    .collect(),
            ),
        }
    }

    /// The edit that puts `material` on the picked faces (Face mode, faces
    /// picked), or else on every face of the selected brushes, entities'
    /// brushes and shapes; or why there is nothing to put it on.
    pub fn apply_material(&self, doc: &Document, material: &str) -> Result<Command, String> {
        let faces = self.material_targets(doc);
        if faces.is_empty() {
            return Err("Select brushes, or pick faces (4), to put the material on.".into());
        }
        Ok(Command::SetMaterial {
            material: material.to_string(),
            faces,
        })
    }

    /// What [`ToolController::apply_material`] puts the material on.
    fn material_targets(&self, doc: &Document) -> Vec<(ObjectId, Option<Vec<usize>>)> {
        if self.select_mode == SelectMode::Face && !self.elements.items.is_empty() {
            let brushes = editable_brushes(doc);
            let mut out: Vec<(ObjectId, Option<Vec<usize>>)> = Vec::new();
            for item in &self.elements.items {
                let Some((_, brush)) = brushes.iter().find(|(id, _)| *id == item.brush) else {
                    continue;
                };
                let Some(index) = item.face_index(brush) else {
                    continue;
                };
                match out.iter_mut().find(|(id, _)| *id == item.brush) {
                    Some((_, Some(list))) => list.push(index),
                    _ => out.push((item.brush, Some(vec![index]))),
                }
            }
            return out;
        }
        doc.selection()
            .iter()
            .filter(|&&id| match doc.get(id) {
                Some(Object::Brush(_) | Object::Shape(_)) => true,
                // Point entities have no faces.
                Some(Object::Entity(_)) => doc.has_brushes(id),
                None => false,
            })
            .map(|id| (*id, None))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Camera, Tool, ToolAction, ToolInput};
    use glam::{Vec2, Vec3};
    use halberd_geom::Aabb;

    fn doc_with_cube() -> Document {
        let mut doc = Document::new();
        let cube = Brush::cuboid(Aabb::from_corners(Vec3::ZERO, Vec3::splat(64.0))).unwrap();
        doc.execute(Command::AddBrushes(vec![cube])).unwrap();
        doc
    }

    #[test]
    fn selected_brushes_take_the_material_on_every_face() {
        let doc = doc_with_cube();
        let tools = ToolController::new(16.0);
        let id = doc.objects().next().unwrap().0;
        assert_eq!(
            tools.apply_material(&doc, "BRICK/BRICKWALL001A").unwrap(),
            Command::SetMaterial {
                material: "BRICK/BRICKWALL001A".into(),
                faces: vec![(id, None)],
            }
        );
        let mut empty = doc;
        empty.clear_selection();
        let why = tools.apply_material(&empty, "X").unwrap_err();
        assert!(why.contains("Select brushes"), "{why}");
    }

    #[test]
    fn new_brushes_get_the_chosen_material() {
        let camera = Camera::looking_at(Vec3::new(0.0, -1.0, 2000.0), Vec3::ZERO);
        let size = Vec2::new(1600.0, 900.0);
        let doc = Document::new();
        let mut tools = ToolController::new(16.0);
        tools.set_tool(Tool::Box);
        tools.set_material(Some("DEV/DEV_MEASUREWALL01A".into()));
        assert_eq!(tools.material(), Some("DEV/DEV_MEASUREWALL01A"));
        let at = |p: Vec3| camera.project(p, size).unwrap();
        let input = |cursor, pressed, held, released| ToolInput {
            size,
            cursor: Some(cursor),
            pressed,
            held,
            released,
            ..ToolInput::default()
        };
        let (from, to) = (at(Vec3::ZERO), at(Vec3::new(128.0, 128.0, 0.0)));
        tools.update(&input(from, true, true, false), &camera, &doc);
        tools.update(&input(to, false, true, false), &camera, &doc);
        match tools.update(&input(to, false, false, true), &camera, &doc) {
            Some(ToolAction::Execute(Command::AddBrushObjects(objects))) => {
                assert!(
                    objects[0]
                        .faces()
                        .iter()
                        .all(|f| f.material == "DEV/DEV_MEASUREWALL01A")
                );
            }
            other => panic!("{other:?}"),
        }
        tools.set_material(Some("  ".into()));
        assert_eq!(tools.material(), None, "blank means the default");
    }
}
