//! Turning picked faces into displacements and back.

use super::ToolController;
use super::operations::{SelectAfter, ShapeOpOutcome};
use crate::elements::ElementShape;
use halberd_doc::{BrushObject, Command, Displacement, Document, ObjectId, ordered_corners};

/// The grid power new displacements get (9 × 9 points), as in Hammer.
pub(crate) const NEW_DISP_POWER: u8 = 3;

impl ToolController {
    /// Makes each picked four-sided face a flat displacement (faces that
    /// already are one keep theirs).
    pub(super) fn make_displacements(&self, doc: &Document) -> Result<ShapeOpOutcome, String> {
        let mut changed: Vec<(ObjectId, BrushObject)> = Vec::new();
        let mut skipped = 0;
        let mut made = 0;
        for (id, object, face) in self.picked_faces(doc) {
            let shape = &object.brush().faces()[face];
            let info = &object.faces()[face];
            if info.displacement().is_some() {
                continue;
            }
            let normal = shape.plane().normal;
            let Some(first) = shape.vertices().first() else {
                continue;
            };
            let Some(corners) = ordered_corners(shape.vertices(), normal, *first) else {
                skipped += 1;
                continue;
            };
            let disp = Displacement::flat(NEW_DISP_POWER, corners[0], normal);
            let base = changed
                .iter()
                .find(|(c, _)| *c == id)
                .map_or_else(|| object.clone(), |(_, b)| b.clone());
            let updated = with_face(&base, face, Some(&disp));
            changed.retain(|(c, _)| *c != id);
            changed.push((id, updated));
            made += 1;
        }
        if made == 0 {
            return Err(if skipped > 0 {
                "Only four-sided faces can be displacements; the picked faces have more or \
                 fewer corners."
                    .into()
            } else {
                "The picked faces are displacements already.".into()
            });
        }
        let mut outcome = edit(
            changed,
            &format!("Make {made} displacement{}", plural(made)),
            self.picked_shapes(),
        );
        if skipped > 0 {
            outcome.note = Some(format!(
                "{skipped} picked face{} not four-sided, so {} left as {}.",
                if skipped == 1 { " is" } else { "s are" },
                if skipped == 1 { "it is" } else { "they are" },
                if skipped == 1 { "it was" } else { "they were" },
            ));
        }
        Ok(outcome)
    }

    /// Turns each picked displacement face back into a plain face.
    pub(super) fn remove_displacements(&self, doc: &Document) -> Result<ShapeOpOutcome, String> {
        let mut changed: Vec<(ObjectId, BrushObject)> = Vec::new();
        let mut removed = 0;
        for (id, object, face) in self.picked_faces(doc) {
            if object.faces()[face].displacement().is_none() {
                continue;
            }
            let base = changed
                .iter()
                .find(|(c, _)| *c == id)
                .map_or_else(|| object.clone(), |(_, b)| b.clone());
            let updated = with_face(&base, face, None);
            changed.retain(|(c, _)| *c != id);
            changed.push((id, updated));
            removed += 1;
        }
        if removed == 0 {
            return Err("None of the picked faces is a displacement.".into());
        }
        Ok(edit(
            changed,
            &format!("Remove {removed} displacement{}", plural(removed)),
            self.picked_shapes(),
        ))
    }
}

