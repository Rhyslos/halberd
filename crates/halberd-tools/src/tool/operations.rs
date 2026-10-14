//! Shape operations: extrude, bevel, join, split, clip, bridge, mirror,
//! merging corners and deleting parts.
//!
//! Each works on the selected brushes and the picked corners, edges or
//! faces, and gives back one [`Command::ShapeEdit`] (or another edit) for
//! the interface to carry out, so it is one undo step. Nothing here changes
//! the map.

mod cut;

use super::ToolController;
use crate::camera::Camera;
use crate::elements::{ElementShape, PickedElement, SelectMode, editable_brushes};
use crate::gizmo::{Axis, GizmoMode};
use glam::Vec3;
use halberd_doc::{
    BrushObject, Command, Document, ElementKind, FaceInfo, Object, ObjectId, TransformKind,
};
use halberd_geom::{Brush, GeomError};

/// A shape operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeOp {
    /// Stand a new brush on each picked face (E).
    Extrude,
    /// Cut picked edges off with a slanted face (Ctrl+B).
    Bevel,
    /// Fill the space spanned by the picked parts with a new brush (F).
    Bridge,
    /// Cut brushes in two along the picked parts (K).
    Split,
    /// Cut brushes along the picked parts and keep the bigger part (X).
    Clip,
    /// Make the selected brushes one brush (Ctrl+J).
    Join,
    /// Move the picked corners together into one (M).
    MergeCorners,
    /// Take away the picked corners or faces (Delete).
    DeleteParts,
    /// Mirror the selected brushes along a world axis.
    Mirror(Axis),
    /// Mirror the selected brushes left to right as seen on screen (Ctrl+M).
    MirrorAcrossView,
}

impl ShapeOp {
    /// The operations in the order the Shape menu lists them.
    pub const MENU: [ShapeOp; 11] = [
        Self::Extrude,
        Self::Bevel,
        Self::Bridge,
        Self::Split,
        Self::Clip,
        Self::Join,
        Self::MergeCorners,
        Self::MirrorAcrossView,
        Self::Mirror(Axis::X),
        Self::Mirror(Axis::Y),
        Self::Mirror(Axis::Z),
    ];

    /// Name for the menu.
    pub fn label(self) -> &'static str {
        match self {
            Self::Extrude => "Extrude faces",
            Self::Bevel => "Bevel edges",
            Self::Bridge => "Bridge",
            Self::Split => "Split",
            Self::Clip => "Clip",
            Self::Join => "Join brushes",
            Self::MergeCorners => "Merge corners",
            Self::DeleteParts => "Delete picked parts",
            Self::Mirror(Axis::X) => "Mirror along X",
            Self::Mirror(Axis::Y) => "Mirror along Y",
            Self::Mirror(Axis::Z) => "Mirror up and down",
            Self::MirrorAcrossView => "Mirror left to right",
        }
    }

    /// One-line explanation, for tooltips.
    pub fn description(self) -> &'static str {
        match self {
            Self::Extrude => {
                "Stand a new brush on each picked face, one grid step deep, with its far face \
                 picked so you can pull it further"
            }
            Self::Bevel => "Cut each picked edge off with a slanted face, one grid step back",
            Self::Bridge => "Fill the space spanned by the picked corners, edges or faces",
            Self::Split => {
                "Cut the selected brushes in two: along a picked face, through three picked \
                 corners, or along a picked edge or two corners as you see them"
            }
            Self::Clip => "Cut like Split, but keep only the bigger part of each brush",
            Self::Join => {
                "Make the selected brushes one brush, filling any gaps between them (a brush \
                 must be convex)"
            }
            Self::MergeCorners => "Move the picked corners together, onto their middle",
            Self::DeleteParts => {
                "Take away the picked corners, or the picked faces (the faces around them \
                 grow to close the gap)"
            }
            Self::Mirror(_) => "Mirror the selected brushes about their middle",
            Self::MirrorAcrossView => {
                "Mirror the selected brushes left to right, as you see them, about their middle"
            }
        }
    }
}

/// What is selected after a shape operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectAfter {
    /// The selection stays (less anything taken away).
    Unchanged,
    /// Only the brushes the operation added.
    Added,
    /// The brushes the operation added and the ones it changed.
    AddedAndChanged,
}

