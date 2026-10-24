//! Editing an entity's outputs in Properties: each connection's output,
//! target, input, parameter, delay and "only once", with the class's
//! outputs and the target's inputs offered from the entity definitions
//! (any name can still be typed), warnings for connections that won't
//! work, and what is connected to this entity.

use super::io_index::{IoIndex, io_index};
use super::keyvalues::commit_text;
use egui::{Color32, RichText, Ui};
use halberd_doc::{Command, Connection, Document, EntityObject, ObjectId, is_special_target};
use halberd_fgd::{Fgd, ResolvedClass};

/// The button that adds an output.
pub(crate) const ADD_OUTPUT_LABEL: &str = "Add output";
/// Colour of a warning about a connection.
const WARNING_COLOR: Color32 = Color32::from_rgb(255, 170, 60);
/// Longest delay that can be typed, in seconds.
const MAX_DELAY: f32 = 1_000_000.0;

/// The accessible name of connection `index`'s field `part`.
pub(crate) fn connection_field_name(index: usize, part: &str) -> String {
    format!("Output {} {part}", index + 1)
}

/// What the rows asked for.
#[derive(Default)]
struct RowEdits {
    remove: Option<usize>,
    /// A delay was dragged or typed: one undo step for the whole edit.
    delay: bool,
}

/// Draws entity `id`'s outputs and what reaches it, and carries out
/// changes. Returns messages for the Console.
pub(crate) fn outputs_editor(
    ui: &mut Ui,
    doc: &mut Document,
    id: ObjectId,
    entity: &EntityObject,
    class: Option<&ResolvedClass>,
    defs: Option<&Fgd>,
) -> Vec<String> {
    let index = io_index(ui, doc);
    let old = entity.connections();
    let mut connections = old.clone();
    let mut edits = RowEdits::default();
    ui.add_space(6.0);
    ui.label(RichText::new(format!("Outputs ({})", connections.len())).strong());
    for (i, connection) in connections.iter_mut().enumerate() {
        let target = target_class(doc, &index, defs, &connection.target);
        connection_row(
            ui,
            (id, i),
            connection,
            class,
            target.as_ref(),
            &index,
            &mut edits,
        );
        warnings(ui, &index, connection, target.as_ref());
        ui.separator();
    }
    if let Some(i) = edits.remove {
        connections.remove(i);
    }
    if ui
        .button(ADD_OUTPUT_LABEL)
        .on_hover_text("Connect one of this entity's outputs to another entity's input")
        .clicked()
    {
        let output = class
            .and_then(|c| c.outputs.first())
            .map_or("OnTrigger", |o| o.name.as_str());
        connections.push(Connection::new(output, "", ""));
    }
    incoming(ui, doc, &index, id);
    if connections == old {
        return Vec::new();
    }
    let command = Command::SetConnections { id, connections };
    let done = if edits.delay {
        let key = id.get().wrapping_mul(37).wrapping_add(0xDE1A);
        doc.execute_merging(command, key)
    } else {
        doc.execute(command)
    };
    match done {
        Ok(()) | Err(halberd_doc::DocError::NothingToDo) => Vec::new(),
        Err(e) => vec![format!("The outputs could not be changed: {e}.")],
    }
}

/// The class of the first entity `target` reaches, with its bases.
fn target_class(
    doc: &Document,
    index: &IoIndex,
    defs: Option<&Fgd>,
    target: &str,
) -> Option<ResolvedClass> {
    let id = *index.reached(target).first()?;
    let classname = &doc.get(id)?.as_entity()?.classname;
    defs?.resolved(classname)
}

/// One connection's fields.
fn connection_row(
    ui: &mut Ui,
    (id, i): (ObjectId, usize),
    c: &mut Connection,
    class: Option<&ResolvedClass>,
    target: Option<&ResolvedClass>,
    index: &IoIndex,
    edits: &mut RowEdits,
) {
    let outputs: Vec<&str> = class
        .map(|c| c.outputs.iter().map(|o| o.name.as_str()).collect())
        .unwrap_or_default();
    let inputs: Vec<&str> = target
        .map(|t| t.inputs.iter().map(|i| i.name.as_str()).collect())
        .unwrap_or_default();
    let names: Vec<&str> = index.names.iter().map(String::as_str).collect();
    egui::Grid::new(("halberd_connection", id, i))
        .num_columns(2)
        .spacing([8.0, 2.0])
        .show(ui, |ui| {
            ui.label("Output");
            type_or_pick(
                ui,
                (id, i, "output"),
                &mut c.output,
                &outputs,
                Some("output"),
            );
            ui.end_row();
            ui.label("Target");
            type_or_pick(
                ui,
                (id, i, "target"),
                &mut c.target,
                &names,
                Some("entity name"),
            );
            ui.end_row();
            ui.label("Input");
            type_or_pick(ui, (id, i, "input"), &mut c.input, &inputs, Some("input"));
            ui.end_row();
            ui.label("Parameter");
            if let Some(new) = commit_text(
                ui,
                ("parameter", id, i),
                &c.parameter,
                &connection_field_name(i, "parameter"),
                Some("none"),
            ) {
                c.parameter = new;
            }
            ui.end_row();
            ui.label("Delay");
            delay_row(ui, i, c, edits);
            ui.end_row();
        });
}

