//! Brush entities and entity classes: tying brushes to a new brush entity
//! (Hammer's "Tie to entity"), moving them back to the world, changing an
//! entity's class, and removing settings.

use super::{Change, DocError, Document, MAX_OBJECTS};
use crate::{EntityObject, Object, ObjectId};
use halberd_kv::Entry;
use std::collections::BTreeSet;

/// Longest class name accepted.
const MAX_CLASSNAME: usize = 128;

impl Document {
    /// The brushes `objects` stand for: brushes themselves, and the
    /// brushes of brush entities. Point entities and shapes are skipped.
    fn brushes_among(&self, objects: &[ObjectId]) -> BTreeSet<ObjectId> {
        let mut out = BTreeSet::new();
        for id in objects {
            match self.objects.get(id) {
                Some(Object::Brush(_)) => {
                    out.insert(*id);
                }
                Some(Object::Entity(_)) => out.extend(self.brushes_of(*id)),
                _ => {}
            }
        }
        out
    }

    /// The brush entities that would be left with no brushes once
    /// `leaving` move out (and hold no brushes Halberd keeps unseen).
    fn emptied_by(
        &self,
        leaving: &BTreeSet<ObjectId>,
    ) -> Result<Vec<(ObjectId, Object)>, DocError> {
        let owners: BTreeSet<ObjectId> = leaving
            .iter()
            .filter_map(|id| self.objects.get(id)?.as_brush()?.entity())
            .collect();
        let mut gone = Vec::new();
        for owner in owners {
            let all_leave = self.brushes_of(owner).all(|b| leaving.contains(&b));
            let object = self
                .objects
                .get(&owner)
                .ok_or(DocError::UnknownObject(owner))?;
            let keeps = object
                .as_entity()
                .is_some_and(EntityObject::has_kept_solids);
            if all_leave && !keeps {
                gone.push((owner, object.clone()));
            }
        }
        Ok(gone)
    }

    /// Checks [`crate::Command::TieToEntity`].
    pub(super) fn prepare_tie(
        &mut self,
        objects: &[ObjectId],
        entity: EntityObject,
    ) -> Result<Change, DocError> {
        let brushes = self.brushes_among(objects);
        if brushes.is_empty() || !valid_classname(&entity.classname) {
            return Err(DocError::NothingToDo);
        }
        if self.objects.len() >= MAX_OBJECTS {
            return Err(DocError::TooManyObjects);
        }
        let gone = self.emptied_by(&brushes)?;
        let id = self.new_id();
        let modified = self.reparented(&brushes, Some(id))?;
        Ok(Change::Several {
            parts: vec![
                Change::Inserted(vec![(id, Object::Entity(entity))]),
                Change::Modified(modified),
                Change::Removed(gone),
            ],
            select_added: false,
        })
    }

    /// Checks [`crate::Command::MoveToWorld`].
    pub(super) fn prepare_move_to_world(&self, objects: &[ObjectId]) -> Result<Change, DocError> {
        let brushes: BTreeSet<ObjectId> = self
            .brushes_among(objects)
            .into_iter()
            .filter(|id| {
                self.objects
                    .get(id)
                    .and_then(Object::as_brush)
                    .is_some_and(|b| b.entity().is_some())
            })
            .collect();
        if brushes.is_empty() {
            return Err(DocError::NothingToDo);
        }
        let gone = self.emptied_by(&brushes)?;
        let modified = self.reparented(&brushes, None)?;
        Ok(Change::Several {
            parts: vec![Change::Modified(modified), Change::Removed(gone)],
            select_added: false,
        })
    }

    /// `brushes`, each moved to `entity` (`None`: the world).
    fn reparented(
        &self,
        brushes: &BTreeSet<ObjectId>,
        entity: Option<ObjectId>,
    ) -> Result<Vec<(ObjectId, Object, Object)>, DocError> {
        brushes
            .iter()
            .map(|id| {
                let before = self.objects.get(id).ok_or(DocError::UnknownObject(*id))?;
                let mut brush = before.as_brush().ok_or(DocError::NotABrush(*id))?.clone();
                brush.set_entity(entity);
                Ok((*id, before.clone(), Object::Brush(brush)))
            })
            .collect()
    }

    /// Checks [`crate::Command::SetEntityClass`].
    pub(super) fn prepare_entity_class(
        &self,
        id: ObjectId,
        classname: &str,
    ) -> Result<Change, DocError> {
        let before = self.objects.get(&id).ok_or(DocError::UnknownObject(id))?;
        let entity = before.as_entity().ok_or(DocError::NotAnEntity(id))?;
        let classname = classname.trim();
        if !valid_classname(classname) || entity.classname == classname {
            return Err(DocError::NothingToDo);
        }
        let mut after = entity.clone();
        after.classname = classname.to_string();
        after.set_setting("classname", classname);
        Ok(Change::Modified(vec![(
            id,
            before.clone(),
            Object::Entity(after),
        )]))
    }

    /// Checks [`crate::Command::RemoveEntitySettings`]. The class and
    /// origin can't be removed.
    pub(super) fn prepare_remove_settings(
        &self,
        id: ObjectId,
        keys: &[String],
    ) -> Result<Change, DocError> {
        let before = self.objects.get(&id).ok_or(DocError::UnknownObject(id))?;
        let entity = before.as_entity().ok_or(DocError::NotAnEntity(id))?;
        let mut after = entity.clone();
        after.file_data.retain(|e| match e {
            Entry::Pair(k, _) => {
                let fixed = k.eq_ignore_ascii_case("classname") || k.eq_ignore_ascii_case("origin");
                fixed || !keys.iter().any(|key| key.eq_ignore_ascii_case(k))
            }
            Entry::Block(_) => true,
        });
        if after == *entity {
            return Err(DocError::NothingToDo);
        }
        Ok(Change::Modified(vec![(
            id,
            before.clone(),
            Object::Entity(after),
        )]))
    }
}

