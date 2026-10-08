# 0007. Editor window: eframe, egui_dock, and accepted licenses

- **Status:** Accepted
- **Date:** 2026-10-07

## Problem

Halberd needs a native window, a frame loop, and dockable panels that users can drag, split and resize, all running on wgpu as chosen in [0002](0002-rust-wgpu-egui.md).

## Options

1. **winit and wgpu wired by hand**, with egui added on top: full control over the frame loop, but a few hundred lines of setup code to own and maintain.
2. **eframe**, egui's official framework: the same winit and wgpu underneath, already wired together, plus saving window size, position and app state between sessions. The 3D viewport can still draw with wgpu through egui's paint callbacks.

For panels: **egui_dock** (drag, split, tab groups, serializable layout) or **egui_tiles**.

## Decision

- **eframe with the wgpu renderer** owns the window and frame loop. `halberd-app` implements `eframe::App`; everything drawn comes from `halberd-ui`.
- **egui_dock** provides the panels. Panels can be rearranged but not closed and not torn off into floating windows, so none can be lost. **View → Reset panel layout** restores the standard arrangement.
- **The layout is saved by eframe** in its storage file (`app.ron` in the system's per-user data folder). A saved layout missing a panel or containing one twice is replaced by the standard layout, with a note in the Console.
- **`--report-only`** keeps the text-only startup report, for troubleshooting and automated tests.
- **Window smoke test in CI:** on every change, Linux CI opens the real window on a virtual screen with a software Vulkan driver, takes a screenshot, quits with Ctrl+Q (falling back to File → Quit) and checks the layout was saved.

Licenses accepted in `deny.toml` for this milestone:

- **BSL-1.0** (Boost), used by the Windows clipboard library: permissive.
- **OFL-1.1** and **Ubuntu Font Licence 1.0**, for egui's built-in fonts: both allow embedding and redistribution as long as the license text is included. The texts are in `licenses/fonts/` and ship with every build.

Advisories accepted, as "unmaintained" notices rather than vulnerabilities, with no replacement available upstream: `paste` (RUSTSEC-2024-0436, compile-time only, via egui_dock) and `ttf-parser` (RUSTSEC-2026-0192, via winit's Linux window decorations). Each is listed in `deny.toml` with its reason and should be removed once the libraries move on.

## Consequences

- Less code to maintain now. If the viewport later needs control eframe does not give, a new decision record will weigh moving to hand-wired winit.
- On Windows a terminal window still opens behind the editor. It shows the startup report and stays useful for diagnosing problems during Phase 0; it will be hidden before the first public release, once errors are shown in proper dialogs.
- Some windowing libraries stop with a panic instead of an error when a system library is missing (found by the smoke test: `libxkbcommon-x11` on a bare Linux machine). Halberd catches that around the window and explains it in plain words, with an install hint, instead of crashing.
- egui_dock's tab buttons are not exposed to screen readers by default; Halberd labels them itself (`on_tab_button`), which also lets UI tests find them.
