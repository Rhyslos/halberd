//! The Library panel: the material and prop browsers once the game's
//! content is read, and what the workbench does when something is picked.

use super::Workbench;
use super::entity_list::{self, BASIC_ENTITIES, ENTITIES_TAB, EntityListAction};
use crate::materials::{AssetBrowser, BrowserAction};
use crate::panels::placeholder;
use egui::Ui;
use halberd_geom::Aabb;
use halberd_tools::PlaceChoice;
use std::collections::HashMap;

/// Label of the Library's materials tab.
pub const MATERIALS_TAB: &str = "Materials";
/// Label of the Library's props tab.
pub const PROPS_TAB: &str = "Props";

/// How far mounting the game's content has got, for the Library panel.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ContentStatus {
    /// Garry's Mod was not found, so there is no content to mount.
    #[default]
    NoGame,
    /// Its content is being mounted.
    Mounting,
    /// Mounted, with a one-line summary.
    Mounted(String),
}

/// The Library panel's state.
pub(super) struct Library {
    pub(super) materials: AssetBrowser,
    pub(super) props: AssetBrowser,
    /// The tab shown.
    tab: Tab,
    /// The game's skies (empty until known).
    skies: Vec<String>,
    /// What is known of each model read (lower-case name), for placing.
    prop_infos: HashMap<String, PlaceChoice>,
    /// A prop picked before its model was read; placed once it is.
    pending_prop: Option<String>,
    pub(super) status: ContentStatus,
}

impl Default for Library {
    fn default() -> Self {
        Self {
            materials: AssetBrowser::materials(),
            props: AssetBrowser::props(),
            tab: Tab::Materials,
            skies: Vec::new(),
            prop_infos: HashMap::new(),
            pending_prop: None,
            status: ContentStatus::NoGame,
        }
    }
}

/// The Library's tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Materials,
    Props,
    Entities,
}

/// What the user did in the Library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LibraryAction {
    Material(BrowserAction),
    Prop(BrowserAction),
    /// Picked basic entity `index` to place.
    Place(usize),
    /// Chose a sky.
    Sky(String),
}

impl Library {
    /// Tells `doc` the box of every model read so far.
    pub(super) fn give_model_boxes(&self, doc: &mut halberd_doc::Document) {
        for (model, prop) in &self.prop_infos {
            doc.set_model_box(model, prop.local);
        }
    }

    /// Draws the panel. What the user did is put in `action`, for the
    /// workbench to carry out once the panels are drawn. `sky` is the
    /// map's sky, if set.
    pub(super) fn show(
        &mut self,
        ui: &mut Ui,
        can_apply: bool,
        sky: Option<&str>,
        action: &mut Option<LibraryAction>,
    ) {
        ui.horizontal(|ui| {
            for (tab, label) in [
                (Tab::Materials, MATERIALS_TAB),
                (Tab::Props, PROPS_TAB),
                (Tab::Entities, ENTITIES_TAB),
            ] {
                if ui.selectable_label(self.tab == tab, label).clicked() {
                    self.tab = tab;
                }
            }
        });
        ui.separator();
        let browser = match self.tab {
            Tab::Entities => {
                *action = match entity_list::show(ui, &self.skies, sky) {
                    Some(EntityListAction::Place(i)) => Some(LibraryAction::Place(i)),
                    Some(EntityListAction::Sky(name)) => Some(LibraryAction::Sky(name)),
                    None => action.take(),
                };
                return;
            }
            Tab::Materials => &mut self.materials,
            Tab::Props => &mut self.props,
        };
        if browser.is_empty() {
            let content = match &self.status {
                ContentStatus::NoGame => "Garry's Mod was not found, so its materials and models \
                     can't be shown. Start Halberd with --gmod-dir to point at it."
                    .to_string(),
                ContentStatus::Mounting => "Reading Garry's Mod's content…".to_string(),
                ContentStatus::Mounted(summary) => summary.clone(),
            };
            placeholder(
                ui,
                "Materials and props",
                &format!(
                    "{content}\n\nMaterials and props appear here once the game's content is \
                     read. Spawn points, lights and the sky are in Entities."
                ),
            );
            return;
        }
        let kind = self.tab;
        if let Some(done) = browser.show(ui, can_apply) {
            *action = Some(match kind {
                Tab::Props => LibraryAction::Prop(done),
                _ => LibraryAction::Material(done),
            });
        }
    }
}

impl Workbench {
    /// Keeps at most `most` pictures in each of the material and prop
    /// browsers (from the memory budget).
    pub fn set_picture_limit(&mut self, most: usize) {
        self.library.materials.set_picture_limit(most);
        self.library.props.set_picture_limit(most);
    }

