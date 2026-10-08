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

`halberd-geom`. Outside library: `glam`.

## Status

The document, commands and undo are implemented (see [decision 0009](../../docs/decisions/0009-document-commands-and-brushes.md)).

- **`Document`**: the objects (each with an `ObjectId` that never changes and is never reused), the selection, the undo history, and revision counters that tell views when to redraw.
- **`Command`**: every edit. So far: `AddBrushes` and `Remove`. `execute` checks it first; on error nothing changes.
- **Undo and redo**: each applied command keeps the change it made (up to 1000 steps); undo reverses it, redo repeats it, with the same ids.
- **Selection**: `set_selection`, `toggle_selected`, `clear_selection`. Not an edit, so not undone, as in Hammer; an undo selects what it brings back.
- **Queries**: `pick` (nearest object along a ray), `selection_bounds`.

Tests include a stress test of 3,000 random adds, deletes, undos and redos.
