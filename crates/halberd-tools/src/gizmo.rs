//! The transform gizmo: handles drawn around the selection to move (W),
//! rotate (R) and scale (S) it, or all three at once (T).
//!
//! The gizmo is worked out in screen space: its handles are projected onto
//! the viewport, the interface draws them as flat shapes, and the pointer
//! is tested against those same shapes. It always looks the same size on
//! screen, whatever the distance.
//!
//! Dragging a handle never changes the map directly: each frame produces a
//! [`ToolAction::ExecuteMerging`] edit, and all the edits of one drag share
//! a key so they make one undo step.

mod drag;

use crate::ToolAction;
use crate::camera::Camera;
use drag::Drag;
use glam::{Vec2, Vec3};
use halberd_doc::Document;
use halberd_geom::Aabb;

/// How long the gizmo's arms look on screen, in points.
pub const GIZMO_ARM_POINTS: f32 = 90.0;
/// How close the pointer must be to a handle to grab it, in points.
pub const GIZMO_GRAB_POINTS: f32 = 8.0;
/// Rotation snaps to this many degrees.
pub const ROTATE_SNAP_DEGREES: f32 = 15.0;

/// Which handles the gizmo shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoMode {
    /// Arrows and plane squares to move (W).
    Move,
    /// Rings to rotate (R).
    Rotate,
    /// Cubes to scale or resize (S).
    Scale,
    /// All of them at once (T).
    All,
}

impl GizmoMode {
    /// Every mode, in the order the toolbar shows them.
    pub const ALL: [GizmoMode; 4] = [Self::Move, Self::Rotate, Self::Scale, Self::All];

    /// Short name for the toolbar.
    pub fn label(self) -> &'static str {
        match self {
            Self::Move => "Move",
            Self::Rotate => "Rotate",
            Self::Scale => "Scale",
            Self::All => "All",
        }
    }

    /// The key that switches to this mode.
    pub fn key(self) -> &'static str {
        match self {
            Self::Move => "W",
            Self::Rotate => "R",
            Self::Scale => "S",
            Self::All => "T",
        }
    }

    /// One-line explanation, for tooltips.
    pub fn description(self) -> &'static str {
        match self {
            Self::Move => "Drag an arrow to move along it, or a square to move across it (W)",
            Self::Rotate => "Drag a ring to rotate, in 15° steps (R)",
            Self::Scale => "Drag a cube to stretch that side, or the middle to resize evenly (S)",
            Self::All => "Move, rotate and scale handles together (T)",
        }
    }

    fn moves(self) -> bool {
        matches!(self, Self::Move | Self::All)
    }

    fn rotates(self) -> bool {
        matches!(self, Self::Rotate | Self::All)
    }

    fn scales(self) -> bool {
        matches!(self, Self::Scale | Self::All)
    }
}

/// One of the three world axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// X (red).
    X,
    /// Y (green).
    Y,
    /// Z (blue, up).
    Z,
}

impl Axis {
    /// All three, in order.
    pub const ALL: [Axis; 3] = [Self::X, Self::Y, Self::Z];

    /// The unit vector along the axis.
    pub fn unit(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
        }
    }

    /// The other two axes.
    fn others(self) -> (Axis, Axis) {
        match self {
            Self::X => (Self::Y, Self::Z),
            Self::Y => (Self::X, Self::Z),
            Self::Z => (Self::X, Self::Y),
        }
    }
}

/// A part of the gizmo that can be dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    /// Arrow: move along the axis.
    MoveAxis(Axis),
    /// Square: move across the plane at right angles to the axis.
    MovePlane(Axis),
    /// Ring: rotate around the axis.
    Rotate(Axis),
    /// Cube on an axis: stretch that side.
    ScaleAxis(Axis),
    /// Cube in the middle: resize evenly.
    ScaleUniform,
}

impl Handle {
    /// The axis the handle belongs to, for its colour (`None`: the centre).
    pub fn axis(self) -> Option<Axis> {
        match self {
            Self::MoveAxis(a) | Self::MovePlane(a) | Self::Rotate(a) | Self::ScaleAxis(a) => {
                Some(a)
            }
            Self::ScaleUniform => None,
        }
    }
}

/// One flat shape of the gizmo, in points from the viewport's top-left.
#[derive(Debug, Clone, PartialEq)]
pub struct GizmoShape {
    /// The handle this shape belongs to.
    pub handle: Handle,
    /// Outline points.
    pub points: Vec<Vec2>,
    /// True: a filled polygon. False: an open line through the points.
    pub filled: bool,
    /// The pointer is over this handle, or it is being dragged.
    pub highlighted: bool,
}

/// Where the gizmo is and how big it is in the world this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Frame {
    /// The middle of the selection.
    pub(crate) pivot: Vec3,
    /// World length of one arm.
    pub(crate) length: f32,
    pub(crate) camera: Camera,
    pub(crate) size: Vec2,
}

