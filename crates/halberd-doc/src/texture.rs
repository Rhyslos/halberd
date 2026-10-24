//! How a texture lies on a face: Hammer's texture axes (`uaxis` and
//! `vaxis`), read from a face's saved data.

use crate::FaceInfo;
use glam::Vec3;
use halberd_kv::Entry;

/// Hammer's usual texture scale: a texel is a quarter of a unit.
pub const DEFAULT_TEXTURE_SCALE: f32 = 0.25;
/// Smallest texture scale taken as real (units per texel); smaller ones in
/// a file are damaged and ignored.
const MIN_SCALE: f32 = 1e-3;
/// Largest texture shift taken as real, in texels.
const MAX_OFFSET: f32 = 1e6;

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
        // At most five, so a huge damaged value costs nothing.
        let numbers: Vec<f32> = inside
            .split_whitespace()
            .take(5)
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
            && offset.abs() <= MAX_OFFSET
            && scale.is_finite()
            && scale.abs() >= MIN_SCALE;
        usable.then_some(Self {
            direction,
            offset,
            scale,
        })
    }

    /// Hammer's form, `[x y z offset] scale`.
    pub fn to_hammer(&self) -> String {
        // -0 would print as "-0".
        let n = |v: f32| if v == 0.0 { 0.0 } else { v };
        let d = self.direction;
        format!(
            "[{} {} {} {}] {}",
            n(d.x),
            n(d.y),
            n(d.z),
            n(self.offset),
            n(self.scale)
        )
    }

    /// True if the axis runs across a face with outward `normal` (not
    /// straight out of it, which would smear the texture into streaks).
    fn fits(&self, normal: Vec3) -> bool {
        self.direction.normalize_or_zero().dot(normal).abs() < 0.99
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

    /// True if both axes run across a face with outward `normal`, and not
    /// along each other.
    pub fn fits(&self, normal: Vec3) -> bool {
        let (u, v) = (
            self.u.direction.normalize_or_zero(),
            self.v.direction.normalize_or_zero(),
        );
        self.u.fits(normal) && self.v.fits(normal) && u.cross(v).length() > 0.01
    }

    /// The texel coordinates of a world point.
    pub fn texels(&self, point: Vec3) -> [f32; 2] {
        [self.u.texel(point), self.v.texel(point)]
    }
}

impl FaceInfo {
    /// The texture axes saved with this face, if they can be read.
    pub fn saved_texture_axes(&self) -> Option<TextureAxes> {
        let find = |key: &str| {
            self.file_data.iter().find_map(|e| match e {
                Entry::Pair(k, v) if k.eq_ignore_ascii_case(key) => TextureAxis::parse(v),
                _ => None,
            })
        };
        Some(TextureAxes {
            u: find("uaxis")?,
            v: find("vaxis")?,
        })
    }

    /// How the texture lies on this face (outward `normal`): as saved, or
    /// world-aligned if nothing usable was saved. Saved axes that no longer
    /// fit the face (it was turned, or its look was copied from a face
    /// facing another way) are replaced by world-aligned ones keeping their
    /// scale; saving writes the same (see [`FaceInfo::refitted_texture`]).
    pub fn texture_axes(&self, normal: Vec3) -> TextureAxes {
        match self.saved_texture_axes() {
            Some(axes) if axes.fits(normal) => axes,
            Some(axes) => refit(&axes, normal),
            None => TextureAxes::world_aligned(normal),
        }
    }

    /// If the saved texture axes no longer fit a face with outward `normal`,
    /// the world-aligned axes (keeping their scale) to save instead.
    pub fn refitted_texture(&self, normal: Vec3) -> Option<TextureAxes> {
        let axes = self.saved_texture_axes()?;
        (!axes.fits(normal)).then(|| refit(&axes, normal))
    }
}

/// World-aligned axes for `normal`, keeping the scales of `axes`.
fn refit(axes: &TextureAxes, normal: Vec3) -> TextureAxes {
    let mut fixed = TextureAxes::world_aligned(normal);
    fixed.u.scale = axes.u.scale;
    fixed.v.scale = axes.v.scale;
    fixed
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
            "[1 0 0 0] 0.0000001",
            "[1 0 0 1e12] 1",
            "[1 0 0 0 5] 0.25",
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

    #[test]
    fn negative_scales_and_long_axes_work_and_hammer_text_round_trips() {
        let axis = TextureAxis::parse("[2 0 0 0] -0.5").unwrap();
        assert_eq!(axis.texel(Vec3::new(1.0, 0.0, 0.0)), -4.0);
        for text in [
            "[1 0 0 0] 0.25",
            "[0 -1 0 16] 0.5",
            "[0 0 -1 0] 0.25",
            "[0.707 0.707 0 3.5] 1",
        ] {
            assert_eq!(TextureAxis::parse(text).unwrap().to_hammer(), text);
        }
        let world = TextureAxes::world_aligned(Vec3::Y);
        assert_eq!(world.u.to_hammer(), "[1 0 0 0] 0.25");
        assert_eq!(world.v.to_hammer(), "[0 0 -1 0] 0.25");
    }

    #[test]
    fn axes_that_no_longer_fit_the_face_are_refitted_keeping_their_scale() {
        // A wall's axes copied onto a floor: v runs straight out of it.
        let info = FaceInfo {
            file_data: vec![
                Entry::Pair("uaxis".into(), "[0 1 0 0] 0.5".into()),
                Entry::Pair("vaxis".into(), "[0 0 -1 0] 0.5".into()),
            ],
            ..FaceInfo::default()
        };
        assert_eq!(info.refitted_texture(Vec3::X), None, "fits the wall");
        let fixed = info.refitted_texture(Vec3::Z).unwrap();
        assert_eq!(fixed.u.to_hammer(), "[1 0 0 0] 0.5");
        assert_eq!(fixed.v.to_hammer(), "[0 -1 0 0] 0.5");
        assert_eq!(info.texture_axes(Vec3::Z), fixed);
        // u along v.
        let same = FaceInfo {
            file_data: vec![
                Entry::Pair("uaxis".into(), "[1 0 0 0] 0.25".into()),
                Entry::Pair("vaxis".into(), "[1 0 0 0] 0.25".into()),
            ],
            ..FaceInfo::default()
        };
        assert!(same.refitted_texture(Vec3::Z).is_some());
    }

    #[test]
    fn fuzz_random_axis_text_never_crashes() {
        let pieces = [
            "[", "]", " ", "1", "-", "0", ".", "e", "9", "nan", "inf", "x", "1e38",
        ];
        let mut seed: u64 = 0x7E57_0000_0000_0001;
        let mut next = move |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        for _ in 0..20_000 {
            let text: String = (0..next(16)).map(|_| pieces[next(pieces.len())]).collect();
            if let Some(axis) = TextureAxis::parse(&text) {
                assert!(axis.scale.abs() >= MIN_SCALE && axis.offset.is_finite());
                let _ = axis.texel(Vec3::splat(16_384.0));
            }
        }
    }
}
