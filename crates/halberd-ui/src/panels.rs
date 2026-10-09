//! What the simpler panels show: Scene, Properties, Console, placeholders
//! and the About box.

use crate::workbench::AppInfo;
use egui::{RichText, ScrollArea, Ui};
use glam::Vec3;
use halberd_config::LengthUnit;
use halberd_doc::{Command, Document, Object, ObjectId};
use halberd_geom::{Aabb, Brush, MIN_SIZE, Plane};
use std::collections::BTreeSet;

/// One line of the Scene list.
struct SceneRow {
    id: ObjectId,
    name: String,
    /// For a brush entity: its brush count, and whether its list is open.
    group: Option<(usize, bool)>,
    /// A brush listed under its entity.
    nested: bool,
}

/// The Scene panel: every object in the map. Click to select; Ctrl+click
/// to add or remove. A brush entity's brushes are listed under it, folded
/// away until its arrow is clicked (or one of them is selected).
pub(crate) fn scene_contents(ui: &mut Ui, doc: &mut Document) {
    if doc.is_empty() {
        placeholder(
            ui,
            "No brushes yet",
            "Choose the Box tool (B) and drag in the viewport to draw one.",
        );
        return;
    }
    // Which lists were opened or closed by hand, for this map only (ids
    // start again in every map).
    let fold_id = egui::Id::new("halberd_scene_folding");
    let mut folding: SceneFolding = ui
        .data(|d| d.get_temp(fold_id))
        .filter(|f: &SceneFolding| f.map == doc.instance())
        .unwrap_or(SceneFolding {
            map: doc.instance(),
            ..SceneFolding::default()
        });
    let rows = scene_rows(doc, &folding);
    let additive = ui.input(|i| i.modifiers.command);
    let row_height = ui.spacing().interact_size.y;
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(ui, row_height, rows.len(), |ui, range| {
            for row in &rows[range] {
                ui.horizontal(|ui| {
                    let indent = ui.spacing().indent;
                    match row.group {
                        Some((_, expanded)) => {
                            let arrow = if expanded { "⏷" } else { "⏵" };
                            let toggle = ui.add(egui::Button::new(arrow).frame(false));
                            let label = if expanded { "Fold" } else { "Unfold" };
                            toggle.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    true,
                                    format!("{label} {} {}", row.name, row.id),
                                )
                            });
                            if toggle.clicked() {
                                folding.set_open(row.id, !expanded);
                            }
                        }
                        None if row.nested => ui.add_space(indent * 1.5),
                        None => ui.add_space(indent * 0.75),
                    }
                    let text = match row.group {
                        Some((count, _)) => format!(
                            "{} {} ({count} brush{})",
                            row.name,
                            row.id,
                            if count == 1 { "" } else { "es" }
                        ),
                        None => format!("{} {}", row.name, row.id),
                    };
                    if ui.selectable_label(doc.is_selected(row.id), text).clicked() {
                        if additive {
                            doc.toggle_selected(row.id);
                        } else {
                            doc.set_selection([row.id]);
                        }
                    }
                });
            }
        });
    ui.data_mut(|d| d.insert_temp(fold_id, folding));
}

/// Brush entity lists in the Scene panel opened or closed by hand. A list
/// not mentioned is open while one of its brushes is selected.
#[derive(Clone, Default)]
struct SceneFolding {
    /// The map these belong to ([`Document::instance`]).
    map: u64,
    opened: BTreeSet<ObjectId>,
    closed: BTreeSet<ObjectId>,
}

impl SceneFolding {
    fn set_open(&mut self, id: ObjectId, open: bool) {
        if open {
            self.closed.remove(&id);
            self.opened.insert(id);
        } else {
            self.opened.remove(&id);
            self.closed.insert(id);
        }
    }

    fn is_open(&self, id: ObjectId, brush_selected: bool) -> bool {
        self.opened.contains(&id) || (brush_selected && !self.closed.contains(&id))
    }
}

/// The Scene list's lines: objects in map order, with a brush entity's
/// brushes under it while it is open.
fn scene_rows(doc: &Document, folding: &SceneFolding) -> Vec<SceneRow> {
    let mut rows = Vec::new();
    for (id, object) in doc.objects() {
        if object.as_brush().and_then(|b| b.entity()).is_some() {
            continue; // Listed under its entity.
        }
        let name = object_name(object);
        if !doc.has_brushes(id) {
            rows.push(SceneRow {
                id,
                name,
                group: None,
                nested: false,
            });
            continue;
        }
        let brushes: Vec<ObjectId> = doc.brushes_of(id).collect();
        let expanded = folding.is_open(id, brushes.iter().any(|b| doc.is_selected(*b)));
        rows.push(SceneRow {
            id,
            name,
            group: Some((brushes.len(), expanded)),
            nested: false,
        });
        if expanded {
            rows.extend(brushes.into_iter().map(|b| SceneRow {
                id: b,
                name: "Brush".to_string(),
                group: None,
                nested: true,
            }));
        }
    }
    rows
}

