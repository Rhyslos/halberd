# Decision records

Short notes on major technical choices: the problem, the options, the choice and why. When a later change touches an area, its record explains why things are the way they are.

- Numbered files, never renumbered. A reversed decision gets a new record that says which one it replaces.
- Copy `template.md` to start a new one.

| # | Decision | Status |
| --- | --- | --- |
| [0001](0001-record-decisions.md) | Record major decisions | Accepted |
| [0002](0002-rust-wgpu-egui.md) | Rust, wgpu and egui | Accepted |
| [0003](0003-layered-crates.md) | Layered single-purpose crates | Accepted |
| [0004](0004-apache-license.md) | Apache 2.0 license | Accepted |
| [0005](0005-safety-lints.md) | No unsafe, no panics in library code | Accepted |
| [0006](0006-settings-file.md) | Settings file format, location and safety | Accepted |
| [0007](0007-window-and-panels.md) | Editor window: eframe, egui_dock, and accepted licenses | Accepted |
| [0008](0008-viewport-rendering-and-camera.md) | Viewport rendering and camera | Accepted |
| [0009](0009-document-commands-and-brushes.md) | Map document, commands, undo and brushes | Accepted |
| [0010](0010-transform-gizmo.md) | Transform gizmo | Accepted |
| [0011](0011-vmf-maps.md) | Opening and saving Hammer maps (VMF) | Accepted |
| [0012](0012-console-and-log-file.md) | The Console and the log file | Accepted |
| [0013](0013-brushes-inside-entities.md) | Brushes inside brush entities | Accepted |
| [0014](0014-primitive-shapes.md) | Ready-made shapes | Accepted |
| [0015](0015-keybinds-viewer.md) | The Keybinds viewer | Accepted |
| [0016](0016-element-modes.md) | Face, edge and vertex modes | Accepted |
| [0017](0017-shape-operations.md) | Shape operations | Accepted |
| [0018](0018-boolean-operations.md) | Boolean operations | Accepted |
| [0019](0019-floor-plan-shapes.md) | Floor plan shapes, split into convex brushes on save | Accepted |
| [0020](0020-multiple-views.md) | Several views in one viewport panel | Accepted |
| [0021](0021-render-modes.md) | Render modes, and textures before textures exist | Accepted |
| [0022](0022-game-content.md) | Reading game content from VPK archives | Accepted |
| [0023](0023-materials.md) | Materials: reading textures, the browser and per-face materials | Accepted |
| [0024](0024-props.md) | Props: reading models, the prop browser and placing props | Accepted |
| [0025](0025-basic-entities.md) | Basic entities: spawn points, lights, the sun and the sky | Accepted |
| [0026](0026-scale-tools.md) | GMod scale tools: player clearance and size readouts | Accepted |
| [0027](0027-map-limits.md) | Map size and engine limit meters | Accepted |
| [0028](0028-quick-compile.md) | Compile and play | Accepted |
| [0029](0029-leak-path.md) | Showing where a map leaks | Accepted |
| [0030](0030-settings-page.md) | The Settings page: keys, camera and memory | Accepted |
