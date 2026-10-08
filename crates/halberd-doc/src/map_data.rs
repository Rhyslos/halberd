//! The parts of a map file the editor keeps but does not use yet.

use halberd_kv::{Entry, LineEnding};

/// What a map file held besides its brushes and entities: Hammer's own
/// blocks (`versioninfo`, `visgroups`, `viewsettings`, `cameras`,
/// `cordons`…), the world's settings (sky, detail sprites…), and the file's
/// line endings. Kept exactly as read, so saving loses nothing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MapFileData {
    /// Top-level blocks before the world, in order.
    pub header: Vec<Entry>,
    /// The world's entries before its first brush (its settings).
    pub world_header: Vec<Entry>,
    /// The world's other entries after its first brush (hidden brushes,
    /// groups…).
    pub world_trailer: Vec<Entry>,
    /// Top-level blocks after the world that are not entities.
    pub footer: Vec<Entry>,
    /// How the file's lines ended.
    pub line_ending: LineEnding,
    /// How the file's text was encoded.
    pub encoding: TextEncoding,
}

/// How a map file's text is encoded. Hammer writes plain bytes; files with
/// accented letters from older tools are often Latin-1 rather than UTF-8,
/// and are written back the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextEncoding {
    /// UTF-8 (which plain English text also is).
    #[default]
    Utf8,
    /// UTF-8 starting with a byte-order mark (three bytes some Windows
    /// editors put at the start), which is written back.
    Utf8WithMark,
    /// Latin-1: every byte is one character.
    Latin1,
}
