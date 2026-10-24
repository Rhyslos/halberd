//! Outputs and inputs across the map: changing an entity's connections,
//! finding entities by name, and what is connected to an entity.

use super::{Change, DocError, Document};
use crate::connection::{Connection, MAX_CONNECTIONS, target_matches};
use crate::{Object, ObjectId};
use std::collections::{HashMap, HashSet};

impl Change {
    /// True if this change may alter entities' names or connections: an
    /// entity comes or goes, or one changes its name or connections.
    pub(crate) fn touches_logic(&self) -> bool {
        let entity = |o: &Object| o.as_entity().is_some();
        match self {
            Change::Inserted(objects) | Change::Removed(objects) => {
                objects.iter().any(|(_, o)| entity(o))
            }
            Change::Modified(objects) => objects.iter().any(|(_, before, after)| {
                match (before.as_entity(), after.as_entity()) {
                    (Some(b), Some(a)) => {
                        b.name() != a.name() || connections_block(b) != connections_block(a)
                    }
                    (None, None) => false,
                    _ => true,
                }
            }),
            Change::World { .. } => false,
            Change::Several { parts, .. } => parts.iter().any(Change::touches_logic),
        }
    }
}

/// An entity's connections blocks, as written.
fn connections_block(e: &crate::EntityObject) -> Vec<&halberd_kv::Entry> {
    e.file_data
        .iter()
        .filter(|en| matches!(en, halberd_kv::Entry::Block(b) if b.name.eq_ignore_ascii_case("connections")))
        .collect()
}

impl Document {
    /// Goes up whenever an entity's name or connections may have changed
    /// (not when entities only move), so connection lists and lines need
    /// working out again only then.
    pub fn logic_revision(&self) -> u64 {
        self.logic_revision
    }

    /// Checks [`crate::Command::SetConnections`].
    pub(super) fn prepare_connections(
        &self,
        id: ObjectId,
        connections: &[Connection],
    ) -> Result<Change, DocError> {
        let before = self.objects.get(&id).ok_or(DocError::UnknownObject(id))?;
        let entity = before.as_entity().ok_or(DocError::NotAnEntity(id))?;
        if connections.len() > MAX_CONNECTIONS {
            return Err(DocError::TooManyConnections);
        }
        let after = entity.with_connections(connections);
        if after == *entity {
            return Err(DocError::NothingToDo);
        }
        Ok(Change::Modified(vec![(
            id,
            before.clone(),
            Object::Entity(after),
        )]))
    }

    /// The entities an output's `target` reaches (by name; a target ending
    /// in `*` reaches every name starting with the rest).
    pub fn targets_of<'a>(&'a self, target: &'a str) -> impl Iterator<Item = ObjectId> + 'a {
        self.objects.iter().filter_map(move |(id, object)| {
            let name = object.as_entity()?.name()?;
            target_matches(target, name).then_some(*id)
        })
    }

    /// The connections in the map that reach entity `id`: each with the
    /// entity it comes from.
    pub fn incoming(&self, id: ObjectId) -> Vec<(ObjectId, Connection)> {
        let Some(name) = self
            .objects
            .get(&id)
            .and_then(Object::as_entity)
            .and_then(|e| e.name())
        else {
            return Vec::new();
        };
        self.objects
            .iter()
            .filter_map(|(from, object)| Some((*from, object.as_entity()?)))
            .flat_map(|(from, entity)| {
                entity
                    .connections()
                    .into_iter()
                    .filter(|c| target_matches(&c.target, name))
                    .map(move |c| (from, c))
            })
            .collect()
    }

    /// Every pair of entities joined by a connection (from, to), each
    /// pair once, for drawing them. Names are looked up once, so a big map
    /// costs one pass (wildcard targets still look at every name).
    pub fn connection_links(&self) -> Vec<(ObjectId, ObjectId)> {
        let mut named: HashMap<String, Vec<ObjectId>> = HashMap::new();
        for (id, object) in &self.objects {
            if let Some(name) = object.as_entity().and_then(|e| e.name()) {
                named
                    .entry(name.to_ascii_lowercase())
                    .or_default()
                    .push(*id);
            }
        }
        let mut links = Vec::new();
        let mut seen = HashSet::new();
        for (from, object) in &self.objects {
            let Some(entity) = object.as_entity() else {
                continue;
            };
            for connection in entity.connections() {
                let target = connection.target.trim().to_ascii_lowercase();
                let mut reached: Vec<ObjectId> = if target.ends_with('*') {
                    named
                        .iter()
                        .filter(|(name, _)| target_matches(&target, name))
                        .flat_map(|(_, ids)| ids.iter().copied())
                        .collect()
                } else {
                    named.get(&target).cloned().unwrap_or_default()
                };
                reached.sort();
                for to in reached {
                    if seen.insert((*from, to)) {
                        links.push((*from, to));
                    }
                }
            }
        }
        links
    }
}

#[cfg(test)]
mod tests {
    use crate::{Command, Connection, Document, EntityObject};
    use glam::Vec3;

    fn add(doc: &mut Document, class: &str, name: &str) -> crate::ObjectId {
        doc.execute(Command::AddEntity(EntityObject::point(
            class,
            Vec3::ZERO,
            &[("targetname", name)],
        )))
        .unwrap();
        *doc.selection().iter().next().unwrap()
    }

    #[test]
    fn connections_are_set_found_and_undone() {
        let mut doc = Document::new();
        let button = add(&mut doc, "func_button", "b1");
        let door = add(&mut doc, "func_door", "door1");
        let other_door = add(&mut doc, "func_door", "door2");
        doc.execute(Command::SetConnections {
            id: button,
            connections: vec![
                Connection::new("OnPressed", "door*", "Open"),
                Connection::new("OnPressed", "nobody", "Kill"),
            ],
        })
        .unwrap();
        assert_eq!(doc.targets_of("DOOR1").collect::<Vec<_>>(), [door]);
        let incoming = doc.incoming(other_door);
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].0, button);
        assert_eq!(
            doc.connection_links(),
            [(button, door), (button, other_door)]
        );
        assert_eq!(doc.undo().as_deref(), Some("Change outputs"));
        assert!(doc.incoming(door).is_empty());
    }

    #[test]
    fn the_logic_revision_moves_only_for_names_and_connections() {
        let mut doc = Document::new();
        let lamp = add(&mut doc, "light", "lamp");
        let before = doc.logic_revision();
        let moved = doc
            .get(lamp)
            .unwrap()
            .as_entity()
            .unwrap()
            .with_origin(Vec3::ONE);
        doc.execute(Command::TransformBrushes {
            kind: crate::TransformKind::Move,
            brushes: vec![],
            shapes: vec![],
            entities: vec![(lamp, moved)],
        })
        .unwrap();
        assert_eq!(doc.logic_revision(), before, "moving isn't a logic change");
        doc.execute(Command::SetEntitySettings {
            id: lamp,
            settings: vec![("targetname".into(), "lamp2".into())],
        })
        .unwrap();
        assert!(doc.logic_revision() > before);
    }
}
