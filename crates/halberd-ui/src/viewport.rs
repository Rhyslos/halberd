//! The 3D viewport panel: turns mouse and keyboard input into camera moves,
//! shows the rendered image, and draws the camera mode switcher.
//!
//! Rendering itself happens elsewhere, behind the [`ViewportRenderer`]
//! trait, so this panel (and its tests) need no GPU.

use egui::{
    Align, Align2, Color32, FontId, Key, Layout, PointerButton, Rect, Sense, Stroke, Ui, UiBuilder,
    pos2, vec2,
};
use glam::{Mat4, Vec2, Vec3};
use halberd_tools::{
    CameraController, CameraMode, CameraState, FlyKeys, GroundPlane, ViewportInput,
};

/// What a renderer needs to draw one viewport frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportView {
    /// Size of the image to draw, in physical pixels.
    pub size_px: [u32; 2],
    /// World-to-screen transform.
    pub view_projection: Mat4,
    /// Where the camera is.
    pub camera_position: Vec3,
    /// The editor grid size, in units.
    pub grid_size: f32,
}

/// Draws viewport images. The editor implements this with the GPU; tests
/// and machines without a usable GPU use [`NoRenderer`].
pub trait ViewportRenderer {
    /// Draws a frame and returns the egui texture showing it, or a
    /// plain-language reason why it could not.
    fn render(&mut self, view: &ViewportView) -> Result<egui::TextureId, String>;
}

/// A renderer that never draws, with a reason to show instead.
#[derive(Debug, Clone)]
pub struct NoRenderer {
    /// Why there is no 3D view.
    pub reason: String,
}

impl Default for NoRenderer {
    fn default() -> Self {
        Self {
            reason: "no graphics device".to_string(),
        }
    }
}

impl ViewportRenderer for NoRenderer {
    fn render(&mut self, _view: &ViewportView) -> Result<egui::TextureId, String> {
        Err(self.reason.clone())
    }
}

/// Settings the viewport reads at startup.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportOptions {
    /// Whether WASD flying (and so Fly mode) is available.
    pub wasd_enabled: bool,
    /// The editor grid size, in units.
    pub grid_size: f32,
}

impl Default for ViewportOptions {
    fn default() -> Self {
        Self {
            wasd_enabled: true,
            grid_size: 16.0,
        }
    }
}

/// Accessible name of the viewport, and the label tests look for.
pub const VIEWPORT_LABEL: &str = "3D viewport";
/// Pivot marker colour.
const PIVOT_COLOR: Color32 = Color32::from_rgb(255, 196, 64);

/// The viewport panel's state.
pub struct ViewportPanel {
    controller: CameraController,
    grid_size: f32,
    right_was_held: bool,
    middle_was_held: bool,
}

impl ViewportPanel {
    /// A viewport starting from a saved camera (or the default one).
    pub fn new(saved: Option<CameraState>, options: ViewportOptions) -> Self {
        Self {
            controller: CameraController::new(saved.unwrap_or_default(), options.wasd_enabled),
            grid_size: options.grid_size,
            right_was_held: false,
            middle_was_held: false,
        }
    }

    /// The camera state to save between sessions.
    pub fn camera_state(&self) -> CameraState {
        self.controller.state()
    }

    /// The camera controller, for reading the camera and mode.
    pub fn controller(&self) -> &CameraController {
        &self.controller
    }

