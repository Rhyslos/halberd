//! Displacements: a four-sided brush face turned into a grid of points that
//! can be raised and lowered (terrain, cliffs, rubble), with a blend
//! amount at each point for mixing two textures.
//!
//! A map file keeps one in the face's `dispinfo` block: its `power` (the
//! grid has 2^power + 1 points a side: 5, 9 or 17), the `startposition`
//! (the face corner the grid starts from), and rows of numbers for each
//! point: `normals` (the direction it moves) times `distances` (how far),
//! plus `offsets`, and `alphas` (0 to 255, the blend). Everything else in
//! the block is kept as read.

use glam::Vec3;
use halberd_kv::{Block, Entry};

mod follow;
pub(crate) use follow::follow_shape;

/// Smallest and largest grid powers Source accepts.
pub const DISP_POWERS: std::ops::RangeInclusive<u8> = 2..=4;
/// How far a point may move from the face, in units (well inside the map).
const MAX_DISTANCE: f32 = 32_768.0;

/// One displacement's grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Displacement {
    /// 2, 3 or 4: the grid has 2^power + 1 points a side.
    pub power: u8,
    /// The face corner the grid starts from.
    pub start: Vec3,
    /// Raises the whole surface along the face's normal, in units.
    pub elevation: f32,
    /// Each point's direction of movement (row by row).
    pub normals: Vec<Vec3>,
    /// How far each point moves along its direction.
    pub distances: Vec<f32>,
    /// A further offset for each point.
    pub offsets: Vec<Vec3>,
    /// Each point's blend between the face's two textures, 0 to 255.
    pub alphas: Vec<f32>,
    /// What else the `dispinfo` block holds (flags, triangle tags…), kept.
    extra: Vec<Entry>,
}

impl Displacement {
    /// A flat displacement of `power` on a face facing `normal`, starting
    /// at corner `start`.
    pub fn flat(power: u8, start: Vec3, normal: Vec3) -> Self {
        let power = power.clamp(*DISP_POWERS.start(), *DISP_POWERS.end());
        let count = points_for(power);
        Self {
            power,
            start,
            elevation: 0.0,
            normals: vec![normal; count],
            distances: vec![0.0; count],
            offsets: vec![Vec3::ZERO; count],
            alphas: vec![0.0; count],
            extra: vec![
                Entry::Pair("flags".into(), "0".into()),
                Entry::Pair("subdiv".into(), "0".into()),
            ],
        }
    }

    /// Points along one side of the grid.
    pub fn side(&self) -> usize {
        (1usize << self.power) + 1
    }

    /// Reads a `dispinfo` block. `None` if it isn't a usable one.
    pub fn from_block(block: &Block) -> Option<Self> {
        let power: u8 = block.get("power")?.trim().parse().ok()?;
        if !DISP_POWERS.contains(&power) {
            return None;
        }
        let side = (1usize << power) + 1;
        let start = parse_bracketed(block.get("startposition")?)?;
        let elevation = block
            .get("elevation")
            .and_then(|e| e.trim().parse().ok())
            .filter(|e: &f32| e.is_finite())
            .unwrap_or(0.0);
        let normals = rows(block, "normals", side, 3).map(to_vectors)?;
        let distances = rows(block, "distances", side, 1)?;
        let offsets = rows(block, "offsets", side, 3)
            .map(to_vectors)
            .unwrap_or_else(|| vec![Vec3::ZERO; side * side]);
        let alphas = rows(block, "alphas", side, 1).unwrap_or_else(|| vec![0.0; side * side]);
        let read = [
            "power",
            "startposition",
            "elevation",
            "normals",
            "distances",
            "offsets",
            "alphas",
        ];
        let extra = block
            .entries
            .iter()
            .filter(|e| {
                let name = match e {
                    Entry::Pair(k, _) => k,
                    Entry::Block(b) => &b.name,
                };
                !read.iter().any(|r| r.eq_ignore_ascii_case(name))
            })
            .cloned()
            .collect();
        Some(Self {
            power,
            start,
            elevation,
            normals,
            distances,
            offsets,
            alphas,
            extra,
        })
    }

