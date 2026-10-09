//! The Draw tool's options, in a second row under the tool buttons: which
//! shape to draw, its height, and the settings of shapes that have them.

use super::ViewportPanel;
use egui::{Align, Color32, Layout, Rect, Ui, UiBuilder, vec2};
use halberd_config::LengthUnit;
use halberd_geom::{SIDES_RANGE, SPHERE_SIDES_RANGE, Shape};
use halberd_tools::BOX_HEIGHT_RANGE;

/// Screen-reader name of the Draw tool's height field.
pub(crate) const NEW_BOX_HEIGHT_NAME: &str = "New box height";
/// Screen-reader name of the shape picker.
pub(crate) const SHAPE_PICKER_NAME: &str = "Shape to draw";
/// Screen-reader name of the sides field.
pub(crate) const SIDES_NAME: &str = "Number of sides";
/// Screen-reader name of the arch thickness field.
pub(crate) const ARCH_THICKNESS_NAME: &str = "Arch thickness";
/// Screen-reader name of the stair step field.
pub(crate) const STEP_HEIGHT_NAME: &str = "Step height";

/// A number field in `unit`, storing whole Hammer units.
fn length_field(
    ui: &mut Ui,
    unit: LengthUnit,
    units: f32,
    range: (f32, f32),
    name: &'static str,
    hint: &str,
) -> Option<f32> {
    let mut value = unit.from_units(f64::from(units));
    let field = egui::DragValue::new(&mut value)
        .speed(if unit == LengthUnit::Metres {
            0.01
        } else {
            1.0
        })
        .range(unit.from_units(f64::from(range.0))..=unit.from_units(f64::from(range.1)))
        .fixed_decimals(unit.decimals())
        .suffix(unit.suffix())
        // Typed values are stored rounded to whole units; applying each
        // keystroke would rewrite the text while it is being typed.
        .update_while_editing(false);
    let response = ui.add(field);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::DragValue, true, name));
    response
        .on_hover_text(hint)
        .changed()
        .then(|| (unit.to_units(value) as f32).round())
}

impl ViewportPanel {
    /// The second row of the toolbar, shown while the Draw tool is active.
    pub(super) fn draw_options(&mut self, ui: &mut Ui, rect: Rect) {
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
                self.shape_picker(ui);
                let unit = self.length_unit;
                ui.label("Height");
                if let Some(height) = length_field(
                    ui,
                    unit,
                    self.tools.box_height(),
                    BOX_HEIGHT_RANGE,
                    NEW_BOX_HEIGHT_NAME,
                    "Height of new shapes. Drag sideways or click to type. \
                     A player is 72 units (1.83 m) tall.",
                ) {
                    self.tools.set_box_height(height);
                }
                self.shape_settings(ui, unit);
                if let Some(problem) = self.tools.preview_problem() {
                    ui.separator();
                    ui.label(egui::RichText::new(problem).color(ui.visuals().warn_fg_color));
                }
            });
    }

    /// Opens the shape list, or closes it if open (B in the Draw tool).
    pub(super) fn toggle_shape_menu(&mut self) {
        self.shape_menu = match self.shape_menu {
            Some(_) => None,
            None => Shape::ALL.iter().position(|s| *s == self.tools.shape()),
        };
    }

    fn shape_picker(&mut self, ui: &mut Ui) {
        ui.label("Shape");
        let shape = self.tools.shape();
        let button = ui.button(format!("{} ⏷", shape.label()));
        button.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, SHAPE_PICKER_NAME)
        });
        let clicked = button.clicked();
        let hint = if shape.has_direction() {
            "Wedges and stairs climb, and arches span, the way you drag (R while drawing \
             turns it). B opens this list."
        } else {
            "What the Draw tool makes. B opens this list; arrow keys and Enter pick."
        };
        let below = button.rect.left_bottom();
        let button_rect = button.rect;
        button.on_hover_text(hint);
        if clicked {
            self.toggle_shape_menu();
        }
        self.shape_menu_list(ui, below, button_rect);
    }

    /// The open shape list: click a shape, or move with the arrow keys and
    /// pick with Enter or Space. Escape, or a click elsewhere, closes it.
    fn shape_menu_list(&mut self, ui: &Ui, at: egui::Pos2, button: Rect) {
        let Some(mut highlight) = self.shape_menu else {
            return;
        };
        let count = Shape::ALL.len();
        let (down, up, pick, close) = ui.input_mut(|i| {
            let mut key = |k| i.consume_key(egui::Modifiers::NONE, k);
            (
                key(egui::Key::ArrowDown),
                key(egui::Key::ArrowUp),
                key(egui::Key::Enter) || key(egui::Key::Space),
                key(egui::Key::Escape),
            )
        });
        if down {
            highlight = (highlight + 1) % count;
        }
        if up {
            highlight = (highlight + count - 1) % count;
        }
        let mut chosen = pick.then_some(Shape::ALL[highlight]);
        let area = egui::Area::new(egui::Id::new("halberd_shape_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    for (index, shape) in Shape::ALL.into_iter().enumerate() {
                        if ui
                            .selectable_label(index == highlight, shape.label())
                            .clicked()
                        {
                            chosen = Some(shape);
                        }
                    }
                });
            });
        let clicked_elsewhere = ui.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !area.response.rect.contains(p) && !button.contains(p))
        });
        if let Some(shape) = chosen {
            self.tools.set_shape(shape);
            self.shape_menu = None;
        } else if close || clicked_elsewhere {
            self.shape_menu = None;
        } else {
            self.shape_menu = Some(highlight);
        }
    }

    /// Sides, arch thickness or step height, for the shapes that have them.
    fn shape_settings(&mut self, ui: &mut Ui, unit: LengthUnit) {
        let shape = self.tools.shape();
        if shape.has_sides() {
            ui.label("Sides");
            let (low, high) = if shape == Shape::Sphere {
                SPHERE_SIDES_RANGE
            } else {
                SIDES_RANGE
            };
            let settings = self.tools.shape_settings_mut();
            let mut sides = settings.sides.clamp(low, high);
            let response = ui.add(egui::DragValue::new(&mut sides).range(low..=high));
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::DragValue, true, SIDES_NAME)
            });
            let hint = if shape == Shape::Sphere {
                "Sides around the sphere (an even number; up to 16, Hammer's face limit)."
            } else {
                "How many flat sides it has. More is rounder but costs more."
            };
            if response.on_hover_text(hint).changed() {
                // A sphere is built with an even count; show what is built.
                settings.sides = if shape == Shape::Sphere {
                    ((sides + 1) & !1).min(high)
                } else {
                    sides
                };
            }
        }
        if shape == Shape::Arch {
            ui.label("Thickness");
            let current = self.tools.shape_settings().arch_thickness;
            if let Some(thickness) = length_field(
                ui,
                unit,
                current,
                (1.0, 4096.0),
                ARCH_THICKNESS_NAME,
                "How thick the arch is, from its outside edge inwards.",
            ) {
                self.tools.shape_settings_mut().arch_thickness = thickness;
            }
        }
        if shape == Shape::Stairs {
            ui.label("Step");
            let current = self.tools.shape_settings().step_height;
            if let Some(step) = length_field(
                ui,
                unit,
                current,
                (1.0, 256.0),
                STEP_HEIGHT_NAME,
                "Height of each step. A GMod player walks up steps of up to 18 units \
                 (0.46 m); 8 is comfortable.",
            ) {
                self.tools.shape_settings_mut().step_height = step;
            }
        }
    }
}
