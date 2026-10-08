//! Drawing the transform gizmo on top of the viewport image.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, vec2};
use halberd_tools::{Axis, GizmoShape, Handle};

/// Highlight for the handle under the pointer or being dragged.
const HIGHLIGHT: Color32 = Color32::from_rgb(255, 210, 64);

/// Colour of a handle: the axis colours used by the world axes (X red,
/// Y green, Z blue), white for the centre.
fn color(handle: Handle) -> Color32 {
    match handle.axis() {
        Some(Axis::X) => Color32::from_rgb(225, 64, 64),
        Some(Axis::Y) => Color32::from_rgb(96, 196, 72),
        Some(Axis::Z) => Color32::from_rgb(72, 128, 235),
        None => Color32::from_gray(230),
    }
}

/// Paints the gizmo's shapes, which are in points from `rect`'s top-left.
pub(super) fn draw(painter: &Painter, rect: Rect, shapes: &[GizmoShape]) {
    let at = |p: glam::Vec2| -> Pos2 { rect.min + vec2(p.x, p.y) };
    // Rings first, so arrows and cubes stay on top of them.
    let mut ordered: Vec<&GizmoShape> = shapes.iter().collect();
    ordered.sort_by_key(|s| !matches!(s.handle, Handle::Rotate(_)));
    for shape in ordered {
        let base = if shape.highlighted {
            HIGHLIGHT
        } else {
            color(shape.handle)
        };
        let points: Vec<Pos2> = shape.points.iter().map(|p| at(*p)).collect();
        if shape.filled {
            let fill = match shape.handle {
                // Plane squares are see-through, so they never hide the
                // brush being moved.
                Handle::MovePlane(_) if !shape.highlighted => base.gamma_multiply(0.45),
                _ => base,
            };
            painter.add(Shape::convex_polygon(
                points,
                fill,
                Stroke::new(1.0, Color32::from_black_alpha(140)),
            ));
        } else {
            let width = if shape.highlighted { 3.5 } else { 2.5 };
            painter.add(Shape::line(points, Stroke::new(width, base)));
        }
    }
}
