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
    /// The asset library: props, materials, entities. It used to be called
    /// "Browsers"; layouts saved under that name still load.
    #[serde(alias = "Browsers")]
    Library,
    /// The tree of everything in the map.
    Scene,
    /// Named sections of the map that can be hidden, locked and selected
    /// together. Saved to Hammer as visgroups.
    Layers,
    /// Settings of the selected object (called "Inspector" in other editors).
    Properties,
    /// Messages, compile logs, warnings and limits.
    Console,
}

impl Panel {
    /// Every panel, in reading order.
    pub const ALL: [Panel; 6] = [
        Self::Viewport,
        Self::Library,
        Self::Scene,
        Self::Layers,
        Self::Properties,
        Self::Console,
    ];

    /// The name shown on the panel's tab.
    pub fn title(self) -> &'static str {
        match self {
            Self::Viewport => "Viewport",
            Self::Library => "Library",
            Self::Scene => "Scene",
            Self::Layers => "Layers",
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
            [
                "Viewport",
                "Library",
                "Scene",
                "Layers",
                "Properties",
                "Console"
            ]
        );
    }

    #[test]
    fn the_old_browsers_name_still_loads_as_library() {
        assert_eq!(ron::from_str::<Panel>("Browsers").unwrap(), Panel::Library);
        assert_eq!(ron::to_string(&Panel::Library).unwrap(), "Library");
    }

    #[test]
    fn the_word_inspector_is_not_used() {
        assert!(Panel::ALL.iter().all(|p| !p.title().contains("Inspector")));
    }
}
