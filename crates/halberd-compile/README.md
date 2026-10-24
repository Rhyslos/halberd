# halberd-compile

**Layer 2 · Services**

Run the map compilers and report their progress.

## Purpose

Compile profiles (geometry only, quick test, final), running vbsp, vvis and vrad as separate processes, incremental change detection, and parsing their logs for the console.

## Must never

- Run anything through a shell
- Run compile tools from outside the configured GMod folder (the game itself is started through Steam, or its own program)
- Block the interface thread

## Depends on

`halberd-config`. For tests: `tempfile`.

## Status

Compiling and starting the game are implemented (see [decision 0028](../../docs/decisions/0028-quick-compile.md); leaks: [decision 0029](../../docs/decisions/0029-leak-path.md)).

- **`CompileJob`**: the map file, Garry's Mod's folders and its tools. `steps` checks them (each tool found, and inside Garry's Mod's folder; paths made absolute) and lists the runs for its `Profile` (`Quick`: `vbsp`, `vvis -fast`, `vrad -fast -ldr`; `LeakCheck`: `vbsp -leaktest` alone, nothing installed). `installed_bsp` is where the map goes for the game.
- **`CompileRunner`**: runs the steps on its own thread, one process at a time, never through a shell; passes on `CompileEvent`s (the step, each output line sorted by `LineKind`, the outcome); `cancel` ends the running tool. A leak stops the compile (`CompileError::Leaked`), after `CompileEvent::LeakPath` with vbsp's leak line when it left one (a stale pointfile is removed before each compile). The compiled map is copied into `garrysmod/maps`, with a marker beside it; a map of the same name Halberd didn't make is never replaced (`CompileError::WouldReplace`). Dropping the runner ends the running tool and waits for its thread.
- **`parse_pointfile`, `read_pointfile`** (`pointfile.rs`): vbsp's leak line (`<map>.lin`, one `x y z` per line), capped at 100,000 points and 16 MB; junk lines skipped; fuzz-tested.
- **`Launcher`**: starts Garry's Mod on a map through Steam (`-applaunch 4000 +map name`), or the game's own program.
- **`CompileError`**: why a compile didn't start or finish, in plain words.
- **`halberd-fake-compiler`**: a stand-in for vbsp, vvis and vrad, used only by the tests (`tests/compile.rs`).
