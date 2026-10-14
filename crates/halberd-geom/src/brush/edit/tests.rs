use super::hull;
use super::*;
use crate::{Aabb, Heading, Shape, ShapeSettings, build_shape};

fn cube() -> Brush {
    Brush::cuboid(Aabb::from_corners(Vec3::ZERO, Vec3::splat(64.0))).unwrap()
}

/// The place in `points()` of the corner at `at`.
fn corner(brush: &Brush, at: Vec3) -> usize {
    brush
        .points()
        .iter()
        .position(|p| p.distance(at) < 1e-3)
        .unwrap()
}

/// The face facing most nearly along `normal`.
fn face_towards(brush: &Brush, normal: Vec3) -> usize {
    (0..brush.faces().len())
        .max_by(|&a, &b| {
            let d = |i: usize| brush.faces()[i].plane().normal.dot(normal);
            d(a).total_cmp(&d(b))
        })
        .unwrap()
}

#[test]
fn a_cube_has_eight_corners_twelve_edges_and_six_four_sided_faces() {
    let b = cube();
    assert_eq!(b.points().len(), 8);
    assert_eq!(b.edges().len(), 12);
    let faces = b.face_points();
    assert_eq!(faces.len(), 6);
    assert!(faces.iter().all(|f| f.len() == 4));
    // Each corner is on three faces.
    for i in 0..8 {
        assert_eq!(faces.iter().filter(|f| f.contains(&i)).count(), 3);
    }
}

#[test]
fn moving_nothing_gives_the_same_brush_with_every_face_whole() {
    for shape in [Shape::Box, Shape::Wedge, Shape::Cylinder, Shape::Cone] {
        let b = build_shape(
            shape,
            &ShapeSettings::default(),
            Aabb::from_corners(Vec3::ZERO, Vec3::new(128.0, 96.0, 64.0)),
            Heading::PosX,
        )
        .unwrap()
        .remove(0);
        let (same, fates) = b.with_moved_points(&[]).unwrap();
        assert_eq!(same, b, "{shape:?}: the very same brush");
        let to_itself: Vec<(usize, Vec3)> = b.points().into_iter().enumerate().collect();
        assert_eq!(b.with_moved_points(&to_itself).unwrap().0, b);
        assert_eq!(same.bounds(), b.bounds(), "{shape:?}");
        assert_eq!(same.faces().len(), b.faces().len(), "{shape:?}");
        assert!(fates.iter().all(|f| f.whole), "{shape:?}");
        for face in same.faces() {
            let old = &b.faces()[face.source()];
            assert!(old.plane().normal.dot(face.plane().normal) > 0.999);
        }
    }
}

#[test]
fn moving_a_whole_face_keeps_every_face_whole() {
    let b = cube();
    let top = face_towards(&b, Vec3::Z);
    let up: Vec<(usize, Vec3)> = b.face_points()[top]
        .iter()
        .map(|&i| (i, b.points()[i] + Vec3::new(0.0, 0.0, 32.0)))
        .collect();
    let (taller, fates) = b.with_moved_points(&up).unwrap();
    assert_eq!(taller.bounds().max, Vec3::new(64.0, 64.0, 96.0));
    assert_eq!(taller.faces().len(), 6);
    assert!(fates.iter().all(|f| f.whole));
    let new_top = face_towards(&taller, Vec3::Z);
    assert_eq!(taller.faces()[new_top].source(), top);
}

#[test]
fn moving_one_corner_splits_the_faces_that_bend() {
    let b = cube();
    let lifted = corner(&b, Vec3::splat(64.0));
    let (bent, fates) = b
        .with_moved_points(&[(lifted, Vec3::new(64.0, 64.0, 96.0))])
        .unwrap();
    // The sides stay flat (the corner went straight up); the top folds
    // into two pieces.
    assert_eq!(bent.faces().len(), 7);
    assert_eq!(fates.iter().filter(|f| !f.whole).count(), 2);
    assert_eq!(bent.bounds().max.z, 96.0);
    // Every piece came from an old face facing roughly the same way.
    for face in bent.faces() {
        let old = &b.faces()[face.source()];
        assert!(old.plane().normal.dot(face.plane().normal) > 0.5);
    }
    // The new corner is a real corner of the brush.
    assert!(
        bent.points()
            .iter()
            .any(|p| p.distance(Vec3::new(64.0, 64.0, 96.0)) < 1e-3)
    );
}

