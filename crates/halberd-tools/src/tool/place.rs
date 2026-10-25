//! The Place tool: puts the prop chosen in the browser where you click,
//! standing on the surface under the pointer.

use super::{Tool, ToolAction, ToolController, ToolInput};
use crate::SceneQuery;
use crate::camera::Ray;
use crate::scene::DocumentScene;
use glam::Vec3;
use halberd_doc::{Command, Document, EntityObject};
use halberd_geom::Aabb;

/// A model chosen to place.
#[derive(Debug, Clone, PartialEq)]
pub struct PropChoice {
    /// The model (`models/props_c17/oildrum001.mdl`).
    pub model: String,
    /// Its box in its own space, so it can stand on the surface.
    pub local: Aabb,
    /// Made for `prop_static`; otherwise it is placed as `prop_dynamic`,
    /// which any model can be.
    pub static_prop: bool,
}

impl PropChoice {
    /// The entity class it is placed as.
    pub fn classname(&self) -> &'static str {
        if self.static_prop {
            "prop_static"
        } else {
            "prop_dynamic"
        }
    }
}

impl ToolController {
    /// The prop the Place tool puts down, if one is chosen.
    pub fn prop(&self) -> Option<&PropChoice> {
        self.prop.as_ref()
    }

    /// Chooses the prop to place and switches to the Place tool (`None`
    /// goes back to Select).
    pub fn set_prop(&mut self, prop: Option<PropChoice>) {
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
            self.set_prop(None);
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
        let entity = EntityObject::point(
            prop.classname(),
            origin,
            &[("model", prop.model.as_str()), ("angles", "0 0 0")],
        );
        Some(ToolAction::Execute(Command::AddEntity(entity)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Camera;
    use glam::Vec2;
    use halberd_geom::Brush;

    fn barrel() -> PropChoice {
        PropChoice {
            model: "models/props_c17/oildrum001.mdl".into(),
            local: Aabb::from_corners(Vec3::new(-14.0, -14.0, -2.0), Vec3::new(14.0, 14.0, 46.0)),
            static_prop: true,
        }
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
        tools.set_prop(Some(barrel()));
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
        assert!(tools.prop().is_none());
    }

    #[test]
    fn models_not_made_for_static_props_are_placed_as_dynamic() {
        let mut tools = ToolController::new(16.0);
        tools.set_prop(Some(PropChoice {
            static_prop: false,
            ..barrel()
        }));
        let action = click_at(&mut tools, &Document::new(), Vec3::ZERO);
        let Some(ToolAction::Execute(Command::AddEntity(entity))) = action else {
            panic!("expected a prop");
        };
        assert_eq!(entity.classname, "prop_dynamic");
        assert_eq!(entity.origin, Some(Vec3::new(0.0, 0.0, 2.0)));
    }
}
