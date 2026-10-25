//! Splits FGD text into tokens: `@words`, names and numbers, quoted
//! strings and punctuation. `//` comments are skipped.

/// One piece of FGD text.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token {
    /// `@PointClass`, `@include` (without the `@`).
    At(String),
    /// A name or number: `light`, `-16`, `0.5`, `spawnflags`.
    Word(String),
    /// A quoted string, without its quotes.
    Text(String),
    /// One of `( ) [ ] : = , +`.
    Punct(char),
}

/// A token and the line it starts on (from 1).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Spanned {
    pub token: Token,
    pub line: usize,
}

/// Most tokens read from one file (a full GMod FGD set has about 200,000).
pub(crate) const MAX_TOKENS: usize = 2_000_000;

/// The tokens of `text`, at most [`MAX_TOKENS`]. Characters that can't
/// start a token are skipped.
pub(crate) fn tokens(text: &str) -> Vec<Spanned> {
    let mut out = Vec::new();
    let mut chars = text.char_indices().peekable();
    let mut line = 1;
    while let Some((at, c)) = chars.next() {
        if out.len() >= MAX_TOKENS {
            break;
        }
        match c {
            '\n' => line += 1,
            c if c.is_whitespace() => {}
            '/' if text[at..].starts_with("//") => {
                // To the end of the line.
                while chars.peek().is_some_and(|(_, c)| *c != '\n') {
                    chars.next();
                }
            }
            '"' => {
                let start = line;
                let mut value = String::new();
                for (_, c) in chars.by_ref() {
                    match c {
                        '"' => break,
                        '\n' => {
                            // An unclosed string ends at the line's end.
                            line += 1;
                            break;
                        }
                        c => value.push(c),
                    }
                }
                out.push(Spanned {
                    token: Token::Text(value),
                    line: start,
                });
            }
            '(' | ')' | '[' | ']' | ':' | '=' | ',' | '+' => out.push(Spanned {
                token: Token::Punct(c),
                line,
            }),
            '@' => {
                let word = take_word(&mut chars, None);
                out.push(Spanned {
                    token: Token::At(word),
                    line,
                });
            }
            c if is_word_char(c) => {
                let word = take_word(&mut chars, Some(c));
                out.push(Spanned {
                    token: Token::Word(word),
                    line,
                });
            }
            _ => {}
        }
    }
    out
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | '\\' | '$' | '*' | '#' | '!')
}

fn take_word(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    first: Option<char>,
) -> String {
    let mut word: String = first.into_iter().collect();
    while let Some((_, c)) = chars.peek() {
        if !is_word_char(*c) {
            break;
        }
        word.push(*c);
        chars.next();
    }
    word
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<Token> {
        tokens(text).into_iter().map(|t| t.token).collect()
    }

    #[test]
    fn words_strings_and_punctuation_are_split() {
        assert_eq!(
            kinds("@PointClass size(-16 -16 0, 16 16 72) = info_player_start : \"Spawn\" []"),
            vec![
                Token::At("PointClass".into()),
                Token::Word("size".into()),
                Token::Punct('('),
                Token::Word("-16".into()),
                Token::Word("-16".into()),
                Token::Word("0".into()),
                Token::Punct(','),
                Token::Word("16".into()),
                Token::Word("16".into()),
                Token::Word("72".into()),
                Token::Punct(')'),
                Token::Punct('='),
                Token::Word("info_player_start".into()),
                Token::Punct(':'),
                Token::Text("Spawn".into()),
                Token::Punct('['),
                Token::Punct(']'),
            ]
        );
    }

    #[test]
    fn comments_are_skipped_and_lines_counted() {
        let t = tokens("// a comment\nlight // more\n\"x\"");
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].line, 2);
        assert_eq!(t[1].line, 3);
    }

    #[test]
    fn an_unclosed_string_ends_at_the_line() {
        let t = tokens("\"open\nnext");
        assert_eq!(t[0].token, Token::Text("open".into()));
        assert_eq!(t[1].token, Token::Word("next".into()));
        assert_eq!(t[1].line, 2);
    }
}
