# 0033. Displacements

- **Status:** Accepted
- **Date:** 2026-10-24

## Problem

Outdoor ground, hills, cliffs and rubble in Source maps are displacements: a four-sided brush face turned into a grid of points that can be raised and lowered, with a blend amount at each point that mixes two textures (grass into dirt). Halberd kept displacements from opened maps unchanged but drew them flat and couldn't make or change them.

## Options

1. **Hammer's way:** a separate displacement dialog with create, paint geometry, paint alpha, sew, subdivide and many more tools.
2. **A small set in Halberd's own style:** make and remove displacements as shape operations on picked faces, and one Sculpt tool with three modes, drawn as real terrain in every view.

## Decision

Option 2. It covers what most maps need; the rest of Hammer's tools can be added to the same data.

- **Data** (`halberd-doc`, `displacement.rs`): a `Displacement` is read from and written to the face's `dispinfo` block: `power` (2, 3 or 4: grids of 5, 9 or 17 points a side), `startposition`, `elevation`, and per point `normals`, `distances`, `offsets` and `alphas`. Everything else in the block (flags, `offset_normals`, triangle tags…) is kept as read, so Hammer's own data survives. Broken blocks (a wrong power, short rows, numbers that aren't finite) are not read; the face is drawn flat and its block kept.
  - A grid is laid from the face corner nearest `startposition`, going round the face the way Source winds faces (`ordered_corners`).
  - **Heights are stored as Hammer stores them:** each point has a unit direction and a distance that is never negative, so a lowered point points down (and Hammer writes `0 0 0` for points it never moved). Halberd works with the height along the face's normal and writes the direction and distance back from it.
  - **Displacements follow their brush:** moving, turning, resizing, mirroring or reshaping a brush moves each displacement's start corner to the same corner of the new face (the one where the same neighbouring faces meet), and turns its directions and sideways offsets the way the face's edges turned. A mirror turns the face's winding round, so the grid's rows and columns are swapped back and the terrain comes out mirrored. Derived data that no longer fits (`offset_normals`, Hammer's walkable-triangle tags) is dropped for Hammer and the compiler to work out again.
- **Making and removing** are shape operations (Shape menu, Face mode 4, picked faces): **Make displacement** turns each picked four-sided face into a flat power 3 displacement (9 × 9 points, as Hammer does; a broken displacement block is replaced); faces with more or fewer corners are left with a note. **Remove displacement** turns them back into plain faces, including faces whose displacement can't be read. The faces stay picked.
- **The Sculpt tool** (G, or the toolbar) works on every displacement under the brush circle, across neighbouring displacements:
  - **Raise / lower:** hold the left button to raise, with Ctrl to lower. Points move most at the middle and less towards the edge.
  - **Smooth:** evens out bumps (each point moves towards its neighbours' average).
  - **Paint blend:** paints the face's second texture in, with Ctrl out.
  - **Size** (8 to 2,048 units) and **Strength** (units, or blend, per second) are in the tool's row. The brush reaches across the face, not through the raised surface, so lowering undoes raising exactly.
  - Holding still keeps working; a whole stroke is one undo step, even when it runs onto other brushes; Escape takes back the stroke in progress, and changing tool ends it.
  - World brushes and shapes in front of the terrain hide it from the brush.
  - The map's displacements are read once and kept while the map stays the same; during a stroke only the brushes that changed are read again, and faces out of reach are skipped by their bounds.
- **Drawing:** displacements are drawn as their grid of triangles, lit by their own slopes, with their grid lines, and the blend shown as a warm orange tint (drawing the second texture itself needs blended materials, which come later).
- **Saving** writes the block the way Hammer does. A new face that becomes a displacement is saved with all the usual side settings as well.

## Consequences

- Every square of the grid is split into triangles along the same diagonal; Source alternates the diagonal, so on very sharp terrain the shape drawn (and where the brush lands) can differ slightly from the game.
- Sewing (joining the edges of neighbouring displacements so no cracks show), changing a grid's power, and painting with a texture's actual second layer are not done yet.
- A brush reshaped so that its displacement face no longer has four corners keeps the displacement block, which the compiler will refuse; it is drawn flat as a warning.
- Undo steps that only change objects now merge during a drag even when the drag reaches more objects, which a sculpt stroke needs; other drags always reach the same objects, so they are unaffected.
