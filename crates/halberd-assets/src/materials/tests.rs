//! Tests for material names and pictures, with a made-up install.

use super::*;
use crate::{FoundBy, inspect_gmod_dir};
use halberd_vpk::VpkBuilder;
use std::path::Path;
use std::time::Duration;

/// A version 7.2 BGRA8888 texture, `size` × `size`, one colour, with all
/// its smaller sizes.
fn texture(size: u16, bgra: [u8; 4]) -> Vec<u8> {
    let mut sizes = Vec::new();
    let mut s = size;
    loop {
        sizes.push(s);
        if s == 1 {
            break;
        }
        s /= 2;
    }
    let mut h = vec![0u8; 80];
    h[..4].copy_from_slice(b"VTF\0");
    h[4..8].copy_from_slice(&7u32.to_le_bytes());
    h[8..12].copy_from_slice(&2u32.to_le_bytes());
    h[12..16].copy_from_slice(&80u32.to_le_bytes());
    h[16..18].copy_from_slice(&size.to_le_bytes());
    h[18..20].copy_from_slice(&size.to_le_bytes());
    h[24..26].copy_from_slice(&1u16.to_le_bytes());
    h[52..56].copy_from_slice(&12u32.to_le_bytes());
    h[56] = sizes.len() as u8;
    h[57..61].copy_from_slice(&u32::MAX.to_le_bytes());
    h[63..65].copy_from_slice(&1u16.to_le_bytes());
    for s in sizes.iter().rev() {
        h.extend_from_slice(&bgra.repeat(usize::from(*s) * usize::from(*s)));
    }
    h
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn content() -> (tempfile::TempDir, GameContent) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("GarrysMod");
    write(
        &root.join("garrysmod/gameinfo.txt"),
        b"GameInfo { FileSystem { SearchPaths {\n game |gameinfo_path|.\n game |all_source_engine_paths|sourceengine/hl2_textures.vpk\n} } }",
    );
    let mut vpk = VpkBuilder::new();
    vpk.add(
        "materials/brick/brickwall001a.vmt",
        b"LightmappedGeneric { $basetexture brick/brickwall001a }",
    )
    .add(
        "materials/brick/brickwall001a.vtf",
        &texture(256, [0, 0, 255, 255]),
    )
    .add(
        "materials/tools/toolsnodraw.vmt",
        b"LightmappedGeneric { $basetexture tools/toolsnodraw %compilenodraw 1 }",
    )
    .add(
        "materials/tools/toolsnodraw.vtf",
        &texture(64, [0, 255, 255, 255]),
    )
    .add(
        "materials/broken/notexture.vmt",
        b"LightmappedGeneric { $basetexture broken/missing }",
    );
    std::fs::create_dir_all(root.join("sourceengine")).unwrap();
    vpk.write_split(&root.join("sourceengine/hl2_textures_dir.vpk"))
        .unwrap();
    // A loose patch material on top of the archived wall.
    write(
        &root.join("garrysmod/materials/Mine/Patched.vmt"),
        b"patch { include materials/brick/brickwall001a.vmt replace { $surfaceprop metal } }",
    );
    let install = inspect_gmod_dir(&root, FoundBy::UserChoice).unwrap();
    (dir, GameContent::mount(&install))
}

#[test]
fn names_come_from_archives_and_folders_in_hammer_style() {
    let (_dir, content) = content();
    assert_eq!(
        material_names(&content),
        [
            "BRICK/BRICKWALL001A",
            "BROKEN/NOTEXTURE",
            "MINE/PATCHED",
            "TOOLS/TOOLSNODRAW"
        ]
    );
}

#[test]
fn a_material_gives_its_texture_at_the_size_asked_for() {
    let (_dir, content) = content();
    let wall = load_material(&content, "BRICK/BRICKWALL001A", 64);
    assert_eq!(wall.problem, None);
    assert_eq!(
        wall.texture_size,
        (256, 256),
        "the full size, for laying it out"
    );
    let image = wall.image.unwrap();
    assert_eq!((image.width, image.height), (64, 64));
    assert_eq!(&image.pixels[..4], &[255, 0, 0, 255]);
    assert!(!wall.tool);
    let nodraw = load_material(&content, "tools/toolsnodraw", 128);
    assert!(nodraw.tool);
    assert_eq!(nodraw.image.unwrap().width, 64);
}

#[test]
fn patches_follow_their_include_and_problems_say_why() {
    let (_dir, content) = content();
    let patched = load_material(&content, "MINE/PATCHED", 32);
    assert_eq!(patched.problem, None);
    assert_eq!(patched.texture_size, (256, 256));
    let broken = load_material(&content, "BROKEN/NOTEXTURE", 32);
    assert!(broken.image.is_none());
    assert!(broken.problem.unwrap().contains("broken/missing.vtf"));
    let missing = load_material(&content, "NOT/THERE", 32);
    assert!(
        missing
            .problem
            .unwrap()
            .contains("not in the game's content")
    );
}

#[test]
fn the_loader_reads_in_the_background_once_per_request() {
    let (_dir, content) = content();
    let (woke, waker) = mpsc::channel();
    let mut loader = MaterialLoader::new(Arc::new(content), move || {
        woke.send(()).ok();
    });
    loader.request("brick/brickwall001a", 32);
    loader.request("BRICK/BRICKWALL001A", 32);
    assert!(loader.is_pending("Brick/BrickWall001A", 32));
    waker.recv_timeout(Duration::from_secs(10)).unwrap();
    let mut ready = loader.poll();
    while ready.is_empty() {
        ready = loader.poll();
    }
    assert_eq!(ready.len(), 1, "asked twice, read once");
    assert_eq!(ready[0].name, "BRICK/BRICKWALL001A");
    assert!(!loader.is_pending("BRICK/BRICKWALL001A", 32));
}