#[test]
fn moving_a_corner_sideways_splits_every_face_that_bends() {
    let b = cube();
    let pulled = corner(&b, Vec3::splat(64.0));
    let (bent, fates) = b
        .with_moved_points(&[(pulled, Vec3::new(80.0, 80.0, 96.0))])
        .unwrap();
    // The top and the two sides meeting at the corner each fold in two.
    assert_eq!(bent.faces().len(), 9);
    assert_eq!(fates.iter().filter(|f| !f.whole).count(), 6);
}

#[test]
fn untouched_faces_keep_their_exact_planes() {
    let b = cube();
    let pulled = corner(&b, Vec3::splat(64.0));
    let (bent, _) = b
        .with_moved_points(&[(pulled, Vec3::new(80.0, 80.0, 96.0))])
        .unwrap();
    for old in [Vec3::NEG_X, Vec3::NEG_Y, Vec3::NEG_Z] {
        let before = b.faces()[face_towards(&b, old)].plane();
        assert!(bent.faces().iter().any(|f| f.plane() == before), "{old}");
    }
}

#[test]
fn moving_an_edge_makes_a_ramp() {
    let b = cube();
    let edge = [
        corner(&b, Vec3::new(64.0, 0.0, 64.0)),
        corner(&b, Vec3::new(64.0, 64.0, 64.0)),
    ];
    let moves: Vec<(usize, Vec3)> = edge
        .iter()
        .map(|&i| (i, b.points()[i] - Vec3::new(0.0, 0.0, 63.0)))
        .collect();
    let (ramp, _) = b.with_moved_points(&moves).unwrap();
    assert_eq!(ramp.bounds(), b.bounds());
    assert_eq!(ramp.points().len(), 8);
    assert!(
        ramp.faces()
            .iter()
            .any(|f| f.plane().normal.z > 0.1 && f.plane().normal.x > 0.1),
        "a sloped top"
    );
}

#[test]
fn a_corner_pushed_inside_disappears() {
    let b = cube();
    let pushed = corner(&b, Vec3::splat(64.0));
    let (cut, _) = b.with_moved_points(&[(pushed, Vec3::splat(40.0))]).unwrap();
    assert_eq!(cut.points().len(), 7, "the corner is cut off");
    assert_eq!(cut.faces().len(), 7);
}

#[test]
fn flattening_or_bad_numbers_are_refused() {
    let b = cube();
    let top = face_towards(&b, Vec3::Z);
    let down: Vec<(usize, Vec3)> = b.face_points()[top]
        .iter()
        .map(|&i| (i, b.points()[i] * Vec3::new(1.0, 1.0, 0.0)))
        .collect();
    assert_eq!(
        b.with_moved_points(&down).unwrap_err(),
        GeomError::NotClosed
    );
    assert_eq!(
        b.with_moved_points(&[(0, Vec3::NAN)]).unwrap_err(),
        GeomError::NotFinite
    );
    assert_eq!(
        b.with_moved_points(&[(0, Vec3::splat(500_000.0))])
            .unwrap_err(),
        GeomError::TooLarge
    );
    // A place that is not a corner is ignored.
    assert!(b.with_moved_points(&[(99, Vec3::ZERO)]).is_ok());
}

#[test]
fn a_sphere_corner_moves_quickly() {
    let sphere = build_shape(
        Shape::Sphere,
        &ShapeSettings {
            sides: 16,
            ..ShapeSettings::default()
        },
        Aabb::from_corners(Vec3::ZERO, Vec3::splat(256.0)),
        Heading::PosX,
    )
    .unwrap()
    .remove(0);
    let points = sphere.points();
    let top = (0..points.len())
        .max_by(|&a, &b| points[a].z.total_cmp(&points[b].z))
        .unwrap();
    let started = std::time::Instant::now();
    for step in 0..20 {
        let to = points[top] + Vec3::new(0.0, 0.0, step as f32);
        let (moved, _) = sphere.with_moved_points(&[(top, to)]).unwrap();
        assert!(moved.bounds().max.z >= points[top].z);
    }
    // Generous: a drag rebuilds once a frame.
    assert!(started.elapsed().as_secs_f32() < 5.0);
}

