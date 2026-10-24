//! The material browser, in the Library panel: every material in the
//! game, searchable, with a picture of each. The material picked is the
//! one new brushes get, and can be put on the selection.
//!
//! The browser never reads files: the program gives it the names
//! ([`MaterialBrowser::set_names`]), asks for the pictures it wants to show
//! ([`MaterialBrowser::take_wanted`]) and hands them over as they arrive
//! ([`MaterialBrowser::add_picture`]).

use egui::{Color32, ColorImage, RichText, Sense, TextureHandle, TextureOptions, Ui, vec2};
use std::collections::{HashMap, VecDeque};

/// Accessible name of the browser's search box.
pub const MATERIAL_SEARCH_NAME: &str = "Search materials";
/// Label of the button that puts the picked material on the selection.
pub const APPLY_MATERIAL_LABEL: &str = "Apply to selection";
/// Size of the pictures the browser asks for, in pixels.
pub const THUMBNAIL_SIZE: u32 = 128;
/// Size of a picture in the grid, in points.
const TILE: f32 = 72.0;
/// Most pictures kept; the ones shown longest ago are let go first.
const MAX_PICTURES: usize = 600;

/// What the user did in the browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserAction {
    /// Picked a material (new brushes get it).
    Pick(String),
    /// Asked to put a material on the selection (the button, or a
    /// double-click).
    Apply(String),
}

/// A material's picture, or why there is none.
enum Picture {
    /// Pixels waiting for the next frame to become a texture.
    Waiting(ColorImage),
    Shown(TextureHandle),
    None(String),
}

/// The material browser's state.
#[derive(Default)]
pub struct MaterialBrowser {
    names: Vec<String>,
    search: String,
    /// Indices into `names` of what the search matches.
    matching: Vec<usize>,
    picked: Option<String>,
    pictures: HashMap<String, Picture>,
    /// Pictures in the order they were added, to let the oldest go.
    order: VecDeque<String>,
    /// Pictures needed for what is on screen, not yet asked for.
    wanted: Vec<String>,
    /// Pictures asked for and not yet back.
    asked: std::collections::HashSet<String>,
}

impl MaterialBrowser {
    /// The materials to list (`BRICK/BRICKWALL001A`…), sorted.
    pub fn set_names(&mut self, names: Vec<String>) {
        self.names = names;
        self.refilter();
    }

    /// How many materials there are.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// True if there are no materials (the game was not found, or not read
    /// yet).
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// The material picked, if any.
    pub fn picked(&self) -> Option<&str> {
        self.picked.as_deref()
    }

    /// Picks `name` (as when another part of the editor chooses one).
    pub fn pick(&mut self, name: Option<String>) {
        self.picked = name;
    }

    /// The pictures wanted since the last call, to be read and handed over
    /// with [`MaterialBrowser::add_picture`].
    pub fn take_wanted(&mut self) -> Vec<String> {
        let wanted = std::mem::take(&mut self.wanted);
        self.asked.extend(wanted.iter().cloned());
        wanted
    }

