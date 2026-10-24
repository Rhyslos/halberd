//! Tests for Compile and play.

use super::file_tests::harness_with_actions;
use super::tests::info;
use super::*;
use crate::console::LogLevel;
use crate::viewport::{GO_TO_LEAK_LABEL, HIDE_LEAK_LABEL, LEAK_BANNER_TEXT};
use egui_kittest::kittest::Queryable;

#[test]
fn f9_and_the_file_menu_ask_for_a_compile() {
    let mut h = harness_with_actions(Workbench::new(info(), None));
    h.run();
    h.key_press(Key::F9);
    h.run();
    h.get_by_label("File").click();
    h.run();
    h.get_by_label_contains(COMPILE_LABEL).click();
    h.run();
    assert_eq!(
        h.state().1,
        [WorkbenchAction::Compile, WorkbenchAction::Compile]
    );
}

#[test]
fn a_running_compile_shows_its_step_and_can_be_stopped() {
    let mut h = harness_with_actions(Workbench::new(info(), None));
    h.run_steps(3);
    h.state_mut().0.compile_started("my_map");
    h.state_mut()
        .0
        .compile_step(1, 3, "Working out visibility (fast) (vvis)");
    h.run_steps(3);
    assert!(h.state().0.is_compiling());
    assert!(
        h.query_by_label("Step 2 of 3: Working out visibility (fast) (vvis)")
            .is_some()
    );
    // While compiling, F9 asks for nothing more.
    h.key_press(Key::F9);
    h.run_steps(3);
    h.get_by_label(STOP_COMPILE_LABEL).click();
    h.run_steps(3);
    assert_eq!(h.state().1, [WorkbenchAction::CancelCompile]);
}

#[test]
fn compiler_output_and_the_outcome_go_to_the_console() {
    let mut wb = Workbench::new(info(), None);
    wb.compile_started("my_map");
    wb.compile_line("WARNING: a brush has no texture", CompileLine::Warning);
    wb.compile_line("**** leaked ****", CompileLine::Error);
    wb.compile_finished(Err("the map leaks".into()));
    assert!(!wb.is_compiling());
    let levels: Vec<(LogLevel, String)> = wb
        .console()
        .entries()
        .map(|e| (e.level, e.message.clone()))
        .collect();
    assert!(levels.contains(&(LogLevel::Warning, "WARNING: a brush has no texture".into())));
    assert!(levels.contains(&(LogLevel::Error, "**** leaked ****".into())));
    assert!(levels.contains(&(LogLevel::Error, "The compile failed: the map leaks".into())));
    wb.compile_started("my_map");
    wb.compile_finished(Ok("Starting Garry's Mod on my_map…".into()));
    let last = wb.console().entries().last().unwrap().message.clone();
    assert!(last.starts_with("Compiled in "), "{last}");
}

#[test]
fn the_file_menu_asks_for_a_leak_check() {
    let mut h = harness_with_actions(Workbench::new(info(), None));
    h.run();
    h.get_by_label("File").click();
    h.run();
    h.get_by_label(LEAK_CHECK_LABEL).click();
    h.run();
    assert_eq!(h.state().1, [WorkbenchAction::CheckLeaks]);
}

#[test]
fn a_leak_check_reports_as_a_check() {
    let mut wb = Workbench::new(info(), None);
    wb.leak_check_started("my_map");
    assert!(wb.is_compiling());
    wb.compile_finished(Ok("No leaks found: the map is sealed.".into()));
    let last = wb.console().entries().last().unwrap().message.clone();
    assert!(last.starts_with("Checked in "), "{last}");
    wb.leak_check_started("my_map");
    wb.compile_finished(Err("vbsp stopped".into()));
    let last = wb.console().entries().last().unwrap().message.clone();
    assert_eq!(last, "The leak check failed: vbsp stopped");
}

const PATH: [[f32; 3]; 3] = [[-512.0, 0.0, 64.0], [0.0, 0.0, 64.0], [0.0, 128.0, 64.0]];

