//! Editing the settings of the basic entities in Properties: a light's
//! colour and brightness, a spotlight's cone, the sun's direction and a
//! spawn point's facing. Everything else is listed read-only until the
//! entity editor (Phase 2).

use egui::{DragValue, RichText, Ui};
use halberd_doc::{Command, Document, EntityObject, ObjectId};

/// Accessible names of the editable fields (for screen readers and tests).
pub(crate) const BRIGHTNESS_NAME: &str = "Light brightness";
pub(crate) const COLOUR_NAME: &str = "Light colour";
pub(crate) const FACING_NAME: &str = "Facing (degrees)";
pub(crate) const PITCH_NAME: &str = "Pitch (degrees)";
pub(crate) const CONE_NAME: &str = "Cone (degrees)";

/// Draws the editable fields for `entity`'s class, if it is one of the
/// basic entities. Returns true if it drew any.
pub(crate) fn basic_entity_editor(
    ui: &mut Ui,
    doc: &mut Document,
    id: ObjectId,
    entity: &EntityObject,
) -> bool {
    let class = entity.classname.to_ascii_lowercase();
    let light = class == "light" || class == "light_spot" || class == "light_environment";
    let spawn = class.starts_with("info_player_");
    if !light && !spawn {
        return false;
    }
    let mut changes: Vec<(String, String)> = Vec::new();
    egui::Grid::new("halberd_basic_entity")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            if light {
                light_fields(ui, entity, &mut changes);
            }
            if class == "light_spot" {
                number_row(
                    ui,
                    entity,
                    "_cone",
                    CONE_NAME,
                    45.0,
                    1.0..=90.0,
                    &mut changes,
                );
            }
            if class == "light_spot" || class == "light_environment" {
                number_row(
                    ui,
                    entity,
                    "pitch",
                    PITCH_NAME,
                    -45.0,
                    -90.0..=90.0,
                    &mut changes,
                );
            }
            // A plain light shines every way, so it has no facing.
            if class != "light" {
                facing_row(ui, entity, &mut changes);
            }
        });
    if !changes.is_empty() {
        // One undo step per field while it is dragged.
        let key = id.get().wrapping_mul(16).wrapping_add(0x5E7);
        doc.execute_merging(
            Command::SetEntitySettings {
                id,
                settings: changes,
            },
            key,
        )
        .ok();
    }
    ui.add_space(4.0);
    true
}

/// The `_light` setting: "red green blue brightness".
fn light_fields(ui: &mut Ui, entity: &EntityObject, changes: &mut Vec<(String, String)>) {
    let [mut r, mut g, mut b, mut brightness] = parse_light(entity.setting("_light"));
    ui.label("Colour");
    let mut colour = [r as u8, g as u8, b as u8];
    let picked = ui.color_edit_button_srgb(&mut colour);
    picked.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::ColorButton, true, COLOUR_NAME)
    });
    let colour_changed = picked.changed();
    ui.end_row();
    ui.label("Brightness");
    let dragged = ui.add(
        DragValue::new(&mut brightness)
            .range(0.0..=100_000.0)
            .speed(2.0),
    );
    dragged.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::DragValue, true, BRIGHTNESS_NAME)
    });
    let brightness_changed = dragged.changed();
    ui.end_row();
    if colour_changed || brightness_changed {
        [r, g, b] = colour.map(f32::from);
        changes.push((
            "_light".into(),
            format!(
                "{} {} {} {}",
                r as u8,
                g as u8,
                b as u8,
                brightness.round() as i64
            ),
        ));
    }
}

/// A number setting with a slider-like drag field.
fn number_row(
    ui: &mut Ui,
    entity: &EntityObject,
    key: &str,
    name: &'static str,
    default: f32,
    range: std::ops::RangeInclusive<f32>,
    changes: &mut Vec<(String, String)>,
) {
    let mut value = entity
        .setting(key)
        .and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(default);
    ui.label(name.split(" (").next().unwrap_or(name));
    let response = ui.add(
        DragValue::new(&mut value)
            .range(range)
            .speed(1.0)
            .suffix("°"),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::DragValue, true, name));
    if response.changed() {
        changes.push((key.to_string(), format!("{}", value.round() as i64)));
    }
    ui.end_row();
}

/// The turn about the vertical (yaw, the middle of `angles`).
fn facing_row(ui: &mut Ui, entity: &EntityObject, changes: &mut Vec<(String, String)>) {
    let angles = entity.angles();
    let mut yaw = angles.y.rem_euclid(360.0);
    ui.label("Facing");
    let response = ui.add(
        DragValue::new(&mut yaw)
            .range(0.0..=359.0)
            .speed(1.0)
            .suffix("°"),
    );
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::DragValue, true, FACING_NAME));
    if response.changed() {
        changes.push((
            "angles".into(),
            format!(
                "{} {} {}",
                angles.x.round() as i64,
                yaw.round() as i64,
                angles.z.round() as i64
            ),
        ));
    }
    ui.end_row();
    ui.label("");
    ui.label(
        RichText::new("0° faces +X (east), 90° faces +Y (north)")
            .weak()
            .small(),
    );
    ui.end_row();
}

/// "r g b brightness" (white at 200 when missing or unreadable).
fn parse_light(text: Option<&str>) -> [f32; 4] {
    let mut out = [255.0, 255.0, 255.0, 200.0];
    if let Some(text) = text {
        for (slot, part) in out.iter_mut().zip(text.split_whitespace()) {
            if let Ok(v) = part.parse::<f32>()
                && v.is_finite()
            {
                *slot = v;
            }
        }
    }
    for c in &mut out[..3] {
        *c = c.clamp(0.0, 255.0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_values_are_read_with_sensible_fallbacks() {
        assert_eq!(
            parse_light(Some("255 128 0 400")),
            [255.0, 128.0, 0.0, 400.0]
        );
        assert_eq!(parse_light(Some("300 x")), [255.0, 255.0, 255.0, 200.0]);
        assert_eq!(parse_light(None), [255.0, 255.0, 255.0, 200.0]);
    }
}
