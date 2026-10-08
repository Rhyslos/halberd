//! The undo and redo lists.

use crate::command::Change;
use std::collections::VecDeque;

/// Most edits that can be undone. The oldest are forgotten beyond this.
pub const MAX_UNDO_STEPS: usize = 1000;

/// One undoable step.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub(crate) label: String,
    /// The change as it was applied.
    pub(crate) change: Change,
}

/// Applied edits that can be undone, and undone edits that can be redone.
#[derive(Debug, Clone, Default)]
pub(crate) struct History {
    undo: VecDeque<Entry>,
    redo: Vec<Entry>,
}

impl History {
    /// Records a newly applied edit. Anything that could be redone is
    /// forgotten, as in every editor.
    pub(crate) fn record(&mut self, entry: Entry) {
        self.redo.clear();
        self.undo.push_back(entry);
        while self.undo.len() > MAX_UNDO_STEPS {
            self.undo.pop_front();
        }
    }

    pub(crate) fn take_undo(&mut self) -> Option<Entry> {
        self.undo.pop_back()
    }

    pub(crate) fn push_redo(&mut self, entry: Entry) {
        self.redo.push(entry);
    }

    pub(crate) fn take_redo(&mut self) -> Option<Entry> {
        self.redo.pop()
    }

    /// Puts a redone edit back on the undo list without clearing redo.
    pub(crate) fn push_undo(&mut self, entry: Entry) {
        self.undo.push_back(entry);
    }

    pub(crate) fn undo_label(&self) -> Option<&str> {
        self.undo.back().map(|e| e.label.as_str())
    }

    pub(crate) fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|e| e.label.as_str())
    }
}
