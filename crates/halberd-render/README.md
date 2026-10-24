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

`halberd-doc`, `halberd-geom`, `halberd-mdl` (plain model meshes). Outside libraries: `wgpu` (same version as eframe, so the window's GPU device is shared), `glam`, `bytemuck`.

## Status

Draws the ground grid, world axes and the map's brushes into an offscreen image per viewport.

- **Target** (`ViewportTarget`): an sRGB colour image (4× multisampled when the GPU supports it) plus a depth buffer. `display_view()` gives the same pixels as plain RGBA, which is what egui expects.
- **Grid**: one large square, flat across the axis `FrameParams::grid` names (`GridPlane`: the ground in the 3D view; behind the map in a side view, without fading), out to ±16384 (the edge of a Source map), with lines computed per pixel in the shader. Three levels (editor grid, ×8, 1024 units) each fade out as they get too dense, so there is no shimmering in the distance. The grid fades towards the horizon.
- **Axes**: X red, Y green (positive halves bright, negative dimmed), Z blue rising from the origin.
- **Brushes** (`SceneGeometry`): faces as triangles with simple fixed lighting (each side of a box has its own shade), dark outlines, and selected brushes in red with outlines drawn on top of everything. Rebuilt only when the document's revision or selection changes (`update_scene`). Faces are drawn first, so the transparent grid hides behind them.
- **Entities**: brushes belonging to entities are teal-grey; point entities are purple 16-unit boxes. Selected ones are red like brushes; a selected entity shows all its brushes red, and a single brush inside an entity can be selected on its own.
- **Floor plan shapes**: drawn as their pieces, but outlined only along the outline (no lines where pieces meet inside).
- **Render modes** (`FrameParams::mode`, `RenderMode`): Wireframe (faces skipped; dark outlines drawn light, entity outlines keeping a teal or purple tint from `ENTITY_EDGE_COLOR` and `POINT_ENTITY_EDGE_COLOR`), Flat, and Textured. Each face vertex carries texture coordinates from its face's texture axes (in textures of `TEXTURE_SIZE`, 128 texels) and its material's hue (`material_hue`, the same for a name in any case); until real textures are read, Textured draws a pattern per material: its colour, a checker every quarter texture and a line at each texture's edge, so alignment, scale and rotation show. Selected faces keep a red tint.
- **Real textures** (`textures.rs`, see [decision 0023](../../docs/decisions/0023-materials.md)): in Textured mode, each material a face uses gets a layer of one texture array (`MATERIAL_TEXTURE_SIZE`, 256 × 256 with mipmaps; at most `MAX_MATERIAL_TEXTURES`, 256, or fewer if the GPU allows fewer). `wanted_textures` names materials whose textures the app should read (each once); `upload_texture` fills a layer (resized, with mipmaps made on the CPU) and records the texture's real size, so it lines up as in the game; `texture_missing` keeps the pattern for a material that can't be shown. Whenever the scene is rebuilt, layers of materials the map no longer shows are given back (`textures/layers.rs`). Faces whose texture isn't loaded keep the pattern. The array is made at its full size only when the first texture arrives. `limit_textures` keeps fewer (the memory budget), before the first texture arrives.
- **Props** (`models.rs`, `scene/props.rs`): `PropModel` holds a model's meshes (material names, vertices, triangles turned to face the way their normals do, and each edge once for outlines). `wanted_models` names models the map's props show (each once); `add_model` keeps one (only if a prop asked for it) and redraws. Props are baked into the scene: turned and placed, in `PROP_COLOR`, textured with the model's own coordinates (`NORMALIZED_UV_FLAG` tells the shader), with their triangle edges as outlines. Until its model arrives a prop is a marker box.
- **Thumbnails** (`renderer/thumbnail.rs`): `render_thumbnail` draws one model alone, framed from the front and a little above, in the Flat look, and reads it back, without touching the map being shown.
- **Basic entities** (`scene/entities.rs`): point entities without a model are boxes coloured by kind (`entity_color`: `SPAWN_COLOR`, `LIGHT_COLOR`, `SUN_COLOR`), sized by class (player-sized spawns), with a line showing which way spawns, spotlights and the sun face.
- **Player figure colour** (`PlayerOutline::color`): `PLAYER_COLOR` where a player fits, `PLAYER_CROUCH_COLOR` or `PLAYER_BLOCKED_COLOR` where not.
- **Shape preview**: the outline of the shape being drawn, in yellow, on top of everything (`set_preview_shape`; without one, the box in `FrameParams::preview`).
- **Player figure**: a light-blue player-sized outline with a ring at eye height, hidden behind brushes like a real object (`FrameParams::player`).
- **`read_pixels`**: copies an image back to the CPU, for tests and screenshots.

Tests (render modes): wireframe shows outlines and no faces, textured shows a pattern where flat is even. Tests: the shader is validated with wgpu's own compiler on every machine. `tests/gpu.rs` renders real frames and checks pixels (background, axis colours, grid lines, solid shaded brushes hiding what is behind them, red selection, yellow box preview, sizes from 1×1 to the GPU limit, 120-frame stability). Without a GPU those tests skip, unless `HALBERD_REQUIRE_GPU` is set, as it is in CI's window job.