/// The Properties panel: the selection's size and position. A single box
/// can be resized and moved by typing or dragging the numbers; several
/// objects show a summary.
pub(crate) fn properties_contents(ui: &mut Ui, doc: &mut Document, unit: LengthUnit) {
    let count = doc.selection().len();
    let single = (count == 1)
        .then(|| doc.selection().iter().next().copied())
        .flatten();
    // An entity is described even when it has no box (no origin, and no
    // brushes Halberd can show).
    if let Some(id) = single
        && let Some(Object::Entity(entity)) = doc.get(id)
    {
        ui.add_space(8.0);
        ui.label(RichText::new("1 object selected").strong());
        entity_details(ui, entity, doc.brushes_of(id).count());
        return;
    }
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
            Some(Object::Brush(brush)) => brush.brush().is_axis_aligned_box(),
            Some(Object::Entity(_)) | None => false,
        });
    if let Some(Object::Brush(brush)) = single.and_then(|id| doc.get(id)) {
        if let Some(owner) = brush.entity() {
            let name = doc.get(owner).map(object_name).unwrap_or_default();
            ui.label(format!("Part of {name} {owner}"));
        }
        ui.label(materials_summary(brush));
    }
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

/// How an object is named in the Scene list: "Brush", or an entity's
/// class, such as "light".
fn object_name(object: &Object) -> String {
    match object {
        Object::Brush(_) => "Brush".to_string(),
        Object::Entity(e) if !e.classname.is_empty() => e.classname.clone(),
        Object::Entity(_) => "Entity".to_string(),
    }
}

/// "Material: X", or how many different materials the faces use.
fn materials_summary(brush: &halberd_doc::BrushObject) -> String {
    let mut names: Vec<&str> = brush.faces().iter().map(|f| f.material.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    match names.as_slice() {
        [one] => format!("Material: {one}"),
        many => format!("Materials: {} different", many.len()),
    }
}

/// An entity's class and settings, read-only for now.
fn entity_details(ui: &mut Ui, entity: &halberd_doc::EntityObject, brushes: usize) {
    ui.label(RichText::new(&entity.classname).strong().size(15.0));
    ui.label(RichText::new("Editing entities comes in Phase 2.").weak());
    ui.add_space(4.0);
    egui::Grid::new("halberd_entity_settings")
        .num_columns(2)
        .striped(true)
        .spacing([12.0, 2.0])
        .show(ui, |ui| {
            for (key, value) in entity.settings() {
                ui.label(RichText::new(key).monospace());
                ui.label(RichText::new(value).monospace());
                ui.end_row();
            }
        });
    if brushes > 0 {
        ui.label(format!(
            "{brushes} brush{}",
            if brushes == 1 { "" } else { "es" }
        ));
        ui.label(
            RichText::new(
                "To pick single brushes, turn on \"Inside entities\" (Ctrl+W) in the \
                 viewport, or open the entity's list in Scene.",
            )
            .weak(),
        );
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
    let Some(old) = doc.get(id).and_then(Object::as_brush) else {
        return;
    };
    let Ok(brush) = resized_box(old.brush(), Aabb::from_corners(corner, corner + size)) else {
        return;
    };
    // One undo step per drag of one field.
    let key = id.get().wrapping_mul(8).wrapping_add(field as u64);
    // Unchanged after rounding, or out of the world: nothing to do.
    doc.execute_merging(Command::ReplaceBrush { id, brush }, key)
        .ok();
}

/// The box `brush` moved and resized to fill `bounds`. Each face is the
/// old face pushed out or pulled in, in the same order, so every face keeps
/// its own material and file data (a fresh cuboid would hand them out in
/// its own face order and mix them up).
fn resized_box(brush: &Brush, bounds: Aabb) -> Result<Brush, halberd_geom::GeomError> {
    let planes: Vec<Plane> = brush
        .faces()
        .iter()
        .map(|face| {
            let normal = face.plane().normal;
            // The corner of the new box furthest along the normal lies on
            // the face; for a box, that is exact.
            let far = Vec3::select(normal.cmpgt(Vec3::ZERO), bounds.max, bounds.min);
            Plane {
                normal,
                distance: normal.dot(far),
            }
        })
        .collect();
    Brush::from_planes(&planes)
}

pub(crate) fn placeholder(ui: &mut Ui, heading: &str, detail: &str) {
    ui.add_space(8.0);
    ui.label(RichText::new(heading).strong());
    ui.label(RichText::new(detail).weak());
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