/// What a shape operation asks the interface to do.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeOpOutcome {
    /// The edit.
    pub command: Command,
    /// What to select afterwards.
    pub select: SelectAfter,
    /// Parts to pick afterwards, on the selected brushes.
    pub pick_after: Vec<ElementShape>,
    /// A gizmo mode to switch to afterwards, if any.
    pub gizmo: Option<GizmoMode>,
    /// Something worth telling the user, for the Console.
    pub note: Option<String>,
}

impl ShapeOpOutcome {
    fn new(command: Command) -> Self {
        Self {
            command,
            select: SelectAfter::Unchanged,
            pick_after: Vec::new(),
            gizmo: None,
            note: None,
        }
    }
}

/// "face" or "3 faces", for edit names.
fn counted(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        one.to_string()
    } else {
        format!("{count} {many}")
    }
}

/// A selected brush object.
fn brush_object(doc: &Document, id: ObjectId) -> Option<&BrushObject> {
    match doc.get(id)? {
        Object::Brush(b) => Some(b),
        Object::Entity(_) => None,
    }
}

impl ToolController {
    /// Whether `op` can run now, or what to do first, in plain words.
    pub fn shape_op_ready(&self, op: ShapeOp, doc: &Document) -> Result<(), String> {
        let picks = &self.elements.items;
        let mode = self.select_mode;
        let need = |ok: bool, why: &str| if ok { Ok(()) } else { Err(why.to_string()) };
        match op {
            ShapeOp::Extrude => need(
                mode == SelectMode::Face && !picks.is_empty(),
                "Pick faces first: press 4 and click a face.",
            ),
            ShapeOp::Bevel => need(
                mode == SelectMode::Edge && !picks.is_empty(),
                "Pick edges first: press 3 and click an edge.",
            ),
            ShapeOp::Bridge => need(
                !picks.is_empty(),
                "Pick the corners, edges or faces to bridge first (2, 3 or 4).",
            ),
            ShapeOp::Split | ShapeOp::Clip => need(
                !picks.is_empty(),
                "Pick where to cut first: a face (4), an edge (3), or two or three corners (2).",
            ),
            ShapeOp::Join => need(
                editable_brushes(doc).len() >= 2,
                "Select two or more brushes to join (Ctrl+click adds to the selection).",
            ),
            ShapeOp::MergeCorners => need(
                mode == SelectMode::Vertex && picks.len() >= 2,
                "Pick two or more corners first: press 2, then Ctrl+click the corners.",
            ),
            ShapeOp::DeleteParts => match mode {
                SelectMode::Object => Err("Pick corners (2) or faces (4) to delete.".into()),
                SelectMode::Edge => {
                    Err("Edges can't be deleted on their own; delete corners or faces.".into())
                }
                SelectMode::Vertex | SelectMode::Face => need(
                    !picks.is_empty(),
                    "Pick the corners or faces to delete first, or press 1 to delete whole \
                     brushes.",
                ),
            },
            ShapeOp::Mirror(_) | ShapeOp::MirrorAcrossView => need(
                !editable_brushes(doc).is_empty(),
                "Select the brushes to mirror first.",
            ),
        }
    }

    /// Works out `op` on the map as it is, seen from `camera`. Gives the
    /// edit to make, or why it can't be done, in plain words.
    pub fn shape_op(
        &self,
        op: ShapeOp,
        doc: &Document,
        camera: &Camera,
    ) -> Result<ShapeOpOutcome, String> {
        self.shape_op_ready(op, doc)?;
        match op {
            ShapeOp::Extrude => self.extrude(doc),
            ShapeOp::Bevel => self.bevel(doc),
            ShapeOp::Bridge => self.bridge(doc),
            ShapeOp::Split => self.cut(doc, camera, true),
            ShapeOp::Clip => self.cut(doc, camera, false),
            ShapeOp::Join => join(doc),
            ShapeOp::MergeCorners => self.merge_corners(doc),
            ShapeOp::DeleteParts => self.delete_parts(doc),
            ShapeOp::Mirror(axis) => mirror(doc, axis as usize),
            ShapeOp::MirrorAcrossView => {
                let right = camera.right();
                mirror(doc, if right.x.abs() >= right.y.abs() { 0 } else { 1 })
            }
        }
    }

