# halberd-tools

**Layer 3 · Front end**

Interaction tools: select, gizmo, shape tools, radial menus.

## Purpose

Turns mouse and keyboard input into commands: selection, the W/R/S/T gizmo, shape editing tools and radial menus.

## Must never

- Draw panels
- Change the document except by issuing commands
- Depend on egui or the GPU: input arrives as plain data (`ViewportInput`), so everything here is testable without a window

## Depends on

`halberd-doc`, `halberd-geom`. Outside libraries: `glam` (vector maths), `serde`.

## Status

The viewport camera and the first left-mouse tools are implemented.

- **`Camera`**: position plus yaw and pitch, Z-up like Hammer. Builds its own view and projection matrices (depth 0 to 1, as wgpu expects), casts rays through the screen and projects points onto it.
- **`CameraController`**: the three modes from the feature spec.

| Mode | Right mouse | Middle mouse | Wheel |
| --- | --- | --- | --- |
| Default | Orbits around the surface under the pointer (the grid plane until maps exist) if it is within 8192 units, else the last pivot | Pans; the point under the pointer follows it | Zooms towards the last orbit point, stopping 8 units short of it; towards the pointer when the orbit point is off screen |
| Orbit | Orbits around the selection (the last pivot until selection exists) | Pans | Zooms, as in Default |
| Fly | Hold to look; WASD moves, Space/C up/down, Shift ×4 | Pans | Zooms towards the pointer; while holding right mouse, changes fly speed |

- Fly mode is hidden when WASD movement is turned off in settings.
- `CameraController::frame` points the camera at the whole of a map when it is opened. `set_wasd_enabled` and `set_fly_speed` apply the Settings page (turning off WASD leaves the Fly camera).
- Guarantees, each covered by tests: the pivot stays exactly under the pointer while orbiting; zooming leaves the orbit point exactly where it is on screen; the view never flips over; the camera stays inside ±131072 units; broken input (NaN, zero sizes, huge frame times) never breaks the camera.
- `CameraState` (camera, mode, pivot, fly speed) is saved between sessions.

### Left-mouse tools

`ToolController` runs the active `Tool`. Tools never change the map: they return a `ToolAction` (select, add/remove from the selection, run a command, or refuse with a reason) for the interface to carry out.

