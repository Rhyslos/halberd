//! The Keybinds window: a drawn keyboard showing every key Halberd uses.
//!
//! Keys that do something are lit. Hovering a key lists what it does;
//! clicking it keeps that list open below the keyboard. The search box
//! finds actions or keys ("save", "ctrl+s", "drawing") and lights up the
//! keys involved.

use crate::keymap::{KEYBINDS, KeyCap, Keybind, binds_with, mouse_binds};
use egui::{Align2, FontId, Key, Rect, RichText, Sense, Stroke, Ui, Vec2, pos2, vec2};

/// Width of one ordinary key, in points.
const UNIT: f32 = 38.0;
/// Space between keys, in points.
const GAP: f32 = 4.0;
/// Height of the list under the keyboard, in points. Fixed, so the
/// keyboard never moves while searching or clicking keys.
const LIST_HEIGHT: f32 = 170.0;
/// Screen-reader name of the search box (and for tests).
pub const KEYBINDS_SEARCH_NAME: &str = "Search keybinds";
/// The menu bar button and the window's title.
pub const KEYBINDS_TITLE: &str = "Keybinds";

/// One key cap on the drawn keyboard: what it is (if Halberd can tell
/// it apart), its label, and its width in key units.
struct Cap {
    key: Option<KeyCap>,
    label: &'static str,
    width: f32,
}

const fn cap(key: Key, label: &'static str, width: f32) -> Cap {
    Cap {
        key: Some(KeyCap::Key(key)),
        label,
        width,
    }
}

const fn modifier(key: KeyCap, label: &'static str, width: f32) -> Cap {
    Cap {
        key: Some(key),
        label,
        width,
    }
}

/// A key Halberd cannot tell apart (Caps Lock, the Windows key): drawn so
/// the keyboard looks right, never lit.
const fn blank(label: &'static str, width: f32) -> Cap {
    Cap {
        key: None,
        label,
        width,
    }
}

/// A gap of `width` key units.
const fn space(width: f32) -> Cap {
    blank("", -width)
}

