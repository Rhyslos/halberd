//! Drawing a floor plan with the Draw tool: click corner after corner on
//! the grid (or on a brush's top), then click the first corner again or
//! press Enter. The outline may have dents; the shape is kept whole and
//! split into convex brushes only when the map is saved.

use super::{ToolAction, ToolController, ToolInput};
use crate::SceneQuery;
use crate::camera::Ray;
use crate::scene::DocumentScene;
use glam::{Vec2, Vec3};
use halberd_doc::{Command, Document, FaceInfo, ShapeObject};

/// A floor plan being drawn.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PlanDraft {
    /// The height it stands on: where the first click landed.
    base: f32,
    /// The corners placed so far, snapped to the grid.
    corners: Vec<Vec2>,
    /// Where the next corner would go, under the pointer.
    next: Option<Vec2>,
}

/// A floor plan being drawn, for the viewport to show.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanPreview {
    /// The corners placed so far, at the height the plan stands on.
    pub corners: Vec<Vec3>,
    /// Where the next corner would go.
    pub next: Option<Vec3>,
    /// True if the next click closes the outline (it is on the first corner).
    pub closes: bool,
}

impl ToolController {
    /// True while a floor plan is being drawn.
    pub fn is_drawing_plan(&self) -> bool {
        self.plan.is_some()
    }

    /// The floor plan being drawn, if one is.
    pub fn plan_preview(&self) -> Option<PlanPreview> {
        let plan = self.plan.as_ref()?;
        Some(PlanPreview {
            corners: plan.corners.iter().map(|c| c.extend(plan.base)).collect(),
            next: plan.next.map(|n| n.extend(plan.base)),
            closes: self.closes(plan),
        })
    }

    /// Takes back the last corner placed (Backspace). Drawing stops when
    /// none are left. Returns false if no plan is being drawn.
    pub fn undo_plan_corner(&mut self) -> bool {
        let Some(plan) = self.plan.as_mut() else {
            return false;
        };
        plan.corners.pop();
        if plan.corners.is_empty() {
            self.plan = None;
        }
        true
    }

    /// Finishes the floor plan being drawn (Enter): the shape to add, or
    /// why it can't be made. `None` if no plan is being drawn.
    pub fn finish_plan(&mut self) -> Option<ToolAction> {
        let plan = self.plan.take()?;
        Some(self.make_plan(&plan))
    }

    /// One frame of left-mouse input while the Draw tool draws floor plans.
    pub(super) fn update_plan(
        &mut self,
        input: &ToolInput,
        ray: Option<&Ray>,
        doc: &Document,
    ) -> Option<ToolAction> {
        if input.cancel {
            self.plan = None;
            return None;
        }
        // The pointer, on the height the plan stands on.
        if let Some(plan) = self.plan.as_mut() {
            plan.next = ray
                .and_then(|r| Some(r.at(r.hit_horizontal_plane(plan.base)?)))
                .filter(|p| p.is_finite())
                .map(|p| (p.truncate() / self.grid).round() * self.grid);
        }
        if !input.pressed {
            return None;
        }
        let Some(plan) = self.plan.as_mut() else {
            // The first corner: on the grid, or on top of what is clicked.
            let point = ray.and_then(|r| DocumentScene::new(doc).pick(r))?;
            let corner = (point.truncate() / self.grid).round() * self.grid;
            self.plan = Some(PlanDraft {
                base: (point.z / self.grid).round() * self.grid,
                corners: vec![corner],
                next: Some(corner),
            });
            return None;
        };
        let corner = plan.next?;
        if plan.corners.len() >= 3 && corner == plan.corners[0] {
            let plan = self.plan.take()?;
            return Some(self.make_plan(&plan));
        }
        if plan.corners.last() != Some(&corner) {
            plan.corners.push(corner);
        }
        None
    }

    /// True if the pointer is on the first corner of a plan that could close.
    fn closes(&self, plan: &PlanDraft) -> bool {
        plan.corners.len() >= 3 && plan.next == plan.corners.first().copied()
    }

    /// The shape a finished plan makes, as tall as the Height field.
    fn make_plan(&self, plan: &PlanDraft) -> ToolAction {
        if plan.corners.len() < 3 {
            return ToolAction::Refused(
                "No floor plan was made: it needs at least three corners.".into(),
            );
        }
        let top = plan.base + self.box_height;
        match ShapeObject::new(&plan.corners, plan.base, top, FaceInfo::default()) {
            Ok(shape) => ToolAction::Execute(Command::AddShape(shape)),
            Err(e) => ToolAction::Refused(format!("No floor plan was made: {e}.")),
        }
    }
}

