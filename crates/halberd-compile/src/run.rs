//! Running a compile in the background: each tool in turn as its own
//! process (never through a shell), its output passed on line by line,
//! with a way to cancel.

use crate::CompileError;
use crate::job::{CompileJob, Step};
use crate::log::{LineKind, classify, is_leak};
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

/// Longest line of output kept (compilers sometimes print very long ones).
const MAX_LINE: usize = 2000;

/// What a running compile reports.
#[derive(Debug, Clone, PartialEq)]
pub enum CompileEvent {
    /// Step `index` (from 0) of `total` started.
    Step {
        /// Which step.
        index: usize,
        /// How many there are.
        total: usize,
        /// The tool (`vbsp`).
        tool: &'static str,
        /// What it does ("Building geometry").
        label: &'static str,
    },
    /// A line of the tool's output.
    Line(String, LineKind),
    /// The map leaks; this is the path vbsp found from outside the map
    /// (first point) to the leaking entity (last point). The gap is where
    /// it crosses the map's outer wall.
    LeakPath(Vec<[f32; 3]>),
    /// The compile is over.
    Finished(Result<Compiled, CompileError>),
}

/// A finished compile.
#[derive(Debug, Clone, PartialEq)]
pub struct Compiled {
    /// The map's name, for loading it in the game.
    pub map_name: String,
    /// Where the map was put for the game (`None` for a leak check).
    pub installed: Option<PathBuf>,
    /// How long it took.
    pub took: Duration,
}

/// A compile running in the background.
pub struct CompileRunner {
    events: Receiver<CompileEvent>,
    cancel: Arc<AtomicBool>,
    finished: bool,
    /// The compile's thread, waited for when the runner goes (so a tool
    /// is never left running when the editor closes).
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CompileRunner {
    /// Starts compiling `job`; `wake` is called whenever there is
    /// something new to read (to redraw the window).
    pub fn start(job: CompileJob, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let (send, events) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&cancel);
        let wake = Arc::new(wake);
        let report = {
            let send = send.clone();
            let wake = Arc::clone(&wake);
            move |event: CompileEvent| {
                send.send(event).ok();
                wake();
            }
        };
        let started = std::thread::Builder::new()
            .name("halberd-compile".into())
            .spawn(move || {
                let result = run(&job, &stop, &report);
                report(CompileEvent::Finished(result));
            });
        let thread = match started {
            Ok(thread) => Some(thread),
            Err(e) => {
                send.send(CompileEvent::Finished(Err(CompileError::CouldNotStart(
                    "the compile",
                    e.to_string(),
                ))))
                .ok();
                None
            }
        };
        Self {
            events,
            cancel,
            finished: false,
            thread,
        }
    }

    /// Asks the compile to stop (the running tool is ended).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Everything reported since the last call.
    pub fn poll(&mut self) -> Vec<CompileEvent> {
        let events: Vec<CompileEvent> = self.events.try_iter().collect();
        if events
            .iter()
            .any(|e| matches!(e, CompileEvent::Finished(_)))
        {
            self.finished = true;
        }
        events
    }

    /// True once the compile is over (finished, failed or cancelled).
    pub fn is_finished(&self) -> bool {
        self.finished
    }
}

impl Drop for CompileRunner {
    fn drop(&mut self) {
        // Closing the window ends a running compile: the thread sees the
        // flag within a tenth of a second, ends the tool and waits for it.
        self.cancel();
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
    }
}

fn run(
    job: &CompileJob,
    stop: &AtomicBool,
    report: &impl Fn(CompileEvent),
) -> Result<Compiled, CompileError> {
    let started = Instant::now();
    let steps = job.steps()?;
    // An old leak path must not be taken for this compile's.
    std::fs::remove_file(job.vmf.with_extension("lin")).ok();
    let total = steps.len();
    for (index, step) in steps.iter().enumerate() {
        report(CompileEvent::Step {
            index,
            total,
            tool: step.tool,
            label: step.label,
        });
        if let Err(e) = run_step(job, step, stop, report) {
            // A leak, or an area portal leak (which vbsp reports in other
            // words), leaves a pointfile.
            if let Some(points) = crate::read_pointfile(&job.vmf) {
                report(CompileEvent::LeakPath(points));
            }
            return Err(e);
        }
    }
    if job.profile == crate::Profile::LeakCheck {
        return Ok(Compiled {
            map_name: job.map_name(),
            installed: None,
            took: started.elapsed(),
        });
    }
    let installed = job.installed_bsp();
    // Never replace a map Halberd didn't put there (a stock map, say).
    let marker = installed.with_extension("bsp.halberd");
    if installed.exists() && !marker.exists() {
        return Err(CompileError::WouldReplace(job.map_name()));
    }
    let copy = || -> std::io::Result<()> {
        if let Some(maps) = installed.parent() {
            std::fs::create_dir_all(maps)?;
        }
        std::fs::copy(job.bsp(), &installed)?;
        std::fs::write(&marker, b"Compiled by Halberd; it may replace this map.\n")
    };
    copy().map_err(|e| CompileError::CouldNotCopy(e.to_string()))?;
    Ok(Compiled {
        map_name: job.map_name(),
        installed: Some(installed),
        took: started.elapsed(),
    })
}

/// Runs one tool to the end, passing its output on.
fn run_step(
    job: &CompileJob,
    step: &Step,
    stop: &AtomicBool,
    report: &impl Fn(CompileEvent),
) -> Result<(), CompileError> {
    let mut command = Command::new(&step.program);
    command
        .args(&step.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = job.vmf.parent() {
        command.current_dir(dir);
    }
    let mut child = command
        .spawn()
        .map_err(|e| CompileError::CouldNotStart(step.tool, e.to_string()))?;
    let (send, lines) = mpsc::channel::<String>();
    let readers = [
        child.stdout.take().map(|out| read_lines(out, send.clone())),
        child.stderr.take().map(|err| read_lines(err, send.clone())),
    ];
    drop(send);
    let mut leaked = false;
    let mut last_error = String::new();
    loop {
        if stop.load(Ordering::Relaxed) {
            end(&mut child);
            return Err(CompileError::Cancelled);
        }
        match lines.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                let kind = classify(&line);
                leaked |= is_leak(&line);
                if kind == LineKind::Error {
                    last_error.clone_from(&line);
                }
                report(CompileEvent::Line(line, kind));
            }
            // Both outputs closed: the tool has finished or is finishing.
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
    for reader in readers.into_iter().flatten() {
        reader.join().ok();
    }
    let status = child
        .wait()
        .map_err(|e| CompileError::CouldNotStart(step.tool, e.to_string()))?;
    if leaked {
        return Err(CompileError::Leaked);
    }
    if !status.success() {
        return Err(CompileError::ToolFailed(
            step.tool,
            last_error.trim().to_string(),
        ));
    }
    Ok(())
}

/// Ends a tool that is still running.
fn end(child: &mut Child) {
    child.kill().ok();
    child.wait().ok();
}

/// Reads `from` line by line on its own thread, sending each line (as
/// text, odd bytes replaced, carriage returns treated as line ends).
fn read_lines(
    from: impl Read + Send + 'static,
    send: Sender<String>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(from);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let text = String::from_utf8_lossy(&buf);
                    for part in text.split('\r') {
                        let line: String = part.trim_end().chars().take(MAX_LINE).collect();
                        if !line.is_empty() && send.send(line).is_err() {
                            return;
                        }
                    }
                }
            }
        }
    })
}