impl Frame {
    /// The gizmo for `selection`, or `None` if the selection is behind the
    /// camera.
    pub(crate) fn new(selection: Aabb, camera: &Camera, size: Vec2) -> Option<Self> {
        let pivot = selection.center();
        let depth = camera.depth_of(pivot);
        if depth.is_nan() || depth <= 1.0 || !pivot.is_finite() {
            return None;
        }
        let size = size.max(Vec2::ONE);
        let length = depth * 2.0 * (camera.fov_y * 0.5).tan() * GIZMO_ARM_POINTS / size.y;
        Some(Self {
            pivot,
            length,
            camera: *camera,
            size,
        })
    }

    pub(crate) fn project(&self, p: Vec3) -> Option<Vec2> {
        self.camera.project(p, self.size).filter(|s| s.is_finite())
    }

    /// How directly the camera looks along `dir` (0: across, 1: along).
    fn facing(&self, dir: Vec3) -> f32 {
        let view = (self.pivot - self.camera.position).normalize_or_zero();
        dir.dot(view).abs()
    }
}

/// Every shape of the gizmo in `mode`, in the order they should be tested
/// for the pointer (most specific first). Shapes for handles pointing
/// straight at the camera are left out: they cannot be dragged sensibly.
pub(crate) fn shapes(mode: GizmoMode, frame: &Frame) -> Vec<GizmoShape> {
    let mut out = Vec::new();
    let mut push = |handle, points: Vec<Vec2>, filled| {
        if points.len() >= 2 {
            out.push(GizmoShape {
                handle,
                points,
                filled,
                highlighted: false,
            });
        }
    };
    let Some(centre) = frame.project(frame.pivot) else {
        return out;
    };
    let l = frame.length;
    if mode.scales() {
        push(Handle::ScaleUniform, square(centre, 7.0), true);
        let reach = if mode == GizmoMode::All { 0.7 } else { 1.0 };
        for axis in Axis::ALL {
            if frame.facing(axis.unit()) > 0.97 {
                continue;
            }
            if let Some(end) = frame.project(frame.pivot + axis.unit() * l * reach) {
                push(Handle::ScaleAxis(axis), square(end, 6.0), true);
                if mode == GizmoMode::Scale {
                    push(Handle::ScaleAxis(axis), vec![centre, end], false);
                }
            }
        }
    }
    if mode.moves() {
        for axis in Axis::ALL {
            if frame.facing(axis.unit()) < 0.2 {
                continue;
            }
            let (b, c) = axis.others();
            let corner =
                |u: f32, v: f32| frame.project(frame.pivot + b.unit() * l * u + c.unit() * l * v);
            let quad = [
                corner(0.25, 0.25),
                corner(0.45, 0.25),
                corner(0.45, 0.45),
                corner(0.25, 0.45),
            ];
            if quad.iter().all(Option::is_some) {
                push(
                    Handle::MovePlane(axis),
                    quad.iter().flatten().copied().collect(),
                    true,
                );
            }
        }
        for axis in Axis::ALL {
            if frame.facing(axis.unit()) > 0.97 {
                continue;
            }
            let (Some(start), Some(tip)) = (
                frame.project(frame.pivot + axis.unit() * l * 0.2),
                frame.project(frame.pivot + axis.unit() * l),
            ) else {
                continue;
            };
            push(Handle::MoveAxis(axis), vec![start, tip], false);
            let along = (tip - start).normalize_or_zero();
            let side = along.perp() * 5.0;
            let base = tip - along * 12.0;
            push(
                Handle::MoveAxis(axis),
                vec![tip, base + side, base - side],
                true,
            );
        }
    }
    if mode.rotates() {
        let radius = if mode == GizmoMode::All { 1.15 } else { 0.9 };
        for axis in Axis::ALL {
            let (b, c) = axis.others();
            // All or nothing: a ring partly behind the camera would be
            // joined across the gap with a false line.
            let ring: Option<Vec<Vec2>> = (0..=48)
                .map(|i| {
                    let t = i as f32 / 48.0 * std::f32::consts::TAU;
                    frame.project(
                        frame.pivot + (b.unit() * t.cos() + c.unit() * t.sin()) * l * radius,
                    )
                })
                .collect();
            if let Some(ring) = ring {
                push(Handle::Rotate(axis), ring, false);
            }
        }
    }
    out
}

/// A small square around a point, in points.
fn square(at: Vec2, half: f32) -> Vec<Vec2> {
    vec![
        at + Vec2::new(-half, -half),
        at + Vec2::new(half, -half),
        at + Vec2::new(half, half),
        at + Vec2::new(-half, half),
    ]
}

/// The handle under `cursor`, if any. A shape the pointer is inside wins
/// (the one whose middle is nearest, if several overlap); otherwise the
/// nearest shape within [`GIZMO_GRAB_POINTS`]. Ties go to the shape listed
/// first.
pub(crate) fn handle_at(shapes: &[GizmoShape], cursor: Vec2) -> Option<Handle> {
    let score = |shape: &GizmoShape| {
        let distance = distance_to_shape(shape, cursor);
        let middle = shape.points.iter().copied().sum::<Vec2>() / shape.points.len() as f32;
        // Inside a filled shape: rank by how central the pointer is.
        if distance == 0.0 {
            -1000.0 + middle.distance(cursor) * 0.01
        } else {
            distance
        }
    };
    let mut best: Option<(f32, Handle)> = None;
    for shape in shapes {
        let s = score(shape);
        if s <= GIZMO_GRAB_POINTS && best.is_none_or(|(b, _)| s < b) {
            best = Some((s, shape.handle));
        }
    }
    best.map(|(_, handle)| handle)
}

