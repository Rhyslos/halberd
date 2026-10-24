//! Tests for reading models, with small model files built here.

use halberd_mdl::example::{ExampleMesh, ExampleModel, ModelFiles, box_mesh, model_files};
use halberd_mdl::{MdlError, Model, Vertex};

fn put_i32(out: &mut [u8], at: usize, v: i32) {
    out[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_f32(out: &mut [u8], at: usize, v: f32) {
    out[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// Vertices numbered from `first`: vertex v at (v, 2v, 3v), facing up,
/// with texture coordinates (v/10, 1 - v/10).
fn numbered(first: usize, count: usize) -> Vec<Vertex> {
    (first..first + count)
        .map(|v| {
            let f = v as f32;
            Vertex {
                position: [f, 2.0 * f, 3.0 * f],
                normal: [0.0, 0.0, 1.0],
                uv: [f / 10.0, 1.0 - f / 10.0],
            }
        })
        .collect()
}

fn build(
    materials: &[&str],
    folders: &[&str],
    skins: &[&[i16]],
    meshes: &[(i32, usize, Vec<u16>)],
) -> ModelFiles {
    let mut first = 0;
    let meshes: Vec<ExampleMesh> = meshes
        .iter()
        .map(|(material, count, triangles)| {
            let mesh = ExampleMesh {
                material: *material,
                vertices: numbered(first, *count),
                triangles: triangles.clone(),
            };
            first += count;
            mesh
        })
        .collect();
    model_files(&ExampleModel {
        name: "test\\box.mdl",
        static_prop: true,
        materials,
        folders,
        skins,
        meshes: &meshes,
    })
}

fn two_meshes() -> ModelFiles {
    build(
        &["oildrum001", "oildrum001_lid"],
        &["models\\props_c17\\", "models/shared/"],
        &[&[1, 0], &[0, 0]],
        &[(0, 4, vec![0, 1, 2, 0, 2, 3]), (1, 3, vec![2, 1, 0])],
    )
}

#[test]
fn a_model_reads_its_meshes_materials_and_bounds() {
    let b = two_meshes();
    let model = Model::parse(&b.mdl, &b.vvd, &b.vtx).unwrap();
    assert_eq!(model.name, "test/box.mdl");
    assert!(model.static_prop);
    assert_eq!(model.materials, ["oildrum001", "oildrum001_lid"]);
    assert_eq!(model.material_folders, ["models\\props_c17\\", "models/shared/"]);
    assert_eq!(model.meshes.len(), 2);
    // The first skin maps reference 0 to texture 1, reference 1 to 0.
    assert_eq!(model.meshes[0].material, 1);
    assert_eq!(model.meshes[1].material, 0);
    assert_eq!(model.meshes[0].indices, [0, 1, 2, 0, 2, 3]);
    assert_eq!(model.meshes[1].vertices[0].position, [4.0, 8.0, 12.0]);
    assert_eq!(model.meshes[0].vertices[1].uv, [0.1, 0.9]);
    assert_eq!(model.bounds, ([0.0; 3], [6.0, 12.0, 18.0]));
    assert_eq!(model.triangle_count(), 3);
}

#[test]
fn files_that_dont_belong_together_are_refused() {
    let mut b = two_meshes();
    put_i32(&mut b.vvd, 8, 99);
    assert_eq!(Model::parse(&b.mdl, &b.vvd, &b.vtx), Err(MdlError::Mismatched));
    let mut b = two_meshes();
    put_i32(&mut b.vtx, 12, 99);
    assert_eq!(Model::parse(&b.mdl, &b.vvd, &b.vtx), Err(MdlError::Mismatched));
}

#[test]
fn bad_models_are_refused_with_a_reason() {
    let b = two_meshes();
    assert_eq!(Model::parse(b"nope", &b.vvd, &b.vtx), Err(MdlError::WrongKind(".mdl")));
    let mut old = b.mdl.clone();
    put_i32(&mut old, 4, 37);
    assert_eq!(
        Model::parse(&old, &b.vvd, &b.vtx),
        Err(MdlError::UnsupportedVersion(".mdl", 37))
    );
    // A triangle naming a vertex the mesh doesn't have.
    let bad = build(&["a"], &[], &[&[0]], &[(0, 3, vec![0, 1, 7])]);
    assert!(matches!(
        Model::parse(&bad.mdl, &bad.vvd, &bad.vtx),
        Err(MdlError::Damaged(".vtx", _))
    ));
    // Huge counts are refused before anything is allocated.
    let mut huge = b.mdl.clone();
    put_i32(&mut huge, 204, i32::MAX);
    assert!(matches!(
        Model::parse(&huge, &b.vvd, &b.vtx),
        Err(MdlError::TooLarge(_))
    ));
    let msg = MdlError::Mismatched.to_string();
    assert!(msg.contains("don't belong together"), "{msg}");
}

#[test]
fn fixups_choose_the_full_detail_vertices() {
    let mut b = build(&["a"], &[], &[], &[(0, 3, vec![0, 1, 2])]);
    // Store 5 vertices; fixups take 1..3 (LOD 0) and 4 (LOD 1, also part
    // of full detail), skipping vertex 0 and 3 (LOD -1 would be skipped).
    let mut vvd = b.vvd[..64].to_vec();
    put_i32(&mut vvd, 48, 3);
    let fixups = vvd.len();
    for (lod, first, count) in [(0, 1, 2), (-1, 0, 1), (1, 4, 1)] {
        let at = vvd.len();
        vvd.resize(at + 12, 0);
        put_i32(&mut vvd, at, lod);
        put_i32(&mut vvd, at + 4, first);
        put_i32(&mut vvd, at + 8, count);
    }
    put_i32(&mut vvd, 52, fixups as i32);
    let data = vvd.len();
    put_i32(&mut vvd, 56, data as i32);
    for v in 0..5 {
        let at = vvd.len();
        vvd.resize(at + 48, 0);
        put_f32(&mut vvd, at + 16, v as f32);
    }
    b.vvd = vvd;
    let model = Model::parse(&b.mdl, &b.vvd, &b.vtx).unwrap();
    let xs: Vec<f32> = model.meshes[0].vertices.iter().map(|v| v.position[0]).collect();
    assert_eq!(xs, [1.0, 2.0, 4.0]);
}

#[test]
fn fuzz_damaged_models_never_crash() {
    let good = two_meshes();
    let mut seed: u64 = 0x3D1_0000_0000_0007;
    let mut next = move |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n.max(1) as u64) as usize
    };
    for round in 0..20_000 {
        let mut files = [good.mdl.clone(), good.vvd.clone(), good.vtx.clone()];
        let which = next(3);
        let bytes = &mut files[which];
        if round % 9 == 0 {
            bytes.truncate(next(bytes.len()));
        }
        for _ in 0..1 + next(4) {
            if !bytes.is_empty() {
                let at = next(bytes.len());
                bytes[at] = match next(4) {
                    0 => 0xFF,
                    1 => 0x7F,
                    _ => next(256) as u8,
                };
            }
        }
        if let Ok(model) = Model::parse(&files[0], &files[1], &files[2]) {
            for mesh in &model.meshes {
                assert!(mesh.indices.iter().all(|&i| (i as usize) < mesh.vertices.len()));
                assert!(mesh.material < model.materials.len().max(1));
            }
        }
    }
}

#[test]
fn an_example_box_has_six_sides_facing_out() {
    let mesh = box_mesh([-8.0, -8.0, 0.0], [8.0, 8.0, 32.0], 0);
    let files = model_files(&ExampleModel {
        name: "props/crate.mdl",
        materials: &["crate"],
        folders: &["models/props/"],
        meshes: &[mesh],
        ..ExampleModel::default()
    });
    let model = Model::parse(&files.mdl, &files.vvd, &files.vtx).unwrap();
    assert!(!model.static_prop);
    assert_eq!(model.triangle_count(), 12);
    assert_eq!(model.bounds, ([-8.0, -8.0, 0.0], [8.0, 8.0, 32.0]));
    let mesh = &model.meshes[0];
    // Every triangle winds so that its normal (by the right-hand rule)
    // points the way its corners face.
    for t in mesh.indices.chunks(3) {
        let [a, b, c] = [0, 1, 2].map(|k| mesh.vertices[t[k] as usize]);
        let e1: Vec<f32> = (0..3).map(|k| b.position[k] - a.position[k]).collect();
        let e2: Vec<f32> = (0..3).map(|k| c.position[k] - a.position[k]).collect();
        let n = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        let dot: f32 = (0..3).map(|k| n[k] * a.normal[k]).sum();
        assert!(dot > 0.0, "a triangle faces inwards");
    }
}
