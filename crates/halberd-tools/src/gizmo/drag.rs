//! What dragging each gizmo handle does to the selection.
//!
//! A drag remembers the selected brushes (or the picked corners, edges or
//! faces, and their brushes) as they were when it began, and every frame
//! rebuilds them from those originals with the total movement so far.
//! Small steps never pile up rounding errors, and moving back to the start
//! gives back exactly the original shapes.

use super::{Frame, Handle, ROTATE_SNAP_DEGREES};
use crate::ToolAction;
use crate::elements::{PickedElement, Targets};
use glam::{DMat4, DVec3, Vec2, Vec3};
use halberd_doc::{Command, Document, ElementKind, ObjectId, TransformKind};
use halberd_geom::{Aabb, Brush, GeomError};

/// Pixels of pointer movement that double (or halve) the size when
/// resizing evenly.
const UNIFORM_SCALE_POINTS: f32 = 100.0;
/// Even resizing snaps to steps of this factor.
const UNIFORM_SCALE_STEP: f32 = 0.125;

/// How far a drag has moved, rotated or scaled what it holds.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Motion {
    /// Moved by this offset.
    Move(Vec3),
    /// Turned by an angle (radians) around an axis through a centre.
    Rotate {
        axis: Vec3,
        angle: f32,
        centre: Vec3,
    },
    /// Stretched by factors along the world axes, keeping an anchor still.
    Scale { factors: Vec3, anchor: Vec3 },
}

impl Motion {
    fn kind(self) -> TransformKind {
        match self {
            Self::Move(_) => TransformKind::Move,
            Self::Rotate { .. } => TransformKind::Rotate,
            Self::Scale { .. } => TransformKind::Scale,
        }
    }

    fn brush(self, brush: &Brush) -> Result<Brush, GeomError> {
        match self {
            Self::Move(offset) => brush.translated(offset),
            Self::Rotate {
                axis,
                angle,
                centre,
            } => brush.rotated(axis, angle, centre),
            Self::Scale { factors, anchor } => brush.scaled(factors, anchor),
        }
    }

    /// Where the motion takes `p` (worked in double precision, as brushes).
    fn point(self, p: Vec3) -> Vec3 {
        let matrix = match self {
            Self::Move(offset) => return p + offset,
            Self::Rotate {
                axis,
                angle,
                centre,
            } => {
                let centre = centre.as_dvec3();
                let axis = axis.as_dvec3().normalize_or(DVec3::Z);
                DMat4::from_translation(centre)
                    * DMat4::from_axis_angle(axis, f64::from(angle))
                    * DMat4::from_translation(-centre)
            }
            Self::Scale { factors, anchor } => {
                let anchor = anchor.as_dvec3();
                DMat4::from_translation(anchor)
                    * DMat4::from_scale(factors.as_dvec3())
                    * DMat4::from_translation(-anchor)
            }
        };
        matrix.transform_point3(p.as_dvec3()).as_vec3()
    }
}

/// What a drag holds.
#[derive(Debug, Clone)]
enum Held {
    /// Whole brushes, as they were.
    Brushes(Vec<(ObjectId, Brush)>),
    /// Picked parts of brushes: the brushes as they were with the corners
    /// to move, and the picked parts as they were.
    Parts {
        kind: ElementKind,
        targets: Targets,
        picked: Vec<PickedElement>,
    },
}

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
    held: Held,
    bounds: Aabb,
    start: Start,
}

