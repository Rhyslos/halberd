//! How the mouse and keyboard reach the views: which view gets this
//! frame's input, what is read from it, and the settings that change it
//! (key bindings, WASD, camera speeds).

use super::{PaneView, ViewportPanel};
use crate::{Bindings, Command};
use egui::{PointerButton, Rect, Ui};
use glam::Vec2;
use halberd_tools::{FlyKeys, ViewportInput};

impl ViewportPanel {
    /// The keys in effect.
    pub fn bindings(&self) -> &Bindings {
        &self.bindings
    }

    /// Uses `bindings` from now on.
    pub fn set_bindings(&mut self, bindings: Bindings) {
        self.bindings = bindings;
    }

    /// How fast dragging turns the 3D camera and the wheel zooms (1.0 is
    /// normal; kept between 0.1 and 4).
    pub fn set_camera_feel(&mut self, turn: f32, zoom: f32) {
        let usable = |v: f32| {
            if v.is_finite() {
                v.clamp(0.1, 4.0)
            } else {
                1.0
            }
        };
        self.turn_factor = usable(turn);
        self.zoom_factor = usable(zoom);
    }

    /// Turns WASD flying (and so the Fly camera) on or off in every view.
    pub fn set_wasd_enabled(&mut self, enabled: bool) {
        for pane in &mut self.panes {
            pane.perspective.set_wasd_enabled(enabled);
        }
    }

    /// Sets every 3D view's flying speed, in units per second.
    pub fn set_fly_speed(&mut self, speed: f32) {
        for pane in &mut self.panes {
            pane.perspective.set_fly_speed(speed);
        }
    }

    /// The flying keys held, by their bindings.
    fn fly_keys(&self, i: &egui::InputState) -> FlyKeys {
        let down = |command| {
            self.bindings
                .key(command)
                .is_some_and(|k| i.key_down(k.logical_key))
        };
        FlyKeys {
            forward: down(Command::FlyForward),
            back: down(Command::FlyBack),
            left: down(Command::FlyLeft),
            right: down(Command::FlyRight),
            up: down(Command::FlyUp),
            down: down(Command::FlyDown),
            fast: i.modifiers.shift,
        }
    }

    /// Picks the views that get this frame's input. The camera's: the one
    /// under the pointer, unless the right or middle button is held in the
    /// current one. The tools': the one under the pointer, unless the left
    /// button is held or a tool is busy there; a floor plan being drawn
    /// moves to any view it can be drawn in, so corners can be placed in
    /// one view and the next in another.
    pub(super) fn choose_active(&mut self, responses: &[egui::Response]) {
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

    /// Reads this frame's mouse and keyboard input for the viewport.
    pub(super) fn gather_input(
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
            let keys = self.fly_keys(i);
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
            let delta = ui.input(|i| i.smooth_scroll_delta.y) * self.zoom_factor;
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
        // Turning speed is for the 3D camera; a side view's drag moves it
        // with the pointer.
        let turn = if pane.view == PaneView::Perspective {
            self.turn_factor
        } else {
            1.0
        };

        ViewportInput {
            size: Vec2::new(rect.width(), rect.height()),
            cursor,
            right_pressed,
            right_held,
            right_delta: if right_held && !right_pressed {
                delta * turn
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
}
