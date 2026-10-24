# halberd-vpk

**Layer 0 · File formats**

Read Valve VPK archives.

## Purpose

GMod and Half-Life 2 base content ships inside VPK archives. This crate lists and reads files inside them, in place.

## Must never

- Extract files to disk
- Load whole archives into memory when only one file is needed
- Trust sizes or offsets in the archive without checking them

## Depends on

none (no other Halberd crates)

## Status

Reading is implemented (see [decision 0022](../../docs/decisions/0022-game-content.md)).

- `Vpk::open` reads only a directory file's header and file list (versions 1 and 2; `name_dir.vpk` with numbered archives beside it, or a single-file `name.vpk`). Sizes are checked before anything is allocated: the file list must fit in the file and under 256 MB (`MAX_TREE_SIZE`), at most 2,000,000 files (`MAX_ENTRIES`).
- `Vpk::read` / `read_entry` read one file in place: its preloaded bytes from the list, the rest from its archive (or from after the list), checking the offset and length against the real file and the CRC-32 against the listing. Files over 1 GB (`MAX_FILE_SIZE`) are refused.
- Lookups (`entry`, `contains`) ignore case and accept `/` or `\\`; the first listing of a path wins.
- `VpkBuilder` writes small archives (single-file or split, version 1 or 2, with preloaded bytes), for tests and tools.
- Errors (`VpkError`) say what is wrong in plain words.

Tests: single and split archives, both versions and preloaded bytes read back; lookups in any case; damaged, cut-short and missing archives; bad headers; a single-file archive pointing at numbered archives; 3,000 randomly damaged directories never crash; a 50,000-file list opens quickly.
