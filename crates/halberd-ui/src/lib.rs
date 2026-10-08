//! The interface panels.
//!
//! The editor window is a [`Workbench`]: a menu bar on top and a dock of
//! [`Panel`]s that users can drag, split and resize. The default arrangement
//! ([`default_layout`]) follows the feature spec: Library on the left, the
//! viewport in the middle, Scene (with Layers as a second tab) and
//! Properties on the right, and the Console along the bottom. The viewport ([`ViewportPanel`]) handles camera
//! input and shows images drawn by a [`ViewportRenderer`] supplied by the
//! program.
//!
//! The workbench owns the open map ([`halberd_doc::Document`]). Panels
//! select directly, and change the map only through the document's
//! commands, so every edit can be undone (Edit menu, Ctrl+Z / Ctrl+Y).

mod layout;
mod panel;
mod panels;
mod viewport;
mod workbench;

pub use layout::{default_layout, is_complete, restore_or_default};
pub use panel::Panel;
pub use viewport::{
    NoRenderer, VIEWPORT_LABEL, ViewportOptions, ViewportPanel, ViewportRenderer, ViewportView,
};
pub use workbench::{
    AppInfo, DELETE_SHORTCUT, PLAYER_TOGGLE_LABEL, QUIT_SHORTCUT, REDO_SHORTCUT, UNDO_SHORTCUT,
    Workbench, WorkbenchAction,
};
