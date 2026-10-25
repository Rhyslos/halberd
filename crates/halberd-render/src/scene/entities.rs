//! Point entities without a model: a box in a colour for their kind (a
//! player-sized green box for spawn points, yellow for lights, orange for
//! the sun), with a line showing which way it faces when that matters.

use super::{Brush, Looks, POINT_ENTITY_COLOR, SELECTED_EDGE_COLOR, SceneGeometry};
use crate::lines::LineVertex;
use glam::Vec3;
use halberd_doc::EntityObject;

/// Colour of player spawn points.
pub const SPAWN_COLOR: [f32; 4] = [0.12, 0.45, 0.18, 1.0];
/// Colour of lights.
pub const LIGHT_COLOR: [f32; 4] = [0.75, 0.62, 0.12, 1.0];
/// Colour of the sun (`light_environment`).
pub const SUN_COLOR: [f32; 4] = [0.85, 0.42, 0.08, 1.0];
/// Colour of the line showing which way an entity faces.
const FACING_COLOR: [f32; 4] = [0.95, 0.95, 0.95, 1.0];
/// How far that line reaches past the box, in units.
const FACING_LENGTH: f32 = 32.0;

/// The colour an entity of `classname` is shown in.
pub fn entity_color(classname: &str) -> [f32; 4] {
    let class = classname.to_ascii_lowercase();
    if class.starts_with("info_player_") {
        SPAWN_COLOR
    } else if class == "light_environment" || class == "env_sun" {
        SUN_COLOR
    } else if class.starts_with("light") {
        LIGHT_COLOR
    } else {
        POINT_ENTITY_COLOR
    }
}

/// True for entities whose facing matters: spawns face where players
/// look, spotlights and the sun shine that way.
fn shows_facing(classname: &str) -> bool {
    let class = classname.to_ascii_lowercase();
    class.starts_with("info_player_")
        || class == "light_spot"
        || class == "light_environment"
        || class == "env_sun"
}

impl SceneGeometry {
    pub(super) fn add_point_entity(
        &mut self,
        entity: &EntityObject,
        marker: Option<halberd_geom::Aabb>,
        selected: bool,
        layer_of: &mut dyn FnMut(&str) -> Option<u32>,
    ) {
        let Some(marker) = marker else {
            return;
        };
        if let Ok(cube) = Brush::cuboid(marker) {
            let color = entity_color(&entity.classname);
            self.add_brush(&cube, color, selected, Looks::None, layer_of);
        }
        if shows_facing(&entity.classname) {
            let start = marker.center();
            let reach = marker.size().max_element() * 0.5 + FACING_LENGTH;
            let end = start + entity.facing() * Vec3::X * reach;
            let color = if selected {
                SELECTED_EDGE_COLOR
            } else {
                FACING_COLOR
            };
            let lines = if selected {
                &mut self.selected_edges
            } else {
                &mut self.edges
            };
            lines.extend([start, end].map(|p| LineVertex {
                position: p.to_array(),
                color,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halberd_doc::{Command, Document};

    #[test]
    fn spawns_lights_and_the_sun_have_their_own_colour_and_facing() {
        let mut doc = Document::new();
        for (class, angles) in [
            ("info_player_start", "0 90 0"),
            ("light", "0 0 0"),
            ("light_environment", "0 0 0"),
        ] {
            doc.execute(Command::AddEntity(EntityObject::point(
                class,
                Vec3::ZERO,
                &[("angles", angles), ("pitch", "-45")],
            )))
            .unwrap();
        }
        doc.clear_selection();
        let scene = SceneGeometry::from_document(&doc);
        let colors: Vec<[f32; 4]> = scene.faces.iter().map(|v| v.color).collect();
        assert!(colors.contains(&SPAWN_COLOR));
        assert!(colors.contains(&LIGHT_COLOR));
        assert!(colors.contains(&SUN_COLOR));
        let facing: Vec<&LineVertex> = scene
            .edges
            .iter()
            .filter(|v| v.color == FACING_COLOR)
            .collect();
        assert_eq!(
            facing.len(),
            4,
            "the spawn and the sun show which way they face"
        );
        // The spawn faces +Y (yaw 90), from the middle of its body.
        assert!((Vec3::from(facing[0].position) - Vec3::new(0.0, 0.0, 36.0)).length() < 1e-4);
        assert!(facing[1].position[1] > 31.0);
        // The sun points down at 45 degrees.
        let sun = Vec3::from(facing[3].position) - Vec3::from(facing[2].position);
        assert!(sun.z < -20.0 && sun.x > 20.0, "{sun}");
        assert_eq!(entity_color("prop_dynamic"), POINT_ENTITY_COLOR);
    }
}
