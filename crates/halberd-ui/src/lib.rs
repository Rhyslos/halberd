//! The interface panels.
//!
//! The editor window is a [`Workbench`]: a menu bar on top and a dock of
//! [`Panel`]s that users can drag, split and resize. The default arrangement
//! ([`default_layout`]) follows the feature spec: Browsers on the left, the
//! viewport in the middle, Scene and Properties on the right, and the
//! Console along the bottom.
//!
//! This crate only draws. It never changes the map document directly; edits
//! will go through commands once the document exists.

mod layout;
mod panel;
mod workbench;

pub use layout::{default_layout, is_complete, restore_or_default};
pub use panel::Panel;
pub use workbench::{AppInfo, Workbench, WorkbenchAction};
