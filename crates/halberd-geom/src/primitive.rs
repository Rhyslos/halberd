//! Ready-made shapes to draw: boxes, wedges, cylinders, cones, spheres,
//! arches and stairs, each fitted into a box.
//!
//! A shape is one or more convex brushes. Corners are rounded to whole
//! units, as Hammer does, so the map compiles without tiny gaps; that is
//! also why very small round shapes can fail to build (their corners would
//! land on top of each other). A sphere's points are rounded too, but its
//! bands are then not perfectly flat, so where they meet a corner can fall
//! between units.

use crate::{Aabb, Brush, GeomError, Plane};
use glam::{Vec2, Vec3};
use std::f32::consts::{PI, TAU};

/// Fewest and most sides for cylinders, cones and arches.
pub const SIDES_RANGE: (u32, u32) = (3, 64);
/// Fewest and most sides around a sphere. Its faces are sides × sides / 2,
/// which must stay within a brush's face limit.
pub const SPHERE_SIDES_RANGE: (u32, u32) = (4, 16);
/// Turns in an outline gentler than this (the sine of the angle) count as
/// straight.
const STRAIGHT: f32 = 0.02;
/// Most brushes one shape may make (stair steps).
pub const MAX_SHAPE_BRUSHES: usize = 256;

/// What the Draw tool makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shape {
    /// A box filling the drawn area.
    #[default]
    Box,
    /// A ramp: full height at one end, nothing at the other.
    Wedge,
    /// An upright cylinder with flat sides.
    Cylinder,
    /// A cone (spike) with its point at the top.
    Cone,
    /// A sphere (an ellipsoid if the box is not a cube).
    Sphere,
    /// A doorway or bridge arch: a half ring standing on the ground, made
    /// of one brush per side.
    Arch,
    /// A flight of solid steps, one brush per step.
    Stairs,
}

impl Shape {
    /// Every shape, in the order the shape picker lists them.
    pub const ALL: [Shape; 7] = [
        Self::Box,
        Self::Wedge,
        Self::Cylinder,
        Self::Cone,
        Self::Sphere,
        Self::Arch,
        Self::Stairs,
    ];

    /// The shape's name in the interface.
    pub fn label(self) -> &'static str {
        match self {
            Self::Box => "Box",
            Self::Wedge => "Wedge",
            Self::Cylinder => "Cylinder",
            Self::Cone => "Cone",
            Self::Sphere => "Sphere",
            Self::Arch => "Arch",
            Self::Stairs => "Stairs",
        }
    }

    /// True if the shape has a side count to choose.
    pub fn has_sides(self) -> bool {
        matches!(
            self,
            Self::Cylinder | Self::Cone | Self::Sphere | Self::Arch
        )
    }

    /// True if the shape faces one way (a ramp climbs, stairs rise, an arch
    /// spans): the way the drag went.
    pub fn has_direction(self) -> bool {
        matches!(self, Self::Wedge | Self::Arch | Self::Stairs)
    }
}

/// Settings for the shapes that have them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeSettings {
    /// Sides around a cylinder, cone, sphere or arch.
    pub sides: u32,
    /// How thick an arch is, in units.
    pub arch_thickness: f32,
    /// How tall each stair step is, in units (rounded to whole units, so
    /// steps may differ by one).
    pub step_height: f32,
}

impl Default for ShapeSettings {
    fn default() -> Self {
        Self {
            sides: 8,
            arch_thickness: 16.0,
            // Comfortable for a GMod player, who climbs up to 18.
            step_height: 8.0,
        }
    }
}

/// Which way a directional shape goes, along the ground: a ramp and
/// stairs climb this way; an arch spans along this axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Heading {
    /// Towards +X.
    #[default]
    PosX,
    /// Towards −X.
    NegX,
    /// Towards +Y.
    PosY,
    /// Towards −Y.
    NegY,
}

impl Heading {
    /// This heading turned `quarters` quarter turns clockwise, seen from
    /// above (Z up): +X, then −Y, −X, +Y.
    pub fn turned_clockwise(self, quarters: u8) -> Self {
        let order = [Self::PosX, Self::NegY, Self::NegX, Self::PosY];
        let at = order.iter().position(|h| *h == self).unwrap_or(0);
        order[(at + usize::from(quarters % 4)) % 4]
    }

    /// The heading of a drag along the ground: its longer direction.
    pub fn of_drag(delta: Vec2) -> Self {
        if delta.y.abs() > delta.x.abs() {
            if delta.y < 0.0 {
                Self::NegY
            } else {
                Self::PosY
            }
        } else if delta.x < 0.0 {
            Self::NegX
        } else {
            Self::PosX
        }
    }
}