    /// The `dispinfo` block for this displacement.
    pub fn to_block(&self) -> Block {
        let side = self.side();
        let mut block = Block::new("dispinfo");
        block.push_pair("power", self.power.to_string());
        block.push_pair(
            "startposition",
            format!(
                "[{} {} {}]",
                num(self.start.x),
                num(self.start.y),
                num(self.start.z)
            ),
        );
        block.push_pair("elevation", num(self.elevation));
        let mut extra = self.extra.clone();
        // Pairs first (flags, subdiv), as Hammer writes them.
        for entry in extra.iter().filter(|e| matches!(e, Entry::Pair(..))) {
            block.entries.push(entry.clone());
        }
        extra.retain(|e| matches!(e, Entry::Block(_)));
        block.push_block(vector_rows("normals", &self.normals, side));
        block.push_block(number_rows("distances", &self.distances, side));
        block.push_block(vector_rows("offsets", &self.offsets, side));
        let has_offset_normals = extra
            .iter()
            .any(|e| matches!(e, Entry::Block(b) if b.name.eq_ignore_ascii_case("offset_normals")));
        if !has_offset_normals {
            block.push_block(vector_rows("offset_normals", &self.normals, side));
        }
        block.push_block(number_rows("alphas", &self.alphas, side));
        block.entries.extend(extra);
        block
    }

    /// The grid's points, row by row, on a face whose corners are
    /// `corners`, in Source's order starting from the start corner (see
    /// [`ordered_corners`]). `None` if the face hasn't four corners.
    pub fn points(&self, corners: &[Vec3]) -> Option<Vec<Vec3>> {
        let [c0, c1, c2, c3] = <[Vec3; 4]>::try_from(corners).ok()?;
        let side = self.side();
        let normal = (c3 - c0).cross(c1 - c0).normalize_or_zero();
        let step = 1.0 / (side - 1) as f32;
        let mut out = Vec::with_capacity(side * side);
        for row in 0..side {
            let t = row as f32 * step;
            let left = c0.lerp(c1, t);
            let right = c3.lerp(c2, t);
            for col in 0..side {
                let k = row * side + col;
                let base = left.lerp(right, col as f32 * step);
                let along = self.normals.get(k).copied().unwrap_or(Vec3::ZERO);
                let distance = self.distances.get(k).copied().unwrap_or(0.0);
                let offset = self.offsets.get(k).copied().unwrap_or(Vec3::ZERO);
                out.push(base + along * distance + offset + normal * self.elevation);
            }
        }
        Some(out)
    }

    /// The grid's points before displacing (flat on the face), row by
    /// row: where textures are laid from.
    pub fn base_points(&self, corners: &[Vec3]) -> Option<Vec<Vec3>> {
        let [c0, c1, c2, c3] = <[Vec3; 4]>::try_from(corners).ok()?;
        let side = self.side();
        let step = 1.0 / (side - 1) as f32;
        let mut out = Vec::with_capacity(side * side);
        for row in 0..side {
            let t = row as f32 * step;
            let (left, right) = (c0.lerp(c1, t), c3.lerp(c2, t));
            for col in 0..side {
                out.push(left.lerp(right, col as f32 * step));
            }
        }
        Some(out)
    }

    /// Moves point `k` by `amount` along `face_normal` (the face's outward
    /// direction; negative lowers), kept within limits.
    ///
    /// A point's movement is stored as Hammer stores it: a unit direction
    /// times a distance that is never negative (a lowered point points
    /// down), so the direction is worked out again from the movement.
    pub fn raise(&mut self, k: usize, amount: f32, face_normal: Vec3) {
        if !amount.is_finite() {
            return;
        }
        let (Some(n), Some(d)) = (self.normals.get_mut(k), self.distances.get_mut(k)) else {
            return;
        };
        let moved = (*n * *d + face_normal * amount).clamp_length_max(MAX_DISTANCE);
        let length = moved.length();
        if length.is_finite() && length > 1e-4 {
            (*n, *d) = (moved / length, length);
        } else {
            (*n, *d) = (face_normal, 0.0);
        }
    }

