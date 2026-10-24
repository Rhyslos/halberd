//! The viewport's left-mouse tools: reading their input, carrying out what
//! they ask, and the tool switcher.

use super::{ViewportPanel, layout};
use crate::Command;
use crate::bindings::pressed;
use egui::{Align, Color32, Key, Layout, Modifiers, PointerButton, Rect, Ui, UiBuilder, vec2};
use glam::Vec2;
use halberd_doc::Document;
use halberd_tools::{GizmoMode, SelectMode, Tool, ToolAction, ToolInput};

/// Start of the Place tool's toolbar row ("Placing oildrum001.mdl").
pub const PLACING_LABEL: &str = "Placing";

/// True if `key` went down this frame with modifiers `held` accepts, not
/// counting the repeats a held key sends (holding W while flying must not
/// flip the gizmo on and off). The modifiers are the ones held when the key
/// went down: Ctrl may already be let go by the frame that reads it (fast
/// typing, scripted input), and Ctrl+W must not count as a plain W.
pub(super) fn first_press(
    input: &egui::InputState,
    key: Key,
    held: impl Fn(Modifiers) -> bool,
) -> bool {
    input.events.iter().any(|e| {
        matches!(
            e,
            egui::Event::Key { key: k, pressed: true, repeat: false, modifiers, .. }
                if *k == key && held(*modifiers)
        )
    })
}

/// The button that picks single brushes inside brush entities (its label
/// also shows the key: Ctrl+W, or Cmd+W on macOS, unless changed).
pub const INSIDE_ENTITIES_LABEL: &str = "Inside entities";

/// The gizmo mode each command picks.
const GIZMO_COMMANDS: [(Command, GizmoMode); 4] = [
    (Command::MoveGizmo, GizmoMode::Move),
    (Command::RotateGizmo, GizmoMode::Rotate),
    (Command::ScaleGizmo, GizmoMode::Scale),
    (Command::AllGizmos, GizmoMode::All),
];
/// The pick mode each command picks.
const SELECT_MODE_COMMANDS: [(Command, SelectMode); 4] = [
    (Command::ObjectMode, SelectMode::Object),
    (Command::VertexMode, SelectMode::Vertex),
    (Command::EdgeMode, SelectMode::Edge),
    (Command::FaceMode, SelectMode::Face),
];

/// "Move W": a button's name and its key, if it has one.
pub(super) fn with_key(ui: &Ui, label: &str, key: Option<egui::KeyboardShortcut>) -> String {
    match key {
        Some(key) => format!("{label} {}", ui.ctx().format_shortcut(&key)),
        None => label.to_string(),
    }
}

