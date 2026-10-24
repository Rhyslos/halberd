//! The Sculpt tool: drag over displacements to raise or lower them, smooth
//! them, or paint the blend between their two textures. Every displacement
//! point within the brush's radius is moved, most at the middle and less
//! towards the edge; strokes run across neighbouring displacements.

use crate::camera::Ray;
use glam::Vec3;
use halberd_doc::{BrushObject, Command, Displacement, Document, Object, ObjectId};

/// What dragging does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SculptMode {
    /// Raise (or, with Ctrl, lower) the surface.
    #[default]
    Raise,
    /// Even out bumps.
    Smooth,
    /// Paint the second texture in (or, with Ctrl, out).
    Paint,
}

impl SculptMode {
    /// Every mode, in the order the toolbar shows them.
    pub const ALL: [SculptMode; 3] = [Self::Raise, Self::Smooth, Self::Paint];

    /// The toolbar's name for it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Raise => "Raise / lower",
            Self::Smooth => "Smooth",
            Self::Paint => "Paint blend",
        }
    }

    /// One-line explanation, for tooltips.
    pub fn description(self) -> &'static str {
        match self {
            Self::Raise => "Drag to raise the surface; hold Ctrl to lower it",
            Self::Smooth => "Drag to even out bumps",
            Self::Paint => {
                "Drag to paint the face's second texture in (blend materials); hold Ctrl to \
                 paint it out"
            }
        }
    }
}

/// The Sculpt tool's settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SculptSettings {
    /// What dragging does.
    pub mode: SculptMode,
    /// How far the brush reaches, in units.
    pub radius: f32,
    /// How fast it works: units a second when raising, blend (0 to 255) a
    /// second when painting.
    pub strength: f32,
}

impl Default for SculptSettings {
    fn default() -> Self {
        Self {
            mode: SculptMode::Raise,
            radius: 96.0,
            strength: 64.0,
        }
    }
}

impl SculptSettings {
    /// Smallest and largest radius, in units.
    pub const RADIUS_RANGE: (f32, f32) = (8.0, 2048.0);
    /// Smallest and largest strength.
    pub const STRENGTH_RANGE: (f32, f32) = (1.0, 1024.0);
}

mod surfaces;
pub(crate) use surfaces::DispCache;
use surfaces::DispFace;

/// Where `ray` first hits a displacement's surface, if it does (and no
/// world brush or shape is in front of it). Reads the map's displacements
/// each time; the Sculpt tool keeps them read instead.
pub fn pick_displacement(doc: &Document, ray: &Ray) -> Option<Vec3> {
    let mut cache = DispCache::default();
    cache.update(doc);
    pick_in(doc, &cache, ray)
}

/// [`pick_displacement`] with displacements already read.
pub(crate) fn pick_in(doc: &Document, cache: &DispCache, ray: &Ray) -> Option<Vec3> {
    let mut best: Option<f32> = None;
    for face in cache.faces() {
        if !ray_box(ray, face.min, face.max) {
            continue;
        }
        let side = face.disp.side();
        for row in 0..side - 1 {
            for col in 0..side - 1 {
                let a = row * side + col;
                let (b, c, d) = (a + 1, a + side, a + side + 1);
                for [i, j, k] in [[a, c, b], [b, c, d]] {
                    let (Some(p), Some(q), Some(r)) =
                        (face.points.get(i), face.points.get(j), face.points.get(k))
                    else {
                        continue;
                    };
                    if let Some(t) = ray_triangle(ray, *p, *q, *r)
                        && best.is_none_or(|b| t < b)
                    {
                        best = Some(t);
                    }
                }
            }
        }
    }
    let t = best?;
    // A wall in front of the terrain hides it. Brushes with displacements
    // don't count: their own faces lie under raised ground.
    let blocked = doc.objects().any(|(id, object)| {
        let world = match object {
            Object::Brush(b) => b.entity().is_none() && !cache.has(id),
            Object::Shape(_) => true,
            Object::Entity(_) => false,
        };
        world
            && object
                .ray_hit(ray.origin, ray.direction)
                .is_some_and(|hit| hit < t - 0.5)
    });
    (!blocked).then(|| ray.origin + ray.direction * t)
}

