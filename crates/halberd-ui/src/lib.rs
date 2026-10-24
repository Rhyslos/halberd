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

mod bindings;
mod console;
mod keybinds;
mod keymap;
mod layout;
mod materials;
mod panel;
mod panels;
mod settings_page;
mod viewport;
mod workbench;

pub use bindings::{
    BindingGroup, BindingRefused, Bindings, Command, MAX_KEY_OVERRIDES, NotAKey, check_binding,
    chord_text, is_modifier_key, parse_chord,
};
pub use console::{
    CONSOLE_CLEAR_BUTTON, CONSOLE_COPY_BUTTON, CONSOLE_SEARCH_NAME, Console, ConsoleMirror,
    LogEntry, LogLevel, MAX_CONSOLE_ENTRIES,
};
pub use keybinds::{KEYBINDS_SEARCH_NAME, KEYBINDS_TITLE, KeybindsViewer, drawn_keys};
pub use keymap::{KEYBINDS, KeyCap, KeyContext, Keybind, binds_with, current_binds, mouse_binds};
pub use layout::{default_layout, is_complete, restore_or_default};
pub use materials::{
    APPLY_MATERIAL_LABEL, AssetBrowser, BrowserAction, MATERIAL_SEARCH_NAME, MaterialBrowser,
    PROP_SEARCH_NAME, THUMBNAIL_SIZE,
};
pub use panel::Panel;
pub use settings_page::{
    KEY_FILTER_NAME, RESET_KEYS_LABEL, SETTINGS_TITLE, SettingsPage, SettingsTab, WASD_LABEL,
    key_button_name,
};
pub use viewport::{
    GO_TO_LEAK_LABEL, HIDE_LEAK_LABEL, INSIDE_ENTITIES_LABEL, LAYOUT_PICKER_LABEL,
    LEAK_BANNER_TEXT, MAX_VIEWS, NoRenderer, PLACING_LABEL, PaneView, RenderMode,
    SHAPE_OP_COMMANDS, VIEWPORT_LABEL, ViewLayout, ViewSetup, ViewportOptions, ViewportPanel,
    ViewportRenderer, ViewportView,
};
pub use workbench::{
    AppInfo, CANCEL_BUTTON, COMPILE_LABEL, COMPILE_SHORTCUT, CompileLine, ContentStatus,
    DEFAULT_SKY, DELETE_SHORTCUT, DONT_SAVE_BUTTON, ENTITIES_TAB, ENTITY_SEARCH_NAME, FileIntent,
    LEAK_CHECK_LABEL, MATERIALS_TAB, NEW_SHORTCUT, OPEN_SHORTCUT, PLAYER_FOLLOW_LABEL,
    PLAYER_TOGGLE_LABEL, PROPS_TAB, QUIT_SHORTCUT, REDO_SHORTCUT, SAVE_AS_SHORTCUT, SAVE_BUTTON,
    SAVE_SHORTCUT, SHOW_LEAK_LABEL, SKY_PICKER_NAME, STOP_COMPILE_LABEL, UNDO_SHORTCUT,
    WASD_NO_LABEL, WASD_QUESTION_TITLE, WASD_YES_LABEL, Workbench, WorkbenchAction, sky_names,
};
