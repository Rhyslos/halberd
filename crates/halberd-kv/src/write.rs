//! Tree to text.

use crate::{Block, Entry};

/// How lines end in written text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineEnding {
    /// `\r\n`, as Hammer writes on Windows.
    #[default]
    Windows,
    /// `\n`.
    Unix,
}

impl LineEnding {
    /// The line ending `text` mostly uses (Windows if it has any `\r\n`).
    pub fn of(text: &str) -> Self {
        if text.contains("\r\n") {
            Self::Windows
        } else {
            Self::Unix
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Windows => "\r\n",
            Self::Unix => "\n",
        }
    }
}

/// Writes entries as KeyValues text in Hammer's layout, with Windows line
/// endings.
pub fn write(entries: &[Entry]) -> String {
    write_with(entries, LineEnding::Windows)
}

/// Writes entries as KeyValues text in Hammer's layout:
///
/// ```text
/// name
/// {
///     "key" "value"
/// }
/// ```
///
/// A double quote inside a key or value cannot be written (the format has
/// no way to escape it), so it becomes a single quote.
pub fn write_with(entries: &[Entry], ending: LineEnding) -> String {
    let mut out = String::new();
    for entry in entries {
        write_entry(&mut out, entry, 0, ending.as_str());
    }
    out
}

fn write_entry(out: &mut String, entry: &Entry, depth: usize, nl: &str) {
    let indent = |out: &mut String| out.extend(std::iter::repeat_n('\t', depth));
    match entry {
        Entry::Pair(key, value) => {
            indent(out);
            out.push('"');
            out.push_str(&clean(key));
            out.push_str("\" \"");
            out.push_str(&clean(value));
            out.push('"');
            out.push_str(nl);
        }
        Entry::Block(Block { name, entries }) => {
            indent(out);
            if is_bare(name) {
                out.push_str(name);
            } else {
                out.push('"');
                out.push_str(&clean(name));
                out.push('"');
            }
            out.push_str(nl);
            indent(out);
            out.push('{');
            out.push_str(nl);
            for child in entries {
                write_entry(out, child, depth + 1, nl);
            }
            indent(out);
            out.push('}');
            out.push_str(nl);
        }
    }
}

/// True if a block name can be written without quotes and read back the
/// same.
fn is_bare(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with("//")
        && name
            .chars()
            .all(|c| !c.is_ascii_whitespace() && !matches!(c, '{' | '}' | '"'))
}

fn clean(text: &str) -> std::borrow::Cow<'_, str> {
    if text.contains('"') {
        text.replace('"', "'").into()
    } else {
        text.into()
    }
}
