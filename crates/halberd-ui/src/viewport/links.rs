//! The connection view: lines in every view from entities to the entities
//! their outputs reach, with an arrow pointing at the receiver. By default
//! only the selected entities' connections are drawn (outgoing in green,
//! incoming in blue); the View menu can show all of them or none.

use super::ViewportPanel;
use egui::{Color32, Painter, Rect, Stroke, vec2};
use glam::{Vec2, Vec3};
use halberd_doc::Document;
use halberd_tools::Camera;

/// Colour of a connection leaving the selection.
const OUTGOING_COLOR: Color32 = Color32::from_rgb(90, 220, 120);
/// Colour of a connection reaching the selection.
const INCOMING_COLOR: Color32 = Color32::from_rgb(110, 170, 255);
/// Colour of other connections (when all are shown).
const OTHER_COLOR: Color32 = Color32::from_rgba_premultiplied(150, 150, 170, 150);
/// Most lines drawn at once.
const MAX_LINES: usize = 4000;

/// Which connections the views draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ConnectionView {
    /// Only those of the selected entities.
    #[default]
    Selected,
    /// Every connection in the map.
    All,
    /// None.
    Off,
}

impl ConnectionView {
    /// Every choice, in menu order.
    pub const ALL: [ConnectionView; 3] = [Self::Selected, Self::All, Self::Off];

    /// The menu's name for it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Selected => "Connections of the selection",
            Self::All => "All connections",
            Self::Off => "No connections",
        }
    }
}

/// One line to draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Link {
    from: Vec3,
    to: Vec3,
    color: Color32,
}

/// The lines, and the document state they were worked out for.
#[derive(Debug, Default)]
pub(super) struct LinkCache {
    key: Option<(u64, u64, u64, u64, ConnectionView)>,
    links: Vec<Link>,
}

impl ViewportPanel {
    /// Which connections the views draw.
    pub fn connection_view(&self) -> ConnectionView {
        self.connection_view
    }

    /// Chooses which connections the views draw.
    pub fn set_connection_view(&mut self, view: ConnectionView) {
        self.connection_view = view;
    }

    /// Works out the lines for `doc` if it changed since last time.
    pub(super) fn update_links(&mut self, doc: &Document) {
        let key = (
            doc.instance(),
            doc.revision(),
            doc.selection_revision(),
            doc.model_box_revision(),
            self.connection_view,
        );
        if self.links.key == Some(key) {
            return;
        }
        self.links.key = Some(key);
        self.links.links = links_for(doc, self.connection_view);
    }

    /// The number of lines drawn now (for tests).
    pub fn connection_lines(&self) -> usize {
        self.links.links.len()
    }

    /// Draws the lines over one view.
    pub(super) fn draw_links(&self, painter: &Painter, rect: Rect, camera: &Camera, size: Vec2) {
        for link in &self.links.links {
            draw_link(painter, rect, camera, size, link);
        }
    }
}

fn links_for(doc: &Document, view: ConnectionView) -> Vec<Link> {
    if view == ConnectionView::Off {
        return Vec::new();
    }
    let selection = doc.selection();
    let center = |id| doc.bounds_of(id).map(|b| b.center());
    doc.connection_links()
        .into_iter()
        .filter_map(|(from, to)| {
            let color = if selection.contains(&from) {
                OUTGOING_COLOR
            } else if selection.contains(&to) {
                INCOMING_COLOR
            } else if view == ConnectionView::All {
                OTHER_COLOR
            } else {
                return None;
            };
            Some(Link {
                from: center(from)?,
                to: center(to)?,
                color,
            })
        })
        .take(MAX_LINES)
        .collect()
}

fn draw_link(painter: &Painter, rect: Rect, camera: &Camera, size: Vec2, link: &Link) {
    let Some((a, b)) = super::leak::clip_to_view(camera, link.from, link.to) else {
        return;
    };
    let (Some(a), Some(b)) = (camera.project(a, size), camera.project(b, size)) else {
        return;
    };
    let (a, b) = (rect.min + vec2(a.x, a.y), rect.min + vec2(b.x, b.y));
    painter.line_segment([a, b], Stroke::new(2.0, link.color));
    // An arrowhead two thirds of the way along, pointing at the receiver.
    let along = b - a;
    let length = along.length();
    if length < 12.0 {
        return;
    }
    let dir = along / length;
    let tip = a + along * 0.66;
    let side = vec2(-dir.y, dir.x);
    let back = tip - dir * 9.0;
    painter.add(egui::Shape::convex_polygon(
        vec![tip, back + side * 5.0, back - side * 5.0],
        link.color,
        Stroke::NONE,
    ));
}
