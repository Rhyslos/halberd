//! Tests for Compile and play.

use super::file_tests::harness_with_actions;
use super::tests::info;
use super::*;
use crate::console::LogLevel;
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
