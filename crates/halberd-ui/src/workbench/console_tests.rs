//! Tests for the Console panel, run through the real interface.

use super::tests::{harness_for, info};
use super::*;
use crate::{
    CONSOLE_CLEAR_BUTTON, CONSOLE_COPY_BUTTON, CONSOLE_SEARCH_NAME, LogEntry, LogLevel,
    MAX_CONSOLE_ENTRIES,
};
use egui_kittest::kittest::Queryable;

fn workbench_with_messages() -> Workbench {
    let mut wb = Workbench::new(info(), None);
    wb.push_console("Opened castle.vmf: 400 brushes, 3 entities.");
    wb.push_warning("2 brushes could not be shown.");
    wb.push_entry(LogEntry::error("castle.vmf could not be saved: disk full"));
    wb
}

fn copied(harness: &egui_kittest::Harness<'_, Workbench>) -> Option<String> {
    harness
        .output()
        .platform_output
        .commands
        .iter()
        .find_map(|c| match c {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
}

#[test]
fn messages_show_with_filter_buttons_and_counts() {
    let mut h = harness_for(workbench_with_messages());
    h.run();
    assert!(h.query_by_label_contains("400 brushes").is_some());
    assert!(h.query_by_label("⚠ Warnings 1").is_some());
    assert!(h.query_by_label("❌ Errors 1").is_some());
    // Hiding warnings hides the warning only.
    h.get_by_label("⚠ Warnings 1").click();
    h.run();
    assert!(h.query_by_label_contains("could not be shown").is_none());
    assert!(h.query_by_label_contains("disk full").is_some());
    assert!(!h.state().console().is_shown(LogLevel::Warning));
}

#[test]
fn searching_narrows_the_messages() {
    let mut h = harness_for(workbench_with_messages());
    h.run();
    h.get_by_label(CONSOLE_SEARCH_NAME).click();
    h.run();
    h.get_by_label(CONSOLE_SEARCH_NAME).type_text("BRUSHES");
    h.run();
    assert!(h.query_by_label_contains("400 brushes").is_some());
    assert!(h.query_by_label_contains("could not be shown").is_some());
    assert!(h.query_by_label_contains("disk full").is_none());
}

#[test]
fn copy_puts_the_shown_messages_on_the_clipboard() {
    let mut h = harness_for(workbench_with_messages());
    h.run();
    h.get_by_label("ℹ Info 1").click();
    h.run();
    h.get_by_label(CONSOLE_COPY_BUTTON).click();
    // The copy is in the output of the frame that saw the click.
    h.step();
    let text = copied(&h).expect("something was copied");
    assert_eq!(text.lines().count(), 2, "{text}");
    assert!(text.contains("warning 2 brushes could not be shown."));
    assert!(text.contains("error   castle.vmf could not be saved"));
}

#[test]
fn clear_empties_the_console() {
    let mut h = harness_for(workbench_with_messages());
    h.run();
    h.get_by_label(CONSOLE_CLEAR_BUTTON).click();
    h.run();
    assert!(h.state().console_lines().is_empty());
    assert!(h.query_by_label_contains("disk full").is_none());
}

#[test]
fn a_hidden_console_tab_counts_new_problems() {
    // Console behind Properties in the same tab group.
    let mut dock = egui_dock::DockState::new(vec![Panel::Viewport]);
    dock.main_surface_mut().split_right(
        egui_dock::NodeIndex::root(),
        0.7,
        vec![
            Panel::Properties,
            Panel::Console,
            Panel::Scene,
            Panel::Layers,
            Panel::Library,
        ],
    );
    let wb = Workbench::new(info(), Some(dock));
    let mut h = harness_for(wb);
    h.run();
    assert!(h.query_by_label("Console").is_some());
    h.state_mut().push_warning("one");
    h.state_mut().push_entry(LogEntry::error("two"));
    h.run();
    assert!(h.query_by_label("Console ❌ 2").is_some());
    // Looking at the Console clears the count.
    h.get_by_label("Console ❌ 2").click();
    h.run();
    h.run();
    assert!(h.query_by_label("Console").is_some());
    assert_eq!(h.state().console().unseen_problems(), (0, 0));
}

#[test]
fn problems_are_logged_at_the_right_level() {
    // A saved layout missing panels is replaced, with a warning.
    let broken = egui_dock::DockState::new(vec![Panel::Console]);
    let mut wb = Workbench::new(info(), Some(broken));
    wb.show_error("could not open");
    wb.push_console("fine");
    let levels: Vec<_> = wb.console().entries().map(|e| e.level).collect();
    assert_eq!(levels, [LogLevel::Warning, LogLevel::Error, LogLevel::Info]);
}

#[test]
fn a_full_console_only_lays_out_the_lines_on_screen() {
    // Regression: every kept message was laid out every frame, so a full
    // Console slowed the whole window down.
    let mut wb = Workbench::new(info(), None);
    for i in 0..MAX_CONSOLE_ENTRIES {
        wb.push_console(format!("message number {i}"));
    }
    let mut h = harness_for(wb);
    h.run();
    let last = format!("message number {}", MAX_CONSOLE_ENTRIES - 1);
    assert!(h.query_by_label(&last).is_some(), "stuck to the newest");
    assert!(
        h.query_by_label("message number 0").is_none(),
        "far above, not laid out"
    );
    let laid_out = h.query_all_by_label_contains("message number").count();
    assert!(laid_out < 100, "{laid_out} lines laid out");
}

#[test]
fn a_visible_console_shows_no_count_on_its_tab() {
    // Regression: the tab is drawn before the Console marks its messages
    // seen, and nothing drew the window again, so a count stayed on it.
    let mut h = harness_for(Workbench::new(info(), None));
    h.run();
    h.state_mut().push_warning("one");
    h.run();
    assert!(h.query_by_label("Console").is_some());
    assert!(h.query_by_label("Console ⚠ 1").is_none());
}

#[test]
fn a_long_or_multi_line_message_stays_on_one_line() {
    let mut wb = Workbench::new(info(), None);
    wb.push_warning("first line\nsecond line");
    let mut h = harness_for(wb);
    h.run();
    assert!(h.query_by_label("first line …").is_some());
    assert!(h.query_by_label_contains("second line").is_none());
}