impl ToolController {
    /// The picked parts, to pick again after the edit (the faces stay
    /// picked, ready for the next step).
    fn picked_shapes(&self) -> Vec<ElementShape> {
        self.elements
            .items
            .iter()
            .map(|i| i.shape.clone())
            .collect()
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// `object` with face `face`'s displacement set (or taken away).
pub(crate) fn with_face(
    object: &BrushObject,
    face: usize,
    disp: Option<&Displacement>,
) -> BrushObject {
    let mut faces = object.faces().to_vec();
    if let Some(info) = faces.get_mut(face) {
        *info = info.with_displacement(disp);
    }
    BrushObject::with_faces(object.brush().clone(), faces, object.file_data.clone())
}

fn edit(
    changed: Vec<(ObjectId, BrushObject)>,
    label: &str,
    pick_after: Vec<ElementShape>,
) -> ShapeOpOutcome {
    let mut outcome = ShapeOpOutcome::new(Command::ShapeEdit {
        label: label.to_string(),
        changed,
        added: Vec::new(),
        removed: Vec::new(),
        select_added: false,
    });
    outcome.select = SelectAfter::Same;
    outcome.pick_after = pick_after;
    outcome
}

#[cfg(test)]
mod tests {
    use super::super::{ShapeOp, ToolController};
    use crate::Camera;
    use crate::elements::{ElementShape, PickedElement, SelectMode};
    use glam::Vec3;
    use halberd_doc::{Command, Document, ObjectId};
    use halberd_geom::{Aabb, Brush};

    fn map() -> (Document, ObjectId) {
        let mut doc = Document::new();
        let cube = Brush::cuboid(Aabb::from_corners(
            Vec3::ZERO,
            Vec3::new(256.0, 128.0, 64.0),
        ));
        doc.execute(Command::AddBrushes(vec![cube.unwrap()]))
            .unwrap();
        doc.end_step();
        let id = *doc.selection().iter().next().unwrap();
        (doc, id)
    }

    /// Face-mode tools with the faces facing `normals` picked.
    fn picking(doc: &Document, id: ObjectId, normals: &[Vec3]) -> ToolController {
        let mut t = ToolController::new(16.0);
        t.set_select_mode(SelectMode::Face);
        let b = doc.get(id).unwrap().as_brush().unwrap().brush();
        t.elements.items = b
            .faces()
            .iter()
            .filter(|f| normals.iter().any(|n| f.plane().normal.dot(*n) > 0.9))
            .map(|f| PickedElement {
                brush: id,
                shape: ElementShape::Face(f.vertices().to_vec()),
            })
            .collect();
        t
    }

    fn disps(doc: &Document, id: ObjectId) -> usize {
        let b = doc.get(id).unwrap().as_brush().unwrap();
        b.faces()
            .iter()
            .filter(|f| f.displacement().is_some())
            .count()
    }

    fn camera() -> Camera {
        Camera::looking_at(Vec3::new(500.0, -700.0, 600.0), Vec3::ZERO)
    }

    #[test]
    fn picked_faces_become_displacements_and_back() {
        let (mut doc, id) = map();
        let mut t = picking(&doc, id, &[Vec3::Z, Vec3::X]);
        assert!(t.shape_op_ready(ShapeOp::MakeDisplacement, &doc).is_ok());
        t.apply_shape_op(ShapeOp::MakeDisplacement, &mut doc, &camera())
            .unwrap();
        assert_eq!(disps(&doc, id), 2);
        assert_eq!(doc.undo_label(), Some("Make 2 displacements"));
        let b = doc.get(id).unwrap().as_brush().unwrap();
        let top = b.faces().iter().find_map(|f| f.displacement()).unwrap();
        assert_eq!(top.power, super::NEW_DISP_POWER);
        assert!(top.distances.iter().all(|d| *d == 0.0), "made flat");
        // Again: they are displacements already.
        let again = t.apply_shape_op(ShapeOp::MakeDisplacement, &mut doc, &camera());
        let again = again.unwrap_err();
        assert!(again.contains("already"), "{again}");
        let mut t = picking(&doc, id, &[Vec3::Z]);
        t.apply_shape_op(ShapeOp::RemoveDisplacement, &mut doc, &camera())
            .unwrap();
        assert_eq!(disps(&doc, id), 1);
        doc.undo();
        doc.undo();
        assert_eq!(disps(&doc, id), 0);
    }

    #[test]
    fn removing_where_there_is_none_says_so() {
        let (mut doc, id) = map();
        let mut t = picking(&doc, id, &[Vec3::Z]);
        let err = t
            .apply_shape_op(ShapeOp::RemoveDisplacement, &mut doc, &camera())
            .unwrap_err();
        assert!(err.ends_with('.'));
        let none = ToolController::new(16.0);
        assert!(
            none.shape_op_ready(ShapeOp::MakeDisplacement, &doc)
                .is_err(),
            "needs picked faces"
        );
    }
}
