//! Why a map could not be opened or saved.

use std::fmt;
use std::path::PathBuf;

/// Why a map could not be opened or saved. Messages are written for the
/// user.
#[derive(Debug)]
pub enum IoError {
    /// The file could not be read.
    Read(PathBuf, std::io::Error),
    /// The file could not be written.
    Write(PathBuf, std::io::Error),
    /// The file is larger than [`crate::MAX_MAP_FILE_BYTES`].
    TooLarge(PathBuf, u64),
    /// The file is not a map Halberd can read.
    NotAMap(PathBuf, halberd_vmf::VmfError),
    /// The map has more objects than the editor allows.
    TooManyObjects,
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(path, e) => write!(f, "{} could not be read: {e}", path.display()),
            Self::Write(path, e) => write!(f, "{} could not be saved: {e}", path.display()),
            Self::TooLarge(path, bytes) => write!(
                f,
                "{} is too large to open ({} MB; the limit is {} MB)",
                path.display(),
                bytes / 1_000_000,
                crate::MAX_MAP_FILE_BYTES / 1_000_000
            ),
            Self::NotAMap(path, e) => write!(f, "{} could not be opened: {e}", path.display()),
            Self::TooManyObjects => f.write_str("the map has more objects than Halberd can hold"),
        }
    }
}

impl std::error::Error for IoError {}
