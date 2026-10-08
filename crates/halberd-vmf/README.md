# halberd-vmf

**Layer 0 · File formats**

Parse and write VMF map files losslessly.

## Purpose

VMF is the text map format Hammer saves. This crate reads a VMF into plain data (solids, sides, entities, groups, visgroups) and writes it back without losing anything, which is what makes Export to Hammer and import possible.

## Must never

- Know about the editor's document, GPU or UI
- Read or write files itself: it works on text it is given
- Silently drop data it does not understand

## Depends on

`halberd-kv`

## Status

A thin layer over `halberd-kv`, implemented.

- `Vmf::parse` (refuses text without a `world` block), `Vmf::to_text`, `world`, `entities`, `top_blocks`, `id_of`.
- Numbers: `parse_plane` (exactly three `(x y z)` points), `parse_vec3`, and `format_number` / `format_plane` writing numbers as Hammer does (whole numbers without decimals, others to six places).
- `tests/data/sample.vmf`: a small map in Hammer's exact layout (versioninfo, a visgroup, world brushes with full sides and editor blocks, a spawn point, a light, a `func_detail`, a `logic_relay` with outputs, cameras and cordons), used by the tests here, in `halberd-io` and by the window smoke test.
