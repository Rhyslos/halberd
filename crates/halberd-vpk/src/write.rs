//! Writing small VPK archives, for tests and tools.

use crate::crc::crc32;
use crate::error::VpkError;
use std::collections::BTreeMap;
use std::path::Path;

/// Files grouped as a VPK file list stores them: extension, then folder,
/// then each name with its contents.
type Tree<'a> = BTreeMap<String, BTreeMap<String, Vec<(String, &'a Vec<u8>)>>>;

/// Collects files and writes them as a VPK archive.
#[derive(Debug, Clone, Default)]
pub struct VpkBuilder {
    /// Path (forward slashes) to contents.
    files: BTreeMap<String, Vec<u8>>,
    version: u32,
    preload: u16,
}

impl VpkBuilder {
    /// An empty version 2 archive with no preloaded bytes.
    pub fn new() -> Self {
        Self {
            files: BTreeMap::new(),
            version: 2,
            preload: 0,
        }
    }

    /// Writes the older version 1 header instead.
    #[must_use]
    pub fn version_1(mut self) -> Self {
        self.version = 1;
        self
    }

    /// Keeps up to `bytes` of each file in the directory (as Valve's tools
    /// do for small files).
    #[must_use]
    pub fn preload(mut self, bytes: u16) -> Self {
        self.preload = bytes;
        self
    }

    /// Adds a file (`materials/brick/wall.vtf`).
    pub fn add(&mut self, path: &str, data: &[u8]) -> &mut Self {
        self.files.insert(path.replace('\\', "/"), data.to_vec());
        self
    }

    /// Writes everything into one file at `path`, the bytes following the
    /// file list.
    pub fn write_single(&self, path: &Path) -> Result<(), VpkError> {
        let (tree, data) = self.build(true);
        std::fs::write(path, self.assemble(&tree, &data))?;
        Ok(())
    }

    /// Writes a directory file `dir_path` (which must end in `_dir.vpk`)
    /// and, beside it, archive `_000.vpk` holding the bytes.
    pub fn write_split(&self, dir_path: &Path) -> Result<(), VpkError> {
        let name = dir_path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix("_dir.vpk"))
            .ok_or(VpkError::Damaged(
                "the directory file's name must end in _dir.vpk",
            ))?;
        let (tree, data) = self.build(false);
        std::fs::write(dir_path, self.assemble(&tree, &[]))?;
        std::fs::write(dir_path.with_file_name(format!("{name}_000.vpk")), data)?;
        Ok(())
    }

    fn assemble(&self, tree: &[u8], data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&0x55AA_1234u32.to_le_bytes());
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&(tree.len() as u32).to_le_bytes());
        if self.version == 2 {
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&[0; 12]);
        }
        out.extend_from_slice(tree);
        out.extend_from_slice(data);
        out
    }

    /// The file list and the bytes it points into.
    fn build(&self, in_directory: bool) -> (Vec<u8>, Vec<u8>) {
        let mut tree: Tree<'_> = BTreeMap::new();
        for (path, data) in &self.files {
            let (folder, file) = match path.rsplit_once('/') {
                Some((folder, file)) => (folder.to_string(), file),
                None => (" ".to_string(), path.as_str()),
            };
            let (name, extension) = match file.rsplit_once('.') {
                Some((name, extension)) => (name.to_string(), extension.to_string()),
                None => (file.to_string(), " ".to_string()),
            };
            tree.entry(extension)
                .or_default()
                .entry(folder)
                .or_default()
                .push((name, data));
        }
        let mut list = Vec::new();
        let mut bytes = Vec::new();
        let string = |list: &mut Vec<u8>, s: &str| {
            list.extend_from_slice(s.as_bytes());
            list.push(0);
        };
        for (extension, folders) in &tree {
            string(&mut list, extension);
            for (folder, files) in folders {
                string(&mut list, folder);
                for (name, data) in files {
                    string(&mut list, name);
                    let preload = data.len().min(usize::from(self.preload));
                    let rest = &data[preload..];
                    list.extend_from_slice(&crc32(data).to_le_bytes());
                    list.extend_from_slice(&(preload as u16).to_le_bytes());
                    let archive: u16 = if in_directory { 0x7FFF } else { 0 };
                    list.extend_from_slice(&archive.to_le_bytes());
                    list.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                    list.extend_from_slice(&(rest.len() as u32).to_le_bytes());
                    list.extend_from_slice(&0xFFFFu16.to_le_bytes());
                    list.extend_from_slice(&data[..preload]);
                    bytes.extend_from_slice(rest);
                }
                list.push(0);
            }
            list.push(0);
        }
        list.push(0);
        (list, bytes)
    }
}
