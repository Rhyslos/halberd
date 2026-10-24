//! The open map.

use crate::command::{Change, Command};
use crate::history::{Entry, History};
use crate::{BrushObject, DocError, MapFileData, MapObject, Object, ObjectId};
use glam::Vec3;
use halberd_geom::Aabb;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};

/// Most objects a map may hold. Far beyond what Source can compile; it only
/// stops runaway scripts or damaged files from using all memory.
pub const MAX_OBJECTS: usize = 1_000_000;

/// Hands out a different instance number to every document.
static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);

/// An open map: its objects, the selection, and the undo history.
#[derive(Debug)]
pub struct Document {
    instance: u64,
    objects: BTreeMap<ObjectId, Object>,
    next_id: u64,
    selection: BTreeSet<ObjectId>,
    /// Each brush entity's brushes, worked out from the brushes.
    members: BTreeMap<ObjectId, BTreeSet<ObjectId>>,
    history: History,
    revision: u64,
    selection_revision: u64,
    saved_revision: u64,
    file_data: MapFileData,
    /// Each model's box in its own space, by lower-case name, as far as
    /// models have been read (for picking and framing props).
    model_boxes: HashMap<String, Aabb>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            instance: NEXT_INSTANCE.fetch_add(1, Ordering::Relaxed),
            objects: BTreeMap::new(),
            next_id: 0,
            selection: BTreeSet::new(),
            members: BTreeMap::new(),
            history: History::default(),
            revision: 0,
            selection_revision: 0,
            saved_revision: 0,
            file_data: MapFileData::default(),
            model_boxes: HashMap::new(),
        }
    }
}

impl Clone for Document {
    /// A copy is a separate document, with its own instance number.
    fn clone(&self) -> Self {
        Self {
            instance: NEXT_INSTANCE.fetch_add(1, Ordering::Relaxed),
            objects: self.objects.clone(),
            next_id: self.next_id,
            selection: self.selection.clone(),
            members: self.members.clone(),
            history: self.history.clone(),
            revision: self.revision,
            selection_revision: self.selection_revision,
            saved_revision: self.saved_revision,
            file_data: self.file_data.clone(),
            model_boxes: self.model_boxes.clone(),
        }
    }
}

impl Document {
    /// An empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// A map read from a file: these objects (given ids in order, each
    /// entity before its brushes), and the rest of the file's contents.
    /// Nothing is selected, there is nothing to undo, and it counts as
    /// saved. Objects beyond [`MAX_OBJECTS`] (brushes included) are refused.
    pub fn from_map(objects: Vec<MapObject>, file_data: MapFileData) -> Result<Self, DocError> {
        let count: usize = objects
            .iter()
            .map(|o| match o {
                MapObject::Brush(_) | MapObject::Shape(_) => 1,
                MapObject::Entity(_, brushes) => 1 + brushes.len(),
            })
            .sum();
        if count > MAX_OBJECTS {
            return Err(DocError::TooManyObjects);
        }
        let mut doc = Self {
            file_data,
            ..Self::default()
        };
        for object in objects {
            match object {
                MapObject::Brush(brush) => {
                    let id = doc.new_id();
                    doc.insert(id, Object::Brush(brush));
                }
                MapObject::Shape(shape) => {
                    let id = doc.new_id();
                    doc.insert(id, Object::Shape(shape));
                }
                MapObject::Entity(entity, brushes) => {
                    let owner = doc.new_id();
                    doc.insert(owner, Object::Entity(entity));
                    for mut brush in brushes {
                        brush.set_entity(Some(owner));
                        let id = doc.new_id();
                        doc.insert(id, Object::Brush(brush));
                    }
                }
            }
        }
        Ok(doc)
    }

    /// What the map's file held besides its objects.
    pub fn file_data(&self) -> &MapFileData {
        &self.file_data
    }