/// True if `ray` passes through the box from `min` to `max`.
fn ray_box(ray: &Ray, min: Vec3, max: Vec3) -> bool {
    let inv = ray.direction.recip();
    let (a, b) = ((min - ray.origin) * inv, (max - ray.origin) * inv);
    let near = a.min(b).max_element();
    let far = a.max(b).min_element();
    // A ray along an axis gives NaN for the flat sides: let it through.
    near.is_nan() || far.is_nan() || (far >= near.max(0.0))
}

/// Distance along `ray` to triangle `a b c` (either side), if it hits.
fn ray_triangle(ray: &Ray, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let (e1, e2) = (b - a, c - a);
    let p = ray.direction.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-8 {
        return None;
    }
    let inv = 1.0 / det;
    let s = ray.origin - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = ray.direction.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 0.0).then_some(t)
}

/// One step of a stroke at `center`, lasting `dt` seconds: the edit, or
/// `None` if no displacement point is in reach. `invert` lowers or paints
/// out.
pub(crate) fn stroke(
    doc: &Document,
    cache: &DispCache,
    center: Vec3,
    settings: &SculptSettings,
    invert: bool,
    dt: f32,
) -> Option<Command> {
    let radius = settings.radius.clamp(
        SculptSettings::RADIUS_RANGE.0,
        SculptSettings::RADIUS_RANGE.1,
    );
    let amount = settings.strength.clamp(
        SculptSettings::STRENGTH_RANGE.0,
        SculptSettings::STRENGTH_RANGE.1,
    ) * if dt.is_finite() {
        dt.clamp(0.0, 0.1)
    } else {
        0.0
    } * if invert { -1.0 } else { 1.0 };
    // A broken setting or a frame with no time moves nothing.
    if amount == 0.0 || !amount.is_finite() || !radius.is_finite() {
        return None;
    }
    let mut changed: Vec<(ObjectId, BrushObject)> = Vec::new();
    for face in cache.faces().filter(|f| f.near(center, radius)) {
        let weights = weights(face, center, radius);
        if weights.iter().all(|w| *w <= 0.0) {
            continue;
        }
        let mut disp = face.disp.clone();
        apply(&mut disp, face.normal, &weights, settings.mode, amount);
        if settings.mode != SculptMode::Paint {
            disp.forget_derived();
        }
        let base = changed
            .iter()
            .find(|(id, _)| *id == face.brush)
            .map(|(_, b)| b.clone())
            .or_else(|| doc.get(face.brush)?.as_brush().cloned())?;
        let updated = crate::tool::with_displacement(&base, face.face, Some(&disp));
        changed.retain(|(id, _)| *id != face.brush);
        changed.push((face.brush, updated));
    }
    if changed.is_empty() {
        return None;
    }
    let label = match settings.mode {
        SculptMode::Raise | SculptMode::Smooth => "Sculpt",
        SculptMode::Paint => "Paint blend",
    };
    Some(Command::ShapeEdit {
        label: label.into(),
        changed,
        added: Vec::new(),
        removed: Vec::new(),
        select_added: false,
    })
}

/// How much each of `face`'s points is moved by a brush at `center`. The
/// reach is measured across the face, not through the raised surface, so
/// raising a point never takes it out of the brush (lowering then undoes
/// raising exactly); points further than `radius` above or below the
/// brush along the face's normal are out of reach.
fn weights(face: &DispFace, center: Vec3, radius: f32) -> Vec<f32> {
    face.base
        .iter()
        .enumerate()
        .map(|(k, b)| {
            let offset = center - *b;
            let along = offset.dot(face.normal);
            if (along - face.disp.height(k, face.normal)).abs() > radius {
                return 0.0;
            }
            falloff((offset - face.normal * along).length(), radius)
        })
        .collect()
}

