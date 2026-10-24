//! How a texture lies on a face: Hammer's texture axes (`uaxis` and
//! `vaxis`), read from a face's saved data.

use crate::FaceInfo;
use glam::Vec3;
use halberd_kv::Entry;

/// Hammer's usual texture scale: a texel is a quarter of a unit.
pub const DEFAULT_TEXTURE_SCALE: f32 = 0.25;

/// One texture axis: the direction texture coordinates run along, how far
/// they are shifted, and how many units one texel covers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextureAxis {
    /// The direction in the world (not necessarily unit length).
    pub direction: Vec3,
    /// Shift, in texels.
    pub offset: f32,
    /// Units per texel.
    pub scale: f32,
}

impl TextureAxis {
    /// The texel coordinate of a world point along this axis.
    pub fn texel(&self, point: Vec3) -> f32 {
        point.dot(self.direction) / self.scale + self.offset
    }

    /// Parses Hammer's form, `[x y z offset] scale`.
    pub fn parse(text: &str) -> Option<Self> {
        let (inside, rest) = text.trim().strip_prefix('[')?.split_once(']')?;
        let numbers: Vec<f32> = inside
            .split_whitespace()
            .map(|n| n.parse().ok())
            .collect::<Option<_>>()?;
        let [x, y, z, offset] = numbers[..] else {
            return None;
        };
        let scale: f32 = rest.trim().parse().ok()?;
        let direction = Vec3::new(x, y, z);
        let usable = direction.is_finite()
            && direction.length_squared() > 1e-12
            && offset.is_finite()
            && scale.is_finite()
            && scale.abs() > 1e-6;
        usable.then_some(Self {
            direction,
            offset,
            scale,
        })
    }
}

/// Both texture axes of a face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextureAxes {
    /// Across the texture.
    pub u: TextureAxis,
    /// Down the texture.
    pub v: TextureAxis,
}

impl TextureAxes {
    /// Hammer's world-aligned axes for a face with outward `normal`:
    /// textures run along the two world axes it is most nearly flat to.
    pub fn world_aligned(normal: Vec3) -> Self {
        let n = normal.abs();
        let (u, v) = if n.z >= n.x && n.z >= n.y {
            (Vec3::X, Vec3::NEG_Y)
        } else if n.x >= n.y {
            (Vec3::Y, Vec3::NEG_Z)
        } else {
            (Vec3::X, Vec3::NEG_Z)
        };
        let axis = |direction| TextureAxis {
            direction,
            offset: 0.0,
            scale: DEFAULT_TEXTURE_SCALE,
        };
        Self {
            u: axis(u),
            v: axis(v),
        }
    }

    /// The texel coordinates of a world point.
    pub fn texels(&self, point: Vec3) -> [f32; 2] {
        [self.u.texel(point), self.v.texel(point)]
    }
}

impl FaceInfo {
    /// How the texture lies on this face: as saved, or world-aligned for a
    /// face with outward `normal` if nothing usable was saved.
    pub fn texture_axes(&self, normal: Vec3) -> TextureAxes {
        let find = |key: &str| {
            self.file_data.iter().find_map(|e| match e {
                Entry::Pair(k, v) if k.eq_ignore_ascii_case(key) => TextureAxis::parse(v),
                _ => None,
            })
        };
        match (find("uaxis"), find("vaxis")) {
            (Some(u), Some(v)) => TextureAxes { u, v },
            _ => TextureAxes::world_aligned(normal),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hammer_axes_are_read_and_bad_ones_fall_back() {
        let axis = TextureAxis::parse("[0 1 0 16] 0.5").unwrap();
        assert_eq!(axis.direction, Vec3::Y);
        assert_eq!(axis.texel(Vec3::new(5.0, 10.0, 0.0)), 36.0);
        for bad in [
            "",
            "[0 1 0] 0.25",
            "[0 0 0 0] 0.25",
            "[1 0 0 0] 0",
            "[1 0 0 nan] 1",
            "1 0 0 0 0.25",
        ] {
            assert_eq!(TextureAxis::parse(bad), None, "{bad:?}");
        }
        let info = FaceInfo {
            file_data: vec![
                Entry::Pair("uaxis".into(), "[1 0 0 0] 0.25".into()),
                Entry::Pair("vaxis".into(), "[0 0 -1 8] 0.25".into()),
            ],
            ..FaceInfo::default()
        };
        assert_eq!(
            info.texture_axes(Vec3::Y).texels(Vec3::new(1.0, 0.0, 2.0)),
            [4.0, 0.0]
        );
        // Nothing saved: world-aligned, as Hammer would make it.
        let floor = FaceInfo::default().texture_axes(Vec3::Z);
        assert_eq!(floor.u.direction, Vec3::X);
        assert_eq!(floor.v.direction, Vec3::NEG_Y);
        let wall = FaceInfo::default().texture_axes(Vec3::X);
        assert_eq!((wall.u.direction, wall.v.direction), (Vec3::Y, Vec3::NEG_Z));
    }
}
