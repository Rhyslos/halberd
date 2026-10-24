//! The `.vtx` file (`.dx90.vtx`): the triangles of each mesh, as indices
//! into the mesh's vertices.
//!
//! Layout (OptimizedModel::FileHeader_t, 36 bytes): version 7, cache
//! size, bone limits, checksum, LOD count, material replacement offset,
//! body part count and offset. Then body parts → models → levels of
//! detail → meshes → strip groups, each found by count and offset from
//! the structure that holds it. A strip group lists its vertices (each
//! naming a vertex of the mesh) and triangle indices into that list.
//! Halberd reads the full-detail level only.

use crate::MdlError;
use crate::bytes::Bytes;

const FILE: &str = ".vtx";
const MAX_LIST: usize = 4096;
const MAX_STRIP_GROUPS: usize = 64;
/// Most triangle indices read in all.
const MAX_INDICES: usize = 1 << 23;

/// Per body part, per model, per mesh: triangle corners as vertex numbers
/// within the mesh.
pub(crate) type Triangles = Vec<Vec<Vec<Vec<u32>>>>;

pub(crate) fn parse(data: &[u8], checksum: i32) -> Result<Triangles, MdlError> {
    let b = Bytes::new(data, FILE);
    let version = b.i32(0)?;
    if version != 7 {
        return Err(MdlError::UnsupportedVersion(FILE, version));
    }
    if b.i32(12)? != checksum {
        return Err(MdlError::Mismatched);
    }
    // Newer compilers add two topology fields to each strip group (33
    // bytes instead of 25); the header doesn't say which, so the usual
    // size is tried first.
    match walk(&b, 25) {
        Ok(found) => Ok(found),
        Err(first) => walk(&b, 33).map_err(|_| first),
    }
}

fn walk(b: &Bytes<'_>, group_size: usize) -> Result<Triangles, MdlError> {
    let mut total = 0usize;
    let part_count = b.count(28, MAX_LIST, "its number of body parts")?;
    let parts_at = b.offset(0, 32)?;
    let mut parts = Vec::with_capacity(part_count);
    for p in 0..part_count {
        let part_at = parts_at + p * 8;
        let model_count = b.count(part_at, MAX_LIST, "its number of models")?;
        let models_at = b.offset(part_at, part_at + 4)?;
        let mut models = Vec::with_capacity(model_count.min(16));
        for m in 0..model_count {
            let model_at = models_at + m * 8;
            let lods = b.count(model_at, 8, "its number of levels of detail")?;
            if lods == 0 {
                models.push(Vec::new());
                continue;
            }
            let lod_at = b.offset(model_at, model_at + 4)?;
            let mesh_count = b.count(lod_at, MAX_LIST, "its number of meshes")?;
            let meshes_at = b.offset(lod_at, lod_at + 4)?;
            let mut meshes = Vec::with_capacity(mesh_count);
            for i in 0..mesh_count {
                // MeshHeader_t: 9 bytes (packed).
                let mesh_at = meshes_at + i * 9;
                meshes.push(mesh(b, mesh_at, group_size, &mut total)?);
            }
            models.push(meshes);
        }
        parts.push(models);
    }
    Ok(parts)
}

/// One mesh's triangles.
fn mesh(b: &Bytes<'_>, at: usize, group_size: usize, total: &mut usize) -> Result<Vec<u32>, MdlError> {
    let group_count = b.count(at, MAX_STRIP_GROUPS, "its number of strip groups")?;
    let groups_at = b.offset(at, at + 4)?;
    let mut corners = Vec::new();
    for g in 0..group_count {
        let group_at = groups_at + g * group_size;
        let vert_count = b.count(group_at, 1 << 16, "a strip group's vertices")?;
        let verts_at = b.offset(group_at, group_at + 4)?;
        let index_count = b.count(group_at + 8, MAX_INDICES, "a strip group's indices")?;
        let indices_at = b.offset(group_at, group_at + 12)?;
        if index_count % 3 != 0 {
            return Err(MdlError::Damaged(FILE, "a triangle has a missing corner"));
        }
        *total += index_count;
        if *total > MAX_INDICES {
            return Err(MdlError::TooLarge("its number of triangles"));
        }
        // Check the whole lists are there before reading them.
        b.slice(verts_at, vert_count * 9)?;
        b.slice(indices_at, index_count * 2)?;
        corners.reserve(index_count);
        for i in 0..index_count {
            let index = usize::from(b.u16(indices_at + i * 2)?);
            if index >= vert_count {
                return Err(MdlError::Damaged(FILE, "a triangle uses a missing vertex"));
            }
            // Vertex_t: 9 bytes; the mesh vertex it names is at 4.
            corners.push(u32::from(b.u16(verts_at + index * 9 + 4)?));
        }
    }
    Ok(corners)
}
