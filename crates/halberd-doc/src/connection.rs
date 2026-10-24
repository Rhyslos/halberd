//! Entity outputs connected to other entities' inputs: "when this button
//! is pressed (`OnPressed`), tell the door named `door1` to `Open`".
//!
//! A map file keeps them in each entity's `connections` block, one per
//! line: `"OnPressed" "door1,Open,,0,-1"`: the target's name, its input, a
//! parameter, a delay in seconds, and how many times it may fire (-1: no
//! limit). Newer Hammers separate the parts with the ESC character instead
//! of commas; both are read. Lines that can't be read are kept as they are.

use crate::EntityObject;
use halberd_kv::{Block, Entry};

/// The character newer Hammers separate a connection's parts with.
const ESC: char = '\u{1b}';
/// Most connections read from one entity.
pub const MAX_CONNECTIONS: usize = 1024;
/// Longest text in any part of a connection.
const MAX_PART: usize = 256;

/// One output connected to an input.
#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    /// The output that fires (`OnTrigger`).
    pub output: String,
    /// The name of the entity it tells (`door1`, or `!player`, `!self`…).
    pub target: String,
    /// The input it calls on the target (`Open`).
    pub input: String,
    /// The value passed with it, if any.
    pub parameter: String,
    /// Seconds to wait first.
    pub delay: f32,
    /// How many times it may fire; -1 for no limit (1 is "only once").
    pub times: i32,
}

impl Connection {
    /// A connection that fires every time, without a delay or parameter.
    pub fn new(output: &str, target: &str, input: &str) -> Self {
        Self {
            output: output.to_string(),
            target: target.to_string(),
            input: input.to_string(),
            parameter: String::new(),
            delay: 0.0,
            times: -1,
        }
    }

    /// Reads a `connections` line: `output` and `target,input,param,delay,times`
    /// (or ESC between the parts). `None` if it can't be read.
    pub fn parse(output: &str, value: &str) -> Option<Self> {
        let separator = if value.contains(ESC) { ESC } else { ',' };
        let parts: Vec<&str> = value.split(separator).collect();
        if parts.len() != 5 || output.trim().is_empty() || output.len() > MAX_PART {
            return None;
        }
        if parts.iter().any(|p| p.len() > MAX_PART) {
            return None;
        }
        let delay: f32 = parts[3]
            .trim()
            .parse()
            .ok()
            .filter(|d: &f32| d.is_finite())?;
        let times: i32 = parts[4].trim().parse().ok()?;
        Some(Self {
            output: output.trim().to_string(),
            target: parts[0].trim().to_string(),
            input: parts[1].trim().to_string(),
            parameter: parts[2].to_string(),
            delay,
            times,
        })
    }

    /// The line's value as a map file writes it: commas, unless a part
    /// holds a comma (then ESC, as newer Hammers write).
    pub fn value(&self) -> String {
        let delay = format_delay(self.delay);
        let parts = [
            self.target.as_str(),
            self.input.as_str(),
            self.parameter.as_str(),
            delay.as_str(),
            &self.times.to_string(),
        ]
        .map(str::to_string);
        let separator = if parts.iter().any(|p| p.contains(',')) {
            ESC.to_string()
        } else {
            ",".to_string()
        };
        parts.join(&separator)
    }

    /// True if it fires only once.
    pub fn only_once(&self) -> bool {
        self.times == 1
    }
}

/// A delay as text: the shortest form that reads back as the same
/// number ("2", "0.5", "0.001"), so delays used to order outputs keep their
/// precision.
fn format_delay(delay: f32) -> String {
    if delay.is_finite() {
        format!("{delay}")
    } else {
        "0".into()
    }
}

impl EntityObject {
    /// Its connections that can be read, in order.
    pub fn connections(&self) -> Vec<Connection> {
        self.file_data
            .iter()
            .filter_map(|e| match e {
                Entry::Block(b) if b.name.eq_ignore_ascii_case("connections") => Some(b),
                _ => None,
            })
            .flat_map(|b| b.entries.iter())
            .filter_map(|e| match e {
                Entry::Pair(output, value) => Connection::parse(output, value),
                Entry::Block(_) => None,
            })
            .take(MAX_CONNECTIONS)
            .collect()
    }