/// How much a point `distance` from the middle is moved: 1 at the middle,
/// easing to 0 at `radius`.
fn falloff(distance: f32, radius: f32) -> f32 {
    let t = (1.0 - distance / radius).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn apply(disp: &mut Displacement, normal: Vec3, weights: &[f32], mode: SculptMode, amount: f32) {
    match mode {
        SculptMode::Raise => {
            for (k, w) in weights.iter().enumerate() {
                disp.raise(k, amount * w, normal);
            }
        }
        SculptMode::Paint => {
            for (k, w) in weights.iter().enumerate() {
                let alpha = disp.alphas.get(k).copied().unwrap_or(0.0);
                disp.set_alpha(k, alpha + amount * w);
            }
        }
        SculptMode::Smooth => smooth(disp, normal, weights, amount),
    }
}

/// Moves each point in reach towards its neighbours' average height.
fn smooth(disp: &mut Displacement, normal: Vec3, weights: &[f32], amount: f32) {
    let side = disp.side();
    let before: Vec<f32> = (0..side * side).map(|k| disp.height(k, normal)).collect();
    // How far towards the average a point moves in one step, at full
    // weight.
    let rate = (amount.abs() / 64.0).clamp(0.0, 1.0);
    for (k, w) in weights.iter().enumerate() {
        let Some(height) = before.get(k).filter(|_| *w > 0.0) else {
            continue;
        };
        let (row, col) = (k / side, k % side);
        let neighbours = [
            (row > 0).then(|| k - side),
            (row + 1 < side).then(|| k + side),
            (col > 0).then(|| k - 1),
            (col + 1 < side).then(|| k + 1),
        ];
        let around: Vec<f32> = neighbours
            .iter()
            .flatten()
            .filter_map(|n| before.get(*n).copied())
            .collect();
        if around.is_empty() {
            continue;
        }
        let average = around.iter().sum::<f32>() / around.len() as f32;
        disp.raise(k, (average - height) * rate * w, normal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halberd_doc::ordered_corners;
    use halberd_geom::{Aabb, Brush};

    /// One stroke step, with the displacements read afresh.
    fn step(
        doc: &Document,
        center: Vec3,
        settings: &SculptSettings,
        invert: bool,
        dt: f32,
    ) -> Option<Command> {
        let mut cache = DispCache::default();
        cache.update(doc);
        stroke(doc, &cache, center, settings, invert, dt)
    }

    /// A map with one 512-unit box whose top is a displacement.
    fn terrain() -> (Document, ObjectId) {
        let mut doc = Document::new();
        let cube = Brush::cuboid(Aabb::from_corners(
            Vec3::new(0.0, 0.0, -16.0),
            Vec3::new(512.0, 512.0, 0.0),
        ))
        .unwrap();
        doc.execute(Command::AddBrushes(vec![cube])).unwrap();
        let id = *doc.selection().iter().next().unwrap();
        let object = doc.get(id).unwrap().as_brush().unwrap().clone();
        let top = object
            .brush()
            .faces()
            .iter()
            .position(|f| f.plane().normal.z > 0.9)
            .unwrap();
        let shape = &object.brush().faces()[top];
        let corners = ordered_corners(shape.vertices(), Vec3::Z, shape.vertices()[0]).unwrap();
        let disp = Displacement::flat(3, corners[0], Vec3::Z);
        let updated = crate::tool::with_displacement(&object, top, Some(&disp));
        doc.execute(Command::ShapeEdit {
            label: "Make displacement".into(),
            changed: vec![(id, updated)],
            added: vec![],
            removed: vec![],
            select_added: false,
        })
        .unwrap();
        (doc, id)
    }

    fn top(doc: &Document, id: ObjectId) -> Displacement {
        let b = doc.get(id).unwrap().as_brush().unwrap();
        b.faces().iter().find_map(|f| f.displacement()).unwrap()
    }

    #[test]
    fn a_ray_hits_the_surface_where_it_is() {
        let (doc, _) = terrain();
        let ray = Ray {
            origin: Vec3::new(100.0, 200.0, 500.0),
            direction: -Vec3::Z,
        };
        let hit = pick_displacement(&doc, &ray).unwrap();
        assert!((hit - Vec3::new(100.0, 200.0, 0.0)).length() < 0.01);
        let miss = Ray {
            origin: Vec3::new(900.0, 200.0, 500.0),
            direction: -Vec3::Z,
        };
        assert_eq!(pick_displacement(&doc, &miss), None);
    }

    #[test]
    fn raising_lifts_the_middle_most_and_lowering_undoes_it() {
        let (mut doc, id) = terrain();
        let settings = SculptSettings::default();
        let center = Vec3::new(256.0, 256.0, 0.0);
        let up = step(&doc, center, &settings, false, 0.1).unwrap();
        doc.execute(up).unwrap();
        let d = top(&doc, id);
        let middle = d.distances[4 * 9 + 4];
        assert!((middle - 6.4).abs() < 0.01, "{middle}");
        assert_eq!(d.distances[0], 0.0, "corners are out of reach");
        let down = step(&doc, center, &settings, true, 0.1).unwrap();
        doc.execute(down).unwrap();
        assert!(top(&doc, id).distances[40].abs() < 0.01);
        // Far away, nothing to do.
        assert!(step(&doc, Vec3::splat(5000.0), &settings, false, 0.1).is_none());
    }

    #[test]
    fn painting_and_smoothing() {
        let (mut doc, id) = terrain();
        let center = Vec3::new(256.0, 256.0, 0.0);
        let paint = SculptSettings {
            mode: SculptMode::Paint,
            strength: 1024.0,
            ..SculptSettings::default()
        };
        doc.execute(step(&doc, center, &paint, false, 0.1).unwrap())
            .unwrap();
        assert!(top(&doc, id).alphas[40] > 100.0);
        // A spike gets smoothed down.
        let spike = SculptSettings {
            radius: 8.0,
            strength: 1024.0,
            ..SculptSettings::default()
        };
        doc.execute(step(&doc, center, &spike, false, 0.1).unwrap())
            .unwrap();
        let tall = top(&doc, id).distances[40];
        let smooth = SculptSettings {
            mode: SculptMode::Smooth,
            radius: 40.0,
            strength: 64.0,
        };
        // The brush is where the pointer meets the spike's tip.
        let tip = center + Vec3::Z * tall;
        doc.execute(step(&doc, tip, &smooth, false, 0.1).unwrap())
            .unwrap();
        assert!(top(&doc, id).distances[40] < tall);
    }

    #[test]
    fn broken_numbers_move_nothing() {
        let (doc, _) = terrain();
        let center = Vec3::new(256.0, 256.0, 0.0);
        let settings = SculptSettings::default();
        assert!(step(&doc, center, &settings, false, 0.0).is_none());
        assert!(step(&doc, center, &settings, false, f32::NAN).is_none());
        assert!(step(&doc, center, &settings, false, f32::INFINITY).is_none());
        let broken = SculptSettings {
            radius: f32::NAN,
            ..SculptSettings::default()
        };
        assert!(step(&doc, center, &broken, false, 0.1).is_none());
        let broken = SculptSettings {
            strength: f32::NAN,
            ..SculptSettings::default()
        };
        assert!(step(&doc, center, &broken, false, 0.1).is_none());
    }

    #[test]
    fn hammers_lowered_points_rise_when_raised() {
        let (mut doc, id) = terrain();
        // As Hammer saves a dip: pointing down, a positive distance.
        let mut d = top(&doc, id);
        d.normals[40] = -Vec3::Z;
        d.distances[40] = 20.0;
        let object = doc.get(id).unwrap().as_brush().unwrap().clone();
        let face = object
            .faces()
            .iter()
            .position(|f| f.displacement().is_some())
            .unwrap();
        doc.execute(Command::ShapeEdit {
            label: "Dip".into(),
            changed: vec![(id, crate::tool::with_displacement(&object, face, Some(&d)))],
            added: vec![],
            removed: vec![],
            select_added: false,
        })
        .unwrap();
        // The pointer meets the dip's floor.
        let center = Vec3::new(256.0, 256.0, -20.0);
        doc.execute(step(&doc, center, &SculptSettings::default(), false, 0.1).unwrap())
            .unwrap();
        let height = top(&doc, id).height(40, Vec3::Z);
        assert!((height - (-20.0 + 6.4)).abs() < 0.01, "{height}");
    }

    #[test]
    fn a_wall_in_front_hides_the_terrain() {
        let (mut doc, _) = terrain();
        let ray = Ray {
            origin: Vec3::new(100.0, 200.0, 500.0),
            direction: -Vec3::Z,
        };
        assert!(pick_displacement(&doc, &ray).is_some());
        let roof = Brush::cuboid(Aabb::from_corners(
            Vec3::new(0.0, 0.0, 200.0),
            Vec3::new(512.0, 512.0, 216.0),
        ))
        .unwrap();
        doc.execute(Command::AddBrushes(vec![roof])).unwrap();
        assert_eq!(pick_displacement(&doc, &ray), None);
    }
}
