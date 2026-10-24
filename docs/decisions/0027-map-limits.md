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
- **What is counted** (`Document::stats`):
  - **Brushes:** brushes and floor plan pieces, against 8192 (`MAX_MAP_BRUSHES`).
  - **Brush sides:** each brush's faces plus a side for each of the six box directions it lacks, as the compiler adds them, against 65,536 (`MAX_MAP_BRUSHSIDES`).
  - **Map entities:** the world and every entity except `func_detail` (merged into the world) and `prop_static` (compiled into a list of its own), against 8192.
  - **Brush entities (models):** the world plus every brush entity except `func_detail`, against 1024 (`MAX_MAP_MODELS`).
  - **Lights:** `light`, `light_spot`, `light_environment` and `light_dynamic`, against 8192 (`MAX_MAP_WORLDLIGHTS`).
  - **Materials:** different materials on brushes, against 2048 (`MAX_MAP_TEXDATA`).
- **Colours:** green, yellow from three quarters, red when full. Hovering a meter says what happens past the limit and how to stay under it.
- **The edge:** objects reaching past ±16384 are counted, with a button that selects them.
- Counted again only when the map changes.

## Consequences

- The compiler's counts can differ a little: it splits brushes along water and areaportals, merges some faces, and drops some entities. The meters are labelled as estimates; the compile milestone can show the real numbers.
- Limits that only exist after compiling (planes, faces, leaves, lighting data) are not shown.
