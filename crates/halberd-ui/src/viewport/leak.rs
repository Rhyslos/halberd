//! The leak path: when a compile finds that the map isn't sealed, the
//! compiler leaves a line from the empty space outside the map to the
//! entity that "sees" it. Every view draws that line over the map in red,
//! with the two ends marked, and a banner offers to fly to it or hide it.

use super::ViewportPanel;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Ui, vec2};
use glam::{Vec2, Vec3};
use halberd_geom::Aabb;
use halberd_tools::{Camera, NEAR_PLANE};

/// The banner's button that points the views at the leak.
pub const GO_TO_LEAK_LABEL: &str = "Go to leak";
/// The banner's button that hides the leak path.
pub const HIDE_LEAK_LABEL: &str = "Hide leak";
/// The banner's text, for tests.
pub const LEAK_BANNER_TEXT: &str = "The map has a leak";

/// Colour of the leak line.
const LEAK_COLOR: Color32 = Color32::from_rgb(255, 60, 60);
/// Colour of the dark edge that keeps the line readable over red brushes.
const EDGE_COLOR: Color32 = Color32::from_black_alpha(200);
/// Colour of the end markers' labels.
const LABEL_COLOR: Color32 = Color32::from_rgb(255, 170, 170);
/// The camera keeps this far from the near plane when cutting lines that
/// run behind it, so their ends never project to infinity.
const CLIP_MARGIN: f32 = 0.5;

/// A leak path from the compiler: from outside the map (first point) to
/// the leaking entity (last point).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct LeakPath {
    points: Vec<Vec3>,
    shown: bool,
}

impl ViewportPanel {
    /// Shows the leak path a compile found (from outside to the leaking
    /// entity), or clears it with `None` or fewer than two points.
    pub fn set_leak_path(&mut self, points: Option<Vec<Vec3>>) {
        self.leak = points
            .map(|p| p.into_iter().filter(|v| v.is_finite()).collect::<Vec<_>>())
            .filter(|p| p.len() >= 2)
            .map(|points| LeakPath {
                points,
                shown: true,
            });
    }

    /// The leak path being shown, if any.
    pub fn leak_path(&self) -> Option<&[Vec3]> {
        self.leak
            .as_ref()
            .filter(|l| l.shown)
            .map(|l| l.points.as_slice())
    }

    /// True if the last compile found a leak path (shown or hidden).
    pub fn has_leak(&self) -> bool {
        self.leak.is_some()
    }

    /// Shows or hides the leak path, if there is one.
    pub fn set_leak_shown(&mut self, shown: bool) {
        if let Some(leak) = &mut self.leak {
            leak.shown = shown;
        }
    }

    /// Points every view at the whole leak path.
    pub fn go_to_leak(&mut self) {
        let Some(path) = &self.leak else {
            return;
        };
        // A little room around it, so a short line isn't filling the
        // screen edge to edge.
        let room = Vec3::splat(64.0);
        if let Some(bounds) = Aabb::from_points(path.points.iter().copied()) {
            self.frame(Aabb::from_corners(bounds.min - room, bounds.max + room));
        }
    }

    /// Draws the leak path over one view.
    pub(super) fn draw_leak(&self, painter: &Painter, rect: Rect, camera: &Camera, size: Vec2) {
        if let Some(path) = self.leak_path() {
            draw(painter, rect, camera, size, path);
        }
    }

    /// The banner across the bottom of the viewport while a leak is shown.
    pub(super) fn leak_banner(&mut self, ui: &mut Ui, full: Rect) {
        if self.leak_path().is_none() {
            return;
        }
        // Its own layer (sized to fit), so clicks on it never reach the
        // view underneath.
        let mut go = false;
        let mut hide = false;
        egui::Area::new(ui.id().with("halberd_leak_banner"))
            .fixed_pos(Pos2::new(full.center().x, full.bottom() - 16.0))
            .pivot(Align2::CENTER_BOTTOM)
            .constrain_to(full)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(90, 16, 16, 235))
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.set_max_width(full.width() - 16.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                egui::RichText::new(LEAK_BANNER_TEXT)
                                    .strong()
                                    .color(LABEL_COLOR),
                            );
                            ui.label(
                                egui::RichText::new(
                                    "(red line: from outside to the leaking entity)",
                                )
                                .small()
                                .color(Color32::from_gray(210)),
                            )
                            .on_hover_text(
                                "Light and visibility can't be worked out until the map is \
                                 sealed. Follow the red line from the outside in: where it passes \
                                 through a wall, there is a gap to close (or an entity sitting \
                                 outside the map).",
                            );
                            go = ui.button(GO_TO_LEAK_LABEL).clicked();
                            hide = ui
                                .button(HIDE_LEAK_LABEL)
                                .on_hover_text("View → Show leak path brings it back")
                                .clicked();
                        });
                    });
            });
        if go {
            self.go_to_leak();
        }
        if hide {
            self.set_leak_shown(false);
        }
    }
}

