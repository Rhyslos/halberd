//! Reading VTF textures written here by hand, and VMT materials.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use halberd_vtf::{ImageFormat, Material, Vtf, VtfError};

const BGRA8888: u32 = 12;
const DXT1: u32 = 13;
const DXT5: u32 = 15;
const P8: u32 = 7;

/// A VTF of version 7.`minor` holding `images` (smallest size first, each
/// already in `format`), `width` × `height` at full size.
fn vtf(
    minor: u32,
    format: u32,
    width: u16,
    height: u16,
    images: &[Vec<u8>],
    flags: u32,
    frames: u16,
) -> Vec<u8> {
    let header_size: u32 = if minor >= 3 { 88 } else { 80 };
    let mut h = vec![0u8; header_size as usize];
    h[..4].copy_from_slice(b"VTF\0");
    h[4..8].copy_from_slice(&7u32.to_le_bytes());
    h[8..12].copy_from_slice(&minor.to_le_bytes());
    h[12..16].copy_from_slice(&header_size.to_le_bytes());
    h[16..18].copy_from_slice(&width.to_le_bytes());
    h[18..20].copy_from_slice(&height.to_le_bytes());
    h[20..24].copy_from_slice(&flags.to_le_bytes());
    h[24..26].copy_from_slice(&frames.to_le_bytes());
    h[26..28].copy_from_slice(&0xFFFFu16.to_le_bytes());
    h[52..56].copy_from_slice(&format.to_le_bytes());
    h[56] = images.len() as u8;
    h[57..61].copy_from_slice(&u32::MAX.to_le_bytes()); // no small preview
    h[63..65].copy_from_slice(&1u16.to_le_bytes());
    if minor >= 3 {
        h[68..72].copy_from_slice(&1u32.to_le_bytes());
        h[80..83].copy_from_slice(&[0x30, 0, 0]);
        h[84..88].copy_from_slice(&header_size.to_le_bytes());
    }
    for image in images {
        h.extend_from_slice(image);
    }
    h
}

fn bgra(w: usize, h: usize, px: [u8; 4]) -> Vec<u8> {
    px.repeat(w * h)
}

#[test]
fn plain_pixels_decode_exactly() {
    let mut pixels = Vec::new();
    for px in [
        [0, 0, 255, 255],
        [0, 255, 0, 128],
        [255, 0, 0, 0],
        [10, 20, 30, 40],
    ] {
        pixels.extend_from_slice(&px);
    }
    let bytes = vtf(2, BGRA8888, 2, 2, &[pixels], 0, 1);
    let tex = Vtf::parse(&bytes).unwrap();
    assert_eq!(
        (tex.width(), tex.height(), tex.format()),
        (2, 2, ImageFormat::Bgra8888)
    );
    let image = tex.decode(1024).unwrap();
    assert_eq!(&image.pixels[..8], &[255, 0, 0, 255, 0, 255, 0, 128]);
    assert_eq!(&image.pixels[12..], &[30, 20, 10, 40]);
}

#[test]
fn dxt_blocks_decode_with_their_alpha() {
    // Solid red: both colours 0xF800, indices all 0.
    let red = [0x00, 0xF8, 0x00, 0xF8, 0, 0, 0, 0];
    let bytes = vtf(5, DXT1, 4, 4, &[red.to_vec()], 0, 1);
    let image = Vtf::parse(&bytes).unwrap().decode(64).unwrap();
    assert!(image.pixels.chunks(4).all(|p| p == [255, 0, 0, 255]));
    // Three-colour mode (first colour not above the second): index 3 is
    // transparent.
    let clear = [0x00, 0x00, 0x00, 0xF8, 0xFF, 0xFF, 0xFF, 0xFF];
    let image = Vtf::parse(&vtf(2, DXT1, 4, 4, &[clear.to_vec()], 0, 1))
        .unwrap()
        .decode(64)
        .unwrap();
    assert!(image.pixels.chunks(4).all(|p| p[3] == 0));
    // DXT5: alpha 255 and 0 by index, over a white block.
    let mut block = vec![255, 0];
    block.extend_from_slice(&[0b0000_1000, 0, 0, 0, 0, 0]); // pixel 1 uses index 1 (0)
    block.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0]);
    let image = Vtf::parse(&vtf(4, DXT5, 4, 4, &[block], 0, 1))
        .unwrap()
        .decode(64)
        .unwrap();
    assert_eq!(&image.pixels[..8], &[255, 255, 255, 255, 255, 255, 255, 0]);
    // Sizes that aren't a multiple of 4.
    let six = [red; 4].concat();
    let image = Vtf::parse(&vtf(2, DXT1, 6, 6, &[six], 0, 1))
        .unwrap()
        .decode(64)
        .unwrap();
    assert_eq!(
        (image.width, image.height, image.pixels.len()),
        (6, 6, 6 * 6 * 4)
    );
}

#[test]
fn the_largest_size_that_fits_is_chosen() {
    let images = vec![
        bgra(1, 1, [1, 1, 1, 255]),
        bgra(2, 2, [2, 2, 2, 255]),
        bgra(4, 4, [3, 3, 3, 255]),
    ];
    for minor in [1, 2, 3, 5] {
        let bytes = vtf(minor, BGRA8888, 4, 4, &images, 0, 1);
        let tex = Vtf::parse(&bytes).unwrap();
        assert_eq!(tex.mip_count(), 3);
        for (max, size, value) in [(100, 4, 3), (2, 2, 2), (1, 1, 1), (0, 1, 1)] {
            let image = tex.decode(max).unwrap();
            assert_eq!(
                (image.width, image.pixels[0]),
                (size, value),
                "7.{minor} max {max}"
            );
        }
    }
}

