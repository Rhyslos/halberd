//! What the outputs editor needs to know about the whole map (names,
//! classes, what reaches each entity), worked out once whenever entities'
//! names or connections change, not every frame.

use egui::Ui;
use halberd_doc::{Connection, Document, ObjectId, target_matches};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Most names offered in a target's list.
const MAX_NAMES: usize = 300;

/// The map's names and connections, indexed.
#[derive(Debug, Default)]
pub(super) struct IoIndex {
    /// The document instance and logic revision this was made for.
    key: (u64, u64),
    /// Names, sorted, at most [`MAX_NAMES`], for target lists.
    pub(super) names: Vec<String>,
    /// Entities by lower-case name.
    by_name: HashMap<String, Vec<ObjectId>>,
    /// Lower-case class names in the map.
    classes: HashSet<String>,
    /// The connections reaching each entity, with where each comes from.
    incoming: HashMap<ObjectId, Vec<(ObjectId, Connection)>>,
}

impl IoIndex {
    fn build(doc: &Document) -> Self {
        let mut index = Self {
            key: (doc.instance(), doc.logic_revision()),
            ..Self::default()
        };
        for (id, object) in doc.objects() {
            let Some(entity) = object.as_entity() else {
                continue;
            };
            index.classes.insert(entity.classname.to_ascii_lowercase());
            if let Some(name) = entity.name() {
                index
                    .by_name
                    .entry(name.to_ascii_lowercase())
                    .or_default()
                    .push(id);
            }
        }
        let mut names: Vec<String> = doc
            .objects()
            .filter_map(|(_, o)| Some(o.as_entity()?.name()?.to_string()))
            .collect();
        names.sort_by_key(|n| n.to_ascii_lowercase());
        names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        names.truncate(MAX_NAMES);
        index.names = names;
        for (from, object) in doc.objects() {
            let Some(entity) = object.as_entity() else {
                continue;
            };
            for connection in entity.connections() {
                for to in index.reached(&connection.target) {
                    index
                        .incoming
                        .entry(to)
                        .or_default()
                        .push((from, connection.clone()));
                }
            }
        }
        index
    }

    /// The entities `target` reaches, in id order.
    pub(super) fn reached(&self, target: &str) -> Vec<ObjectId> {
        let target = target.trim();
        let mut ids: Vec<ObjectId> = if target.contains('*') {
            self.by_name
                .iter()
                .filter(|(name, _)| target_matches(target, name))
                .flat_map(|(_, ids)| ids.iter().copied())
                .collect()
        } else {
            self.by_name
                .get(&target.to_ascii_lowercase())
                .cloned()
                .unwrap_or_default()
        };
        ids.sort();
        ids
    }

    /// True if some entity in the map has class `name`.
    pub(super) fn has_class(&self, name: &str) -> bool {
        self.classes.contains(&name.trim().to_ascii_lowercase())
    }

    /// The connections reaching entity `id`.
    pub(super) fn incoming(&self, id: ObjectId) -> &[(ObjectId, Connection)] {
        self.incoming.get(&id).map_or(&[], Vec::as_slice)
    }
}

/// The index for `doc`, made again only when entities' names or
/// connections changed.
pub(super) fn io_index(ui: &Ui, doc: &Document) -> Arc<IoIndex> {
    let id = egui::Id::new("halberd_io_index");
    let key = (doc.instance(), doc.logic_revision());
    let cached: Option<Arc<IoIndex>> = ui.data(|d| d.get_temp(id));
    if let Some(index) = cached.filter(|i| i.key == key) {
        return index;
    }
    let index = Arc::new(IoIndex::build(doc));
    ui.data_mut(|d| d.insert_temp(id, Arc::clone(&index)));
    index
}
