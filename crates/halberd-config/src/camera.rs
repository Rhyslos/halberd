//! How the viewport camera feels: flying speed, and how fast it turns and
//! zooms.

use crate::settings::Adjustment;
use serde::{Deserialize, Serialize};

/// How the viewport camera feels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraSettings {
    /// Flying speed when Halberd starts, in units per second (the mouse
    /// wheel changes it while flying).
    pub fly_speed: u32,
    /// How fast dragging turns the 3D camera, in percent of normal.
    pub turn_speed_percent: u32,
    /// How far each turn of the mouse wheel zooms, in percent of normal.
    pub zoom_speed_percent: u32,
}

impl CameraSettings {
    /// Slowest flying speed, in units per second.
    pub const MIN_FLY_SPEED: u32 = 32;
    /// Fastest flying speed, in units per second.
    pub const MAX_FLY_SPEED: u32 = 16_384;
    /// Slowest turning or zooming, in percent.
    pub const MIN_PERCENT: u32 = 10;
    /// Fastest turning or zooming, in percent.
    pub const MAX_PERCENT: u32 = 400;

    /// Turning speed as a multiplier (1.0 is normal).
    pub fn turn_factor(&self) -> f32 {
        self.turn_speed_percent as f32 / 100.0
    }

    /// Zooming speed as a multiplier (1.0 is normal).
    pub fn zoom_factor(&self) -> f32 {
        self.zoom_speed_percent as f32 / 100.0
    }

    /// Brings every value within its limits, noting what changed.
    pub(crate) fn sanitize(&mut self, changes: &mut Vec<Adjustment>) {
        let mut fix = |setting: &'static str, value: &mut u32, low: u32, high: u32| {
            let allowed = (*value).clamp(low, high);
            if *value != allowed {
                changes.push(Adjustment {
                    setting,
                    found: value.to_string(),
                    used: allowed.to_string(),
                });
                *value = allowed;
            }
        };
        fix(
            "camera.fly_speed",
            &mut self.fly_speed,
            Self::MIN_FLY_SPEED,
            Self::MAX_FLY_SPEED,
        );
        fix(
            "camera.turn_speed_percent",
            &mut self.turn_speed_percent,
            Self::MIN_PERCENT,
            Self::MAX_PERCENT,
        );
        fix(
            "camera.zoom_speed_percent",
            &mut self.zoom_speed_percent,
            Self::MIN_PERCENT,
            Self::MAX_PERCENT,
        );
    }
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            fly_speed: 600,
            turn_speed_percent: 100,
            zoom_speed_percent: 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_normal_speed() {
        let camera = CameraSettings::default();
        assert_eq!(camera.fly_speed, 600);
        assert_eq!(camera.turn_factor(), 1.0);
        assert_eq!(camera.zoom_factor(), 1.0);
        let mut changes = Vec::new();
        let mut same = camera.clone();
        same.sanitize(&mut changes);
        assert!(changes.is_empty());
        assert_eq!(same, camera);
    }

    #[test]
    fn out_of_range_speeds_are_clamped_and_reported() {
        let mut camera = CameraSettings {
            fly_speed: 0,
            turn_speed_percent: u32::MAX,
            zoom_speed_percent: 5,
        };
        let mut changes = Vec::new();
        camera.sanitize(&mut changes);
        assert_eq!(camera.fly_speed, CameraSettings::MIN_FLY_SPEED);
        assert_eq!(camera.turn_speed_percent, CameraSettings::MAX_PERCENT);
        assert_eq!(camera.zoom_speed_percent, CameraSettings::MIN_PERCENT);
        assert_eq!(changes.len(), 3);
    }
}
