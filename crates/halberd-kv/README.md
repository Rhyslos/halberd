# halberd-kv

**Layer 0 · File formats**

Read and write Valve's KeyValues text format.

## Purpose

KeyValues is the nested `"key" "value"` text format behind VMF maps, VMT materials and Steam's VDF files. This crate turns that text into a plain tree and back, exactly, so other format crates can build on it.

## Must never

- Know what a map, material or Steam library is
- Touch the disk, GPU or UI
- Panic on malformed input

## Depends on

none (no other Halberd crates)

## Status

Reading and writing are implemented.

- `parse` turns text into a list of `Entry` values (`Pair` or `Block`), keeping order, repeated keys and blocks, and text exactly (no escapes; `//` comments skipped; a byte-order mark ignored). Errors give the line and a plain reason. Nesting is limited to 256 levels.
- `write` / `write_with` produce Hammer's layout: block names unquoted when possible, `{` and `}` on their own lines, tab indentation, `"key" "value"` pairs, Windows or Unix line endings. A `"` inside text cannot be written in this format and becomes `'`.
- `Block::get` and `Block::blocks` find pairs and blocks without regard to case, as Valve's tools do.

Tests: a Hammer file reads and writes back byte for byte; 20,000 random texts never crash the reader; 5,000 random trees write and read back unchanged.
