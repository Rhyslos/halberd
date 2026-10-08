# 0010. Transform gizmo

- **Status:** Accepted
- **Date:** 2026-10-08

## Problem

Brushes need to be moved, rotated and resized directly in the viewport with the W / R / S / T gizmo from the feature spec. The gizmo has to be easy to grab at any distance, work with grid snapping, make each drag one undo step, and stay testable without a window or GPU.

## Options

1. **Gizmo as 3D geometry on the GPU:** drawn by the renderer, picked with rays against 3D handle shapes. Looks like part of the scene, but needs its own pipeline, depth handling and 3D picking, and is hard to test without a GPU.
2. **Gizmo in screen space:** the handles' 3D positions are projected onto the viewport, drawn as flat 2D shapes over the image, and the pointer is tested against those same shapes. The way most editors' gizmos actually behave (constant size on screen, always on top).

## Decision

- **Screen space (option 2).** `halberd-tools` works out the handles' shapes (arrows, plane squares, rings, cubes) in points and tests the pointer against them; `halberd-ui` only paints them with egui. The gizmo's arms are always 90 points long on screen.
- **Grabbing:** a filled shape the pointer is inside wins (the most central, if squares overlap); otherwise the nearest shape within 8 points. Handles pointing straight at the camera, and plane squares seen edge-on, are hidden because they cannot be dragged sensibly.
- **Drag maths:** arrows use the point on the axis line closest to the pointer's ray; squares use where the ray meets the plane; rings use the pointer's angle around the gizmo's centre on screen (with the turn direction depending on which way the axis faces the camera), accumulated so a drag can go round more than once; scale cubes use the axis line, keeping the opposite side in place; the centre cube resizes evenly from the floor of the selection, following the pointer's diagonal movement.
- **Snapping:** moves and side lengths snap to the grid; turns snap to 15°; even resizing snaps to steps of ⅛. A side never shrinks below one grid square.
- **Every frame rebuilds from the originals.** A drag keeps the selected brushes as they were when it began and applies the total movement so far, so rounding never builds up and moving back gives the original shapes.
- **One drag, one undo step:** every frame sends `Command::TransformBrushes` through `Document::execute_merging` with the drag's own key. Escape sends `CancelMerging`, and `Document::discard_step` reverses and forgets the open step, leaving no trace in the undo list.
- **Brushes are transformed as planes** (`Brush::transformed` in `halberd-geom`): points on planes move with the transform and normals use the inverse transpose, so uneven scaling of rotated brushes stays correct.
- **Keys:** W, R, S, T toggle Move, Rotate, Scale, All, as the spec says. While the right button is held (flying uses W and S), Shift+W and Shift+S pick Move and Scale; R and T work either way. Keys with Ctrl are left alone (Ctrl+S will be Save).

## Consequences

- The gizmo is always drawn on top of the scene, even when the selection is behind a wall; that is usually what users want while editing.
- Rotated brushes are no longer axis-aligned boxes, so the Properties panel shows a summary for them instead of size fields.
- Snapping cannot be turned off yet; holding a key for free movement can come with the Settings page.
- During a drag, undo, redo, delete and switching tools wait until the drag ends; a drag that comes back to where it started leaves no undo step; the turn direction of a ring is fixed when the drag starts; a held key toggles a mode once, not with every key repeat.
- A ring seen exactly edge-on is still shown; dragging it is awkward (the turn jumps as the pointer crosses the middle). Orbiting a little fixes it. A 3D ring drag could replace the screen angle later.
