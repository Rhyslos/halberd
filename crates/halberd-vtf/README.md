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

Textures and materials are read (see [decision 0023](../../docs/decisions/0023-materials.md)).

- **`Vtf::parse`** reads a VTF texture's header (versions 7.0 to 7.5, with the resource table of 7.3 and later) and checks every size and offset against the file before anything is read. `width`, `height`, `format` (`ImageFormat`, all 27 Source formats named) and `mip_count` describe it.
- **`Vtf::decode(max_size)`** gives the first frame and face as RGBA (`RgbaImage`), from the largest stored size no bigger than `max_size` on either side. DXT1, DXT3, DXT5 and the uncompressed formats (RGBA, BGRA, RGB, BGR, 565, 4444, 5551, I8, IA88, A8 and others) are decoded; palette and HDR formats give `UnsupportedFormat`.
- **`Material::parse`** reads a VMT (through `halberd-kv`): its shader, `param` (any case, `$` included), `include` for `patch` materials, `texture_path` (`$basetexture` as `materials/x/y.vtf`, slashes tidied) and `is_tool` (materials marked `%compilenodraw`, `%compileclip`, `%compiletrigger` or `%tooltexture`, such as nodraw and clip).
- Damaged files give a plain-language `VtfError`, never a crash; both readers are fuzz-tested (`tests/textures.rs`).