fn distance_to_shape(shape: &GizmoShape, p: Vec2) -> f32 {
    if shape.filled && contains(&shape.points, p) {
        return 0.0;
    }
    let n = shape.points.len();
    let segments = if shape.filled { n } else { n - 1 };
    (0..segments)
        .map(|i| distance_to_segment(p, shape.points[i], shape.points[(i + 1) % n]))
        .fold(f32::INFINITY, f32::min)
}

fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 {
        ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + ab * t)
}

/// Point-in-polygon by counting crossings; works for any simple polygon.
fn contains(polygon: &[Vec2], p: Vec2) -> bool {
    let mut inside = false;
    let n = polygon.len();
    for i in 0..n {
        let (a, b) = (polygon[i], polygon[(i + n - 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

/// The gizmo's state: its mode, the handle under the pointer and the drag
/// in progress.
#[derive(Debug, Clone, Default)]
pub(crate) struct Gizmo {
    pub(crate) mode: Option<GizmoMode>,
    hovered: Option<Handle>,
    drag: Option<Drag>,
    drags_started: u64,
}

/// What the gizmo did with a frame of input.
pub(crate) enum GizmoOutcome {
    /// The input was not for the gizmo; the select tool may use it.
    NotMine,
    /// The gizmo used the input (possibly asking for an edit).
    Used(Option<ToolAction>),
}

impl Gizmo {
    /// Switches mode (or hides the gizmo), cancelling nothing: callers
    /// cancel drags first.
    pub(crate) fn set_mode(&mut self, mode: Option<GizmoMode>) {
        self.mode = mode;
        self.hovered = None;
    }

    pub(crate) fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// The gizmo's shapes for drawing, with the hovered or dragged handle
    /// highlighted. Empty when there is no gizmo to show.
    pub(crate) fn shapes(&self, camera: &Camera, size: Vec2, doc: &Document) -> Vec<GizmoShape> {
        let Some((mode, frame)) = self.frame(camera, size, doc) else {
            return Vec::new();
        };
        let active = self.drag.as_ref().map(Drag::handle).or(self.hovered);
        let mut list = shapes(mode, &frame);
        for shape in &mut list {
            shape.highlighted = Some(shape.handle) == active;
        }
        list
    }

    fn frame(&self, camera: &Camera, size: Vec2, doc: &Document) -> Option<(GizmoMode, Frame)> {
        let mode = self.mode?;
        // While dragging, the gizmo stays where the drag began.
        let bounds = match &self.drag {
            Some(drag) => drag.start_bounds(),
            None => doc.selection_bounds()?,
        };
        Some((mode, Frame::new(bounds, camera, size)?))
    }

    /// Handles one frame of left-mouse input.
    pub(crate) fn update(
        &mut self,
        input: &crate::ToolInput,
        cursor: Option<Vec2>,
        camera: &Camera,
        doc: &Document,
        grid: f32,
    ) -> GizmoOutcome {
        let Some((mode, frame)) = self.frame(camera, input.size, doc) else {
            self.hovered = None;
            // Mid-drag, the gizmo can briefly have no frame (the camera
            // passing the selection); the drag simply waits. Escape or
            // letting go still end it.
            return match &self.drag {
                Some(drag) if input.cancel => {
                    let key = drag.key();
                    self.drag = None;
                    GizmoOutcome::Used(Some(ToolAction::CancelMerging(key)))
                }
                Some(_) if !input.held || input.released => {
                    self.drag = None;
                    GizmoOutcome::Used(None)
                }
                Some(_) => GizmoOutcome::Used(None),
                None => GizmoOutcome::NotMine,
            };
        };
        if let Some(drag) = &mut self.drag {
            if input.cancel {
                let key = drag.key();
                self.drag = None;
                return GizmoOutcome::Used(Some(ToolAction::CancelMerging(key)));
            }
            if !input.held || input.released {
                self.drag = None;
                return GizmoOutcome::Used(None);
            }
            let action = cursor.and_then(|c| drag.update(c, &frame, grid));
            return GizmoOutcome::Used(action);
        }
        self.hovered = cursor.and_then(|c| handle_at(&shapes(mode, &frame), c));
        match (input.pressed, self.hovered, cursor) {
            (true, Some(handle), Some(c)) => {
                self.drags_started += 1;
                let key = (1 << 63) | self.drags_started;
                self.drag = Drag::start(handle, c, &frame, doc, key);
                // A handle that cannot be grabbed from this angle leaves the
                // click to the select tool.
                if self.drag.is_some() {
                    GizmoOutcome::Used(None)
                } else {
                    GizmoOutcome::NotMine
                }
            }
            _ => GizmoOutcome::NotMine,
        }
    }
}

#[cfg(test)]
mod tests;