    /// Sets how the map's file text is encoded, as found when it was read.
    pub fn set_encoding(&mut self, encoding: crate::TextEncoding) {
        self.file_data.encoding = encoding;
    }

    /// Records that the map has just been saved.
    pub fn mark_saved(&mut self) {
        self.saved_revision = self.revision;
    }

    /// True if the map changed since it was opened, created or last saved.
    pub fn is_modified(&self) -> bool {
        self.revision != self.saved_revision
    }

    /// A number no other document in this run of the program shares, so
    /// views can tell a replaced map (File → New, Open) from an edited one.
    pub fn instance(&self) -> u64 {
        self.instance
    }

    /// Applies an edit and records it for undo. On error nothing changes.
    pub fn execute(&mut self, command: Command) -> Result<(), DocError> {
        self.record(command, None)
    }

    /// Like [`Self::execute`], but consecutive edits with the same `key`
    /// become one undo step, until [`Self::end_step`] is called. Dragging a
    /// size field sends many small edits; this makes the whole drag undo at
    /// once.
    pub fn execute_merging(&mut self, command: Command, key: u64) -> Result<(), DocError> {
        self.record(command, Some(key))
    }

    /// Throws away an unfinished drag: if the last undo step was made with
    /// `key` and is still open, its change is reversed and forgotten (it
    /// cannot be redone). Returns true if something was reversed. Used when
    /// Escape cancels a gizmo drag.
    pub fn discard_step(&mut self, key: u64) -> bool {
        match self.history.take_open_step(key) {
            Some(entry) => {
                self.apply(entry.change.inverse());
                true
            }
            None => false,
        }
    }

    /// Finishes the current undo step, so the next edit starts a new one.
    /// The interface calls this whenever no drag is in progress.
    pub fn end_step(&mut self) {
        self.history.seal();
    }

    fn record(&mut self, command: Command, merge_key: Option<u64>) -> Result<(), DocError> {
        let label = match &command {
            // Name everything that goes: an entity takes its brushes along.
            Command::Remove(ids) => {
                let all = self.with_dependents(ids.iter().copied().collect());
                Command::Remove(all.into_iter().collect()).describe()
            }
            other => other.describe(),
        };
        let change = self.prepare(command)?;
        let change = self.apply(change);
        self.history.record(Entry {
            label,
            change,
            merge_key,
        });
        Ok(())
    }

    /// Reverses the last edit. Returns its description, or `None` if there
    /// was nothing to undo.
    pub fn undo(&mut self) -> Option<String> {
        let entry = self.history.take_undo()?;
        let label = entry.label.clone();
        let reversed = self.apply(entry.change.inverse());
        self.history.push_redo(Entry {
            label: entry.label,
            change: reversed.inverse(),
            merge_key: None,
        });
        Some(label)
    }

    /// Repeats the last undone edit. Returns its description, or `None` if
    /// there was nothing to redo.
    pub fn redo(&mut self) -> Option<String> {
        let entry = self.history.take_redo()?;
        let label = entry.label.clone();
        let change = self.apply(entry.change);
        self.history.push_undo(Entry {
            label: entry.label,
            change,
            merge_key: None,
        });
        Some(label)
    }

    /// What Undo would reverse, for the Edit menu.
    pub fn undo_label(&self) -> Option<&str> {
        self.history.undo_label()
    }

    /// What Redo would repeat, for the Edit menu.
    pub fn redo_label(&self) -> Option<&str> {
        self.history.redo_label()
    }

    /// Every object, in the order they were created.
    pub fn objects(&self) -> impl Iterator<Item = (ObjectId, &Object)> {
        self.objects.iter().map(|(id, object)| (*id, object))
    }

    /// One object, if it is in the map.
    pub fn get(&self, id: ObjectId) -> Option<&Object> {
        self.objects.get(&id)
    }

