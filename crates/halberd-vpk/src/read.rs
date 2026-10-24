//! Reading a VPK directory and the files it lists.
//!
//! Layout: a header (signature `0x55AA1234`, version, size of the file
//! list; version 2 adds four section sizes), then the file list as a tree
//! of zero-terminated strings: extension, then folder, then name, each
//! name followed by its entry (checksum, preload size, archive number,
//! offset, length, `0xFFFF`) and its preload bytes. A folder or extension
//! of a single space means none. Archive number `0x7FFF` means the bytes
//! follow the file list in the directory file itself.

use crate::crc::crc32;
use crate::error::VpkError;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const SIGNATURE: u32 = 0x55AA_1234;
/// Archive number meaning "in the directory file".
const IN_DIRECTORY: u16 = 0x7FFF;
/// Largest file list read: real ones are a few megabytes.
pub const MAX_TREE_SIZE: u64 = 256 * 1024 * 1024;
/// Most files one archive may list.
pub const MAX_ENTRIES: usize = 2_000_000;
/// Largest single file read from an archive.
pub const MAX_FILE_SIZE: u64 = 1024 * 1024 * 1024;

/// One file listed in an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VpkEntry {
    /// Its path inside the archive, as listed (`materials/brick/wall.vtf`).
    pub path: String,
    /// The CRC-32 of its whole contents.
    pub crc: u32,
    /// Its first bytes, kept in the directory.
    preload: Vec<u8>,
    archive: u16,
    offset: u32,
    length: u32,
}

impl VpkEntry {
    /// Its size in bytes.
    pub fn size(&self) -> u64 {
        self.preload.len() as u64 + u64::from(self.length)
    }
}

/// An open archive: its file list, read from the directory file.
#[derive(Debug, Clone)]
pub struct Vpk {
    dir_path: PathBuf,
    /// Where the bytes kept in the directory file start.
    data_start: u64,
    entries: Vec<VpkEntry>,
    /// Lower-case path to entry.
    index: HashMap<String, usize>,
}

impl Vpk {
    /// Reads the file list of the archive whose directory file is `path`
    /// (`name_dir.vpk`, or a single-file `name.vpk`). File contents are
    /// not read.
    pub fn open(path: &Path) -> Result<Self, VpkError> {
        let mut file = File::open(path)?;
        let file_len = file.metadata()?.len();
        let mut head = [0u8; 28];
        let got = read_up_to(&mut file, &mut head)?;
        let word = |i: usize| -> Option<u32> {
            (i + 4 <= got)
                .then(|| u32::from_le_bytes([head[i], head[i + 1], head[i + 2], head[i + 3]]))
        };
        if word(0) != Some(SIGNATURE) {
            return Err(VpkError::NotVpk);
        }
        let version = word(4).ok_or(VpkError::Damaged("the header is cut short"))?;
        let header_len: u64 = match version {
            1 => 12,
            2 => 28,
            v => return Err(VpkError::UnsupportedVersion(v)),
        };
        if (got as u64) < header_len {
            return Err(VpkError::Damaged("the header is cut short"));
        }
        let tree_len = u64::from(word(8).ok_or(VpkError::Damaged("the header is cut short"))?);
        if tree_len > MAX_TREE_SIZE {
            return Err(VpkError::TooLarge("its file list"));
        }
        if header_len + tree_len > file_len {
            return Err(VpkError::Damaged(
                "the file list runs past the end of the file",
            ));
        }
        file.seek(SeekFrom::Start(header_len))?;
        let mut tree = vec![0u8; tree_len as usize];
        file.read_exact(&mut tree)?;
        let entries = parse_tree(&tree)?;
        let mut index = HashMap::with_capacity(entries.len());
        for (i, entry) in entries.iter().enumerate() {
            // The first listing of a path wins, as in the engine.
            index.entry(entry.path.to_ascii_lowercase()).or_insert(i);
        }
        Ok(Self {
            dir_path: path.to_path_buf(),
            data_start: header_len + tree_len,
            entries,
            index,
        })
    }

    /// How many files it lists.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if it lists no files.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every file it lists, in archive order.
    pub fn entries(&self) -> &[VpkEntry] {
        &self.entries
    }

    /// The listing of `path` (any case, `/` or `\`), if it is in the
    /// archive.
    pub fn entry(&self, path: &str) -> Option<&VpkEntry> {
        let key = normalize(path);
        self.index.get(&key).map(|&i| &self.entries[i])
    }

    /// True if `path` is in the archive.
    pub fn contains(&self, path: &str) -> bool {
        self.entry(path).is_some()
    }

    /// The bytes of `path`, read from the archive and checked against its
    /// checksum.
    pub fn read(&self, path: &str) -> Result<Vec<u8>, VpkError> {
        let entry = self
            .entry(path)
            .ok_or_else(|| VpkError::NotFound(path.to_string()))?;
        self.read_entry(entry)
    }

