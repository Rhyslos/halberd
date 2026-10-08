//! The viewport camera: where it is, where it looks, and how it moves.
//!
//! Coordinates follow Hammer: **Z is up**, units are Hammer units, and the
//! system is right-handed (X forward, Y left when facing +X).

use glam::{Mat4, Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};
use std::f32::consts::{PI, TAU};

/// The world's up direction (Hammer convention).
pub const WORLD_UP: Vec3 = Vec3::Z;
/// Steepest the camera may look up or down: just short of straight, so the
/// view never flips over.
pub const MAX_PITCH: f32 = 89.0 * PI / 180.0;
/// The camera stays within this distance of the origin on every axis. Source
/// maps end at ±16384 (±32768 in some branches); this leaves room around them.
pub const WORLD_LIMIT: f32 = 131_072.0;
/// Nearest distance the camera draws, in units.
pub const NEAR_PLANE: f32 = 1.0;
/// Farthest distance the camera draws, in units.
pub const FAR_PLANE: f32 = 262_144.0;
/// Default vertical field of view.
pub const DEFAULT_FOV_Y: f32 = 70.0 * PI / 180.0;

const MIN_FOV_Y: f32 = 20.0 * PI / 180.0;
const MAX_FOV_Y: f32 = 120.0 * PI / 180.0;

/// A straight line from `origin` in `direction` (unit length).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    /// Where the ray starts.
    pub origin: Vec3,
    /// Which way it points; unit length.
    pub direction: Vec3,
}

impl Ray {
    /// The point `distance` units along the ray.
    pub fn at(&self, distance: f32) -> Vec3 {
        self.origin + self.direction * distance
    }

    /// Distance along the ray to the horizontal plane at height `z`, if the
    /// ray hits it in front of its origin.
    pub fn hit_horizontal_plane(&self, z: f32) -> Option<f32> {
        if self.direction.z.abs() < 1e-6 {
            return None;
        }
        let distance = (z - self.origin.z) / self.direction.z;
        (distance > 0.0 && distance.is_finite()).then_some(distance)
    }
}

/// A perspective camera described by its position and two angles.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    /// Where the camera is.
    pub position: Vec3,
    /// Turn left/right, in radians, measured from +X towards +Y.
    pub yaw: f32,
    /// Look up/down, in radians; positive looks up.
    pub pitch: f32,
    /// Vertical field of view, in radians.
    pub fov_y: f32,
}

impl Default for Camera {
    /// A view of the origin from above and to one side, as when opening a
    /// new map.
    fn default() -> Self {
        Self::looking_at(Vec3::new(-768.0, -768.0, 512.0), Vec3::ZERO)
    }
}

impl Camera {
    /// A camera at `position` looking at `target`.
    pub fn looking_at(position: Vec3, target: Vec3) -> Self {
        let direction = (target - position).normalize_or_zero();
        let (yaw, pitch) = if direction == Vec3::ZERO {
            (0.0, 0.0)
        } else {
            (
                direction.y.atan2(direction.x),
                direction
                    .z
                    .clamp(-1.0, 1.0)
                    .asin()
                    .clamp(-MAX_PITCH, MAX_PITCH),
            )
        };
        Self {
            position,
            yaw,
            pitch,
            fov_y: DEFAULT_FOV_Y,
        }
    }

    /// The direction the camera looks.
    pub fn forward(&self) -> Vec3 {
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        Vec3::new(cos_pitch * cos_yaw, cos_pitch * sin_yaw, sin_pitch)
    }