/// The delay, "only once" and the remove button.
fn delay_row(ui: &mut Ui, i: usize, c: &mut Connection, edits: &mut RowEdits) {
    ui.horizontal(|ui| {
        let delay = ui.add(
            egui::DragValue::new(&mut c.delay)
                .range(0.0..=MAX_DELAY)
                // A delay already in the map is shown as it is, never
                // changed just by being shown.
                .clamp_existing_to_range(false)
                .speed(0.05)
                .suffix(" s"),
        );
        if delay.changed() {
            edits.delay = true;
        }
        let mut once = c.only_once();
        if ui.checkbox(&mut once, "Only once").changed() {
            c.times = if once { 1 } else { -1 };
        }
        let remove = ui.small_button("×").on_hover_text("Remove this output");
        let name = connection_field_name(i, "remove");
        remove.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
        if remove.clicked() {
            edits.remove = Some(i);
        }
    });
}

/// A text field for any name, with a list of `choices` beside it (the
/// class's outputs, the target's inputs, the map's names).
fn type_or_pick(
    ui: &mut Ui,
    (id, i, part): (ObjectId, usize, &str),
    value: &mut String,
    choices: &[&str],
    hint: Option<&str>,
) {
    ui.horizontal(|ui| {
        let name = connection_field_name(i, part);
        if let Some(new) = commit_text(ui, (part, id, i), value, &name, hint) {
            *value = new.trim().to_string();
        }
        if choices.is_empty() {
            return;
        }
        let menu = ui.menu_button("⏷", |ui| {
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    for choice in choices {
                        if ui.button(*choice).clicked() {
                            *value = (*choice).to_string();
                            ui.close();
                        }
                    }
                });
        });
        let list = format!("{name} list");
        menu.response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &list));
    });
}

/// Warnings about a connection that won't work as written.
fn warnings(ui: &mut Ui, index: &IoIndex, c: &Connection, target: Option<&ResolvedClass>) {
    let target_name = c.target.trim();
    let mut warn = |text: String| {
        ui.label(RichText::new(text).color(WARNING_COLOR).small());
    };
    let named = !index.reached(target_name).is_empty();
    if target_name.is_empty() {
        warn("No target yet.".into());
    } else if !named && !is_special_target(target_name) && !index.has_class(target_name) {
        warn(format!("No entity is named \"{target_name}\"."));
    }
    if let Some(target) = target
        && !c.input.is_empty()
        && !target
            .inputs
            .iter()
            .any(|i| i.name.eq_ignore_ascii_case(&c.input))
    {
        warn(format!(
            "A {} has no input \"{}\" in the game's entity list.",
            target.class.name, c.input
        ));
    }
    if c.parameter.contains(',') {
        warn(
            "A comma in the parameter is saved in a way only newer Source games read; \
             Garry's Mod may misread this output."
                .into(),
        );
    }
}

/// The connections that reach this entity, each a button that selects
/// where it comes from.
fn incoming(ui: &mut Ui, doc: &mut Document, index: &IoIndex, id: ObjectId) {
    let reaching = index.incoming(id);
    if reaching.is_empty() {
        return;
    }
    ui.add_space(6.0);
    ui.label(RichText::new(format!("Connected to this ({})", reaching.len())).strong());
    let mut select = None;
    for (from, c) in reaching {
        let source = doc
            .get(*from)
            .and_then(|o| o.as_entity())
            .map(|e| e.name().unwrap_or(&e.classname).to_string())
            .unwrap_or_default();
        let text = format!("{source}: {} -> {}", c.output, c.input);
        if ui
            .button(text)
            .on_hover_text("Select the entity this comes from")
            .clicked()
        {
            select = Some(*from);
        }
    }
    if let Some(from) = select {
        doc.set_selection([from]);
    }
}
