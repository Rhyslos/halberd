//! The Sculpt tool in the controller: its settings, and turning the left
//! button into strokes.

use super::{ToolAction, ToolController, ToolInput};
use crate::camera::Camera;
use crate::{SculptSettings, sculpt};
use glam::{Vec2, Vec3};
use halberd_doc::Document;

/// Merge keys of sculpt strokes start here, apart from other drags'.
const STROKE_KEYS: u64 = 0x5C0_0000_0000;

impl ToolController {
    /// The Sculpt tool's settings.
    pub fn sculpt_settings(&self) -> SculptSettings {
        self.sculpt
    }

    /// Changes the Sculpt tool's settings.
    pub fn set_sculpt_settings(&mut self, settings: SculptSettings) {
        let (lo, hi) = SculptSettings::RADIUS_RANGE;
        let (slo, shi) = SculptSettings::STRENGTH_RANGE;
        self.sculpt = SculptSettings {
            radius: settings.radius.clamp(lo, hi),
            strength: settings.strength.clamp(slo, shi),
            ..settings
        };
    }

    /// Where the Sculpt tool's brush is on a displacement, if over one.
    pub fn sculpt_at(&self) -> Option<Vec3> {
        self.sculpt_at.filter(|_| self.tool == super::Tool::Sculpt)
    }

    /// One frame of the Sculpt tool, `dt` seconds long: while the left
    /// button is held over a displacement, a stroke step (one undo step for
    /// the whole drag). `invert` (Ctrl) lowers or paints out.
    pub fn sculpt(
        &mut self,
        input: &ToolInput,
        camera: &Camera,
        doc: &Document,
        invert: bool,
        dt: f32,
    ) -> Option<ToolAction> {
        let cursor = input.cursor.filter(|c| c.is_finite());
        let ray = cursor.map(|c| camera.ray_through(c, input.size.max(Vec2::ONE)));
        self.sculpt_cache.update(doc);
        self.sculpt_at = ray.and_then(|r| sculpt::pick_in(doc, &self.sculpt_cache, &r));
        if input.cancel {
            // Escape takes back the stroke in progress.
            return self.sculpt_stroke.take().map(ToolAction::CancelMerging);
        }
        if !input.held {
            self.sculpt_stroke = None;
            return None;
        }
        // A stroke starts only when the button goes down, so after Escape
        // nothing happens until it is pressed again.
        if input.pressed {
            self.sculpt_strokes += 1;
            self.sculpt_stroke = Some(STROKE_KEYS + self.sculpt_strokes);
        }
        let center = self.sculpt_at?;
        let command = sculpt::stroke(doc, &self.sculpt_cache, center, &self.sculpt, invert, dt)?;
        Some(ToolAction::ExecuteMerging(command, self.sculpt_stroke?))
    }
}

#[cfg(test)]
mod tests {
    use super::super::Tool;
    use super::*;
    use crate::tool::with_displacement;
    use halberd_doc::{Command, Displacement, ordered_corners};
    use halberd_geom::{Aabb, Brush};

    const SIZE: Vec2 = Vec2::new(1600.0, 900.0);

    /// A 512-unit box whose top is a flat displacement, and its id.
    fn terrain() -> (Document, halberd_doc::ObjectId) {
        let mut doc = Document::new();
        let cube = Brush::cuboid(Aabb::from_corners(
            Vec3::new(0.0, 0.0, -16.0),
            Vec3::new(512.0, 512.0, 0.0),
        ))
        .unwrap();
        doc.execute(Command::AddBrushes(vec![cube])).unwrap();
        let id = *doc.selection().iter().next().unwrap();
        let object = doc.get(id).unwrap().as_brush().unwrap().clone();
        let top = object
            .brush()
            .faces()
            .iter()
            .position(|f| f.plane().normal.z > 0.9)
            .unwrap();
        let shape = &object.brush().faces()[top];
        let corners = ordered_corners(shape.vertices(), Vec3::Z, shape.vertices()[0]).unwrap();
        let disp = Displacement::flat(3, corners[0], Vec3::Z);
        doc.execute(Command::ShapeEdit {
            label: "Make displacement".into(),
            changed: vec![(id, with_displacement(&object, top, Some(&disp)))],
            added: vec![],
            removed: vec![],
            select_added: false,
        })
        .unwrap();
        doc.end_step();
        (doc, id)
    }

