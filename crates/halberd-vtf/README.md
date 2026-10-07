# halberd-vtf

**Layer 0 · File formats**

Read VTF textures and VMT materials.

## Purpose

Materials are VMT text files pointing at VTF textures. This crate reads both into plain data: texture pixels or compressed blocks, and material parameters.

## Must never

- Upload anything to the GPU
- Know about the editor's document or UI
- Panic on malformed input

## Depends on

`halberd-kv`

## Status

Skeleton only. The roadmap milestone that fills this crate in will replace this line.
