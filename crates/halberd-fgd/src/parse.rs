//! Reads the classes of one FGD file. A class that can't be read is
//! reported and skipped; the rest of the file is still read.

use crate::lex::{Spanned, Token, tokens};
use crate::model::{Choice, ClassKind, EntityClass, Helper, IoDef, KeyDef, KeyType};

/// Most keyvalues, inputs and outputs in one class.
const MAX_ITEMS: usize = 4096;
/// Most choices or flags of one keyvalue.
const MAX_CHOICES: usize = 4096;

/// What one FGD file holds.
#[derive(Debug, Default)]
pub(crate) struct FileContents {
    pub classes: Vec<EntityClass>,
    /// Files named by `@include`, in order.
    pub includes: Vec<String>,
    pub map_size: Option<(i64, i64)>,
    /// Problems: the line and what was wrong.
    pub problems: Vec<(usize, String)>,
}

/// A failed read: the line and what was wrong.
type Failed = (usize, String);

struct Parser {
    tokens: Vec<Spanned>,
    at: usize,
}

/// Reads the classes, includes and map size of one FGD file's text.
pub(crate) fn parse_file(text: &str) -> FileContents {
    let mut p = Parser {
        tokens: tokens(text),
        at: 0,
    };
    let mut out = FileContents::default();
    while let Some(next) = p.peek().cloned() {
        match next {
            Token::At(word) => {
                p.at += 1;
                if let Err(problem) = p.top_level(&word, &mut out) {
                    out.problems.push(problem);
                    p.skip_to_next_at();
                }
            }
            _ => {
                out.problems.push((
                    p.line(),
                    "text outside any class was skipped (it should start with @)".into(),
                ));
                p.skip_to_next_at();
            }
        }
    }
    out
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at).map(|t| &t.token)
    }

    fn line(&self) -> usize {
        self.tokens
            .get(self.at)
            .or(self.tokens.last())
            .map_or(1, |t| t.line)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.at).map(|t| t.token.clone());
        self.at += 1;
        token
    }

    fn fail<T>(&self, what: impl Into<String>) -> Result<T, Failed> {
        Err((self.line(), what.into()))
    }

    fn is_punct(&self, c: char) -> bool {
        self.peek() == Some(&Token::Punct(c))
    }

    fn eat(&mut self, c: char) -> bool {
        let found = self.is_punct(c);
        if found {
            self.at += 1;
        }
        found
    }

    fn expect(&mut self, c: char) -> Result<(), Failed> {
        if self.eat(c) {
            Ok(())
        } else {
            self.fail(format!("expected \"{c}\""))
        }
    }

    fn skip_to_next_at(&mut self) {
        while let Some(token) = self.peek() {
            if matches!(token, Token::At(_)) {
                return;
            }
            self.at += 1;
        }
    }

    /// Skips a `[ … ]` block (nested ones too) if one is next.
    fn skip_block(&mut self) {
        if !self.eat('[') {
            return;
        }
        let mut depth = 1usize;
        while depth > 0 {
            match self.next() {
                Some(Token::Punct('[')) => depth += 1,
                Some(Token::Punct(']')) => depth -= 1,
                Some(_) => {}
                None => return,
            }
        }
    }

    fn top_level(&mut self, word: &str, out: &mut FileContents) -> Result<(), Failed> {
        match word.to_ascii_lowercase().as_str() {
            "include" => match self.next() {
                Some(Token::Text(file)) => {
                    out.includes.push(file);
                    Ok(())
                }
                _ => self.fail("@include needs a file name in quotes"),
            },
            "mapsize" => {
                out.map_size = self.map_size();
                Ok(())
            }
            "baseclass" | "pointclass" | "solidclass" | "npcclass" | "keyframeclass"
            | "moveclass" | "filterclass" | "extendclass" => {
                let kind = match word.to_ascii_lowercase().as_str() {
                    "baseclass" | "extendclass" => ClassKind::Base,
                    "solidclass" => ClassKind::Solid,
                    _ => ClassKind::Point,
                };
                out.classes.push(self.class(kind)?);
                Ok(())
            }
            // Editor-only lists other editors use; not needed here.
            _ => {
                while !self.is_punct('[') && !matches!(self.peek(), Some(Token::At(_)) | None) {
                    self.at += 1;
                }
                self.skip_block();
                Ok(())
            }
        }
    }

    /// `@mapsize(-16384, 16384)`.
    fn map_size(&mut self) -> Option<(i64, i64)> {
        let args = self.helper_args().ok()?;
        let low = args.first()?.parse().ok()?;
        let high = args.get(1)?.parse().ok()?;
        Some((low, high))
    }

    /// `helpers… = name : "description" [ items ]`.
    fn class(&mut self, kind: ClassKind) -> Result<EntityClass, Failed> {
        let mut bases = Vec::new();
        let mut helpers = Vec::new();
        while !self.eat('=') {
            let Some(Token::Word(name)) = self.next() else {
                return self.fail("expected \"=\" and the class name");
            };
            let args = if self.is_punct('(') {
                self.helper_args()?
            } else {
                Vec::new()
            };
            let name = name.to_ascii_lowercase();
            if name == "base" {
                bases.extend(args);
            } else {
                helpers.push(Helper { name, args });
            }
        }
        let Some(Token::Word(name)) = self.next() else {
            return self.fail("expected the class name after \"=\"");
        };
        let description = if self.eat(':') {
            self.text()?
        } else {
            String::new()
        };
        let mut class = EntityClass {
            name,
            kind,
            description,
            bases,
            helpers,
            keys: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        };
        self.expect('[')?;
        while !self.eat(']') {
            if self.peek().is_none() {
                return self.fail(format!(
                    "the class \"{}\" never ends (no \"]\")",
                    class.name
                ));
            }
            if class.keys.len() + class.inputs.len() + class.outputs.len() >= MAX_ITEMS {
                return self.fail(format!("the class \"{}\" is too long", class.name));
            }
            self.item(&mut class)?;
        }
        Ok(class)
    }

    /// `( a, b c, "d" )`: the arguments, trimmed, quotes taken off.
    fn helper_args(&mut self) -> Result<Vec<String>, Failed> {
        self.expect('(')?;
        let mut args = Vec::new();
        let mut current: Vec<String> = Vec::new();
        loop {
            match self.next() {
                Some(Token::Punct(')')) => break,
                Some(Token::Punct(',')) => args.push(std::mem::take(&mut current).join(" ")),
                Some(Token::Word(w) | Token::Text(w)) => current.push(w),
                Some(Token::Punct(c)) => current.push(c.to_string()),
                Some(Token::At(_)) | None => return self.fail("expected \")\""),
            }
        }
        if !current.is_empty() || !args.is_empty() {
            args.push(current.join(" "));
        }
        Ok(args)
    }

    /// A quoted string, with `"a" + "b"` joined.
    fn text(&mut self) -> Result<String, Failed> {
        let Some(Token::Text(mut value)) = self.next() else {
            return self.fail("expected text in quotes");
        };
        while self.eat('+') {
            match self.next() {
                Some(Token::Text(more)) => value.push_str(&more),
                _ => return self.fail("expected text in quotes after \"+\""),
            }
        }
        Ok(value)
    }

    /// One keyvalue, input or output.
    fn item(&mut self, class: &mut EntityClass) -> Result<(), Failed> {
        let Some(Token::Word(first)) = self.next() else {
            return self.fail("expected a keyvalue, input or output");
        };
        let lower = first.to_ascii_lowercase();
        if (lower == "input" || lower == "output") && !self.is_punct('(') {
            let io = self.io()?;
            if lower == "input" {
                class.inputs.push(io);
            } else {
                class.outputs.push(io);
            }
            return Ok(());
        }
        let key = self.key(first)?;
        class.keys.push(key);
        Ok(())
    }

    /// `Name(type) : "description"`.
    fn io(&mut self) -> Result<IoDef, Failed> {
        let Some(Token::Word(name)) = self.next() else {
            return self.fail("expected the name of an input or output");
        };
        self.expect('(')?;
        let param = match self.next() {
            Some(Token::Word(w)) => w,
            _ => return self.fail("expected the type of an input or output"),
        };
        self.expect(')')?;
        let description = if self.eat(':') {
            self.text()?
        } else {
            String::new()
        };
        Ok(IoDef {
            name,
            param: param.to_ascii_lowercase(),
            description,
        })
    }

    /// `name(type) [readonly] [report] : "Label" : default : "description"
    /// [= [ choices ]]`; every part after the type may be left out.
    fn key(&mut self, name: String) -> Result<KeyDef, Failed> {
        self.expect('(')?;
        let kind = match self.next() {
            Some(Token::Word(w)) => KeyType::from_name(&w),
            _ => return self.fail(format!("expected the type of \"{name}\"")),
        };
        self.expect(')')?;
        let mut read_only = false;
        while let Some(Token::Word(w)) = self.peek() {
            match w.to_ascii_lowercase().as_str() {
                "readonly" => read_only = true,
                "report" => {}
                _ => break,
            }
            self.at += 1;
        }
        let mut key = KeyDef {
            label: name.clone(),
            name,
            kind,
            default: None,
            description: String::new(),
            choices: Vec::new(),
            read_only,
        };
        if self.eat(':') {
            if matches!(self.peek(), Some(Token::Text(_))) {
                key.label = self.text()?;
            }
            if self.eat(':') {
                key.default = self.default_value();
                if self.eat(':') && matches!(self.peek(), Some(Token::Text(_))) {
                    key.description = self.text()?;
                }
            }
        }
        if self.eat('=') {
            key.choices = self.choices()?;
        }
        Ok(key)
    }

    /// A default value: a number, a word or text; nothing before the next
    /// `:`, `=` or the next item.
    fn default_value(&mut self) -> Option<String> {
        match self.peek() {
            Some(Token::Text(_)) => self.text().ok(),
            Some(Token::Word(w)) => {
                let value = w.clone();
                self.at += 1;
                Some(value)
            }
            _ => None,
        }
    }

    /// `[ value : "label" (: on) … ]`.
    fn choices(&mut self) -> Result<Vec<Choice>, Failed> {
        self.expect('[')?;
        let mut choices = Vec::new();
        while !self.eat(']') {
            if choices.len() >= MAX_CHOICES {
                return self.fail("too many choices");
            }
            let value = match self.next() {
                Some(Token::Word(w) | Token::Text(w)) => w,
                _ => return self.fail("expected a choice's value"),
            };
            self.expect(':')?;
            let label = self.text()?;
            let on_by_default = if self.eat(':') {
                matches!(self.next(), Some(Token::Word(w)) if w == "1")
            } else {
                false
            };
            choices.push(Choice {
                value,
                label,
                on_by_default,
            });
        }
        Ok(choices)
    }
}
