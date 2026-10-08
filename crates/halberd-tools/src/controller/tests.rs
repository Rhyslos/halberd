//! Tests for the camera controller.

use super::*;
use crate::camera::WORLD_LIMIT;

const SIZE: Vec2 = Vec2::new(1600.0, 900.0);

fn controller(mode: CameraMode) -> CameraController {
    CameraController::new(
        CameraState {
            mode,
            ..CameraState::default()
        },
        true,
    )
}

fn controller_keeping_pivot_under_cursor(mode: CameraMode) -> CameraController {
    let mut c = controller(mode);
    c.set_orbit_style(OrbitStyle::KeepUnderCursor);
    c
}

/// Holds the right button for `frames` frames at 60 fps, moving by `delta`
/// each frame. Returns whether the last frame asked for another redraw.
fn hold_right(
    c: &mut CameraController,
    cursor: Vec2,
    delta: Vec2,
    frames: usize,
    scene: &dyn SceneQuery,
) -> bool {
    let mut animating = false;
    for _ in 0..frames {
        let input = ViewportInput {
            cursor: Some(cursor),
            right_held: true,
            right_delta: delta,
            ..frame()
        };
        animating = c.update(&input, scene);
    }
    animating
}

fn frame() -> ViewportInput {
    ViewportInput {
        size: SIZE,
        dt: 1.0 / 60.0,
        ..ViewportInput::default()
    }
}

/// Press right at `cursor`, then drag by `delta` over one frame.
fn right_drag(c: &mut CameraController, cursor: Vec2, delta: Vec2, scene: &dyn SceneQuery) {
    let press = ViewportInput {
        cursor: Some(cursor),
        right_pressed: true,
        right_held: true,
        ..frame()
    };
    c.update(&press, scene);
    let drag = ViewportInput {
        cursor: Some(cursor + delta),
        right_held: true,
        right_delta: delta,
        ..frame()
    };
    c.update(&drag, scene);
}

struct Selected(Vec3);
impl SceneQuery for Selected {
    fn pick(&self, ray: &Ray) -> Option<Vec3> {
        GroundPlane.pick(ray)
    }
    fn selection_center(&self) -> Option<Vec3> {
        Some(self.0)
    }
}

#[test]
fn keep_under_cursor_orbits_with_the_pivot_fixed_under_the_pointer() {
    let mut c = controller_keeping_pivot_under_cursor(CameraMode::Default);
    let cursor = Vec2::new(500.0, 700.0);
    let ground = GroundPlane
        .pick(&c.camera().ray_through(cursor, SIZE))
        .unwrap();
    right_drag(&mut c, cursor, Vec2::new(40.0, -15.0), &GroundPlane);
    assert!(c.active_pivot().unwrap().distance(ground) < 1e-3);
    let still = c.camera().project(ground, SIZE).unwrap();
    assert!(
        still.distance(cursor) < 0.1,
        "pivot should stay under the pointer"
    );
}

#[test]
fn centre_on_pivot_is_the_default_style() {
    assert_eq!(
        controller(CameraMode::Default).orbit_style(),
        OrbitStyle::CenterOnPivot
    );
    assert_eq!(
        CameraState::default().orbit_style,
        OrbitStyle::CenterOnPivot
    );
}

#[test]
fn centre_on_pivot_turns_the_view_until_the_pivot_is_centred() {
    let mut c = controller(CameraMode::Default);
    let cursor = Vec2::new(300.0, 750.0);
    let ground = GroundPlane
        .pick(&c.camera().ray_through(cursor, SIZE))
        .unwrap();
    let distance = c.camera().position.distance(ground);
    right_drag(&mut c, cursor, Vec2::new(5.0, 0.0), &GroundPlane);
    assert_eq!(c.active_pivot(), Some(ground));
    // Still turning a few frames in, and the pivot is on its way to the centre.
    assert!(hold_right(&mut c, cursor, Vec2::ZERO, 3, &GroundPlane));
    let partway = c.camera().project(ground, SIZE).unwrap();
    assert!(partway.distance(SIZE / 2.0) < cursor.distance(SIZE / 2.0));
    // After a second the turn has finished and redrawing stops.
    assert!(!hold_right(&mut c, cursor, Vec2::ZERO, 60, &GroundPlane));
    let centred = c.camera().project(ground, SIZE).unwrap();
    assert!(centred.distance(SIZE / 2.0) < 1.0, "pivot at {centred}");
    // Turning the head never moves the camera towards or away from the pivot.
    assert!((c.camera().position.distance(ground) - distance).abs() < 1e-2);
}