/// The part of the segment `a`–`b` in front of the camera, or `None` if
/// all of it is behind.
fn clip_to_view(camera: &Camera, a: Vec3, b: Vec3) -> Option<(Vec3, Vec3)> {
    if camera.ortho.is_some() {
        return Some((a, b));
    }
    let near = NEAR_PLANE + CLIP_MARGIN;
    let (da, db) = (camera.depth_of(a), camera.depth_of(b));
    match (da >= near, db >= near) {
        (true, true) => Some((a, b)),
        (false, false) => None,
        (front_a, _) => {
            let cut = a + (b - a) * ((near - da) / (db - da));
            Some(if front_a { (a, cut) } else { (cut, b) })
        }
    }
}

/// Draws `path` (at least two points) over the view in `rect`.
pub(super) fn draw(painter: &Painter, rect: Rect, camera: &Camera, size: Vec2, path: &[Vec3]) {
    let to_screen = |p: Vec3| {
        camera
            .project(p, size)
            .map(|at| rect.min + vec2(at.x, at.y))
    };
    for pair in path.windows(2) {
        let Some((a, b)) = clip_to_view(camera, pair[0], pair[1]) else {
            continue;
        };
        if let (Some(a), Some(b)) = (to_screen(a), to_screen(b)) {
            painter.line_segment([a, b], Stroke::new(5.0, EDGE_COLOR));
            painter.line_segment([a, b], Stroke::new(2.5, LEAK_COLOR));
        }
    }
    let ends = [
        (path.first(), "Outside", false),
        (path.last(), "Leaking entity", true),
    ];
    for (point, label, entity) in ends {
        let Some(at) = point
            .filter(|p| camera.ortho.is_some() || camera.depth_of(**p) > NEAR_PLANE)
            .and_then(|p| to_screen(*p))
            .filter(|at| rect.contains(*at))
        else {
            continue;
        };
        if entity {
            painter.circle_filled(at, 6.0, LEAK_COLOR);
            painter.circle_stroke(at, 6.0, Stroke::new(1.5, EDGE_COLOR));
        } else {
            let square = Rect::from_center_size(at, vec2(10.0, 10.0));
            painter.rect_stroke(
                square,
                0.0,
                Stroke::new(2.0, LEAK_COLOR),
                egui::StrokeKind::Middle,
            );
        }
        painter.text(
            at + vec2(0.0, -10.0),
            Align2::CENTER_BOTTOM,
            label,
            FontId::proportional(11.0),
            LABEL_COLOR,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn camera_at_origin_looking_along_x() -> Camera {
        Camera::looking_at(Vec3::ZERO, Vec3::X)
    }

    #[test]
    fn segments_in_front_are_kept_whole() {
        let camera = camera_at_origin_looking_along_x();
        let (a, b) = (Vec3::new(100.0, 0.0, 0.0), Vec3::new(200.0, 50.0, 0.0));
        assert_eq!(clip_to_view(&camera, a, b), Some((a, b)));
    }

    #[test]
    fn segments_behind_are_dropped() {
        let camera = camera_at_origin_looking_along_x();
        let (a, b) = (Vec3::new(-100.0, 0.0, 0.0), Vec3::new(-5.0, 50.0, 0.0));
        assert_eq!(clip_to_view(&camera, a, b), None);
    }

    #[test]
    fn segments_through_the_camera_are_cut_at_the_near_plane() {
        let camera = camera_at_origin_looking_along_x();
        let (a, b) = (Vec3::new(-100.0, 0.0, 0.0), Vec3::new(100.0, 0.0, 0.0));
        let (cut, end) = clip_to_view(&camera, a, b).expect("half is in front");
        assert_eq!(end, b);
        assert!((camera.depth_of(cut) - (NEAR_PLANE + CLIP_MARGIN)).abs() < 1e-3);
        assert!(camera.project(cut, Vec2::new(800.0, 600.0)).is_some());
        // And the other way round.
        let (start, cut) = clip_to_view(&camera, b, a).expect("half is in front");
        assert_eq!(start, b);
        assert!((camera.depth_of(cut) - (NEAR_PLANE + CLIP_MARGIN)).abs() < 1e-3);
    }
}
