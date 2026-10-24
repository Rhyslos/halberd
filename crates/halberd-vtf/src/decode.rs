//! Turning stored pixels into RGBA: the block-compressed DXT formats and
//! the plain ones Source uses.

use crate::vtf::ImageFormat;

/// Decodes `data` (exactly one image of `width` × `height` in `format`;
/// the caller checks its length) into RGBA, 4 bytes a pixel.
pub(crate) fn to_rgba(format: ImageFormat, width: usize, height: usize, data: &[u8]) -> Vec<u8> {
    match format {
        ImageFormat::Dxt1 | ImageFormat::Dxt1OneBitAlpha => {
            blocks(width, height, data, 8, |b, out| color_block(b, true, out))
        }
        ImageFormat::Dxt3 => blocks(width, height, data, 16, |b, out| {
            color_block(&b[8..], false, out);
            for (i, px) in out.iter_mut().enumerate() {
                let nibble = (b[i / 2] >> ((i % 2) * 4)) & 0x0F;
                px[3] = nibble * 17;
            }
        }),
        ImageFormat::Dxt5 => blocks(width, height, data, 16, |b, out| {
            color_block(&b[8..], false, out);
            let alphas = alpha_palette(b[0], b[1]);
            let bits = u64::from_le_bytes([b[2], b[3], b[4], b[5], b[6], b[7], 0, 0]);
            for (i, px) in out.iter_mut().enumerate() {
                px[3] = alphas[((bits >> (3 * i)) & 7) as usize];
            }
        }),
        _ => plain(format, data),
    }
}

/// Pixels in 4 × 4 blocks of `size` bytes each.
fn blocks(
    width: usize,
    height: usize,
    data: &[u8],
    size: usize,
    decode: impl Fn(&[u8], &mut [[u8; 4]; 16]),
) -> Vec<u8> {
    let mut rgba = vec![0u8; width * height * 4];
    let across = width.div_ceil(4).max(1);
    for (n, block) in data.chunks_exact(size).enumerate() {
        let (bx, by) = ((n % across) * 4, (n / across) * 4);
        let mut out = [[0u8; 4]; 16];
        decode(block, &mut out);
        for (i, px) in out.iter().enumerate() {
            let (x, y) = (bx + i % 4, by + i / 4);
            if x < width && y < height {
                let at = (y * width + x) * 4;
                rgba[at..at + 4].copy_from_slice(px);
            }
        }
    }
    rgba
}

/// A DXT colour block: two RGB565 colours and 2-bit indices. In DXT1, a
/// first colour not above the second means three colours and transparent.
fn color_block(b: &[u8], may_be_transparent: bool, out: &mut [[u8; 4]; 16]) {
    let c0 = u16::from_le_bytes([b[0], b[1]]);
    let c1 = u16::from_le_bytes([b[2], b[3]]);
    let (p0, p1) = (rgb565(c0), rgb565(c1));
    let mix = |a: u8, b: u8, wa: u16, wb: u16| {
        ((u16::from(a) * wa + u16::from(b) * wb) / (wa + wb)) as u8
    };
    let blend = |wa, wb| {
        [
            mix(p0[0], p1[0], wa, wb),
            mix(p0[1], p1[1], wa, wb),
            mix(p0[2], p1[2], wa, wb),
            255,
        ]
    };
    let palette = if c0 > c1 || !may_be_transparent {
        [p0, p1, blend(2, 1), blend(1, 2)]
    } else {
        [p0, p1, blend(1, 1), [0, 0, 0, 0]]
    };
    let indices = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
    for (i, px) in out.iter_mut().enumerate() {
        *px = palette[((indices >> (2 * i)) & 3) as usize];
    }
}

fn rgb565(c: u16) -> [u8; 4] {
    let r = ((c >> 11) & 0x1F) as u8;
    let g = ((c >> 5) & 0x3F) as u8;
    let b = (c & 0x1F) as u8;
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
        255,
    ]
}

/// DXT5's eight alpha levels from its two end points.
fn alpha_palette(a0: u8, a1: u8) -> [u8; 8] {
    let (a, b) = (u16::from(a0), u16::from(a1));
    let lerp = |wa: u16, wb: u16, div: u16| ((a * wa + b * wb) / div) as u8;
    if a0 > a1 {
        [
            a0,
            a1,
            lerp(6, 1, 7),
            lerp(5, 2, 7),
            lerp(4, 3, 7),
            lerp(3, 4, 7),
            lerp(2, 5, 7),
            lerp(1, 6, 7),
        ]
    } else {
        [
            a0,
            a1,
            lerp(4, 1, 5),
            lerp(3, 2, 5),
            lerp(2, 3, 5),
            lerp(1, 4, 5),
            0,
            255,
        ]
    }
}

/// The formats that store each pixel on its own.
fn plain(format: ImageFormat, data: &[u8]) -> Vec<u8> {
    let bytes = format.bytes_per_pixel().unwrap_or(4);
    let mut rgba = Vec::with_capacity(data.len() / bytes.max(1) * 4);
    for p in data.chunks_exact(bytes) {
        let px: [u8; 4] = match format {
            ImageFormat::Rgba8888 => [p[0], p[1], p[2], p[3]],
            ImageFormat::Abgr8888 => [p[3], p[2], p[1], p[0]],
            ImageFormat::Argb8888 => [p[1], p[2], p[3], p[0]],
            ImageFormat::Bgra8888 => [p[2], p[1], p[0], p[3]],
            ImageFormat::Bgrx8888 => [p[2], p[1], p[0], 255],
            ImageFormat::Rgb888 | ImageFormat::Rgb888Bluescreen => [p[0], p[1], p[2], 255],
            ImageFormat::Bgr888 | ImageFormat::Bgr888Bluescreen => [p[2], p[1], p[0], 255],
            ImageFormat::Rgb565 => {
                let c = rgb565(u16::from_le_bytes([p[0], p[1]]));
                [c[0], c[1], c[2], 255]
            }
            ImageFormat::Bgr565 => {
                let c = rgb565(u16::from_le_bytes([p[0], p[1]]));
                [c[2], c[1], c[0], 255]
            }
            ImageFormat::Bgra4444 => {
                let v = u16::from_le_bytes([p[0], p[1]]);
                let n = |s: u16| (((v >> s) & 0xF) as u8) * 17;
                [n(8), n(4), n(0), n(12)]
            }
            ImageFormat::Bgra5551 | ImageFormat::Bgrx5551 => {
                let v = u16::from_le_bytes([p[0], p[1]]);
                let n = |s: u16| {
                    let c = ((v >> s) & 0x1F) as u8;
                    (c << 3) | (c >> 2)
                };
                let alpha = if format == ImageFormat::Bgrx5551 || v & 0x8000 != 0 {
                    255
                } else {
                    0
                };
                [n(10), n(5), n(0), alpha]
            }
            ImageFormat::I8 => [p[0], p[0], p[0], 255],
            ImageFormat::Ia88 => [p[0], p[0], p[0], p[1]],
            ImageFormat::A8 => [255, 255, 255, p[0]],
            ImageFormat::Uv88 => [p[0], p[1], 255, 255],
            ImageFormat::Uvwq8888 | ImageFormat::Uvlx8888 => [p[0], p[1], p[2], p[3]],
            _ => [255, 0, 255, 255],
        };
        rgba.extend_from_slice(&px);
    }
    rgba
}