    /// How far point `k` is moved out of the face along `face_normal`
    /// (negative: into it).
    pub fn height(&self, k: usize, face_normal: Vec3) -> f32 {
        match (self.normals.get(k), self.distances.get(k)) {
            (Some(n), Some(d)) => (*n * *d).dot(face_normal),
            _ => 0.0,
        }
    }

    /// Forgets what Hammer works out from the shape (which triangles are
    /// walkable), as it no longer fits once the shape changes; Hammer and
    /// the compiler work it out again.
    pub fn forget_derived(&mut self) {
        self.extra.retain(
            |e| !matches!(e, Entry::Block(b) if b.name.eq_ignore_ascii_case("triangle_tags")),
        );
    }

    /// Sets point `k`'s blend (kept between 0 and 255).
    pub fn set_alpha(&mut self, k: usize, alpha: f32) {
        if let Some(a) = self.alphas.get_mut(k) {
            *a = alpha.clamp(0.0, 255.0);
        }
    }

    /// The same displacement, moved by `offset` (its start corner moves
    /// with the face).
    pub fn translated(&self, offset: Vec3) -> Self {
        Self {
            start: self.start + offset,
            ..self.clone()
        }
    }
}

impl crate::FaceInfo {
    /// True if the face has a displacement block, even one that can't be
    /// read.
    pub fn has_dispinfo(&self) -> bool {
        self.file_data
            .iter()
            .any(|e| matches!(e, Entry::Block(b) if b.name.eq_ignore_ascii_case("dispinfo")))
    }

    /// The face's displacement, if it has a usable one.
    pub fn displacement(&self) -> Option<Displacement> {
        self.file_data.iter().find_map(|e| match e {
            Entry::Block(b) if b.name.eq_ignore_ascii_case("dispinfo") => {
                Displacement::from_block(b)
            }
            _ => None,
        })
    }

    /// The same face with `displacement` in place of the one it had
    /// (`None`: a plain face again).
    pub fn with_displacement(&self, displacement: Option<&Displacement>) -> Self {
        let mut info = self.clone();
        let at = info
            .file_data
            .iter()
            .position(|e| matches!(e, Entry::Block(b) if b.name.eq_ignore_ascii_case("dispinfo")));
        match (at, displacement) {
            (Some(at), Some(d)) => info.file_data[at] = Entry::Block(d.to_block()),
            (Some(at), None) => {
                info.file_data.remove(at);
            }
            (None, Some(d)) => info.file_data.push(Entry::Block(d.to_block())),
            (None, None) => {}
        }
        info
    }
}

/// Points in a grid of `power`.
fn points_for(power: u8) -> usize {
    let side = (1usize << power) + 1;
    side * side
}

/// A face's corners in the order a displacement is laid on them: starting
/// at the corner nearest `start`, going the way Source winds faces
/// (clockwise seen from outside, against `normal`).
pub fn ordered_corners(corners: &[Vec3], normal: Vec3, start: Vec3) -> Option<[Vec3; 4]> {
    let mut c = <[Vec3; 4]>::try_from(corners).ok()?;
    // Clockwise seen from outside: the turn from one edge to the next
    // points into the brush.
    if (c[1] - c[0]).cross(c[2] - c[1]).dot(normal) > 0.0 {
        c.reverse();
    }
    let first = (0..4).min_by(|a, b| {
        c[*a]
            .distance_squared(start)
            .total_cmp(&c[*b].distance_squared(start))
    })?;
    c.rotate_left(first);
    Some(c)
}

/// "[x y z]" as a point.
fn parse_bracketed(text: &str) -> Option<Vec3> {
    let inner = text.trim().trim_start_matches('[').trim_end_matches(']');
    let v: Vec<f32> = inner
        .split_whitespace()
        .map(|p| p.parse().ok().filter(|v: &f32| v.is_finite()))
        .collect::<Option<_>>()?;
    match v.as_slice() {
        [x, y, z] => Some(Vec3::new(*x, *y, *z)),
        _ => None,
    }
}

