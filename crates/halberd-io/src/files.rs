//! Reading and writing map files on disk.

use crate::{Imported, IoError, document_from_vmf, vmf_from_document};
use halberd_doc::{Document, TextEncoding};
use halberd_vmf::Vmf;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Largest map file Halberd opens: 512 MB. Real VMFs are rarely over 50 MB;
/// this only stops a wrong or hostile file from using all memory.
pub const MAX_MAP_FILE_BYTES: u64 = 512 * 1_000_000;
/// Extension of the backup kept when a map is saved over: Hammer's own.
pub const BACKUP_EXTENSION: &str = "vmx";

/// A map read from disk.
#[derive(Debug)]
pub struct OpenedMap {
    /// The map.
    pub document: Document,
    /// Plain-language notes about anything that could not be shown.
    pub notes: Vec<String>,
}

/// Reads a VMF file.
pub fn open_map(path: &Path) -> Result<OpenedMap, IoError> {
    let size = fs::metadata(path)
        .map_err(|e| IoError::Read(path.to_path_buf(), e))?
        .len();
    if size > MAX_MAP_FILE_BYTES {
        return Err(IoError::TooLarge(path.to_path_buf(), size));
    }
    let bytes = fs::read(path).map_err(|e| IoError::Read(path.to_path_buf(), e))?;
    let (text, encoding) = decode(bytes);
    open_map_text(&text, encoding).map_err(|e| match e {
        IoError::NotAMap(_, inner) => IoError::NotAMap(path.to_path_buf(), inner),
        other => other,
    })
}

/// Reads VMF text that was already loaded (for tests and for maps that do
/// not come from a file).
pub fn open_map_text(text: &str, encoding: TextEncoding) -> Result<OpenedMap, IoError> {
    let vmf = Vmf::parse(text).map_err(|e| IoError::NotAMap(PathBuf::new(), e))?;
    let Imported {
        mut document,
        notes,
    } = document_from_vmf(&vmf).map_err(|_| IoError::TooManyObjects)?;
    document.set_encoding(encoding);
    Ok(OpenedMap { document, notes })
}

/// Saves the map as a VMF at `path`, safely: the text goes to a temporary
/// file next to it first; an existing file is kept as a `.vmx` backup (as
/// Hammer does); then the new file takes its place. If anything fails,
/// the original file is untouched. Marks the document as saved.
///
/// A backup that cannot be made (a locked or read-only `.vmx`) does not
/// stop the save; the returned notes say so, for the Console. Saving a
/// `.vmx` file itself makes no backup, since it would be the same file.
pub fn save_map(doc: &mut Document, path: &Path) -> Result<Vec<String>, IoError> {
    let text = vmf_from_document(doc).to_text();
    let bytes = encode(&text, doc.file_data().encoding);
    let failed = |e| IoError::Write(path.to_path_buf(), e);
    let temp = sibling(path, ".halberd-saving");
    {
        let mut file = fs::File::create(&temp).map_err(failed)?;
        let written = file.write_all(&bytes).and_then(|()| file.sync_all());
        if let Err(e) = written {
            drop(file);
            let _ = fs::remove_file(&temp);
            return Err(failed(e));
        }
    }
    let mut notes = Vec::new();
    let backup = path.with_extension(BACKUP_EXTENSION);
    if path.exists()
        && !same_file_name(path, &backup)
        && let Err(e) = fs::copy(path, &backup)
    {
        notes.push(format!(
            "No backup was made ({} could not be written: {e}).",
            backup.display()
        ));
    }
    if let Err(e) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(failed(e));
    }
    doc.mark_saved();
    Ok(notes)
}

/// True if `a` and `b` name the same file, ignoring letter case (as
/// Windows does), so `map.VMX` and `map.vmx` count as one.
fn same_file_name(a: &Path, b: &Path) -> bool {
    a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
}

/// A file next to `path` whose name ends with `suffix`.
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

/// The UTF-8 byte-order mark.
const UTF8_MARK: &[u8] = b"\xEF\xBB\xBF";

/// UTF-8 if the bytes are valid UTF-8, otherwise Latin-1 (one character per
/// byte), so no byte is ever lost or changed.
fn decode(bytes: Vec<u8>) -> (String, TextEncoding) {
    if let Some(rest) = bytes.strip_prefix(UTF8_MARK)
        && let Ok(text) = std::str::from_utf8(rest)
    {
        return (text.to_string(), TextEncoding::Utf8WithMark);
    }
    match String::from_utf8(bytes) {
        Ok(text) => (text, TextEncoding::Utf8),
        Err(e) => (
            e.into_bytes().iter().map(|b| char::from(*b)).collect(),
            TextEncoding::Latin1,
        ),
    }
}

fn encode(text: &str, encoding: TextEncoding) -> Vec<u8> {
    match encoding {
        TextEncoding::Utf8 => text.as_bytes().to_vec(),
        TextEncoding::Utf8WithMark => [UTF8_MARK, text.as_bytes()].concat(),
        // Characters beyond Latin-1 (typed in Halberd) cannot be written in
        // Latin-1; they become '?', as Windows tools do.
        TextEncoding::Latin1 => text
            .chars()
            .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
            .collect(),
    }
}
