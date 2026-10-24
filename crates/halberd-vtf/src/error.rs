//! Why a texture or material could not be read.

use std::fmt;

/// Why a texture or material could not be read, in plain words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VtfError {
    /// Not a VTF texture.
    NotVtf,
    /// A VTF version Halberd does not know.
    UnsupportedVersion(u32, u32),
    /// A pixel format Halberd cannot show (named).
    UnsupportedFormat(&'static str),
    /// The texture is damaged; says how.
    Damaged(&'static str),
    /// The only stored size that could be shown is too large to decode
    /// for a picture of the size asked for.
    TooLarge(u32, u32),
    /// The material text could not be read.
    BadMaterial(String),
}

impl fmt::Display for VtfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotVtf => f.write_str("this is not a VTF texture"),
            Self::UnsupportedVersion(a, b) => write!(f, "VTF version {a}.{b} is not supported"),
            Self::UnsupportedFormat(name) => {
                write!(f, "textures stored as {name} can't be shown yet")
            }
            Self::Damaged(what) => write!(f, "the texture is damaged ({what})"),
            Self::TooLarge(w, h) => write!(
                f,
                "the texture is {w}×{h} with no smaller copy, too large to preview"
            ),
            Self::BadMaterial(why) => write!(f, "the material could not be read: {why}"),
        }
    }
}

impl std::error::Error for VtfError {}
