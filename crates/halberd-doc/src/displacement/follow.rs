//! Keeping displacements on their faces when a brush is moved, turned,
//! resized, mirrored or reshaped: the grid starts from the same corner of
//! the new face, the directions points move in turn with the face, and a
//! mirrored grid is flipped so the terrain is mirrored too.

use super::{Displacement, ordered_corners};
use crate::FaceInfo;
use glam::{Mat3, Vec3};
use halberd_geom::Brush;
use halberd_kv::Entry;

/// How near a corner must be to a face's plane to be on it, in units.
const ON_PLANE: f32 = 0.01;

/// Updates the displacements in `faces` (one per face of `new`, each taken
/// from the face of `old` that `new`'s face came from) to fit `new`.
pub(crate) fn follow_shape(old: &Brush, new: &Brush, faces: &mut [FaceInfo]) {
    for (shape, info) in new.faces().iter().zip(faces.iter_mut()) {
        let Some(disp) = info.displacement() else {
            continue;
        };
        if let Some(moved) = follow_face(old, new, shape.source(), &disp)
            && moved != disp
        {
            *info = info.with_displacement(Some(&moved));
        }
    }
}

/// Displacement `disp` of `old` face `face`, fitted to the face of `new`
/// made from it.
fn follow_face(old: &Brush, new: &Brush, face: usize, disp: &Displacement) -> Option<Displacement> {
    let was = old.faces().get(face)?;
    let now = new.faces().iter().find(|f| f.source() == face)?;
    let (n_old, n_new) = (was.plane().normal, now.plane().normal);
    let from = ordered_corners(was.vertices(), n_old, disp.start)?;
    // Where each of the grid's corners went.
    let mut to = [Vec3::ZERO; 4];
    for (place, corner) in to.iter_mut().zip(from) {
        *place = matching_corner(old, new, face, corner)?;
    }
    let laid = ordered_corners(now.vertices(), n_new, to[0])?;
    // A mirror turns the face's winding round: the grid's rows now run
    // where its columns did.
    let flipped = laid[1].distance_squared(to[3]) < laid[1].distance_squared(to[1]);
    // How directions change: the grid's two edges and the face's normal
    // before, to the same after.
    let before = Mat3::from_cols(from[1] - from[0], from[3] - from[0], n_old);
    let after = Mat3::from_cols(to[1] - to[0], to[3] - to[0], n_new);
    let turn = (before.determinant().abs() > 1e-6)
        .then(|| after * before.inverse())
        .filter(|m| m.is_finite())
        .unwrap_or(Mat3::IDENTITY);
    let mut moved = Displacement {
        start: to[0],
        normals: disp
            .normals
            .iter()
            .map(|n| {
                // Hammer's "0 0 0" for a point it never moved stays so.
                if *n == Vec3::ZERO {
                    Vec3::ZERO
                } else {
                    (turn * *n).normalize_or(n_new)
                }
            })
            .collect(),
        offsets: disp.offsets.iter().map(|o| turn * *o).collect(),
        ..disp.clone()
    };
    if flipped {
        let side = moved.side();
        transpose(&mut moved.normals, side);
        transpose(&mut moved.distances, side);
        transpose(&mut moved.offsets, side);
        transpose(&mut moved.alphas, side);
    }
    if flipped || !turn.abs_diff_eq(Mat3::IDENTITY, 1e-5) {
        // Kept data in the grid's old order or directions no longer fits:
        // the offsets' directions are written again from the normals, and
        // Hammer works out the walkable triangles again.
        moved.extra.retain(
            |e| !matches!(e, Entry::Block(b) if b.name.eq_ignore_ascii_case("offset_normals")),
        );
        moved.forget_derived();
    }
    Some(moved)
}

/// Swaps rows and columns of a `side` × `side` grid.
fn transpose<T: Copy>(values: &mut [T], side: usize) {
    if values.len() != side * side {
        return;
    }
    let was = values.to_vec();
    for r in 0..side {
        for c in 0..side {
            values[r * side + c] = was[c * side + r];
        }
    }
}

