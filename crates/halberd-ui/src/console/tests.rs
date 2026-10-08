//! Tests for the Console's messages and filters.

use super::*;
use std::cell::RefCell;
use std::rc::Rc;

fn entry(level: LogLevel, message: &str) -> LogEntry {
    LogEntry {
        level,
        time: "12:00:00".into(),
        message: message.into(),
        repeats: 0,
    }
}

fn messages(console: &Console) -> Vec<&str> {
    console.visible().map(|e| e.message.as_str()).collect()
}

#[test]
fn new_messages_get_the_local_time() {
    let e = LogEntry::warning("hello");
    assert_eq!(e.level, LogLevel::Warning);
    assert_eq!(e.time.len(), 8, "HH:MM:SS, got {}", e.time);
    assert_eq!(e.time.as_bytes()[2], b':');
    assert_eq!(e.time.as_bytes()[5], b':');
}

#[test]
fn a_repeated_message_is_kept_once_with_a_count() {
    let mut console = Console::new();
    console.push(entry(LogLevel::Warning, "surface lost"));
    let mut later = entry(LogLevel::Warning, "surface lost");
    later.time = "12:00:05".into();
    console.push(later);
    console.push(entry(LogLevel::Warning, "surface lost"));
    console.push(entry(LogLevel::Error, "surface lost"));
    let all: Vec<_> = console.entries().collect();
    assert_eq!(all.len(), 2, "a different level is a different message");
    assert_eq!(all[0].repeats, 2);
    assert_eq!(all[0].time, "12:00:00", "time of the last repeat");
    assert!(all[0].to_text().ends_with("surface lost (×3)"));
}

#[test]
fn the_oldest_messages_go_when_full() {
    let mut console = Console::new();
    for i in 0..MAX_CONSOLE_ENTRIES + 10 {
        console.push(entry(LogLevel::Info, &format!("line {i}")));
    }
    assert_eq!(console.entries().count(), MAX_CONSOLE_ENTRIES);
    assert_eq!(console.dropped(), 10);
    assert_eq!(console.entries().next().unwrap().message, "line 10");
}

#[test]
fn levels_can_be_hidden_and_messages_searched() {
    let mut console = Console::new();
    console.push(entry(LogLevel::Info, "Opened map.vmf"));
    console.push(entry(LogLevel::Warning, "2 brushes not shown"));
    console.push(entry(LogLevel::Error, "Could not save MAP.vmf"));
    assert_eq!(console.count(LogLevel::Warning), 1);
    console.set_shown(LogLevel::Info, false);
    assert_eq!(
        messages(&console),
        ["2 brushes not shown", "Could not save MAP.vmf"]
    );
    console.set_shown(LogLevel::Info, true);
    console.set_search("  map.VMF ");
    assert_eq!(
        messages(&console),
        ["Opened map.vmf", "Could not save MAP.vmf"],
        "search ignores letter case and spaces around it"
    );
    console.set_search("");
    assert_eq!(messages(&console).len(), 3);
}

#[test]
fn copied_text_has_time_level_and_message_per_line() {
    let mut console = Console::new();
    console.push(entry(LogLevel::Info, "Opened map.vmf"));
    console.push(entry(LogLevel::Error, "first line\nsecond line"));
    console.set_shown(LogLevel::Info, false);
    assert_eq!(
        console.visible_text(),
        "12:00:00 error   first line\n                 second line\n"
    );
}

#[test]
fn problems_count_until_the_console_is_seen() {
    let mut console = Console::new();
    assert_eq!(console.tab_title(), ("Console".to_string(), None));
    console.push(entry(LogLevel::Info, "fine"));
    console.push(entry(LogLevel::Warning, "hmm"));
    assert_eq!(
        console.tab_title(),
        ("Console ⚠ 1".to_string(), Some(LogLevel::Warning))
    );
    console.push(entry(LogLevel::Error, "bad"));
    console.push(entry(LogLevel::Error, "bad"));
    assert_eq!(
        console.tab_title(),
        ("Console ❌ 3".to_string(), Some(LogLevel::Error)),
        "repeats count too"
    );
    console.mark_seen();
    assert_eq!(console.unseen_problems(), (0, 0));
}

#[test]
fn clear_empties_everything_shown() {
    let mut console = Console::new();
    console.push(entry(LogLevel::Error, "bad"));
    console.clear();
    assert_eq!(console.entries().count(), 0);
    assert_eq!(console.unseen_problems(), (0, 0));
}

#[test]
fn the_mirror_gets_old_and_new_messages_but_not_logged_ones() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut console = Console::new();
    console.push(entry(LogLevel::Info, "before"));
    let sink = Rc::clone(&seen);
    console.set_mirror(Box::new(move |e: &LogEntry| {
        sink.borrow_mut().push(e.message.clone());
    }));
    console.push(entry(LogLevel::Warning, "after"));
    console.push_logged(entry(LogLevel::Error, "from the logger"));
    assert_eq!(*seen.borrow(), ["before", "after"]);
    assert_eq!(console.entries().count(), 3);
}
