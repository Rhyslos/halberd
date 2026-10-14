# 0017. Shape operations

- **Status:** Accepted
- **Date:** 2026-10-14

## Problem

Blocking out a map means more than drawing boxes and pulling corners. Builders extrude a wall from a floor, bevel the edge of a step, join two brushes into one, cut a brush along a line, fill the space between two pieces, and mirror a half they built. Hammer has some of this (Clip, Carve, the vertex tool's merge), Blender most of it. Halberd needs them, and every result must still be a convex brush, because that is all Source compiles.

## Options

1. **A tool per operation**, each with its own mode, handles and settings (Hammer's Clip tool, for one).
2. **Operations that act on what is selected and picked**: select brushes, pick corners, edges or faces (decision 0016), then press a key or choose from a menu. Each operation is one undo step; its settings come from the grid.

## Decision

Option 2. The parts picked in Vertex, Edge and Face modes say where an operation acts, so no extra tool or handle is needed.

- **A Shape menu** (between Edit and View) lists every operation with its key. An operation that can't run yet is greyed out, and hovering it says what to pick first. Pressing its key then puts the same reason in the Console.
- **The operations:**
  - **Extrude faces (E):** stands a new brush on each picked face, one grid step deep, with the face's look. The new brush's far face is picked and the Move gizmo turns on, so dragging the arrow pulls it further, and E again stacks another.
  - **Bevel edges (Ctrl+B):** cuts each picked edge off with a slanted face starting one grid step back along both faces. It is refused, with the reason, if it would reach another corner.
  - **Bridge (F):** a new brush filling the space spanned by every picked corner, edge or face (their convex hull). For example, pick two facing faces to join two blocks with a beam.
  - **Split (K) and Clip (X):** cut every selected brush the cut passes through. Where the cut goes depends on what is picked:
    - a picked face: along its plane, so you can trim a post at the floor's top;
    - three or more corners: through the first three;
    - two corners or one edge: along that line as seen on screen, like drawing a clip line in Hammer's 2D view. From above, that is a straight-down cut.

    Split keeps both parts and selects them. Clip keeps the bigger part of each brush; to keep the smaller one, Split and delete the other.
  - **Join brushes (Ctrl+J):** the selected brushes become one brush, the convex hull of all their corners. If they did not fit together exactly, the Console says that the gaps were filled, because a brush can't have dents.
  - **Merge corners (M):** moves the picked corners together onto their middle, rounded to whole units.
  - **Delete (in Vertex or Face mode):** deletes the picked corners, or the picked faces. When a face goes, the faces around it grow to close the gap; this is refused when they can't (a box's side would leave it open). Edges can't be deleted on their own.
  - **Mirror:** mirrors the selected brushes about their middle. **Ctrl+M** flips them left to right as you see them; the menu also has X, Y and up-and-down.
- **Box picking:** in Vertex, Edge and Face modes, dragging draws a box that picks every corner, edge or face inside it (Ctrl adds to the pick), seen through the brushes as clicks are.
- **One undo step each,** named after what was done ("Extrude face", "Split 2 brushes", "Join 3 brushes"). Under the hood there is one edit, `ShapeEdit`, that changes, adds and removes brushes together.
- **Faces keep their look:** new faces copy the material and texture alignment of the face they came from, or of the old face lying most nearly the same way. Copies never take a saved face id or displacement, so ids are never repeated in the saved map. A face that did not change keeps everything.
- **Entities:** new brushes join the entity of the brush they came from. Split parts and extrusions of a `func_detail` brush stay in the `func_detail`.

## Consequences

- Sizes come from the grid (16 units unless changed in the settings file), so bevels and extrusions line up with everything else. A size field per operation can come later if needed.
- Every result is a convex brush. Join and Bridge fill in gaps rather than making shapes Source can't compile; the boolean operations milestone (union, subtract) is where non-convex results and automatic splitting into convex pieces come in.
- Mirrored brushes keep their textures but not mirrored texture alignment; that comes with per-face materials.
