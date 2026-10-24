# 0028. Compile and play

- **Status:** Accepted
- **Date:** 2026-10-24

## Problem

A blockout is tested by playing it. Halberd should compile the map with Valve's own tools (vbsp, vvis, vrad, which ship with Garry's Mod on Windows) and start the game on it, in one key press, without freezing the editor or touching the user's map file.

## Options

1. **Compile the saved file in place**, as Hammer does. Needs a save first, and leaves `.bsp`, `.prt`, `.lin` and log files beside the user's map.
2. **Write the map as it is now to a work folder** and compile there; copy the result into Garry's Mod's `maps` folder.

## Decision

Option 2.

- **File → Compile and play (F9):** the tools are checked first; then the map, unsaved changes included, is written to a work folder of this Halberd's own inside Garry's Mod's game folder (`garrysmod/halberd_compile/<process number>`, so two Halberds never share files and the compilers, which can't read every path, get a plain one; folders a week old from earlier sessions are removed), named after the map file (letters, digits and underscores, so the game's `map` command accepts it; `untitled` for a new map).
- **Quick test profile:** `vbsp`, then `vvis -fast`, then `vrad -fast -ldr`, each with `-game` pointing at Garry's Mod's game folder.
- **Running:** each tool is its own process, never through a shell, and only from inside Garry's Mod's folder (paths are made absolute). Output goes to the Console line by line, warnings and errors coloured. A small window shows the step and the time, with **Stop compiling** (the running tool is ended). Closing the editor ends a compile too (the running tool is ended before Halberd closes).
- **A leak** (vbsp prints `**** leaked ****`) stops the compile with a plain explanation; showing where it leaks is the next milestone.
- **When it's done** the map is copied to `garrysmod/maps/`, with a small marker file beside it; a map of the same name that Halberd didn't make (a stock map, say) is never replaced, and the Console says to save under another name. Then and Garry's Mod is started on it through Steam (`-applaunch 4000 +map name`) when Steam was found, else through the game's own program; if neither can be started, or the game was already open (Steam doesn't pass the map to a running game), typing `map name` in the game's console loads it, as the Console says.
- **Tested** with a stand-in compiler (`halberd-fake-compiler`, built only for tests) copied into a made-up Garry's Mod folder.

## Consequences

- Garry's Mod for Linux and macOS has no compile tools; there the Console says so, and that they come with the Windows version.
- Only the quick profile exists; a final profile (full visibility and lighting, HDR) and packing custom content (bspzip) come with the Phase 2 compile profiles.
- The last compile's files stay in the work folder for checking, until a week has passed.