/// The numbers of block `name`'s rows `row0`…, each `side` × `per` long.
fn rows(block: &Block, name: &str, side: usize, per: usize) -> Option<Vec<f32>> {
    let rows = block.blocks(name).next()?;
    let mut out = Vec::with_capacity(side * side * per);
    for r in 0..side {
        let row = rows.get(&format!("row{r}"))?;
        let values: Vec<f32> = row
            .split_whitespace()
            .map(|v| v.parse().ok().filter(|v: &f32| v.is_finite()))
            .collect::<Option<_>>()?;
        if values.len() != side * per {
            return None;
        }
        out.extend(values);
    }
    Some(out)
}

fn to_vectors(values: Vec<f32>) -> Vec<Vec3> {
    values
        .chunks_exact(3)
        .map(|c| Vec3::new(c[0], c[1], c[2]))
        .collect()
}

/// A number as Hammer writes it.
fn num(v: f32) -> String {
    if v == 0.0 { "0".into() } else { format!("{v}") }
}

fn vector_rows(name: &str, values: &[Vec3], side: usize) -> Block {
    let mut block = Block::new(name);
    for (r, row) in values.chunks(side).enumerate().take(side) {
        let text: Vec<String> = row
            .iter()
            .map(|v| format!("{} {} {}", num(v.x), num(v.y), num(v.z)))
            .collect();
        block.push_pair(format!("row{r}"), text.join(" "));
    }
    block
}

