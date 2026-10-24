//! The viewport panel: up to six views (the 3D perspective and the six flat
//! side views) in a preset layout. Turns mouse and keyboard input into
//! camera moves and tool actions, shows the rendered images, and draws the
//! tool, camera mode, view and layout switchers.
//!
//! The view under the pointer gets the input; while a button is held or a
//! tool is busy (a drag, a floor plan being drawn) it keeps it, even if the
//! pointer wanders into another view.
//!
//! Rendering itself happens elsewhere, behind the [`ViewportRenderer`]
//! trait, so this panel (and its tests) need no GPU.

use egui::{Align, Color32, Key, Layout, PointerButton, Rect, Sense, Stroke, Ui, UiBuilder, vec2};
use glam::{Mat4, Vec2, Vec3};
use halberd_config::LengthUnit;
use halberd_doc::Document;
use halberd_geom::Aabb;
use halberd_tools::{
    CameraController, CameraMode, CameraState, DocumentScene, FlyKeys, Tool, ToolController,
    ViewportInput,
};

mod draw_options;
mod elements;
mod gizmo;
mod layout;
mod panes;
mod plan;
mod player;
mod readout;
mod shape_ops;
mod tools;

pub use layout::{MAX_VIEWS, PaneView, RenderMode, ViewLayout, ViewSetup};
pub use panes::LAYOUT_PICKER_LABEL;
pub use shape_ops::SHAPE_OP_KEYS;
pub(crate) use shape_ops::shape_menu;
pub use tools::{INSIDE_ENTITIES_LABEL, PLACING_LABEL};

/// What a renderer needs to draw one viewport frame.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewportView {
    /// Size of the image to draw, in physical pixels.
    pub size_px: [u32; 2],
    /// World-to-screen transform.
    pub view_projection: Mat4,
    /// Where the camera is.
    pub camera_position: Vec3,
    /// The editor grid size, in units.
    pub grid_size: f32,
    /// The box a shape is being drawn in.
    pub preview: Option<Aabb>,
    /// The shape being drawn in it, to outline (empty: outline the box).
    pub preview_shape: Vec<halberd_geom::Brush>,
    /// Where to draw the player figure for scale, if shown.
    pub player: Option<Aabb>,
    /// Whether a player fits where the figure stands (to colour it).
    pub player_fit: Option<halberd_tools::Fit>,
    /// Which view this is (0 to [`MAX_VIEWS`] - 1), so a renderer can keep
    /// an image for each.
    pub id: usize,
    /// The axis the grid is flat across (0 X, 1 Y, 2 Z).
    pub grid_axis: usize,
    /// The grid's position along that axis.
    pub grid_offset: f32,
    /// Whether the grid fades into the distance (the 3D view only).
    pub grid_fade: bool,
    /// How brushes are drawn.
    pub render_mode: RenderMode,
}

/// Draws viewport images. The editor implements this with the GPU; tests
/// and machines without a usable GPU use [`NoRenderer`].
pub trait ViewportRenderer {
    /// Draws a frame of `doc` and returns the egui texture showing it, or a
    /// plain-language reason why it could not.
    fn render(&mut self, view: &ViewportView, doc: &Document) -> Result<egui::TextureId, String>;

