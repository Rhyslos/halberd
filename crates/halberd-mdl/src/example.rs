//! Small, valid model files made in code, for tests and demonstrations
//! (Halberd's own tests, and trying the prop browser without the game).

use crate::Vertex;

/// The checksum example files share.
pub const EXAMPLE_CHECKSUM: i32 = 0x1234_5678;

/// A model's three files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFiles {
    /// The `.mdl` file.
    pub mdl: Vec<u8>,
    /// The `.vvd` file.
    pub vvd: Vec<u8>,
    /// The `.vtx` (`.dx90.vtx`) file.
    pub vtx: Vec<u8>,
}

/// One mesh of an example model.
#[derive(Debug, Clone, PartialEq)]
pub struct ExampleMesh {
    /// Its material reference (through the skin table).
    pub material: i32,
    /// Its corners.
    pub vertices: Vec<Vertex>,
    /// Its triangles, as numbers into `vertices`.
    pub triangles: Vec<u16>,
}

/// What an example model is made of.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExampleModel<'a> {
    /// The name it is compiled with (`props/crate.mdl`).
    pub name: &'a str,
    /// Made for `prop_static`.
    pub static_prop: bool,
    /// Material names.
    pub materials: &'a [&'a str],
    /// Material folders.
    pub folders: &'a [&'a str],
    /// The skin table: per skin, the material each reference uses.
    pub skins: &'a [&'a [i16]],
    /// Its meshes (one body part, one model).
    pub meshes: &'a [ExampleMesh],
}

fn put(out: &mut [u8], at: usize, bytes: [u8; 4]) {
    if let Some(slot) = out.get_mut(at..at + 4) {
        slot.copy_from_slice(&bytes);
    }
}

fn put_i32(out: &mut [u8], at: usize, v: usize) {
    put(out, at, (v as i32).to_le_bytes());
}

/// Writes an example model's three files.
pub fn model_files(model: &ExampleModel<'_>) -> ModelFiles {
    ModelFiles {
        mdl: mdl_file(model),
        vvd: vvd_file(model.meshes),
        vtx: vtx_file(model.meshes),
    }
}

fn mdl_file(model: &ExampleModel<'_>) -> Vec<u8> {
    let mut mdl = vec![0u8; 408];
    mdl[0..4].copy_from_slice(b"IDST");
    put_i32(&mut mdl, 4, 48);
    put(&mut mdl, 8, EXAMPLE_CHECKSUM.to_le_bytes());
    for (i, b) in model.name.bytes().take(63).enumerate() {
        mdl[12 + i] = b;
    }
    put_i32(&mut mdl, 152, if model.static_prop { 0x10 } else { 0 });
    let texture_at = mdl.len();
    mdl.resize(texture_at + model.materials.len() * 64, 0);
    let dir_at = mdl.len();
    mdl.resize(dir_at + model.folders.len() * 4, 0);
    let refs = model.skins.first().map_or(0, |r| r.len());
    let skin_at = mdl.len();
    for row in model.skins {
        for &t in *row {
            mdl.extend_from_slice(&t.to_le_bytes());
        }
    }
    let part_at = mdl.len();
    mdl.resize(part_at + 16, 0);
    let model_at = mdl.len();
    mdl.resize(model_at + 148, 0);
    let mesh_at = mdl.len();
    mdl.resize(mesh_at + model.meshes.len() * 116, 0);
    for (i, name) in model.materials.iter().enumerate() {
        let here = mdl.len();
        mdl.extend_from_slice(name.as_bytes());
        mdl.push(0);
        let t = texture_at + i * 64;
        put_i32(&mut mdl, t, here - t);
    }
    for (i, dir) in model.folders.iter().enumerate() {
        let here = mdl.len();
        mdl.extend_from_slice(dir.as_bytes());
        mdl.push(0);
        put_i32(&mut mdl, dir_at + i * 4, here);
    }
    for (at, v) in [
        (204, model.materials.len()),
        (208, texture_at),
        (212, model.folders.len()),
        (216, dir_at),
        (220, refs),
        (224, model.skins.len()),
        (228, skin_at),
        (232, 1),
        (236, part_at),
        (part_at + 4, 1),
        (part_at + 12, model_at - part_at),
        (model_at + 72, model.meshes.len()),
        (model_at + 76, mesh_at - model_at),
        (
            model_at + 80,
            model.meshes.iter().map(|m| m.vertices.len()).sum(),
        ),
    ] {
        put_i32(&mut mdl, at, v);
    }
    let mut offset = 0;
    for (i, mesh) in model.meshes.iter().enumerate() {
        let m = mesh_at + i * 116;
        put(&mut mdl, m, mesh.material.to_le_bytes());
        put_i32(&mut mdl, m + 8, mesh.vertices.len());
        put_i32(&mut mdl, m + 12, offset);
        offset += mesh.vertices.len();
    }
    let len = mdl.len();
    put_i32(&mut mdl, 76, len);
    mdl
}

