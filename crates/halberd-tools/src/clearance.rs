//! Does a player fit here? The space above a point is checked against the
//! map's solid brushes: standing (72 units), crouching (36), or not at all,
//! and how much room is left above the player's head. Players step up to
//! 18 units, so on a slope or a low step the player stands on its highest
//! point within that.

use crate::camera::Ray;
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
/// Surfaces steeper than this (the up part of their normal) can't be stood
/// on; the player stands on the floor below instead.
const WALKABLE: f32 = 0.7;
/// Brush entities players walk through (triggers, illusions, volumes).
const NOT_SOLID: [&str; 11] = [
    "trigger_",
    "func_illusionary",
    "func_areaportal",
    "func_occluder",
    "func_dustcloud",
    "func_smokevolume",
    "func_precipitation",
    "func_buyzone",
    "env_",
    "fog_volume",
    "color_correction_volume",
];

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
    /// Where the player's feet end up: raised onto a slope or step (up to
    /// 18 units) if needed.
    pub feet: Vec3,
}

/// Whether a player standing with their feet at `feet` fits, against the
/// map's solid brushes (world brushes, solid brush entities and floor plan
/// shapes; triggers and the like are walked through).
pub fn clearance(doc: &Document, feet: Vec3) -> Clearance {
    let solids = solids(doc, Some(feet));
    let clear = |at: Vec3, height: f32| !solids.iter().any(|b| overlaps(b, player_box(at, height)));
    // The lowest lift (up to a step) at which the player fits; once
    // raised, the feet rest on what they cleared (the box's bottom sits a
    // hair above the feet).
    let lift_for = |height: f32| {
        let coarse = (0..=(PLAYER_STEP_HEIGHT * 2.0) as u32)
            .map(|half| half as f32 * 0.5)
            .find(|lift| clear(feet + Vec3::Z * *lift, height))?;
        if coarse == 0.0 {
            return Some(feet);
        }
        let (mut low, mut high) = (coarse - 0.5, coarse);
        for _ in 0..12 {
            let mid = (low + high) * 0.5;
            if clear(feet + Vec3::Z * mid, height) {
                high = mid;
            } else {
                low = mid;
            }
        }
        let rest = ((high + TOUCH * 2.0) * 100.0).round() / 100.0;
        Some(feet + Vec3::Z * rest)
    };
    let (fit, at) = if let Some(at) = lift_for(PLAYER_HEIGHT) {
        (Fit::Stands, at)
    } else if let Some(at) = lift_for(PLAYER_CROUCH_HEIGHT) {
        (Fit::CrouchOnly, at)
    } else {
        (Fit::Blocked, feet)
    };
    let top = at.z
        + match fit {
            Fit::CrouchOnly => PLAYER_CROUCH_HEIGHT,
            Fit::Stands | Fit::Blocked => PLAYER_HEIGHT,
        };
    let headroom = solids
        .iter()
        .filter_map(|b| lowest_above(b, at, top))
        .reduce(f32::min);
    Clearance {
        fit,
        headroom,
        feet: at,
    }
}

/// Where a player would stand for the pointer's ray: on the first solid
/// brush it hits, or, if that surface is too steep to stand on (a wall, a
/// ceiling's underside), on the floor below it. `None` if it hits nothing
/// solid.
pub fn floor_under(doc: &Document, ray: &Ray) -> Option<Vec3> {
    let solids = solids(doc, None);
    let (distance, brush) = solids
        .iter()
        .filter_map(|b| Some((b.ray_hit(ray.origin, ray.direction)?, *b)))
        .min_by(|a, b| a.0.total_cmp(&b.0))?;
    let hit = ray.origin + ray.direction * distance;
    let normal = brush
        .faces()
        .iter()
        .map(|f| f.plane())
        .min_by(|a, b| {
            (a.normal.dot(hit) - a.distance)
                .abs()
                .total_cmp(&(b.normal.dot(hit) - b.distance).abs())
        })
        .map_or(Vec3::Z, |p| p.normal);
    if normal.z >= WALKABLE {
        return Some(hit);
    }
    // Step back out of the wall by half a player, then look down.
    let from =
        hit + Vec3::new(normal.x, normal.y, 0.0).normalize_or_zero() * (PLAYER_WIDTH * 0.5 + 1.0);
    solids
        .iter()
        .filter_map(|b| b.ray_hit(from, Vec3::NEG_Z))
        .reduce(f32::min)
        .map(|down| from - Vec3::Z * down)
}