fn number_rows(name: &str, values: &[f32], side: usize) -> Block {
    let mut block = Block::new(name);
    for (r, row) in values.chunks(side).enumerate().take(side) {
        let text: Vec<String> = row.iter().map(|v| num(*v)).collect();
        block.push_pair(format!("row{r}"), text.join(" "));
    }
    block
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> [Vec3; 4] {
        [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 64.0, 0.0),
            Vec3::new(64.0, 64.0, 0.0),
            Vec3::new(64.0, 0.0, 0.0),
        ]
    }

    #[test]
    fn a_flat_grid_lies_on_its_face() {
        let d = Displacement::flat(2, Vec3::ZERO, Vec3::Z);
        assert_eq!(d.side(), 5);
        let points = d.points(&square()).unwrap();
        assert_eq!(points.len(), 25);
        assert_eq!(points[0], Vec3::ZERO);
        assert_eq!(
            points[4],
            Vec3::new(64.0, 0.0, 0.0),
            "the first row runs along c0 to c3"
        );
        assert_eq!(points[24], Vec3::new(64.0, 64.0, 0.0));
        assert!(points.iter().all(|p| p.z == 0.0));
    }

    #[test]
    fn raised_points_move_along_their_normal() {
        let mut d = Displacement::flat(2, Vec3::ZERO, Vec3::Z);
        d.raise(12, 32.0, Vec3::Z);
        d.raise(0, 1.0e9, Vec3::Z);
        let points = d.points(&square()).unwrap();
        assert_eq!(points[12], Vec3::new(32.0, 32.0, 32.0));
        assert_eq!(d.distances[0], MAX_DISTANCE);
        d.set_alpha(3, 900.0);
        assert_eq!(d.alphas[3], 255.0);
    }

    #[test]
    fn heights_are_stored_as_hammer_stores_them() {
        let mut d = Displacement::flat(2, Vec3::ZERO, Vec3::Z);
        // Lowered: pointing down, a distance that is never negative.
        d.raise(5, -10.0, Vec3::Z);
        assert_eq!((d.normals[5], d.distances[5]), (-Vec3::Z, 10.0));
        assert_eq!(d.height(5, Vec3::Z), -10.0);
        // Raising a lowered point brings it up, through the face.
        d.raise(5, 15.0, Vec3::Z);
        assert_eq!((d.normals[5], d.distances[5]), (Vec3::Z, 5.0));
        // Back to the face: no movement, the face's direction.
        d.raise(5, -5.0, Vec3::Z);
        assert_eq!((d.normals[5], d.distances[5]), (Vec3::Z, 0.0));
        // Hammer writes "0 0 0" for points it never moved; they still move.
        d.normals[6] = Vec3::ZERO;
        d.distances[6] = 3.0;
        d.raise(6, 2.0, Vec3::Z);
        assert_eq!(d.height(6, Vec3::Z), 2.0);
        // Out of range does nothing.
        d.raise(999, 1.0, Vec3::Z);
        assert_eq!(d.height(999, Vec3::Z), 0.0);
        d.raise(7, f32::NAN, Vec3::Z);
        assert_eq!((d.normals[7], d.distances[7]), (Vec3::Z, 0.0));
    }

    #[test]
    fn elevation_lifts_away_from_the_brush() {
        // square() winds clockwise seen from above, so the face looks up.
        let mut d = Displacement::flat(2, Vec3::ZERO, Vec3::Z);
        d.elevation = 8.0;
        let points = d.points(&square()).unwrap();
        assert!(points.iter().all(|p| (p.z - 8.0).abs() < 1e-4));
    }

    #[test]
    fn blocks_round_trip_and_keep_what_they_dont_read() {
        let mut d = Displacement::flat(3, Vec3::new(-8.0, 0.0, 16.0), Vec3::Z);
        d.raise(40, 12.5, Vec3::Z);
        d.set_alpha(2, 128.0);
        let mut block = d.to_block();
        block.push_block(Block::new("triangle_tags"));
        let back = Displacement::from_block(&block).unwrap();
        assert_eq!(back.distances, d.distances);
        assert_eq!(back.alphas, d.alphas);
        assert_eq!(back.start, d.start);
        assert!(back.to_block().blocks("triangle_tags").next().is_some());
    }

    #[test]
    fn broken_blocks_are_not_read() {
        let mut block = Displacement::flat(2, Vec3::ZERO, Vec3::Z).to_block();
        block.entries[0] = Entry::Pair("power".into(), "9".into());
        assert!(Displacement::from_block(&block).is_none());
        let mut short = Displacement::flat(2, Vec3::ZERO, Vec3::Z).to_block();
        for e in &mut short.entries {
            if let Entry::Block(b) = e
                && b.name == "distances"
            {
                b.entries.pop();
            }
        }
        assert!(Displacement::from_block(&short).is_none());
    }

    #[test]
    fn corners_start_nearest_the_start_and_wind_clockwise_from_outside() {
        // Given counter-clockwise (seen from above, +Z outside).
        let ccw = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(64.0, 0.0, 0.0),
            Vec3::new(64.0, 64.0, 0.0),
            Vec3::new(0.0, 64.0, 0.0),
        ];
        let c = ordered_corners(&ccw, Vec3::Z, Vec3::new(63.0, 63.0, 0.0)).unwrap();
        assert_eq!(c[0], Vec3::new(64.0, 64.0, 0.0));
        assert!((c[1] - c[0]).cross(c[2] - c[1]).dot(Vec3::Z) < 0.0);
        assert!(ordered_corners(&ccw[..3], Vec3::Z, Vec3::ZERO).is_none());
    }

    #[test]
    fn fuzz_odd_blocks_never_crash() {
        let mut seed: u64 = 0xD15;
        let words = [
            "2", "3", "9", "-1", "nan", "[0 0 0]", "1 2", "0 0 1", "x", "",
        ];
        for _ in 0..2000 {
            let mut pick = || {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                words[(seed % words.len() as u64) as usize].to_string()
            };
            let mut block = Block::new("dispinfo");
            block.push_pair("power", pick());
            block.push_pair("startposition", pick());
            let mut rows = Block::new("distances");
            for r in 0..5 {
                rows.push_pair(format!("row{r}"), pick());
            }
            block.push_block(rows);
            let _ = Displacement::from_block(&block);
        }
    }
}