    /// The bytes of one listed file, checked against its checksum.
    pub fn read_entry(&self, entry: &VpkEntry) -> Result<Vec<u8>, VpkError> {
        if entry.size() > MAX_FILE_SIZE {
            return Err(VpkError::TooLarge("a file in it"));
        }
        let mut data = Vec::with_capacity(entry.size() as usize);
        data.extend_from_slice(&entry.preload);
        if entry.length > 0 {
            let (archive_path, base) = if entry.archive == IN_DIRECTORY {
                (self.dir_path.clone(), self.data_start)
            } else {
                (self.archive_path(entry.archive)?, 0)
            };
            let mut file = File::open(&archive_path)?;
            let start = base + u64::from(entry.offset);
            let end = start + u64::from(entry.length);
            if end > file.metadata()?.len() {
                return Err(VpkError::Damaged("a file runs past the end of its archive"));
            }
            file.seek(SeekFrom::Start(start))?;
            let mut rest = vec![0u8; entry.length as usize];
            file.read_exact(&mut rest)?;
            data.extend_from_slice(&rest);
        }
        if crc32(&data) != entry.crc {
            return Err(VpkError::ChecksumMismatch(entry.path.clone()));
        }
        Ok(data)
    }

    /// The numbered archive `number` beside the directory file.
    fn archive_path(&self, number: u16) -> Result<PathBuf, VpkError> {
        let name = self
            .dir_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(VpkError::Damaged("its name is not readable"))?;
        let stem = name
            .strip_suffix("_dir.vpk")
            .or_else(|| name.strip_suffix("_DIR.VPK"))
            .ok_or(VpkError::Damaged(
                "a file is in a numbered archive, but this is not a _dir.vpk",
            ))?;
        Ok(self
            .dir_path
            .with_file_name(format!("{stem}_{number:03}.vpk")))
    }
}

/// `path` as the index keys it: lower case, forward slashes, no leading
/// slash.
fn normalize(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches('/')
        .to_ascii_lowercase()
}

/// Reads as much of `buf` as the file has.
fn read_up_to(file: &mut File, buf: &mut [u8]) -> Result<usize, VpkError> {
    let mut got = 0;
    while got < buf.len() {
        match file.read(&mut buf[got..])? {
            0 => break,
            n => got += n,
        }
    }
    Ok(got)
}

/// Reading through the file list.
struct Cursor<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn string(&mut self) -> Result<&'a str, VpkError> {
        let rest = self
            .data
            .get(self.at..)
            .ok_or(VpkError::Damaged("the file list is cut short"))?;
        let end = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or(VpkError::Damaged("a name in the file list never ends"))?;
        self.at += end + 1;
        std::str::from_utf8(&rest[..end]).map_err(|_| VpkError::Damaged("a name is not valid text"))
    }

    fn bytes(&mut self, n: usize) -> Result<&'a [u8], VpkError> {
        let out = self
            .data
            .get(self.at..self.at + n)
            .ok_or(VpkError::Damaged("the file list is cut short"))?;
        self.at += n;
        Ok(out)
    }

    fn u16(&mut self) -> Result<u16, VpkError> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, VpkError> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

/// The files listed in a file list.
fn parse_tree(tree: &[u8]) -> Result<Vec<VpkEntry>, VpkError> {
    let mut cursor = Cursor { data: tree, at: 0 };
    let mut entries = Vec::new();
    loop {
        let extension = cursor.string()?;
        if extension.is_empty() {
            break;
        }
        loop {
            let folder = cursor.string()?;
            if folder.is_empty() {
                break;
            }
            loop {
                let name = cursor.string()?;
                if name.is_empty() {
                    break;
                }
                if entries.len() >= MAX_ENTRIES {
                    return Err(VpkError::TooLarge("its number of files"));
                }
                let crc = cursor.u32()?;
                let preload_len = cursor.u16()?;
                let archive = cursor.u16()?;
                let offset = cursor.u32()?;
                let length = cursor.u32()?;
                if cursor.u16()? != 0xFFFF {
                    return Err(VpkError::Damaged("an entry does not end where it should"));
                }
                let preload = cursor.bytes(usize::from(preload_len))?.to_vec();
                entries.push(VpkEntry {
                    path: join(folder, name, extension),
                    crc,
                    preload,
                    archive,
                    offset,
                    length,
                });
            }
        }
    }
    Ok(entries)
}

/// A file's full path from its folder, name and extension (" " for none).
fn join(folder: &str, name: &str, extension: &str) -> String {
    let mut path = String::new();
    if folder != " " {
        path.push_str(folder);
        path.push('/');
    }
    path.push_str(name);
    if extension != " " {
        path.push('.');
        path.push_str(extension);
    }
    path
}
