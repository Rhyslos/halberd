//! Reading archives written by `VpkBuilder`, and damaged ones.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use halberd_vpk::{Vpk, VpkBuilder, VpkError};
use std::path::{Path, PathBuf};

fn sample() -> VpkBuilder {
    let mut b = VpkBuilder::new();
    b.add(
        "materials/brick/brickwall001a.vmt",
        b"\"LightmappedGeneric\" {}",
    )
    .add("materials/brick/brickwall001a.vtf", &vec![7u8; 5000])
    .add("materials/dev/dev_measuregeneric01b.vmt", b"dev")
    .add("readme", b"no folder, no extension")
    .add("models/props/crate.mdl", &[]);
    b
}

fn all_paths() -> [&'static str; 5] {
    [
        "materials/brick/brickwall001a.vmt",
        "materials/brick/brickwall001a.vtf",
        "materials/dev/dev_measuregeneric01b.vmt",
        "readme",
        "models/props/crate.mdl",
    ]
}

fn check_contents(vpk: &Vpk) {
    assert_eq!(vpk.len(), 5);
    assert_eq!(vpk.read("readme").unwrap(), b"no folder, no extension");
    assert_eq!(
        vpk.read("materials/brick/brickwall001a.vtf").unwrap(),
        vec![7u8; 5000]
    );
    assert_eq!(vpk.read("models/props/crate.mdl").unwrap(), b"");
    assert_eq!(
        vpk.entry("materials/brick/brickwall001a.vtf")
            .unwrap()
            .size(),
        5000
    );
    for path in all_paths() {
        assert!(vpk.contains(path), "{path}");
    }
}

#[test]
fn single_file_archives_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pak01.vpk");
    sample().write_single(&path).unwrap();
    check_contents(&Vpk::open(&path).unwrap());
    // Version 1 and preloaded bytes too.
    sample()
        .version_1()
        .preload(16)
        .write_single(&path)
        .unwrap();
    check_contents(&Vpk::open(&path).unwrap());
}

#[test]
fn split_archives_read_back_with_and_without_preload() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hl2_textures_dir.vpk");
    for preload in [0, 3, 64] {
        sample().preload(preload).write_split(&path).unwrap();
        check_contents(&Vpk::open(&path).unwrap());
    }
}

#[test]
fn paths_are_found_in_any_case_and_with_either_slash() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pak01.vpk");
    sample().write_single(&path).unwrap();
    let vpk = Vpk::open(&path).unwrap();
    assert!(vpk.contains("MATERIALS\\Brick\\BrickWall001A.VMT"));
    assert!(vpk.contains("/materials/dev/dev_measuregeneric01b.vmt"));
    assert!(!vpk.contains("materials/brick"));
    match vpk.read("materials/nothing.vtf") {
        Err(VpkError::NotFound(p)) => assert_eq!(p, "materials/nothing.vtf"),
        other => panic!("{other:?}"),
    }
}

fn split_sample(dir: &Path) -> PathBuf {
    let path = dir.join("pak_dir.vpk");
    sample().write_split(&path).unwrap();
    path
}

#[test]
fn damaged_or_missing_archive_bytes_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let path = split_sample(dir.path());
    let archive = dir.path().join("pak_000.vpk");
    // A changed byte: the checksum catches it.
    let mut bytes = std::fs::read(&archive).unwrap();
    bytes[100] ^= 0xFF;
    std::fs::write(&archive, &bytes).unwrap();
    let vpk = Vpk::open(&path).unwrap();
    assert!(matches!(
        vpk.read("materials/brick/brickwall001a.vtf"),
        Err(VpkError::ChecksumMismatch(_))
    ));
    // A cut-short archive.
    std::fs::write(&archive, &bytes[..1000]).unwrap();
    assert!(matches!(
        vpk.read("materials/brick/brickwall001a.vtf"),
        Err(VpkError::Damaged(_))
    ));
    // No archive at all.
    std::fs::remove_file(&archive).unwrap();
    assert!(matches!(vpk.read("readme"), Err(VpkError::Io(_))));
    // The list itself still opens.
    assert_eq!(Vpk::open(&path).unwrap().len(), 5);
}

#[test]
fn not_an_archive_and_bad_headers_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.vpk");
    for (bytes, expect) in [
        (b"hello world, not a vpk".to_vec(), "not a VPK"),
        (
            vec![0x34, 0x12, 0xAA, 0x55, 9, 0, 0, 0, 0, 0, 0, 0],
            "version 9",
        ),
        (vec![0x34, 0x12, 0xAA, 0x55, 1, 0, 0], "cut short"),
        (
            vec![0x34, 0x12, 0xAA, 0x55, 1, 0, 0, 0, 0xFF, 0xFF, 0, 0],
            "past the end",
        ),
        (
            vec![0x34, 0x12, 0xAA, 0x55, 1, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF],
            "too large",
        ),
    ] {
        std::fs::write(&path, &bytes).unwrap();
        let err = Vpk::open(&path).unwrap_err().to_string();
        assert!(err.contains(expect), "{expect}: {err}");
    }
    assert!(matches!(
        Vpk::open(&dir.path().join("missing.vpk")),
        Err(VpkError::Io(_))
    ));
}

#[test]
fn a_single_file_archive_pointing_into_numbered_archives_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let split = split_sample(dir.path());
    // The same list under a name that is not _dir.vpk.
    let renamed = dir.path().join("renamed.vpk");
    std::fs::copy(&split, &renamed).unwrap();
    let vpk = Vpk::open(&renamed).unwrap();
    let err = vpk.read("readme").unwrap_err().to_string();
    assert!(err.contains("_dir.vpk"), "{err}");
}

