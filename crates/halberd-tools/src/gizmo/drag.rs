//! What dragging each gizmo handle does to the selection.
//!
//! A drag remembers the selected brushes as they were when it began, and
//! every frame rebuilds them from those originals with the total movement
//! so far. Small steps never pile up rounding errors, and moving back to
//! the start gives back exactly the original shapes.

use super::{Frame, Handle, ROTATE_SNAP_DEGREES};
use crate::ToolAction;
use glam::{Vec2, Vec3};
use halberd_doc::{Command, Document, Object, ObjectId, TransformKind};
use halberd_geom::{Aabb, Brush, GeomError};

/// Pixels of pointer movement that double (or halve) the size when
/// resizing evenly.
const UNIFORM_SCALE_POINTS: f32 = 100.0;
/// Even resizing snaps to steps of this factor.
const UNIFORM_SCALE_STEP: f32 = 0.125;

/// Rebuilds one brush for the current frame of a drag.
type Transform = Box<dyn Fn(&Brush) -> Result<Brush, GeomError>>;

/// Where a drag began, in whatever terms its handle measures.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Start {
    /// Distance along the handle's axis line from the pivot.
    Along(f32),
    /// A point on the handle's plane.
    OnPlane(Vec3),
    /// The pointer's angle around the pivot on screen, and the total turn
    /// so far (kept so a drag can go round more than once).
    Angle {
        last: f32,
        total: f32,
        /// The axis pointed at the camera when the drag began; kept, so
        /// orbiting during the drag does not flip the turn.
        towards_camera: bool,
    },
    /// The pointer position on screen.
    Screen(Vec2),
}

/// A gizmo drag in progress.
#[derive(Debug, Clone)]
pub(crate) struct Drag {
    handle: Handle,
    key: u64,
    originals: Vec<(ObjectId, Brush)>,
    bounds: Aabb,
    start: Start,
}

impl Drag {
    /// Starts dragging `handle` with the pointer at `cursor`. Returns `None`
    /// if nothing selected can be transformed or the handle cannot be
    /// grabbed from this angle.
    pub(crate) fn start(
        handle: Handle,
        cursor: Vec2,
        frame: &Frame,
        doc: &Document,
        key: u64,
    ) -> Option<Self> {
        let originals: Vec<(ObjectId, Brush)> = doc
            .selection()
            .iter()
            .filter_map(|id| match doc.get(*id)? {
                Object::Brush(brush) => Some((*id, brush.clone())),
            })
            .collect();
        if originals.is_empty() {
            return None;
        }
        let bounds = doc.selection_bounds()?;
        let start = match handle {
            Handle::MoveAxis(a) | Handle::ScaleAxis(a) => {
                Start::Along(along_axis(frame, a.unit(), cursor)?)
            }
            Handle::MovePlane(a) => Start::OnPlane(on_plane(frame, a.unit(), cursor)?),
            Handle::Rotate(a) => {
                let angle = screen_angle(frame, cursor)?;
                Start::Angle {
                    last: angle,
                    total: 0.0,
                    towards_camera: a.unit().dot(frame.camera.position - frame.pivot) > 0.0,
                }
            }
            Handle::ScaleUniform => Start::Screen(cursor),
        };
        Some(Self {
            handle,
            key,
            originals,
            bounds,
            start,
        })
    }

    pub(crate) fn handle(&self) -> Handle {
        self.handle
    }

    pub(crate) fn key(&self) -> u64 {
        self.key
    }

    /// The selection's bounds when the drag began.
    pub(crate) fn start_bounds(&self) -> Aabb {
        self.bounds
    }

