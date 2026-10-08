//! Tests for the logger and the log file.

use super::*;
use log::Log;
use std::time::Duration;

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

fn record(logger: &Logger, level: log::Level, target: &str, text: &str) {
    logger.log(
        &log::Record::builder()
            .level(level)
            .target(target)
            .args(format_args!("{text}"))
            .build(),
    );
}

#[test]
fn library_chatter_is_left_out_but_its_problems_come_through() {
    let book = LogBook::new(None);
    let logger = Logger::new(Arc::clone(&book));
    record(
        &logger,
        log::Level::Info,
        "wgpu_core::device",
        "made a buffer",
    );
    record(&logger, log::Level::Debug, "halberd_io", "detail");
    record(&logger, log::Level::Info, "halberd_io::files", "opened");
    record(
        &logger,
        log::Level::Warn,
        "wgpu_hal::vulkan::instance",
        "old driver",
    );
    record(&logger, log::Level::Error, "winit", "lost the window");
    let got: Vec<_> = book
        .take()
        .into_iter()
        .map(|e| (e.level, e.message))
        .collect();
    assert_eq!(
        got,
        [
            (LogLevel::Info, "opened".to_string()),
            (LogLevel::Warning, "wgpu_hal: old driver".to_string()),
            (LogLevel::Error, "winit: lost the window".to_string()),
        ]
    );
    assert!(book.take().is_empty(), "taken once");
}

#[test]
fn messages_go_to_the_file_and_the_previous_run_is_kept() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join(LOG_FILE_NAME);
    fs::write(&path, "last time\n").unwrap();
    let book = LogBook::new(Some(LogFile::create(&path, "Halberd 1.0").unwrap()));
    book.record(LogEntry::warning("from the logger"));
    book.write_to_file(&LogEntry::info("from the editor"));
    let text = read(&path);
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines.len(), 3, "{text}");
    assert!(
        lines[0].starts_with("Halberd 1.0, started 20"),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].ends_with("warning from the logger"),
        "{}",
        lines[1]
    );
    assert!(
        lines[2].ends_with("info    from the editor"),
        "{}",
        lines[2]
    );
    assert_eq!(
        read(&folder.path().join(PREVIOUS_LOG_FILE_NAME)),
        "last time\n"
    );
    assert_eq!(book.file_path().unwrap(), path);
    assert_eq!(book.take().len(), 1, "only the logger's message is queued");
}

#[test]
fn the_file_stops_growing_at_its_limit() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join(LOG_FILE_NAME);
    let mut file = LogFile::create(&path, "Halberd").unwrap();
    let long = "x".repeat(100_000);
    for _ in 0..200 {
        file.write(&LogEntry::info(long.clone()));
    }
    let size = fs::metadata(&path).unwrap().len();
    assert!(size <= MAX_LOG_FILE_BYTES + 200, "{size}");
    assert!(read(&path).ends_with("later messages are only in the Console.\n"));
}

#[test]
fn a_folder_that_cannot_hold_the_file_gives_a_note() {
    let folder = tempfile::tempdir().unwrap();
    // A file where the folder should be.
    let blocked = folder.path().join("not-a-folder");
    fs::write(&blocked, "").unwrap();
    let (file, note) = open_log_file(&blocked, "Halberd");
    assert!(file.is_none());
    let note = note.unwrap();
    assert_eq!(note.level, LogLevel::Warning);
    assert!(note.message.contains("No log file"), "{}", note.message);
}

#[test]
fn an_older_previous_log_is_replaced() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join(LOG_FILE_NAME);
    fs::write(folder.path().join(PREVIOUS_LOG_FILE_NAME), "two runs ago\n").unwrap();
    fs::write(&path, "last run\n").unwrap();
    LogFile::create(&path, "Halberd").unwrap();
    assert_eq!(
        read(&folder.path().join(PREVIOUS_LOG_FILE_NAME)),
        "last run\n"
    );
}