fn vvd_file(meshes: &[ExampleMesh]) -> Vec<u8> {
    let total: usize = meshes.iter().map(|m| m.vertices.len()).sum();
    let mut vvd = vec![0u8; 64];
    vvd[0..4].copy_from_slice(b"IDSV");
    put_i32(&mut vvd, 4, 4);
    put(&mut vvd, 8, EXAMPLE_CHECKSUM.to_le_bytes());
    put_i32(&mut vvd, 12, 1);
    put_i32(&mut vvd, 16, total);
    put_i32(&mut vvd, 52, 64);
    put_i32(&mut vvd, 56, 64);
    for v in meshes.iter().flat_map(|m| &m.vertices) {
        let at = vvd.len();
        vvd.resize(at + 48, 0);
        // One bone, full weight.
        put(&mut vvd, at, 1.0f32.to_le_bytes());
        vvd[at + 15] = 1;
        let values = [v.position, v.normal].concat();
        for (k, value) in values.iter().chain(&v.uv).enumerate() {
            put(&mut vvd, at + 16 + k * 4, value.to_le_bytes());
        }
    }
    vvd
}

fn vtx_file(meshes: &[ExampleMesh]) -> Vec<u8> {
    let mut vtx = vec![0u8; 36];
    put_i32(&mut vtx, 0, 7);
    put(&mut vtx, 12, EXAMPLE_CHECKSUM.to_le_bytes());
    put_i32(&mut vtx, 16, 1);
    put_i32(&mut vtx, 28, 1);
    put_i32(&mut vtx, 32, 36);
    let part = vtx.len();
    let model = part + 8;
    let lod = model + 8;
    let mesh_headers = lod + 12;
    vtx.resize(mesh_headers + meshes.len() * 9, 0);
    put_i32(&mut vtx, part, 1);
    put_i32(&mut vtx, part + 4, model - part);
    put_i32(&mut vtx, model, 1);
    put_i32(&mut vtx, model + 4, lod - model);
    put_i32(&mut vtx, lod, meshes.len());
    put_i32(&mut vtx, lod + 4, mesh_headers - lod);
    for (i, mesh) in meshes.iter().enumerate() {
        let header = mesh_headers + i * 9;
        let group = vtx.len();
        vtx.resize(group + 25, 0);
        put_i32(&mut vtx, header, 1);
        put_i32(&mut vtx, header + 4, group - header);
        let verts = vtx.len();
        for v in 0..mesh.vertices.len() {
            vtx.extend_from_slice(&[0, 0, 0, 1]);
            vtx.extend_from_slice(&(v as u16).to_le_bytes());
            vtx.extend_from_slice(&[0, 0, 0]);
        }
        let indices = vtx.len();
        for &t in &mesh.triangles {
            vtx.extend_from_slice(&t.to_le_bytes());
        }
        put_i32(&mut vtx, group, mesh.vertices.len());
        put_i32(&mut vtx, group + 4, verts - group);
        put_i32(&mut vtx, group + 8, mesh.triangles.len());
        put_i32(&mut vtx, group + 12, indices - group);
    }
    vtx
}

/// The six sides of a box from `min` to `max`, with each side's texture
/// stretched across it once, as one mesh.
pub fn box_mesh(min: [f32; 3], max: [f32; 3], material: i32) -> ExampleMesh {
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    // For each side: the axis it faces along, and which way.
    for axis in 0..3 {
        for positive in [false, true] {
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            let mut normal = [0.0; 3];
            normal[axis] = if positive { 1.0 } else { -1.0 };
            let first = vertices.len() as u16;
            for (a, b) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                let mut p = [0.0; 3];
                p[axis] = if positive { max[axis] } else { min[axis] };
                p[u] = min[u] + (max[u] - min[u]) * a;
                p[v] = min[v] + (max[v] - min[v]) * b;
                vertices.push(Vertex {
                    position: p,
                    normal,
                    uv: [a, 1.0 - b],
                });
            }
            let corners: [u16; 6] = if positive {
                [0, 1, 2, 0, 2, 3]
            } else {
                [0, 2, 1, 0, 3, 2]
            };
            triangles.extend(corners.iter().map(|c| first + c));
        }
    }
    ExampleMesh {
        material,
        vertices,
        triangles,
    }
}
