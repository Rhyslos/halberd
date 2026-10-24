//! Running compiles with a stand-in compiler in a made-up Garry's Mod.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use halberd_compile::{CompileError, CompileEvent, CompileJob, CompileRunner, LineKind, Profile};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const FAKE: &str = env!("CARGO_BIN_EXE_halberd-fake-compiler");

/// A Garry's Mod folder with the stand-in as its three tools, and a map
/// file holding `text`.
fn setup(text: &str) -> (tempfile::TempDir, CompileJob) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("GarrysMod");
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(root.join("garrysmod")).unwrap();
    let ext = std::env::consts::EXE_SUFFIX;
    let tool = |name: &str| -> PathBuf {
        let path = bin.join(format!("{name}{ext}"));
        std::fs::copy(FAKE, &path).unwrap();
        path
    };
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let vmf = work.join("test_map.vmf");
    std::fs::write(&vmf, text).unwrap();
    let job = CompileJob {
        vmf,
        game_dir: root.join("garrysmod"),
        vbsp: Some(tool("vbsp")),
        vvis: Some(tool("vvis")),
        vrad: Some(tool("vrad")),
        root,
        profile: Profile::Quick,
    };
    (dir, job)
}

/// Runs `job` to the end (or until `cancel_after` has passed, then
/// cancels), collecting what it reports.
fn run(job: CompileJob, cancel_after: Option<Duration>) -> Vec<CompileEvent> {
    let mut runner = CompileRunner::start(job, || {});
    let started = Instant::now();
    let mut events = Vec::new();
    while !runner.is_finished() {
        events.extend(runner.poll());
        if cancel_after.is_some_and(|after| started.elapsed() > after) {
            runner.cancel();
        }
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "never finished"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    events
}

fn result(events: &[CompileEvent]) -> &Result<halberd_compile::Compiled, CompileError> {
    match events.last() {
        Some(CompileEvent::Finished(result)) => result,
        other => panic!("no result: {other:?}"),
    }
}

#[test]
fn a_compile_runs_each_tool_and_puts_the_map_in_the_game() {
    let (_dir, job) = setup("world {}");
    let installed = job.installed_bsp();
    let events = run(job, None);
    let steps: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            CompileEvent::Step { tool, .. } => Some(*tool),
            _ => None,
        })
        .collect();
    assert_eq!(steps, ["vbsp", "vvis", "vrad"]);
    let compiled = result(&events).as_ref().unwrap();
    assert_eq!(compiled.map_name, "test_map");
    assert_eq!(std::fs::read(&installed).unwrap(), b"VBSP");
    // Output arrives line by line, sorted by kind; carriage returns split.
    let lines: Vec<(String, LineKind)> = events
        .iter()
        .filter_map(|e| match e {
            CompileEvent::Line(text, kind) => Some((text.clone(), *kind)),
            _ => None,
        })
        .collect();
    assert!(
        lines
            .iter()
            .any(|(t, k)| t.contains("texture missing") && *k == LineKind::Warning)
    );
    assert!(lines.iter().any(|(t, _)| t == "done"));
    assert!(lines.iter().any(|(t, _)| t.contains("-fast -ldr")));
}

#[test]
fn a_leak_stops_the_compile_with_a_plain_reason() {
    let (_dir, job) = setup("world { LEAK }");
    let events = run(job.clone(), None);
    assert_eq!(result(&events), &Err(CompileError::Leaked));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, CompileEvent::Step { tool: "vvis", .. }))
    );
    assert!(!job.installed_bsp().exists());
}

