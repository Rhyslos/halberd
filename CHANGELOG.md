# Changelog

All notable changes to Halberd are listed here, newest first. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- The editor window: a menu bar (File, View, Help) and five docked panels (Browsers, Viewport, Scene, Properties, Console) in the standard layout. Panels can be dragged, grouped and resized; **View → Reset panel layout** restores them. The layout and window size are remembered between sessions.
- The Console panel shows the startup report.
- If the window can't open (no suitable graphics driver, or a missing Linux system library), Halberd explains why in plain words instead of crashing.
- **Help → About Halberd** with version, credits and license.
- `--report-only` option: print the startup report and exit without opening the window.
- Window smoke test in CI: every change opens the real window on Linux, takes a screenshot, and quits through File → Quit.
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
