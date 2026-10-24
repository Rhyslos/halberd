//! Keeps the camera's input and state usable: values that aren't finite or
//! are out of range are replaced with harmless ones.

use super::{
    Camera, CameraState, DEFAULT_FLY_SPEED, FLY_SPEED_RANGE, MAX_DT, Vec2, Vec3, ViewportInput,
};

/// Replaces non-finite or out-of-range input with harmless values.
pub(super) fn clean_input(input: &ViewportInput) -> ViewportInput {
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

/// `camera` as a perspective camera.
fn perspective(camera: Camera) -> Camera {
    Camera {
        ortho: None,
        ..camera
    }
}

pub(super) fn sanitize_state(state: CameraState) -> CameraState {
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
    // The controller moves a perspective camera; a flat one (from a damaged
    // settings file) would draw flat while moving in perspective.
    let camera = perspective(state.camera.sanitized());
    CameraState {
        camera,
        pivot,
        fly_speed,
        ..state
    }
}
