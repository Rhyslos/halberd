//! What the simpler panels show: Scene, Properties, Console, placeholders
//! and the About box.

use crate::workbench::AppInfo;
use egui::{RichText, ScrollArea, Ui};
use glam::Vec3;
use halberd_config::LengthUnit;
use halberd_doc::{Command, Document, Object, ObjectId};
use halberd_geom::{Aabb, Brush, MIN_SIZE};

/// The Scene panel: every object in the map. Click to select; Ctrl+click
/// to add or remove.
pub(crate) fn scene_contents(ui: &mut Ui, doc: &mut Document) {
    if doc.is_empty() {
        placeholder(
            ui,
            "No brushes yet",
            "Choose the Box tool (B) and drag in the viewport to draw one.",
        );
        return;
    }
    let rows: Vec<(ObjectId, &'static str)> =
        doc.objects().map(|(id, o)| (id, o.kind_name())).collect();
    let additive = ui.input(|i| i.modifiers.command);
    let row_height = ui.spacing().interact_size.y;
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(ui, row_height, rows.len(), |ui, range| {
            for (id, kind) in rows[range].iter().copied() {
                let selected = doc.is_selected(id);
                if ui
                    .selectable_label(selected, format!("{kind} {id}"))
                    .clicked()
                {
                    if additive {
                        doc.toggle_selected(id);
                    } else {
                        doc.set_selection([id]);
                    }
                }
            }
        });
}

/// The Properties panel: the selection's size and position. A single box
/// can be resized and moved by typing or dragging the numbers; several
/// objects show a summary.
pub(crate) fn properties_contents(ui: &mut Ui, doc: &mut Document, unit: LengthUnit) {
    let count = doc.selection().len();
    let Some(bounds) = doc.selection_bounds() else {
        placeholder(
            ui,
            "Nothing selected",
            "Select something to see its properties.",
        );
        return;
    };
    ui.add_space(8.0);
    let heading = if count == 1 {
        "1 object selected".to_string()
    } else {
        format!("{count} objects selected")
    };
    ui.label(RichText::new(heading).strong());

    let editable_box = (count == 1)
        .then(|| doc.selection().iter().next().copied())
        .flatten()
        .filter(|id| match doc.get(*id) {
            Some(Object::Brush(brush)) => brush.is_axis_aligned_box(),
            None => false,
        });
    match editable_box {
        Some(id) => box_fields(ui, doc, id, bounds, unit),
        None => {
            let size = bounds.size().as_dvec3();
            let (lo, hi) = (bounds.min.as_dvec3(), bounds.max.as_dvec3());
            ui.label(format!(
                "Size: {} × {} × {}",
                unit.format(size.x),
                unit.format(size.y),
                unit.format(size.z)
            ));
            ui.label(format!(
                "From ({}, {}, {}) to ({}, {}, {})",
                unit.format(lo.x),
                unit.format(lo.y),
                unit.format(lo.z),
                unit.format(hi.x),
                unit.format(hi.y),
                unit.format(hi.z)
            ));
        }
    }
}

/// Visible labels of the six number fields.
const BOX_FIELD_LABELS: [&str; 6] = ["Width", "Depth", "Height", "X", "Y", "Z"];
/// Names of the six number fields for screen readers (and tests).
pub(crate) const BOX_FIELD_NAMES: [&str; 6] = [
    "Box width",
    "Box depth",
    "Box height",
    "Box position X",
    "Box position Y",
    "Box position Z",
];

/// Number fields for one box: its size, and the position of its lowest
/// corner. Changing a size keeps that corner where it is. Values are typed
/// in `unit` and stored rounded to whole Hammer units, so brushes stay on
/// the grid Hammer expects.
fn box_fields(ui: &mut Ui, doc: &mut Document, id: ObjectId, bounds: Aabb, unit: LengthUnit) {
    let size = bounds.size();
    let corner = bounds.min;
    let mut values = [size.x, size.y, size.z, corner.x, corner.y, corner.z]
        .map(|v| unit.from_units(f64::from(v)));
    let speed = if unit == LengthUnit::Metres {
        0.01
    } else {
        1.0
    };
    let mut changed = None;
    egui::Grid::new(("halberd_box_fields", id))
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            for (i, label) in BOX_FIELD_LABELS.iter().enumerate() {
                if i == 0 {
                    ui.label(RichText::new("Size").weak());
                    ui.end_row();
                }
                if i == 3 {
                    ui.label(RichText::new("Position (lowest corner)").weak());
                    ui.end_row();
                }
                ui.label(*label);
                let mut field = egui::DragValue::new(&mut values[i])
                    .speed(speed)
                    .fixed_decimals(unit.decimals())
                    .suffix(unit.suffix());
                if i < 3 {
                    field = field.range(unit.from_units(f64::from(MIN_SIZE))..=f64::MAX);
                }
                let response = ui.add(field).on_hover_text(
                    "Drag sideways or click to type. Stored in whole Hammer units (1 unit = 2.54 cm).",
                );
                let name = BOX_FIELD_NAMES[i];
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::DragValue, true, name)
                });
                if response.changed() {
                    changed = Some(i);
                }
                ui.end_row();
            }
        });
    let Some(field) = changed else {
        return;
    };
    let units = values.map(|v| unit.to_units(v).round() as f32);
    let size = Vec3::new(units[0], units[1], units[2]).max(Vec3::splat(MIN_SIZE));
    let corner = Vec3::new(units[3], units[4], units[5]);
    let Ok(brush) = Brush::cuboid(Aabb::from_corners(corner, corner + size)) else {
        return;
    };
    // One undo step per drag of one field.
    let key = id.get().wrapping_mul(8).wrapping_add(field as u64);
    // Unchanged after rounding, or out of the world: nothing to do.
    doc.execute_merging(Command::ReplaceBrush { id, brush }, key)
        .ok();
}

pub(crate) fn placeholder(ui: &mut Ui, heading: &str, detail: &str) {
    ui.add_space(8.0);
    ui.label(RichText::new(heading).strong());
    ui.label(RichText::new(detail).weak());
}

pub(crate) fn console_contents(ui: &mut Ui, lines: &[String]) {
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for line in lines {
                ui.label(RichText::new(line).monospace());
            }
        });
}

pub(crate) fn about_contents(ui: &mut Ui, info: &AppInfo) {
    ui.heading(format!("{} {}", info.name, info.version));
    ui.label("A modern map editor for Garry's Mod.");
    ui.add_space(6.0);
    ui.label("Created by Rhyslos, built with AI assistance (Claude by Anthropic).");
    ui.label("Licensed under the Apache License 2.0.");
    ui.add_space(6.0);
    ui.label(
        RichText::new(
            "Not affiliated with Valve or Facepunch Studios. Garry's Mod is a trademark of \
             Facepunch Studios; Hammer, Source and Steam are trademarks of Valve Corporation.",
        )
        .weak()
        .small(),
    );
}
