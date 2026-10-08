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

- The single source of truth for an open map: brushes, props, entities, groups. Owned by the workbench in `halberd-ui`.
- Every object has a stable ID that is never reused.
- It changes only through commands, which gives undo/redo, autosave and future scripting from one mechanism.
- Editable shapes may be concave; they are split into Source's convex brushes only on export or compile.
- Brushes are stored as planes, as in VMF (`halberd-geom`); face polygons are worked out from them.
- The selection lives in the document but is not an edit, so it is not undone.

### How an edit flows

1. The active tool (`halberd-tools`) reads plain input and returns a `ToolAction`, for example "run `AddBrushes` with this box".
2. The workbench (`halberd-ui`) carries it out: `Document::execute` checks the command, applies it and records how to reverse it.
3. The document's revision goes up; the renderer (`halberd-render`) rebuilds its GPU buffers on the next frame.
4. Undo and redo (Edit menu, Ctrl+Z / Ctrl+Y) replay the recorded changes.
5. Drags (gizmo handles, number fields) send an edit every frame with one merge key, so the whole drag is one undo step; a cancelled drag is reversed and forgotten. See [decision 0010](docs/decisions/0010-transform-gizmo.md).

See [decision 0009](docs/decisions/0009-document-commands-and-brushes.md).

## Files

| Format | Read | Write |
| --- | --- | --- |
| VMF (Hammer maps; `halberd-kv`, `halberd-vmf`, `halberd-io`; see [decision 0011](docs/decisions/0011-vmf-maps.md)) | Yes | Yes |
| FGD (entity definitions) | Yes | No |
| VPK (Valve archives) | Yes | No |
| GMA, `.bin` (Workshop addons) | Yes | No |
| MDL, VVD, VTX (models) | Yes | No |
| VTF, VMT (textures, materials) | Yes | No |
| VDF (Steam config) | Yes | No |
| `.halberd` (Halberd projects, versioned) | Yes | Yes |
| `settings.toml` (editor settings; View menu choices are saved back to it) | Yes | Yes |
| `app.ron` (window size, panel layout, viewport camera; written by eframe) | Yes | Yes |

Compiled `.bsp` maps are written by Valve's compilers (vbsp, vvis, vrad) from the user's own GMod install. Halberd never ships them.

## Startup

1. **Settings** (`halberd-config`): load `Halberd/settings.toml` from the system's settings folder. Never fails; see [decision 0006](docs/decisions/0006-settings-file.md).
2. **Garry's Mod** (`halberd-assets`): use the folder from the command line or settings if valid, otherwise search every Steam installation and library. Check for `garrysmod/gameinfo.txt` and the compile tools.
3. **Remember:** if the found folder differs from the saved one, save it.
4. **Report** what was found in plain words, each line an information message or a warning.
5. **Logging** (`halberd-app`): start `halberd.log` next to the settings file (the previous run's kept as `halberd.previous.log`), connect the standard `log` interface so library warnings and errors reach the Console, and record crashes in the file. See [decision 0012](docs/decisions/0012-console-and-log-file.md).
6. **Window** (`halberd-app` with eframe): open the editor window and show the `halberd-ui` workbench, with the report in the Console panel, and open a map if one was given on the command line. The panel layout and window size are restored from the last session. See [decision 0007](docs/decisions/0007-window-and-panels.md).

Steam libraries are read with the `steamlocate` library, which reads Steam's own files and runs no Steam code.

## Window and panels

eframe owns the window and frame loop and renders with wgpu. Each frame, `halberd-app` hands the window's `Ui` to the `halberd-ui` workbench, which draws the menu bar and the docked panels and returns any actions (such as Quit) for the app to carry out.

## Console and log file

The Console (`halberd-ui`) holds messages with a time and a level (information, warning, error). The editor adds its own messages directly; messages from the `log` interface (any thread) wait in `halberd-app`'s log book and are added at the start of the next frame. Every message also goes to the log file: the editor's through the Console's mirror, the logger's straight from the log book.

## Viewport

Coordinates follow Hammer: **Z is up**, units are Hammer units, right-handed.

Each frame, for each viewport:

1. **Input** (`halberd-ui`): the panel turns egui's mouse and keyboard state into a plain `ViewportInput`.
2. **Camera and tools** (`halberd-tools`): the `CameraController` applies it in the current mode (Default, Orbit or Fly), asking the map what is under the pointer through `SceneQuery` (`DocumentScene`); the active tool turns left-mouse input into a `ToolAction`.
3. **Render** (`halberd-render`, called through `halberd-app`'s `GpuViewport`): brushes, the grid and axes are drawn into the viewport's own offscreen image on the window's GPU device.
4. **Show** (`halberd-ui`): the image is drawn into the panel as an egui texture, with the pivot marker and the camera mode switcher on top.

Rendering sits behind the `ViewportRenderer` trait, so the panel runs in tests with no GPU. See [decision 0008](docs/decisions/0008-viewport-rendering-and-camera.md).

## Opening and saving maps

1. **File menu** (`halberd-ui`): the workbench asks "save changes?" if needed, then returns an action (Open, Save, Save As, Quit).
2. **App** (`halberd-app`): shows the system's file window, then calls `halberd-io`.
3. **Read** (`halberd-io` with `halberd-vmf` and `halberd-kv`): text → KeyValues tree → document. Whatever the editor does not use is kept on the objects and in `MapFileData`.
4. **Write**: document → tree → text, everything kept put back in place; written to a temporary file, the old file kept as `.vmx`, then swapped in.

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
