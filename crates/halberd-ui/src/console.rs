//! The Console panel: everything Halberd has to say, newest at the bottom.
//!
//! Each message has a time and a level (information, warning or error).
//! The panel can show or hide each level, search the messages, copy what is
//! shown (for bug reports) and clear itself. A message repeated straight
//! after itself is shown once with a count, so a noisy warning cannot bury
//! everything else. When warnings or errors arrive while the Console is
//! hidden behind another tab, its tab shows how many.

use egui::{Color32, RichText, ScrollArea, Ui};
use std::collections::VecDeque;
use std::fmt;

/// The most messages the Console keeps; older ones are dropped first.
pub const MAX_CONSOLE_ENTRIES: usize = 5000;
/// Screen-reader name of the Console's search box (and for tests).
pub const CONSOLE_SEARCH_NAME: &str = "Search the Console";
/// The button that copies the shown messages.
pub const CONSOLE_COPY_BUTTON: &str = "Copy";
/// The button that empties the Console.
pub const CONSOLE_CLEAR_BUTTON: &str = "Clear";

/// How serious a message is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LogLevel {
    /// Something worth knowing: a map opened, settings loaded.
    Info,
    /// Something did not work as hoped, but nothing is lost: an edit that
    /// was refused, a brush that cannot be shown.
    Warning,
    /// Something failed: a map could not be opened or saved, a crash.
    Error,
}

impl LogLevel {
    /// Every level, least serious first.
    pub const ALL: [LogLevel; 3] = [Self::Info, Self::Warning, Self::Error];

    /// The level's name in plain words, as written in the log file.
    pub fn name(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    fn index(self) -> usize {
        self as usize
    }

    /// The filter button's label.
    fn plural(self) -> &'static str {
        match self {
            Self::Info => "Info",
            Self::Warning => "Warnings",
            Self::Error => "Errors",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Info => "ℹ",
            Self::Warning => "⚠",
            Self::Error => "❌",
        }
    }

    fn color(self, ui: &Ui) -> Color32 {
        match self {
            Self::Info => ui.visuals().weak_text_color(),
            Self::Warning => ui.visuals().warn_fg_color,
            Self::Error => ui.visuals().error_fg_color,
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One message in the Console.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    /// How serious it is.
    pub level: LogLevel,
    /// Local time it was made (last made, if repeated), as `HH:MM:SS`.
    pub time: String,
    /// The message in plain words. May span several lines.
    pub message: String,
    /// How many more times the same message came straight after it.
    pub repeats: u32,
}

impl LogEntry {
    /// A message made now.
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        Self {
            level,
            time: chrono::Local::now().format("%H:%M:%S").to_string(),
            message: message.into(),
            repeats: 0,
        }
    }

    /// An information message made now.
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Info, message)
    }

    /// A warning made now.
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Warning, message)
    }

    /// An error made now.
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Error, message)
    }

    /// The message as one line of text (plus indented continuation lines
    /// for a message spanning several): time, level, message and repeats.
    /// Used for copying and for the log file.
    pub fn to_text(&self) -> String {
        let mut text = format!("{} {:<7} ", self.time, self.level.name());
        let mut lines = self.message.lines();
        text.push_str(lines.next().unwrap_or_default());
        for line in lines {
            text.push_str("\n                 ");
            text.push_str(line);
        }
        if self.repeats > 0 {
            text.push_str(&format!(" (×{})", self.repeats + 1));
        }
        text
    }
}

/// Where new messages are also sent, such as the log file.
pub type ConsoleMirror = Box<dyn FnMut(&LogEntry)>;

/// The Console's messages and what it currently shows.
#[derive(Default)]
pub struct Console {
    entries: VecDeque<LogEntry>,
    /// Messages dropped to stay under [`MAX_CONSOLE_ENTRIES`].
    dropped: usize,
    /// Warnings and errors that arrived while the Console was not showing.
    unseen: [usize; 3],
    /// Levels hidden by the filter buttons.
    hidden: [bool; 3],
    search: String,
    mirror: Option<ConsoleMirror>,
    /// Kept messages per level.
    counts: [usize; 3],
    /// Goes up whenever the messages change.
    generation: u64,
    /// Which messages the filters let through, worked out again only when
    /// the messages, filters or search change.
    rows: Vec<usize>,
    rows_key: Option<(u64, [bool; 3], String)>,
    /// The tab's problem count changed, so the window should draw again.
    badge_changed: bool,
}

