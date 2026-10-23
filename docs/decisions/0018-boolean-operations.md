# 0018. Boolean operations

- **Status:** Accepted
- **Date:** 2026-10-23

## Problem

Builders cut doorways and windows out of walls, carve a tunnel through rock, or keep only where two shapes meet. Hammer has Carve (subtract), which is known for leaving messy pieces; it has no union or intersect. Halberd needs all three, and every result must still be convex brushes, as Source compiles nothing else.

## Options

1. **Concave shapes in the editor**, split into convex brushes only when saving. This is the most flexible, but it is a larger change to the map model; it belongs with the convex-splitting milestone.
2. **Booleans that produce convex brushes straight away**, cutting a brush one face of the cutter at a time (the standard way, as Hammer's Carve does).

## Decision

Option 2, as three more shape operations in the Shape menu, each one undo step.

- **Subtract (Ctrl+Shift+C, "carve"):** the selected brushes cut their space out of every other brush they overlap.
  - The cut brushes are left in convex pieces, each staying in its brush's entity.
  - The cutting brushes stay, still selected, so they can be moved and used again, or deleted. The Console says so.
- **Union (Ctrl+Shift+U):** the selected brushes fill the space they cover together without overlapping: each is cut by the ones before it. If that space has no dents (two boxes in a row, say), it becomes one brush, as Join would make it.
- **Intersect (Ctrl+Shift+I):** only the space all the selected brushes share is kept, as one brush. As with Join, it is a world brush if any of them is one.
- **Faces keep their look:**
  - A face that came from a cut brush keeps that brush's face data. Only the brush's first piece keeps saved face ids, and only for faces that didn't change, so ids are never repeated.
  - A face made by a cutter copies the look of that cutter face, as in Hammer.
- **Slivers:** pieces thinner than a unit are dropped, as Hammer does.

## Consequences

- A subtraction leaves up to one piece per face of the cutter. A many-sided cutter (a 32-sided cylinder) makes many pieces; that is what Source needs, and the convex-splitting milestone can later tidy pieces up.
- Union of brushes that can't merge into one leaves several brushes. They are ordinary brushes; select them together to move them.
