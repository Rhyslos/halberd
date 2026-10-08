//! The viewport's left-mouse tools: reading their input, carrying out what
//! they ask, and the tool switcher.

use super::ViewportPanel;
use egui::{Align, Color32, Key, Layout, PointerButton, Rect, Ui, UiBuilder, vec2};
use glam::Vec2;
use halberd_config::LengthUnit;
use halberd_doc::Document;
use halberd_tools::{BOX_HEIGHT_RANGE, GizmoMode, Tool, ToolAction, ToolInput};

/// True if `key` went down this frame, not counting the repeats a held key
/// sends: holding W while flying must not flip the gizmo on and off.
fn first_press(input: &egui::InputState, key: Key) -> bool {
    input.events.iter().any(
        |e| matches!(e, egui::Event::Key { key: k, pressed: true, repeat: false, .. } if *k == key),
    )
}

/// Screen-reader name of the Box tool's height field.
pub(crate) const NEW_BOX_HEIGHT_NAME: &str = "New box height";

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
        let popup_open = egui::Popup::is_any_open(ui.ctx()) || self.popup_was_open;
        let (left_down, additive, escape, box_key) = ui.input(|i| {
            (
                i.pointer.button_down(PointerButton::Primary),
                i.modifiers.command,
                !typing && !popup_open && i.key_pressed(Key::Escape),
                !typing && i.modifiers.is_none() && first_press(i, Key::B),
            )
        });
        if !typing {
            self.gizmo_keys(ui);
        }
        if box_key {
            let next = if self.tools.tool() == Tool::Box {
                Tool::Select
            } else {
                Tool::Box
            };
            self.tools.set_tool(next);
        }
        // A left press while the right or middle button is moving the camera
        // belongs to the camera, not the tool.
        let camera_busy = self.right_was_held || self.middle_was_held;
        let pressed_here = response.is_pointer_button_down_on() && !camera_busy;
        let held = left_down && (pressed_here || self.left_was_held);
        let pressed = held && !self.left_was_held;
        let released = self.left_was_held && !left_down;
        self.left_was_held = held;

        if escape && !self.tools.is_busy() {
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
        let picked = ui.input(|i| {
            let plain = i.modifiers.is_none();
            let shift_only = i.modifiers.shift_only();
            let move_or_scale_ok = if flying { shift_only } else { plain };
            let rotate_or_all_ok = plain || (flying && shift_only);
            [
                (Key::W, GizmoMode::Move, move_or_scale_ok),
                (Key::R, GizmoMode::Rotate, rotate_or_all_ok),
                (Key::S, GizmoMode::Scale, move_or_scale_ok),
                (Key::T, GizmoMode::All, rotate_or_all_ok),
            ]
            .into_iter()
            .find(|(key, _, allowed)| *allowed && first_press(i, *key))
            .map(|(_, mode, _)| mode)
        });
        if let Some(mode) = picked {
            self.tools.toggle_gizmo_mode(mode);
        }
    }

    /// The tool buttons in the viewport's top-left corner.
    pub(super) fn tool_switcher(&mut self, ui: &mut Ui, rect: Rect) {
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
                if current == Tool::Box {
                    ui.separator();
                    ui.label("Height");
                    let unit = self.length_unit;
                    let mut value = unit.from_units(f64::from(self.tools.box_height()));
                    let (low, high) = BOX_HEIGHT_RANGE;
                    let field = egui::DragValue::new(&mut value)
                        .speed(if unit == LengthUnit::Metres {
                            0.01
                        } else {
                            1.0
                        })
                        .range(unit.from_units(f64::from(low))..=unit.from_units(f64::from(high)))
                        .fixed_decimals(unit.decimals())
                        .suffix(unit.suffix());
                    let response = ui.add(field);
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::DragValue,
                            true,
                            NEW_BOX_HEIGHT_NAME,
                        )
                    });
                    if response
                        .on_hover_text(
                            "Height of new boxes. Drag sideways or click to type. \
                             A player is 72 units (1.83 m) tall.",
                        )
                        .changed()
                    {
                        self.tools.set_box_height(unit.to_units(value) as f32);
                    }
                }
            });
    }
}
