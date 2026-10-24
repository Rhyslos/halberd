//! Every entity the game knows (from its FGD files), in the Library's
//! Entities tab: search, click a point entity to place it, or a brush
//! entity to tie the selected brushes to it. Also what placing or tying
//! makes: the class with its default keyvalues.

use super::Workbench;
use egui::{RichText, Ui};
use halberd_doc::{Command, EntityObject};
use halberd_fgd::{ClassKind, Fgd, ResolvedClass};
use halberd_geom::Aabb;
use halberd_kv::Entry;
use halberd_tools::PlaceChoice;
use std::sync::Arc;

/// Accessible name of the entity search box.
pub const ENTITY_SEARCH_NAME: &str = "Search entities";
/// Most entities listed at once (the search narrows the rest).
const MAX_LISTED: usize = 400;
/// Height of the list, in points.
const LIST_HEIGHT: f32 = 320.0;

/// What the user picked in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CatalogAction {
    /// Place a point entity of this class.
    Place(String),
    /// Tie the selected brushes to a brush entity of this class.
    Tie(String),
}

/// The list. `search` is the search box's text; `can_tie` says whether
/// brushes are selected.
pub(super) fn show(
    ui: &mut Ui,
    defs: Option<&Fgd>,
    search: &mut String,
    can_tie: bool,
) -> Option<CatalogAction> {
    ui.label(RichText::new("All entities").strong());
    let Some(defs) = defs else {
        ui.label(
            RichText::new(
                "The game's entity list (its FGD files, in Garry's Mod's bin folder) wasn't \
                 found, so only the basic entities above can be placed.",
            )
            .weak(),
        );
        return None;
    };
    let response = ui.add(
        egui::TextEdit::singleline(search)
            .hint_text("Search (light, trigger, npc…)")
            .desired_width(f32::INFINITY),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, ENTITY_SEARCH_NAME)
    });
    let words: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
    let found: Vec<_> = defs
        .placeable()
        .filter(|c| {
            let name = c.name.to_lowercase();
            words.iter().all(|w| name.contains(w.as_str()))
        })
        .collect();
    ui.label(RichText::new(format!("{} of {}", found.len(), defs.placeable().count())).weak());
    let mut action = None;
    egui::ScrollArea::vertical()
        .id_salt("halberd_entity_catalog")
        .max_height(LIST_HEIGHT)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for class in found.iter().take(MAX_LISTED) {
                let solid = class.kind == ClassKind::Solid;
                let text = if solid {
                    format!("{}  (brush)", class.name)
                } else {
                    class.name.clone()
                };
                let enabled = !solid || can_tie;
                let row = ui
                    .add_enabled(enabled, egui::Button::new(text).frame(false))
                    .on_hover_text(first_sentence(&class.description))
                    .on_disabled_hover_text(format!(
                        "{} Select brushes first, then click to tie them to a {}.",
                        first_sentence(&class.description),
                        class.name
                    ));
                if row.clicked() {
                    action = Some(if solid {
                        CatalogAction::Tie(class.name.clone())
                    } else {
                        CatalogAction::Place(class.name.clone())
                    });
                }
            }
            if found.len() > MAX_LISTED {
                ui.label(RichText::new("…search to see the rest.").weak());
            }
        });
    action
}

/// The first sentence of a description, for a tooltip.
fn first_sentence(text: &str) -> String {
    let end = text.find(". ").map_or(text.len(), |at| at + 1);
    text[..end].trim().to_string()
}

/// The keyvalues a new entity of `class` starts with: every key with a
/// default, and the flags on by default.
pub(super) fn default_settings(class: &ResolvedClass) -> Vec<(String, String)> {
    let mut settings = Vec::new();
    for key in &class.keys {
        let name = key.name.to_ascii_lowercase();
        if name == "classname" || name == "origin" {
            continue;
        }
        if key.kind == halberd_fgd::KeyType::Flags {
            let on: u64 = key
                .choices
                .iter()
                .filter(|c| c.on_by_default)
                .filter_map(|c| c.value.trim().parse::<u64>().ok())
                .sum();
            settings.push((key.name.clone(), on.to_string()));
            continue;
        }
        if let Some(default) = key.default.as_ref().filter(|d| !d.is_empty()) {
            settings.push((key.name.clone(), default.clone()));
        }
    }
    settings
}

/// What the Place tool puts down for point class `class`: its defaults,
/// and its size from the definitions when they give one.
pub(super) fn place_choice(class: &ResolvedClass) -> PlaceChoice {
    let settings = default_settings(class);
    let pairs: Vec<(&str, &str)> = settings
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let mut choice = PlaceChoice::entity(&class.class.name, &pairs);
    if let Some((min, max)) = class.size() {
        choice.local = Aabb::from_corners(min.into(), max.into());
    }
    choice
}

