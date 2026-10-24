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

/// A delay as Hammer writes it: whole seconds plainly, else up to two
/// decimals.
fn format_delay(delay: f32) -> String {
    let delay = if delay.is_finite() {
        delay.max(0.0)
    } else {
        0.0
    };
    if (delay - delay.round()).abs() < 0.0005 {
        format!("{}", delay.round() as i64)
    } else {
        let text = format!("{delay:.2}");
        text.trim_end_matches('0').to_string()
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

    /// The same entity with `connections` in place of those it had. Lines
    /// that couldn't be read are kept, after the new ones.
    pub fn with_connections(&self, connections: &[Connection]) -> Self {
        let mut kept = Vec::new();
        let mut at = None;
        let mut file_data = Vec::with_capacity(self.file_data.len() + 1);
        for entry in &self.file_data {
            match entry {
                Entry::Block(b) if b.name.eq_ignore_ascii_case("connections") => {
                    at.get_or_insert(file_data.len());
                    kept.extend(
                        b.entries
                            .iter()
                            .filter(|e| match e {
                                Entry::Pair(o, v) => Connection::parse(o, v).is_none(),
                                Entry::Block(_) => true,
                            })
                            .cloned(),
                    );
                }
                other => file_data.push(other.clone()),
            }
        }
        let mut block = Block::new("connections");
        for c in connections.iter().take(MAX_CONNECTIONS) {
            block.push_pair(c.output.clone(), c.value());
        }
        block.entries.extend(kept);
        if !block.entries.is_empty() {
            // Where the old block was, else after the settings.
            let at = at.unwrap_or_else(|| {
                file_data
                    .iter()
                    .position(|e| matches!(e, Entry::Block(_)))
                    .unwrap_or(file_data.len())
            });
            file_data.insert(at, Entry::Block(block));
        }
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

/// True if an output's `target` text means the entity named `name`:
/// names compare without regard to case, and a target ending in `*`
/// matches every name starting with what comes before it, as in the game.
pub fn target_matches(target: &str, name: &str) -> bool {
    let target = target.trim();
    match target.strip_suffix('*') {
        Some(prefix) => name
            .to_ascii_lowercase()
            .starts_with(&prefix.to_ascii_lowercase()),
        None => target.eq_ignore_ascii_case(name),
    }
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

    #[test]
    fn targets_match_names_and_wildcards() {
        assert!(target_matches("Door1", "door1"));
        assert!(target_matches("door*", "door_left"));
        assert!(!target_matches("door*", "lamp"));
        assert!(!target_matches("door1", "door10"));
        assert!(is_special_target("!player"));
        assert!(!is_special_target("player"));
    }

    #[test]
    fn delays_read_as_hammer_writes_them() {
        assert_eq!(format_delay(2.0), "2");
        assert_eq!(format_delay(0.5), "0.5");
        assert_eq!(format_delay(-3.0), "0");
        assert_eq!(format_delay(f32::NAN), "0");
    }

    #[test]
    fn fuzz_odd_connection_lines_never_crash() {
        let alphabet = b"ab,01.-\x1b!*x ";
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
                assert_eq!(
                    Connection::parse("OnX", &c.value()).map(|d| d.target),
                    Some(c.target)
                );
            }
        }
    }
}
