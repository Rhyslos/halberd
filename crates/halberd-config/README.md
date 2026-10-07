# halberd-config

**Layer 1 · Core**

Settings, key bindings and memory budgets.

## Purpose

Editor settings: key bindings, camera speeds, memory and GPU budgets, GMod install path. Loaded and saved as plain text.

## Must never

- Know about the UI or the document
- Lose the user's settings: saves go to a temporary file first

- Crash or refuse to start because of a bad settings file: fall back to defaults and explain why
- Overwrite a settings file written by a newer Halberd

## Depends on

No other Halberd crates. Outside libraries: `serde`, `toml`.

## Status

Settings file implemented: game folder, memory budget, worker threads, autosave interval, grid size, WASD preference.

- File: `Halberd/settings.toml` in the system's settings folder (`%APPDATA%` on Windows, `~/.config` on Linux, `~/Library/Application Support` on macOS).
- Out-of-range values are corrected on load, with a note.
- A damaged file is renamed to `settings.toml.damaged-N` and defaults are used.
- Key bindings and camera speeds arrive with the viewport milestones.
