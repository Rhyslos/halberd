//! The viewport's left-mouse tools: reading their input, carrying out what
//! they ask, and the tool switcher.

use super::ViewportPanel;
use egui::{Align, Color32, Key, Layout, Modifiers, PointerButton, Rect, Ui, UiBuilder, vec2};
use glam::Vec2;
use halberd_doc::Document;
use halberd_tools::{GizmoMode, SelectMode, Tool, ToolAction, ToolInput};

/// True if `key` went down this frame with modifiers `held` accepts, not
/// counting the repeats a held key sends (holding W while flying must not
/// flip the gizmo on and off). The modifiers are the ones held when the key
/// went down: Ctrl may already be let go by the frame that reads it (fast
/// typing, scripted input), and Ctrl+W must not count as a plain W.
fn first_press(input: &egui::InputState, key: Key, held: impl Fn(Modifiers) -> bool) -> bool {
    input.events.iter().any(|e| {
        matches!(
            e,
            egui::Event::Key { key: k, pressed: true, repeat: false, modifiers, .. }
                if *k == key && held(*modifiers)
        )
    })
}

/// The button that picks single brushes inside brush entities (its label
/// also shows the key: Ctrl+W, or Cmd+W on macOS).
pub const INSIDE_ENTITIES_LABEL: &str = "Inside entities";
/// The key for [`INSIDE_ENTITIES_LABEL`].
const INSIDE_ENTITIES_SHORTCUT: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::W);

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
        let (left_down, additive, escape, box_key, inside_key, select_key) = ui.input(|i| {
            (
                i.pointer.button_down(PointerButton::Primary),
                i.modifiers.command,
                !typing && !popup_open && !self.text_focus_was && i.key_pressed(Key::Escape),
                !typing && first_press(i, Key::B, |m| m.is_none()),
                !typing && first_press(i, Key::W, |m| m.command_only()),
                !typing && first_press(i, Key::Q, |m| m.is_none()),
            )
        });
        let (uniform, centered) = ui.input(|i| (i.modifiers.shift, i.modifiers.alt));
        if inside_key {
            let on = !self.tools.inside_entities();
            self.tools.set_inside_entities(on);
        }
        if !typing {
            if self.tools.is_drawing() {
                // While drawing, R turns the shape; the gizmo keys wait.
                let turn = ui.input(|i| first_press(i, Key::R, |m| !m.command));
                if turn {
                    self.tools.turn_drawing();
                }
            } else {
                self.gizmo_keys(ui);
                self.select_mode_keys(ui);
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
        let camera_busy = self.right_was_held || self.middle_was_held;
        let pressed_here = response.is_pointer_button_down_on() && !camera_busy;
        let held = left_down && (pressed_here || self.left_was_held);
        let pressed = held && !self.left_was_held;
        let released = self.left_was_held && !left_down;
        self.left_was_held = held;

        // Escape lets go of picked corners, edges or faces first, and
        // deselects the brushes the next time.
        if escape && !self.tools.is_busy() && !self.tools.clear_elements() {
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
        let camera = *self.controller.camera();
        let mut notes = Vec::new();
        match self.tools.update(&input, &camera, doc) {
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

    /// The gizmo keys. W, R, S and T pick Move, Rotate, Scale and All, and
    /// pressing the same key again returns to plain selection. While the
    /// right button is held (flying uses W and S), Shift+W and Shift+S pick
    /// Move and Scale instead; R and T work either way.
    fn gizmo_keys(&mut self, ui: &Ui) {
        let flying = self.right_was_held;
        let move_or_scale_ok = move |m: Modifiers| {
            if flying { m.shift_only() } else { m.is_none() }
        };
        let rotate_or_all_ok = move |m: Modifiers| m.is_none() || (flying && m.shift_only());
        let picked = ui.input(|i| {
            [
                (Key::W, GizmoMode::Move),
                (Key::R, GizmoMode::Rotate),
                (Key::S, GizmoMode::Scale),
                (Key::T, GizmoMode::All),
            ]
            .into_iter()
            .find(|(key, mode)| match mode {
                GizmoMode::Move | GizmoMode::Scale => first_press(i, *key, move_or_scale_ok),
                GizmoMode::Rotate | GizmoMode::All => first_press(i, *key, rotate_or_all_ok),
            })
            .map(|(_, mode)| mode)
        });
        if let Some(mode) = picked {
            self.tools.toggle_gizmo_mode(mode);
        }
    }

    /// The keys 1 to 4: what clicks pick (objects, corners, edges, faces).
    fn select_mode_keys(&mut self, ui: &Ui) {
        let keys = [Key::Num1, Key::Num2, Key::Num3, Key::Num4];
        let picked = ui.input(|i| {
            keys.into_iter()
                .zip(SelectMode::ALL)
                .find(|(key, _)| first_press(i, *key, |m| m.is_none()))
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
                for mode in GizmoMode::ALL {
                    let text = format!("{} {}", mode.label(), mode.key());
                    if ui
                        .selectable_label(gizmo == Some(mode), text)
                        .on_hover_text(mode.description())
                        .clicked()
                    {
                        self.tools.toggle_gizmo_mode(mode);
                    }
                }
                ui.separator();
                ui.label("Pick:");
                let current = self.tools.select_mode();
                for mode in SelectMode::ALL {
                    let text = format!("{} {}", mode.label(), mode.key());
                    if ui
                        .selectable_label(current == mode, text)
                        .on_hover_text(mode.description())
                        .clicked()
                    {
                        self.shape_menu = None;
                        self.tools.set_select_mode(mode);
                    }
                }
                ui.separator();
                let mut inside = self.tools.inside_entities();
                if ui
                    .toggle_value(
                        &mut inside,
                        format!(
                            "{INSIDE_ENTITIES_LABEL} {}",
                            ui.ctx().format_shortcut(&INSIDE_ENTITIES_SHORTCUT)
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
        if self.tools.tool() == Tool::Box {
            self.draw_options(ui, rect);
        }
    }
}