#[test]
fn fuzz_damaged_directories_never_crash() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fuzz_dir.vpk");
    sample().preload(8).write_split(&path).unwrap();
    let good = std::fs::read(&path).unwrap();
    let mut seed: u64 = 0x5EED_0FAA_1234_0001;
    let mut next = move |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n.max(1) as u64) as usize
    };
    for round in 0..3000 {
        let mut bytes = good.clone();
        if round % 5 == 0 {
            bytes.truncate(next(bytes.len()));
        }
        for _ in 0..1 + next(6) {
            if bytes.is_empty() {
                break;
            }
            let at = next(bytes.len());
            bytes[at] = next(256) as u8;
        }
        std::fs::write(&path, &bytes).unwrap();
        if let Ok(vpk) = Vpk::open(&path) {
            for entry in vpk.entries().iter().take(10) {
                let _ = vpk.read_entry(entry);
            }
        }
    }
}

#[test]
fn a_large_file_list_opens_quickly() {
    // Half-Life 2's texture archive lists tens of thousands of files.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big_dir.vpk");
    let mut builder = VpkBuilder::new();
    for i in 0..50_000 {
        builder.add(&format!("materials/folder{}/texture{i}.vtf", i % 300), b"x");
    }
    builder.write_split(&path).unwrap();
    let start = std::time::Instant::now();
    let vpk = Vpk::open(&path).unwrap();
    let took = start.elapsed();
    assert_eq!(vpk.len(), 50_000);
    assert!(vpk.contains("materials/folder7/texture307.vtf"));
    // Generous, for slow test machines and unoptimised builds.
    assert!(took.as_secs_f32() < 5.0, "opening took {took:?}");
}

/// A single-file version 1 archive written by hand, for names
/// `VpkBuilder` would never write: (folder, name, extension, contents).
/// (folder, name, extension, contents), as raw bytes.
type RawFile<'a> = (&'a [u8], &'a [u8], &'a [u8], &'a [u8]);

fn raw_archive(path: &Path, files: &[RawFile<'_>]) {
    let mut tree = Vec::new();
    let mut data = Vec::new();
    for (folder, name, extension, contents) in files {
        for part in [*extension, *folder, *name] {
            tree.extend_from_slice(part);
            tree.push(0);
        }
        tree.extend_from_slice(&halberd_vpk::crc32(contents).to_le_bytes());
        tree.extend_from_slice(&0u16.to_le_bytes());
        tree.extend_from_slice(&0x7FFFu16.to_le_bytes());
        tree.extend_from_slice(&(data.len() as u32).to_le_bytes());
        tree.extend_from_slice(&(contents.len() as u32).to_le_bytes());
        tree.extend_from_slice(&0xFFFFu16.to_le_bytes());
        data.extend_from_slice(contents);
        tree.extend_from_slice(&[0, 0]); // end of names, then of folders
    }
    tree.push(0);
    let mut out = Vec::new();
    out.extend_from_slice(&0x55AA_1234u32.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&(tree.len() as u32).to_le_bytes());
    out.extend_from_slice(&tree);
    out.extend_from_slice(&data);
    std::fs::write(path, out).unwrap();
}

#[test]
fn backslash_folders_and_odd_bytes_in_names_still_work() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("odd.vpk");
    raw_archive(
        &path,
        &[
            (b"Materials\\Brick", b"Wall", b"vmt", b"wall"),
            (b"sound", b"caf\xe9", b"wav", b"latin-1 name"),
        ],
    );
    let vpk = Vpk::open(&path).unwrap();
    assert_eq!(vpk.read("materials/brick/wall.vmt").unwrap(), b"wall");
    assert!(vpk.contains("MATERIALS\\BRICK\\WALL.VMT"));
    assert_eq!(vpk.entries()[0].path, "Materials/Brick/Wall.vmt");
    assert_eq!(
        vpk.entries().len(),
        2,
        "the odd name doesn't spoil the archive"
    );
}

#[test]
fn very_long_names_are_refused_before_using_much_memory() {
    // Regression: one huge folder name shared by many entries was copied
    // into every path (a 1 MB file took 2 GB).
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("long.vpk");
    let folder = vec![b'a'; 1024 * 1024];
    raw_archive(&path, &[(&folder, b"x", b"txt", b"")]);
    let err = Vpk::open(&path).unwrap_err().to_string();
    assert!(err.contains("too long"), "{err}");
}

#[test]
fn directory_names_in_any_case_find_their_numbered_archives() {
    let dir = tempfile::tempdir().unwrap();
    for (dir_name, archive_name) in [
        ("pak01_Dir.vpk", "pak01_000.vpk"),
        ("PAK02_DIR.VPK", "PAK02_000.VPK"),
    ] {
        let lower = dir.path().join("tmp_dir.vpk");
        sample().write_split(&lower).unwrap();
        std::fs::rename(&lower, dir.path().join(dir_name)).unwrap();
        std::fs::rename(
            dir.path().join("tmp_000.vpk"),
            dir.path().join(archive_name),
        )
        .unwrap();
        let vpk = Vpk::open(&dir.path().join(dir_name)).unwrap();
        assert_eq!(
            vpk.read("readme").unwrap(),
            b"no folder, no extension",
            "{dir_name}"
        );
    }
}
