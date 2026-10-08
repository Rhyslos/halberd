//! Interaction tools: select, gizmo, shape tools, radial menus.
//!
//! Tools turn mouse and keyboard input into actions. They never draw panels
//! and never change the map document except by issuing commands.
//!
//! Implemented so far: the viewport camera ([`Camera`]) and its controller
//! ([`CameraController`]) with the three camera modes from the feature spec.
//! Input arrives as a plain [`ViewportInput`], so the camera can be tested
//! without a window.

mod camera;
mod controller;

pub use camera::{
    Camera, DEFAULT_FOV_Y, FAR_PLANE, MAX_PITCH, NEAR_PLANE, Ray, WORLD_LIMIT, WORLD_UP,
};
pub use controller::{
    CameraController, CameraMode, CameraState, DEFAULT_FLY_SPEED, FAST_MULTIPLIER, FLY_SPEED_RANGE,
    FlyKeys, GroundPlane, LOOK_SENSITIVITY, MAX_ORBIT_PIVOT_DISTANCE, MAX_PICK_DISTANCE,
    ORBIT_SENSITIVITY, SceneQuery, ViewportInput,
};
