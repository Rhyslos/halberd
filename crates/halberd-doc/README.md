# halberd-doc

**Layer 1 · Core**

The map document: objects, selection, commands and undo.

## Purpose

The single source of truth for an open map: brushes, props, entities and groups with stable IDs, the selection, and the command system every edit goes through, which gives undo and redo.

## Must never

- Read or write files
- Render, draw UI or start threads
- Change state any way other than through a command

## Depends on

`halberd-geom`, `halberd-kv` (to keep file entries). Outside library: `glam`.

## Status

The document, commands and undo are implemented (see [decision 0009](../../docs/decisions/0009-document-commands-and-brushes.md)).

- **`Document`**: the objects (each with an `ObjectId` that never changes and is never reused), the selection, the undo history, and revision counters that tell views when to redraw.
- **`Command`**: every edit. So far: `AddBrushes`, `Remove` and `ReplaceBrush` (new size or position). `execute` checks it first; on error nothing changes.
- **`TransformBrushes`**: new shapes for several brushes at once, named after what was done ("Move 2 brushes", "Rotate brush", "Scale brush").
- **`ShapeEdit`**: one undo step for a shape operation: brushes changed (keeping their entity), added (each joining a given brush entity, and joining the selection) and removed (with what must go with them), named by its label. Undoing brings removed objects back into the selection. `FaceInfo::copied` and `BrushObject::copied_data` give face and brush data for copies (no saved ids or displacements), `BrushObject::with_faces` builds a brush from a shape and its face data. `TransformKind` also has Mirror, Merge and Delete.
- **`EditBrushes`**: new shapes from moving some corners, edges or faces (`ElementKind`), each with what became of every face, named "Move vertex", "Rotate face", "Scale 3 edges". `BrushObject::with_shape_mapped` keeps each face's material and file data; a face that folded or merged loses its saved id (a new one is given on saving) and its displacement.
- **Floor plan shapes** (`ShapeObject`, `Object::Shape`, see [decision 0019](../../docs/decisions/0019-floor-plan-shapes.md)): an outline that may have dents, standing between two heights, with one look for its faces. Its convex pieces are worked out when it is made, for drawing, picking and saving. `AddShape` adds one ("Create floor plan shape"); `TransformBrushes` moves shapes along with brushes (`shapes`; "Move shape", "Move 3 objects"); `mapped` moves one, keeping its corners on whole units; a shape is never replaced by a brush, but `ShapeEdit` can remove it. A shape read from a file remembers its pieces' ids (`PieceIds`, `with_file_ids`), kept through moves while it splits the same way; `copied` drops them.
- **Texture alignment** (`texture.rs`): `FaceInfo::texture_axes` reads a face's saved `uaxis`/`vaxis` (`TextureAxes`, `TextureAxis::parse` for Hammer's `[x y z offset] scale`), or gives Hammer's world-aligned axes when nothing usable is saved (axes that no longer fit the face are refitted, keeping their scale: `refitted_texture`; damaged scales and shifts are ignored); `to_hammer` writes them back; `texels` turns a world point into texture coordinates.
- **Cancelling a drag**: `discard_step` reverses and forgets an unfinished merged step, so a drag cancelled with Escape leaves nothing in the undo list.
- **One drag, one undo step**: `execute_merging` with the same key merges consecutive edits (for example, dragging a size field) until `end_step`, which the interface calls whenever no drag is in progress.
- **Undo and redo**: each applied command keeps the change it made (up to 1000 steps); undo reverses it, redo repeats it, with the same ids.
- **Objects**: `BrushObject` (a shape and, per face, its material and the file entries Halberd does not use yet) and `EntityObject` (class, origin, and its other entries). A brush entity's brushes (a `func_detail`'s, say) are objects of their own that name their entity (`BrushObject::entity`); `Document::brushes_of` lists them, `Document::selectable` says whether a click picks the brush or its entity, and `Document::bounds_of` gives any object's box. Deleting an entity deletes its brushes, and deleting all of an entity's brushes deletes the entity. Point entities are shown as 16-unit boxes. `Document::from_map` builds a map read from a file (`MapObject`: world brushes, and entities with their brushes); `MapFileData` keeps the rest of that file (Hammer's blocks, world settings, line endings, text encoding).
- **Saved state**: `is_modified` and `mark_saved`, for the title bar's dot and the "save changes?" question.
- **Selection**: `set_selection`, `toggle_selected`, `clear_selection`. Not an edit, so not undone, as in Hammer; an undo selects what it brings back.
- **Materials** (`document/materials.rs`): `SetMaterial` puts a material on whole objects or on chosen faces ("Apply material", "Apply material to 3 faces"); an entity passes it to its brushes, a shape sets its one look. `AddBrushObjects` adds brushes with their face data (new brushes drawn with a chosen material). `BrushObject::set_material` and `as_new` help build them.
- **Entities** (`entity.rs`, `document/entities.rs`): `EntityObject::point` makes a point entity with its settings; `setting`, `set_setting`, `model`, `angles`, `rotation` (Source's pitch, yaw, roll; `rotation_of`), `with_origin` and `with_rotation` (both keep the saved settings in step) and `placed_box`. `AddEntity` adds one ("Place prop", "Add light"); `TransformBrushes` moves and turns point entities too (`entities`; "Move entity"). `set_model_box` records a model's box once it is read, so props are picked and framed by their real size (not an edit).
- **Settings** (`document/world.rs`): `SetEntitySettings` changes an entity's settings (never its class or origin; "Change entity settings"); `SetWorldSetting` changes one of the world's (`world_setting` reads them; "Change sky"), as `Change::World`, merging while a value is dragged. `class_box` gives the box a point entity is shown as (player-sized for spawns); `EntityObject::facing` reads a light's pitch as the compiler does (its `pitch` setting, or the pitch in `angles` when that is missing or 0; negative points down), and `rotation` and `with_rotation` use it for lights, so the gizmo tilts them (writing `pitch` and `angles` together, as Hammer does).
- **Map stats** (`stats.rs`, see [decision 0027](../../docs/decisions/0027-map-limits.md)): `Document::stats` gives the map's bounds, the objects reaching past ±16384 (`MAP_HALF_SIZE`), and `Meter`s estimating brushes, compiled brush sides, entities, brush models, lights and materials against Source's limits.
- **Queries**: `pick` (nearest object along a ray), `selection_bounds`.

Tests include a stress test of 3,000 random adds, deletes, undos and redos.

**Brush entities and classes** (`document/tie.rs`, [decision 0031](../../docs/decisions/0031-entity-definitions.md)): `TieToEntity` ties brushes (or a brush entity's brushes) to a new brush entity, `MoveToWorld` gives them back to the world (a brush entity left with no brushes goes), `SetEntityClass` changes a class, `RemoveEntitySettings` removes keys (never the class or origin). A brush moving to another entity is re-filed when a change is applied. `set_class_box` records a point class's size from the entity definitions; `entity_box` gives an entity's box (its model's, else its class's, else the marker).

**Displacements** (`displacement.rs`, `displacement/follow.rs`, [decision 0033](../../docs/decisions/0033-displacements.md)): `Displacement` (power, start corner, elevation, and per point normals, distances, offsets and blend) read from and written to a face's `dispinfo` block, keeping what it doesn't read; `flat`, `points`, `base_points`, `raise` and `height` (along the face's normal, stored as Hammer stores it), `set_alpha`, `forget_derived`; `ordered_corners` lays a grid on a face as Source does; `FaceInfo::displacement`, `with_displacement`. `FaceInfo::has_dispinfo`. `BrushObject::with_shape` keeps each displacement on its face (start corner, directions and offsets follow the brush; a mirror swaps the grid's rows and columns so the terrain is mirrored). Undo steps that only change objects merge during a drag even when later edits reach more objects.

**Connections** (`connection.rs`, `document/connections.rs`, [decision 0032](../../docs/decisions/0032-entity-io.md)): `Connection` (output, target, input, parameter, delay, times) read from and written to an entity's `connections` block (commas or ESC; unreadable lines kept; capped and fuzz-tested); `EntityObject::connections`, `with_connections`, `name`; `SetConnections`; `target_matches` (case-blind, `*` wildcard), `is_special_target` (`!player`…); `Document::targets_of`, `incoming`, `connection_links`.
