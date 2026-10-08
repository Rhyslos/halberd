# halberd-app

**Layer 4 · App**

The Halberd program itself.

## Purpose

Starts everything, wires the crates together and owns the frame loop.

## Must never

- Contain feature logic: features live in the crates they belong to

## Depends on

`halberd-assets`, `halberd-io`, `halberd-compile`, `halberd-render`, `halberd-tools`, `halberd-ui`, `halberd-doc`, `halberd-config`. Outside library: `eframe` (window, frame loop, wgpu renderer).

## Status

1. **Startup** (`startup.rs`): loads settings, finds Garry's Mod, saves what it found, and builds a plain-language report.
2. **Window** (`window.rs`): opens the editor window with eframe and wgpu, shows the `halberd-ui` workbench with the report in the Console, and remembers the panel layout, window size and viewport camera between sessions.
3. **GPU viewport** (`gpu.rs`): implements the viewport renderer with `halberd-render` on the window's own GPU device, and hands each finished image to egui as a texture. Before each frame it passes the open map to the renderer, which redraws brushes only when the map or selection changed. The Console names the graphics device in use.
4. If the window cannot open (no Vulkan, DirectX 12 or Metal driver, or a missing Linux system library), the report and a plain-language reason are printed in the terminal instead. Panics from windowing libraries are caught for this.

Command-line options (`halberd --help`):

| Option | Effect |
| --- | --- |
| `--gmod-dir <folder>` | Use this Garry's Mod folder and remember it |
| `--steam-dir <folder>` | Look for Garry's Mod in this Steam folder |
| `--settings <file>` | Use this settings file instead of the standard one |
| `--report-only` | Print the startup report and exit, without opening the window |

On Windows a terminal window opens behind the editor during Phase 0; it shows the same report and helps diagnose problems.
