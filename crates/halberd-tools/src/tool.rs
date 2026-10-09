//! Left-mouse tools: selecting, and drawing shapes (boxes, wedges,
//! cylinders, cones, spheres, arches, stairs).
//!
//! Tools never change the map themselves. They return a [`ToolAction`],
//! and the interface carries it out (edits through the document's
//! commands, so they can be undone).

use crate::SceneQuery;
use crate::camera::{Camera, Ray};
use crate::elements::{self, ElementOverlay, Elements, PickedElement, SelectMode};
use crate::gizmo::{Gizmo, GizmoMode, GizmoOutcome, GizmoShape};
use crate::scene::DocumentScene;
use glam::Vec2;
use halberd_doc::{Command, Document, ObjectId};
use halberd_geom::{Aabb, Brush, GeomError, Heading, Shape, ShapeSettings, build_shape};
use std::cell::RefCell;

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
    /// Drag on the grid or on a brush to draw the chosen shape (a box at
    /// first).
    Box,
}

impl Tool {
    /// Every tool, in the order the switcher shows them.
    pub const ALL: [Tool; 2] = [Self::Select, Self::Box];

    /// Short name for the tool switcher.
    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Box => "Draw",
        }
    }

    /// One-line explanation, for tooltips.
    pub fn description(self) -> &'static str {
        match self {
            Self::Select => {
                "Click to select; Ctrl+click to add or remove; Esc deselects. 1 to 4 choose \
                 what clicks pick: objects, corners, edges or faces (Q)"
            }
            Self::Box => {
                "Drag on the grid or on a brush to draw the chosen shape; drag along a line \
                 for a wall. Wedges and stairs climb, and arches span, the way you drag; R \
                 while drawing turns the shape. Shift: as wide as long and tall; Alt: the \
                 same, around where you started. B again lists the shapes (B)"
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
    /// Shift is held: a shape being drawn is as wide, deep and tall as its
    /// longest side, from the corner where the drag started.
    pub uniform: bool,
    /// Alt is held: like `uniform`, but the drag's start is the middle of
    /// the shape's ground plan.
    pub centered: bool,
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
    /// Quarter turns clockwise (seen from above) given with R.
    turns: u8,
    /// Shift or Alt as last held (see [`ToolInput`]).
    uniform: bool,
    centered: bool,
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
    /// Clicks pick single brushes inside brush entities (Hammer's
    /// "Ignore groups") instead of the whole entity.
    inside_entities: bool,
    /// What the Draw tool makes, and its settings.
    shape: Shape,
    shape_settings: ShapeSettings,
    /// The last shape built for the preview, kept while nothing that makes
    /// it changes (the preview is asked for every frame).
    built: RefCell<Option<(ShapeKey, Built)>>,
    /// What clicks pick in the Select tool: objects, or parts of brushes.
    select_mode: SelectMode,
    /// The picked corners, edges or faces.
    elements: Elements,
    /// The part under the pointer, for highlighting.
    hovered_element: Option<PickedElement>,
}

/// What a shape is built from.
type ShapeKey = (Aabb, Heading, Shape, ShapeSettings);
/// A built shape, or why not (`None`: the drag is too short to count).
type Built = Result<Vec<Brush>, Option<GeomError>>;

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
            inside_entities: false,
            shape: Shape::Box,
            shape_settings: ShapeSettings::default(),
            built: RefCell::new(None),
            select_mode: SelectMode::Object,
            elements: Elements::default(),
            hovered_element: None,
        }
    }

    /// What clicks pick in the Select tool.
    pub fn select_mode(&self) -> SelectMode {
        self.select_mode
    }

    /// Picks what clicks pick (switching to the Select tool), forgetting
    /// any picked parts. Has no effect during a gizmo drag.
    pub fn set_select_mode(&mut self, mode: SelectMode) {
        if self.gizmo.is_dragging() {
            return;
        }
        self.set_tool(Tool::Select);
        self.elements = Elements::default();
        self.hovered_element = None;
        self.select_mode = mode;
    }

    /// The picked corners, edges or faces.
    pub fn picked_elements(&self) -> &[PickedElement] {
        &self.elements.items
    }

    /// The box around the picked parts' corners, if any are picked.
    pub fn picked_bounds(&self) -> Option<Aabb> {
        self.elements.bounds()
    }

    /// Forgets the picked parts. Returns false if none were picked (or a
    /// gizmo drag is holding them).
    pub fn clear_elements(&mut self) -> bool {
        if self.elements.items.is_empty() || self.gizmo.is_dragging() {
            return false;
        }
        self.elements = Elements::default();
        true
    }

    /// What to draw over the viewport in a part-picking mode: the parts
    /// that can be picked, the picked ones and the one under the pointer.
    /// `None` in Object mode or with the Draw tool.
    pub fn element_overlay(
        &self,
        camera: &Camera,
        size: Vec2,
        doc: &Document,
    ) -> Option<ElementOverlay> {
        let kind = self.select_mode.element()?;
        if self.tool != Tool::Select {
            return None;
        }
        let size = size.max(Vec2::ONE);
        Some(ElementOverlay {
            candidates: elements::candidates(doc, kind, camera, size),
            picked: self
                .elements
                .items
                .iter()
                .filter_map(|i| elements::on_screen(&i.shape, camera, size))
                .collect(),
            hovered: self
                .hovered_element
                .as_ref()
                .filter(|_| !self.gizmo.is_dragging())
                .and_then(|i| elements::on_screen(&i.shape, camera, size)),
        })
    }

    /// The picked parts for the gizmo: `Some` in a part-picking mode.
    fn parts(&self) -> Option<&Elements> {
        self.select_mode.element().map(|_| &self.elements)
    }

    /// True if clicks pick single brushes inside brush entities (such as
    /// one brush of a `func_detail`) rather than the whole entity.
    pub fn inside_entities(&self) -> bool {
        self.inside_entities
    }

    /// Turns picking inside brush entities on or off.
    pub fn set_inside_entities(&mut self, on: bool) {
        self.inside_entities = on;
    }

    /// The active tool.
    pub fn tool(&self) -> Tool {
        self.tool
    }

    /// Switches tool, cancelling anything in progress, except during a
    /// gizmo drag, when it does nothing. The Draw tool hides the gizmo.
    pub fn set_tool(&mut self, tool: Tool) {
        // A gizmo drag must finish (or be cancelled with Escape) first.
        if self.gizmo.is_dragging() {
            return;
        }
        self.tool = tool;
        self.press_at = None;
        self.drag = None;
        if tool == Tool::Box {
            // Picked parts are hidden while drawing; let go of them, so
            // Escape there deselects as it looks like it should.
            self.gizmo.set_mode(None);
            self.elements = Elements::default();
            self.hovered_element = None;
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
        self.gizmo
            .shapes(camera, size.max(Vec2::ONE), doc, self.parts())
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

    /// What the Draw tool makes.
    pub fn shape(&self) -> Shape {
        self.shape
    }

    /// Picks what the Draw tool makes.
    pub fn set_shape(&mut self, shape: Shape) {
        self.shape = shape;
    }

    /// The settings of the shapes that have them (sides, arch thickness,
    /// step height).
    pub fn shape_settings(&self) -> &ShapeSettings {
        &self.shape_settings
    }

    /// Changes the shape settings. Values out of range are kept in range
    /// when the shape is built.
    pub fn shape_settings_mut(&mut self) -> &mut ShapeSettings {
        &mut self.shape_settings
    }

    /// The brushes the drag in progress would make, for the preview.
    pub fn preview_brushes(&self) -> Vec<Brush> {
        self.drag
            .and_then(|drag| self.build(drag).ok())
            .unwrap_or_default()
    }

    /// Why the drag in progress would make nothing, in plain words, if it
    /// would not (shown while dragging, so a refusal is no surprise).
    pub fn preview_problem(&self) -> Option<String> {
        match self.build(self.drag?) {
            Err(Some(e)) => Some(self.explain(e)),
            _ => None,
        }
    }

    /// Turns the shape being drawn a quarter turn clockwise (seen from
    /// above) inside its box (R while drawing): stairs then climb, and an
    /// arch spans, the next way round. Returns false if nothing is being
    /// drawn.
    pub fn turn_drawing(&mut self) -> bool {
        match self.drag.as_mut() {
            Some(drag) => {
                drag.turns = (drag.turns + 1) % 4;
                true
            }
            None => false,
        }
    }

    /// True while a shape is being drawn.
    pub fn is_drawing(&self) -> bool {
        self.drag.is_some()
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
        if drag.uniform || drag.centered {
            // As wide, deep and tall as the longest side of the drag.
            let side = moved.max_element();
            let base = drag.start.extend(drag.base);
            return Some(if drag.centered {
                let reach = Vec2::splat(side);
                Aabb::from_corners(
                    (drag.start - reach).extend(drag.base),
                    (drag.start + reach).extend(drag.base + side * 2.0),
                )
            } else {
                let toward = Vec2::new(
                    if drag.end.x < drag.start.x { -1.0 } else { 1.0 },
                    if drag.end.y < drag.start.y { -1.0 } else { 1.0 },
                );
                Aabb::from_corners(base, (drag.start + toward * side).extend(drag.base + side))
            });
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
        let kind = self.select_mode.element();
        if !self.gizmo.is_dragging() {
            self.elements.refresh(doc, kind);
        }
        if self.tool == Tool::Select {
            let size = input.size.max(Vec2::ONE);
            let input = ToolInput { size, ..*input };
            let parts = kind.map(|_| &mut self.elements);
            match self
                .gizmo
                .update(&input, cursor, camera, doc, parts, self.grid)
            {
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
            Tool::Select => {
                if let Some(kind) = kind {
                    let size = input.size.max(Vec2::ONE);
                    self.hovered_element =
                        cursor.and_then(|c| elements::pick(doc, kind, camera, size, c));
                }
                self.update_select(input, cursor, ray.as_ref(), doc)
            }
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
        if self.select_mode.element().is_some() {
            return self.click_element(input, ray, doc);
        }
        let hit = ray
            .and_then(|r| DocumentScene::new(doc).pick_object(r))
            .map(|(id, point)| (doc.selectable(id, self.inside_entities), point));
        match (hit, input.additive) {
            (Some((id, _)), true) => Some(ToolAction::ToggleSelected(id)),
            (None, true) => None,
            (hit, false) => Some(ToolAction::Select(hit.map(|(id, _)| id))),
        }
    }

    /// A click in a part-picking mode: picks the part under the pointer
    /// (Ctrl+click adds or removes it). Off the selected brushes, a click
    /// on another object selects that object instead, and a click on
    /// nothing lets go of the picked parts but keeps the brushes selected.
    fn click_element(
        &mut self,
        input: &ToolInput,
        ray: Option<&Ray>,
        doc: &Document,
    ) -> Option<ToolAction> {
        if let Some(item) = self.hovered_element.clone() {
            if input.additive {
                self.elements.toggle(item);
            } else if !(self.elements.items.len() == 1 && self.elements.contains(&item)) {
                self.elements.set(Some(item));
            }
            return None;
        }
        let hit = ray
            .and_then(|r| DocumentScene::new(doc).pick_object(r))
            .map(|(id, _)| doc.selectable(id, self.inside_entities))
            .filter(|id| !doc.is_shown_selected(*id));
        match (hit, input.additive) {
            (Some(id), true) => Some(ToolAction::ToggleSelected(id)),
            (Some(id), false) => {
                self.elements.set(None);
                Some(ToolAction::Select(Some(id)))
            }
            (None, true) => None,
            (None, false) => {
                self.elements.set(None);
                None
            }
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
                turns: 0,
                uniform: false,
                centered: false,
            });
        }
        if let Some(drag) = self.drag.as_mut() {
            drag.uniform = input.uniform;
            drag.centered = input.centered;
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

    /// The brushes a drag makes, or why it can't. `Ok` is never empty; a
    /// drag too short to count is `Err(None)`.
    fn build(&self, drag: BoxDrag) -> Built {
        let bounds = self.box_bounds(drag).ok_or(None)?;
        let key = (
            bounds,
            Heading::of_drag(drag.end - drag.start).turned_clockwise(drag.turns),
            self.shape,
            self.shape_settings,
        );
        if let Some((built_key, built)) = &*self.built.borrow()
            && *built_key == key
        {
            return built.clone();
        }
        let built = build_shape(key.2, &key.3, key.0, key.1).map_err(Some);
        *self.built.borrow_mut() = Some((key, built.clone()));
        built
    }

    /// Why a shape could not be made, in plain words.
    fn explain(&self, e: GeomError) -> String {
        let name = self.shape.label().to_lowercase();
        match e {
            GeomError::TooLarge => format!("The {name} would reach beyond the edge of the world."),
            GeomError::TooSmall | GeomError::NotClosed => {
                let advice = if self.shape.has_sides() {
                    " Draw it bigger, or use fewer sides."
                } else if self.shape == Shape::Stairs {
                    " Draw it longer, or use taller steps."
                } else {
                    " Draw it bigger."
                };
                format!("No {name} was made: it is too small for that shape.{advice}")
            }
            e => format!("No {name} was made: {e}."),
        }
    }

    fn finish_box(&self, drag: BoxDrag) -> Option<ToolAction> {
        Some(match self.build(drag) {
            Ok(brushes) => ToolAction::Execute(Command::AddBrushes(brushes)),
            Err(None) => return None,
            Err(Some(e)) => ToolAction::Refused(self.explain(e)),
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
