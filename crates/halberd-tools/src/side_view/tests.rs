//! Tests for the side views.

use super::*;

const SIZE: Vec2 = Vec2::new(800.0, 600.0);

fn close(a: Vec3, b: Vec3) -> bool {
    a.distance(b) < 1e-2
}

#[test]
fn each_side_looks_along_its_axis_with_the_expected_axes() {
    // (side, looks along, right on screen, up on screen)
    let cases = [
        (Side::Top, -Vec3::Z, Vec3::X, Vec3::Y),
        (Side::Bottom, Vec3::Z, Vec3::X, -Vec3::Y),
        (Side::Front, -Vec3::X, Vec3::Y, Vec3::Z),
        (Side::Back, Vec3::X, -Vec3::Y, Vec3::Z),
        (Side::Left, -Vec3::Y, -Vec3::X, Vec3::Z),
        (Side::Right, Vec3::Y, Vec3::X, Vec3::Z),
    ];
    for (side, forward, right, up) in cases {
        let camera = SideCamera::new(side).camera();
        assert!(close(camera.forward(), forward), "{side:?} forward");
        assert!(close(camera.right(), right), "{side:?} right");
        assert!(close(camera.up(), up), "{side:?} up");
    }
}

#[test]
fn points_project_flat_and_rays_come_back_to_them() {
    let view = SideCamera {
        side: Side::Top,
        center: Vec3::new(100.0, 50.0, 0.0),
        height: 600.0,
    };
    let camera = view.camera();
    // 600 units over 600 points: one unit per point, whatever the height.
    for z in [-5000.0, 0.0, 3000.0] {
        let at = camera.project(Vec3::new(200.0, 50.0, z), SIZE).unwrap();
        assert!(at.distance(Vec2::new(500.0, 300.0)) < 0.01, "{z}: {at}");
    }
    let ray = view.ray_through(Vec2::new(500.0, 200.0), SIZE);
    assert!(close(ray.direction, -Vec3::Z));
    let hit = ray.at(ray.hit_horizontal_plane(0.0).unwrap());
    assert!(close(hit, Vec3::new(200.0, 150.0, 0.0)), "{hit}");
    assert_eq!(camera.units_per_point(Vec3::ZERO, SIZE.y), 1.0);
}

#[test]
fn everything_in_the_world_is_in_front_and_drawn() {
    let camera = SideCamera::new(Side::Front).camera();
    for x in [-WORLD_LIMIT * 0.25, 0.0, WORLD_LIMIT * 0.25] {
        let clip = camera.view_projection(SIZE) * Vec3::new(x, 0.0, 0.0).extend(1.0);
        let depth = clip.z / clip.w;
        assert!((0.0..=1.0).contains(&depth), "{x}: {depth}");
    }
    let (axis, at) = SideCamera::new(Side::Front).grid_plane();
    assert_eq!(axis, 0);
    assert!(at < -WORLD_LIMIT * 0.25, "the grid is behind the map: {at}");
}

#[test]
fn dragging_moves_and_scrolling_zooms_around_the_pointer() {
    let mut view = SideCamera::new(Side::Top);
    view.height = 600.0;
    let drag = ViewportInput {
        size: SIZE,
        right_held: true,
        right_delta: Vec2::new(10.0, 0.0),
        ..ViewportInput::default()
    };
    assert!(view.update(&drag));
    assert!(
        close(view.center, Vec3::new(-10.0, 0.0, 0.0)),
        "{}",
        view.center
    );
    // The world point under the pointer stays there while zooming in.
    let cursor = Vec2::new(700.0, 100.0);
    let under = |v: &SideCamera| {
        let ray = v.ray_through(cursor, SIZE);
        ray.at(ray.hit_horizontal_plane(0.0).unwrap())
    };
    let before = under(&view);
    let zoom = ViewportInput {
        size: SIZE,
        cursor: Some(cursor),
        scroll: 200.0,
        ..ViewportInput::default()
    };
    assert!(view.update(&zoom));
    assert!(view.height < 600.0, "scrolling up zooms in");
    assert!(close(under(&view), before), "{} vs {before}", under(&view));
}

#[test]
fn framing_shows_the_whole_box_and_bad_values_are_repaired() {
    let mut view = SideCamera::new(Side::Right);
    view.frame(Aabb::from_corners(
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1000.0, 64.0, 200.0),
    ));
    assert!(close(view.center, Vec3::new(500.0, 32.0, 100.0)));
    assert!(view.height >= 1000.0);
    let broken = SideCamera {
        side: Side::Top,
        center: Vec3::NAN,
        height: f32::INFINITY,
    }
    .sanitized();
    assert_eq!(broken.center, Vec3::ZERO);
    assert_eq!(broken.height, 2048.0);
    let mut zoomed = SideCamera::new(Side::Top);
    for _ in 0..100 {
        zoomed.update(&ViewportInput {
            size: SIZE,
            scroll: 1000.0,
            ..ViewportInput::default()
        });
    }
    assert_eq!(zoomed.height, SIDE_VIEW_HEIGHT_RANGE.0);
}
