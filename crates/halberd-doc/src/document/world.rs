//! The map's own settings (the world's: its sky and so on), and changing
//! an entity's settings.

use super::{Change, DocError, Document};
use crate::{Object, ObjectId};
use halberd_kv::Entry;

/// Longest setting key or value accepted (Hammer's own limits are lower).
const MAX_SETTING_LENGTH: usize = 1024;

impl Document {
    /// The value of the world setting `key` (any case), if the map has one.
    /// A new map has none until one is set; saving fills in the usual ones.
    pub fn world_setting(&self, key: &str) -> Option<&str> {
        self.file_data.world_header.iter().find_map(|e| match e {
            Entry::Pair(k, v) if k.eq_ignore_ascii_case(key) => Some(v.as_str()),
            _ => None,
        })
    }

    pub(super) fn prepare_world_setting(&self, key: &str, value: &str) -> Result<Change, DocError> {
        let key = key.trim();
        if key.is_empty() || key.len() > MAX_SETTING_LENGTH || value.len() > MAX_SETTING_LENGTH {
            return Err(DocError::NothingToDo);
        }
        let before = self.world_setting(key).map(str::to_string);
        if before.as_deref() == Some(value) {
            return Err(DocError::NothingToDo);
        }
        Ok(Change::World {
            key: key.to_string(),
            before,
            after: Some(value.to_string()),
        })
    }

    /// Sets (or, with `None`, removes) a world setting.
    pub(super) fn set_world_value(&mut self, key: &str, value: Option<&str>) {
        let header = &mut self.file_data.world_header;
        let at = header
            .iter()
            .position(|e| matches!(e, Entry::Pair(k, _) if k.eq_ignore_ascii_case(key)));
        match (at, value) {
            (Some(at), Some(value)) => {
                if let Some(Entry::Pair(_, v)) = header.get_mut(at) {
                    *v = value.to_string();
                }
            }
            (Some(at), None) => {
                header.remove(at);
            }
            (None, Some(value)) => {
                let before_blocks = header
                    .iter()
                    .position(|e| matches!(e, Entry::Block(_)))
                    .unwrap_or(header.len());
                header.insert(
                    before_blocks,
                    Entry::Pair(key.to_string(), value.to_string()),
                );
            }
            (None, None) => {}
        }
    }

    pub(super) fn prepare_entity_settings(
        &self,
        id: ObjectId,
        settings: &[(String, String)],
    ) -> Result<Change, DocError> {
        let before = self.objects.get(&id).ok_or(DocError::UnknownObject(id))?;
        let Object::Entity(entity) = before else {
            return Err(DocError::NotAnEntity(id));
        };
        let mut after = entity.clone();
        for (key, value) in settings {
            let key = key.trim();
            if key.is_empty()
                || key.len() > MAX_SETTING_LENGTH
                || value.len() > MAX_SETTING_LENGTH
                || key.eq_ignore_ascii_case("classname")
            {
                continue;
            }
            // The origin moves the entity too.
            if key.eq_ignore_ascii_case("origin") {
                continue;
            }
            after.set_setting(key, value);
        }
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

#[cfg(test)]
mod tests {
    use crate::{Command, Document, EntityObject};
    use glam::Vec3;

    #[test]
    fn the_sky_is_changed_undone_and_merged_while_choosing() {
        let mut doc = Document::new();
        assert_eq!(doc.world_setting("skyname"), None);
        let sky = |name: &str| Command::SetWorldSetting {
            key: "skyname".into(),
            value: name.into(),
        };
        assert_eq!(sky("x").describe(), "Change sky");
        doc.execute(sky("sky_day01_01")).unwrap();
        assert_eq!(doc.world_setting("SKYNAME"), Some("sky_day01_01"));
        assert!(doc.is_modified());
        doc.execute_merging(sky("sky_day02_01"), 7).unwrap();
        doc.execute_merging(sky("sky_borealis01"), 7).unwrap();
        doc.end_step();
        assert_eq!(doc.world_setting("skyname"), Some("sky_borealis01"));
        assert_eq!(doc.undo().as_deref(), Some("Change sky"));
        assert_eq!(
            doc.world_setting("skyname"),
            Some("sky_day01_01"),
            "one step"
        );
        doc.undo();
        assert_eq!(doc.world_setting("skyname"), None);
        doc.redo();
        assert_eq!(doc.world_setting("skyname"), Some("sky_day01_01"));
        assert!(doc.execute(sky("sky_day01_01")).is_err(), "nothing to do");
    }

    #[test]
    fn entity_settings_change_but_never_their_class_or_origin() {
        let mut doc = Document::new();
        doc.execute(Command::AddEntity(EntityObject::point(
            "light",
            Vec3::ZERO,
            &[("_light", "255 255 255 200")],
        )))
        .unwrap();
        let id = doc.objects().next().unwrap().0;
        doc.execute(Command::SetEntitySettings {
            id,
            settings: vec![
                ("_light".into(), "255 128 0 400".into()),
                ("_quadratic_attn".into(), "1".into()),
                ("classname".into(), "prop_static".into()),
                ("origin".into(), "9 9 9".into()),
            ],
        })
        .unwrap();
        let light = doc.get(id).unwrap().as_entity().unwrap();
        assert_eq!(light.setting("_light"), Some("255 128 0 400"));
        assert_eq!(light.setting("_quadratic_attn"), Some("1"));
        assert_eq!(light.classname, "light");
        assert_eq!(light.setting("classname"), Some("light"));
        assert_eq!(light.origin, Some(Vec3::ZERO));
        doc.undo();
        let light = doc.get(id).unwrap().as_entity().unwrap();
        assert_eq!(light.setting("_light"), Some("255 255 255 200"));
    }
}
