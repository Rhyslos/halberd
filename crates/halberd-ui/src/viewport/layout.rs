//! How the viewport panel is divided: up to six views, each showing the 3D
//! perspective or one of the six flat side views, in a few preset layouts.

use egui::{Rect, pos2, vec2};
use halberd_tools::{CameraController, Side, SideCamera};
use serde::{Deserialize, Serialize};

/// Most views the panel shows at once.
pub const MAX_VIEWS: usize = 6;
/// Gap between views, in points.
const GAP: f32 = 2.0;

/// A preset arrangement of views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ViewLayout {
    /// One view filling the panel.
    #[default]
    Single,
    /// Two views side by side.
    SideBySide,
    /// Two views, one above the other.
    Stacked,
    /// A large view on the left, two smaller ones stacked on the right.
    Three,
    /// Four views in a square, as in Hammer.
    Four,
    /// Six views, three across and two down.
    Six,
}

impl ViewLayout {
    /// Every layout, in menu order.
    pub const ALL: [ViewLayout; 6] = [
        Self::Single,
        Self::SideBySide,
        Self::Stacked,
        Self::Three,
        Self::Four,
        Self::Six,
    ];

    /// Name for the interface.
    pub fn label(self) -> &'static str {
        match self {
            Self::Single => "One view",
            Self::SideBySide => "Two side by side",
            Self::Stacked => "Two stacked",
            Self::Three => "Three (one large)",
            Self::Four => "Four (as in Hammer)",
            Self::Six => "Six",
        }
    }

    /// How many views it shows.
    pub fn count(self) -> usize {
        self.rects(Rect::from_min_size(pos2(0.0, 0.0), vec2(600.0, 600.0)))
            .len()
    }

    /// Where each view goes inside `area`, in view order (left to right,
    /// then top to bottom; the large view first).
    pub fn rects(self, area: Rect) -> Vec<Rect> {
        let grid = |columns: usize, rows: usize| -> Vec<Rect> {
            let w = ((area.width() - GAP * (columns - 1) as f32) / columns as f32).max(1.0);
            let h = ((area.height() - GAP * (rows - 1) as f32) / rows as f32).max(1.0);
            (0..rows)
                .flat_map(|r| (0..columns).map(move |c| (c, r)))
                .map(|(c, r)| {
                    let min = area.min + vec2(c as f32 * (w + GAP), r as f32 * (h + GAP));
                    Rect::from_min_size(min, vec2(w, h))
                })
                .collect()
        };
        match self {
            Self::Single => vec![area],
            Self::SideBySide => grid(2, 1),
            Self::Stacked => grid(1, 2),
            Self::Four => grid(2, 2),
            Self::Six => grid(3, 2),
            Self::Three => {
                let left = ((area.width() - GAP) * 0.6).max(1.0);
                let right = (area.width() - GAP - left).max(1.0);
                let half = ((area.height() - GAP) * 0.5).max(1.0);
                vec![
                    Rect::from_min_size(area.min, vec2(left, area.height())),
                    Rect::from_min_size(area.min + vec2(left + GAP, 0.0), vec2(right, half)),
                    Rect::from_min_size(area.min + vec2(left + GAP, half + GAP), vec2(right, half)),
                ]
            }
        }
    }
}

/// What one view shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaneView {
    /// The 3D view, with perspective.
    Perspective,
    /// A flat side view.
    Side(Side),
}

impl PaneView {
    /// Every choice, in menu order.
    pub const ALL: [PaneView; 7] = [
        Self::Perspective,
        Self::Side(Side::Top),
        Self::Side(Side::Front),
        Self::Side(Side::Right),
        Self::Side(Side::Bottom),
        Self::Side(Side::Back),
        Self::Side(Side::Left),
    ];

    /// Name for the interface.
    pub fn label(self) -> &'static str {
        match self {
            Self::Perspective => "Perspective",
            Self::Side(side) => side.label(),
        }
    }

    /// What view `index` shows at first: the 3D view, Top, Front, Right
    /// (Hammer's four), then Bottom and Back.
    pub fn default_for(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(Self::Perspective)
    }
}

/// One view's state: what it shows and its cameras (each kind keeps its
/// own, so switching back finds the view as it was left).
#[derive(Debug, Clone)]
pub(super) struct Pane {
    pub(super) view: PaneView,
    pub(super) perspective: CameraController,
    pub(super) side: SideCamera,
    pub(super) right_was_held: bool,
    pub(super) middle_was_held: bool,
}

impl Pane {
    pub(super) fn new(index: usize, perspective: CameraController) -> Self {
        let view = PaneView::default_for(index);
        let side = match view {
            PaneView::Side(side) => side,
            PaneView::Perspective => Side::Top,
        };
        Self {
            view,
            perspective,
            side: SideCamera::new(side),
            right_was_held: false,
            middle_was_held: false,
        }
    }

    /// Shows `view` from now on.
    pub(super) fn set_view(&mut self, view: PaneView) {
        self.view = view;
        if let PaneView::Side(side) = view {
            self.side.side = side;
        }
    }

    /// True if shapes can be drawn in it: a 3D view, or the Top or Bottom
    /// view (they need a floor to stand on).
    pub(super) fn can_draw(&self) -> bool {
        match self.view {
            PaneView::Perspective => true,
            PaneView::Side(side) => side.is_horizontal(),
        }
    }

    /// The camera it draws and picks with.
    pub(super) fn camera(&self) -> halberd_tools::Camera {
        match self.view {
            PaneView::Perspective => *self.perspective.camera(),
            PaneView::Side(_) => self.side.camera(),
        }
    }
}

/// The views as saved between sessions: the layout, what each view shows,
/// and the side views' cameras.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ViewSetup {
    /// The arrangement.
    pub layout: ViewLayout,
    /// What each view shows, in view order.
    pub views: Vec<PaneView>,
    /// Each view's side camera, in view order.
    pub sides: Vec<SideCamera>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_divide_the_area_without_overlap() {
        let area = Rect::from_min_size(pos2(10.0, 20.0), vec2(900.0, 600.0));
        let counts = [1, 2, 2, 3, 4, 6];
        for (layout, count) in ViewLayout::ALL.into_iter().zip(counts) {
            let rects = layout.rects(area);
            assert_eq!(rects.len(), count, "{layout:?}");
            assert_eq!(layout.count(), count);
            for (i, a) in rects.iter().enumerate() {
                assert!(area.expand(0.01).contains_rect(*a), "{layout:?} {a:?}");
                for b in &rects[i + 1..] {
                    assert!(!a.shrink(0.01).intersects(b.shrink(0.01)), "{layout:?}");
                }
            }
            let covered: f32 = rects.iter().map(|r| r.area()).sum();
            assert!(
                covered > area.area() * 0.98,
                "{layout:?} leaves little unused"
            );
        }
    }

    #[test]
    fn the_first_four_views_are_hammers_and_all_seven_choices_exist() {
        let first: Vec<PaneView> = (0..4).map(PaneView::default_for).collect();
        assert_eq!(
            first,
            [
                PaneView::Perspective,
                PaneView::Side(Side::Top),
                PaneView::Side(Side::Front),
                PaneView::Side(Side::Right),
            ]
        );
        for side in Side::ALL {
            assert!(PaneView::ALL.contains(&PaneView::Side(side)));
        }
    }
}
