//! Editing any entity's keyvalues in Properties. With the game's entity
//! definitions (FGD), every key the class has is listed with its label,
//! a tooltip, and an editor for its type (a list for choices, ticks for
//! flags, a colour button); keys the definitions don't know are listed as
//! text. Keys can be added and removed, and the class changed.
//!
//! Text is put into the map when Enter is pressed or the field is left,
//! as in Hammer, so typing a name is one undo step; Escape puts it back.

use egui::{Color32, Key, RichText, Ui};
use halberd_doc::{Command, Document, EntityObject, ObjectId};
use halberd_fgd::{Fgd, KeyDef, KeyType, ResolvedClass};

/// Accessible name of the class field.
pub(crate) const CLASS_FIELD_NAME: &str = "Entity class";
/// Accessible name of the new key's name field.
pub(crate) const NEW_KEY_NAME: &str = "New key name";
/// Accessible name of the new key's value field.
pub(crate) const NEW_VALUE_NAME: &str = "New key value";
/// The button that adds the new key.
pub(crate) const ADD_KEY_LABEL: &str = "Add key";
/// The button that moves a brush entity's brushes back to the world.
pub(crate) const MOVE_TO_WORLD_LABEL: &str = "Move brushes to world";
/// Colour of a value that is the default (not written yet).
const DEFAULT_COLOR: Color32 = Color32::from_gray(130);

/// What the editor asks the document to do.
#[derive(Default)]
struct Edits {
    set: Vec<(String, String)>,
    /// Changes from a drag (colour, brightness): one undo step per drag.
    dragged: Vec<(String, String)>,
    remove: Vec<String>,
    class: Option<String>,
    to_world: bool,
    /// Why something asked for couldn't be done.
    notes: Vec<String>,
}

/// Keys with their own fields (class, origin) or that aren't settings
/// (Hammer's `id` numbers objects in the file): never edited as keys.
const FIXED_KEYS: [&str; 3] = ["classname", "origin", "id"];

fn is_fixed(key: &str) -> bool {
    FIXED_KEYS
        .iter()
        .any(|f| key.trim().eq_ignore_ascii_case(f))
}

/// The accessible name of key `key`'s value field.
pub(crate) fn value_field_name(key: &str) -> String {
    format!("Value of {key}")
}

/// Draws the class, every keyvalue, inputs and outputs of entity `id`,
/// and carries out what was changed. Returns messages for the Console.
pub(crate) fn keyvalue_editor(
    ui: &mut Ui,
    doc: &mut Document,
    id: ObjectId,
    entity: &EntityObject,
    defs: Option<&Fgd>,
) -> Vec<String> {
    let class = defs.and_then(|d| d.resolved(&entity.classname));
    let brushes = doc.brushes_of(id).count();
    let mut edits = Edits::default();
    class_row(ui, id, entity, class.as_ref(), defs.is_some(), &mut edits);
    egui::Grid::new(("halberd_keyvalues", id))
        .num_columns(3)
        .striped(true)
        .spacing([8.0, 3.0])
        .min_col_width(20.0)
        .show(ui, |ui| {
            if let Some(class) = &class {
                for def in &class.keys {
                    known_row(ui, id, entity, def, &mut edits);
                }
            }
            for (key, value) in entity.settings() {
                let fixed = is_fixed(key);
                let known = class.as_ref().is_some_and(|c| c.key(key).is_some());
                if !fixed && !known {
                    plain_row(ui, id, key, value, &mut edits);
                }
            }
        });
    add_row(ui, id, &mut edits);
    check_class_kind(doc, id, defs, &mut edits);
    if brushes > 0 {
        ui.label(format!(
            "{brushes} brush{}",
            if brushes == 1 { "" } else { "es" }
        ));
        if ui
            .button(MOVE_TO_WORLD_LABEL)
            .on_hover_text("Turn this entity's brushes back into ordinary world brushes")
            .clicked()
        {
            edits.to_world = true;
        }
    }
    let mut notes = super::outputs::outputs_editor(ui, doc, id, entity, class.as_ref(), defs);
    if let Some(class) = &class {
        inputs_list(ui, class);
    }
    notes.extend(carry_out(doc, id, edits));
    notes
}