impl fmt::Debug for Console {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Console")
            .field("entries", &self.entries.len())
            .field("dropped", &self.dropped)
            .finish_non_exhaustive()
    }
}

impl Console {
    /// An empty Console.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sends every message, from now on, also to `mirror` (the log file).
    /// Messages already in the Console are sent to it first.
    pub fn set_mirror(&mut self, mut mirror: ConsoleMirror) {
        for entry in &self.entries {
            mirror(entry);
        }
        self.mirror = Some(mirror);
    }

    /// Adds a message, and sends it to the mirror.
    pub fn push(&mut self, entry: LogEntry) {
        if let Some(mirror) = &mut self.mirror {
            mirror(&entry);
        }
        self.add(entry);
    }

    /// Adds a message that is already in the log file (one that came
    /// through the program's logger), without sending it to the mirror.
    pub fn push_logged(&mut self, entry: LogEntry) {
        self.add(entry);
    }

    fn add(&mut self, entry: LogEntry) {
        self.generation += 1;
        if entry.level > LogLevel::Info {
            self.unseen[entry.level.index()] += 1;
            self.badge_changed = true;
        }
        if let Some(last) = self.entries.back_mut()
            && last.level == entry.level
            && last.message == entry.message
        {
            last.repeats = last.repeats.saturating_add(1);
            last.time = entry.time;
            return;
        }
        self.counts[entry.level.index()] += 1;
        self.entries.push_back(entry);
        while self.entries.len() > MAX_CONSOLE_ENTRIES {
            if let Some(old) = self.entries.pop_front() {
                self.counts[old.level.index()] -= 1;
            }
            self.dropped += 1;
        }
    }

    /// Every message kept, oldest first.
    pub fn entries(&self) -> impl Iterator<Item = &LogEntry> {
        self.entries.iter()
    }

    /// How many messages were dropped because the Console was full.
    pub fn dropped(&self) -> usize {
        self.dropped
    }

    /// How many kept messages have `level` (repeats count once).
    pub fn count(&self, level: LogLevel) -> usize {
        self.counts[level.index()]
    }

    /// Warnings and errors that arrived since the Console was last shown:
    /// (warnings, errors).
    pub fn unseen_problems(&self) -> (usize, usize) {
        (
            self.unseen[LogLevel::Warning.index()],
            self.unseen[LogLevel::Error.index()],
        )
    }

    /// Records that the Console is showing, so nothing in it is new.
    pub fn mark_seen(&mut self) {
        if self.unseen != [0; 3] {
            self.unseen = [0; 3];
            self.badge_changed = true;
        }
    }

    /// True (once) if the tab's problem count changed since last asked,
    /// so the window should be drawn again to show it.
    pub fn take_badge_change(&mut self) -> bool {
        std::mem::take(&mut self.badge_changed)
    }

    /// Shows or hides messages of `level`.
    pub fn set_shown(&mut self, level: LogLevel, shown: bool) {
        self.hidden[level.index()] = !shown;
    }

    /// Whether messages of `level` are shown.
    pub fn is_shown(&self, level: LogLevel) -> bool {
        !self.hidden[level.index()]
    }

    /// Shows only messages containing `text` (any letter case); empty
    /// shows everything.
    pub fn set_search(&mut self, text: impl Into<String>) {
        self.search = text.into();
    }

    /// The messages the filters let through, oldest first.
    pub fn visible(&self) -> impl Iterator<Item = &LogEntry> {
        let needle = self.search.trim().to_lowercase();
        self.entries.iter().filter(move |e| self.passes(e, &needle))
    }

    fn passes(&self, entry: &LogEntry, needle: &str) -> bool {
        self.is_shown(entry.level)
            && (needle.is_empty() || entry.message.to_lowercase().contains(needle))
    }

    /// Works out the shown rows again if anything changed since last time.
    fn refresh_rows(&mut self) {
        let key = (self.generation, self.hidden, self.search.clone());
        if self.rows_key.as_ref() == Some(&key) {
            return;
        }
        let needle = self.search.trim().to_lowercase();
        let rows = (0..self.entries.len())
            .filter(|&i| self.passes(&self.entries[i], &needle))
            .collect();
        self.rows = rows;
        self.rows_key = Some(key);
    }

    /// The shown messages as text, one per line, for the clipboard.
    pub fn visible_text(&self) -> String {
        let mut text = String::new();
        for entry in self.visible() {
            text.push_str(&entry.to_text());
            text.push('\n');
        }
        text
    }

