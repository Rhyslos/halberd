//! Text to tree.

use crate::{Block, Entry};
use std::fmt;

/// Deepest nesting accepted. Real files nest a handful of levels; this only
/// stops hostile files from exhausting the stack.
pub const MAX_DEPTH: usize = 256;

/// Why text could not be read, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvError {
    /// Line the problem was found on, counting from 1.
    pub line: usize,
    /// What is wrong, in plain words.
    pub message: String,
}

impl fmt::Display for KvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for KvError {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token<'a> {
    Text(&'a str),
    Open,
    Close,
}

struct Lexer<'a> {
    text: &'a str,
    pos: usize,
    line: usize,
}

impl<'a> Lexer<'a> {
    fn error(&self, message: impl Into<String>) -> KvError {
        KvError {
            line: self.line,
            message: message.into(),
        }
    }

    /// The next token, or `None` at the end of the text.
    fn next(&mut self) -> Result<Option<Token<'a>>, KvError> {
        let bytes = self.text.as_bytes();
        loop {
            match bytes.get(self.pos) {
                None => return Ok(None),
                Some(b'\n') => {
                    self.line += 1;
                    self.pos += 1;
                }
                Some(c) if c.is_ascii_whitespace() => self.pos += 1,
                Some(b'/') if bytes.get(self.pos + 1) == Some(&b'/') => {
                    while bytes.get(self.pos).is_some_and(|c| *c != b'\n') {
                        self.pos += 1;
                    }
                }
                Some(_) => break,
            }
        }
        let start = self.pos;
        match bytes[start] {
            b'{' => {
                self.pos += 1;
                Ok(Some(Token::Open))
            }
            b'}' => {
                self.pos += 1;
                Ok(Some(Token::Close))
            }
            b'"' => {
                let body = start + 1;
                let Some(len) = self.text[body..].find('"') else {
                    return Err(self.error("a quoted text is never closed"));
                };
                let end = body + len;
                self.line += self.text[body..end].matches('\n').count();
                self.pos = end + 1;
                Ok(Some(Token::Text(&self.text[body..end])))
            }
            _ => {
                let end = self.text[start..]
                    .find(|c: char| c.is_ascii_whitespace() || matches!(c, '{' | '}' | '"'))
                    .map_or(self.text.len(), |n| start + n);
                self.pos = end;
                Ok(Some(Token::Text(&self.text[start..end])))
            }
        }
    }
}

/// Reads KeyValues text into a list of top-level entries.
pub fn parse(text: &str) -> Result<Vec<Entry>, KvError> {
    // A byte-order mark at the start is not part of the text.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lexer = Lexer {
        text,
        pos: 0,
        line: 1,
    };
    // Each open block: its name, its entries, and the line it began on.
    let mut stack: Vec<(Block, usize)> = Vec::new();
    let mut top: Vec<Entry> = Vec::new();
    let mut pending: Option<&str> = None;
    while let Some(token) = lexer.next()? {
        match (token, pending.take()) {
            (Token::Open, Some(name)) => {
                if stack.len() >= MAX_DEPTH {
                    return Err(lexer.error("blocks are nested too deeply"));
                }
                stack.push((Block::new(name), lexer.line));
            }
            (Token::Open, None) => return Err(lexer.error("a block has no name before its '{'")),
            (Token::Close, Some(key)) => {
                return Err(lexer.error(format!("'{key}' has no value")));
            }
            (Token::Close, None) => {
                let Some((block, _)) = stack.pop() else {
                    return Err(lexer.error("there is a '}' without a matching '{'"));
                };
                match stack.last_mut() {
                    Some((parent, _)) => parent.entries.push(Entry::Block(block)),
                    None => top.push(Entry::Block(block)),
                }
            }
            (Token::Text(text), None) => pending = Some(text),
            (Token::Text(value), Some(key)) => {
                let pair = Entry::Pair(key.to_string(), value.to_string());
                match stack.last_mut() {
                    Some((parent, _)) => parent.entries.push(pair),
                    None => top.push(pair),
                }
            }
        }
    }
    if let Some(key) = pending {
        return Err(lexer.error(format!("'{key}' has no value")));
    }
    if let Some((block, line)) = stack.pop() {
        return Err(KvError {
            line,
            message: format!("the block '{}' is never closed", block.name),
        });
    }
    Ok(top)
}