#[test]
fn a_centred_pivot_stays_centred_while_orbiting() {
    let mut c = controller(CameraMode::Default);
    let cursor = Vec2::new(1200.0, 600.0);
    let ground = GroundPlane
        .pick(&c.camera().ray_through(cursor, SIZE))
        .unwrap();
    right_drag(&mut c, cursor, Vec2::ZERO, &GroundPlane);
    hold_right(&mut c, cursor, Vec2::ZERO, 60, &GroundPlane);
    let yaw = c.camera().yaw;
    let still_turning = hold_right(&mut c, cursor, Vec2::new(12.0, 3.0), 30, &GroundPlane);
    assert!(
        !still_turning,
        "orbiting a centred pivot needs no extra turning"
    );
    assert!((c.camera().yaw - yaw).abs() > 1.0, "the camera went around");
    let centred = c.camera().project(ground, SIZE).unwrap();
    assert!(centred.distance(SIZE / 2.0) < 1.0, "pivot at {centred}");
}

#[test]
fn centring_stops_when_the_button_is_released() {
    let mut c = controller(CameraMode::Default);
    right_drag(&mut c, Vec2::new(200.0, 800.0), Vec2::ZERO, &GroundPlane);
    let camera = *c.camera();
    assert!(!c.update(&frame(), &GroundPlane));
    assert_eq!(*c.camera(), camera);
}

#[test]
fn distant_orbit_points_are_ignored() {
    let mut c = CameraController::new(
        CameraState {
            camera: Camera::looking_at(Vec3::new(0.0, 0.0, 64.0), Vec3::new(10_000.0, 0.0, 0.0)),
            pivot: Vec3::new(1.0, 2.0, 0.0),
            ..CameraState::default()
        },
        true,
    );
    let centre = SIZE / 2.0;
    let far = GroundPlane
        .pick(&c.camera().ray_through(centre, SIZE))
        .unwrap();
    assert!(far.distance(c.camera().position) > MAX_ORBIT_PIVOT_DISTANCE);
    right_drag(&mut c, centre, Vec2::ZERO, &GroundPlane);
    assert_eq!(c.active_pivot(), Some(Vec3::new(1.0, 2.0, 0.0)));
}

#[test]
fn saves_from_before_orbit_styles_load_with_the_default() {
    let saved = CameraState {
        orbit_style: OrbitStyle::KeepUnderCursor,
        ..CameraState::default()
    };
    let text = ron::to_string(&saved).unwrap();
    let old = text.replace(",orbit_style:KeepUnderCursor", "");
    assert_ne!(old, text, "the test must actually remove the field: {text}");
    let loaded: CameraState = ron::from_str(&old).unwrap();
    assert_eq!(loaded.orbit_style, OrbitStyle::CenterOnPivot);
    let round_trip: CameraState = ron::from_str(&text).unwrap();
    assert_eq!(round_trip, saved);
}

#[test]
fn default_mode_uses_the_last_pivot_when_pointing_at_the_sky() {
    let mut c = controller(CameraMode::Default);
    // The default view looks down at the origin; the top edge is sky.
    let sky = Vec2::new(800.0, 2.0);
    assert!(
        GroundPlane
            .pick(&c.camera().ray_through(sky, SIZE))
            .is_none()
    );
    right_drag(&mut c, sky, Vec2::new(10.0, 0.0), &GroundPlane);
    assert_eq!(c.active_pivot(), Some(Vec3::ZERO));
}

#[test]
fn orbit_mode_uses_the_selection() {
    let mut c = controller(CameraMode::Orbit);
    let centre = Vec3::new(256.0, 128.0, 64.0);
    right_drag(
        &mut c,
        Vec2::new(100.0, 800.0),
        Vec2::new(20.0, 5.0),
        &Selected(centre),
    );
    assert_eq!(c.active_pivot(), Some(centre));
}

#[test]
fn orbit_mode_without_selection_uses_the_last_pivot() {
    let mut c = controller(CameraMode::Orbit);
    right_drag(
        &mut c,
        Vec2::new(100.0, 800.0),
        Vec2::new(20.0, 5.0),
        &GroundPlane,
    );
    assert_eq!(c.active_pivot(), Some(Vec3::ZERO));
}

#[test]
fn pivot_marker_disappears_when_the_drag_ends() {
    let mut c = controller(CameraMode::Default);
    right_drag(&mut c, SIZE / 2.0, Vec2::new(5.0, 5.0), &GroundPlane);
    assert!(c.active_pivot().is_some());
    c.update(&frame(), &GroundPlane);
    assert!(c.active_pivot().is_none());
}

#[test]
fn dragging_right_turns_the_view() {
    let mut c = controller_keeping_pivot_under_cursor(CameraMode::Default);
    let yaw = c.camera().yaw;
    right_drag(&mut c, SIZE / 2.0, Vec2::new(50.0, 0.0), &GroundPlane);
    assert!((c.camera().yaw - (yaw - 50.0 * ORBIT_SENSITIVITY)).abs() < 1e-4);
}

