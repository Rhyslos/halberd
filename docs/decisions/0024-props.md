# 0024. Props: reading models, the prop browser and placing props

- **Status:** Accepted
- **Date:** 2026-10-24

## Problem

A blockout needs props: barrels, crates, furniture. Source models are three files each (`.mdl`, `.vvd`, `.vtx`), Garry's Mod has thousands, and maps place them as `prop_static` (or `prop_dynamic`) entities with `model`, `origin` and `angles`. Halberd must list them with pictures, place them where you click, draw them in the views, and move them like brushes, without trusting the files.

## Options

For reading:

1. **A model library from crates.io.** The few that exist are incomplete (no `.vtx` triangles, or no fixups) and some panic on damaged files.
2. **Our own reader** in `halberd-mdl`, for what a picture needs: the full-detail level, the first skin, the first model of each body part.

For drawing:

1. **Separate GPU buffers per model**, drawn once per prop with its own transform. Fast for many props; needs a new pipeline and per-prop data.
2. **Props baked into the map's triangles** when the scene is rebuilt, through the same path as brushes, so the Flat, Textured and Wireframe modes, selection and textures all work unchanged.

## Decision

Option 2 for both.

- **Reading:** `Model::parse` reads the `.mdl` (materials, their folders, skins, body parts, meshes), the `.vvd` (vertices, with fixups for the full-detail level) and the `.dx90.vtx` (triangles), and checks that the three were made together (their checksums match). Every count and offset is checked before reading; damaged files give a plain reason, never a crash (fuzz-tested).
- **Materials:** each model material is looked for in the model's material folders, in order, as the engine does.
- **Listing:** every `models/*.mdl` in the archives and folders, listed once after mounting, on the background thread.
- **Pictures:** a model is read in the background when its tile is on screen, then drawn alone (Flat look, from the front and a little above) into a small image.
- **Placing:** clicking a prop picks it and turns on the **Place** tool: a click on the grid or a brush puts the prop there, standing on the surface, on the grid across. Models made for static props (`$staticprop`) are placed as `prop_static`, others as `prop_dynamic` (a `prop_static` with such a model would not show in the game). Esc stops placing.
- **Moving:** the gizmo moves and turns props (and other point entities) with brushes; turning a prop changes its `angles`.
- **Drawing:** props are baked into the map's triangles, turned and placed; their triangles are turned to face the way their normals do, so either winding a compiler wrote is drawn right. Textures use the model's own texture coordinates. Until a model is read, its props are small marker boxes. Picking and framing use the model's real box.

## Consequences

- A map with very many detailed props rebuilds slower after each edit, because every prop's triangles are rebuilt; separate per-model buffers can come later if it matters.
- Skins other than the first, body groups, animations and levels of detail are not shown.
- Model materials that blend several textures, or use a different shader look, show their base texture only.
