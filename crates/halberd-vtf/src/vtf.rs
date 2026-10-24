//! VTF textures: the header, where each size of the image is stored, and
//! decoding one of them.
//!
//! The images follow the header from the smallest size to the largest;
//! within each size come every frame, then every face (six for cube maps),
//! then every depth slice. Version 7.3 and later list where the images
//! start in a table of resources instead.

use crate::decode::to_rgba;
use crate::error::VtfError;

/// Largest width or height accepted (real textures are at most 4096).
pub const MAX_DIMENSION: u32 = 16_384;
/// Most frames, faces or depth slices accepted.
const MAX_LAYERS: u64 = 4096;
/// Most bytes of image data accepted.
const MAX_DATA: u64 = 1024 * 1024 * 1024;
/// Resource tag of the full-size images (version 7.3 and later).
const HIGH_RES_TAG: [u8; 3] = [0x30, 0, 0];
/// Flag: the texture is a cube map.
const ENVMAP: u32 = 0x4000;

/// How a VTF stores its pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum ImageFormat {
    Rgba8888,
    Abgr8888,
    Rgb888,
    Bgr888,
    Rgb565,
    I8,
    Ia88,
    P8,
    A8,
    Rgb888Bluescreen,
    Bgr888Bluescreen,
    Argb8888,
    Bgra8888,
    Dxt1,
    Dxt3,
    Dxt5,
    Bgrx8888,
    Bgr565,
    Bgrx5551,
    Bgra4444,
    Dxt1OneBitAlpha,
    Bgra5551,
    Uv88,
    Uvwq8888,
    Rgba16161616F,
    Rgba16161616,
    Uvlx8888,
}

impl ImageFormat {
    fn from_code(code: u32) -> Option<Self> {
        use ImageFormat::*;
        const ALL: [ImageFormat; 27] = [
            Rgba8888,
            Abgr8888,
            Rgb888,
            Bgr888,
            Rgb565,
            I8,
            Ia88,
            P8,
            A8,
            Rgb888Bluescreen,
            Bgr888Bluescreen,
            Argb8888,
            Bgra8888,
            Dxt1,
            Dxt3,
            Dxt5,
            Bgrx8888,
            Bgr565,
            Bgrx5551,
            Bgra4444,
            Dxt1OneBitAlpha,
            Bgra5551,
            Uv88,
            Uvwq8888,
            Rgba16161616F,
            Rgba16161616,
            Uvlx8888,
        ];
        ALL.get(code as usize).copied()
    }

    /// Bytes a pixel, for the formats that store each pixel on its own.
    pub(crate) fn bytes_per_pixel(self) -> Option<usize> {
        use ImageFormat::*;
        Some(match self {
            Rgba8888 | Abgr8888 | Argb8888 | Bgra8888 | Bgrx8888 | Uvwq8888 | Uvlx8888 => 4,
            Rgb888 | Bgr888 | Rgb888Bluescreen | Bgr888Bluescreen => 3,
            Rgb565 | Ia88 | Bgr565 | Bgrx5551 | Bgra4444 | Bgra5551 | Uv88 => 2,
            I8 | P8 | A8 => 1,
            Rgba16161616F | Rgba16161616 => 8,
            Dxt1 | Dxt3 | Dxt5 | Dxt1OneBitAlpha => return None,
        })
    }

    /// Bytes one image of `width` × `height` takes.
    fn image_bytes(self, width: u64, height: u64) -> u64 {
        use ImageFormat::*;
        match self {
            Dxt1 | Dxt1OneBitAlpha => width.div_ceil(4).max(1) * height.div_ceil(4).max(1) * 8,
            Dxt3 | Dxt5 => width.div_ceil(4).max(1) * height.div_ceil(4).max(1) * 16,
            other => width * height * other.bytes_per_pixel().unwrap_or(4) as u64,
        }
    }

    /// True if [`Vtf::decode`] can show it.
    fn decodable(self) -> bool {
        !matches!(self, Self::P8 | Self::Rgba16161616F | Self::Rgba16161616)
    }

    /// A name for messages.
    fn name(self) -> &'static str {
        match self {
            Self::P8 => "a palette",
            Self::Rgba16161616F | Self::Rgba16161616 => "16-bit colour (HDR)",
            _ => "this format",
        }
    }
}

/// Pixels, 4 bytes each (red, green, blue, alpha), row by row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height × 4` bytes.
    pub pixels: Vec<u8>,
}

/// A parsed VTF texture, borrowing the file's bytes.
#[derive(Debug, Clone)]
pub struct Vtf<'a> {
    bytes: &'a [u8],
    width: u32,
    height: u32,
    format: ImageFormat,
    mip_count: u32,
    /// Frames × faces: images of each size before the first one we want.
    layers: u64,
    depth: u32,
    /// Where the smallest full-size image starts.
    images_at: u64,
}