/// Refuses a class change between point and brush entities (a point
/// entity has no brushes to be a door with; a brush entity's brushes
/// would be lost to a light).
fn check_class_kind(doc: &Document, id: ObjectId, defs: Option<&Fgd>, edits: &mut Edits) {
    let (Some(new), Some(defs)) = (&edits.class, defs) else {
        return;
    };
    let Some(class) = defs.class(new) else {
        return;
    };
    let has_brushes = doc.has_brushes(id);
    let fits = match class.kind {
        halberd_fgd::ClassKind::Solid => has_brushes,
        halberd_fgd::ClassKind::Point => !has_brushes,
        halberd_fgd::ClassKind::Base => false,
    };
    if !fits {
        let why = if has_brushes {
            "it is a point entity, and this one is made of brushes"
        } else {
            "it is made of brushes, and this one is a point entity"
        };
        edits
            .notes
            .push(format!("The class can't be changed to {new}: {why}."));
        edits.class = None;
    }
}

fn carry_out(doc: &mut Document, id: ObjectId, edits: Edits) -> Vec<String> {
    let mut notes = edits.notes;
    if !edits.dragged.is_empty() {
        // One undo step for the whole drag of each field.
        let key = id.get().wrapping_mul(31).wrapping_add(0x6B7);
        let settings = edits.dragged;
        if let Err(e) = doc.execute_merging(Command::SetEntitySettings { id, settings }, key)
            && e != halberd_doc::DocError::NothingToDo
        {
            notes.push(format!("The setting could not be changed: {e}."));
        }
    }
    let brushes: Vec<ObjectId> = doc.brushes_of(id).collect();
    let mut commands = Vec::new();
    if let Some(classname) = edits.class {
        commands.push(Command::SetEntityClass { id, classname });
    }
    if !edits.set.is_empty() {
        commands.push(Command::SetEntitySettings {
            id,
            settings: edits.set,
        });
    }
    if !edits.remove.is_empty() {
        commands.push(Command::RemoveEntitySettings {
            id,
            keys: edits.remove,
        });
    }
    if edits.to_world {
        commands.push(Command::MoveToWorld(vec![id]));
    }
    for command in commands {
        let what = command.describe();
        let to_world = matches!(command, Command::MoveToWorld(_));
        match doc.execute(command) {
            // The brushes stay selected, as in Hammer.
            Ok(()) if to_world => doc.set_selection(brushes.iter().copied()),
            Ok(()) | Err(halberd_doc::DocError::NothingToDo) => {}
            Err(e) => notes.push(format!("{what} could not be done: {e}.")),
        }
    }
    notes
}

fn class_row(
    ui: &mut Ui,
    id: ObjectId,
    entity: &EntityObject,
    class: Option<&ResolvedClass>,
    has_defs: bool,
    edits: &mut Edits,
) {
    ui.horizontal(|ui| {
        ui.label("Class");
        if let Some(new) = commit_text(
            ui,
            ("class", "", id),
            &entity.classname,
            CLASS_FIELD_NAME,
            None,
        ) {
            edits.class = Some(new.trim().to_string());
        }
    });
    match (class, has_defs) {
        (Some(class), _) if !class.class.description.is_empty() => {
            ui.label(RichText::new(&class.class.description).weak().small());
        }
        (None, true) => {
            ui.label(
                RichText::new("The game's entity list doesn't have this class.")
                    .weak()
                    .small(),
            );
        }
        _ => {}
    }
    ui.add_space(4.0);
}