#[test]
fn the_first_frame_and_face_are_shown() {
    // Two frames of a cube map (6 faces): the first image of the full size
    // is frame 0, face 0.
    let mut full = bgra(2, 2, [9, 9, 9, 255]);
    for _ in 1..12 {
        full.extend_from_slice(&bgra(2, 2, [0, 0, 0, 255]));
    }
    let mut small = Vec::new();
    for _ in 0..12 {
        small.extend_from_slice(&bgra(1, 1, [5, 5, 5, 255]));
    }
    let bytes = vtf(5, BGRA8888, 2, 2, &[small, full], 0x4000, 2);
    let image = Vtf::parse(&bytes).unwrap().decode(64).unwrap();
    assert_eq!(image.pixels[0], 9);
}

#[test]
fn bad_textures_are_refused_with_a_reason() {
    let good = vtf(2, BGRA8888, 2, 2, &[bgra(2, 2, [0; 4])], 0, 1);
    let mut wrong_version = good.clone();
    wrong_version[4] = 8;
    let mut huge = good.clone();
    huge[16..18].copy_from_slice(&u16::MAX.to_le_bytes());
    let cases: Vec<(Vec<u8>, &str)> = vec![
        (b"not a texture".to_vec(), "not a VTF"),
        (wrong_version, "version 8"),
        (good[..40].to_vec(), "cut short"),
        (good[..good.len() - 1].to_vec(), "past the end"),
        (huge, "impossible"),
    ];
    for (bytes, expect) in cases {
        let err = Vtf::parse(&bytes).unwrap_err().to_string();
        assert!(err.contains(expect), "{expect}: {err}");
    }
    let palette = vtf(2, P8, 2, 2, &[vec![0; 4]], 0, 1);
    assert!(matches!(
        Vtf::parse(&palette).unwrap().decode(64),
        Err(VtfError::UnsupportedFormat(_))
    ));
}

#[test]
fn fuzz_damaged_textures_never_crash() {
    let images = vec![vec![0x00, 0xF8, 0x00, 0xF8, 1, 2, 3, 4]; 1];
    let good = vtf(5, DXT1, 4, 4, &images, 0, 1);
    let mut seed: u64 = 0x7E71_0000_0000_0005;
    let mut next = move |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n.max(1) as u64) as usize
    };
    for round in 0..20_000 {
        let mut bytes = good.clone();
        if round % 7 == 0 {
            bytes.truncate(next(bytes.len()));
        }
        for _ in 0..1 + next(4) {
            if !bytes.is_empty() {
                let at = next(bytes.len());
                bytes[at] = next(256) as u8;
            }
        }
        if let Ok(tex) = Vtf::parse(&bytes)
            && let Ok(image) = tex.decode(next(64) as u32)
        {
            assert_eq!(
                image.pixels.len(),
                (image.width * image.height * 4) as usize
            );
        }
    }
}

#[test]
fn materials_name_their_texture() {
    let wall = Material::parse(
        "\"LightmappedGeneric\"\n{\n  \"$basetexture\" \"Brick\\\\BrickWall001A\"\n  \"$surfaceprop\" \"brick\"\n  \"Proxies\" { }\n}",
    )
    .unwrap();
    assert_eq!(wall.shader, "LightmappedGeneric");
    assert_eq!(
        wall.texture_path().as_deref(),
        Some("materials/brick/brickwall001a.vtf")
    );
    assert_eq!(wall.param("$SURFACEPROP"), Some("brick"));
    assert!(!wall.is_tool());
    let nodraw = Material::parse(
        "LightmappedGeneric { $basetexture tools/toolsnodraw %tooltexture \"tools/toolsnodraw\" %compilenodraw 1 $translucent 1 [$X360] }",
    )
    .unwrap();
    assert!(nodraw.is_tool());
    assert_eq!(
        nodraw.texture_path().as_deref(),
        Some("materials/tools/toolsnodraw.vtf")
    );
    let patch = Material::parse(
        "patch { include \"materials/brick/brickwall001a.vmt\" replace { $basetexture brick/other } }",
    )
    .unwrap();
    assert_eq!(patch.include(), Some("materials/brick/brickwall001a.vmt"));
    assert_eq!(
        patch.texture_path().as_deref(),
        Some("materials/brick/other.vtf")
    );
    assert!(Material::parse("").is_err());
    assert!(Material::parse("\"key\" \"value\"").is_err());
    assert_eq!(
        Material::parse("UnlitGeneric { }").unwrap().texture_path(),
        None
    );
}

#[test]
fn fuzz_damaged_materials_never_crash() {
    let good = b"\"Patch\"\n{\n  include \"materials/brick/a.vmt\"\n  insert { \"$basetexture\" \"x\\\\\\\\y\" }\n}\n";
    let mut seed: u64 = 0x7E71_0000_0000_0011;
    let mut next = move |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n.max(1) as u64) as usize
    };
    for _ in 0..20_000 {
        let mut bytes = good.to_vec();
        for _ in 0..1 + next(6) {
            let at = next(bytes.len());
            let pool = b"{}\"\\/ \n$ax";
            bytes[at] = pool[next(pool.len())];
        }
        let text = String::from_utf8_lossy(&bytes);
        if let Ok(material) = Material::parse(&text) {
            let _ = material.texture_path();
            let _ = material.is_tool();
        }
    }
}