#[test]
fn a_failing_tool_says_why() {
    let (_dir, job) = setup("world { CRASH }");
    let events = run(job, None);
    match result(&events) {
        Err(CompileError::ToolFailed("vbsp", why)) => assert!(why.contains("no sides"), "{why}"),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn a_compile_can_be_cancelled() {
    let (_dir, job) = setup("world { SLOW }");
    let started = Instant::now();
    let events = run(job, Some(Duration::from_millis(500)));
    assert_eq!(result(&events), &Err(CompileError::Cancelled));
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the slow tool was ended"
    );
}

#[test]
fn a_missing_tool_is_reported_before_anything_runs() {
    let (_dir, mut job) = setup("world {}");
    job.vvis = None;
    let events = run(job, None);
    assert_eq!(result(&events), &Err(CompileError::MissingTool("vvis")));
    assert_eq!(events.len(), 1);
}

#[test]
fn tools_are_only_run_from_inside_the_game_folder() {
    let (dir, mut job) = setup("world {}");
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let outside = elsewhere.join(Path::new(FAKE).file_name().unwrap());
    std::fs::copy(FAKE, &outside).unwrap();
    job.vrad = Some(outside);
    let events = run(job, None);
    assert_eq!(result(&events), &Err(CompileError::ToolOutsideGame("vrad")));
}

#[test]
fn relative_folders_still_work() {
    // Regression: a Garry's Mod folder given as a relative path made the
    // tools impossible to find, since they run in the map's folder.
    let (dir, mut job) = setup("world {}");
    let here = std::env::current_dir().unwrap();
    let relative = |p: &Path| -> PathBuf {
        let mut back = PathBuf::new();
        for _ in here.components().skip(1) {
            back.push("..");
        }
        back.join(p.strip_prefix("/").unwrap_or(p))
    };
    if cfg!(unix) {
        job.root = relative(&job.root);
        job.game_dir = relative(&job.game_dir);
        job.vbsp = job.vbsp.as_deref().map(relative);
        job.vvis = job.vvis.as_deref().map(relative);
        job.vrad = job.vrad.as_deref().map(relative);
    }
    let events = run(job, None);
    assert!(result(&events).is_ok(), "{:?}", result(&events));
    drop(dir);
}

#[test]
fn a_map_halberd_didnt_make_is_never_replaced() {
    let (_dir, job) = setup("world {}");
    let installed = job.installed_bsp();
    std::fs::create_dir_all(installed.parent().unwrap()).unwrap();
    std::fs::write(&installed, b"STOCK").unwrap();
    let events = run(job.clone(), None);
    assert_eq!(
        result(&events),
        &Err(CompileError::WouldReplace("test_map".into()))
    );
    assert_eq!(std::fs::read(&installed).unwrap(), b"STOCK");
    // Its own maps it replaces freely.
    std::fs::remove_file(&installed).unwrap();
    assert!(result(&run(job.clone(), None)).is_ok());
    assert!(
        result(&run(job, None)).is_ok(),
        "compiled again over its own map"
    );
}

#[test]
fn dropping_the_runner_ends_the_running_tool() {
    // Regression: closing the editor left the compiler running.
    let (_dir, job) = setup("world { SLOW }");
    let mut runner = CompileRunner::start(job, || {});
    let started = Instant::now();
    while !runner
        .poll()
        .iter()
        .any(|e| matches!(e, CompileEvent::Step { tool: "vrad", .. }))
    {
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
    let dropping = Instant::now();
    drop(runner);
    assert!(
        dropping.elapsed() < Duration::from_secs(5),
        "the slow tool was ended, not waited for"
    );
}

#[test]
fn a_leak_hands_over_the_path_to_the_gap() {
    let (_dir, job) = setup("world { LEAK }");
    let events = run(job, None);
    let path = events.iter().find_map(|e| match e {
        CompileEvent::LeakPath(points) => Some(points.clone()),
        _ => None,
    });
    assert_eq!(
        path,
        Some(vec![
            [-512.0, 0.0, 64.0],
            [0.0, 0.0, 64.0],
            [0.0, 128.0, 64.0]
        ])
    );
    assert_eq!(result(&events), &Err(CompileError::Leaked));
}

#[test]
fn a_leak_check_runs_vbsp_alone_and_installs_nothing() {
    let (_dir, mut job) = setup("world {}");
    job.profile = Profile::LeakCheck;
    let installed = job.installed_bsp();
    let events = run(job, None);
    let steps: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            CompileEvent::Step { tool, .. } => Some(*tool),
            _ => None,
        })
        .collect();
    assert_eq!(steps, ["vbsp"]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CompileEvent::Line(t, _) if t.contains("-leaktest")))
    );
    assert_eq!(result(&events).as_ref().unwrap().installed, None);
    assert!(!installed.exists());
}