impl Workbench {
    /// Takes the game's entity definitions: the Library lists them, the
    /// Properties panel edits by them, and point entities are shown at
    /// their size. `file` is the file read, for the Console.
    pub fn set_entity_definitions(&mut self, defs: halberd_fgd::Fgd, file: &str) {
        self.push_console(format!(
            "Entity definitions: {} entities from {file}.",
            defs.placeable().count()
        ));
        for problem in defs.problems.iter().take(20) {
            self.push_warning(format!("Entity definitions: {problem}"));
        }
        if defs.problems.len() > 20 {
            self.push_warning(format!(
                "Entity definitions: {} more problems not listed.",
                defs.problems.len() - 20
            ));
        }
        self.library.defs = Some(Arc::new(defs));
        self.give_class_boxes();
    }

    /// Tells the map the size of every point entity the definitions size.
    pub(super) fn give_class_boxes(&mut self) {
        let Some(defs) = self.library.defs.clone() else {
            return;
        };
        for class in defs.placeable() {
            if class.kind != ClassKind::Point {
                continue;
            }
            if let Some((min, max)) = defs.resolved(&class.name).and_then(|c| c.size()) {
                self.doc
                    .set_class_box(&class.name, Aabb::from_corners(min.into(), max.into()));
            }
        }
    }

    /// Carries out a pick in the entity list.
    pub(super) fn catalog_action(&mut self, action: CatalogAction) {
        let defs = self.library.defs.clone();
        let resolve = |name: &str| defs.as_ref().and_then(|d| d.resolved(name));
        match action {
            CatalogAction::Place(name) => {
                if let Some(class) = resolve(&name) {
                    self.library.pending_prop = None;
                    self.viewport
                        .tools_mut()
                        .set_placing(Some(place_choice(&class)));
                }
            }
            CatalogAction::Tie(name) => {
                let Some(class) = resolve(&name) else {
                    return;
                };
                self.tie_selection(&class);
            }
        }
    }

    /// Ties the selected brushes to a new brush entity of `class`, and
    /// selects it.
    fn tie_selection(&mut self, class: &ResolvedClass) {
        let selection: Vec<_> = self.doc.selection().iter().copied().collect();
        let mut file_data = vec![Entry::Pair("classname".into(), class.class.name.clone())];
        file_data.extend(
            default_settings(class)
                .into_iter()
                .map(|(k, v)| Entry::Pair(k, v)),
        );
        let entity = EntityObject {
            classname: class.class.name.clone(),
            origin: None,
            file_data,
        };
        match self.doc.execute(Command::TieToEntity {
            objects: selection.clone(),
            entity,
        }) {
            Ok(()) => {
                // Select the new entity, through one of its brushes.
                let owner = selection.iter().find_map(|id| {
                    let object = self.doc.get(*id)?;
                    match object.as_brush() {
                        Some(brush) => brush.entity(),
                        None => self
                            .doc
                            .brushes_of(*id)
                            .next()
                            .and_then(|b| self.doc.get(b)?.as_brush()?.entity()),
                    }
                });
                if let Some(owner) = owner {
                    self.doc.set_selection([owner]);
                }
            }
            Err(_) => self.push_warning(format!(
                "Nothing to tie to a {}: select brushes first.",
                class.class.name
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFS: &str = r#"
        @BaseClass = Targetname [ targetname(target_source) : "Name" ]
        @PointClass base(Targetname) size(-8 -8 0, 8 8 32) = item_box : "A box. It holds things."
        [
            health(integer) : "Health" : 25
            spawnflags(flags) = [ 1 : "Start asleep" : 1  2 : "Glow" : 0  4 : "Big" : 1 ]
        ]
        @SolidClass base(Targetname) = func_wall : "A wall." [ rendermode(choices) : "Mode" : 0 = [ 0 : "Normal" ] ]
    "#;

    #[test]
    fn new_entities_start_with_their_defaults_and_size() {
        let defs = halberd_fgd::parse(DEFS, "t.fgd");
        let class = defs.resolved("item_box").unwrap();
        let settings = default_settings(&class);
        assert_eq!(
            settings,
            [
                ("health".to_string(), "25".to_string()),
                ("spawnflags".to_string(), "5".to_string())
            ]
        );
        let choice = place_choice(&class);
        assert_eq!(choice.classname, "item_box");
        assert_eq!(choice.local.max.z, 32.0);
    }

    #[test]
    fn tooltips_take_the_first_sentence() {
        assert_eq!(first_sentence("A box. It holds things."), "A box.");
        assert_eq!(first_sentence("No full stop"), "No full stop");
    }
}
