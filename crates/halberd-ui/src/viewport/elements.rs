//! Drawing the corners, edges and faces that can be picked (Vertex, Edge
//! and Face modes) on top of the viewport image.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, vec2};
use halberd_tools::{ElementOverlay, ScreenElement, SelectMode};

/// Parts that can be picked.
const CANDIDATE: Color32 = Color32::from_rgb(170, 200, 235);
/// Picked parts.
const PICKED: Color32 = Color32::from_rgb(255, 120, 40);
/// The part under the pointer.
const HOVERED: Color32 = Color32::from_rgb(255, 225, 80);

/// Paints `overlay`, whose positions are in points from `rect`'s top-left.
pub(super) fn draw(painter: &Painter, rect: Rect, overlay: &ElementOverlay) {
    let at = |p: glam::Vec2| -> Pos2 { rect.min + vec2(p.x, p.y) };
    let paint = |element: &ScreenElement, color: Color32, strong: bool| match element {
        ScreenElement::Point(p) => {
            let half = if strong { 4.5 } else { 3.0 };
            let square = Rect::from_center_size(at(*p), vec2(half * 2.0, half * 2.0));
            painter.rect_filled(square, 0.0, color);
            painter.rect_stroke(
                square,
                0.0,
                Stroke::new(1.0, Color32::from_black_alpha(180)),
                egui::StrokeKind::Outside,
            );
        }
        ScreenElement::Segment(a, b) => {
            let width = if strong { 3.0 } else { 1.5 };
            painter.line_segment([at(*a), at(*b)], Stroke::new(width, color));
        }
        ScreenElement::Polygon(points) => {
            let points: Vec<Pos2> = points.iter().map(|p| at(*p)).collect();
            painter.add(Shape::convex_polygon(
                points,
                color.gamma_multiply(0.35),
                Stroke::new(2.5, color),
            ));
        }
    };
    for element in &overlay.candidates {
        paint(element, CANDIDATE, false);
    }
    for element in &overlay.picked {
        paint(element, PICKED, true);
    }
    if let Some(element) = &overlay.hovered {
        paint(element, HOVERED, true);
    }
}

/// A line under the toolbar saying what to do first in a part-picking
/// mode with nothing selected.
pub(super) fn hint(painter: &Painter, rect: Rect, mode: SelectMode) {
    let part = match mode {
        SelectMode::Object => return,
        SelectMode::Vertex => "corners",
        SelectMode::Edge => "edges",
        SelectMode::Face => "faces",
    };
    painter.text(
        rect.left_top() + vec2(12.0, 98.0),
        Align2::LEFT_TOP,
        format!(
            "{} mode: click a brush to select it, then click its {part}.",
            mode.label()
        ),
        FontId::proportional(13.0),
        HOVERED,
    );
}
