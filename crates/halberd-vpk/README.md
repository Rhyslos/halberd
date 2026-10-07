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

Skeleton only. The roadmap milestone that fills this crate in will replace this line.
