# Architecture

Halberd is a Rust program split into small, single-purpose crates: file formats at the bottom, the map document and geometry in the middle, rendering and interface on top. **Lower layers never depend on higher ones**, so each piece can be built, tested and replaced on its own. The `layer-check` tool enforces this on every change.

## Layers

| Layer | Name | Crates | Rule |
| --- | --- | --- | --- |
| 4 | App | `halberd-app` | Wires everything together and owns the frame loop. No feature logic. |
| 3 | Front end | `halberd-render`, `halberd-tools`, `halberd-ui` | Reads the document; changes it only through commands. |
| 2 | Services | `halberd-assets`, `halberd-io`, `halberd-compile` | Talks to the outside world: game content, files, compilers. |
| 1 | Core | `halberd-doc`, `halberd-geom`, `halberd-config` | The map document, geometry and settings. No files, GPU or UI. |
| 0 | File formats | `halberd-kv`, `halberd-vmf`, `halberd-fgd`, `halberd-vpk`, `halberd-gma`, `halberd-mdl`, `halberd-vtf` | Pure parsers and writers. Know nothing about the editor. |

Each crate's `README.md` states its purpose and what it **must never** do. Read it before changing the crate.

Planned later: `halberd-lighting` (GPU lighting preview, layer 3) and `halberd-plugins` (sandboxed Lua, layer 2).

## How a frame runs

The interface thread draws 60+ frames a second and must never wait on slow work.

1. **Input:** winit delivers mouse and keyboard events.
2. **Interpret:** the active tool turns input into an intent ("move these brushes 16 units right").
3. **Command:** the intent becomes a command and is applied to the document. Every command can undo itself.
4. **Notify:** the document records what changed.
5. **Sync:** the renderer updates GPU data only for what changed.
6. **Draw:** viewports render with wgpu; panels (egui) draw on top.

Slow work (reading assets, booleans, compiles, lighting) runs on background threads. **Background jobs never touch the document.** They send results as messages, and the interface thread applies them at the start of the next frame.

## The document

- The single source of truth for an open map: brushes, props, entities, groups.
- Every object has a stable ID that is never reused.
- It changes only through commands, which gives undo/redo, autosave and future scripting from one mechanism.
- Editable shapes may be concave; they are split into Source's convex brushes only on export or compile.

## Files

| Format | Read | Write |
| --- | --- | --- |
| VMF (Hammer maps) | Yes | Yes |
| FGD (entity definitions) | Yes | No |
| VPK (Valve archives) | Yes | No |
| GMA, `.bin` (Workshop addons) | Yes | No |
| MDL, VVD, VTX (models) | Yes | No |
| VTF, VMT (textures, materials) | Yes | No |
| VDF (Steam config) | Yes | No |
| `.halberd` (Halberd projects, versioned) | Yes | Yes |
| `settings.toml` (editor settings) | Yes | Yes |
| `app.ron` (window size, panel layout, viewport camera; written by eframe) | Yes | Yes |

Compiled `.bsp` maps are written by Valve's compilers (vbsp, vvis, vrad) from the user's own GMod install. Halberd never ships them.

## Startup

1. **Settings** (`halberd-config`): load `Halberd/settings.toml` from the system's settings folder. Never fails; see [decision 0006](docs/decisions/0006-settings-file.md).
2. **Garry's Mod** (`halberd-assets`): use the folder from the command line or settings if valid, otherwise search every Steam installation and library. Check for `garrysmod/gameinfo.txt` and the compile tools.
3. **Remember:** if the found folder differs from the saved one, save it.
4. **Report** what was found in plain words.
5. **Window** (`halberd-app` with eframe): open the editor window and show the `halberd-ui` workbench, with the report in the Console panel. The panel layout and window size are restored from the last session. See [decision 0007](docs/decisions/0007-window-and-panels.md).

Steam libraries are read with the `steamlocate` library, which reads Steam's own files and runs no Steam code.

## Window and panels

eframe owns the window and frame loop and renders with wgpu. Each frame, `halberd-app` hands the window's `Ui` to the `halberd-ui` workbench, which draws the menu bar and the docked panels and returns any actions (such as Quit) for the app to carry out.

## Viewport

Coordinates follow Hammer: **Z is up**, units are Hammer units, right-handed.

Each frame, for each viewport:

1. **Input** (`halberd-ui`): the panel turns egui's mouse and keyboard state into a plain `ViewportInput`.
2. **Camera** (`halberd-tools`): the `CameraController` applies it in the current mode (Default, Orbit or Fly), asking the scene what is under the pointer through `SceneQuery`.
3. **Render** (`halberd-render`, called through `halberd-app`'s `GpuViewport`): the grid and axes are drawn into the viewport's own offscreen image on the window's GPU device.
4. **Show** (`halberd-ui`): the image is drawn into the panel as an egui texture, with the pivot marker and the camera mode switcher on top.

Rendering sits behind the `ViewportRenderer` trait, so the panel runs in tests with no GPU. See [decision 0008](docs/decisions/0008-viewport-rendering-and-camera.md).

## Extension points

| To add | Plug into |
| --- | --- |
| A tool | The `Tool` interface in `halberd-tools` |
| An edit | A `Command` in `halberd-doc` |
| A panel | A panel registered with the dock in `halberd-ui` |
| A file format | A new layer-0 crate |
| A radial-menu action | An action registered by name |
| A compile step | A stage in a compile profile in `halberd-compile` |

## Security

- Files from strangers are hostile: every parser checks sizes before allocating and has a fuzz test.
- Archives are read in place, never extracted; paths that escape an archive are rejected.
- Halberd never runs Lua from addons or maps. Plugins (later) run sandboxed.
- Compilers are started with argument lists, never through a shell, and only from the configured GMod folder.
- No usernames or local paths are written into exported files. No telemetry by default.
- Dependencies are audited for licenses and known vulnerabilities on every change (`cargo deny`).

## Decisions

Why things are the way they are: see [docs/decisions](docs/decisions).
