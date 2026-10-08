//! The panels the editor window is made of.

use serde::{Deserialize, Serialize};

/// One dockable panel of the editor window.
///
/// Each panel appears exactly once in the layout. Panels can be moved and
/// resized but not closed, so none can be lost by accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Panel {
    /// The 3D view of the map.
    Viewport,
    /// Asset browsers: props, materials, entities.
    Browsers,
    /// The tree of everything in the map.
    Scene,
    /// Settings of the selected object (called "Inspector" in other editors).
    Properties,
    /// Messages, compile logs, warnings and limits.
    Console,
}

impl Panel {
    /// Every panel, in reading order.
    pub const ALL: [Panel; 5] = [
        Self::Viewport,
        Self::Browsers,
        Self::Scene,
        Self::Properties,
        Self::Console,
    ];

    /// The name shown on the panel's tab.
    pub fn title(self) -> &'static str {
        match self {
            Self::Viewport => "Viewport",
            Self::Browsers => "Browsers",
            Self::Scene => "Scene",
            Self::Properties => "Properties",
            Self::Console => "Console",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_unique_and_match_the_spec() {
        let titles: Vec<_> = Panel::ALL.iter().map(|p| p.title()).collect();
        assert_eq!(
            titles,
            ["Viewport", "Browsers", "Scene", "Properties", "Console"]
        );
    }

    #[test]
    fn the_word_inspector_is_not_used() {
        assert!(Panel::ALL.iter().all(|p| !p.title().contains("Inspector")));
    }
}