/// The lowest point of `brush` above `top` within the player's footprint
/// at `feet`, as a distance above `top` (within the headroom reach).
fn lowest_above(brush: &Brush, feet: Vec3, top: f32) -> Option<f32> {
    let half = PLAYER_WIDTH * 0.5 - TOUCH;
    let column = |height: f32| {
        Aabb::from_corners(
            Vec3::new(feet.x - half, feet.y - half, top - TOUCH),
            Vec3::new(feet.x + half, feet.y + half, top + height),
        )
    };
    if !overlaps(brush, column(HEADROOM_REACH)) {
        return None;
    }
    // The smallest height of column that reaches it.
    let (mut low, mut high) = (0.0f32, HEADROOM_REACH);
    for _ in 0..24 {
        let mid = (low + high) * 0.5;
        if overlaps(brush, column(mid)) {
            high = mid;
        } else {
            low = mid;
        }
    }
    Some(if high < 0.01 {
        0.0
    } else {
        (high * 100.0).round() / 100.0
    })
}

/// The space a player takes, a hair inside so touching doesn't count.
fn player_box(feet: Vec3, height: f32) -> Aabb {
    let half = PLAYER_WIDTH * 0.5 - TOUCH;
    Aabb::from_corners(
        feet + Vec3::new(-half, -half, TOUCH * 2.0),
        feet + Vec3::new(half, half, height),
    )
}

/// True if `doc` has a solid-to-players owner for a brush owned by
/// `entity` (or none).
fn is_solid(doc: &Document, entity: Option<halberd_doc::ObjectId>) -> bool {
    let Some(entity) = entity else {
        return true;
    };
    let Some(class) = doc.get(entity).and_then(Object::as_entity) else {
        return true;
    };
    let class = class.classname.to_ascii_lowercase();
    !NOT_SOLID.iter().any(|prefix| class.starts_with(prefix))
}