/// The brushes making `shape` fitted into `bounds`, facing `heading`.
pub fn build_shape(
    shape: Shape,
    settings: &ShapeSettings,
    bounds: Aabb,
    heading: Heading,
) -> Result<Vec<Brush>, GeomError> {
    if !(bounds.min.is_finite() && bounds.max.is_finite()) {
        return Err(GeomError::NotFinite);
    }
    let frame = Frame::new(bounds, heading);
    let brushes = match shape {
        Shape::Box => Ok(vec![Brush::cuboid(bounds)?]),
        Shape::Wedge => Ok(vec![wedge(&frame)?]),
        Shape::Cylinder => Ok(vec![cylinder(bounds, clamp_sides(settings.sides))?]),
        Shape::Cone => Ok(vec![cone(bounds, clamp_sides(settings.sides))?]),
        Shape::Sphere => Ok(vec![sphere(bounds, settings.sides)?]),
        Shape::Arch => arch(&frame, clamp_sides(settings.sides), settings.arch_thickness),
        Shape::Stairs => stairs(&frame, settings.step_height),
    }?;
    // Tiny round shapes can round a corner past the box; refuse them rather
    // than draw outside what was asked.
    let (lo, hi) = (bounds.min - 0.05, bounds.max + 0.05);
    let fits = brushes
        .iter()
        .all(|b| b.bounds().min.cmpge(lo).all() && b.bounds().max.cmple(hi).all());
    if fits {
        Ok(brushes)
    } else {
        Err(GeomError::TooSmall)
    }
}

fn clamp_sides(sides: u32) -> u32 {
    sides.clamp(SIDES_RANGE.0, SIDES_RANGE.1)
}

/// A point rounded to whole units, kept within `bounds` (rounding must
/// never push a shape out of the box it was drawn in).
fn fit(p: Vec3, bounds: Aabb) -> Vec3 {
    p.round().max(bounds.min.ceil()).min(bounds.max.floor())
}

/// The plane through three points, facing away from `inside`. `None` if
/// the points are in a line (rounding can do that to tiny shapes).
fn outward(a: Vec3, b: Vec3, c: Vec3, inside: Vec3) -> Option<Plane> {
    let plane = Plane::new((b - a).cross(c - a), a)?;
    if plane.signed_distance(inside) > 0.0 {
        Plane::new(-plane.normal, a)
    } else {
        Some(plane)
    }
}

/// The brush behind `planes`; planes that could not be made are skipped,
/// and the brush then fails to close if they mattered.
fn solid(planes: impl IntoIterator<Item = Option<Plane>>) -> Result<Brush, GeomError> {
    let planes: Vec<Plane> = planes.into_iter().flatten().collect();
    Brush::from_planes(&planes)
}

/// The box seen from a heading: `along` runs the way the shape faces,
/// `across` sideways, and heights stay heights.
struct Frame {
    bounds: Aabb,
    heading: Heading,
}

impl Frame {
    fn new(bounds: Aabb, heading: Heading) -> Self {
        Self { bounds, heading }
    }

    /// Length along the heading, and width across it.
    fn length_width(&self) -> (f32, f32) {
        let size = self.bounds.size();
        match self.heading {
            Heading::PosX | Heading::NegX => (size.x, size.y),
            Heading::PosY | Heading::NegY => (size.y, size.x),
        }
    }

    /// The world point `along` units in the heading's direction from the
    /// start of the box, `across` units across it, at height `up` above
    /// its bottom.
    fn point(&self, along: f32, across: f32, up: f32) -> Vec3 {
        let (lo, hi) = (self.bounds.min, self.bounds.max);
        let z = lo.z + up;
        match self.heading {
            Heading::PosX => Vec3::new(lo.x + along, lo.y + across, z),
            Heading::NegX => Vec3::new(hi.x - along, lo.y + across, z),
            Heading::PosY => Vec3::new(lo.x + across, lo.y + along, z),
            Heading::NegY => Vec3::new(lo.x + across, hi.y - along, z),
        }
    }
}