    /// The brushes of a brush entity, in file order (none for a point
    /// entity or a brush).
    pub fn brushes_of(&self, entity: ObjectId) -> impl Iterator<Item = ObjectId> + '_ {
        self.members.get(&entity).into_iter().flatten().copied()
    }

    /// True if `entity` is a brush entity with brushes in the map.
    pub fn has_brushes(&self, entity: ObjectId) -> bool {
        self.members.contains_key(&entity)
    }

    /// What clicking `id` selects: a brush entity's brush selects the whole
    /// entity, unless `inside_entities` is on (Hammer's "Ignore groups").
    pub fn selectable(&self, id: ObjectId, inside_entities: bool) -> ObjectId {
        if inside_entities {
            return id;
        }
        self.objects
            .get(&id)
            .and_then(Object::as_brush)
            .and_then(BrushObject::entity)
            .unwrap_or(id)
    }

    /// True if `id` is selected, or belongs to a selected brush entity.
    pub fn is_shown_selected(&self, id: ObjectId) -> bool {
        self.selection.contains(&id)
            || self
                .objects
                .get(&id)
                .and_then(Object::as_brush)
                .and_then(BrushObject::entity)
                .is_some_and(|e| self.selection.contains(&e))
    }

    /// The box around an object: a brush's shape, a point entity's
    /// marker, or a brush entity's brushes.
    pub fn bounds_of(&self, id: ObjectId) -> Option<Aabb> {
        let object = self.objects.get(&id)?;
        if self.has_brushes(id) {
            return self
                .brushes_of(id)
                .filter_map(|b| self.objects.get(&b)?.bounds())
                .reduce(Aabb::union);
        }
        self.object_box(object)
    }

    /// How many objects the map holds.
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// True if the map holds no objects.
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Goes up by one whenever an object is added, removed or changed, so
    /// views know when to rebuild what they draw.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Goes up by one whenever the selection changes.
    pub fn selection_revision(&self) -> u64 {
        self.selection_revision
    }

    /// The selected objects.
    pub fn selection(&self) -> &BTreeSet<ObjectId> {
        &self.selection
    }

    /// True if the object is selected.
    pub fn is_selected(&self, id: ObjectId) -> bool {
        self.selection.contains(&id)
    }

    /// Selects exactly these objects. Ids not in the map are ignored.
    pub fn set_selection(&mut self, ids: impl IntoIterator<Item = ObjectId>) {
        let selection: BTreeSet<ObjectId> = ids
            .into_iter()
            .filter(|id| self.objects.contains_key(id))
            .collect();
        if selection != self.selection {
            self.selection = selection;
            self.selection_revision += 1;
        }
    }

    /// Adds the object to the selection, or takes it out if it was in.
    /// Taking out a brush that is selected through its entity selects the
    /// entity's other brushes instead, so the click visibly removes it.
    pub fn toggle_selected(&mut self, id: ObjectId) {
        if !self.objects.contains_key(&id) {
            return;
        }
        let owner = self.selectable(id, false);
        if owner != id && self.selection.contains(&owner) && !self.selection.contains(&id) {
            self.selection.remove(&owner);
            let others: Vec<ObjectId> = self.brushes_of(owner).filter(|b| *b != id).collect();
            self.selection.extend(others);
            self.selection_revision += 1;
            return;
        }
        if !self.selection.remove(&id) {
            self.selection.insert(id);
        }
        self.selection_revision += 1;
    }

    /// Selects nothing.
    pub fn clear_selection(&mut self) {
        self.set_selection([]);
    }

    /// The box around everything selected, or `None` if nothing is.
    pub fn selection_bounds(&self) -> Option<Aabb> {
        self.selection
            .iter()
            .filter_map(|id| self.bounds_of(*id))
            .reduce(Aabb::union)
    }

    /// The nearest object a ray (from `origin` along unit `direction`) hits,
    /// with the distance to it: a brush (world or entity brush) or a point
    /// entity's marker. Use [`Self::selectable`] to turn an entity's brush
    /// into the entity.
    pub fn pick(&self, origin: Vec3, direction: Vec3) -> Option<(ObjectId, f32)> {
        self.objects
            .iter()
            // A brush entity is hit through its brushes, not its origin.
            .filter(|(id, _)| !self.has_brushes(**id))
            .filter_map(|(id, object)| Some((*id, self.object_hit(object, origin, direction)?)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// Checks a command and turns it into the change it would make, without
    /// changing anything.
    fn prepare(&mut self, command: Command) -> Result<Change, DocError> {
        match command {
            Command::AddBrushes(brushes) => {
                if brushes.is_empty() {
                    return Err(DocError::NothingToDo);
                }
                if self.objects.len() + brushes.len() > MAX_OBJECTS {
                    return Err(DocError::TooManyObjects);
                }
                let objects = brushes
                    .into_iter()
                    .map(|brush| (self.new_id(), Object::Brush(BrushObject::new(brush))))
                    .collect();
                Ok(Change::Inserted(objects))
            }
            Command::Remove(ids) => {
                let ids = self.with_dependents(ids.into_iter().collect());
                if ids.is_empty() {
                    return Err(DocError::NothingToDo);
                }
                let objects = ids
                    .into_iter()
                    .map(|id| {
                        let object = self.objects.get(&id).ok_or(DocError::UnknownObject(id))?;
                        Ok((id, object.clone()))
                    })
                    .collect::<Result<_, _>>()?;
                Ok(Change::Removed(objects))
            }
            Command::ReplaceBrush { id, brush } => {
                let before = self.objects.get(&id).ok_or(DocError::UnknownObject(id))?;
                let shaped = before.as_brush().ok_or(DocError::NotABrush(id))?;
                let after = Object::Brush(shaped.with_shape(brush));
                if *before == after {
                    return Err(DocError::NothingToDo);
                }
                Ok(Change::Modified(vec![(id, before.clone(), after)]))
            }
            Command::TransformBrushes {
                brushes,
                shapes,
                entities,
                ..
            } => {
                // Every brush is kept, changed or not, so the edits of one
                // drag always list the same objects and merge into one step.
                let mut seen = BTreeSet::new();
                let mut objects = Vec::with_capacity(brushes.len());
                for (id, brush) in brushes {
                    if !seen.insert(id) {
                        continue;
                    }
                    let before = self.objects.get(&id).ok_or(DocError::UnknownObject(id))?;
                    let shaped = before.as_brush().ok_or(DocError::NotABrush(id))?;
                    objects.push((id, before.clone(), Object::Brush(shaped.with_shape(brush))));
                }
                for (id, shape) in shapes {
                    if !seen.insert(id) {
                        continue;
                    }
                    let before = self.objects.get(&id).ok_or(DocError::UnknownObject(id))?;
                    before.as_shape().ok_or(DocError::NotABrush(id))?;
                    objects.push((id, before.clone(), Object::Shape(shape)));
                }
                for (id, entity) in entities {
                    if !seen.insert(id) {
                        continue;
                    }
                    let before = self.objects.get(&id).ok_or(DocError::UnknownObject(id))?;
                    if !matches!(before, Object::Entity(_)) {
                        return Err(DocError::NotAnEntity(id));
                    }
                    objects.push((id, before.clone(), Object::Entity(entity)));
                }
                if objects.iter().all(|(_, before, after)| before == after) {
                    return Err(DocError::NothingToDo);
                }
                Ok(Change::Modified(objects))
            }
            Command::AddBrushObjects(brushes) => {
                if brushes.is_empty() {
                    return Err(DocError::NothingToDo);
                }
                if self.objects.len() + brushes.len() > MAX_OBJECTS {
                    return Err(DocError::TooManyObjects);
                }
                let objects = brushes
                    .into_iter()
                    .map(|brush| (self.new_id(), Object::Brush(brush.as_new())))
                    .collect();
                Ok(Change::Inserted(objects))
            }
            Command::SetMaterial { material, faces } => self.prepare_set_material(&material, faces),
            Command::AddEntity(entity) => {
                if self.objects.len() >= MAX_OBJECTS {
                    return Err(DocError::TooManyObjects);
                }
                if entity.origin.is_none() {
                    return Err(DocError::NothingToDo);
                }
                Ok(Change::Inserted(vec![(self.new_id(), Object::Entity(entity))]))
            }
            Command::AddShape(shape) => {
                if self.objects.len() >= MAX_OBJECTS {
                    return Err(DocError::TooManyObjects);
                }
                Ok(Change::Inserted(vec![(
                    self.new_id(),
                    Object::Shape(shape),
                )]))
            }
            Command::ShapeEdit {
                changed,
                added,
                removed,
                select_added,
                ..
            } => self.prepare_shape_edit(changed, added, removed, select_added),
            Command::EditBrushes { brushes, .. } => {
                // As above: every brush is kept so a drag merges into one step.
                let mut seen = BTreeSet::new();
                let mut objects = Vec::with_capacity(brushes.len());
                for (id, brush, fates) in brushes {
                    if !seen.insert(id) {
                        continue;
                    }
                    let before = self.objects.get(&id).ok_or(DocError::UnknownObject(id))?;
                    let shaped = before.as_brush().ok_or(DocError::NotABrush(id))?;
                    let after = Object::Brush(shaped.with_shape_mapped(brush, &fates));
                    objects.push((id, before.clone(), after));
                }
                if objects.iter().all(|(_, before, after)| before == after) {
                    return Err(DocError::NothingToDo);
                }
                Ok(Change::Modified(objects))
            }
        }
    }

    /// Carries out a checked change and returns it (for the history).
    fn apply(&mut self, change: Change) -> Change {
        self.apply_part(&change, false);
        self.revision += 1;
        self.selection_revision += 1;
        change
    }

    /// Carries out one part of a change. Objects coming in become the
    /// selection, or (`adding`, inside a change of several parts) join it.
    fn apply_part(&mut self, change: &Change, adding: bool) {
        match change {
            Change::Inserted(objects) => {
                for (id, object) in objects {
                    self.insert(*id, object.clone());
                }
                if objects.is_empty() {
                    return;
                }
                // Select what came in, as one would pick it: an entity's
                // brushes come in with it and are selected through it.
                let came_in: BTreeSet<ObjectId> = objects
                    .iter()
                    .map(|(id, _)| *id)
                    .filter(|id| {
                        let owner = self.selectable(*id, false);
                        owner == *id || !objects.iter().any(|(other, _)| *other == owner)
                    })
                    .collect();
                if adding {
                    self.selection.extend(came_in);
                } else {
                    self.selection = came_in;
                }
            }
            Change::Removed(objects) => {
                for (id, _) in objects {
                    self.remove(*id);
                    self.selection.remove(id);
                }
            }
            Change::Modified(objects) => {
                for (id, _, after) in objects {
                    self.objects.insert(*id, after.clone());
                }
            }
            Change::Several {
                parts,
                select_added,
            } => {
                let selection = self.selection.clone();
                for part in parts {
                    self.apply_part(part, true);
                }
                if !select_added {
                    self.selection = selection
                        .into_iter()
                        .filter(|id| self.objects.contains_key(id))
                        .collect();
                }
            }
        }
    }

    /// Checks a [`Command::ShapeEdit`]: changed brushes keep their entity,
    /// added brushes join theirs, and removed objects take along what must
    /// go with them (but never what is changed, or an entity something is
    /// added to).
    fn prepare_shape_edit(
        &mut self,
        changed: Vec<(ObjectId, BrushObject)>,
        added: Vec<(BrushObject, Option<ObjectId>)>,
        removed: Vec<ObjectId>,
        select_added: bool,
    ) -> Result<Change, DocError> {
        if changed.is_empty() && added.is_empty() && removed.is_empty() {
            return Err(DocError::NothingToDo);
        }
        if self.objects.len() + added.len() > MAX_OBJECTS {
            return Err(DocError::TooManyObjects);
        }
        let mut modified = Vec::with_capacity(changed.len());
        for (id, mut brush) in changed {
            let before = self.objects.get(&id).ok_or(DocError::UnknownObject(id))?;
            let old = before.as_brush().ok_or(DocError::NotABrush(id))?;
            brush.set_entity(old.entity());
            modified.push((id, before.clone(), Object::Brush(brush)));
        }
        let mut inserted = Vec::with_capacity(added.len());
        for (mut brush, entity) in added {
            if let Some(entity) = entity {
                match self.objects.get(&entity) {
                    Some(Object::Entity(_)) => {}
                    Some(Object::Brush(_) | Object::Shape(_)) => {
                        return Err(DocError::NotAnEntity(entity));
                    }
                    None => return Err(DocError::UnknownObject(entity)),
                }
            }
            brush.set_entity(entity);
            inserted.push((self.new_id(), Object::Brush(brush)));
        }
        let keep: BTreeSet<ObjectId> = modified
            .iter()
            .map(|(id, ..)| *id)
            .chain(inserted.iter().filter_map(|(_, o)| o.as_brush()?.entity()))
            .collect();
        let mut gone = Vec::new();
        for id in self.with_dependents(removed.into_iter().collect()) {
            if keep.contains(&id) {
                continue;
            }
            let object = self.objects.get(&id).ok_or(DocError::UnknownObject(id))?;
            gone.push((id, object.clone()));
        }
        Ok(Change::Several {
            parts: vec![
                Change::Modified(modified),
                Change::Removed(gone),
                Change::Inserted(inserted),
            ],
            select_added,
        })
    }

    /// Puts an object in the map, keeping the entity index up to date.
    fn insert(&mut self, id: ObjectId, object: Object) {
        if let Some(entity) = object.as_brush().and_then(BrushObject::entity) {
            self.members.entry(entity).or_default().insert(id);
        }
        self.objects.insert(id, object);
    }

    /// Takes an object out of the map, keeping the entity index up to date.
    fn remove(&mut self, id: ObjectId) {
        let Some(object) = self.objects.remove(&id) else {
            return;
        };
        if let Some(entity) = object.as_brush().and_then(BrushObject::entity)
            && let Some(members) = self.members.get_mut(&entity)
        {
            members.remove(&id);
            if members.is_empty() {
                self.members.remove(&entity);
            }
        }
    }

    /// `ids` plus what must go with them: a brush entity's brushes go with
    /// it, and a brush entity goes when all its brushes do (a brush entity
    /// without brushes is not valid in a map file; Hammer does the same),
    /// unless it still holds brushes Halberd could not show, which must
    /// not be lost.
    fn with_dependents(&self, ids: BTreeSet<ObjectId>) -> BTreeSet<ObjectId> {
        let mut all = ids.clone();
        for id in &ids {
            all.extend(self.brushes_of(*id));
        }
        let emptied: Vec<ObjectId> = self
            .members
            .iter()
            .filter(|(_, brushes)| brushes.iter().all(|b| all.contains(b)))
            .map(|(entity, _)| *entity)
            .filter(|entity| {
                !self
                    .objects
                    .get(entity)
                    .and_then(Object::as_entity)
                    .is_some_and(crate::EntityObject::has_kept_solids)
            })
            .collect();
        all.extend(emptied);
        all
    }

    fn new_id(&mut self) -> ObjectId {
        self.next_id += 1;
        ObjectId(self.next_id)
    }
}

mod entities;
#[cfg(test)]
mod map_tests;
mod materials;
#[cfg(test)]
mod shape_tests;
#[cfg(test)]
mod tests;
