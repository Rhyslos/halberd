//! Sizes shown in the views: the box being drawn (width × depth × height
//! above it, in the chosen unit) and the selection's size in the corner of
//! the first view.

use egui::{Align2, Color32, FontId, Painter, Rect, vec2};
use glam::Vec2;
use halberd_config::LengthUnit;
use halberd_geom::Aabb;
use halberd_tools::Camera;

/// Colour of the box-being-drawn readout (the preview's yellow).
const DRAWING_COLOR: Color32 = Color32::from_rgb(255, 205, 64);
/// Colour of the selection readout.
const SELECTION_COLOR: Color32 = Color32::from_gray(200);
/// Start of the selection readout, for tests.
pub(crate) const SELECTION_READOUT: &str = "Selection:";

/// "256 × 128 × 64 u": width (X), depth (Y) and height (Z).
pub(crate) fn size_text(bounds: Aabb, unit: LengthUnit) -> String {
    let s = bounds.size();
    let n = |v: f32| {
        let text = unit.format(f64::from(v));
        text.trim_end_matches(unit.suffix()).to_string()
    };
    format!("{} × {} × {}{}", n(s.x), n(s.y), n(s.z), unit.suffix())
}

/// Text on a dark backing, so it reads over the grid and brushes.
fn backed_text(
    painter: &Painter,
    at: egui::Pos2,
    align: Align2,
    text: String,
    size: f32,
    color: Color32,
) {
    let galley = painter.layout_no_wrap(text, FontId::proportional(size), color);
    let rect = align.anchor_size(at, galley.size());
    painter.rect_filled(rect.expand(3.0), 3.0, Color32::from_black_alpha(170));
    painter.galley(rect.min, galley, color);
}

/// Draws the readouts for one view.
pub(super) fn draw(
    painter: &Painter,
    rect: Rect,
    camera: &Camera,
    size: Vec2,
    drawing: Option<Aabb>,
    selection: Option<Aabb>,
    unit: LengthUnit,
) {
    if let Some(b) = drawing
        && let Some(at) = camera.project(b.center().with_z(b.max.z), size)
    {
        let at = rect.min + vec2(at.x, at.y - 10.0);
        if rect.contains(at) {
            backed_text(
                painter,
                at,
                Align2::CENTER_BOTTOM,
                size_text(b, unit),
                13.0,
                DRAWING_COLOR,
            );
        }
        return;
    }
    if let Some(b) = selection {
        backed_text(
            painter,
            rect.right_bottom() + vec2(-10.0, -10.0),
            Align2::RIGHT_BOTTOM,
            format!("{SELECTION_READOUT} {}", size_text(b, unit)),
            12.0,
            SELECTION_COLOR,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn sizes_read_width_depth_height_in_the_chosen_unit() {
        let b = Aabb::from_corners(Vec3::ZERO, Vec3::new(256.0, 128.0, 72.0));
        assert_eq!(size_text(b, LengthUnit::Units), "256 × 128 × 72 u");
        assert_eq!(size_text(b, LengthUnit::Metres), "6.50 × 3.25 × 1.83 m");
        assert_eq!(size_text(b, LengthUnit::Feet), "21.3 × 10.7 × 6.0 ft");
    }
}