/// A ramp rising along the heading.
fn wedge(f: &Frame) -> Result<Brush, GeomError> {
    let (l, w) = f.length_width();
    let h = f.bounds.size().z;
    let p = |a, b, c| fit(f.point(a, b, c), f.bounds);
    let inside = f.point(l * 0.75, w * 0.5, h * 0.25);
    solid([
        outward(p(0.0, 0.0, 0.0), p(l, 0.0, 0.0), p(0.0, w, 0.0), inside),
        outward(p(l, 0.0, 0.0), p(l, w, 0.0), p(l, 0.0, h), inside),
        outward(p(0.0, 0.0, 0.0), p(l, 0.0, 0.0), p(l, 0.0, h), inside),
        outward(p(0.0, w, 0.0), p(l, w, 0.0), p(l, w, h), inside),
        outward(p(0.0, 0.0, 0.0), p(0.0, w, 0.0), p(l, 0.0, h), inside),
    ])
}

/// Points around the ellipse filling the box's ground plan, rounded, as a
/// convex outline (rounding can push a point inwards; it is then left out,
/// so every corner stays on whole units). The first side faces −Y, so a
/// 4-sided one fills the box.
fn ring(bounds: Aabb, sides: u32) -> Vec<Vec2> {
    let centre = bounds.center();
    let radius = bounds.size() * 0.5;
    let offset = -PI / 2.0 - PI / sides as f32;
    // A square's corners reach the box's corners, not the circle.
    let reach = if sides == 4 {
        std::f32::consts::SQRT_2
    } else {
        1.0
    };
    let points = (0..sides).map(|i| {
        let angle = offset + TAU * i as f32 / sides as f32;
        fit(
            Vec3::new(
                centre.x + radius.x * reach * angle.cos(),
                centre.y + radius.y * reach * angle.sin(),
                bounds.min.z,
            ),
            bounds,
        )
        .truncate()
    });
    convex_outline(points.collect())
}

