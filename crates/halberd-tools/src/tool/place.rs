//! The Place tool: puts the prop or entity chosen in the Library where
//! you click, standing on the surface under the pointer.

use super::{Tool, ToolAction, ToolController, ToolInput};
use crate::SceneQuery;
use crate::camera::Ray;
use crate::scene::DocumentScene;
use glam::Vec3;
use halberd_doc::{Command, Document, EntityObject};
use halberd_geom::Aabb;

/// What the Place tool puts down: a prop, or another point entity (a
/// spawn point, a light…).
#[derive(Debug, Clone, PartialEq)]
pub struct PlaceChoice {
    /// The entity class (`prop_static`, `info_player_start`, `light`…).
    pub classname: String,
    /// The model, for props (`models/props_c17/oildrum001.mdl`).
    pub model: Option<String>,
    /// Settings the entity is made with, after its class and origin.
    pub settings: Vec<(String, String)>,
    /// Its box around its origin, so it can stand on the surface.
    pub local: Aabb,
}

impl PlaceChoice {
    /// A prop showing `model` (box `local` in its own space). Models made
    /// for static props (`static_prop`) are placed as `prop_static`; others
    /// as `prop_dynamic`, which any model can be.
    pub fn prop(model: &str, local: Aabb, static_prop: bool) -> Self {
        let classname = if static_prop {
            "prop_static"
        } else {
            "prop_dynamic"
        };
        Self {
            classname: classname.to_string(),
            model: Some(model.to_string()),
            settings: vec![
                ("model".into(), model.to_string()),
                ("angles".into(), "0 0 0".into()),
            ],
            local,
        }
    }

    /// A point entity of `classname` with `settings`, shown as the box its
    /// class has ([`halberd_doc::class_box`]).
    pub fn entity(classname: &str, settings: &[(&str, &str)]) -> Self {
        Self {
            classname: classname.to_string(),
            model: None,
            settings: settings
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
            local: halberd_doc::class_box(classname),
        }
    }

    /// A short name for what is placed: the model's file name, or the
    /// class.
    pub fn name(&self) -> &str {
        match &self.model {
            Some(model) => model.rsplit('/').next().unwrap_or(model),
            None => &self.classname,
        }
    }
}

impl ToolController {
    /// What the Place tool puts down, if anything is chosen.
    pub fn placing(&self) -> Option<&PlaceChoice> {
        self.prop.as_ref()
    }

    /// Chooses what to place and switches to the Place tool (`None` goes
    /// back to Select).
    pub fn set_placing(&mut self, prop: Option<PlaceChoice>) {
        let placing = prop.is_some();
        self.prop = prop;
        self.place_at = None;
        self.set_tool(if placing { Tool::Place } else { Tool::Select });
    }

    /// Where the prop would go now: its box, standing where the pointer
    /// is (for the preview).
    pub(super) fn place_preview(&self) -> Option<Aabb> {
        let prop = self.prop.as_ref()?;
        let origin = self.place_at?;
        Some(Aabb::from_corners(
            origin + prop.local.min,
            origin + prop.local.max,
        ))
    }

