# 0011. Opening and saving Hammer maps (VMF)

- **Status:** Accepted
- **Date:** 2026-10-08

## Problem

Maps must open from and save to Hammer's VMF format, and the Phase 0 gate demands that "a Hammer-made VMF opens, shows correctly, and re-saves with nothing lost". Halberd understands only part of a VMF so far (brush shapes and entity positions); the rest (textures and their alignment, visgroups, entity settings and outputs, displacements, cameras, Hammer's own settings) must survive anyway.

## Options

1. **Convert everything into Halberd's own model**, and write VMF from that model. Clean, but anything not modelled yet is lost, and every future Hammer feature needs model work before maps stop losing it.
2. **Keep what is not understood, exactly as read**, next to what is. The document holds brushes and entities as objects, each carrying the file entries it does not use; the rest of the file is kept as it was. Writing puts it all back in place.

## Decision

- **Keep everything (option 2).**
  - `halberd-kv` reads KeyValues into a tree that keeps order, repeats and text exactly, and writes Hammer's layout (tabs, `\r\n` or `\n` as found).
  - `halberd-vmf` is a thin layer over that tree.
  - `halberd-io` converts to and from the document. World brushes become brush objects, each face carrying its material and all its other side entries. Entities become entity objects with all their other entries; their brushes become brush objects that name their entity (see [decision 0013](0013-brushes-inside-entities.md)). Hammer's top-level blocks and the world's settings go into `MapFileData`.
- **An untouched map saves back byte for byte.** Each face keeps its plane's original text and reuses it while the face stays on that plane, so only faces that actually moved are rewritten. "Unchanged" is checked exactly against the plane as read, not within a rounding margin, so faces far from the middle of the map (where rounding is coarse) are not rewritten either. Brushes keep their place before Hammer's `editor` block. The file's line endings and text encoding (UTF-8 with or without a byte-order mark, or Latin-1 for older files) are kept. A file mixing `\r\n` and `\n` comes back with one kind only.
- **Ids are kept** (overlays and decals refer to faces by id). New brushes, faces and entities get ids above any in use anywhere in the file, including hidden objects. Ids larger than Hammer can store are ignored.
- **Plane convention:** a side's normal is `(p0 − p1) × (p2 − p1)`, as Valve's compile tools use. Written planes use the three corners making the largest triangle on the face.
- **Brushes with broken shapes** (planes that do not close, or sides that add nothing to the shape, such as a repeated side) are kept unchanged and written back, but not shown; the Console says how many.
- **New maps** get the blocks Hammer writes (`versioninfo`, `visgroups`, `viewsettings`, world settings with a default sky, `cameras`, `cordons`), and new faces get Hammer's grey `DEV/DEV_MEASUREGENERIC01B` with world-aligned texture axes.
- **Saving is safe:** the text goes to a temporary file, the old file is kept as a `.vmx` backup (as Hammer does), then the new one takes its place. If the backup cannot be written, the save still happens and the Console says so; saving a `.vmx` file itself makes no backup.
- **File windows** use the `rfd` library (MIT): Windows' and macOS's own windows, and the desktop portal on Linux (no GTK needed). The workbench never touches files; it asks `halberd-app` through actions.
- **Unsaved work is protected:** the window title shows a dot, and New, Open, Quit and the window's close button ask "Save changes?" (Enter saves, N doesn't, Escape cancels; only fresh key presses without Ctrl or Shift count). While a box is showing, keys do not change the map behind it.

## Consequences

- Textures are kept and saved but not drawn until materials are loaded (Phase 1). Moving a brush keeps its texture alignment text as it was, so textures may look shifted in Hammer after a move; texture lock comes with materials.
- Entities can be selected and deleted, and their settings are shown, but not edited or moved until Phase 2.
- A map whose world brushes have other blocks between them saves with those blocks after the brushes: nothing is lost, but the order differs.
- Very large maps are limited to 512 MB files.
