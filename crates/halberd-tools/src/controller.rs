//! Turns viewport input into camera movement, following the feature spec's
//! three camera modes.

use crate::camera::{Camera, NEAR_PLANE, Ray, wrap_angle};
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

/// Radians the camera turns per point the pointer moves while orbiting.
pub const ORBIT_SENSITIVITY: f32 = 0.006;
/// Orbit pivots farther away than this are ignored: orbiting around a point
/// near the horizon swings the camera across the whole map.
pub const MAX_ORBIT_PIVOT_DISTANCE: f32 = 8_192.0;
/// How quickly the view turns to centre the pivot, per second. About 95% of
/// the turn is done after a quarter of a second.
const CENTRE_RATE: f32 = 12.0;
/// Angle (radians) at which the pivot counts as centred and the turn snaps.
const CENTRED: f32 = 1e-3;
/// Radians the camera turns per point the pointer moves while flying.
pub const LOOK_SENSITIVITY: f32 = 0.004;
/// Default flying speed, in units per second (a GMod player sprints at about 400).
pub const DEFAULT_FLY_SPEED: f32 = 600.0;
/// Flying speed limits, in units per second.
pub const FLY_SPEED_RANGE: (f32, f32) = (32.0, 16_384.0);
/// Speed multiplier while Shift is held.
pub const FAST_MULTIPLIER: f32 = 4.0;
/// Picks farther away than this are ignored (the grid at a grazing angle).
pub const MAX_PICK_DISTANCE: f32 = 32_768.0;
/// Depth used for panning and zooming when nothing is under the pointer.
const FALLBACK_DEPTH: f32 = 512.0;
/// Closest zooming gets to the point under the pointer.
const MIN_ZOOM_DISTANCE: f32 = 8.0;
/// Smallest step a scroll moves the camera, so zooming never gets stuck.
const MIN_ZOOM_STEP: f32 = 8.0;
/// How strongly scrolling zooms: fraction of the distance per point scrolled.
const ZOOM_RATE: f32 = 0.003;
/// Longest frame time used for movement, so a stall never causes a jump.
const MAX_DT: f32 = 0.1;

/// How right-dragging moves the camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CameraMode {
    /// Right-drag orbits around whatever is under the pointer.
    #[default]
    Default,
    /// Right-drag always orbits around the selection (or the last pivot).
    Orbit,
    /// Hold right mouse to look around; WASD moves.
    Fly,
}

impl CameraMode {
    /// Every mode, in the order the switcher shows them.
    pub const ALL: [CameraMode; 3] = [Self::Default, Self::Orbit, Self::Fly];

    /// Short name for the mode switcher.
    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Orbit => "Orbit",
            Self::Fly => "Fly",
        }
    }

    /// One-line explanation, for tooltips.
    pub fn description(self) -> &'static str {
        match self {
            Self::Default => "Right-drag orbits around the point under the pointer",
            Self::Orbit => "Right-drag orbits around the selection",
            Self::Fly => "Hold right mouse to look; WASD to move, Space/C up/down, Shift faster",
        }
    }
}

/// Where the orbit pivot sits on screen while orbiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OrbitStyle {
    /// The view smoothly turns to bring the pivot to the centre of the
    /// screen, then turns around it there, like walking around an object.
    #[default]
    CenterOnPivot,
    /// The pivot stays exactly where it was clicked.
    KeepUnderCursor,
}

/// Movement keys held while flying.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FlyKeys {
    /// W
    pub forward: bool,
    /// S
    pub back: bool,
    /// A
    pub left: bool,
    /// D
    pub right: bool,
    /// Space
    pub up: bool,
    /// C
    pub down: bool,
    /// Shift
    pub fast: bool,
}

impl FlyKeys {
    /// True if any movement key (not Shift alone) is held.
    pub fn any_movement(&self) -> bool {
        self.forward || self.back || self.left || self.right || self.up || self.down
    }
}

