//! Read Valve VPK archives.
//!
//! GMod and Half-Life 2 base content ships inside VPK archives: a directory
//! file (`name_dir.vpk`) listing every file, and numbered archives
//! (`name_000.vpk`, `name_001.vpk`…) holding their bytes. [`Vpk::open`]
//! reads only the directory's list; [`Vpk::read`] reads one file's bytes
//! in place when asked, checking every size and offset against the real
//! file first.
//!
//! [`VpkBuilder`] writes small archives, for tests and tools.

mod crc;
mod error;
mod read;
mod write;

pub use crc::crc32;
pub use error::VpkError;
pub use read::{MAX_ENTRIES, MAX_FILE_SIZE, MAX_TREE_SIZE, Vpk, VpkEntry};
pub use write::VpkBuilder;

/// The name of this crate, used in logs and diagnostics.
pub const CRATE_NAME: &str = "halberd-vpk";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