    /// The camera's right-hand direction; always horizontal.
    pub fn right(&self) -> Vec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        Vec3::new(sin_yaw, -cos_yaw, 0.0)
    }

    /// The camera's up direction (towards the top of the screen).
    pub fn up(&self) -> Vec3 {
        self.right().cross(self.forward())
    }

    /// World-to-camera transform (right-handed; the camera looks down its
    /// own -Z axis, with its own Y pointing up the screen).
    pub fn view_matrix(&self) -> Mat4 {
        let (f, r, u) = (self.forward(), self.right(), self.up());
        let eye = self.position;
        Mat4::from_cols(
            Vec4::new(r.x, u.x, -f.x, 0.0),
            Vec4::new(r.y, u.y, -f.y, 0.0),
            Vec4::new(r.z, u.z, -f.z, 0.0),
            Vec4::new(-r.dot(eye), -u.dot(eye), f.dot(eye), 1.0),
        )
    }

    /// Camera-to-screen transform for a viewport of the given aspect ratio
    /// (width / height). Depth runs from 0 (near) to 1 (far), as wgpu expects.
    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        let aspect = if aspect.is_finite() && aspect > 1e-3 {
            aspect
        } else {
            1.0
        };
        let y_scale = 1.0 / (self.fov_y * 0.5).tan();
        let x_scale = y_scale / aspect;
        let depth = FAR_PLANE / (NEAR_PLANE - FAR_PLANE);
        Mat4::from_cols(
            Vec4::new(x_scale, 0.0, 0.0, 0.0),
            Vec4::new(0.0, y_scale, 0.0, 0.0),
            Vec4::new(0.0, 0.0, depth, -1.0),
            Vec4::new(0.0, 0.0, depth * NEAR_PLANE, 0.0),
        )
    }

    /// World-to-screen transform for a viewport of `size` (any unit).
    pub fn view_projection(&self, size: Vec2) -> Mat4 {
        self.projection_matrix(aspect_of(size)) * self.view_matrix()
    }

    /// The ray from the camera through a point in the viewport, given in the
    /// same units as `size`, measured from the top-left corner.
    pub fn ray_through(&self, point: Vec2, size: Vec2) -> Ray {
        let size = size.max(Vec2::ONE);
        let ndc_x = 2.0 * point.x / size.x - 1.0;
        let ndc_y = 1.0 - 2.0 * point.y / size.y;
        let half_height = (self.fov_y * 0.5).tan();
        let half_width = half_height * aspect_of(size);
        let direction = (self.forward()
            + self.right() * (ndc_x * half_width)
            + self.up() * (ndc_y * half_height))
            .normalize_or(self.forward());
        Ray {
            origin: self.position,
            direction,
        }
    }

    /// Where a world point appears in a viewport of `size`, measured from the
    /// top-left corner. `None` when the point is behind the camera.
    pub fn project(&self, point: Vec3, size: Vec2) -> Option<Vec2> {
        let clip = self.view_projection(size) * point.extend(1.0);
        if clip.w <= 1e-4 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        Some(Vec2::new(
            (ndc.x + 1.0) * 0.5 * size.x,
            (1.0 - ndc.y) * 0.5 * size.y,
        ))
    }

    /// How far in front of the camera a point is, along the view direction.
    pub fn depth_of(&self, point: Vec3) -> f32 {
        (point - self.position).dot(self.forward())
    }

    /// Turns the camera around a fixed point. The point stays exactly where it
    /// was on screen; the camera's distance to it is kept.
    pub fn orbit(&mut self, pivot: Vec3, delta_yaw: f32, delta_pitch: f32) {
        // The pivot's position in the camera's own frame (right, up, forward).
        let offset = pivot - self.position;
        let local = Vec3::new(
            offset.dot(self.right()),
            offset.dot(self.up()),
            offset.dot(self.forward()),
        );
        self.look(delta_yaw, delta_pitch);
        // Put the camera where the pivot has the same frame coordinates again.
        self.position =
            pivot - (self.right() * local.x + self.up() * local.y + self.forward() * local.z);
    }

    /// Turns the camera's head without moving it.
    pub fn look(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw = wrap_angle(self.yaw + delta_yaw);
        self.pitch = (self.pitch + delta_pitch).clamp(-MAX_PITCH, MAX_PITCH);
    }

    /// Slides the camera sideways and up/down so that whatever is `depth`
    /// units away follows the pointer, which moved by `delta` in a viewport
    /// `viewport_height` tall (same units as `delta`).
    pub fn pan(&mut self, delta: Vec2, depth: f32, viewport_height: f32) {
        let world_per_unit = 2.0 * depth * (self.fov_y * 0.5).tan() / viewport_height.max(1.0);
        self.position += (-self.right() * delta.x + self.up() * delta.y) * world_per_unit;
    }

    /// Returns a usable camera: any non-finite value resets it to the
    /// default, the position is kept inside [`WORLD_LIMIT`], and angles are
    /// brought into range.
    pub fn sanitized(self) -> Self {
        let finite = self.position.is_finite()
            && self.yaw.is_finite()
            && self.pitch.is_finite()
            && self.fov_y.is_finite();
        if !finite {
            return Self::default();
        }
        Self {
            position: self
                .position
                .clamp(Vec3::splat(-WORLD_LIMIT), Vec3::splat(WORLD_LIMIT)),
            yaw: wrap_angle(self.yaw),
            pitch: self.pitch.clamp(-MAX_PITCH, MAX_PITCH),
            fov_y: self.fov_y.clamp(MIN_FOV_Y, MAX_FOV_Y),
        }
    }
}

