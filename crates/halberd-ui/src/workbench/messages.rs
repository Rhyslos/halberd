//! Adding messages to the Console, and reading them back.

use super::Workbench;
use crate::console::{Console, LogEntry};

impl Workbench {
    /// Adds an information message to the Console.
    pub fn push_console(&mut self, line: impl Into<String>) {
        self.console.push(LogEntry::info(line));
    }

    /// Adds a warning to the Console.
    pub fn push_warning(&mut self, line: impl Into<String>) {
        self.console.push(LogEntry::warning(line));
    }

    /// Adds a message of any level to the Console.
    pub fn push_entry(&mut self, entry: LogEntry) {
        self.console.push(entry);
    }

    /// The Console's messages and filters.
    pub fn console(&self) -> &Console {
        &self.console
    }

    /// The Console, for adding messages from the program's logger and
    /// connecting the log file.
    pub fn console_mut(&mut self) -> &mut Console {
        &mut self.console
    }

    /// The text of every message in the Console, oldest first.
    pub fn console_lines(&self) -> Vec<&str> {
        self.console.entries().map(|e| e.message.as_str()).collect()
    }
}