    /// Hands over material `name`'s picture (RGBA pixels, `width` ×
    /// `height`), or why there is none.
    pub fn add_picture(&mut self, name: &str, picture: Result<(u32, u32, Vec<u8>), String>) {
        let name = name.to_ascii_uppercase();
        self.asked.remove(&name);
        let picture = match picture {
            Ok((w, h, pixels))
                if pixels.len() == (w as usize) * (h as usize) * 4 && w > 0 && h > 0 =>
            {
                Picture::Waiting(ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    &pixels,
                ))
            }
            Ok(_) => Picture::None("the picture has the wrong size".into()),
            Err(why) => Picture::None(why),
        };
        if self.pictures.insert(name.clone(), picture).is_none() {
            self.order.push_back(name);
        }
        while self.order.len() > MAX_PICTURES {
            if let Some(old) = self.order.pop_front() {
                self.pictures.remove(&old);
            }
        }
    }

    fn refilter(&mut self) {
        let words: Vec<String> = self
            .search
            .split_whitespace()
            .map(str::to_ascii_uppercase)
            .collect();
        self.matching = self
            .names
            .iter()
            .enumerate()
            .filter(|(_, name)| words.iter().all(|w| name.contains(w.as_str())))
            .map(|(i, _)| i)
            .collect();
    }

    /// Draws the browser. Returns what the user did, if anything.
    pub fn show(&mut self, ui: &mut Ui, can_apply: bool) -> Option<BrowserAction> {
        let mut action = None;
        ui.horizontal(|ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search (brick wall, dev…)")
                    .desired_width(f32::INFINITY),
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, MATERIAL_SEARCH_NAME)
            });
            if response.changed() {
                self.refilter();
            }
        });
        ui.horizontal(|ui| {
            match &self.picked {
                Some(name) => ui.label(RichText::new(name).strong()),
                None => ui.label(RichText::new("No material picked").weak()),
            }
            .on_hover_text("New brushes get this material");
        });
        let apply = ui.add_enabled(
            can_apply && self.picked.is_some(),
            egui::Button::new(APPLY_MATERIAL_LABEL),
        );
        if apply
            .on_hover_text(
                "Put the picked material on the selected brushes, or on the picked faces (4)",
            )
            .clicked()
            && let Some(name) = &self.picked
        {
            action = Some(BrowserAction::Apply(name.clone()));
        }
        ui.label(
            RichText::new(format!(
                "{} of {} materials",
                self.matching.len(),
                self.names.len()
            ))
            .weak(),
        );
        ui.separator();
        if let Some(grid_action) = self.grid(ui, can_apply) {
            action = Some(grid_action);
        }
        action
    }

    /// The grid of pictures; only the rows on screen are built.
    fn grid(&mut self, ui: &mut Ui, can_apply: bool) -> Option<BrowserAction> {
        let mut action = None;
        let per_row = ((ui.available_width() / (TILE + 8.0)).floor() as usize).max(1);
        let rows = self.matching.len().div_ceil(per_row);
        let row_height = TILE + 22.0;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, row_height, rows, |ui, range| {
                for row in range {
                    ui.horizontal(|ui| {
                        for slot in 0..per_row {
                            let Some(&index) = self.matching.get(row * per_row + slot) else {
                                break;
                            };
                            let name = self.names[index].clone();
                            if let Some(tile) = self.tile(ui, &name, can_apply) {
                                action = Some(tile);
                            }
                        }
                    });
                }
            });
        action
    }

    /// One material in the grid.
    fn tile(&mut self, ui: &mut Ui, name: &str, can_apply: bool) -> Option<BrowserAction> {
        let picked = self.picked.as_deref() == Some(name);
        let (rect, response) =
            ui.allocate_exact_size(vec2(TILE + 4.0, TILE + 18.0), Sense::click());
        let picture_rect = egui::Rect::from_min_size(rect.min + vec2(2.0, 2.0), vec2(TILE, TILE));
        if picked {
            ui.painter()
                .rect_filled(rect, 3.0, Color32::from_rgb(40, 70, 120));
        }
        if let Some(Picture::Waiting(image)) = self.pictures.get(name) {
            let texture = ui.ctx().load_texture(
                format!("material {name}"),
                image.clone(),
                TextureOptions::LINEAR,
            );
            self.pictures
                .insert(name.to_string(), Picture::Shown(texture));
        }
        match self.pictures.get(name) {
            Some(Picture::Shown(texture)) => {
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                ui.painter()
                    .image(texture.id(), picture_rect, uv, Color32::WHITE);
            }
            Some(Picture::None(_)) => {
                ui.painter()
                    .rect_filled(picture_rect, 2.0, Color32::from_rgb(60, 30, 60));
            }
            _ => {
                ui.painter()
                    .rect_filled(picture_rect, 2.0, Color32::from_gray(35));
                if !self.asked.contains(name) && !self.wanted.iter().any(|w| w == name) {
                    self.wanted.push(name.to_string());
                }
            }
        }
        let short = name.rsplit('/').next().unwrap_or(name);
        ui.painter().text(
            rect.left_bottom() + vec2(2.0, -2.0),
            egui::Align2::LEFT_BOTTOM,
            truncate(short, 12),
            egui::FontId::proportional(10.0),
            Color32::from_gray(190),
        );
        let hover = match self.pictures.get(name) {
            Some(Picture::None(why)) => format!("{name}\n(no picture: {why})"),
            _ => name.to_string(),
        };
        let response = response.on_hover_text(hover);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
        if response.double_clicked() && can_apply {
            self.picked = Some(name.to_string());
            return Some(BrowserAction::Apply(name.to_string()));
        }
        if response.clicked() {
            self.picked = Some(name.to_string());
            return Some(BrowserAction::Pick(name.to_string()));
        }
        None
    }
}

/// `text` cut to at most `max` characters, with "…".
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let cut: String = text.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

#[cfg(test)]
mod tests;