/// A class name a map file can hold: letters, digits and `_`, not too
/// long, and not `worldspawn` (a map has one world, which the editor makes).
fn valid_classname(name: &str) -> bool {
    !name.is_empty()
        && !name.eq_ignore_ascii_case("worldspawn")
        && name.len() <= MAX_CLASSNAME
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use crate::{Command, Document, EntityObject, ObjectId};
    use glam::Vec3;
    use halberd_geom::{Aabb, Brush};

    fn cube(at: f32) -> Brush {
        Brush::cuboid(Aabb::from_corners(Vec3::splat(at), Vec3::splat(at + 64.0))).expect("a cube")
    }

    fn two_brushes() -> (Document, Vec<ObjectId>) {
        let mut doc = Document::new();
        doc.execute(Command::AddBrushes(vec![cube(0.0), cube(128.0)]))
            .unwrap();
        let ids: Vec<ObjectId> = doc.selection().iter().copied().collect();
        (doc, ids)
    }

    fn brush_entity(classname: &str) -> EntityObject {
        EntityObject {
            classname: classname.into(),
            origin: None,
            file_data: vec![halberd_kv::Entry::Pair(
                "classname".into(),
                classname.into(),
            )],
        }
    }

    fn owner(doc: &Document, brush: ObjectId) -> Option<ObjectId> {
        doc.get(brush)?.as_brush()?.entity()
    }

    #[test]
    fn brushes_are_tied_to_a_new_entity_and_back_on_undo() {
        let (mut doc, ids) = two_brushes();
        doc.execute(Command::TieToEntity {
            objects: ids.clone(),
            entity: brush_entity("func_brush"),
        })
        .unwrap();
        let entity = owner(&doc, ids[0]).expect("tied");
        assert_eq!(owner(&doc, ids[1]), Some(entity));
        assert_eq!(doc.brushes_of(entity).count(), 2);
        assert_eq!(
            doc.get(entity).unwrap().as_entity().unwrap().classname,
            "func_brush"
        );
        doc.undo();
        assert_eq!(owner(&doc, ids[0]), None);
        assert!(doc.get(entity).is_none());
        doc.redo();
        assert_eq!(owner(&doc, ids[1]), Some(entity));
    }

    #[test]
    fn retying_every_brush_of_an_entity_replaces_it() {
        let (mut doc, ids) = two_brushes();
        doc.execute(Command::TieToEntity {
            objects: ids.clone(),
            entity: brush_entity("func_detail"),
        })
        .unwrap();
        let old = owner(&doc, ids[0]).unwrap();
        // Selecting the entity stands for its brushes.
        doc.execute(Command::TieToEntity {
            objects: vec![old],
            entity: brush_entity("func_brush"),
        })
        .unwrap();
        assert!(doc.get(old).is_none(), "the emptied entity goes");
        let new = owner(&doc, ids[0]).unwrap();
        assert_ne!(new, old);
        doc.undo();
        assert_eq!(owner(&doc, ids[0]), Some(old));
    }

    #[test]
    fn moving_to_the_world_frees_brushes_and_drops_an_emptied_entity() {
        let (mut doc, ids) = two_brushes();
        doc.execute(Command::TieToEntity {
            objects: ids.clone(),
            entity: brush_entity("func_detail"),
        })
        .unwrap();
        let entity = owner(&doc, ids[0]).unwrap();
        doc.execute(Command::MoveToWorld(vec![ids[0]])).unwrap();
        assert_eq!(owner(&doc, ids[0]), None);
        assert!(doc.get(entity).is_some(), "it still has a brush");
        doc.execute(Command::MoveToWorld(vec![entity])).unwrap();
        assert!(doc.get(entity).is_none());
        assert_eq!(
            doc.execute(Command::MoveToWorld(vec![ids[0]])),
            Err(crate::DocError::NothingToDo)
        );
    }

    #[test]
    fn classes_change_and_settings_go_but_class_and_origin_stay() {
        let mut doc = Document::new();
        doc.execute(Command::AddEntity(EntityObject::point(
            "info_target",
            Vec3::ZERO,
            &[("targetname", "spot"), ("spawnflags", "1")],
        )))
        .unwrap();
        let id = *doc.selection().iter().next().unwrap();
        doc.execute(Command::SetEntityClass {
            id,
            classname: "info_null".into(),
        })
        .unwrap();
        let e = doc.get(id).unwrap().as_entity().unwrap();
        assert_eq!(e.classname, "info_null");
        assert_eq!(e.setting("classname"), Some("info_null"));
        doc.execute(Command::RemoveEntitySettings {
            id,
            keys: vec!["SPAWNFLAGS".into(), "classname".into(), "origin".into()],
        })
        .unwrap();
        let e = doc.get(id).unwrap().as_entity().unwrap();
        assert_eq!(e.setting("spawnflags"), None);
        assert!(e.setting("origin").is_some() && e.setting("classname").is_some());
        for bad in ["bad name; drop", "worldspawn"] {
            assert!(
                doc.execute(Command::SetEntityClass {
                    id,
                    classname: bad.into()
                })
                .is_err()
            );
        }
    }

    #[test]
    fn tying_needs_brushes_and_a_proper_class() {
        let (mut doc, ids) = two_brushes();
        assert!(
            doc.execute(Command::TieToEntity {
                objects: vec![],
                entity: brush_entity("func_brush"),
            })
            .is_err()
        );
        assert!(
            doc.execute(Command::TieToEntity {
                objects: ids,
                entity: brush_entity(""),
            })
            .is_err()
        );
    }
}
