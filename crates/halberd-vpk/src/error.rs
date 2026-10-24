//! Why an archive or a file in it could not be read.

use std::fmt;

/// Why an archive or a file in it could not be read, in plain words.
#[derive(Debug)]
pub enum VpkError {
    /// The file could not be opened or read.
    Io(std::io::Error),
    /// Not a VPK file (wrong signature).
    NotVpk,
    /// A VPK version other than 1 or 2.
    UnsupportedVersion(u32),
    /// The file list is damaged; says where it went wrong.
    Damaged(&'static str),
    /// Sizes larger than any real archive (a damaged or hostile file).
    TooLarge(&'static str),
    /// No such file in the archive.
    NotFound(String),
    /// A file's bytes do not match the checksum the archive lists.
    ChecksumMismatch(String),
}

impl fmt::Display for VpkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "the archive could not be read ({e})"),
            Self::NotVpk => f.write_str("this is not a VPK archive"),
            Self::UnsupportedVersion(v) => write!(f, "VPK version {v} is not supported"),
            Self::Damaged(what) => write!(f, "the archive is damaged ({what})"),
            Self::TooLarge(what) => write!(f, "the archive is too large to be real ({what})"),
            Self::NotFound(path) => write!(f, "\"{path}\" is not in the archive"),
            Self::ChecksumMismatch(path) => {
                write!(
                    f,
                    "\"{path}\" in the archive is damaged (its checksum does not match)"
                )
            }
        }
    }
}

impl std::error::Error for VpkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for VpkError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
