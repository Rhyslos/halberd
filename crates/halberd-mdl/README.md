# halberd-mdl

**Layer 0 · File formats**

Read Source models (MDL, VVD, VTX) into plain meshes.

## Purpose

Props are Source models split over MDL, VVD and VTX files. This crate decodes them into plain vertex and index lists that the renderer can upload.

## Must never

- Upload anything to the GPU
- Know about the editor's document or UI
- Panic on malformed input

## Depends on

none (no other Halberd crates)

## Status

Skeleton only. The roadmap milestone that fills this crate in will replace this line.
