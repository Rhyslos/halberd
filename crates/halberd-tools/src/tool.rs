//! Left-mouse tools: selecting, and drawing box brushes.
//!
//! Tools never change the map themselves. They return a [`ToolAction`],
//! and the interface carries it out (edits through the document's
//! commands, so they can be undone).

use crate::SceneQuery;
use crate::camera::{Camera, Ray};
use crate::gizmo::{Gizmo, GizmoMode, GizmoOutcome, GizmoShape};
use crate::scene::DocumentScene;
use glam::Vec2;
use halberd_doc::{Command, Document, ObjectId};
use halberd_geom::{Aabb, Brush, GeomError};

/// Height of a newly drawn box, in units: comfortably taller than a GMod
/// player (72 units), like a standard wall.
pub const DEFAULT_BOX_HEIGHT: f32 = 128.0;
/// Limits for the height of new boxes, in units.
pub const BOX_HEIGHT_RANGE: (f32, f32) = (1.0, 16_384.0);
/// How far the pointer may move between press and release, in points, and
/// still count as a click rather than a drag.
pub const CLICK_SLOP: f32 = 4.0;

/// What the left mouse button does in the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    /// Click to select; Ctrl+click to add or remove.
    #[default]
    Select,
    /// Drag on the grid or on a brush to draw a box.
    Box,
}

impl Tool {
    /// Every tool, in the order the switcher shows them.
    pub const ALL: [Tool; 2] = [Self::Select, Self::Box];

    /// Short name for the tool switcher.
    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Box => "Box",
        }
    }

    /// One-line explanation, for tooltips.
    pub fn description(self) -> &'static str {
        match self {
            Self::Select => "Click to select; Ctrl+click to add or remove (Esc)",
            Self::Box => {
                "Drag on the grid or on a brush to draw a box; drag along a line for a wall (B)"
            }
        }
    }
}

/// One frame of left-mouse input, in points from the viewport's top-left.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ToolInput {
    /// Size of the viewport.
    pub size: Vec2,
    /// Where the pointer is, if over the viewport or dragging in it.
    pub cursor: Option<Vec2>,
    /// The left button went down on the viewport this frame.
    pub pressed: bool,
    /// The left button is held after going down on the viewport.
    pub held: bool,
    /// The left button came up this frame after going down on the viewport.
    pub released: bool,
    /// Ctrl is held: clicks add to or remove from the selection.
    pub additive: bool,
    /// Escape was pressed: cancel what is in progress.
    pub cancel: bool,
}

/// What a tool asks the interface to do.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolAction {
    /// Select only this object, or nothing.
    Select(Option<ObjectId>),
    /// Add the object to the selection, or take it out.
    ToggleSelected(ObjectId),
    /// Carry out this edit.
    Execute(Command),
    /// Carry out this edit as part of a drag: every edit with the same key
    /// joins one undo step.
    ExecuteMerging(Command, u64),
    /// A drag was cancelled: reverse and forget the edits made with this key.
    CancelMerging(u64),
    /// Something could not be done; tell the user why.
    Refused(String),
}

/// A box being drawn: its first corner and the height it stands on.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BoxDrag {
    start: Vec2,
    base: f32,
    end: Vec2,
}

/// Runs the active tool.
#[derive(Debug, Clone)]
pub struct ToolController {
    tool: Tool,
    grid: f32,
    box_height: f32,
    press_at: Option<Vec2>,
    drag: Option<BoxDrag>,
    gizmo: Gizmo,
}

impl ToolController {
    /// A controller snapping to `grid_size` units.
    pub fn new(grid_size: f32) -> Self {
        Self {
            tool: Tool::default(),
            grid: sane_grid(grid_size),
            box_height: DEFAULT_BOX_HEIGHT,
            press_at: None,
            drag: None,
            gizmo: Gizmo::default(),
        }
    }

    /// The active tool.
    pub fn tool(&self) -> Tool {
        self.tool
    }

    /// Switches tool, cancelling anything in progress, except during a
    /// gizmo drag, when it does nothing. The Box tool hides the gizmo.
    pub fn set_tool(&mut self, tool: Tool) {
        // A gizmo drag must finish (or be cancelled with Escape) first.
        if self.gizmo.is_dragging() {
            return;
        }
        self.tool = tool;
        self.press_at = None;
        self.drag = None;
        if tool == Tool::Box {
            self.gizmo.set_mode(None);
        }
    }

    /// The gizmo mode, or `None` for plain selection.
    pub fn gizmo_mode(&self) -> Option<GizmoMode> {
        self.gizmo.mode
    }

    /// Shows the gizmo in `mode` (switching to the Select tool), or hides
    /// it with `None`. Has no effect during a gizmo drag.
    pub fn set_gizmo_mode(&mut self, mode: Option<GizmoMode>) {
        if self.gizmo.is_dragging() {
            return;
        }
        if mode.is_some() {
            self.set_tool(Tool::Select);
        }
        self.gizmo.set_mode(mode);
    }

    /// Picks `mode`, or returns to plain selection if it was already picked
    /// (the W / R / S / T keys and toolbar buttons are toggles).
    pub fn toggle_gizmo_mode(&mut self, mode: GizmoMode) {
        let next = (self.gizmo.mode != Some(mode)).then_some(mode);
        self.set_gizmo_mode(next);
    }

    /// The gizmo's shapes for drawing, in points from the viewport's
    /// top-left corner. Empty when there is no gizmo to show.
    pub fn gizmo_shapes(&self, camera: &Camera, size: Vec2, doc: &Document) -> Vec<GizmoShape> {
        if self.tool != Tool::Select {
            return Vec::new();
        }
        self.gizmo.shapes(camera, size.max(Vec2::ONE), doc)
    }

