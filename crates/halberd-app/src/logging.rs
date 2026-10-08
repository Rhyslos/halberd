//! Where Halberd's messages go: the Console panel and a log file.
//!
//! Libraries Halberd uses (wgpu for graphics, winit for the window) report
//! problems through the standard `log` interface. Halberd's [`Logger`]
//! takes those reports, plus anything Halberd's own crates log, and hands
//! them to the Console. Every message, from the logger or from the editor
//! itself, is also written to `halberd.log` next to the settings file, so
//! a problem can be looked up (or attached to a bug report) after Halberd
//! has closed, even after a crash. The previous run's file is kept as
//! `halberd.previous.log`.

use halberd_ui::{LogEntry, LogLevel};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, TryLockError};
use std::time::Duration;

/// Name of the log file, next to the settings file.
pub(crate) const LOG_FILE_NAME: &str = "halberd.log";
/// Name the previous run's log file is kept under.
pub(crate) const PREVIOUS_LOG_FILE_NAME: &str = "halberd.previous.log";
/// The most one run writes to the log file, so a flood of messages can
/// never fill the disk.
pub(crate) const MAX_LOG_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// Locks `mutex`, carrying on even if a thread panicked while holding it:
/// a log must keep working, especially then.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The longest single line written; longer messages are cut, so one
/// message can never use up the file.
pub(crate) const MAX_LOG_LINE_BYTES: usize = 64 * 1024;
/// How long the waker waits before waking the window, so a burst of
/// messages (or one repeated every frame) wakes it once, not constantly.
const WAKE_DELAY: Duration = Duration::from_millis(250);

/// The log file for this run.
pub(crate) struct LogFile {
    path: PathBuf,
    file: File,
    written: u64,
    full: bool,
}

impl LogFile {
    /// Starts a new log at `path`, keeping an existing one as the previous
    /// run's log. `banner` is the first line. If the old log cannot be set
    /// aside (another program has it open, say), this run is added to its
    /// end rather than erasing it.
    pub(crate) fn create(path: &Path, banner: &str) -> std::io::Result<Self> {
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)?;
        }
        let set_aside =
            !path.exists() || fs::rename(path, path.with_file_name(PREVIOUS_LOG_FILE_NAME)).is_ok();
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(!set_aside)
            .truncate(set_aside)
            .open(path)?;
        let mut log = Self {
            path: path.to_path_buf(),
            file,
            written: 0,
            full: false,
        };
        let started = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        log.write_line(&format!("{banner}, started {started}"));
        Ok(log)
    }

    /// Where the file is.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Writes one message. Once the file reaches [`MAX_LOG_FILE_BYTES`], a
    /// last line says so and the rest of this run is not written.
    pub(crate) fn write(&mut self, entry: &LogEntry) {
        self.write_line(&entry.to_text());
    }

    /// Writes the reason for a crash, even when the file is full: that is
    /// the line most worth keeping.
    pub(crate) fn write_crash(&mut self, entry: &LogEntry) {
        let _ = writeln!(self.file, "{}", cut(&entry.to_text()));
        let _ = self.file.flush();
    }

    fn write_line(&mut self, line: &str) {
        if self.full {
            return;
        }
        let line = cut(line);
        let bytes = line.len() as u64 + 1;
        if self.written + bytes > MAX_LOG_FILE_BYTES {
            self.full = true;
            let _ = writeln!(
                self.file,
                "The log file is full; later messages are only in the Console."
            );
            return;
        }
        // Each line goes straight to disk, so it survives a crash.
        if writeln!(self.file, "{line}").is_ok() {
            self.written += bytes;
        }
        let _ = self.file.flush();
    }
}

/// `line`, cut to [`MAX_LOG_LINE_BYTES`] (at a character boundary).
fn cut(line: &str) -> std::borrow::Cow<'_, str> {
    if line.len() <= MAX_LOG_LINE_BYTES {
        return line.into();
    }
    let mut end = MAX_LOG_LINE_BYTES;
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{} … (message cut, it was {} bytes)",
        &line[..end],
        line.len()
    )
    .into()
}

/// Messages waiting for the Console, and the log file. Shared between the
/// logger (any thread) and the window (the interface thread).
#[derive(Default)]
pub(crate) struct LogBook {
    inbox: Mutex<Vec<LogEntry>>,
    file: Mutex<Option<LogFile>>,
    /// Signals the waker thread that messages are waiting.
    waker: OnceLock<SyncSender<()>>,
}

impl LogBook {
    /// A log book writing to `file` (or to no file).
    pub(crate) fn new(file: Option<LogFile>) -> Arc<Self> {
        Arc::new(Self {
            file: Mutex::new(file),
            ..Self::default()
        })
    }

    /// Where the log file is, if there is one.
    pub(crate) fn file_path(&self) -> Option<PathBuf> {
        lock(&self.file).as_ref().map(|f| f.path().to_path_buf())
    }

