//! Does a player fit here? The space above a point is checked against the
//! map's solid brushes: standing (72 units), crouching (36), or not at all,
//! and how much room is left above the player's head.

use crate::player::{PLAYER_HEIGHT, PLAYER_WIDTH};
use glam::Vec3;
use halberd_doc::{Document, Object};
use halberd_geom::{Aabb, Brush};

/// Height of a crouching player, in units.
pub const PLAYER_CROUCH_HEIGHT: f32 = 36.0;
/// Highest step a player walks up without jumping, in units.
pub const PLAYER_STEP_HEIGHT: f32 = 18.0;
/// How far above the head the room is measured, in units.
const HEADROOM_REACH: f32 = 4096.0;
/// Touching a wall or standing on the floor is not being stuck in it.
const TOUCH: f32 = 0.25;

/// Whether a player fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// Standing up.
    Stands,
    /// Only crouching (the ceiling is lower than a player).
    CrouchOnly,
    /// Not even crouching (inside a wall, or a gap too narrow or low).
    Blocked,
}

/// What [`clearance`] found.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clearance {
    /// Whether a player fits.
    pub fit: Fit,
    /// Room between the top of the player (standing, or crouching if only
    /// that fits) and the first thing above, if there is something within
    /// 4096 units.
    pub headroom: Option<f32>,
}

/// Whether a player standing with their feet at `feet` fits, against the
/// map's brushes (world brushes, brush entities and floor plan shapes).
pub fn clearance(doc: &Document, feet: Vec3) -> Clearance {
    let solids = solids(doc, feet);
    let body = |height: f32| player_box(feet, height);
    let fit = if !solids.iter().any(|b| overlaps(b, body(PLAYER_HEIGHT))) {
        Fit::Stands
    } else if !solids
        .iter()
        .any(|b| overlaps(b, body(PLAYER_CROUCH_HEIGHT)))
    {
        Fit::CrouchOnly
    } else {
        Fit::Blocked
    };
    let top = feet.z
        + match fit {
            Fit::CrouchOnly => PLAYER_CROUCH_HEIGHT,
            Fit::Stands | Fit::Blocked => PLAYER_HEIGHT,
        };
    let half = PLAYER_WIDTH * 0.5 - TOUCH;
    let columns = [
        Vec3::ZERO,
        Vec3::new(half, half, 0.0),
        Vec3::new(-half, half, 0.0),
        Vec3::new(half, -half, 0.0),
        Vec3::new(-half, -half, 0.0),
    ];
    let headroom = columns
        .iter()
        .flat_map(|offset| {
            // From just below the head, so a ceiling touching it counts.
            let from = Vec3::new(feet.x, feet.y, top - TOUCH) + *offset;
            solids.iter().filter_map(move |b| b.ray_hit(from, Vec3::Z))
        })
        .map(|d| (d - TOUCH).max(0.0))
        .filter(|d| *d <= HEADROOM_REACH)
        .reduce(f32::min);
    Clearance { fit, headroom }
}

/// The space a player takes, a hair inside so touching doesn't count.
fn player_box(feet: Vec3, height: f32) -> Aabb {
    let half = PLAYER_WIDTH * 0.5 - TOUCH;
    Aabb::from_corners(
        feet + Vec3::new(-half, -half, TOUCH * 2.0),
        feet + Vec3::new(half, half, height),
    )
}

/// Every solid brush near the column above `feet`.
fn solids(doc: &Document, feet: Vec3) -> Vec<Brush> {
    let half = PLAYER_WIDTH * 0.5;
    let column = Aabb::from_corners(
        feet - Vec3::new(half, half, 1.0),
        feet + Vec3::new(half, half, PLAYER_HEIGHT + HEADROOM_REACH),
    );
    let near = |b: &Aabb| {
        b.min.x <= column.max.x
            && b.max.x >= column.min.x
            && b.min.y <= column.max.y
            && b.max.y >= column.min.y
            && b.min.z <= column.max.z
            && b.max.z >= column.min.z
    };
    let mut out = Vec::new();
    for (_, object) in doc.objects() {
        match object {
            Object::Brush(brush) if near(&brush.brush().bounds()) => {
                out.push(brush.brush().clone())
            }
            Object::Shape(shape) => {
                out.extend(shape.pieces().iter().filter(|p| near(&p.bounds())).cloned());
            }
            _ => {}
        }
    }
    out
}