impl ViewportPanel {
    /// Reads this frame's left-mouse and tool keys, runs the active tool and
    /// carries out what it asks. Returns messages for the Console.
    pub(super) fn update_tools(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        rect: Rect,
        doc: &mut Document,
    ) -> Vec<String> {
        let typing = ui.ctx().egui_wants_keyboard_input() || self.keys_blocked;
        // Escape that closes a menu or popup is for that menu only. The menu
        // may already have closed itself this frame, so also ask whether one
        // was open when the frame began.
        let popup_open =
            egui::Popup::is_any_open(ui.ctx()) || self.popup_was_open || self.shape_menu.is_some();
        let key = |command| self.bindings.key(command);
        let (draw, inside, select) = (
            key(Command::DrawTool),
            key(Command::InsideEntities),
            key(Command::SelectTool),
        );
        let (left_down, additive, escape, box_key, inside_key, select_key) = ui.input(|i| {
            let hit =
                |k: Option<egui::KeyboardShortcut>| !typing && k.is_some_and(|k| pressed(i, k));
            (
                i.pointer.button_down(PointerButton::Primary),
                i.modifiers.command,
                !typing && !popup_open && !self.text_focus_was && i.key_pressed(Key::Escape),
                hit(draw),
                hit(inside),
                hit(select),
            )
        });
        let (uniform, centered) = ui.input(|i| (i.modifiers.shift, i.modifiers.alt));
        if inside_key {
            let on = !self.tools.inside_entities();
            self.tools.set_inside_entities(on);
        }
        let mut notes = Vec::new();
        let mut plan_action = None;
        if !typing && self.tools.is_drawing_plan() {
            // Drawing a floor plan: Enter finishes it, Backspace takes back
            // the last corner.
            let (finish, back) = ui.input(|i| {
                (
                    first_press(i, Key::Enter, |m| m.is_none()),
                    first_press(i, Key::Backspace, |m| m.is_none()),
                )
            });
            if finish {
                plan_action = self.tools.finish_plan();
            }
            if back {
                self.tools.undo_plan_corner();
            }
        }
        if !typing {
            if self.tools.is_drawing() {
                // While drawing, R turns the shape; the gizmo keys wait.
                // Shift and Alt may be held (they shape the box).
                let turn_key = self.bindings.key(Command::TurnShape);
                let turn = turn_key.is_some_and(|k| {
                    ui.input(|i| {
                        if k.modifiers.is_none() {
                            first_press(i, k.logical_key, |m| !m.command && !m.ctrl)
                        } else {
                            pressed(i, k)
                        }
                    })
                });
                if turn {
                    self.tools.turn_drawing();
                }
            } else {
                self.gizmo_keys(ui);
                self.select_mode_keys(ui);
                if !popup_open && !self.tools.is_busy() {
                    notes.extend(self.shape_op_keys(ui, doc));
                }
            }
        }
        if box_key {
            // B picks Draw; B again opens the shape list.
            if self.tools.tool() == Tool::Box {
                self.toggle_shape_menu();
            } else {
                self.tools.set_tool(Tool::Box);
            }
        }
        if select_key {
            self.shape_menu = None;
            self.tools.set_tool(Tool::Select);
        }
        // A left press while the right or middle button is moving the camera
        // belongs to the camera, not the tool.
        let pane = &self.panes[self.camera_view];
        let camera_busy = pane.right_was_held || pane.middle_was_held;
        let pressed_here = response.is_pointer_button_down_on() && !camera_busy;
        // A click so quick that it went down and up within one frame (a
        // slow frame, or a touchpad tap) counts as held for this frame and
        // let go in the next, so the tool still sees the press.
        let quick_click = !left_down
            && !self.left_was_held
            && !camera_busy
            && response.hovered()
            && ui.input(|i| {
                i.pointer.primary_released()
                    && i.events.iter().any(|e| {
                        matches!(e, egui::Event::PointerButton {
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            pos,
                            ..
                        } if response.rect.contains(*pos))
                    })
            });
        if quick_click {
            ui.ctx().request_repaint();
        }
        let left_down = left_down || quick_click;
        let held = left_down && (pressed_here || quick_click || self.left_was_held);
        let pressed = held && !self.left_was_held;
        let released = self.left_was_held && !left_down;
        self.left_was_held = held;

        // Escape lets go of picked corners, edges or faces first, and
        // deselects the brushes the next time. While placing props, it only
        // stops placing.
        if escape
            && self.tools.tool() != Tool::Place
            && !self.tools.is_busy()
            && !self.tools.clear_elements()
        {
            doc.clear_selection();
        }
        let input = ToolInput {
            size: Vec2::new(rect.width(), rect.height()),
            cursor: response
                .interact_pointer_pos()
                .or_else(|| response.hover_pos())
                .or_else(|| ui.input(|i| i.pointer.latest_pos()))
                .map(|p| Vec2::new(p.x - rect.min.x, p.y - rect.min.y)),
            pressed,
            held,
            released,
            additive,
            cancel: escape,
            uniform,
            centered,
        };
        let camera = self.active_camera();
        let mut input = input;
        // Drawing needs a floor to stand shapes on: the 3D view, or the
        // Top or Bottom view.
        if let layout::PaneView::Side(side) = self.panes[self.active].view
            && !side.is_horizontal()
            && self.tools.tool() == Tool::Box
            && (!self.tools.is_drawing() || self.tools.is_drawing_plan())
            && input.pressed
        {
            notes.push(format!(
                "Shapes can't be drawn in the {} view yet: draw in the 3D view or the Top \
                 view, then move them here.",
                side.label()
            ));
            input.pressed = false;
            input.held = false;
        }
        let action = plan_action.or_else(|| self.tools.update(&input, &camera, doc));
        match action {
            Some(ToolAction::Select(id)) => doc.set_selection(id),
            Some(ToolAction::ToggleSelected(id)) => doc.toggle_selected(id),
            Some(ToolAction::Execute(command)) => {
                if let Err(e) = doc.execute(command) {
                    notes.push(format!("That edit could not be made: {e}."));
                }
            }
            Some(ToolAction::ExecuteMerging(command, key)) => {
                // A drag that cannot go further this frame (for example,
                // back where it started) simply waits for the next one.
                doc.execute_merging(command, key).ok();
            }
            Some(ToolAction::CancelMerging(key)) => {
                doc.discard_step(key);
            }
            Some(ToolAction::Refused(reason)) => notes.push(reason),
            None => {}
        }
        notes
    }

