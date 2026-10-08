//! The size of a Garry's Mod player, for the scale figure in the viewport.
//!
//! These are the standard Half-Life 2 player sizes, which GMod uses.

use glam::Vec3;
use halberd_geom::Aabb;

/// Height of a standing player, in units (about 1.83 m).
pub const PLAYER_HEIGHT: f32 = 72.0;
/// Width and depth of a player, in units.
pub const PLAYER_WIDTH: f32 = 32.0;
/// Height of a standing player's eyes, in units.
pub const PLAYER_EYE_HEIGHT: f32 = 64.0;

/// The space a standing player takes up, with their feet centred on `feet`.
pub fn player_bounds(feet: Vec3) -> Aabb {
    let half = PLAYER_WIDTH * 0.5;
    Aabb::from_corners(
        feet - Vec3::new(half, half, 0.0),
        feet + Vec3::new(half, half, PLAYER_HEIGHT),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_player_stands_on_the_point_given() {
        let b = player_bounds(Vec3::new(100.0, 0.0, 64.0));
        assert_eq!(b.size(), Vec3::new(32.0, 32.0, 72.0));
        assert_eq!(b.min.z, 64.0);
        assert_eq!(b.center().truncate(), glam::Vec2::new(100.0, 0.0));
    }
}