    /// The material browser, for handing it names and pictures.
    pub fn materials_mut(&mut self) -> &mut AssetBrowser {
        &mut self.library.materials
    }

    /// The prop browser, for handing it model names and pictures.
    pub fn props_mut(&mut self) -> &mut AssetBrowser {
        &mut self.library.props
    }

    /// Tells the Library what model `model` is like once it has been read
    /// (its box in its own space, and whether it was made static), so it
    /// can be placed. A prop picked while it was being read is chosen
    /// for the Place tool now.
    pub fn add_model_info(&mut self, model: &str, local: Aabb, static_prop: bool) {
        let prop = PlaceChoice::prop(model, local, static_prop);
        let key = model.to_ascii_lowercase();
        self.doc.set_model_box(model, local);
        if self.library.pending_prop.as_deref() == Some(key.as_str()) {
            self.library.pending_prop = None;
            self.viewport.tools_mut().set_placing(Some(prop.clone()));
        }
        self.library.prop_infos.insert(key, prop);
    }

    /// A model could not be read. If the user picked it to place, says
    /// why in the Console instead of leaving the click unanswered.
    pub fn prop_failed(&mut self, model: &str, why: &str) {
        let key = model.to_ascii_lowercase();
        if self.library.pending_prop.as_deref() == Some(key.as_str()) {
            self.library.pending_prop = None;
            self.push_warning(format!("{model} can't be placed: {why}."));
        }
    }

    /// Tells the Library panel how far mounting the game's content has got.
    pub fn set_content_status(&mut self, status: ContentStatus) {
        self.library.status = status;
    }

    /// What the Library panel says about the game's content.
    pub fn content_status(&self) -> &ContentStatus {
        &self.library.status
    }

    /// The game's skies, for the sky picker.
    pub fn set_sky_names(&mut self, skies: Vec<String>) {
        self.library.skies = skies;
    }

    /// Carries out what the user did in the Library.
    pub(super) fn library_action(&mut self, action: LibraryAction) {
        match action {
            LibraryAction::Place(index) => {
                let Some(entity) = BASIC_ENTITIES.get(index) else {
                    return;
                };
                // A map has one sun: picking it again selects the one there.
                let sun = (entity.classname == "light_environment")
                    .then(|| {
                        self.doc.objects().find_map(|(id, o)| {
                            o.as_entity()
                                .is_some_and(|e| {
                                    e.classname.eq_ignore_ascii_case("light_environment")
                                })
                                .then_some(id)
                        })
                    })
                    .flatten();
                if let Some(sun) = sun {
                    self.doc.set_selection([sun]);
                    self.push_warning(
                        "The map already has a sun (light_environment); it is selected. A map \
                         has only one.",
                    );
                    return;
                }
                self.library.pending_prop = None;
                self.viewport.tools_mut().set_placing(Some(entity.choice()));
            }
            LibraryAction::Sky(name) => {
                self.doc
                    .execute(halberd_doc::Command::SetWorldSetting {
                        key: "skyname".into(),
                        value: name,
                    })
                    .ok();
            }
            LibraryAction::Material(action) => self.browser_action(action),
            LibraryAction::Prop(BrowserAction::Pick(name) | BrowserAction::Apply(name)) => {
                let key = name.to_ascii_lowercase();
                match self.library.prop_infos.get(&key) {
                    Some(prop) => {
                        let prop = prop.clone();
                        self.library.pending_prop = None;
                        self.viewport.tools_mut().set_placing(Some(prop));
                    }
                    // Its model is still being read (its picture is on the
                    // way); it is chosen when it arrives.
                    None => self.library.pending_prop = Some(key),
                }
            }
        }
    }

    /// Carries out what the user did in the material browser: a picked
    /// material is the one new brushes get; applying also puts it on the
    /// selection (or the picked faces).
    pub(super) fn browser_action(&mut self, action: BrowserAction) {
        let (BrowserAction::Pick(name) | BrowserAction::Apply(name)) = &action;
        self.viewport.tools_mut().set_material(Some(name.clone()));
        if let BrowserAction::Apply(name) = action {
            match self.viewport.tools().apply_material(&self.doc, &name) {
                Ok(command) => {
                    if let Err(e) = self.doc.execute(command)
                        && e != halberd_doc::DocError::NothingToDo
                    {
                        self.push_warning(format!("The material could not be applied: {e}."));
                    }
                }
                Err(why) => self.push_warning(why),
            }
        }
    }
}
