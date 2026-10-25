# 0027. Map size and engine limit meters

- **Status:** Accepted
- **Date:** 2026-10-24

## Problem

Source maps have hard limits. Everything must lie within ±16384 units of the middle, and the compiler stops when a map has too many brushes, brush sides, brush models, lights or materials; Garry's Mod allows 8192 networked entities while playing, and the map's own count towards them. Mappers usually find out at compile time, after hours of work.

## Options

1. **Read the counts from a compile.** Exact, but only after compiling, and compiling arrives later.
2. **Estimate the counts from the map as Halberd holds it**, live, and show them where the map's own properties belong.

## Decision

Option 2.

- **Where:** Properties with nothing selected shows the **Map**: its size (width × depth × height), how much of the largest possible map it spans, and **Engine limits** as meters.
- **What is counted** (`Document::stats`), following the 2013 compilers Garry's Mod uses:
  - **Brushes:** brushes and floor plan pieces, against 8192 (`MAX_MAP_BRUSHES`).
  - **Brush sides:** each brush's faces plus a side for each of the six box directions it lacks, as the compiler adds them, against 65,536 (`MAX_MAP_BRUSHSIDES`). Brushes slanted in two directions also get sides along their slanted edges, so for them the count is a lower bound.
  - **Map entities:** every entity in the map file, the world, `func_detail` and static props included (the compiler counts them all as it reads the file, before folding some away), against 8192 (`MAX_MAP_ENTITIES`). Garry's Mod also allows 8192 networked entities in the game, which the hover text mentions.
  - **Brush entities (models):** the world plus every brush entity except `func_detail`, `func_ladder`, `func_occluder` and areaportals, against 1024 (`MAX_MAP_MODELS`).
  - **Lights:** world lights as the lighting compiler makes them: one per `light` and `light_spot`, two for the first `light_environment` (sunlight and sky light), none for `light_dynamic` (a real entity in the game); against 8192. Glowing materials add more, which can't be counted from the map.
  - **Materials:** different materials on brushes, against 2048 (`MAX_MAP_TEXDATA`).
- **Colours:** green, yellow from three quarters, red when full. Hovering a meter says what happens past the limit and how to stay under it.
- **The edge:** brushes, shapes and props reaching past ±16384, and point entities whose origin is past it, are counted (a brush entity once, as the entity), with a button that selects them. The compiler doesn't stop for them, but cuts them off, and the game can't handle positions out there.
- Counted again only when the map changes, or a prop's model size arrives.

## Consequences

- The compiler's counts can differ a little: it splits brushes along water and areaportals, merges some faces, and drops some entities. The meters are labelled as estimates; the compile milestone can show the real numbers.
- Limits that only exist after compiling (planes, faces, leaves, lighting data) are not shown; nor is `MAX_MAP_TEXINFO` (12,288 combinations of material, alignment and lightmap scale), which needs the compiler's own reckoning.
- Brushes Halberd keeps but can't show (inside entities, as read) are left out of the brush, side and material counts; their entities still count as brush entities.
