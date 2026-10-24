# halberd-fgd

**Layer 0 · File formats**

Read FGD entity definition files.

## Purpose

FGD files list every entity a game supports, with its keyvalues, inputs and outputs. This crate parses them so the editor can offer GMod's real entity list.

## Must never

- Know about the editor's document, GPU or UI
- Panic on malformed input

## Depends on

none (no other Halberd crates). Outside libraries: none (`tempfile` in tests).

## Status

Implemented (see [decision 0031](../../docs/decisions/0031-entity-definitions.md)).

- **`parse(text, file)`** reads one file; **`load(path)`** reads a file and its `@include`s (beside it only; included files first; each file once; at most 64 files, 16 MB each).
- **`Fgd`**: point and solid classes in file order (`placeable`), base classes kept apart (Valve has both `Light` and `light`), `map_size`, `problems` (file, line, what was wrong), `files` read.
- **`EntityClass`**: name, kind (`ClassKind::Base`, `Point`, `Solid`), description (joined across `+`), bases, helpers (`size`, `color`, `studio`… with their arguments), keys (`KeyDef`: name, `KeyType`, label, default, description, choices or flags, read-only), inputs and outputs (`IoDef`).
- **`Fgd::resolved(name)`** gives a `ResolvedClass` with its bases' keys, inputs, outputs and helpers included (each once, the class's own winning; loops and missing bases skipped), with `size()`, `color()` and `model()`.
- Never panics: a class that can't be read is reported and skipped. Tokens, items per class and choices are capped. Tests: unit tests, a fuzz test, and `tests/fgd.rs` reading `tests/data` (FGD files written for these tests in the Source SDK's style).
