//! The `.vvd` file: the model's vertices (position, normal, texture
//! coordinates).
//!
//! Layout (vertexFileHeader_t): `IDSV`, version 4, checksum, LOD count,
//! vertex count per LOD (8), fixup count, fixup table offset, vertex data
//! offset, tangent data offset. Fixups, when present, say which runs of
//! the stored vertices make up each level of detail.

use crate::bytes::Bytes;
use crate::mdl::VERTEX_SIZE;
use crate::{MAX_VERTICES, MdlError, Vertex};

const FILE: &str = ".vvd";
const SIGNATURE: &[u8; 4] = b"IDSV";
const MAX_FIXUPS: usize = 1 << 16;

/// The full-detail vertices, checked against the `.mdl`'s checksum.
pub(crate) fn parse(data: &[u8], checksum: i32) -> Result<Vec<Vertex>, MdlError> {
    let b = Bytes::new(data, FILE);
    if b.slice(0, 4).ok() != Some(SIGNATURE.as_slice()) {
        return Err(MdlError::WrongKind(FILE));
    }
    let version = b.i32(4)?;
    if version != 4 {
        return Err(MdlError::UnsupportedVersion(FILE, version));
    }
    if b.i32(8)? != checksum {
        return Err(MdlError::Mismatched);
    }
    let lod0 = b.count(16, MAX_VERTICES, "its number of vertices")?;
    let fixup_count = b.count(48, MAX_FIXUPS, "its number of fixups")?;
    let fixup_at = b.offset(0, 52)?;
    let vertex_at = b.offset(0, 56)?;
    let stored = (data.len() - vertex_at) / VERTEX_SIZE;

    let mut runs = Vec::new();
    if fixup_count == 0 {
        runs.push((0, lod0));
    } else {
        for i in 0..fixup_count {
            // vertexFileFixup_t: LOD, first vertex, count. Every fixup
            // whose LOD is 0 or more belongs to the full-detail model.
            let at = fixup_at + i * 12;
            let lod = b.i32(at)?;
            let first = b.count(at + 4, MAX_VERTICES, "a fixup")?;
            let count = b.count(at + 8, MAX_VERTICES, "a fixup")?;
            if lod >= 0 {
                runs.push((first, count));
            }
        }
    }
    let total: usize = runs.iter().map(|&(_, n)| n).sum();
    if total > MAX_VERTICES {
        return Err(MdlError::TooLarge("its number of vertices"));
    }
    if runs.iter().any(|&(first, count)| first.saturating_add(count) > stored) {
        return Err(MdlError::Damaged(FILE, "a vertex is missing"));
    }
    let mut vertices = Vec::with_capacity(total);
    for (first, count) in runs {
        for v in first..first + count {
            let at = vertex_at + v * VERTEX_SIZE;
            vertices.push(Vertex {
                position: b.vec3(at + 16)?,
                normal: b.vec3(at + 28)?,
                uv: [b.f32(at + 40)?, b.f32(at + 44)?],
            });
        }
    }
    Ok(vertices)
}
