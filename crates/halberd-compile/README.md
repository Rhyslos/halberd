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

Compiling and starting the game are implemented (see [decision 0028](../../docs/decisions/0028-quick-compile.md)).

- **`CompileJob`**: the map file, Garry's Mod's folders and its tools. `steps` checks them (each tool found, and inside Garry's Mod's folder; paths made absolute) and lists the runs for its `Profile` (`Quick`: `vbsp`, `vvis -fast`, `vrad -fast -ldr`). `installed_bsp` is where the map goes for the game.
- **`CompileRunner`**: runs the steps on its own thread, one process at a time, never through a shell; passes on `CompileEvent`s (the step, each output line sorted by `LineKind`, the outcome); `cancel` ends the running tool. A leak stops the compile (`CompileError::Leaked`). The compiled map is copied into `garrysmod/maps`.
- **`Launcher`**: starts Garry's Mod on a map through Steam (`-applaunch 4000 +map name`), or the game's own program.
- **`CompileError`**: why a compile didn't start or finish, in plain words.
- **`halberd-fake-compiler`**: a stand-in for vbsp, vvis and vrad, used only by the tests (`tests/compile.rs`).
