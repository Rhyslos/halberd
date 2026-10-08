//! The parsed tree.

/// One item in a KeyValues file: a key with a value, or a named block of
/// further items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// `"key" "value"`
    Pair(String, String),
    /// `name { … }`
    Block(Block),
}

/// A named block: `name { … }`. Order and repeated names are kept.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Block {
    /// The block's name, such as `solid` or `side`.
    pub name: String,
    /// What the block holds, in order.
    pub entries: Vec<Entry>,
}

impl Block {
    /// An empty block called `name`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entries: Vec::new(),
        }
    }

    /// The value of the first pair called `key` (names compare without
    /// regard to case, as Valve's tools do).
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.iter().find_map(|e| match e {
            Entry::Pair(k, v) if k.eq_ignore_ascii_case(key) => Some(v.as_str()),
            _ => None,
        })
    }

    /// Every block inside this one called `name` (any case), in order.
    pub fn blocks<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Block> + 'a {
        self.entries.iter().filter_map(move |e| match e {
            Entry::Block(b) if b.name.eq_ignore_ascii_case(name) => Some(b),
            _ => None,
        })
    }

    /// Adds a pair at the end.
    pub fn push_pair(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.entries.push(Entry::Pair(key.into(), value.into()));
    }

    /// Adds a block at the end.
    pub fn push_block(&mut self, block: Block) {
        self.entries.push(Entry::Block(block));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookups_ignore_case_and_keep_order() {
        let mut b = Block::new("entity");
        b.push_pair("classname", "light");
        b.push_pair("ClassName", "second");
        b.push_block(Block::new("connections"));
        b.push_block(Block::new("Connections"));
        assert_eq!(b.get("CLASSNAME"), Some("light"));
        assert_eq!(b.get("missing"), None);
        assert_eq!(b.blocks("connections").count(), 2);
    }
}