/// A key the definitions describe.
fn known_row(ui: &mut Ui, id: ObjectId, entity: &EntityObject, def: &KeyDef, edits: &mut Edits) {
    let set = entity.setting(&def.name);
    let label = ui.label(&def.label);
    let tip = if def.description.is_empty() {
        def.name.clone()
    } else {
        format!("{} ({})", def.description, def.name)
    };
    label.on_hover_text(tip);
    let value = set.or(def.default.as_deref()).unwrap_or("");
    let fixed = is_fixed(&def.name);
    let changed = if def.read_only || fixed {
        ui.label(RichText::new(value).monospace());
        None
    } else {
        match def.kind {
            KeyType::Choices if !def.choices.is_empty() => choices(ui, id, def, value),
            KeyType::Flags if !def.choices.is_empty() => flags(ui, def, value),
            KeyType::Color255 => {
                if let Some(new) = color(ui, id, &def.name, value) {
                    edits.dragged.push((def.name.clone(), new));
                }
                None
            }
            _ => commit_text(
                ui,
                ("kv", def.name.as_str(), id),
                set.unwrap_or(""),
                &value_field_name(&def.name),
                def.default.as_deref(),
            ),
        }
    };
    if let Some(new) = changed {
        edits.set.push((def.name.clone(), new));
    }
    remove_button(ui, &def.name, set.is_some() && !fixed, edits);
    ui.end_row();
}

/// A key the definitions don't describe: plain text.
fn plain_row(ui: &mut Ui, id: ObjectId, key: &str, value: &str, edits: &mut Edits) {
    ui.label(RichText::new(key).monospace());
    if let Some(new) = commit_text(ui, ("kv", key, id), value, &value_field_name(key), None) {
        edits.set.push((key.to_string(), new));
    }
    remove_button(ui, key, true, edits);
    ui.end_row();
}

fn remove_button(ui: &mut Ui, key: &str, present: bool, edits: &mut Edits) {
    let button = ui
        .add_enabled(present, egui::Button::new("×").small())
        .on_hover_text(format!("Remove {key} (it goes back to its default)"));
    if button.clicked() {
        edits.remove.push(key.to_string());
    }
}

/// A text field whose text goes into the map on Enter or when left, and
/// back to `current` on Escape. Returns the new text when it changed.
pub(super) fn commit_text(
    ui: &mut Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    current: &str,
    name: &str,
    default: Option<&str>,
) -> Option<String> {
    let id = ui.id().with(("halberd_kv_text", salt));
    let mut text = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| current.to_string());
    let mut edit = egui::TextEdit::singleline(&mut text).desired_width(150.0);
    if let Some(default) = default {
        edit = edit.hint_text(RichText::new(default).color(DEFAULT_COLOR));
    }
    let response = ui.add(edit);
    let accessible = name.to_string();
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, &accessible));
    if response.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text));
        return None;
    }
    ui.data_mut(|d| d.remove::<String>(id));
    let escaped = ui.input(|i| i.key_pressed(Key::Escape));
    (response.lost_focus() && !escaped && text != current).then_some(text)
}

fn choices(ui: &mut Ui, id: ObjectId, def: &KeyDef, value: &str) -> Option<String> {
    let shown = def
        .choices
        .iter()
        .find(|c| c.value == value)
        .map_or_else(|| value.to_string(), |c| c.label.clone());
    let mut picked = None;
    let combo = egui::ComboBox::from_id_salt(("halberd_kv_choice", id, def.name.as_str()))
        .selected_text(shown)
        .width(150.0)
        .show_ui(ui, |ui| {
            for choice in &def.choices {
                if ui
                    .selectable_label(choice.value == value, &choice.label)
                    .clicked()
                    && choice.value != value
                {
                    picked = Some(choice.value.clone());
                }
            }
        });
    let name = value_field_name(&def.name);
    combo
        .response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, &name));
    picked
}

fn flags(ui: &mut Ui, def: &KeyDef, value: &str) -> Option<String> {
    let current: u64 = value.trim().parse().unwrap_or(0);
    let mut bits = current;
    ui.vertical(|ui| {
        for flag in &def.choices {
            let Ok(bit) = flag.value.trim().parse::<u64>() else {
                continue;
            };
            let mut on = bits & bit != 0;
            if ui.checkbox(&mut on, &flag.label).changed() {
                bits = if on { bits | bit } else { bits & !bit };
            }
        }
    });
    (bits != current).then(|| bits.to_string())
}