    /// A message from the logger: written to the file and queued for the
    /// Console.
    ///
    /// This never calls into the window or egui: libraries (egui itself)
    /// log while holding their own locks, and calling back into them from
    /// here would freeze Halberd. The waker thread wakes the window instead.
    pub(crate) fn record(&self, entry: LogEntry) {
        self.write_to_file(&entry);
        lock(&self.inbox).push(entry);
        if let Some(waker) = self.waker.get() {
            // Full means a wake-up is already on its way.
            let _ = waker.try_send(());
        }
    }

    /// Writes a message to the file only (one the Console already has).
    pub(crate) fn write_to_file(&self, entry: &LogEntry) {
        if let Some(file) = lock(&self.file).as_mut() {
            file.write(entry);
        }
    }

    /// Takes the messages waiting for the Console.
    pub(crate) fn take(&self) -> Vec<LogEntry> {
        std::mem::take(&mut *lock(&self.inbox))
    }

    fn has_waiting(&self) -> bool {
        !lock(&self.inbox).is_empty()
    }

    /// Starts a thread that calls `wake` (which wakes the window) shortly
    /// after messages arrive, at most once per [`WAKE_DELAY`].
    pub(crate) fn start_waker(self: &Arc<Self>, wake: impl Fn() + Send + 'static) {
        let (signal, wait) = sync_channel(1);
        if self.waker.set(signal).is_err() {
            return; // Already started.
        }
        let book = Arc::clone(self);
        // Without the thread, messages still appear on the next frame.
        let _ = std::thread::Builder::new()
            .name("halberd-log-waker".into())
            .spawn(move || {
                while wait.recv().is_ok() {
                    std::thread::sleep(WAKE_DELAY);
                    if book.has_waiting() {
                        wake();
                    }
                }
            });
    }

    /// Writes the reason for a crash, without ever waiting: if the crash
    /// happened while the file was being written, waiting would hang
    /// instead of closing.
    fn write_crash(&self, message: String) {
        let mut guard = match self.file.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => return,
        };
        if let Some(file) = guard.as_mut() {
            file.write_crash(&LogEntry::error(message));
        }
    }
}

/// The `log` interface's receiver: passes messages to a [`LogBook`].
pub(crate) struct Logger {
    book: Arc<LogBook>,
}

impl Logger {
    pub(crate) fn new(book: Arc<LogBook>) -> Self {
        Self { book }
    }
}

/// True for messages from Halberd's own crates.
fn is_halberd(target: &str) -> bool {
    target.starts_with("halberd")
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        // Halberd's own information; only warnings and errors from
        // libraries, which are chatty about things users can't act on.
        let quietest = if is_halberd(metadata.target()) {
            log::Level::Info
        } else {
            log::Level::Warn
        };
        metadata.level() <= quietest
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let level = match record.level() {
            log::Level::Error => LogLevel::Error,
            log::Level::Warn => LogLevel::Warning,
            _ => LogLevel::Info,
        };
        let target = record.target();
        let message = if is_halberd(target) {
            record.args().to_string()
        } else {
            // Say where a library's message came from (for bug reports).
            let library = target.split("::").next().unwrap_or(target);
            format!("{library}: {}", record.args())
        };
        self.book.record(LogEntry::new(level, message));
    }

    fn flush(&self) {}
}

/// Opens this run's log file in `folder`, or explains why not.
pub(crate) fn open_log_file(folder: &Path, banner: &str) -> (Option<LogFile>, Option<LogEntry>) {
    let path = folder.join(LOG_FILE_NAME);
    match LogFile::create(&path, banner) {
        Ok(file) => (Some(file), None),
        Err(e) => (
            None,
            Some(LogEntry::warning(format!(
                "No log file this time: {} could not be written ({e}).",
                path.display()
            ))),
        ),
    }
}

/// Starts logging for this run: opens the log file in `folder` (if given),
/// connects the `log` interface, and records crashes. Returns the log book
/// and a note for the Console if the file could not be made.
pub(crate) fn install(folder: Option<&Path>, banner: &str) -> (Arc<LogBook>, Option<LogEntry>) {
    let (file, note) = match folder {
        Some(folder) => open_log_file(folder, banner),
        None => (None, None),
    };
    let book = LogBook::new(file);
    // Fails only if a logger is already set (never, outside tests).
    if log::set_boxed_logger(Box::new(Logger::new(Arc::clone(&book)))).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
    record_crashes(Arc::clone(&book));
    (book, note)
}

/// Writes the reason for a crash (a panic) into the log before Halberd
/// stops, then lets the usual crash handling carry on.
fn record_crashes(book: Arc<LogBook>) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        book.write_crash(crash_message(info));
        previous(info);
    }));
}

/// A crash in plain words, with where in the code it happened.
fn crash_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    let what = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("no reason given");
    match info.location() {
        Some(at) => format!(
            "Something went wrong inside Halberd: {what} (at {}:{})",
            at.file(),
            at.line()
        ),
        None => format!("Something went wrong inside Halberd: {what}"),
    }
}

#[cfg(test)]
mod tests;
