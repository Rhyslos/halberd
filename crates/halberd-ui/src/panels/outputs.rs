//! Editing an entity's outputs in Properties: each connection's output,
//! target, input, parameter, delay and "only once", with the target's
//! inputs offered from the entity definitions, warnings for targets and
//! inputs that don't exist, and what is connected to this entity.

use super::keyvalues::commit_text;
use egui::{Color32, RichText, Ui};
use halberd_doc::{Command, Connection, Document, EntityObject, ObjectId, is_special_target};
use halberd_fgd::{Fgd, ResolvedClass};

/// The button that adds an output.
pub(crate) const ADD_OUTPUT_LABEL: &str = "Add output";
/// Most entity names offered in a target's list.
const MAX_NAMES: usize = 300;
/// Colour of a warning about a connection.
const WARNING_COLOR: Color32 = Color32::from_rgb(255, 170, 60);

/// The accessible name of connection `index`'s field `part`.
pub(crate) fn connection_field_name(index: usize, part: &str) -> String {
    format!("Output {} {part}", index + 1)
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
    let old = entity.connections();
    let mut connections = old.clone();
    let names = entity_names(doc);
    ui.add_space(6.0);
    ui.label(RichText::new(format!("Outputs ({})", connections.len())).strong());
    let mut remove = None;
    for (i, connection) in connections.iter_mut().enumerate() {
        let target_class = target_class(doc, defs, &connection.target);
        if connection_row(ui, id, i, connection, class, target_class.as_ref(), &names) {
            remove = Some(i);
        }
        warnings(ui, doc, connection, target_class.as_ref());
        ui.separator();
    }
    if let Some(i) = remove {
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
    incoming(ui, doc, id);
    if connections == old {
        return Vec::new();
    }
    match doc.execute(Command::SetConnections { id, connections }) {
        Ok(()) | Err(halberd_doc::DocError::NothingToDo) => Vec::new(),
        Err(e) => vec![format!("The outputs could not be changed: {e}.")],
    }
}

/// Names of the map's entities, sorted, for target lists.
fn entity_names(doc: &Document) -> Vec<String> {
    let mut names: Vec<String> = doc
        .objects()
        .filter_map(|(_, o)| Some(o.as_entity()?.name()?.to_string()))
        .collect();
    names.sort_by_key(|n| n.to_ascii_lowercase());
    names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    names.truncate(MAX_NAMES);
    names
}

/// The class of the first entity `target` reaches, with its bases.
fn target_class(doc: &Document, defs: Option<&Fgd>, target: &str) -> Option<ResolvedClass> {
    let id = doc.targets_of(target).next()?;
    let classname = &doc.get(id)?.as_entity()?.classname;
    defs?.resolved(classname)
}

/// One connection's fields. Returns true if its remove button was clicked.
fn connection_row(
    ui: &mut Ui,
    id: ObjectId,
    i: usize,
    c: &mut Connection,
    class: Option<&ResolvedClass>,
    target: Option<&ResolvedClass>,
    names: &[String],
) -> bool {
    let mut removed = false;
    egui::Grid::new(("halberd_connection", id, i))
        .num_columns(2)
        .spacing([8.0, 2.0])
        .show(ui, |ui| {
            ui.label("Output");
            let outputs: Vec<&str> = class
                .map(|c| c.outputs.iter().map(|o| o.name.as_str()).collect())
                .unwrap_or_default();
            pick_or_type(
                ui,
                (id, i, "output"),
                &mut c.output,
                &outputs,
                &connection_field_name(i, "output"),
            );
            ui.end_row();
            ui.label("Target");
            ui.horizontal(|ui| {
                if let Some(new) = commit_text(
                    ui,
                    ("target", id, i),
                    &c.target,
                    &connection_field_name(i, "target"),
                    Some("entity name"),
                ) {
                    c.target = new.trim().to_string();
                }
                ui.menu_button("⏷", |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(240.0)
                        .show(ui, |ui| {
                            for name in names {
                                if ui.button(name).clicked() {
                                    c.target = name.clone();
                                    ui.close();
                                }
                            }
                        });
                })
                .response
                .on_hover_text("Pick a named entity");
            });
            ui.end_row();
            ui.label("Input");
            let inputs: Vec<&str> = target
                .map(|t| t.inputs.iter().map(|i| i.name.as_str()).collect())
                .unwrap_or_default();
            pick_or_type(
                ui,
                (id, i, "input"),
                &mut c.input,
                &inputs,
                &connection_field_name(i, "input"),
            );
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
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut c.delay)
                        .range(0.0..=3600.0)
                        .speed(0.05)
                        .suffix(" s"),
                );
                let mut once = c.only_once();
                if ui.checkbox(&mut once, "Only once").changed() {
                    c.times = if once { 1 } else { -1 };
                }
                let remove = ui.small_button("×").on_hover_text("Remove this output");
                let name = connection_field_name(i, "remove");
                remove.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name)
                });
                if remove.clicked() {
                    removed = true;
                }
            });
            ui.end_row();
        });
    removed
}

/// A list of `choices` when there are some (with the current value kept
/// even if it isn't one), else a text field.
fn pick_or_type(
    ui: &mut Ui,
    salt: (ObjectId, usize, &str),
    value: &mut String,
    choices: &[&str],
    name: &str,
) {
    if choices.is_empty() {
        if let Some(new) = commit_text(ui, (salt.2, salt.0, salt.1), value, name, None) {
            *value = new.trim().to_string();
        }
        return;
    }
    let combo = egui::ComboBox::from_id_salt(("halberd_pick", salt.0, salt.1, salt.2))
        .selected_text(value.as_str())
        .width(170.0)
        .show_ui(ui, |ui| {
            for choice in choices {
                if ui
                    .selectable_label(value.eq_ignore_ascii_case(choice), *choice)
                    .clicked()
                {
                    *value = (*choice).to_string();
                }
            }
        });
    let name = name.to_string();
    combo
        .response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, &name));
}

/// Warnings about a connection that won't work as written.
fn warnings(ui: &mut Ui, doc: &Document, c: &Connection, target: Option<&ResolvedClass>) {
    let target_name = c.target.trim();
    let mut warn = |text: String| {
        ui.label(RichText::new(text).color(WARNING_COLOR).small());
    };
    if target_name.is_empty() {
        warn("No target yet.".into());
    } else if !is_special_target(target_name) && doc.targets_of(target_name).next().is_none() {
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
            "A {} has no input \"{}\".",
            target.class.name, c.input
        ));
    }
}

/// The connections that reach this entity, each a button that selects
/// where it comes from.
fn incoming(ui: &mut Ui, doc: &mut Document, id: ObjectId) {
    let reaching = doc.incoming(id);
    if reaching.is_empty() {
        return;
    }
    ui.add_space(6.0);
    ui.label(RichText::new(format!("Connected to this ({})", reaching.len())).strong());
    let mut select = None;
    for (from, c) in &reaching {
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