/// A colour button, and a brightness field when the value has one
/// ("255 255 255 200"). A value that isn't a plain colour (such as HDR's
/// "-1 -1 -1 1", "same as the normal colour") is edited as text, so it
/// isn't turned into black.
fn color(ui: &mut Ui, id: ObjectId, key: &str, value: &str) -> Option<String> {
    let parts: Option<Vec<f32>> = value
        .split_whitespace()
        .map(|p| p.parse::<f32>().ok().filter(|v| v.is_finite()))
        .collect();
    let plain = parts
        .as_ref()
        .filter(|p| (3..=4).contains(&p.len()) && p[..3].iter().all(|v| (0.0..=255.0).contains(v)));
    let Some(parts) = plain else {
        return commit_text(ui, ("kv", key, id), value, &value_field_name(key), None);
    };
    let mut rgb = [parts[0] as u8, parts[1] as u8, parts[2] as u8];
    let mut brightness = parts.get(3).copied();
    let mut changed = false;
    ui.horizontal(|ui| {
        changed |= ui.color_edit_button_srgb(&mut rgb).changed();
        if let Some(b) = &mut brightness {
            changed |= ui
                .add(egui::DragValue::new(b).range(0.0..=100_000.0).speed(2.0))
                .on_hover_text("Brightness")
                .changed();
        }
    });
    changed.then(|| match brightness {
        Some(b) => format!("{} {} {} {}", rgb[0], rgb[1], rgb[2], trim_number(b)),
        None => format!("{} {} {}", rgb[0], rgb[1], rgb[2]),
    })
}

/// A number as Hammer writes it: whole numbers plainly, else up to two
/// decimals.
fn trim_number(v: f32) -> String {
    if (v - v.round()).abs() < 0.005 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}").trim_end_matches('0').to_string()
    }
}

fn add_row(ui: &mut Ui, id: ObjectId, edits: &mut Edits) {
    let key_id = ui.id().with(("halberd_new_key", id));
    let value_id = ui.id().with(("halberd_new_value", id));
    let mut key: String = ui.data_mut(|d| d.get_temp(key_id)).unwrap_or_default();
    let mut value: String = ui.data_mut(|d| d.get_temp(value_id)).unwrap_or_default();
    ui.horizontal(|ui| {
        let k = ui.add(
            egui::TextEdit::singleline(&mut key)
                .hint_text("key")
                .desired_width(90.0),
        );
        k.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, NEW_KEY_NAME));
        let v = ui.add(
            egui::TextEdit::singleline(&mut value)
                .hint_text("value")
                .desired_width(110.0),
        );
        v.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, NEW_VALUE_NAME)
        });
        let ready = !key.trim().is_empty();
        let entered = v.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        let clicked = ui
            .add_enabled(ready, egui::Button::new(ADD_KEY_LABEL))
            .clicked();
        if ready && (clicked || entered) {
            if is_fixed(&key) {
                edits.notes.push(format!(
                    "\"{}\" can't be added as a key: the class and position have their own \
                     fields, and \"id\" is the editor's own number.",
                    key.trim()
                ));
            } else {
                edits.set.push((key.trim().to_string(), value.clone()));
            }
            key.clear();
            value.clear();
        }
    });
    ui.data_mut(|d| {
        d.insert_temp(key_id, key);
        d.insert_temp(value_id, value);
    });
}

/// The inputs the class takes, for reference.
fn inputs_list(ui: &mut Ui, class: &ResolvedClass) {
    if class.inputs.is_empty() {
        return;
    }
    egui::CollapsingHeader::new(format!("Inputs it takes ({})", class.inputs.len()))
        .id_salt("halberd_inputs_taken")
        .show(ui, |ui| {
            for input in &class.inputs {
                let text = if input.param == "void" {
                    input.name.clone()
                } else {
                    format!("{} ({})", input.name, input.param)
                };
                ui.label(RichText::new(text).monospace())
                    .on_hover_text(&input.description);
            }
        });
}
