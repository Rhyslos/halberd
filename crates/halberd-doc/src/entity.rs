//! Entity settings: reading and changing an entity's keys, its model, and
//! where it is and which way it faces.

use crate::{EntityObject, POINT_ENTITY_HALF_SIZE};
use glam::{EulerRot, Quat, Vec3};
use halberd_geom::Aabb;
use halberd_kv::Entry;

impl EntityObject {
    /// A new point entity of `classname` at `origin`, with `settings`
    /// (key and value pairs) after its class and origin.
    pub fn point(classname: &str, origin: Vec3, settings: &[(&str, &str)]) -> Self {
        let mut file_data = vec![
            Entry::Pair("classname".into(), classname.into()),
            Entry::Pair("origin".into(), format_vec3(origin)),
        ];
        file_data.extend(
            settings
                .iter()
                .map(|(k, v)| Entry::Pair((*k).to_string(), (*v).to_string())),
        );
        Self {
            classname: classname.to_string(),
            origin: Some(origin),
            file_data,
        }
    }

    /// The value of setting `key` (any case), if it has one.
    pub fn setting(&self, key: &str) -> Option<&str> {
        self.settings()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v)
    }

    /// Sets `key` to `value`, changing it where it is or adding it at the
    /// end of the settings.
    pub fn set_setting(&mut self, key: &str, value: &str) {
        let found = self.file_data.iter_mut().find_map(|e| match e {
            Entry::Pair(k, v) if k.eq_ignore_ascii_case(key) => Some(v),
            _ => None,
        });
        match found {
            Some(v) => *v = value.to_string(),
            None => {
                // Before the first block (Hammer writes settings first).
                let at = self
                    .file_data
                    .iter()
                    .position(|e| matches!(e, Entry::Block(_)))
                    .unwrap_or(self.file_data.len());
                self.file_data
                    .insert(at, Entry::Pair(key.to_string(), value.to_string()));
            }
        }
    }

    /// The model it shows (`models/props_c17/oildrum001.mdl`), if any.
    pub fn model(&self) -> Option<&str> {
        self.setting("model").filter(|m| !m.trim().is_empty())
    }

    /// Which way it faces: pitch, yaw and roll in degrees (`angles`; 0 0 0
    /// when missing or unreadable).
    pub fn angles(&self) -> Vec3 {
        self.setting("angles")
            .and_then(parse_vec3)
            .unwrap_or(Vec3::ZERO)
    }

    /// Its turn as a rotation, as Source applies `angles`: roll about X,
    /// then pitch about Y (positive tips the front down), then yaw about Z.
    pub fn rotation(&self) -> Quat {
        rotation_of(self.angles())
    }

    /// The same entity at `origin` (its setting too).
    pub fn with_origin(&self, origin: Vec3) -> Self {
        let mut moved = self.clone();
        moved.origin = Some(origin);
        moved.set_setting("origin", &format_vec3(origin));
        moved
    }

    /// The same entity turned to face `rotation`.
    pub fn with_rotation(&self, rotation: Quat) -> Self {
        let mut turned = self.clone();
        let (yaw, pitch, roll) = rotation.to_euler(EulerRot::ZYX);
        let degrees = Vec3::new(pitch, yaw, roll).map(f32::to_degrees);
        turned.set_setting("angles", &format_vec3(degrees));
        turned
    }

    /// The box it fills: its model's box (`model_box`, in the model's own
    /// space) turned and placed at its origin, or the small marker box.
    pub fn placed_box(&self, model_box: Option<Aabb>) -> Option<Aabb> {
        let origin = self.origin?;
        let Some(local) = model_box else {
            return self.marker();
        };
        let rotation = self.rotation();
        let corners = (0..8).map(|i| {
            let pick = |bit: usize, axis: usize| {
                if i & bit == 0 {
                    local.min[axis]
                } else {
                    local.max[axis]
                }
            };
            origin + rotation * Vec3::new(pick(1, 0), pick(2, 1), pick(4, 2))
        });
        let mut placed = Aabb::from_corners(origin, origin);
        for corner in corners {
            placed = placed.union(Aabb::from_corners(corner, corner));
        }
        // A model too small to click is given the marker's size.
        let min_half = Vec3::splat(POINT_ENTITY_HALF_SIZE / 4.0);
        if (placed.max - placed.min).min_element() < 1.0 {
            placed = placed.union(Aabb::from_corners(origin - min_half, origin + min_half));
        }
        Some(placed)
    }
}

