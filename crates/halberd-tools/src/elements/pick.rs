//! Finding the part under the pointer, and the parts as drawn on screen.

use super::{ELEMENT_GRAB_POINTS, ElementShape, PickedElement, editable_brushes};
use crate::camera::{Camera, Ray};
use glam::{Vec2, Vec3};
use halberd_doc::{Document, ElementKind};
use halberd_geom::Brush;

/// The part of kind `kind` under `cursor`, among the editable brushes.
/// Corners and edges are picked on screen, nearest first (the one nearer
/// the camera if two overlap), seen through the brushes as in Hammer;
/// faces by where the pointer's ray meets them, unless another object is
/// in front.
pub(crate) fn pick(
    doc: &Document,
    kind: ElementKind,
    camera: &Camera,
    size: Vec2,
    cursor: Vec2,
) -> Option<PickedElement> {
    let brushes = editable_brushes(doc);
    let project = |p: Vec3| camera.project(p, size).filter(|s| s.is_finite());
    // Best so far: (distance on screen, depth, part).
    let mut best: Option<(f32, f32, PickedElement)> = None;
    let mut offer = |distance: f32, depth: f32, item: PickedElement| {
        let better = best
            .as_ref()
            .is_none_or(|(d, z, _)| distance < d - 0.5 || (distance < d + 0.5 && depth < *z));
        if distance <= ELEMENT_GRAB_POINTS && better {
            best = Some((distance, depth, item));
        }
    };
    match kind {
        ElementKind::Vertex => {
            for (id, brush) in &brushes {
                for p in brush.points() {
                    if let Some(s) = project(p) {
                        let item = PickedElement {
                            brush: *id,
                            shape: ElementShape::Vertex(p),
                        };
                        offer(s.distance(cursor), camera.depth_of(p), item);
                    }
                }
            }
        }
        ElementKind::Edge => {
            for (id, brush) in &brushes {
                let (points, _, edges) = brush.topology();
                for (a, b) in edges {
                    let (pa, pb) = (points[a], points[b]);
                    if let (Some(sa), Some(sb)) = (project(pa), project(pb)) {
                        let item = PickedElement {
                            brush: *id,
                            shape: ElementShape::Edge([pa, pb]),
                        };
                        let depth = camera.depth_of((pa + pb) * 0.5);
                        offer(distance_to_segment(cursor, sa, sb), depth, item);
                    }
                }
            }
        }
        ElementKind::Face => {
            let ray = camera.ray_through(cursor, size);
            let mut nearest: Option<(f32, PickedElement)> = None;
            for (id, brush) in &brushes {
                if let Some((t, face)) = face_hit(brush, &ray)
                    && nearest.as_ref().is_none_or(|(n, _)| t < *n)
                {
                    let corners = brush.faces()[face].vertices().to_vec();
                    nearest = Some((
                        t,
                        PickedElement {
                            brush: *id,
                            shape: ElementShape::Face(corners),
                        },
                    ));
                }
            }
            // Something else in front (an unselected wall, say) hides the
            // face; the click is then for that object.
            let hidden = nearest.as_ref().is_some_and(|(t, _)| {
                doc.pick(ray.origin, ray.direction)
                    .is_some_and(|(id, d)| d < t - 0.5 && !brushes.iter().any(|(b, _)| *b == id))
            });
            return nearest.filter(|_| !hidden).map(|(_, item)| item);
        }
    }
    best.map(|(_, _, item)| item)
}

/// The nearest face of `brush` facing the ray that the ray meets, with the
/// distance to it.
fn face_hit(brush: &Brush, ray: &Ray) -> Option<(f32, usize)> {
    let mut best: Option<(f32, usize)> = None;
    for (i, face) in brush.faces().iter().enumerate() {
        let plane = face.plane();
        let facing = plane.normal.dot(ray.direction);
        if facing >= -1e-6 {
            continue;
        }
        let t = -plane.signed_distance(ray.origin) / facing;
        if !(t > 0.0 && t.is_finite()) {
            continue;
        }
        let at = ray.at(t);
        let inside = brush
            .faces()
            .iter()
            .enumerate()
            .all(|(j, other)| j == i || other.plane().signed_distance(at) <= 0.01);
        if inside && best.is_none_or(|(b, _)| t < b) {
            best = Some((t, i));
        }
    }
    best
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

/// One part as drawn on screen, in points from the viewport's top-left.
#[derive(Debug, Clone, PartialEq)]
pub enum ScreenElement {
    /// A corner.
    Point(Vec2),
    /// An edge.
    Segment(Vec2, Vec2),
    /// A face's outline.
    Polygon(Vec<Vec2>),
}

/// What to draw over the viewport in a part-picking mode.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ElementOverlay {
    /// Every part that can be picked (corners or edges; faces are left
    /// out, the brush already shows them).
    pub candidates: Vec<ScreenElement>,
    /// The picked parts.
    pub picked: Vec<ScreenElement>,
    /// The part under the pointer, if any.
    pub hovered: Option<ScreenElement>,
}

/// `shape` on screen, or `None` if any of it is behind the camera.
pub(crate) fn on_screen(
    shape: &ElementShape,
    camera: &Camera,
    size: Vec2,
) -> Option<ScreenElement> {
    let project = |p: Vec3| camera.project(p, size).filter(|s| s.is_finite());
    Some(match shape {
        ElementShape::Vertex(p) => ScreenElement::Point(project(*p)?),
        ElementShape::Edge([a, b]) => ScreenElement::Segment(project(*a)?, project(*b)?),
        ElementShape::Face(corners) => {
            ScreenElement::Polygon(corners.iter().map(|c| project(*c)).collect::<Option<_>>()?)
        }
    })
}

/// Every corner or edge of the editable brushes, on screen.
pub(crate) fn candidates(
    doc: &Document,
    kind: ElementKind,
    camera: &Camera,
    size: Vec2,
) -> Vec<ScreenElement> {
    let project = |p: Vec3| camera.project(p, size).filter(|s| s.is_finite());
    let mut out = Vec::new();
    for (_, brush) in editable_brushes(doc) {
        match kind {
            ElementKind::Vertex => {
                out.extend(
                    brush
                        .points()
                        .iter()
                        .filter_map(|p| project(*p))
                        .map(ScreenElement::Point),
                );
            }
            ElementKind::Edge => {
                let (points, _, edges) = brush.topology();
                for (a, b) in edges {
                    if let (Some(sa), Some(sb)) = (project(points[a]), project(points[b])) {
                        out.push(ScreenElement::Segment(sa, sb));
                    }
                }
            }
            ElementKind::Face => {}
        }
    }
    out
}
