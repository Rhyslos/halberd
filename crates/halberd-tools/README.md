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
- Guarantees, each covered by tests: the pivot stays exactly under the pointer while orbiting; zooming leaves the orbit point exactly where it is on screen; the view never flips over; the camera stays inside ±131072 units; broken input (NaN, zero sizes, huge frame times) never breaks the camera.
- `CameraState` (camera, mode, pivot, fly speed) is saved between sessions.

### Left-mouse tools

`ToolController` runs the active `Tool`. Tools never change the map: they return a `ToolAction` (select, add/remove from the selection, run a command, or refuse with a reason) for the interface to carry out.

| Tool | Left mouse |
| --- | --- |
| Select | Click selects what is under the pointer (nothing: clears the selection). Ctrl+click adds or removes. Moving more than 4 points between press and release is not a click. |
| Box (B) | Drag to draw a box, snapped to the grid, standing on the grid or on top of the brush under the pointer, as tall as `box_height` (128 units by default, settable, whole units). A drag along one grid line makes a wall one grid square thick; a click makes nothing. Escape cancels. `preview()` gives the box while dragging. |

`DocumentScene` is what the camera and tools see of an open map: brush surfaces first, the grid plane elsewhere, and the selection's centre for Orbit mode.

### Player size

`PLAYER_HEIGHT` (72), `PLAYER_WIDTH` (32), `PLAYER_EYE_HEIGHT` (64) and `player_bounds`: the standard Half-Life 2 player that GMod uses, for the scale figure in the viewport.
