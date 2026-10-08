# halberd-render

**Layer 3 · Front end**

GPU rendering of the viewports.

## Purpose

Draws each viewport with wgpu: brushes, props, entities, gizmos and grids, updating only the GPU data for what changed.

## Must never

- Change the document
- Decode files itself: it gets plain meshes and textures from other crates
- Know about egui or windows: it draws into its own images, and the program decides how to show them

## Depends on

`halberd-doc`, `halberd-geom`. Outside libraries: `wgpu` (same version as eframe, so the window's GPU device is shared), `glam`, `bytemuck`.

## Status

Draws the ground grid, world axes and the map's brushes into an offscreen image per viewport.

- **Target** (`ViewportTarget`): an sRGB colour image (4× multisampled when the GPU supports it) plus a depth buffer. `display_view()` gives the same pixels as plain RGBA, which is what egui expects.
- **Grid**: one large square on Z = 0 out to ±16384 (the edge of a Source map), with lines computed per pixel in the shader. Three levels (editor grid, ×8, 1024 units) each fade out as they get too dense, so there is no shimmering in the distance. The grid fades towards the horizon.
- **Axes**: X red, Y green (positive halves bright, negative dimmed), Z blue rising from the origin.
- **Brushes** (`SceneGeometry`): faces as triangles with simple fixed lighting (each side of a box has its own shade), dark outlines, and selected brushes in red with outlines drawn on top of everything. Rebuilt only when the document's revision or selection changes (`update_scene`). Faces are drawn first, so the transparent grid hides behind them.
- **Box preview**: the outline of a box being drawn, in yellow, on top of everything (`FrameParams::preview`).
- **`read_pixels`**: copies an image back to the CPU, for tests and screenshots.

Tests: the shader is validated with wgpu's own compiler on every machine. `tests/gpu.rs` renders real frames and checks pixels (background, axis colours, grid lines, solid shaded brushes hiding what is behind them, red selection, yellow box preview, sizes from 1×1 to the GPU limit, 120-frame stability). Without a GPU those tests skip, unless `HALBERD_REQUIRE_GPU` is set, as it is in CI's window job.
