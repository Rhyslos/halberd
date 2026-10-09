//! Picking the corners, edges and faces of selected brushes, to move,
//! rotate or scale them with the gizmo (like Hammer's vertex tool).
//!
//! Picked parts are remembered by where they are, not by number: a brush's
//! corners are numbered afresh whenever its shape changes, while a corner's
//! position only changes when it is moved, and then the gizmo moves the
//! remembered position along with it. Parts that no longer exist (undone,
//! merged away) drop out of the pick on the next frame.

mod pick;

use glam::Vec3;
use halberd_doc::{Document, ElementKind, Object, ObjectId};
use halberd_geom::{Aabb, Brush};
pub use pick::{ElementOverlay, ScreenElement};
pub(crate) use pick::{candidates, on_screen, pick};

/// How close the pointer must be to a corner or edge to pick it, in points.
pub const ELEMENT_GRAB_POINTS: f32 = 8.0;
/// Corners within this many units of a remembered position are the same
/// corner (positions pass through rounding when a brush is rebuilt).
const SAME_PLACE: f32 = 0.1;

/// What a click picks in the Select tool: whole objects, or the corners,
/// edges or faces of the selected brushes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectMode {
    /// Whole objects (1).
    #[default]
    Object,
    /// Corners (2).
    Vertex,
    /// Edges (3).
    Edge,
    /// Faces (4).
    Face,
}

impl SelectMode {
    /// Every mode, in the order the toolbar shows them.
    pub const ALL: [SelectMode; 4] = [Self::Object, Self::Vertex, Self::Edge, Self::Face];

    /// Short name for the toolbar.
    pub fn label(self) -> &'static str {
        match self {
            Self::Object => "Object",
            Self::Vertex => "Vertex",
            Self::Edge => "Edge",
            Self::Face => "Face",
        }
    }

    /// The key that picks this mode.
    pub fn key(self) -> &'static str {
        match self {
            Self::Object => "1",
            Self::Vertex => "2",
            Self::Edge => "3",
            Self::Face => "4",
        }
    }

    /// One-line explanation, for tooltips.
    pub fn description(self) -> &'static str {
        match self {
            Self::Object => "Clicks pick whole objects (1)",
            Self::Vertex => {
                "Clicks pick corners of the selected brushes; move them with the gizmo (2)"
            }
            Self::Edge => "Clicks pick edges of the selected brushes; move them with the gizmo (3)",
            Self::Face => "Clicks pick faces of the selected brushes; move them with the gizmo (4)",
        }
    }

    /// The kind of part this mode picks, or `None` for whole objects.
    pub fn element(self) -> Option<ElementKind> {
        match self {
            Self::Object => None,
            Self::Vertex => Some(ElementKind::Vertex),
            Self::Edge => Some(ElementKind::Edge),
            Self::Face => Some(ElementKind::Face),
        }
    }
}

/// Where a picked part is: a corner, an edge's two ends, or a face's
/// corners.
#[derive(Debug, Clone, PartialEq)]
pub enum ElementShape {
    /// A corner.
    Vertex(Vec3),
    /// An edge's two ends.
    Edge([Vec3; 2]),
    /// A face's corners.
    Face(Vec<Vec3>),
}

impl ElementShape {
    /// The kind of part.
    pub fn kind(&self) -> ElementKind {
        match self {
            Self::Vertex(_) => ElementKind::Vertex,
            Self::Edge(_) => ElementKind::Edge,
            Self::Face(_) => ElementKind::Face,
        }
    }

    /// Its corners.
    pub fn corners(&self) -> &[Vec3] {
        match self {
            Self::Vertex(p) => std::slice::from_ref(p),
            Self::Edge(ends) => ends,
            Self::Face(corners) => corners,
        }
    }

    /// The same part with every corner passed through `f`.
    pub(crate) fn mapped(&self, f: impl Fn(Vec3) -> Vec3) -> Self {
        match self {
            Self::Vertex(p) => Self::Vertex(f(*p)),
            Self::Edge([a, b]) => Self::Edge([f(*a), f(*b)]),
            Self::Face(corners) => Self::Face(corners.iter().map(|p| f(*p)).collect()),
        }
    }
}

/// A picked part of a brush.
#[derive(Debug, Clone, PartialEq)]
pub struct PickedElement {
    /// The brush it belongs to.
    pub brush: ObjectId,
    /// Where it is.
    pub shape: ElementShape,
}

impl PickedElement {
    /// The part's corners as places in [`Brush::points`], or `None` if the
    /// brush no longer has this part.
    pub(crate) fn resolve(&self, brush: &Brush) -> Option<Vec<usize>> {
        let (points, faces, edges) = brush.topology();
        let find = |at: Vec3| points.iter().position(|p| p.distance(at) < SAME_PLACE);
        match &self.shape {
            ElementShape::Vertex(at) => Some(vec![find(*at)?]),
            ElementShape::Edge([a, b]) => {
                let ends = (find(*a)?, find(*b)?);
                let edge = (ends.0.min(ends.1), ends.0.max(ends.1));
                edges.contains(&edge).then(|| vec![ends.0, ends.1])
            }
            ElementShape::Face(corners) => {
                let mut wanted: Vec<usize> =
                    corners.iter().map(|c| find(*c)).collect::<Option<_>>()?;
                wanted.sort_unstable();
                faces
                    .into_iter()
                    .find(|face| {
                        let mut face = face.clone();
                        face.sort_unstable();
                        face == wanted
                    })
                    .map(|_| wanted)
            }
        }
    }