#[test]
fn a_leak_is_shown_with_a_banner_until_hidden() {
    let mut h = harness_with_actions(Workbench::new(info(), None));
    h.run_steps(3);
    assert!(h.query_by_label(LEAK_BANNER_TEXT).is_none());
    h.state_mut().0.set_leak_path(&PATH);
    h.run_steps(3);
    assert!(h.query_by_label(LEAK_BANNER_TEXT).is_some());
    assert_eq!(h.state().0.viewport().leak_path().map(<[_]>::len), Some(3));
    let warned = h
        .state()
        .0
        .console()
        .entries()
        .any(|e| e.level == LogLevel::Warning && e.message.starts_with("The map has a leak"));
    assert!(warned);

    // Go to leak points the 3D view at the line.
    h.get_by_label(GO_TO_LEAK_LABEL).click();
    h.run_steps(3);
    let camera = h.state().0.viewport().active_camera();
    let middle = glam::Vec3::new(-256.0, 64.0, 64.0);
    assert!(camera.depth_of(middle) > 0.0, "the leak is in front");
    assert!(
        camera
            .project(middle, glam::Vec2::new(800.0, 600.0))
            .is_some()
    );

    h.get_by_label(HIDE_LEAK_LABEL).click();
    h.run_steps(3);
    assert!(h.state().0.viewport().leak_path().is_none());
    assert!(h.query_by_label(LEAK_BANNER_TEXT).is_none());
}

#[test]
fn a_successful_compile_or_another_map_clears_the_leak() {
    let mut wb = Workbench::new(info(), None);
    wb.set_leak_path(&PATH);
    wb.compile_started("my_map");
    wb.compile_finished(Ok("Starting".into()));
    assert!(wb.viewport().leak_path().is_none());

    wb.set_leak_path(&PATH);
    wb.compile_started("my_map");
    wb.compile_finished(Err("the map leaks".into()));
    assert!(
        wb.viewport().leak_path().is_some(),
        "a failed compile keeps it"
    );
    wb.new_document();
    assert!(wb.viewport().leak_path().is_none());
}

#[test]
fn a_path_too_short_or_broken_is_not_shown() {
    let mut wb = Workbench::new(info(), None);
    wb.set_leak_path(&[[0.0, 0.0, 0.0]]);
    assert!(wb.viewport().leak_path().is_none());
    wb.set_leak_path(&[[f32::NAN, 0.0, 0.0], [1.0, 0.0, 0.0]]);
    assert!(wb.viewport().leak_path().is_none());
    let quiet = !wb
        .console()
        .entries()
        .any(|e| e.message.starts_with("The map has a leak"));
    assert!(quiet, "no warning without a line to show");
}

#[test]
fn a_leak_found_by_a_check_is_its_answer_not_a_failure() {
    let mut wb = Workbench::new(info(), None);
    wb.leak_check_started("my_map");
    wb.set_leak_path(&PATH);
    wb.compile_found_leak();
    assert!(!wb.is_compiling());
    let last = wb.console().entries().last().unwrap().clone();
    assert_eq!(last.level, LogLevel::Warning);
    assert!(last.message.starts_with("Leak found"), "{}", last.message);
    assert!(last.message.contains("red line") && !last.message.contains("failed"));

    // A compile without a path to show says to look for the gap.
    let mut wb = Workbench::new(info(), None);
    wb.compile_started("my_map");
    wb.compile_found_leak();
    let last = wb.console().entries().last().unwrap().clone();
    assert_eq!(last.level, LogLevel::Error);
    assert!(
        last.message
            .starts_with("The compile stopped: the map leaks")
    );
    assert!(last.message.contains("didn't say where"));
}

#[test]
fn a_hidden_leak_comes_back_from_the_view_menu() {
    let mut h = harness_with_actions(Workbench::new(info(), None));
    h.run_steps(3);
    h.state_mut().0.set_leak_path(&PATH);
    h.run_steps(3);
    h.get_by_label(HIDE_LEAK_LABEL).click();
    h.run_steps(3);
    assert!(h.state().0.viewport().leak_path().is_none());
    assert!(h.state().0.viewport().has_leak());
    h.get_by_label("View").click();
    h.run_steps(3);
    h.get_by_label(SHOW_LEAK_LABEL).click();
    h.run_steps(3);
    assert!(h.state().0.viewport().leak_path().is_some());
    assert!(h.query_by_label(LEAK_BANNER_TEXT).is_some());
}
