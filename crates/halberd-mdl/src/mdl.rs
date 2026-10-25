//! The `.mdl` file: the model's name, flags, materials, skins and how its
//! meshes are laid out over the vertices in the `.vvd` file.
//!
//! Layout (studiohdr_t): `IDST`, version (44 to 49), checksum, a 64-byte
//! name, then fixed fields; lists are found by count and offset from the
//! start of the structure that holds them.

use crate::MdlError;
use crate::bytes::Bytes;

const FILE: &str = ".mdl";
const SIGNATURE: &[u8; 4] = b"IDST";
/// Size of one vertex in the `.vvd` file (mstudiovertex_t).
pub(crate) const VERTEX_SIZE: usize = 48;
/// Set on models made for `prop_static`.
const STATIC_PROP_FLAG: i32 = 0x10;

const MAX_TEXTURES: usize = 4096;
const MAX_TEXTURE_DIRS: usize = 256;
const MAX_SKIN_REFS: usize = 4096;
const MAX_SKIN_FAMILIES: usize = 1024;
const MAX_BODY_PARTS: usize = 256;
const MAX_MODELS: usize = 1024;
const MAX_MESHES: usize = 4096;
/// Most skin-table entries read (families × references).
const MAX_SKIN_ENTRIES: usize = 1 << 20;

/// What the `.mdl` file says.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MdlInfo {
    pub(crate) version: i32,
    pub(crate) checksum: i32,
    pub(crate) name: String,
    pub(crate) flags: i32,
    pub(crate) textures: Vec<String>,
    pub(crate) texture_dirs: Vec<String>,
    /// Per skin family, the texture each material reference uses.
    pub(crate) skins: Vec<Vec<u16>>,
    /// Per body part, its models (the first is shown).
    pub(crate) body_parts: Vec<Vec<MdlModel>>,
}

impl MdlInfo {
    pub(crate) fn is_static_prop(&self) -> bool {
        self.flags & STATIC_PROP_FLAG != 0
    }
}

/// One model of a body part.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MdlModel {
    /// Its first vertex in the `.vvd` vertices.
    pub(crate) vertex_start: usize,
    pub(crate) vertex_count: usize,
    pub(crate) meshes: Vec<MdlMesh>,
}

/// One mesh of a model: some of its vertices, with one material.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MdlMesh {
    /// Material reference (through the skin table).
    pub(crate) material: usize,
    /// Its first vertex, counted from the model's first.
    pub(crate) vertex_offset: usize,
    pub(crate) vertex_count: usize,
}

pub(crate) fn parse(data: &[u8]) -> Result<MdlInfo, MdlError> {
    let b = Bytes::new(data, FILE);
    if b.slice(0, 4).ok() != Some(SIGNATURE.as_slice()) {
        return Err(MdlError::WrongKind(FILE));
    }
    let version = b.i32(4)?;
    if !(44..=49).contains(&version) {
        return Err(MdlError::UnsupportedVersion(FILE, version));
    }
    let checksum = b.i32(8)?;
    let name = b.fixed_name(12, 64)?;
    let flags = b.i32(152)?;

    let texture_count = b.count(204, MAX_TEXTURES, "its number of materials")?;
    let texture_at = b.offset(0, 208)?;
    let mut textures = Vec::with_capacity(texture_count);
    for i in 0..texture_count {
        // mstudiotexture_t: 64 bytes; its name offset is from itself.
        let at = texture_at + i * 64;
        textures.push(b.name(b.offset(at, at)?)?);
    }

    let dir_count = b.count(212, MAX_TEXTURE_DIRS, "its number of material folders")?;
    let dir_at = b.offset(0, 216)?;
    let mut texture_dirs = Vec::with_capacity(dir_count);
    for i in 0..dir_count {
        texture_dirs.push(b.name(b.offset(0, dir_at + i * 4)?)?);
    }

    let refs = b.count(220, MAX_SKIN_REFS, "its number of materials")?;
    let families = b.count(224, MAX_SKIN_FAMILIES, "its number of skins")?;
    if refs.saturating_mul(families) > MAX_SKIN_ENTRIES {
        return Err(MdlError::TooLarge("its skins"));
    }
    let skin_at = b.offset(0, 228)?;
    let mut skins = Vec::with_capacity(families);
    for family in 0..families {
        let mut row = Vec::with_capacity(refs);
        for r in 0..refs {
            let texture = b.i16(skin_at + (family * refs + r) * 2)?;
            row.push(u16::try_from(texture).unwrap_or(0));
        }
        skins.push(row);
    }

    let part_count = b.count(232, MAX_BODY_PARTS, "its number of body parts")?;
    let part_at = b.offset(0, 236)?;
    let mut body_parts = Vec::with_capacity(part_count);
    let mut meshes_seen = 0usize;
    for p in 0..part_count {
        // mstudiobodyparts_t: name, model count, base, model offset.
        let at = part_at + p * 16;
        let model_count = b.count(at + 4, MAX_MODELS, "its number of models")?;
        let models_at = b.offset(at, at + 12)?;
        let mut models = Vec::with_capacity(model_count.min(16));
        for m in 0..model_count {
            let model = parse_model(&b, models_at + m * 148, &mut meshes_seen)?;
            models.push(model);
        }
        body_parts.push(models);
    }

    Ok(MdlInfo {
        version,
        checksum,
        name,
        flags,
        textures,
        texture_dirs,
        skins,
        body_parts,
    })
}

/// mstudiomodel_t (148 bytes) at `at`.
fn parse_model(b: &Bytes<'_>, at: usize, meshes_seen: &mut usize) -> Result<MdlModel, MdlError> {
    let mesh_count = b.count(at + 72, MAX_MESHES, "its number of meshes")?;
    *meshes_seen += mesh_count;
    if *meshes_seen > MAX_MESHES {
        return Err(MdlError::TooLarge("its number of meshes"));
    }
    let meshes_at = b.offset(at, at + 76)?;
    let vertex_count = b.count(at + 80, crate::MAX_VERTICES, "its number of vertices")?;
    let vertex_byte = b.count(at + 84, usize::MAX >> 1, "where its vertices are")?;
    let mut meshes = Vec::with_capacity(mesh_count);
    for i in 0..mesh_count {
        // mstudiomesh_t: 116 bytes.
        let mesh_at = meshes_at + i * 116;
        meshes.push(MdlMesh {
            material: b.count(mesh_at, MAX_SKIN_REFS, "a mesh's material")?,
            vertex_count: b.count(mesh_at + 8, crate::MAX_VERTICES, "a mesh's vertices")?,
            vertex_offset: b.count(mesh_at + 12, crate::MAX_VERTICES, "a mesh's vertices")?,
        });
    }
    Ok(MdlModel {
        vertex_start: vertex_byte / VERTEX_SIZE,
        vertex_count,
        meshes,
    })
}
