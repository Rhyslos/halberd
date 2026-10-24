//! The player figure: where it stands (beside what you are working on, or
//! under the pointer), whether a player fits there, and its label.

use super::{PLAYER_GAP, PaneView, ViewportPanel};
use glam::{Vec2, Vec3};
use halberd_config::LengthUnit;
use halberd_doc::Document;
use halberd_geom::Aabb;
use halberd_tools::{
    Clearance, Fit, PLAYER_CROUCH_HEIGHT, PLAYER_HEIGHT, clearance, floor_under, player_bounds,
};

/// What the player figure knows between frames.
#[derive(Debug, Default)]
pub(super) struct PlayerProbe {
    /// The figure stands where the pointer is in the 3D view.
    pub(super) follows: bool,
    /// Where it last stood under the pointer, and on which map.
    follow_feet: Option<(u64, Vec3)>,
    /// The last check (`None`: nothing worth checking), with where and on
    /// which map state it was made.
    checked: Option<((Vec3, u64, u64), Option<Clearance>)>,
}

impl ViewportPanel {
    /// Whether the player figure stands where the pointer is (in a 3D
    /// view) instead of beside what you are working on.
    pub fn player_follows(&self) -> bool {
        self.probe.follows
    }

    /// Makes the player figure stand where the pointer is, or not.
    pub fn set_player_follows(&mut self, follows: bool) {
        self.probe.follows = follows;
        if !follows {
            self.probe.follow_feet = None;
        }
    }

    /// Whether a player fits where the figure stands now, if it is shown
    /// somewhere worth checking (beside the selection or the box being
    /// drawn, or under the pointer; not at the origin with nothing there).
    pub fn player_clearance(&self) -> Option<Clearance> {
        self.player_at?;
        self.probe.checked.and_then(|(_, clearance)| clearance)
    }

    /// Places the player figure for this frame and checks whether a
    /// player fits there. `pointer` is where the pointer is in the active
    /// view (points from its top-left) and that view's size.
    pub(super) fn place_player(&mut self, doc: &Document, pointer: Option<(Vec2, Vec2)>) {
        if !self.show_player {
            self.player_at = None;
            return;
        }
        let instance = doc.instance();
        if self.probe.follows
            && self.panes[self.active].view == PaneView::Perspective
            && let Some((at, size)) = pointer
        {
            let ray = self.active_camera().ray_through(at, size);
            if let Some(feet) = floor_under(doc, &ray) {
                self.probe.follow_feet = Some((instance, feet));
            }
        }
        // A position from another map means nothing on this one.
        if self
            .probe
            .follow_feet
            .is_some_and(|(map, _)| map != instance)
        {
            self.probe.follow_feet = None;
        }
        let following = self
            .probe
            .follows
            .then_some(self.probe.follow_feet)
            .flatten();
        let (feet, worth_checking) = match following {
            Some((_, feet)) => (feet, true),
            None => {
                let camera = self.reference_camera();
                let preview = self.tools.preview();
                let something = preview.is_some() || !doc.selection().is_empty();
                (beside(preview, doc, camera.position), something)
            }
        };
        let key = (feet, instance, doc.revision());
        let fresh = self.probe.checked.is_some_and(|(k, _)| k == key);
        if !fresh {
            let check = worth_checking.then(|| clearance(doc, feet));
            self.probe.checked = Some((key, check));
        }
        // On a slope or a low step the figure stands where it would rest.
        let feet = self
            .probe
            .checked
            .and_then(|(_, c)| c)
            .filter(|c| c.fit != Fit::Blocked)
            .map_or(feet, |c| c.feet);
        self.player_at = Some(player_bounds(feet));
    }
}

/// Where the player figure stands when it doesn't follow the pointer:
/// beside the box being drawn, else beside the selection, on its floor
/// and on the side facing the camera; with neither, at the origin.
pub(super) fn beside(preview: Option<Aabb>, doc: &Document, camera: Vec3) -> Vec3 {
    let Some(b) = preview.or_else(|| doc.selection_bounds()) else {
        return Vec3::ZERO;
    };
    let reach = PLAYER_GAP + halberd_tools::PLAYER_WIDTH * 0.5;
    let c = b.center();
    let sides = [
        Vec3::new(b.max.x + reach, c.y, b.min.z),
        Vec3::new(b.min.x - reach, c.y, b.min.z),
        Vec3::new(c.x, b.max.y + reach, b.min.z),
        Vec3::new(c.x, b.min.y - reach, b.min.z),
    ];
    let distance = |p: &Vec3| p.truncate().distance_squared(camera.truncate());
    sides
        .into_iter()
        .min_by(|a, b| distance(a).total_cmp(&distance(b)))
        .unwrap_or(Vec3::ZERO)
}

/// The figure's label: its height, and whether a player fits there.
pub(super) fn label(unit: LengthUnit, clearance: Option<Clearance>) -> String {
    let height = unit.format(f64::from(PLAYER_HEIGHT));
    let Some(c) = clearance else {
        return format!("Player · {height}");
    };
    let room = |r: Option<f32>| match r {
        Some(r) => format!(", {} above", unit.format(f64::from(r))),
        None => String::new(),
    };
    match c.fit {
        Fit::Stands => format!("Player · {height}{}", room(c.headroom)),
        Fit::CrouchOnly => format!(
            "Crouch only · {}{}",
            unit.format(f64::from(PLAYER_CROUCH_HEIGHT)),
            room(c.headroom)
        ),
        Fit::Blocked => "No room for a player here".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_label_says_whether_a_player_fits() {
        let stands = Clearance {
            fit: Fit::Stands,
            headroom: Some(56.0),
            feet: Vec3::ZERO,
        };
        assert_eq!(
            label(LengthUnit::Units, Some(stands)),
            "Player · 72 u, 56 u above"
        );
        assert_eq!(label(LengthUnit::Units, None), "Player · 72 u");
        let crouch = Clearance {
            fit: Fit::CrouchOnly,
            headroom: Some(4.0),
            feet: Vec3::ZERO,
        };
        assert_eq!(
            label(LengthUnit::Feet, Some(crouch)),
            "Crouch only · 3.0 ft, 0.3 ft above"
        );
        let blocked = Clearance {
            fit: Fit::Blocked,
            headroom: None,
            feet: Vec3::ZERO,
        };
        assert_eq!(
            label(LengthUnit::Units, Some(blocked)),
            "No room for a player here"
        );
    }
}
