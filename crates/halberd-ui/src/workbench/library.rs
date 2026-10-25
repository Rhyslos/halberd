//! The Library panel: the material browser once the game's content is
//! read, and what the workbench does when a material is picked or applied.

use super::Workbench;
use crate::materials::{BrowserAction, MaterialBrowser};
use crate::panels::placeholder;
use egui::Ui;

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

/// Draws the Library panel. What the user did in the browser is put in
/// `action`, for the workbench to carry out once the panels are drawn.
pub(super) fn show(
    ui: &mut Ui,
    materials: &mut MaterialBrowser,
    status: &ContentStatus,
    can_apply: bool,
    action: &mut Option<BrowserAction>,
) {
    if !materials.is_empty() {
        if let Some(done) = materials.show(ui, can_apply) {
            *action = Some(done);
        }
        return;
    }
    let content = match status {
        ContentStatus::NoGame => "Garry's Mod was not found, so its materials and models can't be \
             shown. Start Halberd with --gmod-dir to point at it."
            .to_string(),
        ContentStatus::Mounting => "Reading Garry's Mod's content…".to_string(),
        ContentStatus::Mounted(summary) => summary.clone(),
    };
    placeholder(
        ui,
        "Props, materials and entities",
        &format!("{content}\n\nMaterials appear here once the game's content is read."),
    );
}

impl Workbench {
    /// The material browser, for handing it names and pictures.
    pub fn materials_mut(&mut self) -> &mut MaterialBrowser {
        &mut self.materials
    }

    /// Tells the Library panel how far mounting the game's content has got.
    pub fn set_content_status(&mut self, status: ContentStatus) {
        self.content_status = status;
    }

    /// What the Library panel says about the game's content.
    pub fn content_status(&self) -> &ContentStatus {
        &self.content_status
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