/// True if `brush` and `space` overlap: no face of the brush has the whole
/// box outside it, and their bounds overlap (enough for walls, floors and
/// ceilings; a slanted edge passing a box's corner may count as touching).
fn overlaps(brush: &Brush, space: Aabb) -> bool {
    let b = brush.bounds();
    let bounds_overlap = b.min.x < space.max.x
        && b.max.x > space.min.x
        && b.min.y < space.max.y
        && b.max.y > space.min.y
        && b.min.z < space.max.z
        && b.max.z > space.min.z;
    if !bounds_overlap {
        return false;
    }
    let corners: Vec<Vec3> = (0..8)
        .map(|i| {
            Vec3::new(
                if i & 1 == 0 { space.min.x } else { space.max.x },
                if i & 2 == 0 { space.min.y } else { space.max.y },
                if i & 4 == 0 { space.min.z } else { space.max.z },
            )
        })
        .collect();
    !brush.faces().iter().any(|face| {
        let plane = face.plane();
        corners
            .iter()
            .all(|c| plane.normal.dot(*c) - plane.distance > 0.0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use halberd_doc::Command;

    fn cuboid(min: [f32; 3], max: [f32; 3]) -> Brush {
        Brush::cuboid(Aabb::from_corners(Vec3::from(min), Vec3::from(max))).unwrap()
    }

    /// A floor, and a ceiling `height` above it.
    fn room(height: f32) -> Document {
        let mut doc = Document::new();
        doc.execute(Command::AddBrushes(vec![
            cuboid([-256.0, -256.0, -16.0], [256.0, 256.0, 0.0]),
            cuboid([-256.0, -256.0, height], [256.0, 256.0, height + 16.0]),
        ]))
        .unwrap();
        doc
    }

    #[test]
    fn a_tall_room_fits_a_player_with_room_to_spare() {
        let c = clearance(&room(128.0), Vec3::ZERO);
        assert_eq!(c.fit, Fit::Stands);
        assert_eq!(c.headroom, Some(56.0));
        // Exactly a player's height still fits (touching isn't stuck).
        let c = clearance(&room(72.0), Vec3::ZERO);
        assert_eq!(c.fit, Fit::Stands);
        assert_eq!(c.headroom, Some(0.0));
    }

    #[test]
    fn a_low_ceiling_means_crouching_and_a_lower_one_no_way_through() {
        let c = clearance(&room(48.0), Vec3::ZERO);
        assert_eq!(c.fit, Fit::CrouchOnly);
        assert_eq!(c.headroom, Some(12.0));
        assert_eq!(clearance(&room(30.0), Vec3::ZERO).fit, Fit::Blocked);
    }

    #[test]
    fn a_gap_narrower_than_a_player_is_blocked_and_open_sky_has_no_ceiling() {
        let mut doc = Document::new();
        // Two walls 24 units apart (a player is 32 wide).
        doc.execute(Command::AddBrushes(vec![
            cuboid([-64.0, -64.0, 0.0], [-12.0, 64.0, 128.0]),
            cuboid([12.0, -64.0, 0.0], [64.0, 64.0, 128.0]),
        ]))
        .unwrap();
        assert_eq!(clearance(&doc, Vec3::ZERO).fit, Fit::Blocked);
        let open = clearance(&Document::new(), Vec3::ZERO);
        assert_eq!(
            open,
            Clearance {
                fit: Fit::Stands,
                headroom: None
            }
        );
    }
}