    /// The same entity with `connections` in place of those
    /// [`Self::connections`] gave. Lines that didn't change are kept
    /// exactly as they were (their spacing, separators and place); lines
    /// that can't be read, and readable lines past [`MAX_CONNECTIONS`], are
    /// kept where they are; new lines go where they belong in the order,
    /// or at the end of the last `connections` block.
    pub fn with_connections(&self, connections: &[Connection]) -> Self {
        let mut wanted = connections.iter().take(MAX_CONNECTIONS).peekable();
        let olds = self.connections();
        let mut readable = 0usize;
        let mut file_data = Vec::with_capacity(self.file_data.len() + 1);
        let mut last_block = None;
        for entry in &self.file_data {
            let Entry::Block(block) = entry else {
                file_data.push(entry.clone());
                continue;
            };
            if !block.name.eq_ignore_ascii_case("connections") {
                file_data.push(entry.clone());
                continue;
            }
            let mut out = Block::new(block.name.clone());
            for line in &block.entries {
                let old = match line {
                    Entry::Pair(o, v) if readable < MAX_CONNECTIONS => Connection::parse(o, v),
                    _ => None,
                };
                let Some(old) = old else {
                    out.entries.push(line.clone());
                    continue;
                };
                readable += 1;
                let rest: Vec<&Connection> = wanted.clone().collect();
                if !rest.contains(&&old) {
                    // Changed or removed. A wanted line that isn't one of
                    // the old lines still to come takes its place.
                    let later = olds.get(readable..).unwrap_or(&[]);
                    if let Some(next) = wanted.next_if(|n| !later.contains(n)) {
                        out.push_pair(next.output.clone(), next.value());
                    }
                    continue;
                }
                // Write the wanted lines that come before this one.
                for next in wanted.by_ref() {
                    if *next == old {
                        out.entries.push(line.clone());
                        break;
                    }
                    out.push_pair(next.output.clone(), next.value());
                }
            }
            last_block = Some(file_data.len());
            file_data.push(Entry::Block(out));
        }
        let left: Vec<&Connection> = wanted.collect();
        if !left.is_empty() {
            let at = match last_block {
                Some(at) => at,
                None => {
                    // After the settings, before any other block.
                    let at = file_data
                        .iter()
                        .position(|e| matches!(e, Entry::Block(_)))
                        .unwrap_or(file_data.len());
                    file_data.insert(at, Entry::Block(Block::new("connections")));
                    at
                }
            };
            if let Some(Entry::Block(block)) = file_data.get_mut(at) {
                for c in left {
                    block.push_pair(c.output.clone(), c.value());
                }
            }
        }
        // A block left empty by this edit goes (one that was empty stays).
        file_data.retain(|e| match e {
            Entry::Block(b)
                if b.name.eq_ignore_ascii_case("connections") && b.entries.is_empty() =>
            {
                self.file_data.contains(e)
            }
            _ => true,
        });
        Self {
            file_data,
            ..self.clone()
        }
    }

    /// The name others call it by (`targetname`), if it has one.
    pub fn name(&self) -> Option<&str> {
        self.setting("targetname").filter(|n| !n.trim().is_empty())
    }
}

/// True if an output's `target` text means the entity named `name`, as
/// the game decides: letters compare without regard to case, and a `*`
/// matches everything from there on (so `door*` and `do*r` both match
/// every name starting with "do").
pub fn target_matches(target: &str, name: &str) -> bool {
    let target = target.trim();
    let mut name_chars = name.chars();
    for t in target.chars() {
        if t == '*' {
            return true;
        }
        match name_chars.next() {
            Some(n) if n.eq_ignore_ascii_case(&t) => {}
            _ => return false,
        }
    }
    name_chars.next().is_none()
}

