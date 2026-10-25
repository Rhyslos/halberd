use super::*;
use crate::{Aabb, Heading, Shape, ShapeSettings, build_shape};
use glam::Vec3;

fn block(min: Vec3, max: Vec3) -> Brush {
    Brush::cuboid(Aabb::from_corners(min, max)).unwrap()
}

fn total(pieces: &[Brush]) -> f32 {
    pieces.iter().map(Brush::volume).sum()
}

#[test]
fn cutting_a_doorway_out_of_a_wall() {
    let wall = block(Vec3::ZERO, Vec3::new(256.0, 16.0, 128.0));
    let door = block(Vec3::new(96.0, -8.0, 0.0), Vec3::new(160.0, 24.0, 96.0));
    let Subtracted::Pieces(pieces) = wall.subtract(&door) else {
        panic!("the door cuts the wall");
    };
    // Left, right and above the doorway.
    assert_eq!(pieces.len(), 3);
    let expected = wall.volume() - 64.0 * 16.0 * 96.0;
    assert!((total(&pieces) - expected).abs() < 1.0);
    for piece in &pieces {
        assert!(!piece.overlaps(&door));
        for v in piece.points() {
            assert_eq!(v, v.round(), "on whole units");
        }
    }
    // The faces cut by the door say which door face made them.
    let from_door: usize = pieces
        .iter()
        .flat_map(|p| p.faces())
        .filter(|f| f.source() >= wall.faces().len())
        .count();
    // The doorway's sides and top, plus the faces where pieces meet.
    assert!(from_door >= 3, "{from_door}");
}

#[test]
fn untouched_inside_and_touching() {
    let a = block(Vec3::ZERO, Vec3::splat(64.0));
    let far = block(Vec3::splat(100.0), Vec3::splat(164.0));
    assert_eq!(a.subtract(&far), Subtracted::Untouched);
    let touching = block(Vec3::new(64.0, 0.0, 0.0), Vec3::new(128.0, 64.0, 64.0));
    assert_eq!(
        a.subtract(&touching),
        Subtracted::Untouched,
        "sharing a face"
    );
    let around = block(Vec3::splat(-10.0), Vec3::splat(100.0));
    assert_eq!(a.subtract(&around), Subtracted::Gone);
    assert!(!a.overlaps(&touching));
}

#[test]
fn intersections_are_the_common_part() {
    let a = block(Vec3::ZERO, Vec3::splat(64.0));
    let b = block(Vec3::splat(32.0), Vec3::splat(96.0));
    let common = a.intersection(&b).unwrap();
    assert_eq!(
        common.bounds(),
        Aabb::from_corners(Vec3::splat(32.0), Vec3::splat(64.0))
    );
    assert!(common.faces().iter().any(|f| f.source() >= 6));
    let far = block(Vec3::splat(100.0), Vec3::splat(164.0));
    assert!(a.intersection(&far).is_none());
}

#[test]
fn a_cylinder_hole_through_a_block() {
    let block_ = block(Vec3::ZERO, Vec3::new(128.0, 128.0, 32.0));
    let post = build_shape(
        Shape::Cylinder,
        &ShapeSettings::default(),
        Aabb::from_corners(Vec3::new(32.0, 32.0, -16.0), Vec3::new(96.0, 96.0, 48.0)),
        Heading::PosX,
    )
    .unwrap()
    .remove(0);
    let Subtracted::Pieces(pieces) = block_.subtract(&post) else {
        panic!("the post cuts the block");
    };
    let hole = block_.intersection(&post).unwrap().volume();
    assert!((total(&pieces) - (block_.volume() - hole)).abs() < 2.0);
    assert!(pieces.iter().all(|p| !p.overlaps(&post)));
}

#[test]
fn random_subtractions_keep_the_volume_right() {
    let mut seed: u64 = 0xB001_EA45_0000_0001;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % 129) as f32
    };
    for _ in 0..300 {
        let a = block(
            Vec3::ZERO,
            Vec3::new(64.0 + next(), 64.0 + next(), 64.0 + next()),
        );
        let min = Vec3::new(next(), next(), next()) - Vec3::splat(32.0);
        let b = block(
            min,
            min + Vec3::new(8.0 + next(), 8.0 + next(), 8.0 + next()),
        );
        let common = a.intersection(&b).map_or(0.0, |c| c.volume());
        let left = match a.subtract(&b) {
            Subtracted::Untouched => a.volume(),
            Subtracted::Gone => 0.0,
            Subtracted::Pieces(pieces) => {
                for p in &pieces {
                    assert!(!p.overlaps(&b));
                }
                total(&pieces)
            }
        };
        assert!(
            (left + common - a.volume()).abs() < 2.0,
            "{left} + {common} vs {}",
            a.volume()
        );
    }
}
