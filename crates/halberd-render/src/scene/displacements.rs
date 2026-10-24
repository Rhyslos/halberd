//! Displacement faces: drawn as their grid of triangles, with the blend
//! between their two textures shown as a warm tint (the second texture
//! itself isn't drawn yet), and the grid's lines as outlines.

use super::{FaceVertex, TEXTURE_SIZE};
use crate::lines::LineVertex;
use glam::Vec3;
use halberd_doc::{Displacement, ordered_corners};
use halberd_geom::Face;

/// Colour the blend amount tints a displacement towards.
const BLEND_TINT: [f32; 3] = [0.85, 0.45, 0.12];
/// How strongly a full blend tints.
const BLEND_STRENGTH: f32 = 0.65;

/// How a displacement's vertices look, besides where they are.
pub(super) struct DispLook {
    pub color: [f32; 4],
    pub edge_color: [f32; 4],
    pub hue: f32,
    pub flag: f32,
    pub axes: Option<halberd_doc::TextureAxes>,
}

/// Adds `face` as displacement `disp` to `faces` and `edges`. Returns
/// false if it can't be drawn as one (the face isn't four-sided), so it is
/// drawn flat.
pub(super) fn add_displacement(
    faces: &mut Vec<FaceVertex>,
    face: &Face,
    disp: &Displacement,
    look: &DispLook,
    edges: &mut Vec<LineVertex>,
) -> bool {
    let normal = face.plane().normal;
    let Some(corners) = ordered_corners(face.vertices(), normal, disp.start) else {
        return false;
    };
    let (Some(points), Some(base)) = (disp.points(&corners), disp.base_points(&corners)) else {
        return false;
    };
    let side = disp.side();
    let vertex = |k: usize, n: Vec3| {
        let alpha = disp.alphas.get(k).copied().unwrap_or(0.0) / 255.0 * BLEND_STRENGTH;
        let mut color = look.color;
        for (c, tint) in color.iter_mut().zip(BLEND_TINT) {
            *c += (tint - *c) * alpha;
        }
        FaceVertex {
            position: points[k].to_array(),
            normal: n.to_array(),
            color,
            texture: match look.axes {
                Some(axes) => {
                    let [u, v] = axes.texels(base[k]);
                    [u / TEXTURE_SIZE, v / TEXTURE_SIZE, look.hue, look.flag]
                }
                None => [0.0; 4],
            },
        }
    };
    // Wind the triangles as the face's own corners wind, so the side
    // that faces out is drawn (the other side is culled). Along a row the
    // grid steps from the start corner towards the second corner, which
    // with Source's order winds against the face's normal.
    let v = face.vertices();
    let face_winds_out = v.len() >= 3 && (v[1] - v[0]).cross(v[2] - v[0]).dot(normal) > 0.0;
    for row in 0..side - 1 {
        for col in 0..side - 1 {
            let a = row * side + col;
            let (b, c, d) = (a + 1, a + side, a + side + 1);
            let triangles = if face_winds_out {
                [[a, b, c], [b, d, c]]
            } else {
                [[a, c, b], [b, c, d]]
            };
            for [i, j, k] in triangles {
                let mut n = (points[j] - points[i]).cross(points[k] - points[i]);
                // Face the same way as the face.
                if n.dot(normal) < 0.0 {
                    n = -n;
                }
                let n = n.normalize_or(normal);
                faces.extend([vertex(i, n), vertex(j, n), vertex(k, n)]);
            }
        }
    }
    let line = |p: Vec3| LineVertex {
        position: p.to_array(),
        color: look.edge_color,
    };
    for row in 0..side {
        for col in 0..side - 1 {
            let a = row * side + col;
            edges.extend([line(points[a]), line(points[a + 1])]);
            let (r, c) = (col, row);
            let b = r * side + c;
            edges.extend([line(points[b]), line(points[b + side])]);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::super::SceneGeometry;
    use glam::Vec3;
    use halberd_doc::{BrushObject, Command, Displacement, Document, ordered_corners};
    use halberd_geom::{Aabb, Brush};

    /// One 256-unit box; with `raise`, its top is a displacement raised in
    /// the middle and fully blended at its first point.
    fn map(raise: bool) -> Document {
        let brush = Brush::cuboid(Aabb::from_corners(Vec3::ZERO, Vec3::splat(256.0))).unwrap();
        let mut doc = Document::new();
        doc.execute(Command::AddBrushes(vec![brush.clone()]))
            .unwrap();
        if !raise {
            return doc;
        }
        let id = *doc.selection().iter().next().unwrap();
        let top = brush
            .faces()
            .iter()
            .position(|f| f.plane().normal.z > 0.9)
            .unwrap();
        let shape = &brush.faces()[top];
        let corners = ordered_corners(shape.vertices(), Vec3::Z, shape.vertices()[0]).unwrap();
        let mut disp = Displacement::flat(2, corners[0], Vec3::Z);
        disp.raise(12, 100.0, Vec3::Z);
        disp.set_alpha(0, 255.0);
        let object = doc.get(id).unwrap().as_brush().unwrap();
        let mut faces = object.faces().to_vec();
        faces[top] = faces[top].with_displacement(Some(&disp));
        let changed = BrushObject::with_faces(brush, faces, Vec::new());
        doc.execute(Command::ShapeEdit {
            label: "Make displacement".into(),
            changed: vec![(id, changed)],
            added: vec![],
            removed: vec![],
            select_added: false,
        })
        .unwrap();
        doc
    }

    #[test]
    fn a_displacement_is_drawn_as_its_grid() {
        let flat = SceneGeometry::from_document(&map(false));
        let disp = SceneGeometry::from_document(&map(true));
        // A 5 × 5 grid is 4 × 4 squares of two triangles: 96 vertices in
        // place of the top's 2 triangles.
        assert_eq!(disp.faces.len(), flat.faces.len() - 6 + 96);
        let highest = disp
            .faces
            .iter()
            .map(|v| v.position[2])
            .fold(f32::MIN, f32::max);
        assert!((highest - 356.0).abs() < 1e-3, "{highest}");
        let lines = |s: &SceneGeometry| s.edges.len() + s.selected_edges.len();
        assert!(lines(&disp) > lines(&flat), "grid lines are drawn");
        // The blended corner is tinted, unlike the rest.
        let colors: Vec<[f32; 4]> = disp
            .faces
            .iter()
            .filter(|v| v.position[2] > 255.0)
            .map(|v| v.color)
            .collect();
        assert!(colors.iter().any(|c| c != &colors[colors.len() - 1]));
        assert!(disp.faces.iter().all(|v| v.normal[2].is_finite()));
        // Every triangle winds the same way round its normal as the flat
        // faces do, so none is culled when seen from outside.
        let winding = |s: &SceneGeometry| -> Vec<bool> {
            s.faces
                .chunks_exact(3)
                .map(|t| {
                    let p = |k: usize| Vec3::from_array(t[k].position);
                    let n = Vec3::from_array(t[0].normal);
                    (p(1) - p(0)).cross(p(2) - p(0)).dot(n) > 0.0
                })
                .collect()
        };
        let flat_way = winding(&flat)[0];
        assert!(winding(&flat).iter().all(|w| *w == flat_way));
        assert!(winding(&disp).iter().all(|w| *w == flat_way));
    }
}
