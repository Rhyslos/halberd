//! The menu bar (File, Edit, Shape, View, Keybinds, Settings, Help) and
//! the Edit menu's keys (undo, redo, delete).

use super::*;

impl Workbench {
    /// Undo, redo and delete from the keyboard, unless a text box is in use.
    pub(super) fn edit_shortcuts(&mut self, ui: &Ui) {
        // Mid-drag (a box being drawn, a gizmo handle held), undo and delete
        // would pull the map from under the drag; they wait until it ends.
        // While a box ("save changes?", a problem) is showing, the map
        // behind it stays as it is.
        if ui.ctx().egui_wants_keyboard_input()
            || self.viewport.tools().is_busy()
            || self.is_asking_to_save()
            || self.error.is_some()
        {
            return;
        }
        let consume = |shortcut: Option<KeyboardShortcut>| {
            shortcut.is_some_and(|s| ui.input_mut(|i| crate::bindings::consume(i, s)))
        };
        let bindings = self.viewport.bindings().clone();
        let key = |command| bindings.key(command);
        // Ctrl+Shift+Z and Backspace always work too.
        if consume(Some(REDO_ALT_SHORTCUT)) || consume(key(crate::Command::Redo)) {
            self.redo();
        } else if consume(key(crate::Command::Undo)) {
            self.undo();
        }
        if consume(key(crate::Command::Delete)) || consume(Some(DELETE_ALT_SHORTCUT)) {
            self.delete_selection();
        }
    }

    fn edit_menu(&mut self, ui: &mut Ui) {
        let shortcut = |command| self.key_text(ui, command);
        let undo = match self.doc.undo_label() {
            Some(label) => format!("Undo {label}"),
            None => "Undo".to_string(),
        };
        let redo = match self.doc.redo_label() {
            Some(label) => format!("Redo {label}"),
            None => "Redo".to_string(),
        };
        let (undo_keys, redo_keys, delete_keys) = (
            shortcut(crate::Command::Undo),
            shortcut(crate::Command::Redo),
            shortcut(crate::Command::Delete),
        );
        let can_undo = self.doc.undo_label().is_some();
        let can_redo = self.doc.redo_label().is_some();
        let can_delete = if self.delete_waits() {
            self.viewport
                .shape_op_ready(halberd_tools::ShapeOp::DeleteParts, &self.doc)
                .is_ok()
        } else {
            !self.doc.selection().is_empty()
        };
        if ui
            .add_enabled(can_undo, egui::Button::new(undo).shortcut_text(undo_keys))
            .clicked()
        {
            self.undo();
        }
        if ui
            .add_enabled(can_redo, egui::Button::new(redo).shortcut_text(redo_keys))
            .clicked()
        {
            self.redo();
        }
        ui.separator();
        if ui
            .add_enabled(
                can_delete,
                egui::Button::new("Delete").shortcut_text(delete_keys),
            )
            .clicked()
        {
            self.delete_selection();
        }
    }

    pub(super) fn menu_bar(&mut self, ui: &mut Ui, actions: &mut Vec<WorkbenchAction>) {
        ui.menu_button("File", |ui| self.file_menu(ui, actions));
        ui.menu_button("Edit", |ui| self.edit_menu(ui));
        ui.menu_button("Shape", |ui| {
            let notes = crate::viewport::shape_menu(ui, &mut self.viewport, &mut self.doc);
            for note in notes {
                self.push_warning(note);
            }
        });
        ui.menu_button("View", |ui| {
            if ui.button("Reset panel layout").clicked() {
                self.reset_layout();
                self.push_console("Panel layout reset to the standard arrangement.");
            }
            ui.separator();
            ui.label("Viewport views");
            let current = self.viewport.layout();
            for layout in crate::ViewLayout::ALL {
                if ui.radio(current == layout, layout.label()).clicked() {
                    self.viewport.set_layout(layout);
                }
            }
            ui.separator();
            ui.label("Show lengths in");
            for unit in LengthUnit::CHOICES {
                if ui
                    .radio(self.length_unit == unit, unit.label())
                    .on_hover_text("Maps are always saved in Hammer units; 1 unit is 2.54 cm")
                    .clicked()
                {
                    self.set_length_unit(unit);
                }
            }
            ui.separator();
            ui.checkbox(&mut self.show_player, PLAYER_TOGGLE_LABEL)
                .on_hover_text(
                    "A 72-unit (1.83 m) player figure next to what you are working on. It turns \
                     orange where a player could only crouch, red where one wouldn't fit.",
                );
            let mut follows = self.viewport.player_follows();
            if ui
                .add_enabled(
                    self.show_player,
                    egui::Checkbox::new(&mut follows, PLAYER_FOLLOW_LABEL),
                )
                .on_hover_text(
                    "The player figure stands on whatever is under the pointer in the 3D view, \
                     to check doorways, ceilings and gaps",
                )
                .changed()
            {
                self.viewport.set_player_follows(follows);
            }
            self.leak_menu_item(ui);
            ui.separator();
            ui.label("Entity connections");
            let shown = self.viewport.connection_view();
            for view in crate::ConnectionView::ALL {
                if ui
                    .radio(shown == view, view.label())
                    .on_hover_text(
                        "Lines from entities to the entities their outputs reach (green: from \
                         the selection, blue: to it)",
                    )
                    .clicked()
                {
                    self.viewport.set_connection_view(view);
                }
            }
        });
        if ui
            .button(KEYBINDS_TITLE)
            .on_hover_text("Every key and what it does, on a drawn keyboard")
            .clicked()
        {
            self.keybinds_open = !self.keybinds_open;
        }
        if ui
            .button(crate::SETTINGS_TITLE)
            .on_hover_text("Keys, the camera and memory")
            .clicked()
        {
            self.set_settings_open(!self.settings_open());
        }
        ui.menu_button("Help", |ui| {
            if ui.button("About Halberd").clicked() {
                self.about_open = true;
            }
        });
    }
}
