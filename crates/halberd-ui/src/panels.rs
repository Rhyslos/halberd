//! What the simpler panels show: Scene, Properties, Console, placeholders
//! and the About box.

use crate::workbench::AppInfo;
use egui::{RichText, ScrollArea, Ui};
use halberd_doc::{Document, ObjectId};

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

/// The Properties panel: a summary of the selection. Editing values
/// arrives with the full entity and brush properties.
pub(crate) fn properties_contents(ui: &mut Ui, doc: &Document) {
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
    let size = bounds.size();
    let (lo, hi) = (bounds.min, bounds.max);
    ui.label(format!("Size: {} × {} × {} units", size.x, size.y, size.z));
    ui.label(format!(
        "From ({}, {}, {}) to ({}, {}, {})",
        lo.x, lo.y, lo.z, hi.x, hi.y, hi.z
    ));
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
