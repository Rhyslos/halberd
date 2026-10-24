//! The six flat side views (Top, Bottom, Front, Back, Left and Right), as
//! in Hammer's 2D views: no perspective, looking straight along one axis.
//! Either mouse button that moves the camera (right or middle) drags the
//! view along; scrolling zooms in and out around the pointer.

use crate::camera::{Camera, DEFAULT_FOV_Y, Ray};
use crate::controller::ViewportInput;
use glam::{Vec2, Vec3};
use halberd_geom::Aabb;
use serde::{Deserialize, Serialize};
use std::f32::consts::{FRAC_PI_2, PI};

/// How far behind what it shows a side view's camera stands, so the whole
/// map is in front of it (and, with the centre kept within
/// [`SIDE_VIEW_REACH`], inside the far plane).
const BACK_OFF: f32 = 100_000.0;
/// How far from the origin a side view's centre may go, on every axis: past
/// the edge of the largest Source maps (±32768).
pub const SIDE_VIEW_REACH: f32 = 32_768.0;
/// How far beyond the view's centre its grid is drawn, so it lies behind
/// everything in the map (twice [`SIDE_VIEW_REACH`], and a little more).
pub const GRID_BEHIND: f32 = 70_000.0;
/// Least and most a side view can show from top to bottom, in units.
pub const SIDE_VIEW_HEIGHT_RANGE: (f32, f32) = (16.0, 131_072.0);
/// How strongly scrolling zooms: fraction per point scrolled.
const ZOOM_RATE: f32 = 0.003;

/// Which way a side view looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    /// From above, looking down (X right, Y up), Hammer's Top (X/Y).
    Top,
    /// From below, looking up (X right, Y down).
    Bottom,
    /// Looking along -X (Y right, Z up), Hammer's Front (Y/Z).
    Front,
    /// Looking along +X (Y left, Z up).
    Back,
    /// Looking along -Y (X left, Z up).
    Left,
    /// Looking along +Y (X right, Z up), Hammer's Side (X/Z).
    Right,
}

impl Side {
    /// Every side, in menu order.
    pub const ALL: [Side; 6] = [
        Self::Top,
        Self::Front,
        Self::Right,
        Self::Bottom,
        Self::Back,
        Self::Left,
    ];

    /// Name for the interface, with the axes it shows across and up.
    pub fn label(self) -> &'static str {
        match self {
            Self::Top => "Top (X/Y)",
            Self::Bottom => "Bottom (X/Y)",
            Self::Front => "Front (Y/Z)",
            Self::Back => "Back (Y/Z)",
            Self::Left => "Left (X/Z)",
            Self::Right => "Right (X/Z)",
        }
    }

    /// The camera's turn and tilt (see [`Camera`]).
    fn angles(self) -> (f32, f32) {
        match self {
            Self::Top => (FRAC_PI_2, -FRAC_PI_2),
            Self::Bottom => (FRAC_PI_2, FRAC_PI_2),
            Self::Front => (PI, 0.0),
            Self::Back => (0.0, 0.0),
            Self::Left => (-FRAC_PI_2, 0.0),
            Self::Right => (FRAC_PI_2, 0.0),
        }
    }

    /// The world axis this view looks along (0 X, 1 Y, 2 Z).
    pub fn depth_axis(self) -> usize {
        match self {
            Self::Top | Self::Bottom => 2,
            Self::Front | Self::Back => 0,
            Self::Left | Self::Right => 1,
        }
    }

    /// True for the views that look straight up or down.
    pub fn is_horizontal(self) -> bool {
        self.depth_axis() == 2
    }
}

/// A side view's camera: which way it looks, the point in the middle of
/// it, and how much it shows.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SideCamera {
    /// Which way it looks.
    pub side: Side,
    /// The point shown in the middle.
    pub center: Vec3,
    /// How many units it shows from top to bottom.
    pub height: f32,
}

impl SideCamera {
    /// A view of the origin from `side`, 2048 units tall.
    pub fn new(side: Side) -> Self {
        Self {
            side,
            center: Vec3::ZERO,
            height: 2048.0,
        }
    }

    /// The camera to draw and pick with: flat, standing far back.
    pub fn camera(&self) -> Camera {
        let (yaw, pitch) = self.side.angles();
        let mut camera = Camera {
            position: Vec3::ZERO,
            yaw,
            pitch,
            fov_y: DEFAULT_FOV_Y,
            ortho: Some(self.height * 0.5),
        };
        camera.position = self.center - camera.forward() * BACK_OFF;
        camera
    }

    /// The ray through a point of a viewport `size` big.
    pub fn ray_through(&self, point: Vec2, size: Vec2) -> Ray {
        self.camera().ray_through(point, size)
    }

    /// Where the view's grid is drawn: the axis it is flat across and its
    /// position along that axis, behind everything shown.
    pub fn grid_plane(&self) -> (usize, f32) {
        let camera = self.camera();
        let axis = self.side.depth_axis();
        let behind = self.center + camera.forward() * GRID_BEHIND;
        (axis, behind[axis])
    }

    /// Applies one frame of input: right or middle drag moves the view,
    /// scrolling zooms around the pointer. Returns true if it changed.
    pub fn update(&mut self, input: &ViewportInput) -> bool {
        let before = *self;
        let size = input.size.max(Vec2::ONE);
        let camera = self.camera();
        let per_point = self.height / size.y;
        let drag = if input.right_held {
            input.right_delta
        } else if input.middle_held {
            input.middle_delta
        } else {
            Vec2::ZERO
        };
        if drag.is_finite() && drag != Vec2::ZERO {
            self.center += (-camera.right() * drag.x + camera.up() * drag.y) * per_point;
        }
        if input.scroll.is_finite() && input.scroll != 0.0 {
            let factor = (-input.scroll * ZOOM_RATE).exp();
            let height =
                (self.height * factor).clamp(SIDE_VIEW_HEIGHT_RANGE.0, SIDE_VIEW_HEIGHT_RANGE.1);
            // The point under the pointer stays under it.
            if let Some(cursor) = input.cursor.filter(|c| c.is_finite()) {
                let from_middle = cursor - size * 0.5;
                let shift = from_middle * (per_point - height / size.y);
                self.center += camera.right() * shift.x - camera.up() * shift.y;
            }
            self.height = height;
        }
        *self = self.sanitized();
        *self != before
    }

    /// Shows the whole of `bounds`, with a margin.
    pub fn frame(&mut self, bounds: Aabb) {
        let center = bounds.center();
        let size = bounds.size();
        if !center.is_finite() || !size.is_finite() {
            return;
        }
        let camera = self.camera();
        let across = size
            .dot(camera.right().abs())
            .max(size.dot(camera.up().abs()));
        self.center = center;
        self.height = (across * 1.2).max(256.0);
        *self = self.sanitized();
    }

    /// A usable view whatever was saved.
    pub fn sanitized(self) -> Self {
        Self {
            side: self.side,
            center: if self.center.is_finite() {
                self.center
                    .clamp(Vec3::splat(-SIDE_VIEW_REACH), Vec3::splat(SIDE_VIEW_REACH))
            } else {
                Vec3::ZERO
            },
            height: if self.height.is_finite() {
                self.height
                    .clamp(SIDE_VIEW_HEIGHT_RANGE.0, SIDE_VIEW_HEIGHT_RANGE.1)
            } else {
                2048.0
            },
        }
    }
}

#[cfg(test)]
mod tests;