/// The rotation for Source `angles` (pitch, yaw, roll in degrees).
pub fn rotation_of(angles: Vec3) -> Quat {
    let r = angles.map(f32::to_radians);
    Quat::from_rotation_z(r.y) * Quat::from_rotation_y(r.x) * Quat::from_rotation_x(r.z)
}

/// "x y z" as Hammer writes it: whole numbers plainly, others with up to
/// three decimals.
pub(crate) fn format_vec3(v: Vec3) -> String {
    let one = |x: f32| {
        let x = if x.abs() < 0.0005 { 0.0 } else { x };
        if (x - x.round()).abs() < 0.0005 {
            format!("{}", x.round() as i64)
        } else {
            let text = format!("{x:.3}");
            text.trim_end_matches('0').trim_end_matches('.').to_string()
        }
    };
    format!("{} {} {}", one(v.x), one(v.y), one(v.z))
}

fn parse_vec3(text: &str) -> Option<Vec3> {
    let mut parts = text.split_whitespace().map(|p| p.parse::<f32>().ok());
    let v = Vec3::new(parts.next()??, parts.next()??, parts.next()??);
    v.is_finite().then_some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_prop_writes_its_settings_as_hammer_does() {
        let prop = EntityObject::point(
            "prop_static",
            Vec3::new(12.0, -3.5, 0.0),
            &[("model", "models/props_c17/oildrum001.mdl"), ("angles", "0 90 0")],
        );
        let pairs: Vec<(&str, &str)> = prop.settings().collect();
        assert_eq!(
            pairs,
            [
                ("classname", "prop_static"),
                ("origin", "12 -3.5 0"),
                ("model", "models/props_c17/oildrum001.mdl"),
                ("angles", "0 90 0"),
            ]
        );
        assert_eq!(prop.model(), Some("models/props_c17/oildrum001.mdl"));
        assert_eq!(prop.angles(), Vec3::new(0.0, 90.0, 0.0));
    }

    #[test]
    fn moving_and_turning_update_the_settings() {
        let prop = EntityObject::point("prop_static", Vec3::ZERO, &[]);
        let moved = prop.with_origin(Vec3::new(64.0, 0.25, -8.0));
        assert_eq!(moved.setting("ORIGIN"), Some("64 0.25 -8"));
        assert_eq!(moved.origin, Some(Vec3::new(64.0, 0.25, -8.0)));
        let turned = moved.with_rotation(rotation_of(Vec3::new(30.0, 45.0, 10.0)));
        let angles = turned.angles();
        assert!((angles - Vec3::new(30.0, 45.0, 10.0)).abs().max_element() < 0.01, "{angles}");
        // Yaw 90 turns the model's front (X) to face Y; pitch tips it down.
        let front = rotation_of(Vec3::new(0.0, 90.0, 0.0)) * Vec3::X;
        assert!((front - Vec3::Y).length() < 1e-5);
        let down = rotation_of(Vec3::new(90.0, 0.0, 0.0)) * Vec3::X;
        assert!((down - Vec3::NEG_Z).length() < 1e-5);
    }

    #[test]
    fn a_placed_model_fills_its_turned_box() {
        let local = Aabb::from_corners(Vec3::new(-10.0, -5.0, 0.0), Vec3::new(10.0, 5.0, 40.0));
        let prop = EntityObject::point(
            "prop_static",
            Vec3::new(100.0, 0.0, 0.0),
            &[("angles", "0 90 0")],
        );
        let placed = prop.placed_box(Some(local)).unwrap();
        assert!((placed.min - Vec3::new(95.0, -10.0, 0.0)).abs().max_element() < 1e-4);
        assert!((placed.max - Vec3::new(105.0, 10.0, 40.0)).abs().max_element() < 1e-4);
        assert_eq!(prop.placed_box(None), prop.marker());
        let mut bad = prop.clone();
        bad.set_setting("angles", "nonsense");
        assert_eq!(bad.angles(), Vec3::ZERO);
    }
}