    /// The gizmo keys (W, R, S and T unless changed) pick Move, Rotate,
    /// Scale and All, and pressing the same key again returns to plain
    /// selection. While the right button is held in the 3D view (flying), a
    /// plain key that also flies (W, S) needs Shift, and other plain keys
    /// work with or without it.
    fn gizmo_keys(&mut self, ui: &Ui) {
        let pane = &self.panes[self.camera_view];
        let flying = pane.view == layout::PaneView::Perspective && pane.right_was_held;
        let bindings = &self.bindings;
        let flies = |k: Key| {
            bindings
                .key_down_commands()
                .any(|f| bindings.key(f).is_some_and(|b| b.logical_key == k))
        };
        let picked = ui.input(|i| {
            GIZMO_COMMANDS
                .into_iter()
                .find(|(command, _)| {
                    let Some(key) = bindings.key(*command) else {
                        return false;
                    };
                    if flying && key.modifiers.is_none() {
                        let needs_shift = flies(key.logical_key);
                        first_press(i, key.logical_key, |m| {
                            m.shift_only() || (!needs_shift && m.is_none())
                        })
                    } else {
                        pressed(i, key)
                    }
                })
                .map(|(_, mode)| mode)
        });
        if let Some(mode) = picked {
            self.tools.toggle_gizmo_mode(mode);
        }
    }

    /// The pick mode keys (1 to 4 unless changed): what clicks pick
    /// (objects, corners, edges, faces).
    fn select_mode_keys(&mut self, ui: &Ui) {
        let bindings = &self.bindings;
        let picked = ui.input(|i| {
            SELECT_MODE_COMMANDS
                .into_iter()
                .find(|(command, _)| bindings.key(*command).is_some_and(|k| pressed(i, k)))
                .map(|(_, mode)| mode)
        });
        if let Some(mode) = picked {
            self.shape_menu = None;
            self.tools.set_select_mode(mode);
        }
    }

    /// The tool buttons in the viewport's top-left corner.
    pub(super) fn tool_switcher(&mut self, ui: &mut Ui, rect: Rect) {
        if self.tools.tool() != Tool::Box {
            self.shape_menu = None;
        }
        let area = Rect::from_min_size(
            rect.left_top() + vec2(8.0, 26.0),
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
                ui.label("Tool:");
                let current = self.tools.tool();
                for tool in Tool::ALL {
                    if ui
                        .selectable_label(current == tool, tool.label())
                        .on_hover_text(tool.description())
                        .clicked()
                    {
                        self.tools.set_tool(tool);
                    }
                }
                ui.separator();
                let gizmo = self.tools.gizmo_mode();
                for (command, mode) in GIZMO_COMMANDS {
                    let text = with_key(ui, mode.label(), self.bindings.key(command));
                    if ui
                        .selectable_label(gizmo == Some(mode), text)
                        .on_hover_text(mode.description())
                        .clicked()
                    {
                        self.tools.toggle_gizmo_mode(mode);
                    }
                }
                ui.separator();
                let mut inside = self.tools.inside_entities();
                if ui
                    .toggle_value(
                        &mut inside,
                        with_key(
                            ui,
                            INSIDE_ENTITIES_LABEL,
                            self.bindings.key(Command::InsideEntities),
                        ),
                    )
                    .on_hover_text(
                        "Click picks single brushes inside brush entities such as \
                         func_detail, instead of the whole entity (Hammer's \"Ignore groups\").",
                    )
                    .changed()
                {
                    self.tools.set_inside_entities(inside);
                }
            });
        match self.tools.tool() {
            Tool::Box => self.draw_options(ui, rect),
            Tool::Place => self.place_options(ui, rect),
            Tool::Select => self.pick_options(ui, rect),
        }
    }

    /// The Place tool's second toolbar row: what is being placed.
    fn place_options(&mut self, ui: &mut Ui, rect: Rect) {
        let Some(placing) = self.tools.placing() else {
            return;
        };
        let name = placing.name().to_string();
        let class = placing.classname.clone();
        let area = Rect::from_min_size(
            rect.left_top() + vec2(8.0, 60.0),
            vec2((rect.width() - 16.0).max(0.0), 30.0),
        );
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(area)
                .layout(Layout::left_to_right(Align::Center)),
        );
        let mut stop = false;
        egui::Frame::new()
            .fill(Color32::from_black_alpha(170))
            .corner_radius(4.0)
            .inner_margin(4.0)
            .show(&mut child, |ui| {
                ui.label(format!("{PLACING_LABEL} {name} ({class})"))
                    .on_hover_text(Tool::Place.description());
                ui.label(egui::RichText::new("Click to place · Esc stops").weak());
                stop = ui.button("Stop").clicked();
            });
        if stop {
            self.tools.set_placing(None);
        }
    }

    /// The Select tool's second toolbar row: what clicks pick (1 to 4).
    fn pick_options(&mut self, ui: &mut Ui, rect: Rect) {
        let area = Rect::from_min_size(
            rect.left_top() + vec2(8.0, 60.0),
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
                ui.label("Pick:");
                let current = self.tools.select_mode();
                for (command, mode) in SELECT_MODE_COMMANDS {
                    let text = with_key(ui, mode.label(), self.bindings.key(command));
                    if ui
                        .selectable_label(current == mode, text)
                        .on_hover_text(mode.description())
                        .clicked()
                    {
                        self.tools.set_select_mode(mode);
                    }
                }
            });
    }
}