    /// The grid size boxes snap to.
    pub fn grid_size(&self) -> f32 {
        self.grid
    }

    /// Height of new boxes, in units.
    pub fn box_height(&self) -> f32 {
        self.box_height
    }

    /// Sets the height of new boxes, in units, kept within
    /// [`BOX_HEIGHT_RANGE`] and rounded to whole units.
    pub fn set_box_height(&mut self, height: f32) {
        let (low, high) = BOX_HEIGHT_RANGE;
        if height.is_finite() {
            self.box_height = height.round().clamp(low, high);
        }
    }

    /// True while something is in progress that Escape would cancel.
    pub fn is_busy(&self) -> bool {
        self.drag.is_some() || self.gizmo.is_dragging()
    }

    /// The box being drawn, for the preview outline.
    pub fn preview(&self) -> Option<Aabb> {
        self.box_bounds(self.drag?)
    }

    /// The box a drag would make. A drag along one grid line makes a wall
    /// one grid square thick; a drag that barely moved makes nothing.
    fn box_bounds(&self, drag: BoxDrag) -> Option<Aabb> {
        let half = self.grid * 0.5;
        let moved = (drag.end - drag.start).abs();
        if moved.max_element() < half {
            return None;
        }
        let thicken = |start: f32, end: f32| {
            if (end - start).abs() < half {
                start + self.grid
            } else {
                end
            }
        };
        let end = Vec2::new(
            thicken(drag.start.x, drag.end.x),
            thicken(drag.start.y, drag.end.y),
        );
        Some(Aabb::from_corners(
            drag.start.extend(drag.base),
            end.extend(drag.base + self.box_height),
        ))
    }

    /// Applies one frame of input.
    pub fn update(
        &mut self,
        input: &ToolInput,
        camera: &Camera,
        doc: &Document,
    ) -> Option<ToolAction> {
        let cursor = input.cursor.filter(|c| c.is_finite());
        if self.tool == Tool::Select {
            let size = input.size.max(Vec2::ONE);
            let input = ToolInput { size, ..*input };
            match self.gizmo.update(&input, cursor, camera, doc, self.grid) {
                GizmoOutcome::Used(action) => {
                    self.press_at = None;
                    return action;
                }
                GizmoOutcome::NotMine => {}
            }
        }
        if input.cancel && (self.drag.is_some() || self.press_at.is_some()) {
            self.press_at = None;
            self.drag = None;
            return None;
        }
        let ray = cursor.map(|c| camera.ray_through(c, input.size.max(Vec2::ONE)));
        match self.tool {
            Tool::Select => self.update_select(input, cursor, ray.as_ref(), doc),
            Tool::Box => self.update_box(input, ray.as_ref(), doc),
        }
    }

    fn update_select(
        &mut self,
        input: &ToolInput,
        cursor: Option<Vec2>,
        ray: Option<&Ray>,
        doc: &Document,
    ) -> Option<ToolAction> {
        if input.pressed {
            self.press_at = cursor;
        }
        if !input.released {
            return None;
        }
        let pressed_at = self.press_at.take()?;
        let cursor = cursor?;
        if cursor.distance(pressed_at) > CLICK_SLOP {
            return None;
        }
        let hit = ray.and_then(|r| DocumentScene::new(doc).pick_object(r));
        match (hit, input.additive) {
            (Some((id, _)), true) => Some(ToolAction::ToggleSelected(id)),
            (None, true) => None,
            (hit, false) => Some(ToolAction::Select(hit.map(|(id, _)| id))),
        }
    }

    fn update_box(
        &mut self,
        input: &ToolInput,
        ray: Option<&Ray>,
        doc: &Document,
    ) -> Option<ToolAction> {
        if input.pressed
            && let Some(point) = ray.and_then(|r| DocumentScene::new(doc).pick(r))
        {
            let start = self.snap(point.truncate());
            self.drag = Some(BoxDrag {
                start,
                base: self.snap_one(point.z),
                end: start,
            });
        }
        if let Some(drag) = self.drag.as_mut()
            && let Some(ray) = ray
            && let Some(distance) = ray.hit_horizontal_plane(drag.base)
        {
            let point = ray.at(distance);
            if point.distance(ray.origin) <= crate::MAX_PICK_DISTANCE {
                drag.end = (point.truncate() / self.grid).round() * self.grid;
            }
        }
        if input.released || !input.held {
            let drag = self.drag.take()?;
            return self.finish_box(drag);
        }
        None
    }

    fn finish_box(&self, drag: BoxDrag) -> Option<ToolAction> {
        let bounds = self.box_bounds(drag)?;
        Some(match Brush::cuboid(bounds) {
            Ok(brush) => ToolAction::Execute(Command::AddBrushes(vec![brush])),
            Err(GeomError::TooLarge) => {
                ToolAction::Refused("The box would reach beyond the edge of the world.".into())
            }
            Err(e) => ToolAction::Refused(format!("No box was made: {e}.")),
        })
    }

    fn snap(&self, v: Vec2) -> Vec2 {
        (v / self.grid).round() * self.grid
    }

    fn snap_one(&self, x: f32) -> f32 {
        (x / self.grid).round() * self.grid
    }
}

/// Keeps the grid size usable whatever the settings file says.
fn sane_grid(grid: f32) -> f32 {
    if grid.is_finite() && grid >= 1.0 {
        grid.min(4096.0)
    } else {
        16.0
    }
}

#[cfg(test)]
mod tests;
