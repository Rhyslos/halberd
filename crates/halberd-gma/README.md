# halberd-gma

**Layer 0 · File formats**

Read Workshop GMA archives.

## Purpose

Workshop addons are downloaded as GMA archives (older ones as LZMA-compressed .bin files). This crate lists and reads files inside them, in place.

## Must never

- Extract files to disk
- Accept file paths that escape the archive (such as ../)
- Trust sizes or offsets in the archive without checking them

## Depends on

none (no other Halberd crates)

## Status

Skeleton only. The roadmap milestone that fills this crate in will replace this line.