    /// Works out `op` and carries it out on `doc` as one undo step, then
    /// selects and picks what it asks for. Returns a note worth telling
    /// the user, if any, or why it could not be done, in plain words.
    pub fn apply_shape_op(
        &mut self,
        op: ShapeOp,
        doc: &mut Document,
        camera: &Camera,
    ) -> Result<Option<String>, String> {
        let outcome = self.shape_op(op, doc, camera)?;
        // New objects get ids above every id so far.
        let newest = doc.objects().map(|(id, _)| id).max();
        doc.end_step();
        doc.execute(outcome.command.clone())
            .map_err(|e| format!("That edit could not be made: {e}."))?;
        doc.end_step();
        let added: Vec<ObjectId> = doc
            .objects()
            .map(|(id, _)| id)
            .filter(|id| newest.is_none_or(|n| *id > n))
            .collect();
        self.finish_shape_op(&outcome, doc, &added);
        Ok(outcome.note)
    }

    /// After an operation's edit: selects what it asked for (`added`: the
    /// objects the edit added) and picks the parts it named.
    fn finish_shape_op(
        &mut self,
        outcome: &ShapeOpOutcome,
        doc: &mut Document,
        added: &[ObjectId],
    ) {
        let changed: Vec<ObjectId> = match &outcome.command {
            Command::ShapeEdit { changed, .. } => changed.iter().map(|(id, _)| *id).collect(),
            _ => Vec::new(),
        };
        match outcome.select {
            SelectAfter::Unchanged => {}
            SelectAfter::Added => doc.set_selection(added.iter().copied()),
            SelectAfter::AddedAndChanged => {
                doc.set_selection(added.iter().chain(&changed).copied());
            }
        }
        let brushes = editable_brushes(doc);
        self.elements.items = outcome
            .pick_after
            .iter()
            .filter_map(|shape| {
                brushes.iter().find_map(|(id, brush)| {
                    let item = PickedElement {
                        brush: *id,
                        shape: shape.clone(),
                    };
                    item.resolve(brush).map(|_| item)
                })
            })
            .collect();
        self.hovered_element = None;
        if let Some(mode) = outcome.gizmo
            && self.gizmo.mode.is_none()
        {
            self.gizmo.set_mode(Some(mode));
        }
    }

