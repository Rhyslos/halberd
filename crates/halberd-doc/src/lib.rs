//! The map document: objects, selection, commands and undo.
//!
//! The single source of truth for an open map: brushes, props, entities and groups with stable IDs, the selection, and the command system every edit goes through, which gives undo and redo.
//!
//! Status: skeleton. See this crate's README for what it must never do.

/// The name of this crate, used in logs and diagnostics.
pub const CRATE_NAME: &str = "halberd-doc";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