impl<'a> Vtf<'a> {
    /// Reads a texture's header and checks that all its images fit in the
    /// file.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, VtfError> {
        if bytes.get(..4) != Some(b"VTF\0".as_slice()) {
            return Err(VtfError::NotVtf);
        }
        let header = Header { bytes };
        let (major, minor) = (header.u32(4)?, header.u32(8)?);
        if major != 7 || minor > 5 {
            return Err(VtfError::UnsupportedVersion(major, minor));
        }
        let header_size = u64::from(header.u32(12)?);
        let width = u32::from(header.u16(16)?);
        let height = u32::from(header.u16(18)?);
        let flags = header.u32(20)?;
        let frames = u64::from(header.u16(24)?.max(1));
        let first_frame = header.u16(26)?;
        let format = ImageFormat::from_code(header.u32(52)?)
            .ok_or(VtfError::Damaged("an unknown pixel format"))?;
        let mip_count = u32::from(header.u8(56)?.max(1));
        let low_format = header.u32(57)?;
        let (low_width, low_height) = (u64::from(header.u8(61)?), u64::from(header.u8(62)?));
        let depth = if minor >= 2 {
            u32::from(header.u16(63)?.max(1))
        } else {
            1
        };
        if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
            return Err(VtfError::Damaged("its size is impossible"));
        }
        if mip_count > 16 || frames > MAX_LAYERS || u64::from(depth) > MAX_LAYERS {
            return Err(VtfError::Damaged("it claims too many images"));
        }
        let faces: u64 = if flags & ENVMAP == 0 {
            1
        } else if minor < 5 && first_frame != 0xFFFF {
            7 // Older cube maps carry a sphere map too.
        } else {
            6
        };
        let images_at = if minor >= 3 {
            let count = u64::from(header.u32(68)?).min(64);
            (0..count)
                .find_map(|i| {
                    let at = 80 + i as usize * 8;
                    let tag = bytes.get(at..at + 3)?;
                    (tag == HIGH_RES_TAG).then(|| header.u32(at + 4).ok().map(u64::from))?
                })
                .ok_or(VtfError::Damaged("it has no image"))?
        } else {
            let low = ImageFormat::from_code(low_format)
                .filter(|_| low_width > 0 && low_height > 0)
                .map_or(0, |f| f.image_bytes(low_width, low_height));
            header_size + low
        };
        let vtf = Self {
            bytes,
            width,
            height,
            format,
            mip_count,
            layers: frames * faces,
            depth,
            images_at,
        };
        // Every size must fit in the file.
        let (_, end) = vtf.span(0)?;
        if end > bytes.len() as u64 {
            return Err(VtfError::Damaged("its images run past the end of the file"));
        }
        Ok(vtf)
    }

    /// Full-size width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Full-size height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// How the pixels are stored.
    pub fn format(&self) -> ImageFormat {
        self.format
    }

    /// How many sizes (mipmaps) it stores.
    pub fn mip_count(&self) -> u32 {
        self.mip_count
    }

    /// The first frame, face and slice of the largest stored size that is
    /// at most `max_size` pixels on each side (the smallest size if none
    /// is), as RGBA.
    pub fn decode(&self, max_size: u32) -> Result<RgbaImage, VtfError> {
        if !self.format.decodable() {
            return Err(VtfError::UnsupportedFormat(self.format.name()));
        }
        let mip = (0..self.mip_count)
            .find(|&m| self.size_at(m).0 <= max_size && self.size_at(m).1 <= max_size)
            .unwrap_or(self.mip_count - 1);
        let (w, h) = self.size_at(mip);
        let (start, _) = self.span(mip)?;
        let len = self.format.image_bytes(u64::from(w), u64::from(h));
        let data = usize::try_from(start)
            .ok()
            .zip(usize::try_from(start + len).ok())
            .and_then(|(a, b)| self.bytes.get(a..b))
            .ok_or(VtfError::Damaged("an image runs past the end of the file"))?;
        Ok(RgbaImage {
            width: w,
            height: h,
            pixels: to_rgba(self.format, w as usize, h as usize, data),
        })
    }

    /// Width and height of size `mip` (0 is the full size).
    fn size_at(&self, mip: u32) -> (u32, u32) {
        ((self.width >> mip).max(1), (self.height >> mip).max(1))
    }

    /// Where the images of size `mip` start, and where the largest size
    /// ends (the end of all image data).
    fn span(&self, mip: u32) -> Result<(u64, u64), VtfError> {
        let mut at = self.images_at;
        let mut start = None;
        for m in (0..self.mip_count).rev() {
            if m == mip {
                start = Some(at);
            }
            let (w, h) = self.size_at(m);
            let depth = u64::from((self.depth >> m).max(1));
            let all = self.format.image_bytes(u64::from(w), u64::from(h)) * self.layers * depth;
            at = at
                .checked_add(all)
                .ok_or(VtfError::Damaged("its sizes overflow"))?;
            if at - self.images_at > MAX_DATA {
                return Err(VtfError::Damaged(
                    "it claims more image data than any real texture",
                ));
            }
        }
        Ok((start.unwrap_or(self.images_at), at))
    }
}

/// Reading little-endian numbers from the header, with bounds checks.
struct Header<'a> {
    bytes: &'a [u8],
}

impl Header<'_> {
    fn get<const N: usize>(&self, at: usize) -> Result<[u8; N], VtfError> {
        self.bytes
            .get(at..at + N)
            .and_then(|s| s.try_into().ok())
            .ok_or(VtfError::Damaged("the header is cut short"))
    }

    fn u8(&self, at: usize) -> Result<u8, VtfError> {
        Ok(self.get::<1>(at)?[0])
    }

    fn u16(&self, at: usize) -> Result<u16, VtfError> {
        Ok(u16::from_le_bytes(self.get(at)?))
    }

    fn u32(&self, at: usize) -> Result<u32, VtfError> {
        Ok(u32::from_le_bytes(self.get(at)?))
    }
}