impl Drag {
    /// Starts dragging `handle` with the pointer at `cursor`, holding the
    /// selected brushes, or the picked parts if any are picked. Returns
    /// `None` if there is nothing to transform or the handle cannot be
    /// grabbed from this angle.
    pub(crate) fn start(
        handle: Handle,
        cursor: Vec2,
        frame: &Frame,
        doc: &Document,
        parts: Option<&crate::elements::Elements>,
        key: u64,
    ) -> Option<Self> {
        let (held, bounds) = match parts {
            Some(parts) => {
                let kind = parts.items.first()?.shape.kind();
                let targets = parts.targets(doc);
                if targets.is_empty() {
                    return None;
                }
                let held = Held::Parts {
                    kind,
                    targets,
                    picked: parts.items.clone(),
                };
                (held, parts.bounds()?)
            }
            None => {
                let originals: Vec<(ObjectId, Brush)> = doc
                    .selection()
                    .iter()
                    .filter_map(|id| Some((*id, doc.get(*id)?.as_brush()?.brush().clone())))
                    .collect();
                if originals.is_empty() {
                    return None;
                }
                (Held::Brushes(originals), super::selected_brush_bounds(doc)?)
            }
        };
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
            held,
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
    /// result would be an impossible shape). When the drag holds picked
    /// parts, also gives where those parts are now.
    pub(crate) fn update(
        &mut self,
        cursor: Vec2,
        frame: &Frame,
        grid: f32,
    ) -> Option<(ToolAction, Option<Vec<PickedElement>>)> {
        let motion = self.motion(cursor, frame, grid)?;
        let kind = motion.kind();
        match &self.held {
            Held::Brushes(originals) => {
                let brushes = originals
                    .iter()
                    .map(|(id, brush)| Some((*id, motion.brush(brush).ok()?)))
                    .collect::<Option<Vec<_>>>()?;
                Some((
                    ToolAction::ExecuteMerging(
                        Command::TransformBrushes { kind, brushes },
                        self.key,
                    ),
                    None,
                ))
            }
            Held::Parts {
                kind: part,
                targets,
                picked,
            } => {
                let brushes = targets
                    .iter()
                    .map(|(id, brush, corners)| {
                        let points = brush.points();
                        let moves: Vec<(usize, Vec3)> = corners
                            .iter()
                            .map(|&c| (c, motion.point(points[c])))
                            .collect();
                        let (shaped, fates) = brush.with_moved_points(&moves).ok()?;
                        Some((*id, shaped, fates))
                    })
                    .collect::<Option<Vec<_>>>()?;
                let moved = picked
                    .iter()
                    .map(|item| PickedElement {
                        brush: item.brush,
                        shape: item.shape.mapped(|p| motion.point(p)),
                    })
                    .collect();
                let command = Command::EditBrushes {
                    kind,
                    parts: (*part, picked.len()),
                    brushes,
                };
                Some((ToolAction::ExecuteMerging(command, self.key), Some(moved)))
            }
        }
    }

    /// How far the drag has moved, turned or stretched, with the pointer
    /// now at `cursor`.
    fn motion(&mut self, cursor: Vec2, frame: &Frame, grid: f32) -> Option<Motion> {
        let snap = |v: f32| (v / grid).round() * grid;
        let bounds = self.bounds;
        let centre = bounds.center();
        Some(match (self.handle, &mut self.start) {
            (Handle::MoveAxis(a), Start::Along(t0)) => {
                let t = along_axis(frame, a.unit(), cursor)?;
                Motion::Move(a.unit() * snap(t - *t0))
            }
            (Handle::MovePlane(a), Start::OnPlane(p0)) => {
                let p = on_plane(frame, a.unit(), cursor)?;
                let d = p - *p0;
                Motion::Move(Vec3::new(snap(d.x), snap(d.y), snap(d.z)) * (Vec3::ONE - a.unit()))
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
                Motion::Rotate {
                    axis: a.unit(),
                    angle: (turn / step).round() * step,
                    centre,
                }
            }
            (Handle::ScaleAxis(a), Start::Along(t0)) => {
                let t = along_axis(frame, a.unit(), cursor)?;
                let i = a as usize;
                let old = bounds.size()[i];
                // A single corner, or parts flat on this axis, have no
                // size to stretch.
                if old < 1e-3 {
                    return None;
                }
                let smallest = grid.min(old);
                let new = (old + snap(t - *t0)).max(smallest);
                let mut factors = Vec3::ONE;
                factors[i] = new / old;
                let mut anchor = centre;
                anchor[i] = bounds.min[i];
                Motion::Scale { factors, anchor }
            }
            (Handle::ScaleUniform, Start::Screen(c0)) => {
                let moved = (cursor.x - c0.x) - (cursor.y - c0.y);
                let raw = 2f32.powf(moved / UNIFORM_SCALE_POINTS);
                let factor = ((raw / UNIFORM_SCALE_STEP).round() * UNIFORM_SCALE_STEP)
                    .max(UNIFORM_SCALE_STEP);
                Motion::Scale {
                    factors: Vec3::splat(factor),
                    anchor: centre.with_z(bounds.min.z),
                }
            }
            _ => return None,
        })
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