    fn camera() -> Camera {
        Camera::looking_at(Vec3::new(256.0, 255.0, 900.0), Vec3::new(256.0, 256.0, 0.0))
    }

    fn frame(pressed: bool, held: bool) -> ToolInput {
        ToolInput {
            size: SIZE,
            cursor: camera().project(Vec3::new(256.0, 256.0, 0.0), SIZE),
            pressed,
            held,
            dt: 0.05,
            ..ToolInput::default()
        }
    }

    fn sculpt_tool() -> ToolController {
        let mut t = ToolController::new(16.0);
        t.set_tool(Tool::Sculpt);
        t
    }

    /// One frame of the tool; the stroke's edit is carried out.
    fn apply(doc: &mut Document, t: &mut ToolController, input: ToolInput) -> u64 {
        match t.update(&input, &camera(), doc) {
            Some(ToolAction::ExecuteMerging(command, key)) => {
                doc.execute_merging(command, key).unwrap();
                key
            }
            other => panic!("expected a stroke, got {other:?}"),
        }
    }

    fn middle_height(doc: &Document, id: halberd_doc::ObjectId) -> f32 {
        let b = doc.get(id).unwrap().as_brush().unwrap();
        b.faces()
            .iter()
            .find_map(|f| f.displacement())
            .unwrap()
            .distances[40]
    }

    #[test]
    fn a_held_stroke_is_one_undo_step() {
        let (mut doc, id) = terrain();
        let mut t = sculpt_tool();
        let cam = camera();
        let first = apply(&mut doc, &mut t, frame(true, true));
        let second = apply(&mut doc, &mut t, frame(false, true));
        assert_eq!(first, second, "one stroke");
        assert!(middle_height(&doc, id) > 6.0);
        assert!(t.sculpt_at().is_some(), "the brush circle shows");
        assert_eq!(t.update(&frame(false, false), &cam, &doc), None);
        doc.end_step();
        let next = apply(&mut doc, &mut t, frame(true, true));
        assert_ne!(next, first, "a new press is a new stroke");
        doc.end_step();
        doc.undo();
        doc.undo();
        assert_eq!(middle_height(&doc, id), 0.0);
    }

    #[test]
    fn escape_takes_the_stroke_back() {
        let (mut doc, id) = terrain();
        let mut t = sculpt_tool();
        let cam = camera();
        let key = apply(&mut doc, &mut t, frame(true, true));
        let cancel = ToolInput {
            cancel: true,
            ..frame(false, true)
        };
        assert_eq!(
            t.update(&cancel, &cam, &doc),
            Some(ToolAction::CancelMerging(key))
        );
        doc.discard_step(key);
        assert_eq!(middle_height(&doc, id), 0.0);
        // Still holding the button does nothing until it is pressed again.
        assert_eq!(t.update(&frame(false, true), &cam, &doc), None);
    }

    #[test]
    fn changing_tool_mid_stroke_ends_it() {
        let (mut doc, _) = terrain();
        let mut t = sculpt_tool();
        apply(&mut doc, &mut t, frame(true, true));
        assert!(t.is_busy());
        t.set_tool(Tool::Select);
        assert!(!t.is_busy(), "nothing is left waiting");
        assert_eq!(t.sculpt_at(), None);
    }

    #[test]
    fn settings_are_kept_in_range() {
        let mut t = sculpt_tool();
        t.set_sculpt_settings(SculptSettings {
            radius: 1.0e9,
            strength: -5.0,
            ..SculptSettings::default()
        });
        let s = t.sculpt_settings();
        assert_eq!(s.radius, SculptSettings::RADIUS_RANGE.1);
        assert_eq!(s.strength, SculptSettings::STRENGTH_RANGE.0);
        assert_eq!(t.sculpt_at(), None);
    }
}