/// True for the game's special targets (`!player`, `!self`, `!activator`,
/// `!caller`, `!picker`…), which name no entity in the map.
pub fn is_special_target(target: &str) -> bool {
    target.trim_start().starts_with('!')
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn lines_are_read_with_commas_or_esc() {
        let c = Connection::parse("OnPressed", "door1,Open,,0,-1").unwrap();
        assert_eq!(c, Connection::new("OnPressed", "door1", "Open"));
        let esc = Connection::parse("OnTrigger", "relay\u{1b}FireUser1\u{1b}a,b\u{1b}1.5\u{1b}1")
            .unwrap();
        assert_eq!(esc.parameter, "a,b");
        assert_eq!(esc.delay, 1.5);
        assert!(esc.only_once());
        assert_eq!(esc.value(), "relay\u{1b}FireUser1\u{1b}a,b\u{1b}1.5\u{1b}1");
        assert_eq!(c.value(), "door1,Open,,0,-1");
    }

    #[test]
    fn broken_lines_are_not_read() {
        for value in [
            "door1,Open",
            "door1,Open,,x,-1",
            "a,b,c,0,1,extra",
            "a,b,c,NaN,1",
            "",
        ] {
            assert!(Connection::parse("OnX", value).is_none(), "{value}");
        }
        assert!(Connection::parse("", "a,b,,0,-1").is_none());
    }

    #[test]
    fn connections_are_replaced_keeping_unreadable_lines() {
        let mut e = EntityObject::point("func_button", Vec3::ZERO, &[("targetname", "b1")]);
        let mut block = Block::new("connections");
        block.push_pair("OnPressed", "door1,Open,,0,-1");
        block.push_pair("OnBroken", "not a connection");
        e.file_data.push(Entry::Block(block));
        assert_eq!(e.connections().len(), 1);
        let mut new = Connection::new("OnPressed", "light1", "TurnOn");
        new.delay = 0.25;
        let changed = e.with_connections(&[new.clone()]);
        assert_eq!(changed.connections(), [new]);
        let Some(Entry::Block(b)) = changed.file_data.last() else {
            panic!("the block is kept in place");
        };
        assert_eq!(b.entries.len(), 2, "the unreadable line stays");
        assert_eq!(b.get("OnPressed"), Some("light1,TurnOn,,0.25,-1"));
        // No connections and nothing kept: no block at all.
        let plain = EntityObject::point("info_target", Vec3::ZERO, &[]);
        assert_eq!(plain.with_connections(&[]), plain);
    }

    /// An entity whose connections block holds `lines` (output, value).
    fn with_lines(lines: &[(&str, &str)]) -> EntityObject {
        let mut e = EntityObject::point("logic_relay", Vec3::ZERO, &[]);
        let mut block = Block::new("Connections");
        for (o, v) in lines {
            block.push_pair(*o, *v);
        }
        e.file_data.push(Entry::Block(block));
        e
    }

    #[test]
    fn untouched_lines_come_back_exactly() {
        // Regression: an edit rewrote every line (rounding delays, turning
        // ESC into commas, moving unreadable lines, renaming the block).
        let e = with_lines(&[
            ("OnTrigger", "a,Open,,0.001,-1"),
            ("OnTrigger", "b\u{1b}Close\u{1b}x,y\u{1b}0.125\u{1b}1"),
            ("Junk", "not readable"),
            ("OnTrigger", " c , Toggle,,2.996,-1"),
        ]);
        assert_eq!(e.with_connections(&e.connections()), e);
        // Changing the second keeps the others as they were.
        let mut changed = e.connections();
        changed[1].delay = 5.0;
        let after = e.with_connections(&changed);
        let Some(Entry::Block(b)) = after.file_data.last() else {
            panic!("block kept");
        };
        assert_eq!(b.name, "Connections");
        assert_eq!(
            b.entries[0],
            Entry::Pair("OnTrigger".into(), "a,Open,,0.001,-1".into())
        );
        assert_eq!(
            b.entries[2],
            Entry::Pair("Junk".into(), "not readable".into())
        );
        assert_eq!(b.entries.len(), 4);
        // Removing the first and adding one at the end.
        let mut fewer = e.connections();
        fewer.remove(0);
        fewer.push(Connection::new("OnUser1", "d", "Kill"));
        let after = e.with_connections(&fewer);
        assert_eq!(after.connections(), fewer);
    }

    #[test]
    fn lines_past_the_limit_are_kept_on_any_edit() {
        // Regression: readable lines past the limit were deleted.
        let lines: Vec<(String, String)> = (0..MAX_CONNECTIONS + 9)
            .map(|i| ("OnTrigger".to_string(), format!("t{i},Kill,,0,-1")))
            .collect();
        let pairs: Vec<(&str, &str)> = lines
            .iter()
            .map(|(o, v)| (o.as_str(), v.as_str()))
            .collect();
        let e = with_lines(&pairs);
        let mut all = e.connections();
        assert_eq!(all.len(), MAX_CONNECTIONS);
        all[0].delay = 1.0;
        let after = e.with_connections(&all);
        let Some(Entry::Block(b)) = after.file_data.last() else {
            panic!("block kept");
        };
        assert_eq!(b.entries.len(), MAX_CONNECTIONS + 9);
    }

    #[test]
    fn targets_match_names_and_wildcards() {
        assert!(target_matches("Door1", "door1"));
        assert!(target_matches("door*", "door_left"));
        assert!(!target_matches("door*", "lamp"));
        assert!(!target_matches("door1", "door10"));
        // As in the game, a star anywhere ends the comparison.
        assert!(target_matches("do*r", "dog"));
        assert!(target_matches("*", "anything"));
        assert!(!target_matches("door", "doo"));
        assert!(is_special_target("!player"));
        assert!(!is_special_target("player"));
    }

    #[test]
    fn delays_read_as_hammer_writes_them() {
        assert_eq!(format_delay(2.0), "2");
        assert_eq!(format_delay(0.5), "0.5");
        assert_eq!(format_delay(-3.0), "-3");
        assert_eq!(format_delay(0.001), "0.001");
        assert_eq!(format_delay(0.125), "0.125");
        assert_eq!(format_delay(2.996), "2.996");
        assert_eq!(format_delay(f32::NAN), "0");
    }

    #[test]
    fn fuzz_odd_connection_lines_never_crash() {
        let alphabet = b"ab,01.-\x1b!*x e9";
        let mut seed: u64 = 0xC0;
        for _ in 0..5000 {
            let text: String = (0..20)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    alphabet[(seed % alphabet.len() as u64) as usize] as char
                })
                .collect();
            if let Some(c) = Connection::parse("OnX", &text) {
                // Whatever is read is written so it reads back the same.
                assert_eq!(
                    Connection::parse("OnX", &c.value()),
                    Some(c.clone()),
                    "{text:?}"
                );
                let e = with_lines(&[("OnX", text.as_str())]);
                assert_eq!(e.with_connections(&e.connections()), e);
            }
        }
    }
}
