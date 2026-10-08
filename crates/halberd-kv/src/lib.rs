//! Read and write Valve's KeyValues text format.
//!
//! KeyValues is the nested `"key" "value"` text format behind VMF maps, VMT
//! materials and Steam's VDF files:
//!
//! ```text
//! versioninfo
//! {
//!     "editorversion" "400"
//! }
//! ```
//!
//! [`parse`] turns text into a plain tree of [`Entry`] values, keeping the
//! order of everything and any repeated keys or blocks. [`write()`] turns the
//! tree back into text in the layout Hammer uses (tabs, one item a line), so
//! an untouched Hammer file comes back the same.
//!
//! Text is taken literally: backslashes are not escapes (Hammer writes
//! Windows paths with single backslashes). Comments (`// …`) are skipped.
//! Malformed input gives a [`KvError`] with the line it was found on; it
//! never panics.

mod parse;
mod tree;
mod write;

pub use parse::{KvError, MAX_DEPTH, parse};
pub use tree::{Block, Entry};
pub use write::{LineEnding, write, write_with};
