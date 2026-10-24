//! Outputs and inputs across the map: changing an entity's connections,
//! finding entities by name, and what is connected to an entity.

use super::{Change, DocError, Document};
use crate::connection::{Connection, MAX_CONNECTIONS, target_matches};
use crate::{Object, ObjectId};
use std::collections::{HashMap, HashSet};

impl Document {
    /// Checks [`crate::Command::SetConnections`].
    pub(super) fn prepare_connections(
        &self,
        id: ObjectId,
        connections: &[Connection],
    ) -> Result<Change, DocError> {
        let before = self.objects.get(&id).ok_or(DocError::UnknownObject(id))?;
        let entity = before.as_entity().ok_or(DocError::NotAnEntity(id))?;
        if connections.len() > MAX_CONNECTIONS {
            return Err(DocError::NothingToDo);
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
}
