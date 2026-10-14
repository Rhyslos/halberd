//! Interaction tools: select, gizmo, shape tools, radial menus.
//!
//! Tools turn mouse and keyboard input into actions. They never draw panels
//! and never change the map document except by issuing commands.
//!
//! Implemented so far:
//!
//! - The viewport camera ([`Camera`]) and its controller
//!   ([`CameraController`]) with the three camera modes from the feature spec.
//! - The left-mouse tools ([`ToolController`]): Select (click, Ctrl+click)
//!   and Box (drag to draw a box brush, snapped to the grid).
//! - The transform gizmo ([`GizmoMode`]): move (W), rotate (R), scale (S)
//!   or all (T) the selection by dragging handles; each drag is one undo
//!   step.
//! - Part picking ([`SelectMode`]): in Vertex, Edge or Face mode (keys 2,
//!   3, 4; 1 returns to Object), clicks pick corners, edges or faces of the
//!   selected brushes, and the gizmo moves, rotates or scales them,
//!   reshaping the brushes ([`halberd_geom::Brush::with_moved_points`]).
//! - [`DocumentScene`]: what the camera and tools see of an open map.
//!
//! Input arrives as plain data ([`ViewportInput`], [`ToolInput`]), so
//! everything here can be tested without a window. Tools return a
//! [`ToolAction`] rather than changing the map themselves.

mod camera;
mod controller;
mod elements;
mod gizmo;
mod player;
mod scene;
mod tool;

pub use camera::{
    Camera, DEFAULT_FOV_Y, FAR_PLANE, MAX_PITCH, NEAR_PLANE, Ray, WORLD_LIMIT, WORLD_UP,
};
pub use controller::{
    CameraController, CameraMode, CameraState, DEFAULT_FLY_SPEED, FAST_MULTIPLIER, FLY_SPEED_RANGE,
    FlyKeys, GroundPlane, LOOK_SENSITIVITY, MAX_ORBIT_PIVOT_DISTANCE, MAX_PICK_DISTANCE,
    ORBIT_SENSITIVITY, SceneQuery, ViewportInput,
};
pub use elements::{
    ELEMENT_GRAB_POINTS, ElementOverlay, ElementShape, PickedElement, ScreenElement, SelectMode,
};
pub use gizmo::{
    Axis, GIZMO_ARM_POINTS, GIZMO_GRAB_POINTS, GizmoMode, GizmoShape, Handle, ROTATE_SNAP_DEGREES,
};
pub use player::{PLAYER_EYE_HEIGHT, PLAYER_HEIGHT, PLAYER_WIDTH, player_bounds};
pub use scene::DocumentScene;
pub use tool::{
    BOX_HEIGHT_RANGE, CLICK_SLOP, DEFAULT_BOX_HEIGHT, Tool, ToolAction, ToolController, ToolInput,
};
