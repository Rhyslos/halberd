# halberd-doc

**Layer 1 · Core**

The map document: objects, selection, commands and undo.

## Purpose

The single source of truth for an open map: brushes, props, entities and groups with stable IDs, the selection, and the command system every edit goes through, which gives undo and redo.

## Must never

- Read or write files
- Render, draw UI or start threads
- Change state any way other than through a command

## Depends on

`halberd-geom`, `halberd-kv` (to keep file entries). Outside library: `glam`.

## Status

The document, commands and undo are implemented (see [decision 0009](../../docs/decisions/0009-document-commands-and-brushes.md)).

- **`Document`**: the objects (each with an `ObjectId` that never changes and is never reused), the selection, the undo history, and revision counters that tell views when to redraw.
- **`Command`**: every edit. So far: `AddBrushes`, `Remove` and `ReplaceBrush` (new size or position). `execute` checks it first; on error nothing changes.
- **`TransformBrushes`**: new shapes for several brushes at once, named after what was done ("Move 2 brushes", "Rotate brush", "Scale brush").
- **Cancelling a drag**: `discard_step` reverses and forgets an unfinished merged step, so a drag cancelled with Escape leaves nothing in the undo list.
- **One drag, one undo step**: `execute_merging` with the same key merges consecutive edits (for example, dragging a size field) until `end_step`, which the interface calls whenever no drag is in progress.
- **Undo and redo**: each applied command keeps the change it made (up to 1000 steps); undo reverses it, redo repeats it, with the same ids.
- **Objects**: `BrushObject` (a shape and, per face, its material and the file entries Halberd does not use yet) and `EntityObject` (class, origin, its own brushes, and its other entries). Point entities are shown as 16-unit boxes. `Document::from_map` builds a map read from a file; `MapFileData` keeps the rest of that file (Hammer's blocks, world settings, line endings, text encoding).
- **Saved state**: `is_modified` and `mark_saved`, for the title bar's dot and the "save changes?" question.
- **Selection**: `set_selection`, `toggle_selected`, `clear_selection`. Not an edit, so not undone, as in Hammer; an undo selects what it brings back.
- **Queries**: `pick` (nearest object along a ray), `selection_bounds`.

Tests include a stress test of 3,000 random adds, deletes, undos and redos.