    /// The same part, with its remembered corners moved onto the brush's
    /// actual corners.
    fn settled(&self, brush: &Brush, indices: &[usize]) -> Self {
        let points = brush.points();
        let at = |i: usize| points[indices[i]];
        let shape = match &self.shape {
            ElementShape::Vertex(_) => ElementShape::Vertex(at(0)),
            ElementShape::Edge(_) => ElementShape::Edge([at(0), at(1)]),
            ElementShape::Face(corners) => ElementShape::Face(
                corners
                    .iter()
                    .map(|c| {
                        points
                            .iter()
                            .copied()
                            .find(|p| p.distance(*c) < SAME_PLACE)
                            .unwrap_or(*c)
                    })
                    .collect(),
            ),
        };
        Self {
            brush: self.brush,
            shape,
        }
    }
}

/// The brushes whose parts can be picked: the selected brushes, and the
/// brushes of selected brush entities.
pub(crate) fn editable_brushes(doc: &Document) -> Vec<(ObjectId, &Brush)> {
    let mut out = Vec::new();
    for &id in doc.selection() {
        let ids: Vec<ObjectId> = if doc.has_brushes(id) {
            doc.brushes_of(id).collect()
        } else {
            vec![id]
        };
        for id in ids {
            if let Some(Object::Brush(b)) = doc.get(id)
                && !out.iter().any(|(o, _)| *o == id)
            {
                out.push((id, b.brush()));
            }
        }
    }
    out
}

/// The picked parts.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Elements {
    pub(crate) items: Vec<PickedElement>,
}

/// The brushes to change for the picked parts: each with its shape now and
/// the places of the corners to move.
pub(crate) type Targets = Vec<(ObjectId, Brush, Vec<usize>)>;

impl Elements {
    /// Drops parts of brushes no longer selected or no longer there, and
    /// settles the rest onto their brushes' actual corners.
    pub(crate) fn refresh(&mut self, doc: &Document, kind: Option<ElementKind>) {
        let brushes = editable_brushes(doc);
        self.items = self
            .items
            .iter()
            .filter(|item| Some(item.shape.kind()) == kind)
            .filter_map(|item| {
                let (_, brush) = brushes.iter().find(|(id, _)| *id == item.brush)?;
                let indices = item.resolve(brush)?;
                Some(item.settled(brush, &indices))
            })
            .collect();
    }

    /// Picks only `item`, or nothing.
    pub(crate) fn set(&mut self, item: Option<PickedElement>) {
        self.items = item.into_iter().collect();
    }

    /// Adds `item`, or takes it out if it was picked.
    pub(crate) fn toggle(&mut self, item: PickedElement) {
        match self.items.iter().position(|i| same_part(i, &item)) {
            Some(at) => {
                self.items.remove(at);
            }
            None => self.items.push(item),
        }
    }

    /// True if `item` is picked.
    pub(crate) fn contains(&self, item: &PickedElement) -> bool {
        self.items.iter().any(|i| same_part(i, item))
    }

    /// The brushes and corners the picked parts cover.
    pub(crate) fn targets(&self, doc: &Document) -> Targets {
        let brushes = editable_brushes(doc);
        let mut out: Targets = Vec::new();
        for item in &self.items {
            let Some((_, brush)) = brushes.iter().find(|(id, _)| *id == item.brush) else {
                continue;
            };
            let Some(indices) = item.resolve(brush) else {
                continue;
            };
            let at = match out.iter().position(|(id, ..)| *id == item.brush) {
                Some(at) => at,
                None => {
                    out.push((item.brush, (*brush).clone(), Vec::new()));
                    out.len() - 1
                }
            };
            let entry = &mut out[at];
            for i in indices {
                if !entry.2.contains(&i) {
                    entry.2.push(i);
                }
            }
        }
        out
    }

    /// The box around every picked corner.
    pub(crate) fn bounds(&self) -> Option<Aabb> {
        Aabb::from_points(
            self.items
                .iter()
                .flat_map(|i| i.shape.corners().iter().copied()),
        )
    }
}

/// True if both are the same part of the same brush.
fn same_part(a: &PickedElement, b: &PickedElement) -> bool {
    if a.brush != b.brush || a.shape.kind() != b.shape.kind() {
        return false;
    }
    let (ca, cb) = (a.shape.corners(), b.shape.corners());
    ca.len() == cb.len()
        && ca
            .iter()
            .all(|p| cb.iter().any(|q| p.distance(*q) < SAME_PLACE))
}

#[cfg(test)]
mod tests;
