//! Compile and play: File → Compile and play (F9) asks the program to
//! compile the map and start Garry's Mod on it; while it runs, a small
//! window shows which step it is on, with a button to stop it.

use super::{Workbench, WorkbenchAction};
use crate::console::LogEntry;
use egui::{Align2, Key, KeyboardShortcut, Modifiers, RichText, Ui};
use std::time::Instant;

/// File → Compile and play (F9, as in Hammer).
pub const COMPILE_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F9);
/// The menu entry.
pub const COMPILE_LABEL: &str = "Compile and play";
/// The button that stops a compile.
pub const STOP_COMPILE_LABEL: &str = "Stop compiling";

/// How serious a line of compiler output is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileLine {
    /// Progress.
    Info,
    /// Worth a look.
    Warning,
    /// Something went wrong.
    Error,
}

/// A compile in progress, as the workbench shows it.
#[derive(Debug, Clone)]
pub(super) struct CompileProgress {
    map: String,
    step: usize,
    total: usize,
    label: String,
    started: Instant,
}

impl Workbench {
    /// True while a compile is running.
    pub fn is_compiling(&self) -> bool {
        self.compiling.is_some()
    }

    /// A compile of `map` started.
    pub fn compile_started(&mut self, map: &str) {
        self.compiling = Some(CompileProgress {
            map: map.to_string(),
            step: 0,
            total: 0,
            label: "Starting".into(),
            started: Instant::now(),
        });
        self.push_console(format!("Compiling {map} (quick test)…"));
    }

    /// The compile is on step `index` (from 0) of `total`, doing `label`.
    pub fn compile_step(&mut self, index: usize, total: usize, label: &str) {
        if let Some(progress) = &mut self.compiling {
            progress.step = index;
            progress.total = total;
            progress.label = label.to_string();
        }
    }

    /// A line of the compilers' output, for the Console.
    pub fn compile_line(&mut self, text: &str, kind: CompileLine) {
        let entry = match kind {
            CompileLine::Info => LogEntry::info(text),
            CompileLine::Warning => LogEntry::warning(text),
            CompileLine::Error => LogEntry::error(text),
        };
        self.push_entry(entry);
    }

    /// The compile is over: `Ok` with a note (such as "Starting Garry's
    /// Mod…"), or why it failed.
    pub fn compile_finished(&mut self, outcome: Result<String, String>) {
        let took = self
            .compiling
            .take()
            .map(|p| p.started.elapsed().as_secs())
            .unwrap_or(0);
        match outcome {
            Ok(note) => self.push_console(format!("Compiled in {took} s. {note}")),
            Err(why) => self.push_entry(LogEntry::error(format!("The compile failed: {why}"))),
        }
    }

    /// F9 asks for a compile (unless one is running, or a drag is).
    pub(super) fn compile_shortcut(&mut self, ui: &Ui, actions: &mut Vec<WorkbenchAction>) {
        if self.viewport.tools().is_busy() || self.is_asking_to_save() {
            return;
        }
        if ui.input_mut(|i| i.consume_shortcut(&COMPILE_SHORTCUT)) && !self.is_compiling() {
            actions.push(WorkbenchAction::Compile);
        }
    }

    /// The File menu's entry.
    pub(super) fn compile_menu_item(&self, ui: &mut Ui, actions: &mut Vec<WorkbenchAction>) {
        let keys = ui.ctx().format_shortcut(&COMPILE_SHORTCUT);
        let item = ui
            .add_enabled(
                !self.is_compiling(),
                egui::Button::new(COMPILE_LABEL).shortcut_text(keys),
            )
            .on_hover_text(
                "Compile a quick test of the map (fast visibility and lighting) and start \
                 Garry's Mod on it. Your map file isn't changed; unsaved changes are included.",
            );
        if item.clicked() {
            actions.push(WorkbenchAction::Compile);
        }
    }

    /// The progress window, while compiling.
    pub(super) fn compile_window(&self, ui: &Ui, actions: &mut Vec<WorkbenchAction>) {
        let Some(progress) = &self.compiling else {
            return;
        };
        egui::Window::new("Compiling")
            .anchor(Align2::RIGHT_TOP, [-12.0, 40.0])
            .resizable(false)
            .collapsible(false)
            .show(ui.ctx(), |ui| {
                ui.label(RichText::new(&progress.map).strong());
                let step = if progress.total == 0 {
                    progress.label.clone()
                } else {
                    format!(
                        "Step {} of {}: {}",
                        progress.step + 1,
                        progress.total,
                        progress.label
                    )
                };
                ui.label(step);
                let fraction = if progress.total == 0 {
                    0.0
                } else {
                    progress.step as f32 / progress.total as f32
                };
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .animate(true)
                        .text(format!("{} s", progress.started.elapsed().as_secs())),
                );
                ui.label(
                    RichText::new("Compiler output is in the Console.")
                        .weak()
                        .small(),
                );
                if ui.button(STOP_COMPILE_LABEL).clicked() {
                    actions.push(WorkbenchAction::CancelCompile);
                }
            });
        // The elapsed time keeps counting.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(500));
    }
}