/// The corner of `new`'s face from `old` face `face` that is where `old`'s
/// corner nearest `point` was: the one on the same neighbouring faces.
fn matching_corner(old: &Brush, new: &Brush, face: usize, point: Vec3) -> Option<Vec3> {
    let was = old.faces().get(face)?;
    let corner = was.vertices().iter().copied().min_by(|a, b| {
        a.distance_squared(point)
            .total_cmp(&b.distance_squared(point))
    })?;
    // The old faces meeting at that corner, by index.
    let meeting: Vec<usize> = old
        .faces()
        .iter()
        .enumerate()
        .filter(|(_, f)| f.plane().signed_distance(corner).abs() < ON_PLANE)
        .map(|(k, _)| k)
        .collect();
    let now = new.faces().iter().find(|f| f.source() == face)?;
    now.vertices().iter().copied().max_by_key(|v| {
        new.faces()
            .iter()
            .filter(|f| {
                f.plane().signed_distance(*v).abs() < ON_PLANE && meeting.contains(&f.source())
            })
            .count()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BrushObject, ordered_corners};
    use halberd_geom::Aabb;

    fn block() -> Brush {
        Brush::cuboid(Aabb::from_corners(
            Vec3::ZERO,
            Vec3::new(256.0, 128.0, 64.0),
        ))
        .unwrap()
    }

    /// The box with a raised displacement on its top, started at the
    /// corner at (256, 0, 64).
    fn terrain() -> BrushObject {
        let brush = block();
        let top = brush
            .faces()
            .iter()
            .position(|f| f.plane().normal.z > 0.9)
            .unwrap();
        let mut disp = Displacement::flat(2, Vec3::new(256.0, 0.0, 64.0), Vec3::Z);
        disp.raise(6, 40.0, Vec3::Z);
        let mut faces = BrushObject::new(brush.clone()).faces().to_vec();
        faces[top] = faces[top].with_displacement(Some(&disp));
        BrushObject::with_faces(brush, faces, Vec::new())
    }

    fn top_disp(object: &BrushObject) -> (Displacement, [Vec3; 4]) {
        let (shape, info) = object
            .brush()
            .faces()
            .iter()
            .zip(object.faces())
            .find(|(_, i)| i.displacement().is_some())
            .unwrap();
        let disp = info.displacement().unwrap();
        let corners = ordered_corners(shape.vertices(), shape.plane().normal, disp.start).unwrap();
        (disp, corners)
    }

    #[test]
    fn moving_takes_the_start_corner_along() {
        let object = terrain();
        let offset = Vec3::new(1000.0, -700.0, 32.0);
        let moved = object.with_shape(object.brush().translated(offset).unwrap());
        let (disp, corners) = top_disp(&moved);
        assert_eq!(disp.start, Vec3::new(256.0, 0.0, 64.0) + offset);
        let (before, old_corners) = top_disp(&object);
        let old_points = before.points(&old_corners).unwrap();
        let new_points = disp.points(&corners).unwrap();
        for (a, b) in old_points.iter().zip(&new_points) {
            assert!((*a + offset - *b).length() < 1e-3, "{a} {b}");
        }
    }

    #[test]
    fn stretching_keeps_the_same_corner() {
        let object = terrain();
        let bigger = Brush::cuboid(Aabb::from_corners(
            Vec3::new(-64.0, 0.0, 0.0),
            Vec3::new(512.0, 300.0, 96.0),
        ))
        .unwrap();
        let (disp, _) = top_disp(&object.with_shape(bigger));
        assert_eq!(disp.start, Vec3::new(512.0, 0.0, 96.0));
        assert_eq!(disp.distances[6], 40.0);
    }

    #[test]
    fn plain_faces_are_left_alone() {
        let plain = BrushObject::new(block());
        let moved = plain.with_shape(plain.brush().translated(Vec3::X).unwrap());
        assert!(moved.faces().iter().all(|f| f.displacement().is_none()));
    }

    /// A lopsided terrain: every point raised by a different amount, one
    /// pushed sideways and one blended.
    fn lopsided() -> BrushObject {
        let brush = block();
        let top = brush
            .faces()
            .iter()
            .position(|f| f.plane().normal.z > 0.9)
            .unwrap();
        let mut disp = Displacement::flat(2, Vec3::new(256.0, 0.0, 64.0), Vec3::Z);
        for k in 0..25 {
            disp.raise(k, k as f32 * 3.0, Vec3::Z);
        }
        disp.offsets[7] = Vec3::new(10.0, 0.0, 0.0);
        disp.set_alpha(3, 255.0);
        let mut faces = BrushObject::new(brush.clone()).faces().to_vec();
        faces[top] = faces[top].with_displacement(Some(&disp));
        BrushObject::with_faces(brush, faces, Vec::new())
    }

    /// The surface's points (and blends), in no particular order.
    fn surface(object: &BrushObject) -> Vec<(Vec3, f32)> {
        let (disp, corners) = top_disp(object);
        disp.points(&corners)
            .unwrap()
            .into_iter()
            .zip(disp.alphas.iter().copied())
            .collect()
    }

    fn same_surface(got: &[(Vec3, f32)], want: &[(Vec3, f32)]) {
        assert_eq!(got.len(), want.len());
        for (p, a) in want {
            assert!(
                got.iter().any(|(q, b)| q.distance(*p) < 0.01 && a == b),
                "{p} (blend {a}) is missing"
            );
        }
    }

    #[test]
    fn mirroring_mirrors_the_terrain() {
        let object = lopsided();
        let before = surface(&object);
        let mirrored = object.with_shape(object.brush().mirrored(0, 128.0).unwrap());
        let want: Vec<(Vec3, f32)> = before
            .iter()
            .map(|(p, a)| (Vec3::new(256.0 - p.x, p.y, p.z), *a))
            .collect();
        same_surface(&surface(&mirrored), &want);
        // Twice is back where it started.
        let back = mirrored.with_shape(mirrored.brush().mirrored(0, 128.0).unwrap());
        same_surface(&surface(&back), &before);
    }

    #[test]
    fn turning_turns_the_terrain_with_its_sideways_parts() {
        let object = lopsided();
        let before = surface(&object);
        let pivot = Vec3::new(128.0, 64.0, 0.0);
        let angle = std::f32::consts::FRAC_PI_2;
        let turned = object.with_shape(object.brush().rotated(Vec3::Z, angle, pivot).unwrap());
        let spin = glam::Quat::from_rotation_z(angle);
        let want: Vec<(Vec3, f32)> = before
            .iter()
            .map(|(p, a)| (pivot + spin * (*p - pivot), *a))
            .collect();
        same_surface(&surface(&turned), &want);
    }
}
