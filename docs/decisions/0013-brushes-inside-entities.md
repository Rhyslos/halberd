# 0013. Brushes inside brush entities

- **Status:** Accepted
- **Date:** 2026-10-09

## Problem

Many maps keep much of their detail in brush entities such as `func_detail`: one entity can hold hundreds of brushes. Halberd stored an entity's brushes inside the entity, so they could only be selected as a whole, and entities cannot be edited until Phase 2. A map like that could be looked at but not worked on. Hammer solves this with "Ignore groups": a click picks the single brush under the pointer instead of its entity.

## Options

1. **Point into the entity:** keep the brushes inside the entity, and let the selection name "brush 12 of entity 5". Every command, the gizmo and the panels would need to understand that second kind of address, and positions in the list shift when a brush is deleted.
2. **Brushes as objects of their own:** every brush, world or entity, is a document object with its own id. An entity's brush records which entity it belongs to, and the document keeps an index from each entity to its brushes. This is how Hammer itself stores them.

## Decision

Option 2.

- **Every brush is an object** with a permanent id. `BrushObject::entity` names its brush entity, or is empty for a world brush. `Document::brushes_of` lists an entity's brushes in file order.
- **Moving, resizing, rotating and deleting** a brush inside an entity uses the same commands as any brush. Undo and redo work the same way.
- **Picking:** a click picks the whole entity by default, as in Hammer. With **Inside entities** on (Ctrl+W, or the button in the viewport's toolbar), it picks the single brush. The Scene panel lists an entity's brushes under it, folded away until opened (or until one of them is selected), and clicking one there selects that brush.
- **A selected entity shows all its brushes selected.** Ctrl+clicking one of them (with Inside entities on) takes just that brush out: the entity's other brushes stay selected. Entities themselves still cannot be moved or edited until Phase 2, so the gizmo appears only for brushes.
- **Deleting:**
  - deleting an entity deletes its brushes;
  - deleting some of an entity's brushes keeps the rest;
  - deleting all of them deletes the entity too, because a brush entity without brushes is not valid in a map (Hammer does the same);
  - except when the entity also holds brushes Halberd could not show: those must be saved, so the entity stays;
  - the Edit menu names everything that goes ("Delete 401 objects" for a `func_detail` with 400 brushes).
- **Saving:** each entity's brushes are written back inside it, each in the place it was read from (a placeholder in the entity's kept entries marks the spot, so brushes Halberd could not show keep their place too). Each keeps its id, and an untouched map still saves byte for byte.
- **Shortcut keys** check the modifier keys held when the key went down, not at the end of the frame. Otherwise Ctrl+W released quickly would also count as W (the Move gizmo).

## Consequences

- Panels and tools see one kind of brush. Moving brushes between entities ("Tie to Entity", "Move to World") becomes a change of `entity`, ready for Phase 2.
- An entity's box (for framing, Properties and the selection) comes from the document (`Document::bounds_of`), since the entity object alone does not know its brushes.
- Opening a map reports every brush, including those inside entities, in its brush count.