/// Width divided by height, guarding against empty sizes.
fn aspect_of(size: Vec2) -> f32 {
    if size.x > 0.0 && size.y > 0.0 && size.x.is_finite() && size.y.is_finite() {
        size.x / size.y
    } else {
        1.0
    }
}

/// Brings an angle into the range (-π, π].
pub(crate) fn wrap_angle(angle: f32) -> f32 {
    // Angles already in range are returned untouched, so saving and loading
    // a camera gives back exactly the same values.
    if angle > -PI && angle <= PI {
        return angle;
    }
    let wrapped = (angle + PI).rem_euclid(TAU) - PI;
    if wrapped <= -PI {
        wrapped + TAU
    } else {
        wrapped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: Vec2 = Vec2::new(1600.0, 900.0);

    fn close(a: Vec3, b: Vec3, tolerance: f32) -> bool {
        (a - b).length() <= tolerance
    }

    fn sample_cameras() -> Vec<Camera> {
        let mut cameras = vec![Camera::default()];
        for yaw in [-3.0_f32, -1.2, 0.0, 0.7, 2.9] {
            for pitch in [-1.5_f32, -0.6, 0.0, 0.4, 1.5] {
                cameras.push(Camera {
                    position: Vec3::new(120.0, -340.0, 64.0),
                    yaw,
                    pitch,
                    fov_y: DEFAULT_FOV_Y,
                });
            }
        }
        cameras
    }

    #[test]
    fn default_camera_looks_at_the_origin() {
        let camera = Camera::default();
        let to_origin = (Vec3::ZERO - camera.position).normalize();
        assert!(camera.forward().dot(to_origin) > 0.9999);
        assert!(
            camera
                .project(Vec3::ZERO, SIZE)
                .unwrap()
                .distance(SIZE / 2.0)
                < 0.5
        );
    }

    #[test]
    fn basis_is_orthonormal_and_upright() {
        for c in sample_cameras() {
            let (f, r, u) = (c.forward(), c.right(), c.up());
            for v in [f, r, u] {
                assert!((v.length() - 1.0).abs() < 1e-5);
            }
            assert!(f.dot(r).abs() < 1e-5 && f.dot(u).abs() < 1e-5 && r.dot(u).abs() < 1e-5);
            assert!(r.z.abs() < 1e-6, "right is horizontal");
            assert!(u.z > 0.0, "up points upward");
        }
    }

    #[test]
    fn facing_plus_x_puts_minus_y_on_the_right() {
        let camera = Camera::looking_at(Vec3::ZERO, Vec3::X);
        assert!(close(camera.right(), -Vec3::Y, 1e-6));
        assert!(close(camera.up(), Vec3::Z, 1e-6));
    }

    #[test]
    fn looking_straight_down_stays_valid() {
        let camera = Camera::looking_at(Vec3::new(0.0, 0.0, 500.0), Vec3::ZERO);
        assert!((camera.pitch + MAX_PITCH).abs() < 1e-6);
        assert!(camera.forward().is_finite() && camera.view_matrix().is_finite());
    }

    #[test]
    fn looking_at_itself_does_not_break() {
        let camera = Camera::looking_at(Vec3::ONE, Vec3::ONE);
        assert!(camera.forward().is_finite());
    }

    #[test]
    fn depth_runs_from_zero_at_near_to_one_at_far() {
        let camera = Camera::default();
        let depth_at = |distance: f32| {
            let clip = camera.view_projection(SIZE)
                * camera
                    .position
                    .lerp(camera.position + camera.forward(), distance)
                    .extend(1.0);
            clip.z / clip.w
        };
        assert!(depth_at(NEAR_PLANE).abs() < 1e-5);
        assert!((depth_at(FAR_PLANE) - 1.0).abs() < 1e-4);
        assert!(depth_at(1000.0) > depth_at(100.0), "farther is deeper");
    }

    #[test]
    fn view_matrix_places_the_camera_at_the_origin_looking_down_minus_z() {
        let camera = Camera::default();
        let view = camera.view_matrix();
        assert!(view.transform_point3(camera.position).length() < 1e-3);
        let ahead = view.transform_point3(camera.position + camera.forward() * 10.0);
        assert!((ahead - Vec3::new(0.0, 0.0, -10.0)).length() < 1e-3);
        let above = view.transform_point3(camera.position + camera.up() * 10.0);
        assert!((above - Vec3::new(0.0, 10.0, 0.0)).length() < 1e-3);
    }

    #[test]
    fn points_behind_the_camera_are_not_projected() {
        let camera = Camera::default();
        assert!(
            camera
                .project(camera.position - camera.forward() * 100.0, SIZE)
                .is_none()
        );
    }

    #[test]
    fn ray_through_the_centre_is_the_view_direction() {
        let camera = Camera::default();
        let ray = camera.ray_through(SIZE / 2.0, SIZE);
        assert!(close(ray.direction, camera.forward(), 1e-5));
    }

    #[test]
    fn ray_and_projection_agree() {
        for camera in sample_cameras() {
            for pixel in [
                Vec2::new(10.0, 20.0),
                Vec2::new(800.0, 450.0),
                Vec2::new(1590.0, 880.0),
            ] {
                let point = camera.ray_through(pixel, SIZE).at(700.0);
                let back = camera.project(point, SIZE).unwrap();
                assert!(back.distance(pixel) < 0.05, "{pixel} -> {back}");
            }
        }
    }

    #[test]
    fn orbit_keeps_the_pivot_still_on_screen() {
        let pivot = Vec3::new(64.0, 32.0, 0.0);
        let mut camera = Camera::default();
        let before = camera.project(pivot, SIZE).unwrap();
        let distance = camera.position.distance(pivot);
        for (dy, dp) in [
            (0.3, 0.1),
            (-1.1, -0.4),
            (2.5, 0.9),
            (0.05, -2.0),
            (-0.7, 3.0),
        ] {
            camera.orbit(pivot, dy, dp);
            let now = camera.project(pivot, SIZE).unwrap();
            assert!(
                now.distance(before) < 0.05,
                "pivot drifted: {before} -> {now}"
            );
            assert!((camera.position.distance(pivot) - distance).abs() < 0.01);
        }
    }

    #[test]
    fn orbit_around_an_off_centre_point_keeps_it_still() {
        let mut camera = Camera::default();
        let pivot = camera.ray_through(Vec2::new(300.0, 700.0), SIZE).at(900.0);
        let before = camera.project(pivot, SIZE).unwrap();
        camera.orbit(pivot, 0.8, -0.3);
        assert!(camera.project(pivot, SIZE).unwrap().distance(before) < 0.05);
    }

    #[test]
    fn orbiting_up_stops_before_flipping() {
        let pivot = Vec3::ZERO;
        let mut camera = Camera::default();
        for _ in 0..100 {
            camera.orbit(pivot, 0.0, 0.2);
        }
        assert!((camera.pitch - MAX_PITCH).abs() < 1e-6);
        assert!(camera.forward().z < 1.0);
        assert!(camera.view_matrix().is_finite());
    }

    #[test]
    fn look_turns_without_moving() {
        let mut camera = Camera::default();
        let position = camera.position;
        camera.look(0.4, -0.2);
        assert_eq!(camera.position, position);
    }

    #[test]
    fn pan_moves_the_scene_with_the_pointer() {
        let mut camera = Camera::default();
        let depth = 900.0;
        let point = camera.ray_through(SIZE / 2.0, SIZE).at(depth);
        let start = camera.project(point, SIZE).unwrap();
        let along_view = camera.depth_of(point);
        let delta = Vec2::new(37.0, -21.0);
        camera.pan(delta, along_view, SIZE.y);
        let end = camera.project(point, SIZE).unwrap();
        assert!(
            (end - start - delta).length() < 0.05,
            "moved {:?}",
            end - start
        );
    }

    #[test]
    fn horizontal_plane_hits() {
        let ray = Ray {
            origin: Vec3::new(0.0, 0.0, 100.0),
            direction: -Vec3::Z,
        };
        assert_eq!(ray.hit_horizontal_plane(0.0), Some(100.0));
        let upward = Ray {
            origin: Vec3::new(0.0, 0.0, 100.0),
            direction: Vec3::Z,
        };
        assert_eq!(upward.hit_horizontal_plane(0.0), None);
        let level = Ray {
            origin: Vec3::new(0.0, 0.0, 100.0),
            direction: Vec3::X,
        };
        assert_eq!(level.hit_horizontal_plane(0.0), None);
    }

    #[test]
    fn sanitizing_repairs_bad_values() {
        let broken = Camera {
            position: Vec3::new(f32::NAN, 0.0, 0.0),
            ..Camera::default()
        };
        assert_eq!(broken.sanitized(), Camera::default());

        let far = Camera {
            position: Vec3::splat(1e9),
            ..Camera::default()
        }
        .sanitized();
        assert_eq!(far.position, Vec3::splat(WORLD_LIMIT));

        let spun = Camera {
            yaw: 10.0 * TAU + 0.5,
            pitch: 9.0,
            fov_y: 0.01,
            ..Camera::default()
        }
        .sanitized();
        assert!((spun.yaw - 0.5).abs() < 1e-3);
        assert_eq!(spun.pitch, MAX_PITCH);
        assert_eq!(spun.fov_y, MIN_FOV_Y);
    }

    #[test]
    fn wrapped_angles_stay_in_range() {
        for i in -100..100 {
            let a = wrap_angle(i as f32 * 0.37);
            assert!(a > -PI - 1e-6 && a <= PI + 1e-6, "{a}");
        }
    }

    #[test]
    fn empty_viewport_size_is_harmless() {
        let camera = Camera::default();
        assert!(camera.view_projection(Vec2::ZERO).is_finite());
        assert!(
            camera
                .ray_through(Vec2::ZERO, Vec2::ZERO)
                .direction
                .is_finite()
        );
    }

    #[test]
    fn camera_survives_saving_and_loading() {
        let camera = Camera::looking_at(Vec3::new(1.0, 2.0, 3.0), Vec3::new(40.0, -5.0, 0.0));
        let text = ron::to_string(&camera).unwrap();
        assert_eq!(ron::from_str::<Camera>(&text).unwrap(), camera);
    }
}