/// The main keys of a US layout, row by row, then the keys to their right.
fn layout() -> [(f32, Vec<Cap>); 6] {
    use Key::*;
    let letters = |keys: &[(Key, &'static str)]| -> Vec<Cap> {
        keys.iter().map(|&(k, l)| cap(k, l, 1.0)).collect()
    };
    let mut row1 = vec![cap(Backtick, "`", 1.0)];
    row1.extend(letters(&[
        (Num1, "1"),
        (Num2, "2"),
        (Num3, "3"),
        (Num4, "4"),
        (Num5, "5"),
        (Num6, "6"),
        (Num7, "7"),
        (Num8, "8"),
        (Num9, "9"),
        (Num0, "0"),
        (Minus, "-"),
        (Equals, "="),
    ]));
    row1.extend([
        cap(Backspace, "Backspace", 2.0),
        space(0.5),
        cap(Insert, "Ins", 1.0),
        cap(Home, "Home", 1.0),
        cap(PageUp, "PgUp", 1.0),
    ]);
    let mut row2 = vec![cap(Tab, "Tab", 1.5)];
    row2.extend(letters(&[
        (Q, "Q"),
        (W, "W"),
        (E, "E"),
        (R, "R"),
        (T, "T"),
        (Y, "Y"),
        (U, "U"),
        (I, "I"),
        (O, "O"),
        (P, "P"),
        (OpenBracket, "["),
        (CloseBracket, "]"),
    ]));
    row2.extend([
        cap(Backslash, "\\", 1.5),
        space(0.5),
        cap(Delete, "Del", 1.0),
        cap(End, "End", 1.0),
        cap(PageDown, "PgDn", 1.0),
    ]);
    let mut row3 = vec![blank("Caps", 1.75)];
    row3.extend(letters(&[
        (A, "A"),
        (S, "S"),
        (D, "D"),
        (F, "F"),
        (G, "G"),
        (H, "H"),
        (J, "J"),
        (K, "K"),
        (L, "L"),
        (Semicolon, ";"),
        (Quote, "'"),
    ]));
    row3.push(cap(Enter, "Enter", 2.25));
    let mut row4 = vec![modifier(KeyCap::Shift, "Shift", 2.25)];
    row4.extend(letters(&[
        (Z, "Z"),
        (X, "X"),
        (C, "C"),
        (V, "V"),
        (B, "B"),
        (N, "N"),
        (M, "M"),
        (Comma, ","),
        (Period, "."),
        (Slash, "/"),
    ]));
    row4.extend([
        modifier(KeyCap::Shift, "Shift", 2.75),
        space(1.5),
        cap(ArrowUp, "⏶", 1.0),
    ]);
    let ctrl = KeyCap::Ctrl.name();
    let row5 = vec![
        modifier(KeyCap::Ctrl, ctrl, 1.25),
        blank("Win", 1.25),
        modifier(KeyCap::Alt, "Alt", 1.25),
        cap(Space, "Space", 6.25),
        modifier(KeyCap::Alt, "Alt", 1.25),
        blank("Win", 1.25),
        blank("Menu", 1.25),
        modifier(KeyCap::Ctrl, ctrl, 1.25),
        space(0.5),
        cap(ArrowLeft, "⏴", 1.0),
        cap(ArrowDown, "⏷", 1.0),
        cap(ArrowRight, "⏵", 1.0),
    ];
    let mut row0 = vec![cap(Escape, "Esc", 1.0), space(1.0)];
    for (i, (k, l)) in [
        (F1, "F1"),
        (F2, "F2"),
        (F3, "F3"),
        (F4, "F4"),
        (F5, "F5"),
        (F6, "F6"),
        (F7, "F7"),
        (F8, "F8"),
        (F9, "F9"),
        (F10, "F10"),
        (F11, "F11"),
        (F12, "F12"),
    ]
    .into_iter()
    .enumerate()
    {
        if i > 0 && i % 4 == 0 {
            row0.push(space(0.5));
        }
        row0.push(cap(k, l, 1.0));
    }
    // The extra space under the function keys sets the rows apart.
    [
        (0.0, row0),
        (0.35, row1),
        (0.0, row2),
        (0.0, row3),
        (0.0, row4),
        (0.0, row5),
    ]
}

/// Every key the drawn keyboard can light up.
pub fn drawn_keys() -> Vec<KeyCap> {
    layout()
        .into_iter()
        .flat_map(|(_, row)| row.into_iter().filter_map(|c| c.key))
        .collect()
}

/// The Keybinds window's state.
#[derive(Debug, Default)]
pub struct KeybindsViewer {
    /// The text in the search box.
    pub search: String,
    /// The key clicked last, whose actions are listed.
    pub picked: Option<KeyCap>,
}

impl KeybindsViewer {
    /// The bindings the search finds.
    pub fn found(&self) -> Vec<&'static Keybind> {
        KEYBINDS
            .iter()
            .filter(|b| b.matches(&self.search))
            .collect()
    }

    /// Draws the window's contents.
    pub fn show(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("Search");
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text(format!(
                        "an action or keys, like \"save\" or \"{}+s\"",
                        KeyCap::Ctrl.name().to_lowercase()
                    ))
                    .desired_width(320.0),
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, KEYBINDS_SEARCH_NAME)
            });
        });
        ui.add_space(6.0);
        let searching = !self.search.trim().is_empty();
        let found = self.found();
        let lit: Vec<KeyCap> = found.iter().flat_map(|b| b.keys.iter().copied()).collect();
        let picked = if searching { None } else { self.picked };
        self.keyboard(ui, &lit, picked);
        ui.add_space(6.0);
        let width = ui.available_width();
        ui.allocate_ui(vec2(width, LIST_HEIGHT), |ui| {
            ui.set_min_size(vec2(width, LIST_HEIGHT));
            egui::ScrollArea::vertical()
                .max_height(LIST_HEIGHT)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if searching {
                        if found.is_empty() {
                            ui.label(RichText::new("No keys match.").weak());
                        }
                        bind_list(ui, &found);
                    } else if let Some(picked) = picked {
                        ui.label(RichText::new(picked.name()).strong());
                        let binds: Vec<_> = binds_with(picked).collect();
                        if binds.is_empty() {
                            ui.label(RichText::new("Not used yet.").weak());
                        }
                        bind_list(ui, &binds);
                    } else {
                        ui.label(
                            RichText::new(
                                "Lit keys do something. Hover a key to see what, or click \
                                 it to keep the list here.",
                            )
                            .weak(),
                        );
                    }
                });
        });
        ui.add_space(6.0);
        ui.label(RichText::new("Mouse").strong());
        egui::Grid::new("halberd_mouse_binds")
            .num_columns(2)
            .spacing([12.0, 2.0])
            .show(ui, |ui| {
                for (what, does) in mouse_binds() {
                    ui.label(RichText::new(what).monospace());
                    ui.label(does);
                    ui.end_row();
                }
            });
    }

    /// The drawn keyboard. Keys in `lit` are highlighted (search results).
    fn keyboard(&mut self, ui: &mut Ui, lit: &[KeyCap], picked: Option<KeyCap>) {
        let rows = layout();
        let width = rows
            .iter()
            .map(|(_, row)| row.iter().map(|c| c.width.abs()).sum::<f32>())
            .fold(0.0, f32::max);
        let extra: f32 = rows.iter().map(|(gap, _)| gap).sum();
        let size = vec2(width * UNIT, (rows.len() as f32 + extra) * UNIT);
        let (area, _) = ui.allocate_exact_size(size, Sense::hover());
        let visuals = ui.visuals().clone();
        let mut y = area.top();
        for (row_index, (gap, row)) in rows.into_iter().enumerate() {
            y += gap * UNIT;
            let mut x = area.left();
            for (column, c) in row.into_iter().enumerate() {
                let w = c.width.abs() * UNIT;
                if c.width > 0.0 {
                    let rect =
                        Rect::from_min_size(pos2(x, y), Vec2::new(w, UNIT)).shrink(GAP / 2.0);
                    let id = ui.id().with(("halberd_key", row_index, column));
                    self.key_cap(ui, &visuals, (id, rect), &c, (lit, picked));
                }
                x += w;
            }
            y += UNIT;
        }
    }

    fn key_cap(
        &mut self,
        ui: &mut Ui,
        visuals: &egui::Visuals,
        (id, rect): (egui::Id, Rect),
        c: &Cap,
        (lit, picked): (&[KeyCap], Option<KeyCap>),
    ) {
        let binds: Vec<&Keybind> = c.key.map(|k| binds_with(k).collect()).unwrap_or_default();
        let response = ui.interact(rect, id, Sense::click());
        let name = format!("Key {}", c.label);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
        let highlighted = c.key.is_some_and(|k| lit.contains(&k) || picked == Some(k));
        let (fill, text) = if highlighted {
            (visuals.selection.bg_fill, visuals.selection.stroke.color)
        } else if !binds.is_empty() {
            (
                visuals.widgets.inactive.bg_fill,
                visuals.strong_text_color(),
            )
        } else {
            (visuals.extreme_bg_color, visuals.weak_text_color())
        };
        let stroke = if response.hovered() {
            visuals.widgets.hovered.fg_stroke
        } else {
            Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color)
        };
        let painter = ui.painter();
        painter.rect(rect, 4.0, fill, stroke, egui::StrokeKind::Inside);
        let size = if c.label.chars().count() > 3 {
            11.0
        } else {
            14.0
        };
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            c.label,
            FontId::proportional(size),
            text,
        );
        if response.clicked() {
            self.picked = match (self.picked, c.key) {
                (Some(old), Some(new)) if old == new => None,
                (_, key) => key,
            };
        }
        if !binds.is_empty() {
            response.on_hover_ui(|ui| bind_list(ui, &binds));
        } else if c.key.is_some() {
            response.on_hover_text("Not used yet.");
        } else {
            response.on_hover_text("Halberd can't tell this key apart from others.");
        }
    }
}

/// Bindings as a table: keys, what they do, and when.
fn bind_list(ui: &mut Ui, binds: &[&Keybind]) {
    if binds.is_empty() {
        return;
    }
    egui::Grid::new(ui.id().with("halberd_bind_list"))
        .num_columns(3)
        .striped(true)
        .spacing([12.0, 2.0])
        .show(ui, |ui| {
            for bind in binds {
                ui.label(RichText::new(bind.chord()).monospace().strong());
                ui.label(bind.action);
                ui.label(RichText::new(bind.context.label()).weak());
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests;
