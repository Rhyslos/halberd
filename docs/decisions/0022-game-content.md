# 0022. Reading game content from VPK archives

- **Status:** Accepted
- **Date:** 2026-10-24

## Problem

Materials and models for the browsers, the textured view and props come from the game: Half-Life 2's and Garry's Mod's base content is inside VPK archives (`sourceengine/hl2_textures_dir.vpk` and others), the rest in folders (`garrysmod/`, addons). Halberd must read them in place, as the game does, without unpacking anything and without trusting the files.

## Options

1. **A VPK library from crates.io.** The few that exist are small and unmaintained, and some read whole archives into memory.
2. **Our own reader** in `halberd-vpk` (the format is simple: a header, a tree of names, and offsets into numbered archives), plus mounting in `halberd-assets`.

## Decision

Option 2.

- **Reading:** only a directory file's list is read when mounting; a file's bytes are read when asked, checked against the real file sizes and the archive's CRC-32. Every size and count is checked before allocating, and damaged archives give plain-language errors, never a crash (fuzz-tested).
- **Mounting** follows `gameinfo.txt`: its `game` search paths, in order, with `|gameinfo_path|` and `|all_source_engine_paths|` filled in and `addons/*` expanded. The first place that has a file wins, as in the engine. Paths that would leave the install are never followed.
- **In the background:** the app mounts on its own thread so the window opens at once; the Console reports what was found.

## Consequences

- Workshop items (`.gma` files) and other games mounted through `mount.cfg` are not included yet; they come with the Workshop and mounted content milestone in Phase 2.
- Loose folders are not indexed yet (looked into only when a file is asked for); the browsers will add that.