#[test]
fn fly_mode_moves_forward_with_w() {
    let mut c = controller(CameraMode::Fly);
    let start = c.camera().position;
    let forward = c.camera().forward();
    let input = ViewportInput {
        right_held: true,
        keys: FlyKeys {
            forward: true,
            ..FlyKeys::default()
        },
        dt: 0.05,
        ..frame()
    };
    assert!(
        c.update(&input, &GroundPlane),
        "keeps animating while flying"
    );
    let moved = c.camera().position - start;
    assert!((moved - forward * DEFAULT_FLY_SPEED * 0.05).length() < 1e-3);
}

#[test]
fn shift_flies_faster_and_space_goes_up() {
    let mut c = controller(CameraMode::Fly);
    let start = c.camera().position;
    let input = ViewportInput {
        right_held: true,
        keys: FlyKeys {
            up: true,
            fast: true,
            ..FlyKeys::default()
        },
        dt: 0.05,
        ..frame()
    };
    c.update(&input, &GroundPlane);
    let expected = Vec3::Z * DEFAULT_FLY_SPEED * FAST_MULTIPLIER * 0.05;
    assert!((c.camera().position - start - expected).length() < 1e-3);
}

#[test]
fn fly_keys_do_nothing_without_the_right_button() {
    let mut c = controller(CameraMode::Fly);
    let start = c.camera().position;
    let input = ViewportInput {
        keys: FlyKeys {
            forward: true,
            ..FlyKeys::default()
        },
        ..frame()
    };
    assert!(!c.update(&input, &GroundPlane));
    assert_eq!(c.camera().position, start);
}

#[test]
fn fly_mode_looks_without_moving() {
    let mut c = controller(CameraMode::Fly);
    let start = c.camera().position;
    let input = ViewportInput {
        right_held: true,
        right_delta: Vec2::new(30.0, 10.0),
        ..frame()
    };
    c.update(&input, &GroundPlane);
    assert_eq!(c.camera().position, start);
    assert!(c.active_pivot().is_none());
}

#[test]
fn a_long_stall_does_not_cause_a_jump() {
    let mut c = controller(CameraMode::Fly);
    let start = c.camera().position;
    let input = ViewportInput {
        right_held: true,
        keys: FlyKeys {
            forward: true,
            ..FlyKeys::default()
        },
        dt: 5.0,
        ..frame()
    };
    c.update(&input, &GroundPlane);
    let moved = c.camera().position.distance(start);
    assert!((moved - DEFAULT_FLY_SPEED * MAX_DT).abs() < 1e-2);
}

#[test]
fn wasd_off_hides_fly_mode() {
    let mut c = CameraController::new(CameraState::default(), false);
    assert_eq!(
        c.available_modes(),
        [CameraMode::Default, CameraMode::Orbit]
    );
    assert_eq!(c.set_mode(CameraMode::Fly), CameraMode::Default);
}

#[test]
fn saved_fly_mode_falls_back_when_wasd_is_off() {
    let state = CameraState {
        mode: CameraMode::Fly,
        ..CameraState::default()
    };
    assert_eq!(
        CameraController::new(state, false).mode(),
        CameraMode::Default
    );
    assert_eq!(CameraController::new(state, true).mode(), CameraMode::Fly);
}

#[test]
fn middle_drag_pans_the_scene_with_the_pointer() {
    let mut c = controller(CameraMode::Default);
    let cursor = Vec2::new(900.0, 600.0);
    let ground = GroundPlane
        .pick(&c.camera().ray_through(cursor, SIZE))
        .unwrap();
    let press = ViewportInput {
        cursor: Some(cursor),
        middle_pressed: true,
        middle_held: true,
        ..frame()
    };
    c.update(&press, &GroundPlane);
    let delta = Vec2::new(-60.0, 25.0);
    let drag = ViewportInput {
        cursor: Some(cursor + delta),
        middle_held: true,
        middle_delta: delta,
        ..frame()
    };
    c.update(&drag, &GroundPlane);
    let now = c.camera().project(ground, SIZE).unwrap();
    assert!(
        now.distance(cursor + delta) < 0.1,
        "grabbed point follows the pointer"
    );
}

#[test]
fn scrolling_zooms_towards_the_pointer() {
    let mut c = controller(CameraMode::Default);
    let cursor = Vec2::new(400.0, 650.0);
    let target = GroundPlane
        .pick(&c.camera().ray_through(cursor, SIZE))
        .unwrap();
    let before = c.camera().position.distance(target);
    c.update(
        &ViewportInput {
            cursor: Some(cursor),
            scroll: 100.0,
            ..frame()
        },
        &GroundPlane,
    );
    let after = c.camera().position.distance(target);
    assert!(after < before);
    let still = c.camera().project(target, SIZE).unwrap();
    assert!(
        still.distance(cursor) < 0.1,
        "zooming keeps the point under the pointer"
    );
}

