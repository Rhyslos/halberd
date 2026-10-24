//! The Sculpt tool in the viewport: its toolbar row (what dragging does,
//! the brush's size and strength) and the brush's circle on the surface.

use super::ViewportPanel;
use egui::{Align, Color32, Layout, Painter, Rect, Stroke, Ui, UiBuilder, vec2};
use glam::Vec2;
use halberd_tools::{Camera, SculptMode, SculptSettings};

/// Accessible name of the brush size field.
pub const SCULPT_RADIUS_NAME: &str = "Sculpt brush size";
/// Accessible name of the strength field.
pub const SCULPT_STRENGTH_NAME: &str = "Sculpt strength";
/// Colour of the brush's circle.
const BRUSH_COLOR: Color32 = Color32::from_rgb(255, 220, 90);

impl ViewportPanel {
    /// The Sculpt tool's second toolbar row.
    pub(super) fn sculpt_options(&mut self, ui: &mut Ui, rect: Rect) {
        let area = Rect::from_min_size(
            rect.left_top() + vec2(8.0, 60.0),
            vec2((rect.width() - 16.0).max(0.0), 30.0),
        );
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(area)
                .layout(Layout::left_to_right(Align::Center)),
        );
        let mut settings = self.tools.sculpt_settings();
        egui::Frame::new()
            .fill(Color32::from_black_alpha(170))
            .corner_radius(4.0)
            .inner_margin(4.0)
            .show(&mut child, |ui| {
                for mode in SculptMode::ALL {
                    if ui
                        .selectable_label(settings.mode == mode, mode.label())
                        .on_hover_text(mode.description())
                        .clicked()
                    {
                        settings.mode = mode;
                    }
                }
                ui.separator();
                ui.label("Size");
                let (lo, hi) = SculptSettings::RADIUS_RANGE;
                let size = ui.add(
                    egui::DragValue::new(&mut settings.radius)
                        .range(lo..=hi)
                        .speed(2.0)
                        .suffix(" u"),
                );
                size.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::DragValue, true, SCULPT_RADIUS_NAME)
                });
                ui.label("Strength");
                let (lo, hi) = SculptSettings::STRENGTH_RANGE;
                let strength = ui.add(egui::DragValue::new(&mut settings.strength).range(lo..=hi));
                strength.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::DragValue,
                        true,
                        SCULPT_STRENGTH_NAME,
                    )
                });
                ui.label(egui::RichText::new("Ctrl: the other way").weak());
            });
        self.tools.set_sculpt_settings(settings);
    }

    /// The brush's circle where it is on a displacement, in one view.
    pub(super) fn draw_sculpt_brush(
        &self,
        painter: &Painter,
        rect: Rect,
        camera: &Camera,
        size: Vec2,
    ) {
        let Some(at) = self.tools.sculpt_at() else {
            return;
        };
        let Some(center) = camera.project(at, size) else {
            return;
        };
        let radius_points =
            self.tools.sculpt_settings().radius / camera.units_per_point(at, size.y).max(1e-3);
        painter.circle_stroke(
            rect.min + vec2(center.x, center.y),
            radius_points.min(4000.0),
            Stroke::new(1.5, BRUSH_COLOR),
        );
    }
}