/// One frame of viewport input, in points measured from the viewport's
/// top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ViewportInput {
    /// Size of the viewport.
    pub size: Vec2,
    /// Where the pointer is, if it is over the viewport or dragging in it.
    pub cursor: Option<Vec2>,
    /// The right button went down on the viewport this frame.
    pub right_pressed: bool,
    /// The right button is held after going down on the viewport.
    pub right_held: bool,
    /// How far the pointer moved this frame while the right button was held.
    pub right_delta: Vec2,
    /// The middle button went down on the viewport this frame.
    pub middle_pressed: bool,
    /// The middle button is held after going down on the viewport.
    pub middle_held: bool,
    /// How far the pointer moved this frame while the middle button was held.
    pub middle_delta: Vec2,
    /// Scroll this frame, in points; positive means away from the user (zoom in).
    pub scroll: f32,
    /// Flying keys held.
    pub keys: FlyKeys,
    /// Seconds since the previous frame.
    pub dt: f32,
}

/// What the camera can ask about the scene.
pub trait SceneQuery {
    /// The first surface the ray hits, if any.
    fn pick(&self, ray: &Ray) -> Option<Vec3>;
    /// The centre of the current selection, if anything is selected.
    fn selection_center(&self) -> Option<Vec3>;
}

/// The scene before maps exist: just the grid plane (Z = 0), nothing
/// selectable.
#[derive(Debug, Clone, Copy, Default)]
pub struct GroundPlane;

impl SceneQuery for GroundPlane {
    fn pick(&self, ray: &Ray) -> Option<Vec3> {
        let distance = ray.hit_horizontal_plane(0.0)?;
        (distance <= MAX_PICK_DISTANCE).then(|| ray.at(distance))
    }

    fn selection_center(&self) -> Option<Vec3> {
        None
    }
}

/// The parts of the camera worth remembering between sessions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraState {
    /// The camera itself.
    pub camera: Camera,
    /// Selected camera mode.
    pub mode: CameraMode,
    /// The last orbit pivot.
    pub pivot: Vec3,
    /// Flying speed, in units per second.
    pub fly_speed: f32,
    /// Where the pivot sits on screen while orbiting. Saves from before this
    /// existed load with the default.
    #[serde(default)]
    pub orbit_style: OrbitStyle,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            camera: Camera::default(),
            mode: CameraMode::Default,
            pivot: Vec3::ZERO,
            fly_speed: DEFAULT_FLY_SPEED,
            orbit_style: OrbitStyle::default(),
        }
    }
}

/// Moves a camera in response to viewport input.
#[derive(Debug, Clone)]
pub struct CameraController {
    state: CameraState,
    wasd_enabled: bool,
    /// The pivot of the orbit in progress, chosen when the drag began.
    orbit_pivot: Option<Vec3>,
    /// The depth used by the pan in progress, chosen when the drag began.
    pan_depth: Option<f32>,
}

impl CameraController {
    /// A controller starting from saved state. Values are repaired if needed.
    /// With `wasd_enabled` false, Fly mode is unavailable.
    pub fn new(state: CameraState, wasd_enabled: bool) -> Self {
        let mut controller = Self {
            state: sanitize_state(state),
            wasd_enabled,
            orbit_pivot: None,
            pan_depth: None,
        };
        controller.set_mode(controller.state.mode);
        controller
    }

    /// The state to save between sessions.
    pub fn state(&self) -> CameraState {
        self.state
    }

    /// The camera.
    pub fn camera(&self) -> &Camera {
        &self.state.camera
    }

    /// The current mode.
    pub fn mode(&self) -> CameraMode {
        self.state.mode
    }

    /// The modes the user may choose (Fly is hidden when WASD is turned off).
    pub fn available_modes(&self) -> Vec<CameraMode> {
        CameraMode::ALL
            .into_iter()
            .filter(|m| self.mode_allowed(*m))
            .collect()
    }

    /// Switches mode. Fly falls back to Default when WASD is turned off.
    /// Returns the mode actually in use.
    pub fn set_mode(&mut self, mode: CameraMode) -> CameraMode {
        self.state.mode = if self.mode_allowed(mode) {
            mode
        } else {
            CameraMode::Default
        };
        self.orbit_pivot = None;
        self.state.mode
    }

    /// The pivot of the orbit in progress, for drawing its marker.
    pub fn active_pivot(&self) -> Option<Vec3> {
        self.orbit_pivot
    }

    /// Flying speed, in units per second.
    pub fn fly_speed(&self) -> f32 {
        self.state.fly_speed
    }

    /// Where the pivot sits on screen while orbiting.
    pub fn orbit_style(&self) -> OrbitStyle {
        self.state.orbit_style
    }

