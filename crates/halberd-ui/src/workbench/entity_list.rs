//! The Library's Entities tab: the basic entities a blockout needs (a
//! spawn point, lights, the sun), placed with the Place tool, and the
//! map's sky.

use egui::{RichText, Ui};
use halberd_tools::PlaceChoice;

/// Label of the Library's entities tab.
pub const ENTITIES_TAB: &str = "Entities";
/// Accessible name of the sky picker.
pub const SKY_PICKER_NAME: &str = "Map sky";
/// The sky a map gets when none is chosen (Hammer's default).
pub const DEFAULT_SKY: &str = "sky_day01_01";

/// Skies in Half-Life 2, offered when the game's own list isn't known.
const FALLBACK_SKIES: [&str; 8] = [
    "sky_borealis01",
    "sky_day01_01",
    "sky_day01_04",
    "sky_day01_05",
    "sky_day01_08",
    "sky_day02_01",
    "sky_day03_01",
    "sky_wasteland02",
];

/// One entity in the list.
pub(super) struct BasicEntity {
    pub(super) label: &'static str,
    pub(super) classname: &'static str,
    description: &'static str,
    settings: &'static [(&'static str, &'static str)],
}

impl BasicEntity {
    /// What the Place tool puts down for it.
    pub(super) fn choice(&self) -> PlaceChoice {
        PlaceChoice::entity(self.classname, self.settings)
    }
}

/// The basic entities, in the order shown.
pub(super) const BASIC_ENTITIES: [BasicEntity; 4] = [
    BasicEntity {
        label: "Player spawn",
        classname: "info_player_start",
        description: "Where players appear. Its line shows where they look; turn it with the \
                      gizmo (R) or its Facing in Properties.",
        settings: &[("angles", "0 0 0")],
    },
    BasicEntity {
        label: "Light",
        classname: "light",
        description: "A light shining in every direction. Set its colour and brightness in \
                      Properties.",
        settings: &[("_light", "255 255 255 200"), ("_quadratic_attn", "1")],
    },
    BasicEntity {
        label: "Spotlight",
        classname: "light_spot",
        description: "A cone of light, pointing down at first. Set its cone, pitch and facing \
                      in Properties.",
        settings: &[
            ("_light", "255 255 255 200"),
            ("_quadratic_attn", "1"),
            ("_cone", "45"),
            ("_inner_cone", "30"),
            ("_exponent", "1"),
            ("pitch", "-90"),
            ("angles", "0 0 0"),
        ],
    },
    BasicEntity {
        label: "Sun",
        classname: "light_environment",
        description: "Sunlight and sky light for the whole map, through sky brushes \
                      (tools/toolsskybox). A map has one. Set its direction in Properties.",
        settings: &[
            ("_light", "255 248 230 300"),
            ("_ambient", "150 170 200 60"),
            ("pitch", "-50"),
            ("angles", "0 45 0"),
            ("SunSpreadAngle", "5"),
        ],
    },
];

/// What the user did in the Entities tab.
pub(super) enum EntityListAction {
    /// Picked entity `index` of [`BASIC_ENTITIES`] to place.
    Place(usize),
    /// Chose a sky.
    Sky(String),
}

/// Draws the Entities tab. `skies` are the game's skies (empty: not known);
/// `current` is the map's sky, if set.
pub(super) fn show(
    ui: &mut Ui,
    skies: &[String],
    current: Option<&str>,
) -> Option<EntityListAction> {
    let mut action = None;
    ui.label(RichText::new("Click one, then click in the view to place it.").weak());
    ui.add_space(4.0);
    for (i, entity) in BASIC_ENTITIES.iter().enumerate() {
        let response = ui
            .add_sized(
                [ui.available_width(), 24.0],
                egui::Button::new(format!("{}  ({})", entity.label, entity.classname)),
            )
            .on_hover_text(entity.description);
        if response.clicked() {
            action = Some(EntityListAction::Place(i));
        }
    }
    ui.add_space(10.0);
    ui.separator();
    ui.label(RichText::new("Sky").strong());
    let shown = current.unwrap_or(DEFAULT_SKY).to_string();
    let fallback: Vec<String> = FALLBACK_SKIES.iter().map(|s| s.to_string()).collect();
    let list = if skies.is_empty() { &fallback } else { skies };
    let combo = egui::ComboBox::from_id_salt("halberd_sky_picker")
        .selected_text(shown.clone())
        .width(ui.available_width() - 8.0)
        .show_ui(ui, |ui| {
            for sky in list {
                if ui.selectable_label(*sky == shown, sky).clicked() && *sky != shown {
                    action = Some(EntityListAction::Sky(sky.clone()));
                }
            }
        });
    combo.response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, SKY_PICKER_NAME)
    });
    combo.response.on_hover_text(
        "The picture seen through sky brushes (tools/toolsskybox). Put the Sun in the map too, \
         so the sky lights it.",
    );
    action
}

/// The skies in the game's materials: `SKYBOX/<NAME>UP` gives `name`
/// (each sky has six pictures; the upward one names it), sorted.
pub fn sky_names(material_names: &[String]) -> Vec<String> {
    let mut skies: Vec<String> = material_names
        .iter()
        .filter_map(|m| {
            let rest = m.strip_prefix("SKYBOX/")?;
            let name = rest.strip_suffix("UP")?;
            (!name.is_empty() && !name.contains('/')).then(|| name.to_ascii_lowercase())
        })
        .collect();
    skies.sort();
    skies.dedup();
    skies
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skies_are_named_by_their_upward_picture() {
        let names: Vec<String> = [
            "SKYBOX/SKY_DAY01_01BK",
            "SKYBOX/SKY_DAY01_01UP",
            "SKYBOX/SKY_BOREALIS01UP",
            "SKYBOX/UP",
            "SKYBOX/HDR/SKY_XUP",
            "BRICK/WALLUP",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(sky_names(&names), ["sky_borealis01", "sky_day01_01"]);
    }
}
