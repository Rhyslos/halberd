//! Why a model could not be read.

use std::fmt;

/// Why a model could not be read, in plain words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MdlError {
    /// The named file (`.mdl`, `.vvd` or `.vtx`) is not what it should be.
    WrongKind(&'static str),
    /// A version of the named file Halberd does not know.
    UnsupportedVersion(&'static str, i32),
    /// The model's files were not made together (their checksums differ).
    Mismatched,
    /// The named file is damaged; says how.
    Damaged(&'static str, &'static str),
    /// The model is bigger than Halberd will read (says what).
    TooLarge(&'static str),
}

impl fmt::Display for MdlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongKind(file) => write!(f, "its {file} file is not a Source model file"),
            Self::UnsupportedVersion(file, v) => {
                write!(f, "its {file} file is version {v}, which is not supported")
            }
            Self::Mismatched => f.write_str(
                "its .mdl, .vvd and .vtx files don't belong together (they were made at \
                 different times)",
            ),
            Self::Damaged(file, what) => write!(f, "its {file} file is damaged ({what})"),
            Self::TooLarge(what) => write!(f, "it is too large to show ({what})"),
        }
    }
}

impl std::error::Error for MdlError {}
