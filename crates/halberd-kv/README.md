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

Skeleton only. The roadmap milestone that fills this crate in will replace this line.