    /// Draws the viewport filling `ui` and handles its input.
    pub fn show(&mut self, ui: &mut Ui, renderer: &mut dyn ViewportRenderer) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, VIEWPORT_LABEL)
        });

        let input = self.gather_input(ui, &response, rect);
        let animating = self.controller.update(&input, &GroundPlane);
        // Pointer movement already triggers redraws; only flying with a key
        // held needs frames without new input.
        if animating {
            ui.ctx().request_repaint();
        }

        let size = Vec2::new(rect.width().max(1.0), rect.height().max(1.0));
        let ppp = ui.ctx().pixels_per_point();
        let camera = *self.controller.camera();
        let view = ViewportView {
            size_px: [
                (rect.width() * ppp).round().max(1.0) as u32,
                (rect.height() * ppp).round().max(1.0) as u32,
            ],
            view_projection: camera.view_projection(size),
            camera_position: camera.position,
            grid_size: self.grid_size,
        };

        let painter = ui.painter_at(rect);
        match renderer.render(&view) {
            Ok(texture) => {
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                painter.image(texture, rect, uv, Color32::WHITE);
            }
            Err(reason) => {
                painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 20, 24));
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    format!("3D view unavailable: {reason}"),
                    FontId::proportional(14.0),
                    Color32::from_gray(150),
                );
            }
        }

        if let Some(pivot) = self.controller.active_pivot()
            && let Some(at) = camera.project(pivot, size)
        {
            let centre = rect.min + vec2(at.x, at.y);
            painter.circle_stroke(centre, 6.0, Stroke::new(1.5, PIVOT_COLOR));
            painter.circle_filled(centre, 2.0, PIVOT_COLOR);
        }

        painter.text(
            rect.left_top() + vec2(8.0, 6.0),
            Align2::LEFT_TOP,
            "Perspective",
            FontId::proportional(12.0),
            Color32::from_gray(160),
        );

        self.mode_switcher(ui, rect);
    }

    /// Reads this frame's mouse and keyboard input for the viewport.
    fn gather_input(&mut self, ui: &Ui, response: &egui::Response, rect: Rect) -> ViewportInput {
        let pressed_here = response.is_pointer_button_down_on();
        let (right_down, middle_down, pointer_delta, dt, keys) = ui.input(|i| {
            let keys = FlyKeys {
                forward: i.key_down(Key::W),
                back: i.key_down(Key::S),
                left: i.key_down(Key::A),
                right: i.key_down(Key::D),
                up: i.key_down(Key::Space),
                down: i.key_down(Key::C),
                fast: i.modifiers.shift,
            };
            (
                i.pointer.button_down(PointerButton::Secondary),
                i.pointer.button_down(PointerButton::Middle),
                i.pointer.delta(),
                i.stable_dt,
                keys,
            )
        });
        let right_held = right_down && (pressed_here || self.right_was_held);
        let middle_held = middle_down && (pressed_here || self.middle_was_held);
        let right_pressed = right_held && !self.right_was_held;
        let middle_pressed = middle_held && !self.middle_was_held;
        self.right_was_held = right_held;
        self.middle_was_held = middle_held;

        let scroll = if response.hovered() {
            let delta = ui.input(|i| i.smooth_scroll_delta.y);
            ui.ctx()
                .input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
            delta
        } else {
            0.0
        };

        let cursor = response
            .interact_pointer_pos()
            .or_else(|| response.hover_pos())
            .map(|p| Vec2::new(p.x - rect.min.x, p.y - rect.min.y));
        let delta = Vec2::new(pointer_delta.x, pointer_delta.y);

        ViewportInput {
            size: Vec2::new(rect.width(), rect.height()),
            cursor,
            right_pressed,
            right_held,
            right_delta: if right_held && !right_pressed {
                delta
            } else {
                Vec2::ZERO
            },
            middle_pressed,
            middle_held,
            middle_delta: if middle_held && !middle_pressed {
                delta
            } else {
                Vec2::ZERO
            },
            scroll,
            keys,
            dt,
        }
    }

    /// The camera mode buttons in the viewport's bottom-left corner.
    fn mode_switcher(&mut self, ui: &mut Ui, rect: Rect) {
        let area = Rect::from_min_size(
            rect.left_bottom() + vec2(8.0, -38.0),
            vec2((rect.width() - 16.0).max(0.0), 30.0),
        );
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(area)
                .layout(Layout::left_to_right(Align::Center)),
        );
        egui::Frame::new()
            .fill(Color32::from_black_alpha(170))
            .corner_radius(4.0)
            .inner_margin(4.0)
            .show(&mut child, |ui| {
                ui.label("Camera:");
                let current = self.controller.mode();
                for mode in self.controller.available_modes() {
                    if ui
                        .selectable_label(current == mode, mode.label())
                        .on_hover_text(mode.description())
                        .clicked()
                    {
                        self.controller.set_mode(mode);
                    }
                }
                if current == CameraMode::Fly {
                    ui.label(format!("speed {:.0}", self.controller.fly_speed()))
                        .on_hover_text("Scroll while flying to change speed");
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Modifiers};
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    struct Recording {
        views: Vec<ViewportView>,
    }

    impl ViewportRenderer for Recording {
        fn render(&mut self, view: &ViewportView) -> Result<egui::TextureId, String> {
            self.views.push(*view);
            Err("recording only".into())
        }
    }

    struct State {
        panel: ViewportPanel,
        renderer: Recording,
    }

    fn harness(options: ViewportOptions) -> Harness<'static, State> {
        Harness::builder().with_size([800.0, 600.0]).build_ui_state(
            |ui, s: &mut State| s.panel.show(ui, &mut s.renderer),
            State {
                panel: ViewportPanel::new(None, options),
                renderer: Recording { views: Vec::new() },
            },
        )
    }

    fn press(h: &Harness<'_, State>, button: PointerButton, at: egui::Pos2, pressed: bool) {
        h.event(Event::PointerButton {
            pos: at,
            button,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }

    #[test]
    fn renders_at_the_panel_size_in_pixels() {
        let mut h = harness(ViewportOptions::default());
        h.run();
        let view = *h.state().renderer.views.last().unwrap();
        assert!(
            view.size_px[0] >= 780 && view.size_px[1] >= 580,
            "{:?}",
            view.size_px
        );
        assert_eq!(view.grid_size, 16.0);
        assert!(view.view_projection.is_finite());
    }

    #[test]
    fn missing_renderer_shows_a_reason_instead_of_crashing() {
        let mut h = Harness::builder().with_size([400.0, 300.0]).build_ui_state(
            |ui, panel: &mut ViewportPanel| panel.show(ui, &mut NoRenderer::default()),
            ViewportPanel::new(None, ViewportOptions::default()),
        );
        h.run();
        assert!(h.query_by_label(VIEWPORT_LABEL).is_some());
    }

    #[test]
    fn right_drag_orbits_the_camera() {
        let mut h = harness(ViewportOptions::default());
        h.run();
        let yaw = h.state().panel.controller().camera().yaw;
        let start = egui::pos2(400.0, 400.0);
        h.hover_at(start);
        press(&h, PointerButton::Secondary, start, true);
        h.run();
        h.hover_at(start + vec2(60.0, 0.0));
        h.run();
        assert!(
            h.state().panel.controller().active_pivot().is_some(),
            "pivot marker shown"
        );
        press(&h, PointerButton::Secondary, start + vec2(60.0, 0.0), false);
        h.run();
        assert!(
            h.state().panel.controller().camera().yaw < yaw,
            "dragging right turns right"
        );
        assert!(
            h.state().panel.controller().active_pivot().is_none(),
            "marker gone"
        );
    }

    #[test]
    fn middle_drag_pans_the_camera() {
        let mut h = harness(ViewportOptions::default());
        h.run();
        let before = h.state().panel.controller().camera().position;
        let start = egui::pos2(400.0, 300.0);
        h.hover_at(start);
        press(&h, PointerButton::Middle, start, true);
        h.run();
        h.hover_at(start + vec2(-40.0, 25.0));
        h.run();
        press(&h, PointerButton::Middle, start + vec2(-40.0, 25.0), false);
        h.run();
        let after = h.state().panel.controller().camera().position;
        assert!(before.distance(after) > 1.0);
    }

    #[test]
    fn scrolling_zooms() {
        let mut h = harness(ViewportOptions::default());
        h.run();
        let before = h.state().panel.controller().camera().position.length();
        h.hover_at(egui::pos2(400.0, 300.0));
        h.event(Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, 120.0),
            modifiers: Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        for _ in 0..30 {
            h.step();
        }
        let after = h.state().panel.controller().camera().position.length();
        assert!(
            after < before,
            "scrolling up moves closer: {before} -> {after}"
        );
    }

    #[test]
    fn fly_mode_moves_with_right_mouse_and_w() {
        let mut h = harness(ViewportOptions::default());
        h.run();
        h.get_by_label("Fly").click();
        h.run();
        let start = h.state().panel.controller().camera().position;
        let forward = h.state().panel.controller().camera().forward();
        let centre = egui::pos2(400.0, 300.0);
        h.hover_at(centre);
        press(&h, PointerButton::Secondary, centre, true);
        h.key_down(Key::W);
        // Flying keeps requesting frames, so step instead of run.
        for _ in 0..20 {
            h.step();
        }
        h.key_up(Key::W);
        press(&h, PointerButton::Secondary, centre, false);
        h.step();
        let moved = h.state().panel.controller().camera().position - start;
        assert!(moved.length() > 1.0, "camera should have flown forward");
        assert!(
            moved.normalize().dot(forward) > 0.99,
            "moved along the view direction"
        );
    }

    #[test]
    fn w_alone_does_not_fly() {
        let mut h = harness(ViewportOptions::default());
        h.run();
        h.get_by_label("Fly").click();
        h.run();
        let start = h.state().panel.controller().camera().position;
        h.hover_at(egui::pos2(400.0, 300.0));
        h.key_down(Key::W);
        for _ in 0..10 {
            h.step();
        }
        h.key_up(Key::W);
        h.step();
        assert_eq!(h.state().panel.controller().camera().position, start);
    }

    #[test]
    fn mode_switcher_changes_mode() {
        let mut h = harness(ViewportOptions::default());
        h.run();
        h.get_by_label("Fly").click();
        h.run();
        assert_eq!(h.state().panel.controller().mode(), CameraMode::Fly);
        h.get_by_label("Orbit").click();
        h.run();
        assert_eq!(h.state().panel.controller().mode(), CameraMode::Orbit);
    }

    #[test]
    fn fly_button_is_hidden_when_wasd_is_off() {
        let mut h = harness(ViewportOptions {
            wasd_enabled: false,
            grid_size: 16.0,
        });
        h.run();
        assert!(h.query_by_label("Default").is_some());
        assert!(h.query_by_label("Fly").is_none());
    }

    #[test]
    fn clicking_the_switcher_does_not_move_the_camera() {
        let mut h = harness(ViewportOptions::default());
        h.run();
        let before = *h.state().panel.controller().camera();
        h.get_by_label("Orbit").click();
        h.run();
        assert_eq!(*h.state().panel.controller().camera(), before);
    }

    #[test]
    fn saved_camera_is_restored() {
        let mut state = CameraState::default();
        state.camera.position = Vec3::new(10.0, 20.0, 30.0);
        state.mode = CameraMode::Orbit;
        let panel = ViewportPanel::new(Some(state), ViewportOptions::default());
        assert_eq!(panel.camera_state(), state);
    }
}
