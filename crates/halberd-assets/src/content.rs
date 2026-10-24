//! Mounting game content: the places Garry's Mod reads files from, in the
//! order it searches them, as its `gameinfo.txt` lists them. Base content
//! (Half-Life 2's and GMod's own) is inside VPK archives; the rest is loose
//! files in folders.
//!
//! A file is looked for in each place in turn and the first one that has
//! it wins, as in the engine. Archive file lists are read when mounting;
//! loose folders are only looked into when a file is asked for.

use crate::GmodInstall;
use halberd_kv::{Block, Entry};
use halberd_vpk::Vpk;
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

/// Largest loose file read.
const MAX_LOOSE_FILE: u64 = 1024 * 1024 * 1024;
/// Most places one `*` search path mounts (addons): more are skipped, but
/// the search paths after it are still mounted.
const MAX_EXPANDED: usize = 4096;
/// Largest `gameinfo.txt` read.
const MAX_GAMEINFO: u64 = 1024 * 1024;

/// One place files are read from.
#[derive(Debug)]
enum Source {
    Folder(PathBuf),
    Archive(Vpk),
}

/// Why a file could not be read from the game's content.
#[derive(Debug)]
pub enum ContentError {
    /// No mounted place has it.
    NotFound(String),
    /// The path is not a plain relative path inside the game (`..`, a
    /// drive or a leading slash).
    BadPath(String),
    /// It was found but could not be read.
    Unreadable(String, String),
}

impl std::fmt::Display for ContentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(p) => write!(f, "\"{p}\" is not in the game's content"),
            Self::BadPath(p) => write!(f, "\"{p}\" is not a path inside the game"),
            Self::Unreadable(p, why) => write!(f, "\"{p}\" could not be read: {why}"),
        }
    }
}

impl std::error::Error for ContentError {}

/// The game's content, mounted.
#[derive(Debug, Default)]
pub struct GameContent {
    sources: Vec<Source>,
    /// Plain-language notes about places that could not be mounted.
    problems: Vec<String>,
}

impl GameContent {
    /// Mounts what `install`'s `gameinfo.txt` lists. Places that can't be
    /// mounted are skipped with a note (see [`GameContent::problems`]).
    pub fn mount(install: &GmodInstall) -> Self {
        let gameinfo = install.game_dir.join("gameinfo.txt");
        let text = std::fs::metadata(&gameinfo)
            .ok()
            .filter(|m| m.is_file() && m.len() <= MAX_GAMEINFO)
            .and_then(|_| std::fs::read(&gameinfo).ok())
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
        let Some(text) = text else {
            let mut content = Self::default();
            content
                .problems
                .push(format!("{} could not be read.", gameinfo.display()));
            return content;
        };
        match halberd_kv::parse_game_file(&text) {
            Ok(entries) => {
                Self::from_search_paths(&install.root, &install.game_dir, &search_paths(&entries))
            }
            Err(e) => {
                let mut content = Self::default();
                content.problems.push(format!(
                    "{} could not be understood: {e}",
                    gameinfo.display()
                ));
                content
            }
        }
    }

    /// Mounts `paths` (the values of `gameinfo.txt`'s `game` search paths,
    /// in order). `root` is the `GarrysMod` folder, `game_dir` the folder
    /// holding `gameinfo.txt`.
    pub fn from_search_paths(root: &Path, game_dir: &Path, paths: &[String]) -> Self {
        let mut content = Self::default();
        let mut seen = BTreeSet::new();
        for value in paths {
            let (places, cut_short) = expand(root, game_dir, value);
            if cut_short {
                content.problems.push(format!(
                    "\"{value}\" holds more than {MAX_EXPANDED} addons; the rest are not read."
                ));
            }
            for place in places {
                if seen.insert(place.clone()) {
                    content.add(place);
                }
            }
        }
        content
    }

