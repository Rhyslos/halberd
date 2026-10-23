//! Drawing a floor plan being drawn on top of the viewport image, with a
//! line saying what to do next.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, vec2};
use halberd_tools::{Camera, PlanPreview};

/// The outline being drawn (the Draw tool's preview yellow).
const LINE: Color32 = Color32::from_rgb(255, 204, 64);

/// Paints `plan` (world points) seen from `camera` over `rect`.
pub(super) fn draw(painter: &Painter, rect: Rect, camera: &Camera, plan: &PlanPreview) {
    let size = glam::Vec2::new(rect.width().max(1.0), rect.height().max(1.0));
    let at = |p: glam::Vec3| -> Option<Pos2> {
        camera
            .project(p, size)
            .filter(|s| s.is_finite())
            .map(|s| rect.min + vec2(s.x, s.y))
    };
    let corners: Vec<Option<Pos2>> = plan.corners.iter().map(|c| at(*c)).collect();
    for pair in corners.windows(2) {
        if let [Some(a), Some(b)] = pair {
            painter.line_segment([*a, *b], Stroke::new(2.0, LINE));
        }
    }
    if let (Some(Some(last)), Some(next)) = (corners.last(), plan.next.and_then(at)) {
        painter.line_segment([*last, next], Stroke::new(1.5, LINE.gamma_multiply(0.8)));
        // Where the outline would close.
        if let Some(Some(first)) = corners.first()
            && corners.len() >= 2
        {
            painter.line_segment([next, *first], Stroke::new(1.0, LINE.gamma_multiply(0.35)));
        }
    }
    for (i, c) in corners.iter().enumerate() {
        if let Some(c) = c {
            painter.circle_filled(*c, 3.5, LINE);
            if i == 0 && plan.closes {
                painter.circle_stroke(*c, 8.0, Stroke::new(2.0, LINE));
            }
        }
    }
}

/// The line under the toolbars saying what to do next.
pub(super) fn hint(painter: &Painter, rect: Rect, plan: Option<&PlanPreview>) {
    let text = match plan {
        None => "Floor plan: click to place the first corner.".to_string(),
        Some(p) if p.closes => "Click to close the outline.".to_string(),
        Some(p) => format!(
            "Floor plan, {} corner{}: click the next corner. Click the first corner or press \
             Enter to finish · Backspace takes one back · Esc cancels.",
            p.corners.len(),
            if p.corners.len() == 1 { "" } else { "s" }
        ),
    };
    painter.text(
        rect.left_top() + vec2(12.0, 98.0),
        Align2::LEFT_TOP,
        text,
        FontId::proportional(13.0),
        LINE,
    );
}
