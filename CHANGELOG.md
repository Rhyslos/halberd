# Changelog

All notable changes to Halberd are listed here, newest first. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **Shapes.** The Box tool is now **Draw** (still B), with a **Shape** picker in a second toolbar row: box, wedge, cylinder, cone, sphere, arch and stairs. Each fills the box you drag, as tall as the Height field. Wedges and stairs climb, and arches span, the way you drag. Cylinders, cones, spheres and arches have a **Sides** setting, arches a **Thickness**, and stairs a **Step** height (8 units by default). While you drag, the actual shape is outlined. Corners land on whole units, as in Hammer, so maps compile cleanly.
- **Pick single brushes inside entities.** Brush entities such as `func_detail` can hold hundreds of brushes. Turn on **Inside entities** (Ctrl+W, or the button in the viewport's toolbar), and a click picks the single brush under the pointer instead of the whole entity, like Hammer's "Ignore groups". That brush can then be moved, rotated, resized (gizmo or Properties) and deleted like any other, and it stays part of its entity when saved. The **Scene** panel lists an entity's brushes under it (click the arrow), and Properties says which entity a brush is part of.
- Deleting a brush entity deletes its brushes; deleting all of an entity's brushes deletes the entity too, as in Hammer.

- **The Console** shows every message with its time and level: information, warnings (yellow) and errors (red). Buttons show or hide each level, a search box finds messages, **Copy** puts what is shown on the clipboard for bug reports, and **Clear** empties it. A message repeated many times is shown once with a count. If warnings or errors arrive while the Console is hidden behind another tab, its tab shows how many.
- **Log file:** every message is also written to `halberd.log`, next to the settings file, including the reason if Halberd crashes. The previous run's log is kept as `halberd.previous.log`. The Console says where the file is.
- Problems reported by the graphics driver and window system now appear in the Console as warnings and errors.
- **Opening and saving Hammer maps (VMF).** File → New (Ctrl+N), Open… (Ctrl+O), Save (Ctrl+S), Save As… (Ctrl+Shift+S), with the system's own file windows. A map can also be opened by starting Halberd with it (`halberd map.vmf`). The camera frames the map when it opens.
- **Nothing lost:** everything Halberd does not use yet (textures on each face, visgroups, entity settings and outputs, cameras, Hammer's settings) is kept and saved back; an untouched map saves byte for byte. Ids are kept, and new ones never clash with any in the file (hidden objects included). Saving keeps the old file as a `.vmx` backup, as Hammer does; if the backup can't be written, the map is still saved and the Console says so. Brushes Halberd can't show (broken shapes, repeated sides) are kept and saved unchanged.
- **Entities** from opened maps are shown: point entities as purple boxes, brush entities (such as `func_detail`) in teal-grey. They can be selected and deleted; Properties lists their settings.
- **Unsaved changes are protected:** the title bar shows the map's name with a dot when unsaved, and New, Open, Quit and closing the window ask "Save changes?" (Enter saves, N doesn't, Escape cancels; holding Ctrl+N can't answer it by accident). While a box is showing, keys leave the map alone. Problems opening or saving are explained in a message box.
- New boxes get Hammer's grey `DEV/DEV_MEASUREGENERIC01B` material; Properties shows a brush's material.
- **The transform gizmo.** Select brushes, then press **W** to move (arrows, and squares to move across two axes), **R** to rotate (rings, 15° steps), **S** to scale (a cube per side, and one in the middle to resize evenly) or **T** for all three. The same key again returns to plain selection; the modes are also buttons next to the tools. While holding right mouse, Shift+W and Shift+S pick Move and Scale. Moves and sizes snap to the grid, each drag is one undo step ("Move brush", "Rotate 2 brushes"), and Escape cancels a drag.
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

### Fixed

- Ctrl+W released quickly no longer also counts as W, and other shortcut keys check the modifier keys held when the key went down.