    /// Picked faces, as (brush, its object, face place), skipping any that
    /// no longer resolve.
    fn picked_faces<'d>(&self, doc: &'d Document) -> Vec<(ObjectId, &'d BrushObject, usize)> {
        self.elements
            .items
            .iter()
            .filter_map(|item| {
                let object = brush_object(doc, item.brush)?;
                Some((item.brush, object, item.face_index(object.brush())?))
            })
            .collect()
    }

    fn extrude(&self, doc: &Document) -> Result<ShapeOpOutcome, String> {
        let faces = self.picked_faces(doc);
        let mut added = Vec::new();
        let mut far_faces = Vec::new();
        for (_, object, face) in &faces {
            let base = &object.brush().faces()[*face];
            let new = object
                .brush()
                .extruded_face(*face, self.grid)
                .map_err(|e| format!("That face could not be extruded: {e}."))?;
            if let Some(far) = new
                .faces()
                .iter()
                .find(|f| f.plane().normal.dot(base.plane().normal) > 0.999)
            {
                far_faces.push(ElementShape::Face(far.vertices().to_vec()));
            }
            let infos = vec![object.faces()[*face].copied(); new.faces().len()];
            let brush = BrushObject::with_faces(new, infos, object.copied_data());
            added.push((brush, object.entity()));
        }
        let label = format!("Extrude {}", counted(added.len(), "face", "faces"));
        let mut outcome = ShapeOpOutcome::new(Command::ShapeEdit {
            label,
            changed: Vec::new(),
            added,
            removed: Vec::new(),
        });
        outcome.pick_after = far_faces;
        outcome.select = SelectAfter::Added;
        outcome.gizmo = Some(GizmoMode::Move);
        Ok(outcome)
    }

    fn bevel(&self, doc: &Document) -> Result<ShapeOpOutcome, String> {
        // The picked edges of each brush, as pairs of corners.
        let mut per_brush: Vec<(ObjectId, Vec<(usize, usize)>)> = Vec::new();
        for item in &self.elements.items {
            let Some(object) = brush_object(doc, item.brush) else {
                continue;
            };
            if let Some(&[a, b]) = item.resolve(object.brush()).as_deref() {
                match per_brush.iter_mut().find(|(id, _)| *id == item.brush) {
                    Some((_, edges)) => edges.push((a, b)),
                    None => per_brush.push((item.brush, vec![(a, b)])),
                }
            }
        }
        let mut changed = Vec::new();
        let mut count = 0;
        for (id, edges) in per_brush {
            let object = brush_object(doc, id).ok_or("A picked brush is gone.")?;
            count += edges.len();
            let (shaped, fates) = object.brush().bevelled(&edges, self.grid).map_err(
                |e| match e {
                    GeomError::BevelTooWide => format!(
                        "No bevel was made: a {} unit bevel is wider than the faces beside that \
                     edge. Pick a longer edge, or use a smaller grid.",
                        self.grid
                    ),
                    e => format!("No bevel was made: {e}."),
                },
            )?;
            changed.push((id, object.with_shape_mapped(shaped, &fates)));
        }
        let label = format!("Bevel {}", counted(count, "edge", "edges"));
        Ok(ShapeOpOutcome::new(Command::ShapeEdit {
            label,
            changed,
            added: Vec::new(),
            removed: Vec::new(),
        }))
    }

    fn bridge(&self, doc: &Document) -> Result<ShapeOpOutcome, String> {
        let points = picked_corners(&self.elements.items);
        let hull = Brush::hull(&points).map_err(|_| {
            "Nothing was bridged: pick parts that span a solid, at least four corners not all \
             on one flat surface."
                .to_string()
        })?;
        let sources: Vec<&BrushObject> = self
            .elements
            .items
            .iter()
            .filter_map(|item| brush_object(doc, item.brush))
            .collect();
        let infos = matching_faces(&hull, &sources);
        let entity = sources.first().and_then(|b| b.entity());
        let brush = BrushObject::with_faces(hull, infos, Vec::new());
        let mut outcome = ShapeOpOutcome::new(Command::ShapeEdit {
            label: "Bridge".into(),
            changed: Vec::new(),
            added: vec![(brush, entity)],
            removed: Vec::new(),
        });
        outcome.select = SelectAfter::Added;
        Ok(outcome)
    }

    fn merge_corners(&self, doc: &Document) -> Result<ShapeOpOutcome, String> {
        let points = picked_corners(&self.elements.items);
        let middle = (points.iter().copied().sum::<Vec3>() / points.len().max(1) as f32).round();
        let mut brushes = Vec::new();
        for (id, brush, corners) in self.elements.targets(doc) {
            let moves: Vec<(usize, Vec3)> = corners.iter().map(|&c| (c, middle)).collect();
            let (shaped, fates) = brush.with_moved_points(&moves).map_err(|_| {
                "The corners were not merged: the brush would be left flat.".to_string()
            })?;
            brushes.push((id, shaped, fates));
        }
        let mut outcome = ShapeOpOutcome::new(Command::EditBrushes {
            kind: TransformKind::Merge,
            parts: (ElementKind::Vertex, self.elements.items.len()),
            brushes,
        });
        outcome.pick_after = vec![ElementShape::Vertex(middle)];
        Ok(outcome)
    }

    fn delete_parts(&self, doc: &Document) -> Result<ShapeOpOutcome, String> {
        let kind = if self.select_mode == SelectMode::Face {
            ElementKind::Face
        } else {
            ElementKind::Vertex
        };
        let mut brushes = Vec::new();
        for (id, brush, corners) in self.elements.targets(doc) {
            let result = if kind == ElementKind::Face {
                let object = brush_object(doc, id).ok_or("A picked brush is gone.")?;
                let faces: Vec<usize> = self
                    .elements
                    .items
                    .iter()
                    .filter(|item| item.brush == id)
                    .filter_map(|item| item.face_index(object.brush()))
                    .collect();
                brush.without_faces(&faces)
            } else {
                brush.without_points(&corners)
            };
            let (shaped, fates) = result.map_err(|e| match e {
                GeomError::WouldOpen => {
                    "Nothing was deleted: without that face the brush would be open. Only a \
                     face whose neighbours can close the gap (a cut-off corner, say) can go."
                        .to_string()
                }
                _ => "Nothing was deleted: too little of the brush would be left.".to_string(),
            })?;
            brushes.push((id, shaped, fates));
        }
        Ok(ShapeOpOutcome::new(Command::EditBrushes {
            kind: TransformKind::Delete,
            parts: (kind, self.elements.items.len()),
            brushes,
        }))
    }
}