    /// Changes where the pivot sits on screen while orbiting.
    pub fn set_orbit_style(&mut self, style: OrbitStyle) {
        self.state.orbit_style = style;
    }

    /// Applies one frame of input. Returns true while the camera keeps moving
    /// without new input (flying with a key held, or turning to centre the
    /// orbit pivot), so the caller should keep redrawing.
    pub fn update(&mut self, input: &ViewportInput, scene: &dyn SceneQuery) -> bool {
        let input = clean_input(input);
        let centring = match self.state.mode {
            CameraMode::Default | CameraMode::Orbit => self.update_orbit(&input, scene),
            CameraMode::Fly => {
                self.update_fly(&input);
                false
            }
        };
        self.update_pan(&input, scene);
        self.update_scroll(&input, scene);
        self.state = sanitize_state(self.state);
        let flying =
            self.state.mode == CameraMode::Fly && input.right_held && input.keys.any_movement();
        flying || centring
    }

    fn mode_allowed(&self, mode: CameraMode) -> bool {
        mode != CameraMode::Fly || self.wasd_enabled
    }

    /// Returns true while the view is still turning to centre the pivot.
    fn update_orbit(&mut self, input: &ViewportInput, scene: &dyn SceneQuery) -> bool {
        if !input.right_held {
            self.orbit_pivot = None;
            return false;
        }
        if input.right_pressed || self.orbit_pivot.is_none() {
            let pivot = self.choose_pivot(input, scene);
            self.state.pivot = pivot;
            self.orbit_pivot = Some(pivot);
        }
        let Some(pivot) = self.orbit_pivot else {
            return false;
        };
        if input.right_delta != Vec2::ZERO {
            let turn = -input.right_delta * ORBIT_SENSITIVITY;
            self.state.camera.orbit(pivot, turn.x, turn.y);
        }
        match self.state.orbit_style {
            OrbitStyle::CenterOnPivot => self.turn_towards(pivot, input.dt),
            OrbitStyle::KeepUnderCursor => false,
        }
    }

    /// Turns the camera's head part of the way towards `point`, easing out.
    /// Returns true if it still has turning left to do.
    fn turn_towards(&mut self, point: Vec3, dt: f32) -> bool {
        let camera = &mut self.state.camera;
        if camera.position.distance(point) < NEAR_PLANE {
            return false;
        }
        let target = Camera::looking_at(camera.position, point);
        let yaw = wrap_angle(target.yaw - camera.yaw);
        let pitch = target.pitch - camera.pitch;
        if yaw.abs() < CENTRED && pitch.abs() < CENTRED {
            camera.look(yaw, pitch);
            return false;
        }
        let k = 1.0 - (-CENTRE_RATE * dt).exp();
        camera.look(yaw * k, pitch * k);
        true
    }

    /// Default mode: the surface under the pointer if it is near enough,
    /// else the last pivot. Orbit mode: the selection, else the last pivot.
    fn choose_pivot(&self, input: &ViewportInput, scene: &dyn SceneQuery) -> Vec3 {
        let position = self.state.camera.position;
        let picked = match self.state.mode {
            CameraMode::Default => input
                .cursor
                .and_then(|c| self.pick_at(c, input.size, scene))
                .filter(|p| p.distance(position) <= MAX_ORBIT_PIVOT_DISTANCE),
            CameraMode::Orbit | CameraMode::Fly => scene.selection_center(),
        };
        picked.unwrap_or(self.state.pivot)
    }

    fn update_fly(&mut self, input: &ViewportInput) {
        self.orbit_pivot = None;
        if !input.right_held {
            return;
        }
        let turn = -input.right_delta * LOOK_SENSITIVITY;
        self.state.camera.look(turn.x, turn.y);

        let keys = input.keys;
        let axis = |positive: bool, negative: bool| f32::from(positive) - f32::from(negative);
        let camera = &self.state.camera;
        let direction = camera.forward() * axis(keys.forward, keys.back)
            + camera.right() * axis(keys.right, keys.left)
            + Vec3::Z * axis(keys.up, keys.down);
        if direction != Vec3::ZERO {
            let speed = self.state.fly_speed * if keys.fast { FAST_MULTIPLIER } else { 1.0 };
            self.state.camera.position += direction.normalize() * speed * input.dt;
        }
    }

