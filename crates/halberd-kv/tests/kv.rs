//! Reading and writing KeyValues text.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use halberd_kv::{Block, Entry, LineEnding, MAX_DEPTH, parse, write, write_with};

const HAMMER: &str = "versioninfo\r\n{\r\n\t\"editorversion\" \"400\"\r\n\t\"editorbuild\" \"8864\"\r\n}\r\nworld\r\n{\r\n\t\"id\" \"1\"\r\n\tsolid\r\n\t{\r\n\t\t\"id\" \"2\"\r\n\t\tside\r\n\t\t{\r\n\t\t\t\"plane\" \"(-64 64 64) (64 64 64) (64 -64 64)\"\r\n\t\t\t\"material\" \"DEV/DEV_MEASUREGENERIC01B\"\r\n\t\t}\r\n\t}\r\n}\r\n";

#[test]
fn a_hammer_file_reads_and_writes_back_byte_for_byte() {
    let entries = parse(HAMMER).unwrap();
    assert_eq!(entries.len(), 2);
    let Entry::Block(world) = &entries[1] else {
        panic!("world is a block")
    };
    assert_eq!(world.get("id"), Some("1"));
    let solid = world.blocks("solid").next().unwrap();
    let side = solid.blocks("side").next().unwrap();
    assert_eq!(
        side.get("plane"),
        Some("(-64 64 64) (64 64 64) (64 -64 64)")
    );
    assert_eq!(write(&entries), HAMMER);
    assert_eq!(LineEnding::of(HAMMER), LineEnding::Windows);
}

#[test]
fn unix_line_endings_round_trip_too() {
    let unix = HAMMER.replace("\r\n", "\n");
    let entries = parse(&unix).unwrap();
    assert_eq!(write_with(&entries, LineEnding::of(&unix)), unix);
}

#[test]
fn unquoted_tokens_comments_and_odd_spacing_are_read() {
    let text =
        "// a comment\nroot { key value \"spaced key\" \"a b\"\n  child{x 1}// trailing\n}\n";
    let entries = parse(text).unwrap();
    let Entry::Block(root) = &entries[0] else {
        panic!()
    };
    assert_eq!(root.get("key"), Some("value"));
    assert_eq!(root.get("spaced key"), Some("a b"));
    assert_eq!(root.blocks("child").next().unwrap().get("x"), Some("1"));
}

#[test]
fn repeated_keys_and_blocks_keep_their_order() {
    let text = "e\n{\n\"output\" \"a\"\n\"output\" \"b\"\nx\n{\n}\nx\n{\n}\n}\n";
    let entries = parse(text).unwrap();
    let Entry::Block(e) = &entries[0] else {
        panic!()
    };
    assert_eq!(e.entries.len(), 4);
    assert_eq!(e.entries[1], Entry::Pair("output".into(), "b".into()));
}

#[test]
fn backslashes_and_empty_values_are_kept_literally() {
    let text = "a\n{\n\t\"path\" \"C:\\maps\\new\"\n\t\"empty\" \"\"\n}\n";
    let entries = parse(text).unwrap();
    assert_eq!(write_with(&entries, LineEnding::Unix), text);
}

#[test]
fn a_byte_order_mark_is_ignored() {
    let entries = parse("\u{feff}a { b c }").unwrap();
    assert_eq!(entries.len(), 1);
}

#[test]
fn malformed_text_gives_errors_with_line_numbers() {
    let cases = [
        ("a\n{\n\"b\" \"c\"\n", 2, "never closed"),
        ("a { b }", 1, "has no value"),
        ("}", 1, "without a matching"),
        ("{ }", 1, "no name"),
        ("a { \"b\" \"unfinished\n\n }", 1, "never closed"),
        ("a { b c }\nlonely", 2, "has no value"),
    ];
    for (text, line, words) in cases {
        let err = parse(text).unwrap_err();
        assert_eq!(err.line, line, "{text:?}: {err}");
        assert!(err.message.contains(words), "{text:?}: {err}");
        assert!(err.to_string().starts_with(&format!("line {line}:")));
    }
}

#[test]
fn nesting_too_deep_is_refused_not_a_crash() {
    let deep = "a {".repeat(MAX_DEPTH + 1) + &"}".repeat(MAX_DEPTH + 1);
    assert!(parse(&deep).unwrap_err().message.contains("too deeply"));
    let fine = "a {".repeat(MAX_DEPTH) + &"}".repeat(MAX_DEPTH);
    assert!(parse(&fine).is_ok());
}

#[test]
fn quotes_inside_values_are_replaced_so_the_text_reads_back() {
    let mut b = Block::new("odd name");
    b.push_pair("say", "he said \"hi\"");
    let text = write(&[Entry::Block(b)]);
    let back = parse(&text).unwrap();
    let Entry::Block(b) = &back[0] else { panic!() };
    assert_eq!(b.name, "odd name");
    assert_eq!(b.get("say"), Some("he said 'hi'"));
}

/// A small random generator, so the fuzz tests are repeatable.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

fn random_text(rng: &mut Rng) -> String {
    const PIECES: [&str; 12] = [
        "{", "}", "\"", "a", " ", "\n", "//", "\"x y\"", "key", "\u{e9}", "\r\n", "\t",
    ];
    (0..rng.below(80))
        .map(|_| PIECES[rng.below(PIECES.len() as u64) as usize])
        .collect()
}

fn random_tree(rng: &mut Rng, depth: usize) -> Vec<Entry> {
    (0..rng.below(5))
        .map(|i| {
            if depth < 4 && rng.below(3) == 0 {
                Entry::Block(Block {
                    name: format!("block{i}"),
                    entries: random_tree(rng, depth + 1),
                })
            } else {
                let value = ["", "1", "a b", "C:\\x", "(0 0 0)", "\u{e6}\u{f8}\u{e5}"]
                    [rng.below(6) as usize];
                Entry::Pair(format!("k{}", rng.below(3)), value.to_string())
            }
        })
        .collect()
}

#[test]
fn fuzz_random_text_never_panics() {
    let mut rng = Rng(0xC0FF_EE00_1234_5678);
    for _ in 0..20_000 {
        let text = random_text(&mut rng);
        let _ = parse(&text);
    }
}

#[test]
fn fuzz_random_trees_round_trip_exactly() {
    let mut rng = Rng(0x0BAD_5EED_0000_0007);
    for _ in 0..5_000 {
        let tree = random_tree(&mut rng, 0);
        for ending in [LineEnding::Windows, LineEnding::Unix] {
            let text = write_with(&tree, ending);
            assert_eq!(parse(&text).unwrap(), tree, "{text}");
        }
    }
}
