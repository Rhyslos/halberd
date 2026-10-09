//! Tests for the ready-made shapes.

use super::*;

fn bounds(x: f32, y: f32, z: f32) -> Aabb {
    Aabb::from_corners(Vec3::ZERO, Vec3::new(x, y, z))
}

fn one(shape: Shape, b: Aabb, heading: Heading) -> Brush {
    let mut brushes = build_shape(shape, &ShapeSettings::default(), b, heading).unwrap();
    assert_eq!(brushes.len(), 1, "{shape:?}");
    brushes.remove(0)
}

/// Every corner of every brush sits inside `b`; on whole units unless
/// `on_grid` is false.
fn assert_inside(brushes: &[Brush], b: Aabb, on_grid: bool) {
    let slack = if on_grid { 1e-3 } else { 0.05 };
    for brush in brushes {
        for face in brush.faces() {
            for v in face.vertices() {
                if on_grid {
                    assert_eq!(*v, v.round(), "corner {v} is on whole units");
                }
                assert!(
                    v.cmpge(b.min - slack).all() && v.cmple(b.max + slack).all(),
                    "corner {v} inside {b:?}"
                );
            }
        }
    }
}

/// Every corner of every brush sits on whole units, inside `b`.
fn assert_on_grid_inside(brushes: &[Brush], b: Aabb) {
    assert_inside(brushes, b, true);
}

/// The checks every shape passes. A sphere's points are on whole units, but
/// where its slightly uneven bands meet, corners can fall between units (as
/// in Hammer).
fn check(shape: Shape, brushes: &[Brush], b: Aabb) {
    assert_inside(brushes, b, shape != Shape::Sphere);
}

#[test]
fn every_shape_fits_its_box_on_whole_units() {
    let b = Aabb::from_corners(Vec3::new(-64.0, 32.0, 16.0), Vec3::new(192.0, 160.0, 144.0));
    for shape in Shape::ALL {
        for heading in [Heading::PosX, Heading::NegX, Heading::PosY, Heading::NegY] {
            let brushes = build_shape(shape, &ShapeSettings::default(), b, heading).unwrap();
            assert!(!brushes.is_empty());
            check(shape, &brushes, b);
            let all = brushes
                .iter()
                .map(Brush::bounds)
                .reduce(Aabb::union)
                .unwrap();
            assert_eq!(all.min.z, b.min.z, "{shape:?} stands on the bottom");
            assert_eq!(all.max.z, b.max.z, "{shape:?} reaches the top");
        }
    }
}

#[test]
fn a_box_is_the_box() {
    let b = bounds(64.0, 32.0, 16.0);
    assert_eq!(one(Shape::Box, b, Heading::PosX).bounds(), b);
}

#[test]
fn a_wedge_climbs_the_way_the_drag_went() {
    let b = bounds(128.0, 64.0, 64.0);
    let wedge = one(Shape::Wedge, b, Heading::PosX);
    assert_eq!(wedge.faces().len(), 5);
    assert_eq!(wedge.bounds(), b);
    // The tall end is at +X: the top edge lies there.
    let top: Vec<Vec3> = wedge
        .faces()
        .iter()
        .flat_map(|f| f.vertices().iter().copied())
        .filter(|v| v.z == 64.0)
        .collect();
    assert!(!top.is_empty() && top.iter().all(|v| v.x == 128.0));
    let back = one(Shape::Wedge, b, Heading::NegX);
    let top_back: Vec<Vec3> = back
        .faces()
        .iter()
        .flat_map(|f| f.vertices().iter().copied())
        .filter(|v| v.z == 64.0)
        .collect();
    assert!(top_back.iter().all(|v| v.x == 0.0));
}

#[test]
fn a_cylinder_has_its_sides_plus_top_and_bottom() {
    let b = bounds(256.0, 256.0, 128.0);
    for sides in [3, 8, 12, 32] {
        let settings = ShapeSettings {
            sides,
            ..ShapeSettings::default()
        };
        let brush = build_shape(Shape::Cylinder, &settings, b, Heading::PosX).unwrap();
        assert_eq!(brush[0].faces().len(), sides as usize + 2, "{sides} sides");
    }
    // Four sides fill the box exactly.
    let square = build_shape(
        Shape::Cylinder,
        &ShapeSettings {
            sides: 4,
            ..ShapeSettings::default()
        },
        b,
        Heading::PosX,
    )
    .unwrap();
    assert_eq!(square[0].bounds(), b);
}