    /// The edit for the pointer now at `cursor`, or `None` if it cannot be
    /// worked out this frame (the pointer is somewhere meaningless, or the
    /// result would be an impossible shape).
    pub(crate) fn update(&mut self, cursor: Vec2, frame: &Frame, grid: f32) -> Option<ToolAction> {
        let snap = |v: f32| (v / grid).round() * grid;
        let bounds = self.bounds;
        let centre = bounds.center();
        let (kind, transform): (TransformKind, Transform) = match (self.handle, &mut self.start) {
            (Handle::MoveAxis(a), Start::Along(t0)) => {
                let t = along_axis(frame, a.unit(), cursor)?;
                let offset = a.unit() * snap(t - *t0);
                (TransformKind::Move, Box::new(move |b| b.translated(offset)))
            }
            (Handle::MovePlane(a), Start::OnPlane(p0)) => {
                let p = on_plane(frame, a.unit(), cursor)?;
                let d = p - *p0;
                let offset = Vec3::new(snap(d.x), snap(d.y), snap(d.z)) * (Vec3::ONE - a.unit());
                (TransformKind::Move, Box::new(move |b| b.translated(offset)))
            }
            (
                Handle::Rotate(a),
                Start::Angle {
                    last,
                    total,
                    towards_camera,
                },
            ) => {
                let angle = screen_angle(frame, cursor)?;
                *total += wrap(angle - *last);
                *last = angle;
                // Screen angles grow clockwise (y points down). Seen
                // from the tip of the axis, a positive turn is
                // counter-clockwise, so the sign depends on which way
                // the axis points relative to the camera.
                let turn = if *towards_camera { -*total } else { *total };
                let step = ROTATE_SNAP_DEGREES.to_radians();
                let snapped = (turn / step).round() * step;
                let axis = a.unit();
                (
                    TransformKind::Rotate,
                    Box::new(move |b| b.rotated(axis, snapped, centre)),
                )
            }
            (Handle::ScaleAxis(a), Start::Along(t0)) => {
                let t = along_axis(frame, a.unit(), cursor)?;
                let i = a as usize;
                let old = bounds.size()[i];
                let smallest = grid.min(old);
                let new = (old + snap(t - *t0)).max(smallest);
                let mut factors = Vec3::ONE;
                factors[i] = new / old;
                let mut anchor = centre;
                anchor[i] = bounds.min[i];
                (
                    TransformKind::Scale,
                    Box::new(move |b| b.scaled(factors, anchor)),
                )
            }
            (Handle::ScaleUniform, Start::Screen(c0)) => {
                let moved = (cursor.x - c0.x) - (cursor.y - c0.y);
                let raw = 2f32.powf(moved / UNIFORM_SCALE_POINTS);
                let factor = ((raw / UNIFORM_SCALE_STEP).round() * UNIFORM_SCALE_STEP)
                    .max(UNIFORM_SCALE_STEP);
                let anchor = centre.with_z(bounds.min.z);
                (
                    TransformKind::Scale,
                    Box::new(move |b| b.scaled(Vec3::splat(factor), anchor)),
                )
            }
            _ => return None,
        };
        let brushes = self
            .originals
            .iter()
            .map(|(id, brush)| Some((*id, transform(brush).ok()?)))
            .collect::<Option<Vec<_>>>()?;
        Some(ToolAction::ExecuteMerging(
            Command::TransformBrushes { kind, brushes },
            self.key,
        ))
    }
}

/// Distance along the axis line through the pivot to the point closest to
/// the pointer's ray. `None` when the axis points (almost) along the ray.
fn along_axis(frame: &Frame, axis: Vec3, cursor: Vec2) -> Option<f32> {
    let ray = frame.camera.ray_through(cursor, frame.size);
    let w = frame.pivot - ray.origin;
    let b = axis.dot(ray.direction);
    let denom = 1.0 - b * b;
    if denom < 1e-4 {
        return None;
    }
    let t = (b * ray.direction.dot(w) - axis.dot(w)) / denom;
    t.is_finite().then_some(t)
}

/// Where the pointer's ray meets the plane through the pivot at right
/// angles to `normal`. `None` when the plane is seen edge-on or behind.
fn on_plane(frame: &Frame, normal: Vec3, cursor: Vec2) -> Option<Vec3> {
    let ray = frame.camera.ray_through(cursor, frame.size);
    let facing = ray.direction.dot(normal);
    if facing.abs() < 1e-3 {
        return None;
    }
    let t = (frame.pivot - ray.origin).dot(normal) / facing;
    (t > 0.0 && t.is_finite()).then(|| ray.at(t))
}

/// The pointer's angle around the pivot on screen, in radians. `None` when
/// the pointer is right on the pivot.
fn screen_angle(frame: &Frame, cursor: Vec2) -> Option<f32> {
    let centre = frame.project(frame.pivot)?;
    let v = cursor - centre;
    (v.length() > 2.0).then(|| v.y.atan2(v.x))
}

/// Brings an angle difference into (-π, π].
fn wrap(angle: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let a = (angle + PI).rem_euclid(TAU) - PI;
    if a <= -PI { a + TAU } else { a }
}
