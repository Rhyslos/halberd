//! The 3D viewport panel: turns mouse and keyboard input into camera moves
//! and tool actions, shows the rendered image, and draws the tool and
//! camera mode switchers.
//!
//! Rendering itself happens elsewhere, behind the [`ViewportRenderer`]
//! trait, so this panel (and its tests) need no GPU.

use egui::{
    Align, Align2, Color32, FontId, Key, Layout, PointerButton, Rect, Sense, Stroke, Ui, UiBuilder,
    pos2, vec2,
};
use glam::{Mat4, Vec2, Vec3};
use halberd_config::LengthUnit;
use halberd_doc::Document;
use halberd_geom::Aabb;
use halberd_tools::{
    CameraController, CameraMode, CameraState, DocumentScene, FlyKeys, PLAYER_HEIGHT, Tool,
    ToolController, ViewportInput, player_bounds,
};

mod gizmo;
mod tools;

pub use tools::INSIDE_ENTITIES_LABEL;

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
    /// A box being drawn, to outline.
    pub preview: Option<Aabb>,
    /// Where to draw the player figure for scale, if shown.
    pub player: Option<Aabb>,
}

/// Draws viewport images. The editor implements this with the GPU; tests
/// and machines without a usable GPU use [`NoRenderer`].
pub trait ViewportRenderer {
    /// Draws a frame of `doc` and returns the egui texture showing it, or a
    /// plain-language reason why it could not.
    fn render(&mut self, view: &ViewportView, doc: &Document) -> Result<egui::TextureId, String>;
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
    fn render(&mut self, _view: &ViewportView, _doc: &Document) -> Result<egui::TextureId, String> {
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
/// Colour of the player figure's label (matches the figure).
const PLAYER_LABEL_COLOR: Color32 = Color32::from_rgb(120, 190, 255);
/// Gap between the player figure and what it stands next to, in units.
const PLAYER_GAP: f32 = 32.0;

/// The viewport panel's state.
pub struct ViewportPanel {
    controller: CameraController,
    tools: ToolController,
    grid_size: f32,
    left_was_held: bool,
    /// A menu or popup was open when this frame began.
    popup_was_open: bool,
    /// A box ("save changes?", a problem) is showing: tool keys wait.
    keys_blocked: bool,
    length_unit: LengthUnit,
    show_player: bool,
    right_was_held: bool,
    middle_was_held: bool,
}

impl ViewportPanel {
    /// A viewport starting from a saved camera (or the default one).
    pub fn new(saved: Option<CameraState>, options: ViewportOptions) -> Self {
        Self {
            controller: CameraController::new(saved.unwrap_or_default(), options.wasd_enabled),
            tools: ToolController::new(options.grid_size),
            grid_size: options.grid_size,
            left_was_held: false,
            popup_was_open: false,
            keys_blocked: false,
            length_unit: LengthUnit::Units,
            show_player: true,
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

    /// The active left-mouse tool.
    pub fn tool(&self) -> Tool {
        self.tools.tool()
    }

    /// Switches the left-mouse tool.
    pub fn set_tool(&mut self, tool: Tool) {
        self.tools.set_tool(tool);
    }

    /// The unit lengths are shown in, and whether the player figure is
    /// shown. The workbench calls this every frame.
    pub fn set_display(&mut self, length_unit: LengthUnit, show_player: bool) {
        self.length_unit = length_unit;
        self.show_player = show_player;
    }

    /// The tools, for reading the Box tool's height.
    pub fn tools(&self) -> &ToolController {
        &self.tools
    }

    /// Points the camera at the whole of `bounds` (a newly opened map).
    pub fn frame(&mut self, bounds: Aabb) {
        self.controller.frame(bounds);
    }

    /// The tools, for changing the gizmo mode.
    pub fn tools_mut(&mut self) -> &mut ToolController {
        &mut self.tools
    }

    /// Tells the viewport whether a menu or popup was open when this frame
    /// began, so an Escape that closes it does not also deselect. The
    /// workbench calls this before drawing the menu bar.
    pub fn note_popup_open(&mut self, open: bool) {
        self.popup_was_open = open;
    }

    /// Tells the viewport whether a message box is showing. While one is,
    /// tool keys (B, W, R, S, T, Escape) leave the map alone.
    pub fn block_keys(&mut self, blocked: bool) {
        self.keys_blocked = blocked;
    }

    /// Draws the viewport filling `ui` and handles its input: the camera,
    /// and the active tool, which may select in or edit `doc`. Returns
    /// messages for the Console.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        renderer: &mut dyn ViewportRenderer,
        doc: &mut Document,
    ) -> Vec<String> {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, VIEWPORT_LABEL)
        });

        let input = self.gather_input(ui, &response, rect);
        let animating = self.controller.update(&input, &DocumentScene::new(doc));
        let notes = self.update_tools(ui, &response, rect, doc);
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
            preview: self.tools.preview(),
            player: self
                .show_player
                .then(|| player_bounds(player_feet(self.tools.preview(), doc, camera.position))),
        };

        let painter = ui.painter_at(rect);
        match renderer.render(&view, doc) {
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

        if let Some(player) = view.player
            && let Some(at) = camera.project(player.center().with_z(player.max.z + 8.0), size)
            && rect.contains(rect.min + vec2(at.x, at.y))
        {
            painter.text(
                rect.min + vec2(at.x, at.y),
                Align2::CENTER_BOTTOM,
                format!(
                    "Player · {}",
                    self.length_unit.format(f64::from(PLAYER_HEIGHT))
                ),
                FontId::proportional(11.0),
                PLAYER_LABEL_COLOR,
            );
        }

        let gizmo_shapes = self.tools.gizmo_shapes(&camera, size, doc);
        gizmo::draw(&painter, rect, &gizmo_shapes);

        painter.text(
            rect.left_top() + vec2(8.0, 6.0),
            Align2::LEFT_TOP,
            "Perspective",
            FontId::proportional(12.0),
            Color32::from_gray(160),
        );

        self.tool_switcher(ui, rect);
        self.mode_switcher(ui, rect);
        notes
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

/// Where the player figure stands: beside the box being drawn, else beside
/// the selection, on its floor and on the side facing the camera; with
/// neither, at the origin.
fn player_feet(preview: Option<Aabb>, doc: &Document, camera: Vec3) -> Vec3 {
    let Some(b) = preview.or_else(|| doc.selection_bounds()) else {
        return Vec3::ZERO;
    };
    let reach = PLAYER_GAP + halberd_tools::PLAYER_WIDTH * 0.5;
    let c = b.center();
    let sides = [
        Vec3::new(b.max.x + reach, c.y, b.min.z),
        Vec3::new(b.min.x - reach, c.y, b.min.z),
        Vec3::new(c.x, b.max.y + reach, b.min.z),
        Vec3::new(c.x, b.min.y - reach, b.min.z),
    ];
    let distance = |p: &Vec3| p.truncate().distance_squared(camera.truncate());
    sides
        .into_iter()
        .min_by(|a, b| distance(a).total_cmp(&distance(b)))
        .unwrap_or(Vec3::ZERO)
}

#[cfg(test)]
mod tests;