#[test]
fn a_cone_comes_to_a_point_at_the_top_centre() {
    let b = bounds(128.0, 128.0, 256.0);
    let cone = one(Shape::Cone, b, Heading::PosX);
    assert_eq!(cone.faces().len(), 8 + 1);
    let top: Vec<Vec3> = cone
        .faces()
        .iter()
        .flat_map(|f| f.vertices().iter().copied())
        .filter(|v| v.z == 256.0)
        .collect();
    assert!(top.iter().all(|v| *v == Vec3::new(64.0, 64.0, 256.0)));
}

#[test]
fn a_sphere_stays_within_the_face_limit() {
    let b = bounds(512.0, 512.0, 512.0);
    for sides in [1, 4, 7, 16, 99] {
        let settings = ShapeSettings {
            sides,
            ..ShapeSettings::default()
        };
        let sphere = build_shape(Shape::Sphere, &settings, b, Heading::PosX).unwrap();
        let faces = sphere[0].faces().len();
        assert!(faces <= crate::MAX_FACES, "{sides}: {faces}");
        assert!(faces >= 8, "{sides}: {faces}");
    }
}

#[test]
fn an_arch_is_one_brush_per_side_standing_on_both_feet() {
    let b = bounds(256.0, 32.0, 128.0);
    let settings = ShapeSettings {
        sides: 8,
        arch_thickness: 32.0,
        ..ShapeSettings::default()
    };
    let arch = build_shape(Shape::Arch, &settings, b, Heading::PosX).unwrap();
    assert_eq!(arch.len(), 8);
    let feet: Vec<Aabb> = arch
        .iter()
        .map(Brush::bounds)
        .filter(|a| a.min.z == 0.0)
        .collect();
    assert_eq!(feet.len(), 2, "one foot each side");
    assert!(feet.iter().any(|f| f.min.x == 0.0));
    assert!(feet.iter().any(|f| f.max.x == 256.0));
    // The opening under it is clear.
    let middle = Vec3::new(128.0, 16.0, 32.0);
    let inside = |a: &Brush| {
        a.faces()
            .iter()
            .all(|f| f.plane().signed_distance(middle) < 0.0)
    };
    assert!(!arch.iter().any(inside));
    // Spanning along Y instead.
    let along_y = build_shape(
        Shape::Arch,
        &settings,
        bounds(32.0, 256.0, 128.0),
        Heading::PosY,
    )
    .unwrap();
    let span = along_y
        .iter()
        .map(Brush::bounds)
        .reduce(Aabb::union)
        .unwrap();
    assert_eq!(span.size(), Vec3::new(32.0, 256.0, 128.0));
}

#[test]
fn stairs_rise_in_steps_of_about_the_chosen_height() {
    let b = bounds(256.0, 64.0, 64.0);
    let stairs = build_shape(Shape::Stairs, &ShapeSettings::default(), b, Heading::PosX).unwrap();
    assert_eq!(stairs.len(), 8, "64 units in 8-unit steps");
    for (i, step) in stairs.iter().enumerate() {
        let s = step.bounds();
        assert_eq!(s.min.x, 32.0 * i as f32);
        assert_eq!(s.max.z, 8.0 * (i + 1) as f32);
        assert_eq!(s.min.z, 0.0, "solid down to the floor");
    }
    // Going the other way, the top step is at the low X end.
    let down = build_shape(Shape::Stairs, &ShapeSettings::default(), b, Heading::NegX).unwrap();
    let last = down.last().unwrap().bounds();
    assert_eq!((last.min.x, last.max.z), (0.0, 64.0));
    // Steps that do not divide the height evenly stay on whole units.
    let odd = build_shape(
        Shape::Stairs,
        &ShapeSettings {
            step_height: 7.0,
            ..ShapeSettings::default()
        },
        b,
        Heading::PosY,
    )
    .unwrap();
    assert_on_grid_inside(&odd, b);
}

#[test]
fn shapes_too_small_to_build_are_refused_not_broken() {
    let tiny = bounds(2.0, 2.0, 2.0);
    for shape in Shape::ALL {
        // Either a valid shape or an error; never a panic.
        if let Ok(brushes) = build_shape(shape, &ShapeSettings::default(), tiny, Heading::PosX) {
            check(shape, &brushes, tiny);
        }
    }
    let short = bounds(4.0, 64.0, 128.0);
    assert!(
        build_shape(
            Shape::Stairs,
            &ShapeSettings::default(),
            short,
            Heading::PosX
        )
        .is_err()
    );
    let nan = Aabb::from_corners(Vec3::ZERO, Vec3::new(f32::NAN, 1.0, 1.0));
    assert!(
        build_shape(
            Shape::Cylinder,
            &ShapeSettings::default(),
            nan,
            Heading::PosX
        )
        .is_err()
    );
}

