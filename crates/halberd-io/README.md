# halberd-io

**Layer 2 · Services**

Import and export VMF, save and load Halberd projects.

## Purpose

Converts between the document and files: VMF import and Export to Hammer, and the `.halberd` project format with versioned migrations.

## Must never

- Draw anything
- Write usernames or local paths into exported files
- Overwrite a file in place: write to a temporary file, then swap

## Depends on

`halberd-doc`, `halberd-geom`, `halberd-vmf`. Outside library: `glam`.

## Status

VMF import and export are implemented (see [decision 0011](../../docs/decisions/0011-vmf-maps.md)); the `.halberd` project format comes later.

- `document_from_vmf`: world brushes, entities and the entities' brushes become objects (each entity brush naming its entity); everything else is kept (`MapFileData`, and each object's and face's `file_data`). Brushes whose planes do not make a shape are kept unchanged but not shown, with a note.
- `vmf_from_document`: puts everything back. Unchanged faces (checked exactly) keep their exact plane text, so an untouched map is written back byte for byte. New objects get fresh ids above every id in the file, hidden objects included; new maps get Hammer's standard blocks; new faces get `DEV/DEV_MEASUREGENERIC01B` with world-aligned texture axes.
- **Floor plan shapes** (`shapes.rs`) are saved as their convex pieces, ordinary world brushes any editor can use, each with `halberd_shape` (a number per shape) and `halberd_outline` (`"x y x y … | bottom top"`) in its `editor` block, and an orange editor colour. On opening, pieces with the same number become the shape again only if they are exactly as Halberd wrote them: the same pieces face for face (planes compared one to one), one material, world-aligned textures, the standard side settings and editor block. Otherwise (moved, reshaped, retextured, deleted, copied or grouped elsewhere) they open as ordinary brushes with the edits kept and the marks removed, and a note says so. A reopened shape keeps its pieces' solid and side ids (`PieceIds`), so they don't change from save to save. Rebuilding stops after 100,000 outline corners per file.
- `open_map` (512 MB limit; UTF-8 with or without a byte-order mark, else Latin-1, so no byte changes) and `save_map` (temporary file, old file kept as `.vmx`, then swap; marks the map saved; a backup that cannot be written becomes a note, not a failure).

Tests: byte-for-byte round trip; moved brushes keep ids, materials and editor data with only their moved planes rewritten; deleting and undoing gives back the exact file; new maps are complete and their planes face outwards; ids never clash; broken brushes and brushes with repeated sides are kept; hidden ids and absurd ids; faces far from the middle stay unchanged; byte-order marks; saving a `.vmx` and a blocked backup; safe saving with backup; Latin-1 files; 2,000 randomly damaged maps never crash and whatever opens saves and reopens. `tests/shapes.rs`: shapes save as convex world brushes with unique ids, come back as the same shapes and save to the very same file, and stay brushes when a piece was changed or the outline note is broken. `tests/shape_edits.rs`: a deleted piece never captures another shape later, a reshape with the same box, a retextured face and a copied shape all stay brushes with the edits kept, ids stay the same from save to save, and 1,000 damaged shape maps never crash and save the same twice.