    fn update_pan(&mut self, input: &ViewportInput, scene: &dyn SceneQuery) {
        if !input.middle_held {
            self.pan_depth = None;
            return;
        }
        if input.middle_pressed || self.pan_depth.is_none() {
            self.pan_depth = Some(self.depth_under(input, scene));
        }
        if let Some(depth) = self.pan_depth {
            self.state
                .camera
                .pan(input.middle_delta, depth, input.size.y);
        }
    }

    fn update_scroll(&mut self, input: &ViewportInput, scene: &dyn SceneQuery) {
        if input.scroll == 0.0 {
            return;
        }
        if self.state.mode == CameraMode::Fly && input.right_held {
            // While flying, the wheel changes speed instead of zooming.
            let factor = (input.scroll * ZOOM_RATE).exp();
            let (low, high) = FLY_SPEED_RANGE;
            self.state.fly_speed = (self.state.fly_speed * factor).clamp(low, high);
            return;
        }
        let cursor = input.cursor.unwrap_or(input.size * 0.5);
        let ray = self.state.camera.ray_through(cursor, input.size);
        let hit = scene
            .pick(&ray)
            .filter(|p| p.distance(ray.origin) <= MAX_PICK_DISTANCE);
        let distance = match hit {
            Some(point) => point.distance(ray.origin),
            None => self.fallback_distance(),
        };
        let fraction = 1.0 - (-input.scroll.abs() * ZOOM_RATE).exp();
        let mut step = (distance * fraction).max(MIN_ZOOM_STEP);
        if input.scroll > 0.0 {
            if hit.is_some() {
                step = step.min((distance - MIN_ZOOM_DISTANCE).max(0.0));
            }
        } else {
            step = -step;
        }
        self.state.camera.position += ray.direction * step;
    }

    /// Depth along the view of what is under the pointer, for panning.
    fn depth_under(&self, input: &ViewportInput, scene: &dyn SceneQuery) -> f32 {
        let camera = &self.state.camera;
        input
            .cursor
            .and_then(|c| self.pick_at(c, input.size, scene))
            .map(|p| camera.depth_of(p))
            .filter(|d| *d > NEAR_PLANE)
            .unwrap_or_else(|| self.fallback_distance())
            .max(16.0)
    }

    /// Distance to the last pivot if it is in front of the camera, otherwise
    /// a fixed default.
    fn fallback_distance(&self) -> f32 {
        let depth = self.state.camera.depth_of(self.state.pivot);
        if depth > NEAR_PLANE * 16.0 && depth < MAX_PICK_DISTANCE {
            depth
        } else {
            FALLBACK_DEPTH
        }
    }

    fn pick_at(&self, cursor: Vec2, size: Vec2, scene: &dyn SceneQuery) -> Option<Vec3> {
        let ray = self.state.camera.ray_through(cursor, size);
        scene
            .pick(&ray)
            .filter(|p| p.is_finite() && p.distance(ray.origin) <= MAX_PICK_DISTANCE)
    }
}

/// Replaces non-finite or out-of-range input with harmless values.
fn clean_input(input: &ViewportInput) -> ViewportInput {
    let finite_or_zero = |v: Vec2| if v.is_finite() { v } else { Vec2::ZERO };
    let size = if input.size.is_finite() {
        input.size.max(Vec2::ONE)
    } else {
        Vec2::ONE
    };
    ViewportInput {
        size,
        cursor: input.cursor.filter(|c| c.is_finite()),
        right_delta: finite_or_zero(input.right_delta),
        middle_delta: finite_or_zero(input.middle_delta),
        scroll: if input.scroll.is_finite() {
            input.scroll.clamp(-2000.0, 2000.0)
        } else {
            0.0
        },
        dt: if input.dt.is_finite() {
            input.dt.clamp(0.0, MAX_DT)
        } else {
            0.0
        },
        ..*input
    }
}

fn sanitize_state(state: CameraState) -> CameraState {
    let (low, high) = FLY_SPEED_RANGE;
    let fly_speed = if state.fly_speed.is_finite() {
        state.fly_speed.clamp(low, high)
    } else {
        DEFAULT_FLY_SPEED
    };
    let pivot = if state.pivot.is_finite() {
        state.pivot
    } else {
        Vec3::ZERO
    };
    CameraState {
        camera: state.camera.sanitized(),
        pivot,
        fly_speed,
        ..state
    }
}

#[cfg(test)]
mod tests;