#[test]
fn an_old_log_that_cannot_be_set_aside_is_added_to_not_erased() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join(LOG_FILE_NAME);
    fs::write(&path, "last run\n").unwrap();
    // A folder with something in it where the previous log would go:
    // the old log cannot be renamed onto it.
    let previous = folder.path().join(PREVIOUS_LOG_FILE_NAME);
    fs::create_dir(&previous).unwrap();
    fs::write(previous.join("x"), "").unwrap();
    let mut file = LogFile::create(&path, "Halberd").unwrap();
    file.write(&LogEntry::info("this run"));
    let text = read(&path);
    assert!(text.starts_with("last run\nHalberd, started"), "{text}");
    assert!(text.ends_with("this run\n"), "{text}");
}

#[test]
fn one_huge_message_is_cut_and_logging_carries_on() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join(LOG_FILE_NAME);
    let mut file = LogFile::create(&path, "Halberd").unwrap();
    file.write(&LogEntry::info("é".repeat(MAX_LOG_FILE_BYTES as usize)));
    file.write(&LogEntry::info("still logging"));
    let text = read(&path);
    assert!(text.contains("message cut"), "cut, not dropped");
    assert!(text.ends_with("still logging\n"));
    assert!((text.len() as u64) < 2 * MAX_LOG_LINE_BYTES as u64);
}

#[test]
fn a_crash_is_written_even_when_the_file_is_full_or_poisoned() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join(LOG_FILE_NAME);
    let mut file = LogFile::create(&path, "Halberd").unwrap();
    let line = "x".repeat(60_000);
    while !file.full {
        file.write(&LogEntry::info(line.clone()));
    }
    let book = LogBook::new(Some(file));
    // A thread that panics while holding the file poisons its lock.
    let poisoner = Arc::clone(&book);
    let _ = std::thread::spawn(move || {
        let _guard = poisoner.file.lock();
        std::panic::panic_any("poison");
    })
    .join();
    assert!(book.file.is_poisoned());
    book.write_crash("Something went wrong inside Halberd: surface lost".into());
    assert!(read(&path).ends_with("surface lost\n"));
}

#[test]
fn logging_from_inside_egui_does_not_freeze() {
    // Regression: egui logs some warnings while holding its own lock; the
    // logger used to call back into egui (to repaint) and froze Halberd.
    let ctx = eframe::egui::Context::default();
    let book = LogBook::new(None);
    let (woke, wakes) = std::sync::mpsc::channel();
    let waker_ctx = ctx.clone();
    book.start_waker(move || {
        waker_ctx.request_repaint();
        let _ = woke.send(());
    });
    let logger = Logger::new(Arc::clone(&book));
    let (done, finished) = std::sync::mpsc::channel();
    let inside = ctx.clone();
    std::thread::spawn(move || {
        // Holding egui's lock, as egui does when it logs.
        inside.data_mut(|_| {
            record(&logger, log::Level::Warn, "egui::util", "could not read");
        });
        let _ = done.send(());
    });
    finished
        .recv_timeout(Duration::from_secs(5))
        .expect("logging returned instead of freezing");
    wakes
        .recv_timeout(Duration::from_secs(5))
        .expect("the window was woken afterwards");
    assert_eq!(book.take().len(), 1);
}

#[test]
fn a_crash_is_described_with_its_place() {
    let message = Arc::new(Mutex::new(String::new()));
    let seen = Arc::clone(&message);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Other tests may panic on purpose at the same time.
        let text = crash_message(info);
        if text.contains("surface lost") {
            *lock(&seen) = text;
        }
    }));
    let _ = std::panic::catch_unwind(|| std::panic::panic_any(String::from("surface lost")));
    std::panic::set_hook(previous);
    let text = lock(&message).clone();
    assert!(
        text.starts_with("Something went wrong inside Halberd: surface lost (at "),
        "{text}"
    );
    assert!(text.contains("tests.rs"), "{text}");
}
