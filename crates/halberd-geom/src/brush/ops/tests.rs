use super::*;
use crate::{Aabb, Heading, Shape, ShapeSettings, build_shape};

fn block(min: Vec3, max: Vec3) -> Brush {
    Brush::cuboid(Aabb::from_corners(min, max)).unwrap()
}

fn cube() -> Brush {
    block(Vec3::ZERO, Vec3::splat(64.0))
}

fn face_towards(brush: &Brush, normal: Vec3) -> usize {
    (0..brush.faces().len())
        .max_by(|&a, &b| {
            let d = |i: usize| brush.faces()[i].plane().normal.dot(normal);
            d(a).total_cmp(&d(b))
        })
        .unwrap()
}

fn corner(brush: &Brush, at: Vec3) -> usize {
    brush
        .points()
        .iter()
        .position(|p| p.distance(at) < 1e-3)
        .unwrap()
}

#[test]
fn volume_of_boxes_and_wedges() {
    assert_eq!(cube().volume(), 64.0 * 64.0 * 64.0);
    assert_eq!(
        block(Vec3::splat(-10.0), Vec3::new(10.0, 20.0, 30.0)).volume(),
        20.0 * 30.0 * 40.0
    );
    let wedge = build_shape(
        Shape::Wedge,
        &ShapeSettings::default(),
        Aabb::from_corners(Vec3::ZERO, Vec3::new(64.0, 64.0, 64.0)),
        Heading::PosX,
    )
    .unwrap()
    .remove(0);
    assert!((wedge.volume() - 64.0 * 64.0 * 32.0).abs() < 1.0);
}

#[test]
fn extruding_the_top_face_stands_a_new_brush_on_it() {
    let b = cube();
    let top = face_towards(&b, Vec3::Z);
    let new = b.extruded_face(top, 16.0).unwrap();
    assert_eq!(
        new.bounds(),
        Aabb::from_corners(Vec3::new(0.0, 0.0, 64.0), Vec3::new(64.0, 64.0, 80.0))
    );
    assert!(new.faces().iter().all(|f| f.source() == top));
    // A sloped face extrudes along its own normal.
    let wedge = build_shape(
        Shape::Wedge,
        &ShapeSettings::default(),
        Aabb::from_corners(Vec3::ZERO, Vec3::new(64.0, 64.0, 64.0)),
        Heading::PosX,
    )
    .unwrap()
    .remove(0);
    let slope = (0..wedge.faces().len())
        .find(|&f| {
            let n = wedge.faces()[f].plane().normal;
            n.z > 0.1 && n.x.abs() > 0.1
        })
        .unwrap();
    let ramp = wedge.extruded_face(slope, 8.0).unwrap();
    assert_eq!(ramp.faces().len(), 6);
    assert!((ramp.volume() - 8.0 * 64.0 * 64.0 * 2f32.sqrt()).abs() < 50.0);
    assert!(b.extruded_face(top, 0.5).is_err(), "too thin");
    assert!(b.extruded_face(99, 16.0).is_err());
}

#[test]
fn bevelling_an_edge_adds_a_slanted_face() {
    let b = cube();
    let edge = (
        corner(&b, Vec3::new(64.0, 0.0, 64.0)),
        corner(&b, Vec3::new(64.0, 64.0, 64.0)),
    );
    let (bevelled, fates) = b.bevelled(&[edge], 16.0).unwrap();
    assert_eq!(bevelled.faces().len(), 7);
    assert_eq!(fates.len(), 7);
    let slant = bevelled
        .faces()
        .iter()
        .find(|f| f.plane().normal.x > 0.5 && f.plane().normal.z > 0.5)
        .unwrap();
    assert!(
        slant.vertices().iter().all(|v| *v == v.round()),
        "on whole units"
    );
    assert!(bevelled.points().contains(&Vec3::new(48.0, 0.0, 64.0)));
    assert!(bevelled.points().contains(&Vec3::new(64.0, 0.0, 48.0)));
    // Only the bottom and far side keep exactly their corners (the two
    // ends gain one each).
    assert_eq!(fates.iter().filter(|f| f.whole).count(), 2);
    assert_eq!(
        b.bevelled(&[edge], 64.0).unwrap_err(),
        GeomError::BevelTooWide
    );
    // Two edges at once; a pair that is not an edge is ignored.
    let other = (
        corner(&b, Vec3::new(0.0, 0.0, 64.0)),
        corner(&b, Vec3::new(0.0, 64.0, 64.0)),
    );
    let diagonal = (corner(&b, Vec3::ZERO), corner(&b, Vec3::splat(64.0)));
    let (two, _) = b.bevelled(&[edge, other, diagonal], 8.0).unwrap();
    assert_eq!(two.faces().len(), 8);
}

#[test]
fn cutting_gives_two_parts_that_fill_the_brush() {
    let b = cube();
    let plane = Plane::new(Vec3::new(1.0, 0.0, 1.0), Vec3::new(32.0, 0.0, 32.0)).unwrap();
    let (behind, front) = b.cut(plane);
    let (behind, front) = (behind.unwrap(), front.unwrap());
    assert!((behind.volume() + front.volume() - b.volume()).abs() < 1.0);
    assert!(behind.volume() > front.volume());
    assert!(
        behind.faces().iter().any(|f| f.source() == 6),
        "the cut face"
    );
    // A plane that misses, or lies on a face, cuts nothing.
    let away = Plane::new(Vec3::Z, Vec3::new(0.0, 0.0, 500.0)).unwrap();
    assert_eq!(b.cut(away), (Some(b.clone()), None));
    let on_top = b.faces()[face_towards(&b, Vec3::Z)].plane();
    assert_eq!(b.cut(on_top), (Some(b.clone()), None));
    let below = Plane::new(Vec3::NEG_Z, Vec3::new(0.0, 0.0, 500.0)).unwrap();
    assert_eq!(b.cut(below), (None, Some(b.clone())));
}