/// The convex outline around `points`, counter-clockwise, without points
/// in a line (Andrew's monotone chain).
fn convex_outline(mut points: Vec<Vec2>) -> Vec<Vec2> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    // A point where the outline turns by less than about a degree is left
    // out too: its two sides would be almost parallel, and where nearly
    // parallel sides meet, rounding in the stored planes moves the corner
    // off its whole unit.
    let turn = |o: Vec2, a: Vec2, b: Vec2| {
        let (u, v) = (a - o, b - a);
        let sine = u.perp_dot(v) / (u.length() * v.length()).max(f32::MIN_POSITIVE);
        if sine < STRAIGHT { 0.0 } else { sine }
    };
    let mut hull: Vec<Vec2> = Vec::with_capacity(points.len() * 2);
    for pass in [false, true] {
        let start = hull.len();
        let ordered: Box<dyn Iterator<Item = &Vec2>> = if pass {
            Box::new(points.iter().rev())
        } else {
            Box::new(points.iter())
        };
        for &p in ordered {
            while hull.len() >= start + 2
                && turn(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0
            {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    hull
}

/// A prism: the outline `polygon` (in its own 2D plane) swept from 0 to
/// `extent`. `place` puts an outline point at a sweep distance in the world
/// exactly; corners are rounded into `bounds` from there.
fn prism(
    polygon: &[Vec2],
    extent: f32,
    bounds: Aabb,
    place: impl Fn(Vec2, f32) -> Vec3,
) -> Result<Brush, GeomError> {
    if polygon.len() < 3 {
        return Err(GeomError::TooSmall);
    }
    let at = |p: Vec2, d: f32| fit(place(p, d), bounds);
    // Not rounded: rounding could put it on a side of a thin piece, and
    // then that side would face the wrong way.
    let centre = polygon.iter().copied().sum::<Vec2>() / polygon.len() as f32;
    let inside = place(centre, extent * 0.5);
    let (a, b, c) = (polygon[0], polygon[1], polygon[2]);
    let mut planes = vec![
        outward(at(a, 0.0), at(b, 0.0), at(c, 0.0), inside),
        outward(at(a, extent), at(b, extent), at(c, extent), inside),
    ];
    for (i, &p) in polygon.iter().enumerate() {
        let q = polygon[(i + 1) % polygon.len()];
        planes.push(outward(at(p, 0.0), at(q, 0.0), at(p, extent), inside));
    }
    solid(planes)
}

/// An upright prism with `sides` flat sides.
fn cylinder(bounds: Aabb, sides: u32) -> Result<Brush, GeomError> {
    let z = bounds.min.z;
    prism(&ring(bounds, sides), bounds.size().z, bounds, |p, t| {
        p.extend(z + t)
    })
}

/// A cone with `sides` flat sides and its point at the top centre.
fn cone(bounds: Aabb, sides: u32) -> Result<Brush, GeomError> {
    let base: Vec<Vec3> = ring(bounds, sides)
        .into_iter()
        .map(|p| p.extend(bounds.min.z))
        .collect();
    if base.len() < 3 {
        return Err(GeomError::TooSmall);
    }
    let apex = fit(
        Vec3::new(bounds.center().x, bounds.center().y, bounds.max.z),
        bounds,
    );
    let inside = Vec3::new(bounds.center().x, bounds.center().y, bounds.min.z).lerp(apex, 0.25);
    let mut planes = vec![Plane::new(Vec3::NEG_Z, bounds.min)];
    for (i, &a) in base.iter().enumerate() {
        let b = base[(i + 1) % base.len()];
        planes.push(outward(a, b, apex, inside));
    }
    solid(planes)
}

/// A sphere with `sides` around and half as many bands from pole to pole.
fn sphere(bounds: Aabb, sides: u32) -> Result<Brush, GeomError> {
    let (low, high) = SPHERE_SIDES_RANGE;
    // An even count keeps a band on the equator, so it is symmetric.
    let sides = (sides.clamp(low, high) + 1) & !1;
    let bands = sides / 2;
    let centre = bounds.center();
    let radius = bounds.size() * 0.5;
    let offset = -PI / 2.0 - PI / sides as f32;
    let point = |band: u32, side: u32| {
        let polar = PI * band as f32 / bands as f32;
        let angle = offset + TAU * side as f32 / sides as f32;
        fit(
            Vec3::new(
                centre.x + radius.x * polar.sin() * angle.cos(),
                centre.y + radius.y * polar.sin() * angle.sin(),
                centre.z + radius.z * polar.cos(),
            ),
            bounds,
        )
    };
    let mut planes = Vec::with_capacity((sides * bands) as usize);
    for band in 0..bands {
        for side in 0..sides {
            let next = (side + 1) % sides;
            let (a, b, c) = if band == 0 {
                (point(0, 0), point(1, side), point(1, next))
            } else {
                (point(band, side), point(band, next), point(band + 1, side))
            };
            planes.push(outward(a, b, c, centre));
        }
    }
    solid(planes)
}

/// A half ring standing on the ground, spanning along the heading, `depth`
/// across it, made of `sides` brushes.
fn arch(f: &Frame, sides: u32, thickness: f32) -> Result<Vec<Brush>, GeomError> {
    let (span, depth) = f.length_width();
    let height = f.bounds.size().z;
    let (rx, rz) = (span * 0.5, height);
    if rx.min(rz) < 1.0 {
        return Err(GeomError::TooSmall);
    }
    // A thickness beyond the radius makes a solid half disc.
    let t = if thickness.is_finite() {
        thickness.clamp(1.0, rx.min(rz))
    } else {
        ShapeSettings::default().arch_thickness.min(rx.min(rz))
    };
    let on_arc = |k: u32, inner: bool| {
        let angle = PI * k as f32 / sides as f32;
        let (ax, az) = if inner { (rx - t, rz - t) } else { (rx, rz) };
        Vec2::new(rx - ax * angle.cos(), az * angle.sin())
            .round()
            .clamp(Vec2::ZERO, Vec2::new(span, height).floor().max(Vec2::ZERO))
    };
    (0..sides)
        .map(|k| {
            // Each piece is the outline between two cuts, made convex in
            // case rounding bent it.
            let outline = convex_outline(vec![
                on_arc(k, false),
                on_arc(k + 1, false),
                on_arc(k + 1, true),
                on_arc(k, true),
            ]);
            prism(&outline, depth, f.bounds, |p, d| f.point(p.x, d, p.y))
        })
        .collect()
}

/// Solid steps rising along the heading, about `step_height` each.
fn stairs(f: &Frame, step_height: f32) -> Result<Vec<Brush>, GeomError> {
    let (length, width) = f.length_width();
    let height = f.bounds.size().z;
    let step = if step_height.is_finite() && step_height >= 1.0 {
        step_height
    } else {
        ShapeSettings::default().step_height
    };
    let steps = ((height / step).round() as usize).max(1);
    if steps > MAX_SHAPE_BRUSHES {
        return Err(GeomError::TooManyPieces);
    }
    let run = |i: usize| (length * i as f32 / steps as f32).round();
    let rise = |i: usize| (height * i as f32 / steps as f32).round();
    (0..steps)
        .map(|i| {
            let (start, end, top) = (run(i), run(i + 1), rise(i + 1));
            if end - start < 1.0 || top < 1.0 {
                return Err(GeomError::TooSmall);
            }
            let a = f.point(start, 0.0, 0.0);
            let b = f.point(end, width, top);
            Brush::cuboid(Aabb::from_corners(a.min(b), a.max(b)))
        })
        .collect()
}

#[cfg(test)]
mod tests;
