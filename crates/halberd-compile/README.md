# halberd-compile

**Layer 2 · Services**

Run the map compilers and report their progress.

## Purpose

Compile profiles (geometry only, quick test, final), running vbsp, vvis and vrad as separate processes, incremental change detection, and parsing their logs for the console.

## Must never

- Run anything through a shell
- Run executables from outside the configured GMod folder
- Block the interface thread

## Depends on

`halberd-config`

## Status

Skeleton only. The roadmap milestone that fills this crate in will replace this line.
