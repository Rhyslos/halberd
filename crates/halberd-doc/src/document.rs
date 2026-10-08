//! The open map.

use crate::command::{Change, Command};
use crate::history::{Entry, History};
use crate::{DocError, Object, ObjectId};
use glam::Vec3;
use halberd_geom::Aabb;
use std::collections::{BTreeMap, BTreeSet};
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
    history: History,
    revision: u64,
    selection_revision: u64,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            instance: NEXT_INSTANCE.fetch_add(1, Ordering::Relaxed),
            objects: BTreeMap::new(),
            next_id: 0,
            selection: BTreeSet::new(),
            history: History::default(),
            revision: 0,
            selection_revision: 0,
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
            history: self.history.clone(),
            revision: self.revision,
            selection_revision: self.selection_revision,
        }
    }
}

impl Document {
    /// An empty map.
    pub fn new() -> Self {
        Self::default()
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
        let label = command.describe();
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
    pub fn toggle_selected(&mut self, id: ObjectId) {
        if !self.objects.contains_key(&id) {
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
            .filter_map(|id| self.objects.get(id))
            .map(Object::bounds)
            .reduce(Aabb::union)
    }

    /// The nearest object a ray (from `origin` along unit `direction`) hits,
    /// with the distance to it.
    pub fn pick(&self, origin: Vec3, direction: Vec3) -> Option<(ObjectId, f32)> {
        self.objects
            .iter()
            .filter_map(|(id, object)| Some((*id, object.ray_hit(origin, direction)?)))
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
                    .map(|brush| (self.new_id(), Object::Brush(brush)))
                    .collect();
                Ok(Change::Inserted(objects))
            }
            Command::Remove(ids) => {
                let ids: BTreeSet<ObjectId> = ids.into_iter().collect();
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
                let after = Object::Brush(brush);
                if *before == after {
                    return Err(DocError::NothingToDo);
                }
                Ok(Change::Modified(vec![(id, before.clone(), after)]))
            }
            Command::TransformBrushes { brushes, .. } => {
                // Every brush is kept, changed or not, so the edits of one
                // drag always list the same objects and merge into one step.
                let mut seen = BTreeSet::new();
                let mut objects = Vec::with_capacity(brushes.len());
                for (id, brush) in brushes {
                    if !seen.insert(id) {
                        continue;
                    }
                    let before = self.objects.get(&id).ok_or(DocError::UnknownObject(id))?;
                    objects.push((id, before.clone(), Object::Brush(brush)));
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
        match &change {
            Change::Inserted(objects) => {
                for (id, object) in objects {
                    self.objects.insert(*id, object.clone());
                }
                self.selection = objects.iter().map(|(id, _)| *id).collect();
            }
            Change::Removed(objects) => {
                for (id, _) in objects {
                    self.objects.remove(id);
                    self.selection.remove(id);
                }
            }
            Change::Modified(objects) => {
                for (id, _, after) in objects {
                    self.objects.insert(*id, after.clone());
                }
            }
        }
        self.revision += 1;
        self.selection_revision += 1;
        change
    }

    fn new_id(&mut self) -> ObjectId {
        self.next_id += 1;
        ObjectId(self.next_id)
    }
}

#[cfg(test)]
mod tests;