/// Every distinct corner of the picked parts.
fn picked_corners(items: &[PickedElement]) -> Vec<Vec3> {
    let mut points: Vec<Vec3> = Vec::new();
    for p in items.iter().flat_map(|i| i.shape.corners()) {
        if !points.iter().any(|q| q.distance(*p) < 0.01) {
            points.push(*p);
        }
    }
    points
}

/// Face data for each face of `brush`, copied from the face of `sources`
/// that lies on the same plane, or failing that faces the most nearly the
/// same way, so materials and texture alignment carry over.
fn matching_faces(brush: &Brush, sources: &[&BrushObject]) -> Vec<FaceInfo> {
    brush
        .faces()
        .iter()
        .map(|face| {
            let plane = face.plane();
            let score = |other: &halberd_geom::Face| {
                let o = other.plane();
                let same_place = (o.distance - plane.distance).abs() < 0.05;
                o.normal.dot(plane.normal) + if same_place { 1.0 } else { 0.0 }
            };
            sources
                .iter()
                .flat_map(|b| b.brush().faces().iter().zip(b.faces()))
                .max_by(|a, b| score(a.0).total_cmp(&score(b.0)))
                .map(|(_, info)| info.copied())
                .unwrap_or_default()
        })
        .collect()
}

fn join(doc: &Document) -> Result<ShapeOpOutcome, String> {
    let ids: Vec<ObjectId> = editable_brushes(doc).iter().map(|(id, _)| *id).collect();
    let objects: Vec<&BrushObject> = ids.iter().filter_map(|id| brush_object(doc, *id)).collect();
    let points: Vec<Vec3> = objects.iter().flat_map(|b| b.brush().points()).collect();
    let hull =
        Brush::hull(&points).map_err(|e| format!("The brushes could not be joined: {e}."))?;
    let separate: f32 = objects.iter().map(|b| b.brush().volume()).sum();
    let gaps = hull.volume() > separate * 1.001 + 1.0;
    let infos = matching_faces(&hull, &objects);
    let (keep, rest) = ids.split_first().ok_or("Nothing to join.")?;
    let kept = objects.first().ok_or("Nothing to join.")?;
    let joined = BrushObject::with_faces(hull, infos, kept.file_data.clone());
    let mut outcome = ShapeOpOutcome::new(Command::ShapeEdit {
        label: format!("Join {} brushes", ids.len()),
        changed: vec![(*keep, joined)],
        added: Vec::new(),
        removed: rest.to_vec(),
    });
    if gaps {
        outcome.note = Some(
            "The joined brush also fills the space between the brushes: a brush must be \
             convex (no dents), as Source requires."
                .into(),
        );
    }
    Ok(outcome)
}

fn mirror(doc: &Document, axis: usize) -> Result<ShapeOpOutcome, String> {
    let brushes = editable_brushes(doc);
    let bounds = brushes
        .iter()
        .map(|(_, b)| b.bounds())
        .reduce(halberd_geom::Aabb::union)
        .ok_or("Select the brushes to mirror first.")?;
    let at = bounds.center()[axis];
    let mirrored = brushes
        .iter()
        .map(|(id, b)| Ok((*id, b.mirrored(axis, at)?)))
        .collect::<Result<Vec<_>, GeomError>>()
        .map_err(|e| format!("Nothing was mirrored: {e}."))?;
    Ok(ShapeOpOutcome::new(Command::TransformBrushes {
        kind: TransformKind::Mirror,
        brushes: mirrored,
    }))
}

#[cfg(test)]
mod tests;
