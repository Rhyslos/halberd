# 0008. Viewport rendering and camera

- **Status:** Accepted
- **Date:** 2026-10-08

## Problem

The viewport needs real 3D drawing on the GPU, sharing the window's wgpu device, and a camera that implements the feature spec's three modes. Both will grow a lot (brushes, props, gizmos, lighting preview, six viewports), so the first version sets the pattern.

## Options

For drawing:

1. **Draw inside egui's own render pass** with a paint callback: no extra images, but egui's pass has no depth buffer or multisampling of our choosing, and our pipelines would have to match its formats.
2. **Draw into an offscreen image per viewport** and show it in egui as a texture: one extra copy of the viewport on the GPU, but full control over depth, multisampling and formats, and the renderer does not depend on egui at all.

For the camera maths: glam's camera module (reorganised in glam 0.34, released days before this milestone, with Y-up conventions) or a few lines of our own.

## Decision

- **Offscreen images.** `halberd-render` draws into a `ViewportTarget` (sRGB colour, 4× multisampling where supported, depth) with no knowledge of egui. `halberd-app` registers the finished image with egui; `halberd-ui` shows it through the `ViewportRenderer` trait, so the panel and its tests need no GPU.
- **egui reads a plain-RGBA view of the sRGB image.** egui expects non-sRGB native textures; handing it the sRGB view decodes colours twice and darkens everything. Found by looking at the first screenshot, now checked by the window smoke test.
- **Our own view and projection matrices** in `halberd-tools::Camera`: right-handed, **Z-up like Hammer**, depth 0 to 1 as wgpu expects. Tests pin the conventions down.
- **The camera lives in `halberd-tools`**, takes input as plain data (`ViewportInput`) and asks the scene through a `SceneQuery` trait. Until maps exist, the scene is the grid plane (`GroundPlane`), so Default mode's "orbit around what's under the pointer" orbits around the grid point under the pointer.
- **Orbit keeps the pivot exactly under the pointer:** the camera is moved so the pivot keeps the same coordinates in the camera's own frame. Panning and zooming likewise keep the grabbed point under the pointer.
- **Grid drawn per pixel** on one large square to ±16384 (the edge of a Source map), with three fading levels, rather than thousands of line segments: crisp at every distance, no shimmering.
- **Redraw only when needed:** egui redraws on input; the viewport asks for continuous frames only while flying with a movement key held.

## Consequences

- One extra viewport-sized copy on the GPU per viewport; negligible even with six viewports.
- The WASD preference has no first-launch question yet; "not asked" counts as on until the Settings page exists (Phase 1).
- The camera mode and position persist in eframe's storage file alongside the panel layout, not in `settings.toml`, since they describe the session, not preferences.
- Depth uses a standard 0-to-1 range with a 1-unit near plane. If large maps show depth fighting once geometry exists, switching to reversed depth is a contained change in `Camera::projection_matrix` and the pipelines.
