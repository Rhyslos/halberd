# Changelog

All notable changes to Halberd are listed here, newest first. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **Box brushes.** The Box tool (**B**, or the switcher in the viewport's top-left corner) draws a box by dragging: snapped to the grid, standing on the grid or on top of the brush under the pointer, 128 units tall. Dragging along one grid line makes a wall one grid square thick. Escape cancels.
- **Selecting.** The Select tool clicks to select a brush (selected brushes are red and outlined through everything); Ctrl+click adds or removes; clicking empty space or pressing Escape deselects. Orbit mode turns around the selection.
- **Edit menu** with **Undo** and **Redo** for every edit, named after what they reverse (Ctrl+Z, Ctrl+Y or Ctrl+Shift+Z), and **Delete** (the Delete key).
- The **Scene** panel lists every object and selects on click; **Properties** shows the selection's size and extent.
- Brushes are drawn solid, with a different shade per side, so shapes read clearly.
- **Metres or Hammer units**: View → *Show lengths in* switches every size Halberd shows and accepts (1 unit = 2.54 cm, so a player is 1.83 m). Maps are always stored in Hammer units. Remembered in the settings file.
- **Editable size and position**: the Properties panel has number fields for a box's width, depth, height and position; drag them or type. Each drag is one undo step.
- **Box height**: the Box tool has a height field next to it (128 units by default).
- **Player figure for scale**: a 72-unit (1.83 m) player outline stands beside the selection or the box being drawn, facing the camera, labelled with its height. View → *Show player for scale* turns it off.
- The window smoke test now draws a box with the real mouse and undoes it.
- **The 3D viewport:** a grid out to the edge of a Source map (±16384 units) that stays crisp at any distance, and coloured world axes (X red, Y green, Z blue). Z is up, as in Hammer.
- **Camera controls:** middle mouse pans, right mouse orbits, the wheel zooms towards the orbit point (as in 3ds Max), or towards the pointer when the orbit point is off screen and in Fly mode. Three modes, switched in the viewport's bottom-left corner:
  - **Default:** orbits around the point under the pointer. Points near the horizon (over 8192 units away) are ignored.
  - **Orbit:** orbits around the selection (the last pivot until selection exists).
  - **Fly:** hold right mouse to look; WASD to move, Space/C for up/down, Shift for 4× speed, wheel to change speed. Hidden when WASD is turned off in settings.
- A pivot marker shows what the camera is orbiting around.
- The camera position and mode are remembered between sessions.
- The Console names the graphics device in use.
- GPU rendering tests, and window smoke test checks for the viewport's colours, orbiting and zooming.
- The editor window: a menu bar (File, View, Help) and six docked panels (Library, Viewport, Scene, Layers, Properties, Console) in the standard layout, with Layers as a tab beside Scene. Layers will group sections of a map so they can be hidden, locked and selected together, saved to Hammer as visgroups. Panels can be dragged, grouped and resized; **View → Reset panel layout** restores them. The layout and window size are remembered between sessions.
- The Console panel shows the startup report.
- If the window can't open (no suitable graphics driver, or a missing Linux system library), Halberd explains why in plain words instead of crashing.
- **Help → About Halberd** with version, credits and license.
- **Ctrl+Q** (Cmd+Q on macOS) quits, shown next to File → Quit.
- `--report-only` option: print the startup report and exit without opening the window.
- Window smoke test in CI: every change opens the real window on Linux, takes a screenshot, quits with Ctrl+Q (File → Quit as a fallback), and checks the layout was saved.
- License texts for the embedded fonts, shipped in a `licenses` folder with every build.
- Settings file (`settings.toml`) in the system's settings folder, holding the Garry's Mod folder, memory budget, worker threads, autosave interval, grid size and WASD preference. Damaged files are set aside and defaults used; out-of-range values are corrected; saving cannot corrupt the file; files from newer versions are never overwritten.
- Garry's Mod detection: finds GMod through Steam, including extra Steam libraries on other drives, checks the install, finds the compile tools and the Workshop folder, and remembers the result.
- Startup report listing the settings file, the GMod folder, the Workshop folder and which compile tools were found, with advice when something is missing.
- Command-line options `--gmod-dir`, `--steam-dir`, `--settings` and `--help`.
- Project foundations: Cargo workspace with 17 single-purpose crates in five layers.
- `layer-check` tool whose tests enforce the architecture rules on every change.
- Workspace-wide safety lints: no `unsafe` code, no `unwrap`/`expect`/`panic!` in library code, documentation required on every public item.
- Continuous integration: formatting, lint, tests and docs on Windows, Linux and macOS; license and vulnerability audit; downloadable builds for all three platforms; changelog check on pull requests.
- API reference published to GitHub Pages from `main`.
- Documentation: README with progress tracker, architecture overview, contributing guide, security policy, decision records, pull request and issue templates.
- `halberd` program skeleton that reports its version. When started by double-clicking, it waits for Enter so the message can be read.
- Downloaded builds open straight to the program, with no nested folders.
