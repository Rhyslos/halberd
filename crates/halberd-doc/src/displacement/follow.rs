//! Keeping displacements on their faces when a brush is moved, turned,
//! resized or reshaped: the start corner goes to the same corner of the
//! new face, and the directions points move in turn with the face.

use super::Displacement;
use crate::FaceInfo;
use glam::{Quat, Vec3};
use halberd_geom::Brush;

/// How near a corner must be to a face's plane to be on it, in units.
const ON_PLANE: f32 = 0.01;

/// Updates the displacements in `faces` (one per face of `new`, each taken
/// from the face of `old` that `new`'s face came from) to fit `new`.
pub(crate) fn follow_shape(old: &Brush, new: &Brush, faces: &mut [FaceInfo]) {
    for (shape, info) in new.faces().iter().zip(faces.iter_mut()) {
        let Some(disp) = info.displacement() else {
            continue;
        };
        let Some(was) = old.faces().get(shape.source()) else {
            continue;
        };
        let Some(start) = matching_corner(old, new, shape.source(), disp.start) else {
            continue;
        };
        let turn = Quat::from_rotation_arc(was.plane().normal, shape.plane().normal);
        let moved = Displacement {
            start,
            normals: disp.normals.iter().map(|n| turn * *n).collect(),
            offsets: disp.offsets.iter().map(|o| turn * *o).collect(),
            ..disp.clone()
        };
        if moved != disp {
            *info = info.with_displacement(Some(&moved));
        }
    }
}

/// The corner of `new`'s face from `old` face `face` that is where `old`'s
/// corner nearest `start` was: the one on the same neighbouring faces.
fn matching_corner(old: &Brush, new: &Brush, face: usize, start: Vec3) -> Option<Vec3> {
    let was = old.faces().get(face)?;
    let corner = was.vertices().iter().copied().min_by(|a, b| {
        a.distance_squared(start)
            .total_cmp(&b.distance_squared(start))
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
        disp.raise(6, 40.0);
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
}
