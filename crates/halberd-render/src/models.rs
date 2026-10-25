//! Prop models, ready to be drawn wherever props show them.

use std::collections::HashSet;

/// One part of a prop model drawn with one material.
#[derive(Debug, Clone, PartialEq)]
pub struct PropMesh {
    /// Its material's full name (`MODELS/PROPS_C17/OILDRUM001`).
    pub material: String,
    /// Its corners, in the model's own space.
    pub vertices: Vec<halberd_mdl::Vertex>,
    /// Its triangles (three indices into `vertices` each).
    pub indices: Vec<u32>,
    /// Each edge of its triangles once, as pairs of indices (for outlines).
    pub edges: Vec<[u32; 2]>,
}

/// A model as the renderer keeps it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PropModel {
    /// Its meshes.
    pub meshes: Vec<PropMesh>,
}

impl PropModel {
    /// A model read by `halberd-mdl`, with the full name of each of its
    /// materials (`materials[i]` for the model's material `i`).
    pub fn new(model: &halberd_mdl::Model, materials: &[String]) -> Self {
        let meshes = model
            .meshes
            .iter()
            .map(|mesh| {
                let indices = facing_out(&mesh.vertices, &mesh.indices);
                let mut seen = HashSet::new();
                let mut edges = Vec::new();
                for t in indices.chunks_exact(3) {
                    for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                        if seen.insert((a.min(b), a.max(b))) {
                            edges.push([a, b]);
                        }
                    }
                }
                PropMesh {
                    material: materials.get(mesh.material).cloned().unwrap_or_default(),
                    vertices: mesh.vertices.clone(),
                    indices,
                    edges,
                }
            })
            .collect();
        Self { meshes }
    }

    /// The smallest box around its corners (min, max), if it has any.
    pub fn bounds(&self) -> Option<(glam::Vec3, glam::Vec3)> {
        let mut points = self
            .meshes
            .iter()
            .flat_map(|m| &m.vertices)
            .map(|v| glam::Vec3::from(v.position))
            .filter(|p| p.is_finite());
        let first = points.next()?;
        Some(points.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p))))
    }

    /// How many triangles it has.
    pub fn triangle_count(&self) -> usize {
        self.meshes.iter().map(|m| m.indices.len() / 3).sum()
    }
}

/// `indices` with each triangle's corners in the order that faces the way
/// its corners' normals do (counter-clockwise seen from outside), so the
/// view hides back faces correctly whichever way the model was written.
fn facing_out(vertices: &[halberd_mdl::Vertex], indices: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(indices.len());
    for t in indices.chunks_exact(3) {
        let corner = |k: usize| vertices.get(t[k] as usize);
        let (Some(a), Some(b), Some(c)) = (corner(0), corner(1), corner(2)) else {
            continue;
        };
        let p = |v: &halberd_mdl::Vertex| glam::Vec3::from(v.position);
        let n = |v: &halberd_mdl::Vertex| glam::Vec3::from(v.normal);
        let geometric = (p(b) - p(a)).cross(p(c) - p(a));
        if geometric.dot(n(a) + n(b) + n(c)) < 0.0 {
            out.extend([t[0], t[2], t[1]]);
        } else {
            out.extend_from_slice(t);
        }
    }
    out
}

/// What the renderer knows of a model.
#[derive(Debug)]
enum Known {
    /// Asked for; not here yet.
    Asked,
    Ready(PropModel),
    /// It could not be read; its props stay marker boxes.
    Missing,
}

/// The models props show, as they arrive.
#[derive(Debug, Default)]
pub(crate) struct ModelCache {
    /// By lower-case name.
    known: std::collections::HashMap<String, Known>,
    /// Models wanted and not yet handed out by `take_wanted`.
    wanted: Vec<String>,
}

impl ModelCache {
    /// Model `name`, if it has arrived.
    pub(crate) fn get(&self, name: &str) -> Option<&PropModel> {
        match self.known.get(&name.to_ascii_lowercase()) {
            Some(Known::Ready(model)) => Some(model),
            _ => None,
        }
    }

    /// Notes every model the map's props show, wanting those not seen yet.
    pub(crate) fn note_used(&mut self, doc: &halberd_doc::Document) {
        for (_, object) in doc.objects() {
            let Some(model) = object.as_entity().and_then(|e| e.model()) else {
                continue;
            };
            let key = model.to_ascii_lowercase();
            if !self.known.contains_key(&key) {
                self.known.insert(key.clone(), Known::Asked);
                self.wanted.push(key);
            }
        }
    }

    /// Models wanted since the last call (each once).
    pub(crate) fn take_wanted(&mut self) -> Vec<String> {
        std::mem::take(&mut self.wanted)
    }

    /// Model `name` arrived (`None`: it can't be shown). Kept only if the
    /// map's props asked for it (models read for other reasons, such as
    /// browser pictures, are not kept).
    pub(crate) fn arrived(&mut self, name: &str, model: Option<PropModel>) -> bool {
        let Some(known) = self.known.get_mut(&name.to_ascii_lowercase()) else {
            return false;
        };
        *known = match model {
            Some(model) => Known::Ready(model),
            None => Known::Missing,
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halberd_mdl::example::{ExampleModel, box_mesh, model_files};

    #[test]
    fn triangles_are_turned_to_face_out_and_edges_listed_once() {
        let files = model_files(&ExampleModel {
            materials: &["crate"],
            meshes: &[box_mesh([0.0; 3], [16.0; 3], 0)],
            ..ExampleModel::default()
        });
        let mut model = halberd_mdl::Model::parse(&files.mdl, &files.vvd, &files.vtx).unwrap();
        // Written the other way round, as some compilers do.
        for t in model.meshes[0].indices.chunks_exact_mut(3) {
            t.swap(1, 2);
        }
        let prop = PropModel::new(&model, &["MODELS/CRATE".to_string()]);
        let mesh = &prop.meshes[0];
        assert_eq!(mesh.material, "MODELS/CRATE");
        assert_eq!(prop.triangle_count(), 12);
        for t in mesh.indices.chunks_exact(3) {
            let p = |k: usize| glam::Vec3::from(mesh.vertices[t[k] as usize].position);
            let n = glam::Vec3::from(mesh.vertices[t[0] as usize].normal);
            assert!((p(1) - p(0)).cross(p(2) - p(0)).dot(n) > 0.0);
        }
        // 6 sides × 4 outline edges + 1 diagonal each.
        assert_eq!(mesh.edges.len(), 30);
    }

    #[test]
    fn models_are_wanted_once_and_kept_when_they_arrive() {
        use halberd_doc::{Command, Document, EntityObject};
        let mut doc = Document::new();
        for x in [0.0, 64.0] {
            doc.execute(Command::AddEntity(EntityObject::point(
                "prop_static",
                glam::Vec3::new(x, 0.0, 0.0),
                &[("model", "Models/Crate.mdl")],
            )))
            .unwrap();
        }
        let mut cache = ModelCache::default();
        cache.note_used(&doc);
        assert_eq!(cache.take_wanted(), ["models/crate.mdl"]);
        cache.note_used(&doc);
        assert!(cache.take_wanted().is_empty());
        assert!(cache.get("models/crate.mdl").is_none());
        assert!(cache.arrived("MODELS/CRATE.MDL", Some(PropModel::default())));
        assert!(cache.get("models/crate.mdl").is_some());
        // Models no prop asked for are not kept.
        assert!(!cache.arrived("models/other.mdl", Some(PropModel::default())));
        assert!(cache.get("models/other.mdl").is_none());
    }
}
