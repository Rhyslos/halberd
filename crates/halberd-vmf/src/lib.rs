//! Read and write Hammer's VMF map format.
//!
//! A VMF is KeyValues text (see `halberd-kv`): a `world` block holding the
//! map's brushes (`solid` blocks of `side` blocks), `entity` blocks, and
//! Hammer's own blocks (`versioninfo`, `visgroups`, `viewsettings`,
//! `cameras`, `cordons`…).
//!
//! This crate stays close to the text on purpose: a [`Vmf`] is the parsed
//! tree, with helpers to find the world, entities, solids and sides and to
//! read their planes, origins and ids. Anything it does not interpret is
//! simply kept, so writing a map back loses nothing.

mod error;
mod numbers;

pub use error::VmfError;
pub use halberd_kv::{Block, Entry, LineEnding};
pub use numbers::{format_number, format_plane, format_vec3, parse_plane, parse_vec3};

/// A whole VMF file: its top-level entries in order, and its line endings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vmf {
    /// Top-level blocks (and any stray pairs) in file order.
    pub entries: Vec<Entry>,
    /// How the file's lines end, so it is written back the same way.
    pub line_ending: LineEnding,
}

impl Vmf {
    /// Reads VMF text. Fails if the text is not valid KeyValues or has no
    /// `world` block.
    pub fn parse(text: &str) -> Result<Self, VmfError> {
        let entries = halberd_kv::parse(text).map_err(VmfError::Syntax)?;
        let vmf = Self {
            entries,
            line_ending: LineEnding::of(text),
        };
        if vmf.world().is_none() {
            return Err(VmfError::NoWorld);
        }
        Ok(vmf)
    }

    /// Writes the map as text, in Hammer's layout.
    pub fn to_text(&self) -> String {
        halberd_kv::write_with(&self.entries, self.line_ending)
    }

    /// The `world` block: world keyvalues and world brushes.
    pub fn world(&self) -> Option<&Block> {
        self.top_blocks("world").next()
    }

    /// The `world` block, for changing it.
    pub fn world_mut(&mut self) -> Option<&mut Block> {
        self.entries.iter_mut().find_map(|e| match e {
            Entry::Block(b) if b.name.eq_ignore_ascii_case("world") => Some(b),
            _ => None,
        })
    }

    /// Every top-level `entity` block, in order.
    pub fn entities(&self) -> impl Iterator<Item = &Block> {
        self.top_blocks("entity")
    }

    /// Top-level blocks called `name` (any case).
    pub fn top_blocks<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Block> + 'a {
        self.entries.iter().filter_map(move |e| match e {
            Entry::Block(b) if b.name.eq_ignore_ascii_case(name) => Some(b),
            _ => None,
        })
    }
}

/// The `id` of a solid, side or entity, if it has a valid one.
pub fn id_of(block: &Block) -> Option<u64> {
    block.get("id")?.trim().parse().ok()
}

#[cfg(test)]
mod tests;
