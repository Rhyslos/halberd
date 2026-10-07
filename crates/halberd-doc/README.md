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

`halberd-geom`

## Status

Skeleton only. The roadmap milestone that fills this crate in will replace this line.