#[test]
fn nonsense_settings_are_repaired() {
    let b = bounds(128.0, 128.0, 128.0);
    let settings = ShapeSettings {
        sides: 0,
        arch_thickness: f32::NAN,
        step_height: -3.0,
    };
    for shape in Shape::ALL {
        build_shape(shape, &settings, b, Heading::NegY).unwrap();
    }
}

#[test]
fn the_heading_of_a_drag_is_its_longer_direction() {
    assert_eq!(Heading::of_drag(Vec2::new(10.0, 3.0)), Heading::PosX);
    assert_eq!(Heading::of_drag(Vec2::new(-10.0, 3.0)), Heading::NegX);
    assert_eq!(Heading::of_drag(Vec2::new(1.0, 30.0)), Heading::PosY);
    assert_eq!(Heading::of_drag(Vec2::new(1.0, -30.0)), Heading::NegY);
    assert_eq!(Heading::of_drag(Vec2::ZERO), Heading::PosX);
}

#[test]
fn fuzz_random_boxes_and_settings_never_panic() {
    // A simple repeatable sequence; no outside random library needed.
    let mut seed = 0x2545_f491_u32;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    for _ in 0..2000 {
        let corner = |n: u32| (n % 2000) as f32 - 1000.0;
        let a = Vec3::new(corner(next()), corner(next()), corner(next()));
        let b = Vec3::new(corner(next()), corner(next()), corner(next()));
        let settings = ShapeSettings {
            sides: next() % 80,
            arch_thickness: (next() % 300) as f32,
            step_height: (next() % 40) as f32,
        };
        let shape = Shape::ALL[(next() % 7) as usize];
        let heading =
            [Heading::PosX, Heading::NegX, Heading::PosY, Heading::NegY][(next() % 4) as usize];
        let bounds = Aabb::from_corners(a.min(b), a.max(b));
        if let Ok(brushes) = build_shape(shape, &settings, bounds, heading) {
            assert!(brushes.len() <= MAX_SHAPE_BRUSHES);
            // Very thin shapes with many sides meet at sharp angles, where
            // a corner can land a hair off a whole unit; only inside is
            // checked here, the exact checks are above.
            assert_inside(&brushes, bounds, false);
        }
    }
}

#[test]
fn a_tiny_arch_is_refused_not_a_crash() {
    // Regression: a 1-unit arch made `clamp` panic (its low end above its
    // high end), on every frame of the drag.
    for size in [Vec3::new(1.0, 1.0, 128.0), Vec3::new(64.0, 8.0, 0.5)] {
        let b = Aabb::from_corners(Vec3::ZERO, size);
        assert_eq!(
            build_shape(Shape::Arch, &ShapeSettings::default(), b, Heading::PosX),
            Err(GeomError::TooSmall)
        );
    }
}

#[test]
fn a_tiny_sphere_never_reaches_outside_its_box() {
    // Regression: rounding put a 2×2×3 sphere's corner a unit above its box.
    for size in [
        Vec3::new(2.0, 2.0, 3.0),
        Vec3::new(2.0, 5.0, 4.0),
        Vec3::new(5.0, 2.0, 4.0),
    ] {
        let b = Aabb::from_corners(Vec3::ZERO, size);
        if let Ok(sphere) = build_shape(Shape::Sphere, &ShapeSettings::default(), b, Heading::PosX)
        {
            assert_inside(&sphere, b, false);
        }
    }
}

#[test]
fn stairs_needing_too_many_steps_are_refused_not_stretched() {
    // Regression: 4096 units of 8-unit steps silently became 16-unit steps.
    let b = bounds(8192.0, 64.0, 4096.0);
    assert_eq!(
        build_shape(Shape::Stairs, &ShapeSettings::default(), b, Heading::PosX),
        Err(GeomError::TooManyPieces)
    );
}

#[test]
fn long_ovals_with_many_sides_stay_on_whole_units() {
    // Regression: nearly straight runs of points made nearly parallel sides,
    // whose corners drifted off whole units.
    for (shape, size, sides) in [
        (Shape::Cylinder, Vec3::new(1000.0, 256.0, 128.0), 64),
        (Shape::Cylinder, Vec3::new(1000.0, 64.0, 128.0), 33),
        (Shape::Cone, Vec3::new(7.0, 1000.0, 128.0), 16),
    ] {
        let b = Aabb::from_corners(Vec3::ZERO, size);
        let settings = ShapeSettings {
            sides,
            ..ShapeSettings::default()
        };
        let brushes = build_shape(shape, &settings, b, Heading::PosX).unwrap();
        assert_on_grid_inside(&brushes, b);
    }
}