#[test]
fn mirroring_flips_a_wedge_and_keeps_face_order() {
    let wedge = build_shape(
        Shape::Wedge,
        &ShapeSettings::default(),
        Aabb::from_corners(Vec3::ZERO, Vec3::new(128.0, 64.0, 64.0)),
        Heading::PosX,
    )
    .unwrap()
    .remove(0);
    let flipped = wedge.mirrored(0, 64.0).unwrap();
    assert_eq!(flipped.bounds(), wedge.bounds());
    // The ramp now climbs the other way.
    let high = |b: &Brush| {
        b.points()
            .iter()
            .filter(|p| p.z == 64.0)
            .map(|p| p.x)
            .fold(0.0, f32::max)
    };
    assert_eq!(high(&wedge), 128.0);
    assert_eq!(high(&flipped), 0.0, "{:?}", flipped.points());
    for (i, f) in flipped.faces().iter().enumerate() {
        assert_eq!(f.source(), i);
    }
    assert!((flipped.volume() - wedge.volume()).abs() < 1.0);
    assert!(wedge.mirrored(3, 0.0).is_err());
}

#[test]
fn taking_away_a_face_grows_the_others_or_is_refused() {
    let b = cube();
    assert_eq!(b.without_faces(&[0]).unwrap_err(), GeomError::WouldOpen);
    // A box with a corner cut off: removing the cut grows the box back.
    let plane = Plane::new(Vec3::new(1.0, 1.0, 1.0), Vec3::new(64.0, 64.0, 48.0)).unwrap();
    let (cut, _) = b.cut(plane);
    let cut = cut.unwrap();
    let slant = face_towards(&cut, Vec3::ONE);
    let (back, fates) = cut.without_faces(&[slant]).unwrap();
    assert_eq!(back.bounds(), b.bounds());
    assert_eq!(back.faces().len(), 6);
    assert!(back.faces().iter().all(|f| f.source() != slant));
    assert_eq!(fates.iter().filter(|f| f.whole).count(), 3);
}

#[test]
fn removing_corners_and_hulls() {
    let b = cube();
    let (cut, _) = b.without_points(&[corner(&b, Vec3::splat(64.0))]).unwrap();
    assert_eq!(cut.points().len(), 7);
    assert!((cut.volume() - (b.volume() - 64.0f32.powi(3) / 6.0)).abs() < 1.0);
    let top: Vec<usize> = (0..8).filter(|&i| b.points()[i].z == 64.0).collect();
    let mut all_but_three = top.clone();
    all_but_three.push(corner(&b, Vec3::ZERO));
    assert_eq!(
        b.without_points(&all_but_three).unwrap_err(),
        GeomError::NotClosed
    );
    let hull = Brush::hull(&[Vec3::ZERO, Vec3::X * 64.0, Vec3::Y * 64.0, Vec3::Z * 64.0]).unwrap();
    assert_eq!(hull.faces().len(), 4);
    assert!((hull.volume() - 64.0f32.powi(3) / 6.0).abs() < 1.0);
    assert!(Brush::hull(&[Vec3::ZERO, Vec3::X, Vec3::Y]).is_err());
    assert!(Brush::hull(&[Vec3::NAN, Vec3::X, Vec3::Y, Vec3::Z]).is_err());
}

#[test]
fn bevels_that_run_into_each_other_are_refused() {
    // Regression: four 40-unit bevels round a 64 cube's top each passed on
    // their own but together swallowed the top face.
    let b = cube();
    let top: Vec<usize> = (0..8).filter(|&i| b.points()[i].z == 64.0).collect();
    let edges: Vec<(usize, usize)> = b
        .edges()
        .into_iter()
        .filter(|(i, j)| top.contains(i) && top.contains(j))
        .collect();
    assert_eq!(edges.len(), 4);
    assert_eq!(
        b.bevelled(&edges, 40.0).unwrap_err(),
        GeomError::BevelTooWide
    );
    let (four, _) = b.bevelled(&edges, 8.0).unwrap();
    assert_eq!(four.faces().len(), 10);
}

#[test]
fn a_hull_of_too_many_points_is_refused_quickly() {
    let started = std::time::Instant::now();
    let many: Vec<Vec3> = (0..2000)
        .map(|i| {
            let (a, b) = (i as f32 * 0.37, i as f32 * 0.11);
            Vec3::new(a.cos() * b.sin(), a.sin() * b.sin(), b.cos()) * 500.0
        })
        .collect();
    assert_eq!(Brush::hull(&many).unwrap_err(), GeomError::TooManyFaces);
    let some: Vec<Vec3> = many[..300].to_vec();
    assert!(Brush::hull(&some).is_err());
    assert!(
        started.elapsed().as_secs_f32() < 20.0,
        "{:?}",
        started.elapsed()
    );
}