    pub(super) fn update_place(
        &mut self,
        input: &ToolInput,
        ray: Option<&Ray>,
        doc: &Document,
    ) -> Option<ToolAction> {
        if input.cancel {
            self.set_placing(None);
            return None;
        }
        let prop = self.prop.clone()?;
        self.place_at = ray
            .and_then(|r| DocumentScene::new(doc).pick(r))
            .map(|hit| {
                // On the grid across, standing on the surface.
                let across = self.snap(hit.truncate());
                Vec3::new(across.x, across.y, hit.z - prop.local.min.z)
            });
        if !input.pressed {
            return None;
        }
        let origin = self.place_at?;
        let settings: Vec<(&str, &str)> = prop
            .settings
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let entity = EntityObject::point(&prop.classname, origin, &settings);
        // A map has one sun: placing it ends placing.
        if prop.classname.eq_ignore_ascii_case("light_environment") {
            self.set_placing(None);
        }
        Some(ToolAction::Execute(Command::AddEntity(entity)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Camera;
    use glam::Vec2;
    use halberd_geom::Brush;

    fn barrel() -> PlaceChoice {
        PlaceChoice::prop(
            "models/props_c17/oildrum001.mdl",
            Aabb::from_corners(Vec3::new(-14.0, -14.0, -2.0), Vec3::new(14.0, 14.0, 46.0)),
            true,
        )
    }

    fn click_at(tools: &mut ToolController, doc: &Document, world: Vec3) -> Option<ToolAction> {
        let camera = Camera::looking_at(Vec3::new(1.0, -400.0, 900.0), world);
        let size = Vec2::new(1600.0, 900.0);
        let input = ToolInput {
            size,
            cursor: camera.project(world, size),
            pressed: true,
            held: true,
            ..ToolInput::default()
        };
        tools.update(&input, &camera, doc)
    }

    #[test]
    fn a_click_places_the_prop_standing_on_the_surface() {
        let mut doc = Document::new();
        let floor = Brush::cuboid(Aabb::from_corners(
            Vec3::new(-256.0, -256.0, 0.0),
            Vec3::new(256.0, 256.0, 64.0),
        ))
        .unwrap();
        doc.execute(Command::AddBrushes(vec![floor])).unwrap();
        let mut tools = ToolController::new(16.0);
        tools.set_placing(Some(barrel()));
        assert_eq!(tools.tool(), Tool::Place);
        let action = click_at(&mut tools, &doc, Vec3::new(37.0, 5.0, 64.0));
        let Some(ToolAction::Execute(Command::AddEntity(entity))) = action else {
            panic!("expected a prop, got {action:?}");
        };
        assert_eq!(entity.classname, "prop_static");
        assert_eq!(entity.model(), Some("models/props_c17/oildrum001.mdl"));
        // On the grid across; its bottom (2 below its origin) on the top.
        let origin = entity.origin.unwrap();
        assert!(
            (origin - Vec3::new(32.0, 0.0, 66.0)).length() < 0.01,
            "{origin}"
        );
        let preview = tools.preview().unwrap();
        assert!((preview.min.z - 64.0).abs() < 0.01);
        // It stays in the Place tool for the next; Escape stops.
        assert_eq!(tools.tool(), Tool::Place);
        let cancel = ToolInput {
            cancel: true,
            ..ToolInput::default()
        };
        let camera = Camera::looking_at(Vec3::new(0.0, -1.0, 500.0), Vec3::ZERO);
        assert_eq!(tools.update(&cancel, &camera, &doc), None);
        assert_eq!(tools.tool(), Tool::Select);
        assert!(tools.placing().is_none());
    }

    #[test]
    fn models_not_made_for_static_props_are_placed_as_dynamic() {
        let mut tools = ToolController::new(16.0);
        tools.set_placing(Some(PlaceChoice::prop(
            "models/props_c17/oildrum001.mdl",
            barrel().local,
            false,
        )));
        let action = click_at(&mut tools, &Document::new(), Vec3::ZERO);
        let Some(ToolAction::Execute(Command::AddEntity(entity))) = action else {
            panic!("expected a prop");
        };
        assert_eq!(entity.classname, "prop_dynamic");
        assert_eq!(entity.origin, Some(Vec3::new(0.0, 0.0, 2.0)));
    }
}

#[cfg(test)]
mod entity_tests {
    use super::*;
    use crate::Camera;
    use glam::Vec2;

    #[test]
    fn a_spawn_point_stands_on_the_ground_with_its_settings() {
        let mut tools = ToolController::new(16.0);
        tools.set_placing(Some(PlaceChoice::entity(
            "info_player_start",
            &[("angles", "0 90 0")],
        )));
        assert_eq!(tools.placing().unwrap().name(), "info_player_start");
        let camera = Camera::looking_at(Vec3::new(1.0, -400.0, 900.0), Vec3::ZERO);
        let size = Vec2::new(1600.0, 900.0);
        let input = ToolInput {
            size,
            cursor: camera.project(Vec3::new(40.0, 0.0, 0.0), size),
            pressed: true,
            held: true,
            ..ToolInput::default()
        };
        let Some(ToolAction::Execute(Command::AddEntity(spawn))) =
            tools.update(&input, &camera, &Document::new())
        else {
            panic!("expected a spawn point");
        };
        assert_eq!(spawn.classname, "info_player_start");
        // Its origin is at its feet, on the ground.
        assert_eq!(spawn.origin, Some(Vec3::new(48.0, 0.0, 0.0)));
        assert_eq!(spawn.setting("angles"), Some("0 90 0"));
        assert_eq!(spawn.marker().unwrap().max.z, 72.0);
    }
}

#[cfg(test)]
mod sun_tests {
    use super::*;
    use crate::Camera;
    use glam::Vec2;

    #[test]
    fn placing_the_sun_ends_after_one() {
        // Regression: the Place tool stayed on, so two clicks made two suns.
        let mut tools = ToolController::new(16.0);
        tools.set_placing(Some(PlaceChoice::entity("light_environment", &[])));
        let camera = Camera::looking_at(Vec3::new(1.0, -400.0, 900.0), Vec3::ZERO);
        let size = Vec2::new(1600.0, 900.0);
        let input = ToolInput {
            size,
            cursor: camera.project(Vec3::ZERO, size),
            pressed: true,
            held: true,
            ..ToolInput::default()
        };
        let first = tools.update(&input, &camera, &Document::new());
        assert!(matches!(
            first,
            Some(ToolAction::Execute(Command::AddEntity(_)))
        ));
        assert_eq!(tools.tool(), Tool::Select);
        assert!(tools.placing().is_none());
    }
}