    /// Empties the Console (the log file keeps everything).
    pub fn clear(&mut self) {
        self.entries.clear();
        self.dropped = 0;
        self.counts = [0; 3];
        self.generation += 1;
        self.mark_seen();
    }

    /// The Console tab's name as drawn: coloured when it has a count.
    pub fn tab_text(&self) -> egui::WidgetText {
        let (title, level) = self.tab_title();
        match level {
            Some(LogLevel::Error) => RichText::new(title).color(Color32::from_rgb(235, 90, 80)),
            Some(_) => RichText::new(title).color(Color32::from_rgb(230, 180, 60)),
            None => RichText::new(title),
        }
        .into()
    }

    /// The Console tab's name: "Console", plus a count of warnings and
    /// errors that arrived while it was hidden.
    pub fn tab_title(&self) -> (String, Option<LogLevel>) {
        let (warnings, errors) = self.unseen_problems();
        match (warnings, errors) {
            (0, 0) => ("Console".to_string(), None),
            (w, 0) => (format!("Console ⚠ {w}"), Some(LogLevel::Warning)),
            (w, e) => (format!("Console ❌ {}", w + e), Some(LogLevel::Error)),
        }
    }

    /// Draws the panel: filter buttons, search, Copy and Clear, then the
    /// messages.
    pub(crate) fn show(&mut self, ui: &mut Ui) {
        self.mark_seen();
        ui.horizontal(|ui| {
            for level in LogLevel::ALL {
                let mut shown = self.is_shown(level);
                let label = format!("{} {} {}", level.icon(), level.plural(), self.count(level));
                // Plain text: coloured text is hard to read on the
                // button's "on" background.
                let text = if shown {
                    RichText::new(label)
                } else {
                    RichText::new(label).weak()
                };
                if ui
                    .toggle_value(&mut shown, text)
                    .on_hover_text(format!("Show or hide {}", level.plural().to_lowercase()))
                    .changed()
                {
                    self.set_shown(level, shown);
                }
            }
            ui.separator();
            let search = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search")
                    .desired_width(180.0),
            );
            search.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, CONSOLE_SEARCH_NAME)
            });
            if ui
                .button(CONSOLE_COPY_BUTTON)
                .on_hover_text("Copy the shown messages, for example into a bug report")
                .clicked()
            {
                ui.ctx().copy_text(self.visible_text());
            }
            if ui
                .button(CONSOLE_CLEAR_BUTTON)
                .on_hover_text("Empty the Console (the log file keeps everything)")
                .clicked()
            {
                self.clear();
            }
        });
        ui.separator();
        if self.dropped > 0 {
            ui.label(
                RichText::new(format!(
                    "{} older messages were removed; the log file has them all.",
                    self.dropped
                ))
                .weak(),
            );
        }
        self.refresh_rows();
        // One line per message, and only the lines on screen are laid out,
        // so a full Console costs no more than a nearly empty one.
        let row_height = ui
            .text_style_height(&egui::TextStyle::Monospace)
            .max(ui.text_style_height(&egui::TextStyle::Body));
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show_rows(ui, row_height, self.rows.len(), |ui, range| {
                for &i in &self.rows[range] {
                    if let Some(entry) = self.entries.get(i) {
                        entry_row(ui, entry, row_height);
                    }
                }
            });
    }
}

/// One message on one line: time, level icon, repeat count and text. A
/// message too long for the line is cut short; hovering shows all of it.
fn entry_row(ui: &mut Ui, entry: &LogEntry, height: f32) {
    let size = egui::vec2(ui.available_width(), height);
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    ui.allocate_ui_with_layout(size, layout, |ui| {
        ui.label(RichText::new(&entry.time).monospace().weak());
        let color = entry.level.color(ui);
        ui.label(RichText::new(entry.level.icon()).monospace().color(color))
            .on_hover_text(entry.level.name());
        if entry.repeats > 0 {
            ui.label(RichText::new(format!("×{}", entry.repeats + 1)).weak())
                .on_hover_text("The same message came this many times in a row");
        }
        let mut lines = entry.message.lines();
        let first = lines.next().unwrap_or_default();
        let more = lines.next().is_some();
        let shown = if more {
            format!("{first} …")
        } else {
            first.to_string()
        };
        let mut text = RichText::new(shown).monospace();
        if entry.level > LogLevel::Info {
            text = text.color(color);
        }
        let label = ui.add(egui::Label::new(text).truncate());
        if more {
            label.on_hover_text(&entry.message);
        }
    });
}

#[cfg(test)]
mod tests;
