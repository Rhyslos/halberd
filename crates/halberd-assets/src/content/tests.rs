//! Tests for mounting game content, with a small made-up install.

use super::*;
use crate::{FoundBy, inspect_gmod_dir};
use halberd_vpk::VpkBuilder;

/// Search paths shaped like Garry's Mod's.
const GAMEINFO: &str = r#""GameInfo"
{
    game "Garry's Mod"
    "FileSystem"
    {
        "SteamAppId" "4000"
        "SearchPaths"
        {
            "game+mod" "garrysmod/addons/*"
            "game+mod+mod_write+default_write_path" "|gameinfo_path|."
            "gamebin" "|gameinfo_path|bin"
            "game" "|all_source_engine_paths|sourceengine/hl2_textures.vpk"
            "game" "|all_source_engine_paths|sourceengine/hl2_misc.vpk"
            "platform" "|all_source_engine_paths|platform/platform_misc.vpk"
            "game" "garrysmod/download"
            "game" "../../outside"
        }
    }
}
"#;

fn write(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// A made-up install: HL2 textures in a split archive, misc in a single
/// file, a loose material, and one addon.
fn install() -> (tempfile::TempDir, GmodInstall) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("GarrysMod");
    write(&root.join("garrysmod/gameinfo.txt"), GAMEINFO.as_bytes());
    let engine = root.join("sourceengine");
    std::fs::create_dir_all(&engine).unwrap();
    let mut textures = VpkBuilder::new();
    textures
        .add("materials/brick/brickwall001a.vmt", b"from hl2")
        .add("materials/brick/brickwall001a.vtf", b"vtf bytes")
        .add("materials/dev/dev_measuregeneric01b.vmt", b"dev from hl2");
    textures
        .write_split(&engine.join("hl2_textures_dir.vpk"))
        .unwrap();
    let mut misc = VpkBuilder::new();
    misc.add("models/props_c17/oildrum001.mdl", b"model");
    misc.write_single(&engine.join("hl2_misc.vpk")).unwrap();
    // Loose GMod files override the archives (they come first).
    write(
        &root.join("garrysmod/materials/Dev/Dev_MeasureGeneric01B.vmt"),
        b"loose gmod",
    );
    write(
        &root.join("garrysmod/addons/my_addon/materials/addon/thing.vmt"),
        b"addon",
    );
    let install = inspect_gmod_dir(&root, FoundBy::UserChoice).unwrap();
    (dir, install)
}

#[test]
fn gameinfo_search_paths_mount_in_order() {
    let (_dir, install) = install();
    let content = GameContent::mount(&install);
    assert!(content.problems().is_empty(), "{:?}", content.problems());
    assert_eq!(content.archive_count(), 2);
    assert_eq!(content.folder_count(), 2, "the addon and garrysmod");
    assert_eq!(content.archived_file_count(), 4);
    assert_eq!(
        content.summary(),
        "Game content mounted: 4 files in 2 archives, and 2 folders."
    );
    assert_eq!(
        content.read("materials/brick/brickwall001a.vmt").unwrap(),
        b"from hl2"
    );
    assert_eq!(
        content.read("models/props_c17/oildrum001.mdl").unwrap(),
        b"model"
    );
    assert_eq!(content.read("materials/addon/thing.vmt").unwrap(), b"addon");
}

#[test]
fn the_first_place_with_a_file_wins_and_case_does_not_matter() {
    let (_dir, install) = install();
    let content = GameContent::mount(&install);
    // garrysmod's loose file comes before the HL2 archive, whatever its case.
    assert_eq!(
        content
            .read("MATERIALS\\dev\\dev_measuregeneric01b.vmt")
            .unwrap(),
        b"loose gmod"
    );
    assert!(content.contains("materials/brick/BRICKWALL001A.VTF"));
    assert!(!content.contains("materials/nothing.vmt"));
    assert!(matches!(
        content.read("materials/nothing.vmt"),
        Err(ContentError::NotFound(_))
    ));
}

#[test]
fn paths_that_leave_the_game_are_refused() {
    let (dir, install) = install();
    write(&dir.path().join("outside/secret.txt"), b"secret");
    let content = GameContent::mount(&install);
    for bad in [
        "../secret.txt",
        "../../outside/secret.txt",
        "/etc/passwd",
        "",
        "materials/../../x",
    ] {
        assert!(
            matches!(content.read(bad), Err(ContentError::BadPath(_))),
            "{bad}"
        );
        assert!(!content.contains(bad));
    }
    // "../../outside" in gameinfo was not mounted either.
    assert!(!content.contains("secret.txt"));
}

#[test]
fn archived_files_are_listed_by_folder_and_extension() {
    let (_dir, install) = install();
    let content = GameContent::mount(&install);
    assert_eq!(
        content.archived_files("materials/", ".vmt"),
        [
            "materials/brick/brickwall001a.vmt",
            "materials/dev/dev_measuregeneric01b.vmt"
        ]
    );
    assert_eq!(content.archived_files("MODELS", "MDL").len(), 1);
}

#[test]
fn damaged_archives_and_missing_gameinfo_become_notes() {
    let (_dir, install) = install();
    std::fs::write(install.root.join("sourceengine/hl2_misc.vpk"), b"garbage").unwrap();
    let content = GameContent::mount(&install);
    assert_eq!(content.archive_count(), 1);
    assert_eq!(content.problems().len(), 1);
    assert!(
        content.problems()[0].contains("hl2_misc.vpk"),
        "{:?}",
        content.problems()
    );
    std::fs::remove_file(install.game_dir.join("gameinfo.txt")).unwrap();
    let content = GameContent::mount(&install);
    assert_eq!(content.archive_count(), 0);
    assert!(content.problems()[0].contains("gameinfo.txt"));
}

#[test]
fn search_paths_keep_only_game_ones_and_numbers_read_well() {
    let entries = halberd_kv::parse(GAMEINFO).unwrap();
    let paths = search_paths(&entries);
    assert_eq!(paths.len(), 6, "{paths:?}");
    assert_eq!(paths[0], "garrysmod/addons/*");
    assert!(
        !paths
            .iter()
            .any(|p| p.contains("bin") || p.contains("platform"))
    );
    assert_eq!(group_digits(0), "0");
    assert_eq!(group_digits(41_203), "41,203");
    assert_eq!(group_digits(1_234_567), "1,234,567");
    assert!(search_paths(&halberd_kv::parse("\"Other\" {}").unwrap()).is_empty());
}