#[test]
fn zooming_in_stops_before_the_surface() {
    let mut c = controller(CameraMode::Default);
    let cursor = SIZE / 2.0;
    for _ in 0..200 {
        c.update(
            &ViewportInput {
                cursor: Some(cursor),
                scroll: 400.0,
                ..frame()
            },
            &GroundPlane,
        );
    }
    let target = GroundPlane
        .pick(&c.camera().ray_through(cursor, SIZE))
        .unwrap();
    assert!(c.camera().position.distance(target) >= MIN_ZOOM_DISTANCE - 1e-2);
}

#[test]
fn scrolling_out_moves_away() {
    let mut c = controller(CameraMode::Default);
    let before = c.camera().position.length();
    c.update(
        &ViewportInput {
            cursor: Some(SIZE / 2.0),
            scroll: -100.0,
            ..frame()
        },
        &GroundPlane,
    );
    assert!(c.camera().position.length() > before);
}

#[test]
fn scrolling_while_flying_changes_speed() {
    let mut c = controller(CameraMode::Fly);
    let start = c.camera().position;
    c.update(
        &ViewportInput {
            right_held: true,
            scroll: 200.0,
            ..frame()
        },
        &GroundPlane,
    );
    assert!(c.fly_speed() > DEFAULT_FLY_SPEED);
    assert_eq!(c.camera().position, start);
    for _ in 0..100 {
        c.update(
            &ViewportInput {
                right_held: true,
                scroll: -2000.0,
                ..frame()
            },
            &GroundPlane,
        );
    }
    assert_eq!(c.fly_speed(), FLY_SPEED_RANGE.0);
}

#[test]
fn broken_input_leaves_a_working_camera() {
    let mut c = controller(CameraMode::Default);
    let nan = Vec2::splat(f32::NAN);
    let input = ViewportInput {
        size: Vec2::ZERO,
        cursor: Some(nan),
        right_pressed: true,
        right_held: true,
        right_delta: nan,
        middle_pressed: true,
        middle_held: true,
        middle_delta: Vec2::splat(f32::INFINITY),
        scroll: f32::NAN,
        keys: FlyKeys::default(),
        dt: f32::NAN,
    };
    c.update(&input, &GroundPlane);
    assert!(c.camera().position.is_finite() && c.camera().view_matrix().is_finite());
}

#[test]
fn broken_saved_state_is_repaired() {
    let state = CameraState {
        camera: Camera {
            yaw: f32::NAN,
            ..Camera::default()
        },
        pivot: Vec3::splat(f32::INFINITY),
        fly_speed: -5.0,
        mode: CameraMode::Orbit,
        orbit_style: OrbitStyle::KeepUnderCursor,
    };
    let c = CameraController::new(state, true);
    assert_eq!(*c.camera(), Camera::default());
    assert_eq!(c.state().pivot, Vec3::ZERO);
    assert_eq!(c.fly_speed(), FLY_SPEED_RANGE.0);
}

#[test]
fn a_long_random_session_stays_sane() {
    // A built-in stress test: thousands of frames of random input in
    // every mode must keep the camera finite and inside the world.
    let mut seed: u64 = 0xDEC0_DE12_3456_789A;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut unit = move || (next() % 20_001) as f32 / 10_000.0 - 1.0;
    let mut c = controller(CameraMode::Default);
    for i in 0..20_000 {
        if i % 500 == 0 {
            c.set_mode(CameraMode::ALL[(i / 500) % 3]);
        }
        let input = ViewportInput {
            size: SIZE,
            cursor: Some(Vec2::new((unit() + 1.0) * 800.0, (unit() + 1.0) * 450.0)),
            right_pressed: unit() > 0.8,
            right_held: unit() > -0.2,
            right_delta: Vec2::new(unit(), unit()) * 200.0,
            middle_pressed: unit() > 0.9,
            middle_held: unit() > 0.5,
            middle_delta: Vec2::new(unit(), unit()) * 300.0,
            scroll: if unit() > 0.7 { unit() * 1500.0 } else { 0.0 },
            keys: FlyKeys {
                forward: unit() > 0.0,
                left: unit() > 0.5,
                up: unit() > 0.6,
                fast: unit() > 0.3,
                ..FlyKeys::default()
            },
            dt: (unit() + 1.0) * 0.06,
        };
        c.update(&input, &GroundPlane);
        let cam = c.camera();
        assert!(cam.view_matrix().is_finite(), "frame {i}");
        assert!(cam.position.abs().max_element() <= WORLD_LIMIT, "frame {i}");
    }
}