| Tool | Left mouse |
| --- | --- |
| Select | In Object mode (the default; see below for the others), a click selects what is under the pointer (nothing: clears the selection). Ctrl+click adds or removes. Moving more than 4 points between press and release is not a click. A brush inside a brush entity selects the whole entity, unless `set_inside_entities(true)` (Hammer's "Ignore groups"); the gizmo appears only around brushes. |
| Draw (B) | Drag to draw the chosen `Shape` (`set_shape`: box, wedge, cylinder, cone, sphere, arch, stairs; settings in `shape_settings_mut`) in a box snapped to the grid, standing on the grid or on top of the brush under the pointer, as tall as `box_height` (128 units by default, settable, whole units). A drag along one grid line makes a wall one grid square thick; a click makes nothing. Wedges and stairs climb, and arches span, along the drag's longer direction, towards where it ended. `turn_drawing` (R) turns the shape a quarter turn clockwise while drawing; `ToolInput::uniform` (Shift) and `centered` (Alt) make it as wide, deep and tall as the drag's longest side, from the starting corner or around the starting point. A shape too small to build is refused with a reason. Escape cancels. `preview()` gives the box and `preview_brushes()` the shape while dragging. **Floor plan** (`tool/plan.rs`): click corner after corner instead of dragging; the first click stands the plan on the grid or on the brush under the pointer, later corners snap to the grid at that height. Clicking the first corner again (with three or more), or `finish_plan` (Enter), makes the shape as tall as `box_height`; `undo_plan_corner` (Backspace) takes back a corner. `plan_preview()` gives the corners, the pointer's corner and whether the next click closes it. While a plan is drawn, `is_busy` is true, so undo and delete wait. |

`DocumentScene` is what the camera and tools see of an open map: brush surfaces first, the grid plane elsewhere, and the selection's centre for Orbit mode.

### Side views

`SideCamera` (`side_view.rs`) is a flat view looking along one axis (`Side`: Top, Bottom, Front, Back, Left, Right, with Hammer's axes: Top shows X right and Y up, Front Y/Z, Right X/Z). Its `Camera` has `ortho` set (half the height shown), stands far back so the whole map is in front, and starts its rays at the edge of the map (`MAP_EDGE`) so picking reaches everything. `update` pans with the right or middle button and zooms around the pointer; `frame` shows a box; `grid_plane` says where the view's grid lies (flat behind the map). `Camera::units_per_point` sizes things on screen (the gizmo) in both kinds of view.

### Player size

`PLAYER_HEIGHT` (72), `PLAYER_WIDTH` (32), `PLAYER_EYE_HEIGHT` (64) and `player_bounds`: the standard Half-Life 2 player that GMod uses, for the scale figure in the viewport.

### Transform gizmo

With the Select tool and a gizmo mode picked, handles appear around the selection (see [decision 0010](../../docs/decisions/0010-transform-gizmo.md)):

| Mode (key) | Handles | Dragging |
| --- | --- | --- |
| Move (W) | Arrows; squares between two axes | Moves along the arrow, or across the square's plane, in grid steps |
| Rotate (R) | Rings | Turns around the ring's axis through the selection's middle, in 15° steps |
| Scale (S) | A cube on each axis; one in the middle | Stretches that side, keeping the opposite side in place (never below one grid square); the middle resizes evenly from the floor, in ⅛ steps |
| All (T) | All of the above | |

- `ToolController::toggle_gizmo_mode` (the keys and toolbar are toggles), `gizmo_mode`, `gizmo_shapes` (the shapes to draw, in points, with the hovered or dragged handle highlighted).
- Each frame of a drag returns `ToolAction::ExecuteMerging` with the drag's key; Escape returns `ToolAction::CancelMerging`.
- Tests drive real drags through the controller and check the map, plus a stress test of 300 random drags in every mode.

### Shape operations

`ShapeOp` lists the operations (see [decisions 0017](../../docs/decisions/0017-shape-operations.md) and [0018](../../docs/decisions/0018-boolean-operations.md)): Extrude, Bevel, Bridge, Split, Clip, Join, MergeCorners, DeleteParts, Mirror (X, Y, Z), MirrorAcrossView, the booleans Union, Subtract (the selected brushes carve every brush they overlap and stay selected) and Intersect, and MakeBrushes (selected floor plan shapes become ordinary brushes, one per convex piece, selected). The gizmo moves, scales and turns shapes too; they only turn about the upright axis, as they always stand upright. `ToolController::shape_op_ready` says whether one can run or what to pick first; `shape_op` works it out on the map as an edit (`ShapeOpOutcome`: the command, what to select and pick afterwards, a gizmo mode, a note); `apply_shape_op` carries it out as one undo step and selects and picks what it asked for. Sizes come from the grid. The cutting plane for Split and Clip comes from a picked face, three picked corners, or a line (an edge, two corners) along the view. Tests run every operation on real maps, including brushes inside a `func_detail`.

### Face, edge and vertex modes

`SelectMode` (Object 1, Vertex 2, Edge 3, Face 4; `set_select_mode`) chooses what clicks pick in the Select tool (see [decision 0016](../../docs/decisions/0016-element-modes.md)):

- In Vertex, Edge and Face modes a click picks a corner, edge or face of the selected brushes (and of a selected brush entity's brushes): corners and edges within 8 points on screen (the nearer one if two overlap), faces where the pointer's ray meets them (unless another object is in front). Ctrl+click adds or removes; a click on another object selects it; a click on nothing lets go of the picked parts. `picked_elements`, `picked_bounds`, `clear_elements` (Escape).
- Picked parts (`PickedElement`) are remembered by position, so they survive the brush being rebuilt; parts that no longer exist drop out on the next frame, and a cancelled drag puts them back. Picking a mode or the Draw tool lets go of them.
- The gizmo appears around the picked parts and moves, rotates or scales their corners, returning `Command::EditBrushes` (via `Brush::with_moved_points`) each frame of the drag, and moves the picked positions along.
- In these modes a drag draws a box that picks every part inside it (`marquee` gives the box while dragging; Ctrl adds).
- `element_overlay` gives what to draw: every corner or edge that can be picked, the picked ones, and the one under the pointer, in screen points.
- Tests drive clicks and drags through the controller: picking each kind, moving a face, a corner and an edge, turning a face, Escape, entity brushes, and 60 random drags.
- **Materials** (`tool/material.rs`): `set_material` chooses the material new brushes and shapes get (`material`; none means Halberd's default). `apply_material` gives the `SetMaterial` edit for the picked faces in Face mode, otherwise for the selected brushes, shapes and brush entities, or a plain-language reason when nothing is selected.
- **Place tool** (`tool/place.rs`): `set_placing` chooses what to place (`PlaceChoice`: a prop, `PlaceChoice::prop`, as `prop_static` or `prop_dynamic` for models not made static; or another point entity with its settings, `PlaceChoice::entity`) and turns on `Tool::Place`; a click on the grid or a brush gives `AddEntity` standing on the surface, on the grid across; `preview` gives its box under the pointer; Escape stops. The gizmo moves and turns selected point entities with brushes (turning changes their `angles`).
- **Clearance** (`clearance.rs`, see [decision 0026](../../docs/decisions/0026-scale-tools.md)): `clearance(doc, feet)` says whether a player standing there fits standing, only crouching (`PLAYER_CROUCH_HEIGHT`) or not at all (`Fit`), against the map's brushes and shapes, and how much room is left above their head.
