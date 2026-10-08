# 0009. Map document, commands, undo and brushes

- **Status:** Accepted
- **Date:** 2026-10-08

## Problem

The first editable objects (box brushes) need a home: how the map is stored, how every edit can be undone, how brushes are represented, and how the tools, panels and renderer reach the map without tangling the crates.

## Options

1. **Undo by snapshots:** copy the whole map before every edit. Simple, but memory grows with map size × history length; a large map with 1000 undo steps would not fit.
2. **Undo by commands that record their own reversal:** each edit is a `Command`; applying it produces a `Change` holding exactly what is needed to reverse it (for a delete, the removed objects). Memory grows only with what changed.
3. **Brushes as boxes (min and max corners):** trivial now, but rotation, clipping and VMF faces would force a rewrite soon.
4. **Brushes as planes:** the same as Source and Hammer's VMF files, which store each face as a plane; face polygons are worked out from the planes. Works for any convex shape.

## Decision

- **Commands with recorded changes (option 2).** `Document::execute` checks a command, applies it, and keeps the `Change` on the undo list (up to 1000 steps). Undo applies the inverse change; redo re-applies the change. Objects keep their `ObjectId` through undo and redo, and ids are never reused.
- **The selection is part of the document but not an edit** (as in Hammer): selecting is not undone. An undo selects what it brings back.
- **Brushes are sets of planes (option 4).** `halberd-geom` cuts each face's polygon from a huge square using the other planes, in double precision, and snaps corners within 0.001 of a whole unit. This matches VMF, which arrives two milestones later.
- **Tools ask, the interface acts.** Tools in `halberd-tools` return a `ToolAction` (select, toggle, execute a command, or refuse with a reason) instead of changing the map, so they stay pure and testable. The workbench in `halberd-ui` owns the document and carries actions out.
- **The renderer reads the document directly** and rebuilds its GPU buffers only when `Document::revision` or `selection_revision` changes.
- **Milestone order changed:** the tracker had selection and gizmos before undo and brushes, but there is nothing to select without brushes. The document, undo, box drawing, selection and delete come first; the W / R / S / T gizmo follows.

## Consequences

- Every future edit (move, resize, clip, entity changes) is a new `Command` variant with its `Change`; the history, Edit menu and shortcuts work for it automatically.
- Boxes are drawn on the grid or on top of the brush under the pointer, 128 units tall; resizing comes with the gizmo. A drag along a single grid line makes a wall one grid square thick.
- Maps are not saved to disk yet; VMF import and export is the next milestone after the gizmo.
- Picking checks every object; fine for thousands of brushes, but large maps will need a spatial index later.
- A ray that starts inside a brush ignores that brush, so a camera inside a large brush can still pick what is in front of it (as in Hammer).
- Two brushes in exactly the same place: a click always picks the first. Clicking again to cycle through them (as Hammer does) can come later.