    /// Mounts one place: an archive (`.vpk`, its `_dir.vpk` if that is how
    /// it is stored) or a folder.
    fn add(&mut self, place: PathBuf) {
        let is_vpk = place
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("vpk"))
            && !place.is_dir();
        if !is_vpk {
            if place.is_dir() {
                self.sources.push(Source::Folder(place));
            }
            // Folders that don't exist (download caches, say) are normal.
            return;
        }
        let dir_file = if place.is_file() {
            place.clone()
        } else {
            let stem = place
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            place.with_file_name(format!("{stem}_dir.vpk"))
        };
        if !dir_file.is_file() {
            return;
        }
        match Vpk::open(&dir_file) {
            Ok(vpk) => self.sources.push(Source::Archive(vpk)),
            Err(e) => self
                .problems
                .push(format!("{} could not be mounted: {e}.", dir_file.display())),
        }
    }

    /// Notes about places that could not be mounted, in plain words.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    /// How many archives are mounted.
    pub fn archive_count(&self) -> usize {
        self.sources
            .iter()
            .filter(|s| matches!(s, Source::Archive(_)))
            .count()
    }

    /// How many loose folders are mounted.
    pub fn folder_count(&self) -> usize {
        self.sources.len() - self.archive_count()
    }

    /// How many files the mounted archives list.
    pub fn archived_file_count(&self) -> usize {
        self.sources
            .iter()
            .map(|s| match s {
                Source::Archive(vpk) => vpk.len(),
                Source::Folder(_) => 0,
            })
            .sum()
    }

    /// A one-line summary for the Console.
    pub fn summary(&self) -> String {
        format!(
            "Game content mounted: {} files in {} archive{}, and {} folder{}.",
            group_digits(self.archived_file_count()),
            self.archive_count(),
            if self.archive_count() == 1 { "" } else { "s" },
            self.folder_count(),
            if self.folder_count() == 1 { "" } else { "s" },
        )
    }

    /// True if some mounted place has `path`.
    pub fn contains(&self, path: &str) -> bool {
        let Some(relative) = relative_path(path) else {
            return false;
        };
        let key = relative.to_string_lossy().replace('\\', "/");
        self.sources.iter().any(|source| match source {
            Source::Archive(vpk) => vpk.contains(&key),
            Source::Folder(folder) => find_in_folder(folder, &relative).is_some(),
        })
    }

    /// The bytes of `path` (`materials/brick/brickwall001a.vtf`, any case),
    /// from the first mounted place that has it.
    pub fn read(&self, path: &str) -> Result<Vec<u8>, ContentError> {
        self.read_at_most(path, MAX_LOOSE_FILE)
    }

    /// Like [`Self::read`], but a file bigger than `limit` bytes is refused
    /// before anything is read (for small text files such as materials).
    pub fn read_at_most(&self, path: &str, limit: u64) -> Result<Vec<u8>, ContentError> {
        let relative =
            relative_path(path).ok_or_else(|| ContentError::BadPath(path.to_string()))?;
        let key = relative.to_string_lossy().replace('\\', "/");
        // A copy that can't be read (damaged, checksum wrong) gives way to
        // the next place that has the file.
        let mut problem = None;
        for source in &self.sources {
            let found = match source {
                Source::Archive(vpk) => vpk.entry(&key).map(|entry| {
                    if entry.size() > limit {
                        return Err(TOO_LARGE.to_string());
                    }
                    vpk.read_entry(entry).map_err(|e| e.to_string())
                }),
                Source::Folder(folder) => {
                    find_in_folder(folder, &relative).map(|f| read_loose(&f, limit))
                }
            };
            match found {
                Some(Ok(bytes)) => return Ok(bytes),
                Some(Err(why)) => {
                    problem.get_or_insert(why);
                }
                None => {}
            }
        }
        Err(match problem {
            Some(why) => ContentError::Unreadable(path.to_string(), why),
            None => ContentError::NotFound(path.to_string()),
        })
    }

    /// Every loose file (in mounted folders) under the folder `prefix`
    /// whose name ends with `extension`, as paths inside the game, lower
    /// case; at most `limit`. Slow (it walks the folders): call it off the
    /// interface thread.
    pub fn loose_files(&self, prefix: &str, extension: &str, limit: usize) -> Vec<String> {
        let Some(relative) = relative_path(prefix) else {
            return Vec::new();
        };
        let extension = extension.to_ascii_lowercase();
        let mut found = BTreeSet::new();
        for source in &self.sources {
            let Source::Folder(folder) = source else {
                continue;
            };
            let Some(start) = find_dir_in_folder(folder, &relative) else {
                continue;
            };
            let mut stack = vec![start];
            while let Some(dir) = stack.pop() {
                for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                    if found.len() >= limit {
                        return found.into_iter().collect();
                    }
                    let path = entry.path();
                    match entry.file_type() {
                        Ok(t) if t.is_dir() => stack.push(path),
                        Ok(t) if t.is_file() => {
                            let inside = path.strip_prefix(folder).ok();
                            let name = inside.map(|p| {
                                p.to_string_lossy().replace('\\', "/").to_ascii_lowercase()
                            });
                            if let Some(name) = name.filter(|n| n.ends_with(&extension)) {
                                found.insert(name);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        found.into_iter().collect()
    }

    /// Every archived file whose path starts with `prefix` and ends with
    /// `extension` (both any case), once each, sorted. Loose files are not
    /// listed.
    pub fn archived_files(&self, prefix: &str, extension: &str) -> Vec<String> {
        let prefix = prefix.to_ascii_lowercase().replace('\\', "/");
        let extension = extension.to_ascii_lowercase();
        let mut found = BTreeSet::new();
        for source in &self.sources {
            if let Source::Archive(vpk) = source {
                for entry in vpk.entries() {
                    let path = entry.path.to_ascii_lowercase();
                    if path.starts_with(&prefix) && path.ends_with(&extension) {
                        found.insert(path);
                    }
                }
            }
        }
        found.into_iter().collect()
    }
}

/// The values of `game` search paths in a parsed `gameinfo.txt`, in order.
pub fn search_paths(entries: &[Entry]) -> Vec<String> {
    let find = |entries: &[Entry], name: &str| -> Option<Block> {
        entries.iter().find_map(|e| match e {
            Entry::Block(b) if b.name.eq_ignore_ascii_case(name) => Some(b.clone()),
            _ => None,
        })
    };
    let Some(game_info) = find(entries, "GameInfo") else {
        return Vec::new();
    };
    let Some(file_system) = find(&game_info.entries, "FileSystem") else {
        return Vec::new();
    };
    let Some(paths) = find(&file_system.entries, "SearchPaths") else {
        return Vec::new();
    };
    paths
        .entries
        .iter()
        .filter_map(|e| match e {
            Entry::Pair(key, value)
                if key.split('+').any(|part| part.eq_ignore_ascii_case("game")) =>
            {
                Some(value.clone())
            }
            _ => None,
        })
        .collect()
}

/// The places one search path names: its macros replaced, and a trailing
/// `*` (as for addons) expanded to every folder and VPK archive inside, as
/// the engine does. Also says whether a `*` held more than
/// [`MAX_EXPANDED`] and was cut short.
fn expand(root: &Path, game_dir: &Path, value: &str) -> (Vec<PathBuf>, bool) {
    let value = value.trim().replace('\\', "/");
    let (base, rest) = if let Some(rest) = strip_prefix_ci(&value, "|gameinfo_path|") {
        (game_dir.to_path_buf(), rest)
    } else if let Some(rest) = strip_prefix_ci(&value, "|all_source_engine_paths|") {
        (root.to_path_buf(), rest)
    } else {
        (root.to_path_buf(), value.as_str())
    };
    let every = rest.ends_with('*');
    let inner = rest.trim_end_matches('*').trim_end_matches('/');
    let inner = inner.trim_start_matches("./");
    // "." and "" are the base itself; anything else must stay inside it.
    let place = if inner.is_empty() || inner == "." {
        base
    } else {
        match relative_path(inner) {
            Some(relative) => base.join(relative),
            None => return (Vec::new(), false),
        }
    };
    if !every {
        return (vec![place], false);
    }
    let mut found = Vec::new();
    let mut cut_short = false;
    for entry in std::fs::read_dir(&place).into_iter().flatten().flatten() {
        if found.len() >= MAX_EXPANDED {
            cut_short = true;
            break;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
        let is_archive = !is_dir && name.ends_with(".vpk") && !is_numbered_archive(&name);
        if is_dir || is_archive {
            found.push(path);
        }
    }
    found.sort();
    (found, cut_short)
}

/// True for `name_000.vpk`-style numbered archives (read through their
/// `_dir.vpk`, never mounted on their own).
fn is_numbered_archive(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".vpk") else {
        return false;
    };
    stem.len() > 4
        && stem.as_bytes()[stem.len() - 4] == b'_'
        && stem[stem.len() - 3..].bytes().all(|b| b.is_ascii_digit())
}

fn strip_prefix_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

/// `path` as a relative path inside a folder, or `None` if it would leave
/// it (`..`, a drive, a leading slash) or names nothing.
fn relative_path(path: &str) -> Option<PathBuf> {
    let path = path.replace('\\', "/");
    let path = path.trim_start_matches("./");
    if path.is_empty() || path.starts_with('/') {
        return None;
    }
    let candidate = Path::new(path);
    let mut out = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// The file at `relative` inside `folder`, matching names without regard
/// to case (the engine ignores case; Linux file systems do not).
fn find_in_folder(folder: &Path, relative: &Path) -> Option<PathBuf> {
    let direct = folder.join(relative);
    if direct.is_file() {
        return Some(direct);
    }
    let mut at = folder.to_path_buf();
    for part in relative.components() {
        let wanted = part.as_os_str().to_str()?;
        let found = std::fs::read_dir(&at).ok()?.flatten().find(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(wanted))
        })?;
        at = found.path();
    }
    at.is_file().then_some(at)
}

/// The folder at `relative` inside `folder`, matching names without regard
/// to case.
fn find_dir_in_folder(folder: &Path, relative: &Path) -> Option<PathBuf> {
    let mut at = folder.to_path_buf();
    for part in relative.components() {
        let wanted = part.as_os_str().to_str()?;
        let direct = at.join(wanted);
        at = if direct.is_dir() {
            direct
        } else {
            std::fs::read_dir(&at)
                .ok()?
                .flatten()
                .find(|e| {
                    e.file_name()
                        .to_str()
                        .is_some_and(|n| n.eq_ignore_ascii_case(wanted))
                })?
                .path()
        };
    }
    at.is_dir().then_some(at)
}

/// Why a file over the size limit was not read.
const TOO_LARGE: &str = "it is larger than any real file of its kind";

fn read_loose(file: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(file).map_err(|e| e.to_string())?.len();
    if size > limit.min(MAX_LOOSE_FILE) {
        return Err(TOO_LARGE.into());
    }
    std::fs::read(file).map_err(|e| e.to_string())
}

/// 41203 as "41,203".
fn group_digits(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests;
