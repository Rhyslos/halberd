//! A whole model, put together from its three files.

use crate::{MdlError, mdl, vtx, vvd};

/// One corner of a model's triangles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    /// Where it is, in the model's own space (Hammer units, Z up).
    pub position: [f32; 3],
    /// Which way the surface faces there.
    pub normal: [f32; 3],
    /// Texture coordinates (0 to 1 across the texture, V downwards).
    pub uv: [f32; 2],
}

/// Part of a model drawn with one material.
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    /// Its material: an index into [`Model::materials`].
    pub material: usize,
    /// Its corners.
    pub vertices: Vec<Vertex>,
    /// Its triangles: three indices into `vertices` each.
    pub indices: Vec<u32>,
}

/// A model, ready to draw: its default look (first skin, first model of
/// each body part, full detail).
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    /// The name the model was compiled with (`props_c17/oildrum001.mdl`).
    pub name: String,
    /// True if it was made for `prop_static` (`$staticprop`).
    pub static_prop: bool,
    /// Its material names, without folder (`oildrum001`).
    pub materials: Vec<String>,
    /// Folders to look for its materials in, in order (`models/props_c17/`).
    pub material_folders: Vec<String>,
    /// Its meshes.
    pub meshes: Vec<Mesh>,
    /// The smallest box around its triangles (min, max); all zero when it
    /// has none.
    pub bounds: ([f32; 3], [f32; 3]),
}

impl Model {
    /// Reads a model from its `.mdl`, `.vvd` and `.vtx` (`.dx90.vtx`)
    /// files. Damaged or mismatched files give a reason, never a crash.
    pub fn parse(mdl: &[u8], vvd: &[u8], vtx: &[u8]) -> Result<Self, MdlError> {
        let info = mdl::parse(mdl)?;
        let vertices = vvd::parse(vvd, info.checksum)?;
        let triangles = vtx::parse(vtx, info.checksum)?;
        // Meshes may claim the same vertices over and over; their total
        // is capped before anything is copied.
        let claimed: usize = info
            .body_parts
            .iter()
            .filter_map(|models| models.first())
            .flat_map(|model| &model.meshes)
            .map(|mesh| mesh.vertex_count)
            .fold(0usize, usize::saturating_add);
        if claimed > crate::MAX_VERTICES {
            return Err(MdlError::TooLarge("its number of vertices"));
        }
        let skin = info.skins.first();
        let mut meshes = Vec::new();
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for (part, models) in info.body_parts.iter().enumerate() {
            let Some(model) = models.first() else {
                continue;
            };
            let mesh_corners = triangles
                .get(part)
                .and_then(|m| m.first())
                .ok_or(MdlError::Damaged(".vtx", "a body part is missing"))?;
            for (i, mesh) in model.meshes.iter().enumerate() {
                let corners = mesh_corners
                    .get(i)
                    .ok_or(MdlError::Damaged(".vtx", "a mesh is missing"))?;
                if corners.is_empty() {
                    continue;
                }
                let first = model.vertex_start.saturating_add(mesh.vertex_offset);
                let mesh_vertices = first
                    .checked_add(mesh.vertex_count)
                    .and_then(|end| vertices.get(first..end))
                    .ok_or(MdlError::Damaged(".vvd", "a mesh's vertices are missing"))?;
                if corners.iter().any(|&c| c as usize >= mesh_vertices.len()) {
                    return Err(MdlError::Damaged(
                        ".vtx",
                        "a triangle uses a missing vertex",
                    ));
                }
                let texture = match skin {
                    Some(row) => row.get(mesh.material).map_or(0, |&t| usize::from(t)),
                    None => mesh.material,
                };
                for v in mesh_vertices {
                    for k in 0..3 {
                        min[k] = min[k].min(v.position[k]);
                        max[k] = max[k].max(v.position[k]);
                    }
                }
                meshes.push(Mesh {
                    material: texture.min(info.textures.len().saturating_sub(1)),
                    vertices: mesh_vertices.to_vec(),
                    indices: corners.clone(),
                });
            }
        }
        let finite = min.iter().chain(&max).all(|v| v.is_finite());
        let bounds = if finite {
            (min, max)
        } else {
            ([0.0; 3], [0.0; 3])
        };
        Ok(Self {
            name: info.name.replace('\\', "/"),
            static_prop: info.is_static_prop(),
            materials: info.textures,
            material_folders: info.texture_dirs,
            meshes,
            bounds,
        })
    }

    /// How many triangles it has.
    pub fn triangle_count(&self) -> usize {
        self.meshes.iter().map(|m| m.indices.len() / 3).sum()
    }
}
