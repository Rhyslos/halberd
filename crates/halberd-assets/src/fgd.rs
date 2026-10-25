//! Garry's Mod's entity definitions (FGD files): where they are, and
//! reading them.

use crate::GmodInstall;
use halberd_fgd::Fgd;
use std::path::{Path, PathBuf};

/// The FGD files Garry's Mod ships, best first. `garrysmod.fgd` includes
/// Half-Life 2's and the base game's.
const KNOWN_FGDS: [&str; 3] = ["garrysmod.fgd", "halflife2.fgd", "base.fgd"];
/// Most files looked at in `bin` when none of the known ones is there.
const MAX_LISTED: usize = 4096;

/// The FGD file to read for the install at `root`: one of Garry's Mod's
/// own in `bin`, else any `.fgd` there (alphabetically first), else none.
pub fn find_fgd(root: &Path) -> Option<PathBuf> {
    let bin = root.join("bin");
    for name in KNOWN_FGDS {
        let path = bin.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    let mut others: Vec<PathBuf> = std::fs::read_dir(&bin)
        .ok()?
        .take(MAX_LISTED)
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("fgd")))
        .collect();
    others.sort();
    others.into_iter().next()
}

/// Reads `install`'s entity definitions, if it has any: the file read and
/// what it holds (problems included).
pub fn load_fgd(install: &GmodInstall) -> Option<(PathBuf, Fgd)> {
    let path = find_fgd(&install.root)?;
    let fgd = halberd_fgd::load(&path);
    Some((path, fgd))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garrysmods_own_file_is_preferred_then_any() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(find_fgd(root.path()), None, "no bin folder");
        let bin = root.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        std::fs::write(bin.join("zz.fgd"), "").unwrap();
        std::fs::write(bin.join("mod.FGD"), "").unwrap();
        assert_eq!(find_fgd(root.path()), Some(bin.join("mod.FGD")));
        std::fs::write(bin.join("halflife2.fgd"), "").unwrap();
        assert_eq!(find_fgd(root.path()), Some(bin.join("halflife2.fgd")));
        std::fs::write(bin.join("garrysmod.fgd"), "").unwrap();
        assert_eq!(find_fgd(root.path()), Some(bin.join("garrysmod.fgd")));
    }
}
