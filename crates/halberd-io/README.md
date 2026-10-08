# halberd-io

**Layer 2 · Services**

Import and export VMF, save and load Halberd projects.

## Purpose

Converts between the document and files: VMF import and Export to Hammer, and the `.halberd` project format with versioned migrations.

## Must never

- Draw anything
- Write usernames or local paths into exported files
- Overwrite a file in place: write to a temporary file, then swap

## Depends on

`halberd-doc`, `halberd-geom`, `halberd-vmf`. Outside library: `glam`.

## Status

VMF import and export are implemented (see [decision 0011](../../docs/decisions/0011-vmf-maps.md)); the `.halberd` project format comes later.

- `document_from_vmf`: world brushes and entities become objects; everything else is kept (`MapFileData`, and each object's and face's `file_data`). Brushes whose planes do not make a shape are kept unchanged but not shown, with a note.
- `vmf_from_document`: puts everything back. Unchanged faces (checked exactly) keep their exact plane text, so an untouched map is written back byte for byte. New objects get fresh ids above every id in the file, hidden objects included; new maps get Hammer's standard blocks; new faces get `DEV/DEV_MEASUREGENERIC01B` with world-aligned texture axes.
- `open_map` (512 MB limit; UTF-8 with or without a byte-order mark, else Latin-1, so no byte changes) and `save_map` (temporary file, old file kept as `.vmx`, then swap; marks the map saved; a backup that cannot be written becomes a note, not a failure).

Tests: byte-for-byte round trip; moved brushes keep ids, materials and editor data with only their moved planes rewritten; deleting and undoing gives back the exact file; new maps are complete and their planes face outwards; ids never clash; broken brushes and brushes with repeated sides are kept; hidden ids and absurd ids; faces far from the middle stay unchanged; byte-order marks; saving a `.vmx` and a blocked backup; safe saving with backup; Latin-1 files; 2,000 randomly damaged maps never crash and whatever opens saves and reopens.