/// Every solid brush, or (with `feet`) those near the column above it.
fn solids(doc: &Document, feet: Option<Vec3>) -> Vec<&Brush> {
    let half = PLAYER_WIDTH * 0.5;
    let column = feet.map(|feet| {
        Aabb::from_corners(
            feet - Vec3::new(half, half, 1.0),
            feet + Vec3::new(
                half,
                half,
                PLAYER_STEP_HEIGHT + PLAYER_HEIGHT + HEADROOM_REACH,
            ),
        )
    });
    let near = |b: &Aabb| {
        column.is_none_or(|c| {
            b.min.x <= c.max.x
                && b.max.x >= c.min.x
                && b.min.y <= c.max.y
                && b.max.y >= c.min.y
                && b.min.z <= c.max.z
                && b.max.z >= c.min.z
        })
    };
    let mut out = Vec::new();
    for (_, object) in doc.objects() {
        match object {
            Object::Brush(brush)
                if near(&brush.brush().bounds()) && is_solid(doc, brush.entity()) =>
            {
                out.push(brush.brush());
            }
            Object::Shape(shape) => {
                out.extend(shape.pieces().iter().filter(|p| near(&p.bounds())));
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
        assert_eq!(open.fit, Fit::Stands);
        assert_eq!(open.headroom, None);
    }

    #[test]
    fn a_player_stands_on_slopes_and_low_steps() {
        // Regression: the box dug into any slope, so ramps said "no room".
        for run in [64.0f32, 128.0, 256.0] {
            let mut doc = Document::new();
            // A ramp rising 16 units over `run` towards +X.
            let plane = |n: Vec3, p: Vec3| halberd_geom::Plane::new(n, p).unwrap();
            let ramp = Brush::from_planes(&[
                plane(
                    Vec3::new(-16.0, 0.0, 2.0 * run).normalize(),
                    Vec3::new(0.0, 0.0, 8.0),
                ),
                plane(Vec3::NEG_Z, Vec3::new(0.0, 0.0, -64.0)),
                plane(Vec3::X, Vec3::new(run, 0.0, 0.0)),
                plane(Vec3::NEG_X, Vec3::new(-run, 0.0, 0.0)),
                plane(Vec3::Y, Vec3::new(0.0, 256.0, 0.0)),
                plane(Vec3::NEG_Y, Vec3::new(0.0, -256.0, 0.0)),
            ]);
            let ramp = ramp.unwrap();
            doc.execute(Command::AddBrushes(vec![ramp])).unwrap();
            let c = clearance(&doc, Vec3::new(0.0, 0.0, 8.0));
            assert_eq!(c.fit, Fit::Stands, "1 in {}", run / 8.0);
            assert!(c.feet.z >= 8.0 && c.feet.z <= 8.0 + PLAYER_STEP_HEIGHT);
        }
        // A 16-unit step under part of the player: stood on.
        let mut doc = Document::new();
        doc.execute(Command::AddBrushes(vec![cuboid(
            [0.0, -64.0, 0.0],
            [64.0, 64.0, 16.0],
        )]))
        .unwrap();
        let c = clearance(&doc, Vec3::ZERO);
        assert_eq!(c.fit, Fit::Stands);
        assert_eq!(c.feet.z, 16.0);
    }

    #[test]
    fn a_thin_beam_overhead_is_found() {
        // Regression: only five rays went up, missing narrow beams.
        let mut doc = Document::new();
        doc.execute(Command::AddBrushes(vec![cuboid(
            [4.0, -64.0, 80.0],
            [12.0, 64.0, 88.0],
        )]))
        .unwrap();
        let c = clearance(&doc, Vec3::ZERO);
        assert_eq!(c.fit, Fit::Stands);
        assert_eq!(c.headroom, Some(8.0));
    }

    #[test]
    fn triggers_are_walked_through() {
        use halberd_doc::{BrushObject, EntityObject, MapObject};
        let trigger = MapObject::Entity(
            EntityObject::point("trigger_multiple", Vec3::ZERO, &[]),
            vec![BrushObject::new(cuboid(
                [-64.0, -64.0, 0.0],
                [64.0, 64.0, 128.0],
            ))],
        );
        let doc = Document::from_map(vec![trigger], Default::default()).unwrap();
        assert_eq!(clearance(&doc, Vec3::ZERO).fit, Fit::Stands);
    }

    #[test]
    fn pointing_at_a_wall_stands_the_player_on_the_floor_beside_it() {
        let mut doc = Document::new();
        doc.execute(Command::AddBrushes(vec![
            cuboid([-512.0, -512.0, -80.0], [512.0, 512.0, -64.0]),
            cuboid([-320.0, -512.0, -64.0], [-300.0, 512.0, 256.0]),
        ]))
        .unwrap();
        // Looking at the wall's +X face from the east.
        let ray = Ray {
            origin: Vec3::new(400.0, 0.0, 100.0),
            direction: Vec3::NEG_X,
        };
        let feet = floor_under(&doc, &ray).unwrap();
        assert!((feet.z + 64.0).abs() < 1e-3, "on the floor below: {feet}");
        assert!(feet.x > -300.0 + 16.0, "clear of the wall: {feet}");
        // A floor below zero is found (the grid plane isn't solid).
        let down = Ray {
            origin: Vec3::new(0.0, 0.0, 500.0),
            direction: Vec3::NEG_Z,
        };
        assert!((floor_under(&doc, &down).unwrap().z + 64.0).abs() < 1e-3);
        assert_eq!(floor_under(&Document::new(), &down), None);
    }

    #[test]
    fn checking_a_big_map_is_quick() {
        // 10,000 brushes on a grid: one check each frame must stay cheap.
        let mut doc = Document::new();
        let brushes: Vec<Brush> = (0..10_000)
            .map(|i| {
                let (x, y) = ((i % 100) as f32 * 64.0, (i / 100) as f32 * 64.0);
                cuboid([x, y, 0.0], [x + 32.0, y + 32.0, 128.0])
            })
            .collect();
        doc.execute(Command::AddBrushes(brushes)).unwrap();
        let started = std::time::Instant::now();
        for i in 0..20 {
            clearance(&doc, Vec3::new(48.0 + i as f32 * 64.0, 48.0, 0.0));
        }
        let each = started.elapsed() / 20;
        assert!(each.as_millis() < 50, "{each:?} per check");
    }
}
