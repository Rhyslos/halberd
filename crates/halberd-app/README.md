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
2. **Window** (`window.rs`): opens the editor window with eframe and wgpu, shows the `halberd-ui` workbench with the report in the Console, and remembers the panel layout and window size between sessions.
3. If the window cannot open (no Vulkan, DirectX 12 or Metal driver), the report and the reason are printed in the terminal instead.

Command-line options (`halberd --help`):

| Option | Effect |
| --- | --- |
| `--gmod-dir <folder>` | Use this Garry's Mod folder and remember it |
| `--steam-dir <folder>` | Look for Garry's Mod in this Steam folder |
| `--settings <file>` | Use this settings file instead of the standard one |
| `--report-only` | Print the startup report and exit, without opening the window |

On Windows a terminal window opens behind the editor during Phase 0; it shows the same report and helps diagnose problems.
