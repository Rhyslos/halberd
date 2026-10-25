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

Models are read (see [decision 0024](../../docs/decisions/0024-props.md)).

- **`Model::parse(mdl, vvd, vtx)`** reads a model's three files and checks that they belong together (matching checksums). It gives the model's name, whether it is a static prop (`$staticprop`), its material names and folders, its bounds, and its meshes: plain vertices (position, normal, texture coordinates) and triangle indices, with each mesh's material through the first skin. The full-detail level and the first model of each body part are read. MDL versions 44 to 49, VVD version 4 (fixups followed) and VTX version 7 (both strip group layouts) are supported.
- Counts and offsets are checked before anything is read or allocated, including totals across the file (structures visited, vertices claimed by all meshes; `MAX_VERTICES`), since parts may point at the same data again and again; damaged files give a plain-language `MdlError`, never a crash (fuzz-tested in `tests/models.rs`).
- **`example`**: writes small valid model files in code (`model_files`, `box_mesh`), for tests and for trying the prop browser without the game.