#[test]
fn random_moves_never_crash_and_keep_every_point_inside() {
    let mut seed: u64 = 0xED17_0000_1234_5678;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % 10_001) as f32 / 10_000.0
    };
    let shapes = [
        Shape::Box,
        Shape::Wedge,
        Shape::Cylinder,
        Shape::Cone,
        Shape::Sphere,
    ];
    let mut made = 0;
    for round in 0..600 {
        let shape = shapes[round % shapes.len()];
        let settings = ShapeSettings {
            sides: [8, 16, 64][(round / shapes.len()) % 3],
            ..ShapeSettings::default()
        };
        let brush = build_shape(
            shape,
            &settings,
            Aabb::from_corners(Vec3::ZERO, Vec3::new(128.0, 96.0, 64.0)),
            Heading::PosX,
        )
        .unwrap()
        .remove(0);
        let mut points = brush.points();
        let moves: Vec<(usize, Vec3)> = (0..1 + (next() * 4.0) as usize)
            .map(|_| {
                let i = (next() * points.len() as f32) as usize % points.len();
                let to = (points[i] + Vec3::new(next() - 0.5, next() - 0.5, next() - 0.5) * 160.0)
                    .round();
                points[i] = to;
                (i, to)
            })
            .collect();
        if let Ok((moved, fates)) = brush.with_moved_points(&moves) {
            made += 1;
            assert_eq!(fates.len(), moved.faces().len());
            // Nor does the result bulge past the points' hull anywhere.
            let wide: Vec<DVec3> = points.iter().map(|p| p.as_dvec3()).collect();
            for t in hull::convex_hull_for_tests(&wide) {
                let [p, q, r] = t.map(|i| wide[i]);
                let Some(normal) = (q - p).cross(r - p).try_normalize() else {
                    continue;
                };
                let height = |x: DVec3| normal.dot(x - p);
                if wide.iter().any(|x| height(*x) > 0.02) {
                    continue; // A sliver leaning off the surface.
                }
                for corner in moved.points() {
                    let out = height(corner.as_dvec3());
                    assert!(out < 0.05, "{shape:?}: corner {corner} bulges {out} out");
                }
            }
            for face in moved.faces() {
                assert!(face.source() < brush.faces().len());
                for p in &points {
                    assert!(
                        face.plane().signed_distance(*p) < 0.05,
                        "{shape:?}: a moved point is {} outside the result",
                        face.plane().signed_distance(*p)
                    );
                }
            }
        }
    }
    assert!(made > 450, "most random moves should work: {made}");
}

#[test]
fn faces_keep_the_old_order() {
    let b = cube();
    let lifted = corner(&b, Vec3::splat(64.0));
    let (bent, _) = b
        .with_moved_points(&[(lifted, Vec3::new(80.0, 80.0, 96.0))])
        .unwrap();
    let sources: Vec<usize> = bent.faces().iter().map(|f| f.source()).collect();
    assert!(sources.windows(2).all(|w| w[0] <= w[1]), "{sources:?}");
}

#[test]
fn a_sixteen_sided_sphere_of_any_size_can_be_edited() {
    // Regression: rounded corners made the hull find one sliver too many,
    // over the 128-face limit, so nothing could be moved.
    for size in [
        Vec3::new(128.0, 96.0, 64.0),
        Vec3::new(200.0, 300.0, 100.0),
        Vec3::splat(256.0),
    ] {
        let sphere = build_shape(
            Shape::Sphere,
            &ShapeSettings {
                sides: 16,
                ..ShapeSettings::default()
            },
            Aabb::from_corners(Vec3::ZERO, size),
            Heading::PosX,
        )
        .unwrap()
        .remove(0);
        let points = sphere.points();
        let top = (0..points.len())
            .max_by(|&a, &b| points[a].z.total_cmp(&points[b].z))
            .unwrap();
        let moved = sphere.with_moved_points(&[(top, points[top] + Vec3::new(0.0, 0.0, 16.0))]);
        let (moved, _) = moved.unwrap_or_else(|e| panic!("{size}: {e}"));
        assert!(moved.faces().len() <= MAX_FACES);
        assert_eq!(moved.bounds().max.z, size.z + 16.0, "{size}");
    }
}
