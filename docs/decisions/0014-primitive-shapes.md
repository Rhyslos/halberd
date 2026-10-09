# 0014. Ready-made shapes

- **Status:** Accepted
- **Date:** 2026-10-09

## Problem

Blocking out a map with boxes alone is slow: a ramp, a pillar, a dome, a doorway or a staircase each take many boxes, or Hammer. Phase 1 calls for wedges, cylinders, cones, spheres, arches and stairs. Each has choices to make: how it fits what was drawn, which way it faces, how round it is, and how to keep it compiling cleanly.

## Options

1. **A separate tool per shape.** Clear, but seven tools crowd the toolbar, and drawing works the same way for all of them.
2. **One Draw tool with a shape picker**, like Hammer's Block tool with its "Objects" list. Every shape is drawn by the same drag; the second toolbar row picks the shape and shows only the settings that shape has.

## Decision

Option 2. The Box tool becomes **Draw** (still B), with a **Shape** picker; Box stays the default.

- **Every shape fills the drawn box** (the drag on the ground plus the height field), so drawing one is the same as drawing a box.
- **Keys:**
  - **B** picks Draw; **B** again opens the shape list (arrow keys move, Enter or Space picks, Escape closes); **Q** goes back to Select.
  - **R** while drawing turns the shape a quarter turn clockwise (seen from above) inside its box; the gizmo keys wait until the drawing is done.
  - **Shift** while drawing makes the shape as wide, deep and tall as the drag's longest side, from the corner where it started; **Alt** does the same around the starting point, which becomes the middle of its ground plan (the shape still stands on the ground). On some Linux desktops Alt+drag moves the window instead.
- **Direction comes from the drag:** wedges and stairs climb, and arches span, along the drag's longer side, towards where it ended. Drawing stairs from bottom to top therefore does what one expects, and nothing else needs choosing. The gizmo can turn shapes afterwards.
- **Shapes:**
  - **Wedge:** a ramp, full height at one end.
  - **Cylinder** and **cone:** 3 to 64 flat sides (8 by default), fitted to the ellipse inside the box. A 4-sided cylinder fills the box.
  - **Sphere:** 4 to 16 sides around (an even number) and half as many bands. 16 is the most that stays within a brush's 128-face limit.
  - **Arch:** a doorway or bridge arch, a half ring standing on its two feet and spanning the box, as thick as chosen (16 by default). One brush per side, because a curved piece is not convex. (Hammer's own arch lies flat and usually has to be turned upright first.)
  - **Stairs:** solid steps of about the chosen height (8 by default; a GMod player climbs up to 18), one brush per step, each reaching down to the floor. Heights that don't divide evenly are spread so every step stays on whole units.
- **Whole units:**
  - Every corner is rounded to whole units and kept inside the drawn box, as Hammer does, so maps compile without hairline gaps.
  - Where rounding would dent a round outline, the dented point is left out, so the shape stays convex and its corners stay exact. So is a point where the outline turns by less than about a degree (along a long oval's sides), because sides that are nearly parallel meet at a corner that drifts off its whole unit.
  - A sphere's points are rounded too; its bands are then not perfectly flat, so where they meet a corner can fall between units (Hammer's spheres do the same).
  - Brush corners within 0.01 of a whole unit are now snapped onto it (it was 0.001), which removes the tiny float errors of slanted faces.
- **Refused, never broken:**
  - a shape too small for its sides or steps is refused with a reason ("Draw it bigger, or use fewer sides");
  - so is a tiny round shape whose rounded corners would poke out of its box;
  - stairs that would need more than 256 steps are refused ("use taller steps"), instead of quietly getting steps too tall to climb.
- **Preview:** while dragging, the viewport outlines the actual shape, not just its box. If the shape can't be made, the reason shows next to the Draw settings before the mouse is let go. The shape is built once per change of the drag or settings, not every frame.

## Consequences

- One Draw tool to learn. New shapes later (a torus, say) are one more entry in the picker.
- Shapes are ordinary brushes: they can be moved, turned, resized, deleted and saved like any other. Shapes made of several brushes are selected together right after drawing.
- Very thin shapes with many sides can end up with a corner a hair off a whole unit, because many faces meet at sharp angles. They still save and compile.
