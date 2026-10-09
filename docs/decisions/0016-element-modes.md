# 0016. Face, edge and vertex modes

- **Status:** Accepted
- **Date:** 2026-10-09

## Problem

Blocking out a map often needs shapes that are not boxes: a sloped roof, a ramp that narrows, a wall leaning in. Hammer's vertex tool reshapes a brush by dragging its corners. Halberd needs the same: pick the corners, edges or faces of a brush and move, rotate or scale them, without ever producing a brush Source can't compile.

## Options

1. **A separate vertex tool** with its own handles, as in Hammer.
2. **Selection modes** (Object, Vertex, Edge, Face) on the Select tool, reusing the transform gizmo on whatever is picked, as in Blender and Unity's ProBuilder.

For the reshaping itself:

- **(a) Move the corners and keep the faces as they were**, refusing any move that bends a face. Safe, but most useful moves (one corner up) bend a face, so most moves would be refused.
- **(b) Move the corners and rebuild the brush as the smallest convex solid around them** (the convex hull). A face that no longer lies flat folds into two; a corner pushed inside the shape disappears. Every result is a valid convex brush.

## Decision

Option 2 with (b).

- **Modes:** keys **1** (Object), **2** (Vertex), **3** (Edge) and **4** (Face), also buttons in the Select tool's second toolbar row ("Pick: Object 1 · Vertex 2 · Edge 3 · Face 4"), as the Draw tool has its shape row there. Picking a mode (even the same one again) switches to the Select tool and starts with nothing picked; switching to the Draw tool lets go of picked parts too.
- **What can be picked:** the corners, edges or faces of the selected brushes, and of the brushes of a selected brush entity (so a `func_detail`'s brushes can be reshaped without Inside entities). Corners and edges are drawn over the image, seen through the brush as in Hammer; the picked ones are orange, the one under the pointer yellow, and a picked face is filled.
- **Clicks:** a click picks the part under the pointer, Ctrl+click adds or removes it. A click on another object selects it instead (in Face mode, that includes an object standing in front of the face); a click on nothing lets go of the picked parts and keeps the brushes selected. With nothing selected, a line under the toolbar says to select a brush first.
- **The gizmo** (W, R, S, T) appears around the picked parts and moves, turns or stretches them, in grid steps and 15° turns like whole brushes. Each drag is one undo step, named after what was done ("Move vertex", "Rotate face", "Scale 3 edges"); Escape cancels it. A single corner has nothing to turn or stretch, so those handles do nothing for it.
- **Escape** lets go of the picked parts first; a second Escape deselects the brushes.
- **Delete** does nothing in Vertex, Edge and Face modes while brushes are selected (Edit → Delete is greyed out), with a note in the Console: deleting a corner or face is a shape operation for a later milestone, and deleting the whole brush there would surprise.
- **Picked parts are remembered by position,** not by number, because a brush's corners are numbered afresh whenever its shape changes. The gizmo moves the remembered positions along with the parts; parts that no longer exist (merged away, or undone) drop out of the pick.
- **Faces keep their materials:** each face of the new shape takes the material and saved data of the old face it shares most corners with. A face that folded or merged gets a new face id when saved (two faces must never share one; overlays refer to faces by id) and loses its displacement, which only fits the face it was made for. A face that only slid, rotated or tilted keeps everything, and faces that did not move keep their exact planes, so they save with the very same text.

## Consequences

- No reshaping can make an invalid brush; at worst a corner disappears into the shape, and Ctrl+Z brings it back.
- A drag that ends where it began (or turns a single corner) changes nothing and leaves nothing to undo: a brush rebuilt from unmoved corners is the very same brush, faces in the same order.
- Rounded shapes' corners are not exactly flat (they are rounded to whole units), so the hull treats points within 0.02 units of a side as on it, turns thin leaning slivers until every point is behind them, and drops slivers that add nothing when a shape would otherwise go over the 128-face limit. Tests check that no result bulges past its corners.
- A drag that would flatten the brush (all corners in one plane) stops at the last shape that worked.
- Moving one corner of a box straight up folds only its top; moving it sideways folds the two sides as well. This is what Hammer does too.
- Box-selecting many corners by dragging a rectangle, deleting parts, and merging corners on purpose come with the shape operations milestone.
