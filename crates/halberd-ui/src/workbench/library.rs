//! The Library panel: the material and prop browsers once the game's
//! content is read, and what the workbench does when something is picked.

use super::Workbench;
use crate::materials::{AssetBrowser, BrowserAction};
use crate::panels::placeholder;
use egui::Ui;
use halberd_tools::PropChoice;
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
    /// True while the props tab is shown.
    showing_props: bool,
    /// What is known of each model read (lower-case name), for placing.
    prop_infos: HashMap<String, PropChoice>,
    /// A prop picked before its model was read; placed once it is.
    pending_prop: Option<String>,
    pub(super) status: ContentStatus,
}

impl Default for Library {
    fn default() -> Self {
        Self {
            materials: AssetBrowser::materials(),
            props: AssetBrowser::props(),
            showing_props: false,
            prop_infos: HashMap::new(),
            pending_prop: None,
            status: ContentStatus::NoGame,
        }
    }
}

/// What the user did in the Library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LibraryAction {
    Material(BrowserAction),
    Prop(BrowserAction),
}

impl Library {
    /// Tells `doc` the box of every model read so far.
    pub(super) fn give_model_boxes(&self, doc: &mut halberd_doc::Document) {
        for prop in self.prop_infos.values() {
            doc.set_model_box(&prop.model, prop.local);
        }
    }

    /// Draws the panel. What the user did is put in `action`, for the
    /// workbench to carry out once the panels are drawn.
    pub(super) fn show(
        &mut self,
        ui: &mut Ui,
        can_apply: bool,
        action: &mut Option<LibraryAction>,
    ) {
        if self.materials.is_empty() && self.props.is_empty() {
            let content = match &self.status {
                ContentStatus::NoGame => "Garry's Mod was not found, so its materials and models \
                     can't be shown. Start Halberd with --gmod-dir to point at it."
                    .to_string(),
                ContentStatus::Mounting => "Reading Garry's Mod's content…".to_string(),
                ContentStatus::Mounted(summary) => summary.clone(),
            };
            placeholder(
                ui,
                "Props, materials and entities",
                &format!(
                    "{content}\n\nMaterials and props appear here once the game's content is read."
                ),
            );
            return;
        }
        ui.horizontal(|ui| {
            if ui
                .selectable_label(!self.showing_props, MATERIALS_TAB)
                .clicked()
            {
                self.showing_props = false;
            }
            if ui.selectable_label(self.showing_props, PROPS_TAB).clicked() {
                self.showing_props = true;
            }
        });
        ui.separator();
        if self.showing_props {
            if let Some(done) = self.props.show(ui, can_apply) {
                *action = Some(LibraryAction::Prop(done));
            }
        } else if let Some(done) = self.materials.show(ui, can_apply) {
            *action = Some(LibraryAction::Material(done));
        }
    }
}

impl Workbench {
    /// The material browser, for handing it names and pictures.
    pub fn materials_mut(&mut self) -> &mut AssetBrowser {
        &mut self.library.materials
    }

    /// The prop browser, for handing it model names and pictures.
    pub fn props_mut(&mut self) -> &mut AssetBrowser {
        &mut self.library.props
    }

    /// Tells the Library what a model is like once it has been read, so
    /// it can be placed. A prop picked while it was being read is chosen
    /// for the Place tool now.
    pub fn add_prop_info(&mut self, prop: PropChoice) {
        let key = prop.model.to_ascii_lowercase();
        self.doc.set_model_box(&prop.model, prop.local);
        if self.library.pending_prop.as_deref() == Some(key.as_str()) {
            self.library.pending_prop = None;
            self.viewport.tools_mut().set_prop(Some(prop.clone()));
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

    /// Carries out what the user did in the Library.
    pub(super) fn library_action(&mut self, action: LibraryAction) {
        match action {
            LibraryAction::Material(action) => self.browser_action(action),
            LibraryAction::Prop(BrowserAction::Pick(name) | BrowserAction::Apply(name)) => {
                let key = name.to_ascii_lowercase();
                match self.library.prop_infos.get(&key) {
                    Some(prop) => {
                        let prop = prop.clone();
                        self.library.pending_prop = None;
                        self.viewport.tools_mut().set_prop(Some(prop));
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
