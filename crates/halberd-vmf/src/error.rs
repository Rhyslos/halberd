//! Why a VMF could not be read.

use halberd_kv::KvError;
use std::fmt;

/// Why a VMF could not be read. Messages are written for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmfError {
    /// The text is not valid KeyValues.
    Syntax(KvError),
    /// There is no `world` block, so this is not a Hammer map.
    NoWorld,
    /// A value that should hold numbers does not.
    BadNumbers {
        /// What was being read, such as "plane".
        what: &'static str,
        /// The text found.
        found: String,
    },
}

impl fmt::Display for VmfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(e) => write!(f, "the file is damaged ({e})"),
            Self::NoWorld => f.write_str("the file has no world block, so it is not a Hammer map"),
            Self::BadNumbers { what, found } => {
                write!(f, "a {what} has unreadable numbers: \"{found}\"")
            }
        }
    }
}

impl std::error::Error for VmfError {}