    /// Only views `0..shown` are shown now: images kept for the others can
    /// be let go. Called once a frame, after the views are drawn.
    fn release_unused(&mut self, shown: usize) {
        let _ = shown;
    }
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

/// Accessible name of the first view, and the label tests look for. The
/// other views are named "View 2" to "View 6".
pub const VIEWPORT_LABEL: &str = "3D viewport";
/// Pivot marker colour.
const PIVOT_COLOR: Color32 = Color32::from_rgb(255, 196, 64);
/// Colour of the player figure's label (matches the figure).
const PLAYER_LABEL_COLOR: Color32 = Color32::from_rgb(120, 190, 255);
/// Label colour when a player only fits crouching.
const PLAYER_CROUCH_LABEL_COLOR: Color32 = Color32::from_rgb(255, 170, 60);
/// Label colour when a player doesn't fit.
const PLAYER_BLOCKED_LABEL_COLOR: Color32 = Color32::from_rgb(255, 90, 80);
/// Gap between the player figure and what it stands next to, in units.
const PLAYER_GAP: f32 = 32.0;

/// The viewport panel's state.
pub struct ViewportPanel {
    /// Every view's state, shown or not (a layout with fewer views keeps
    /// the rest as they were).
    panes: Vec<layout::Pane>,
    layout: ViewLayout,
    /// The view the tools work in (left mouse, tool keys).
    active: usize,
    /// The view whose camera gets the right and middle buttons, the wheel
    /// and the flying keys: the one under the pointer, kept while one of
    /// those buttons is held.
    camera_view: usize,
    /// Where the player figure stands this frame, the same in every view.
    player_at: Option<Aabb>,
    /// Where the figure stands under the pointer, and whether it fits.
    probe: player::PlayerProbe,
    tools: ToolController,
    grid_size: f32,
    left_was_held: bool,
    /// A menu or popup was open when this frame began.
    popup_was_open: bool,
    /// A box ("save changes?", a problem) is showing: tool keys wait.
    keys_blocked: bool,
    /// The shape list is open (B in the Draw tool), with this entry
    /// highlighted.
    shape_menu: Option<usize>,
    /// A text box had the keyboard when the last frame ended.
    text_focus_was: bool,
    length_unit: LengthUnit,
    show_player: bool,
}

impl ViewportPanel {
    /// A viewport starting from a saved camera (or the default one).
    pub fn new(saved: Option<CameraState>, options: ViewportOptions) -> Self {
        let first = CameraController::new(saved.unwrap_or_default(), options.wasd_enabled);
        let panes = (0..MAX_VIEWS)
            .map(|i| {
                let controller = if i == 0 {
                    first.clone()
                } else {
                    CameraController::new(CameraState::default(), options.wasd_enabled)
                };
                layout::Pane::new(i, controller)
            })
            .collect();
        Self {
            panes,
            layout: ViewLayout::Single,
            active: 0,
            camera_view: 0,
            player_at: None,
            probe: player::PlayerProbe::default(),
            tools: ToolController::new(options.grid_size),
            grid_size: options.grid_size,
            left_was_held: false,
            popup_was_open: false,
            keys_blocked: false,
            shape_menu: None,
            text_focus_was: false,
            length_unit: LengthUnit::Units,
            show_player: true,
        }
    }

    /// The camera of the view that gets the input.
    pub fn active_camera(&self) -> halberd_tools::Camera {
        self.panes[self.active].camera()
    }

    /// The view that gets the input (0 to [`MAX_VIEWS`] - 1).
    pub fn active_view(&self) -> usize {
        self.active
    }

    /// The first view's 3D camera state, to save between sessions.
    pub fn camera_state(&self) -> CameraState {
        self.panes[0].perspective.state()
    }

