# halberd-render

**Layer 3 · Front end**

GPU rendering of the viewports.

## Purpose

Draws each viewport with wgpu: brushes, props, entities, gizmos and grids, updating only the GPU data for what changed.

## Must never

- Change the document
- Decode files itself: it gets plain meshes and textures from other crates

## Depends on

`halberd-doc`, `halberd-geom`

## Status

Skeleton only. The roadmap milestone that fills this crate in will replace this line.
