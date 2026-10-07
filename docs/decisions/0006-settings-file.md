# 0006. Settings file format, location and safety

- **Status:** Accepted
- **Date:** 2026-10-07

## Problem

Halberd needs to remember settings between launches, starting with the Garry's Mod folder. Settings must survive crashes, hand edits and version changes without ever stopping the editor from starting.

## Options

1. **Windows registry or a binary file:** compact, but not hand-editable and Windows-only.
2. **JSON:** common, but no comments and easy to break by hand.
3. **TOML:** plain text, readable, allows comments, standard in the Rust world.

For the location: a folder next to the program (breaks when installed to Program Files) or the system's per-user settings folder.

## Decision

- **TOML**, in `Halberd/settings.toml` inside the system's per-user settings folder (`%APPDATA%`, `~/.config`, `~/Library/Application Support`).
- **Every field has a default and unknown fields are ignored,** so older and newer files both load.
- **Loading never fails.** Missing file: defaults. Damaged file: renamed to `settings.toml.damaged-N`, defaults used, a note explains it. Out-of-range values: corrected, with a note.
- **Saving is atomic:** write a temporary file, flush it to disk, rename it over the old one.
- **A file from a newer Halberd is never overwritten**, since saving would drop settings this version does not know.
- **The settings folder is found with a few lines of our own code** rather than the `dirs` library, whose dependency `option-ext` is MPL-2.0, a license outside our allowed list.

## Consequences

- Users can fix or reset settings by editing or deleting one text file.
- A format version number (`format_version`) is stored; renaming or changing the meaning of a field requires raising it and adding a migration.