    /// The first view's 3D camera controller, for reading the camera and
    /// mode.
    pub fn controller(&self) -> &CameraController {
        &self.panes[0].perspective
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

    /// The tools, for reading the Draw tool's settings.
    pub fn tools(&self) -> &ToolController {
        &self.tools
    }

    /// Points the camera at the whole of `bounds` (a newly opened map).
    pub fn frame(&mut self, bounds: Aabb) {
        for pane in &mut self.panes {
            pane.perspective.frame(bounds);
            pane.side.frame(bounds);
        }
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

    /// Tells the viewport whether a text box had the keyboard at the end of
    /// the frame. egui lets go of a text box at the very start of the frame
    /// in which Escape is pressed, so without this, the Escape that leaves a
    /// text box would also deselect. The workbench calls this every frame.
    pub fn note_text_focus(&mut self, focused: bool) {
        self.text_focus_was = focused;
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
        let (full, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let rects = self.layout.rects(full);
        let responses: Vec<egui::Response> = rects
            .iter()
            .enumerate()
            .map(|(i, rect)| {
                let response = ui.interact(
                    *rect,
                    ui.id().with(("halberd_view", i)),
                    Sense::click_and_drag(),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Other, true, view_label(i))
                });
                response
            })
            .collect();
        self.choose_active(&responses);

        let mut animating = false;
        for (i, (rect, response)) in rects.iter().zip(&responses).enumerate() {
            let input = self.gather_input(ui, response, *rect, i);
            let pane = &mut self.panes[i];
            animating |= match pane.view {
                PaneView::Perspective => pane.perspective.update(&input, &DocumentScene::new(doc)),
                PaneView::Side(_) => {
                    pane.side.update(&input);
                    false
                }
            };
        }
        let active = self.active;
        let notes = self.update_tools(ui, &responses[active], rects[active], doc);
        let pointer = responses[active].hover_pos().map(|p| {
            let rect = rects[active];
            (
                Vec2::new(p.x - rect.min.x, p.y - rect.min.y),
                Vec2::new(rect.width().max(1.0), rect.height().max(1.0)),
            )
        });
        self.place_player(doc, pointer);
        // Pointer movement already triggers redraws; only flying with a key
        // held needs frames without new input.
        if animating {
            ui.ctx().request_repaint();
        }
        for (i, rect) in rects.iter().enumerate() {
            self.draw_pane(ui, renderer, doc, i, *rect);
        }
        renderer.release_unused(rects.len());
        if rects.len() > 1 {
            let painter = ui.painter_at(full);
            painter.rect_stroke(
                rects[active].shrink(0.5),
                0.0,
                Stroke::new(1.0, ACTIVE_VIEW_COLOR),
                egui::StrokeKind::Inside,
            );
        }
        self.tool_switcher(ui, rects[0]);
        self.layout_picker(ui, full);
        notes
    }

    /// Picks the views that get this frame's input. The camera's: the one
    /// under the pointer, unless the right or middle button is held in the
    /// current one. The tools': the one under the pointer, unless the left
    /// button is held or a tool is busy there; a floor plan being drawn
    /// moves to any view it can be drawn in, so corners can be placed in
    /// one view and the next in another.
    fn choose_active(&mut self, responses: &[egui::Response]) {
        let count = responses.len();
        if self.active >= count {
            self.active = 0;
        }
        if self.camera_view >= count {
            self.camera_view = 0;
        }
        let hovered = responses.iter().position(egui::Response::hovered);
        let camera = &self.panes[self.camera_view];
        if !(camera.right_was_held || camera.middle_was_held)
            && let Some(i) = hovered
        {
            self.camera_view = i;
        }
        let plan = self.tools.is_drawing_plan();
        let sticky = self.left_was_held || (self.tools.is_busy() && !plan);
        if let Some(i) = hovered
            && !sticky
            && (!plan || self.panes[i].can_draw())
        {
            self.active = i;
        }
    }

    /// The camera the player figure is placed for: the tools' view if it is
    /// a 3D view, else the first 3D view shown, else the first view's 3D
    /// camera. (A side view's camera stands far away, which would put the
    /// figure in front of the selection.)
    fn reference_camera(&self) -> halberd_tools::Camera {
        let shown = self.layout.count();
        let pane = std::iter::once(self.active)
            .chain(0..shown)
            .map(|i| &self.panes[i])
            .find(|p| p.view == PaneView::Perspective)
            .unwrap_or(&self.panes[0]);
        *pane.perspective.camera()
    }

    /// Reads this frame's mouse and keyboard input for the viewport.
    fn gather_input(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        rect: Rect,
        index: usize,
    ) -> ViewportInput {
        if index != self.camera_view {
            let pane = &mut self.panes[index];
            pane.right_was_held = false;
            pane.middle_was_held = false;
            return ViewportInput {
                size: Vec2::new(rect.width(), rect.height()),
                dt: ui.input(|i| i.stable_dt),
                ..ViewportInput::default()
            };
        }
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
        let pane = &mut self.panes[index];
        let right_held = right_down && (pressed_here || pane.right_was_held);
        let middle_held = middle_down && (pressed_here || pane.middle_was_held);
        let right_pressed = right_held && !pane.right_was_held;
        let middle_pressed = middle_held && !pane.middle_was_held;
        pane.right_was_held = right_held;
        pane.middle_was_held = middle_held;
        // Flying is for the 3D view only.
        let keys = if pane.view == PaneView::Perspective {
            keys
        } else {
            FlyKeys::default()
        };

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
    fn mode_switcher(&mut self, ui: &mut Ui, rect: Rect, index: usize) {
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
                let controller = &mut self.panes[index].perspective;
                let current = controller.mode();
                for mode in controller.available_modes() {
                    if ui
                        .selectable_label(current == mode, mode.label())
                        .on_hover_text(mode.description())
                        .clicked()
                    {
                        controller.set_mode(mode);
                    }
                }
                if current == CameraMode::Fly {
                    ui.label(format!("speed {:.0}", controller.fly_speed()))
                        .on_hover_text("Scroll while flying to change speed");
                }
            });
    }
}

/// The accessible name of view `index`.
fn view_label(index: usize) -> String {
    if index == 0 {
        VIEWPORT_LABEL.to_string()
    } else {
        format!("View {}", index + 1)
    }
}

/// Outline of the view that gets the input, when there are several.
const ACTIVE_VIEW_COLOR: Color32 = Color32::from_rgb(90, 120, 170);

#[cfg(test)]
mod tests;
