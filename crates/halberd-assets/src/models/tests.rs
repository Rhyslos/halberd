//! Tests for model names and loading, with a made-up install.

use super::*;
use crate::{FoundBy, inspect_gmod_dir};
use halberd_mdl::example::{ExampleModel, box_mesh, model_files};
use halberd_vpk::VpkBuilder;
use std::path::Path;
use std::time::Duration;

fn write(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// An install with an archived crate (its material in a second folder),
/// a loose barrel with only an old-style `.vtx`, and a model missing its
/// vertices.
fn content() -> (tempfile::TempDir, GameContent) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("GarrysMod");
    write(
        &root.join("garrysmod/gameinfo.txt"),
        b"GameInfo { FileSystem { SearchPaths {\n game |gameinfo_path|.\n game |all_source_engine_paths|sourceengine/hl2_misc.vpk\n} } }",
    );
    let crate_files = model_files(&ExampleModel {
        name: "props_junk/wood_crate001a.mdl",
        static_prop: true,
        materials: &["wood_crate001a", "Missing"],
        folders: &["models\\props_junk\\", "models/props_debris/"],
        meshes: &[box_mesh([-20.0; 3], [20.0; 3], 0)],
        ..ExampleModel::default()
    });
    let mut vpk = VpkBuilder::new();
    vpk.add("models/props_junk/wood_crate001a.mdl", &crate_files.mdl)
        .add("models/props_junk/wood_crate001a.vvd", &crate_files.vvd)
        .add(
            "models/props_junk/wood_crate001a.dx90.vtx",
            &crate_files.vtx,
        )
        .add(
            "materials/models/props_debris/wood_crate001a.vmt",
            b"VertexLitGeneric { $basetexture models/props_debris/wood_crate001a }",
        )
        .add("models/broken/novertices.mdl", &crate_files.mdl);
    std::fs::create_dir_all(root.join("sourceengine")).unwrap();
    vpk.write_split(&root.join("sourceengine/hl2_misc_dir.vpk"))
        .unwrap();
    let barrel = model_files(&ExampleModel {
        name: "mine/barrel.mdl",
        materials: &["barrel"],
        folders: &["models/mine/"],
        meshes: &[box_mesh([0.0; 3], [16.0, 16.0, 40.0], 0)],
        ..ExampleModel::default()
    });
    let mine = root.join("garrysmod/models/Mine");
    write(&mine.join("Barrel.mdl"), &barrel.mdl);
    write(&mine.join("Barrel.vvd"), &barrel.vvd);
    write(&mine.join("Barrel.vtx"), &barrel.vtx);
    let install = inspect_gmod_dir(&root, FoundBy::UserChoice).unwrap();
    (dir, GameContent::mount(&install))
}

#[test]
fn names_come_from_archives_and_folders_as_maps_name_them() {
    let (_dir, content) = content();
    assert_eq!(
        model_names(&content),
        [
            "models/broken/novertices.mdl",
            "models/mine/barrel.mdl",
            "models/props_junk/wood_crate001a.mdl",
        ]
    );
}

#[test]
fn a_model_loads_with_its_materials_found_in_its_folders() {
    let (_dir, content) = content();
    let loaded = load_model(&content, "models/props_junk/wood_crate001a.mdl");
    let model = loaded.model.unwrap();
    assert!(model.static_prop);
    assert_eq!(model.triangle_count(), 12);
    // Found in the second folder; the missing one assumes the first.
    assert_eq!(
        loaded.materials,
        [
            "MODELS/PROPS_DEBRIS/WOOD_CRATE001A",
            "MODELS/PROPS_JUNK/MISSING"
        ]
    );
    let barrel = load_model(&content, "MODELS\\MINE\\BARREL.MDL");
    assert_eq!(barrel.model.unwrap().bounds.1, [16.0, 16.0, 40.0]);
}

#[test]
fn problems_say_why_in_plain_words() {
    let (_dir, content) = content();
    let broken = load_model(&content, "models/broken/novertices.mdl");
    let why = broken.model.unwrap_err();
    assert!(why.contains("novertices.vvd"), "{why}");
    let wrong = load_model(&content, "models/props_junk/wood_crate001a.vmt");
    assert!(wrong.model.unwrap_err().contains(".mdl"));
}

#[test]
fn the_loader_reads_in_the_background_once_per_request() {
    let (_dir, content) = content();
    let (woke, waker) = mpsc::channel();
    let mut loader = ModelLoader::new(Arc::new(content), move || {
        woke.send(()).ok();
    });
    loader.request("models/mine/barrel.mdl");
    loader.request("MODELS/MINE/BARREL.MDL");
    assert!(loader.is_pending("models/mine/barrel.mdl"));
    waker.recv_timeout(Duration::from_secs(10)).unwrap();
    let ready = loader.poll();
    assert_eq!(ready.len(), 1);
    assert!(ready[0].model.is_ok());
    assert!(!loader.is_pending("models/mine/barrel.mdl"));
    assert!(
        waker.recv_timeout(Duration::from_millis(200)).is_err(),
        "asked once"
    );
}
