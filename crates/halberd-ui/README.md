# halberd-ui

**Layer 3 · Front end**

The interface panels.

## Purpose

The egui panels: Browsers, Scene, Properties, Console and Settings, docked around the viewports.

## Must never

- Change the document except through commands
- Do slow work on the interface thread
- Open windows or own the frame loop: that is `halberd-app`'s job

## Depends on

`halberd-doc`, `halberd-config`. Outside libraries: `egui`, `egui_dock`, `serde`.

## Status

The workbench is implemented: a menu bar (File, View, Help) and five docked panels in the standard layout from the feature spec.

```
┌──────────┬────────────────────┬────────────┐
│ Browsers │      Viewport      │   Scene    │
│          │                    ├────────────┤
├──────────┴────────────────────┤ Properties │
│            Console            │            │
└───────────────────────────────┴────────────┘
```

- Panels can be dragged, split, grouped and resized, but not closed or floated, so none can be lost.
- **View → Reset panel layout** restores the standard arrangement.
- A saved layout that is missing a panel is replaced by the standard one.
- The Console shows the startup report. Browsers, Scene, Properties and the Viewport show placeholders until their milestones.
- Tab buttons and the viewport are labelled for screen readers.

Tests use `egui_kittest` to run the real interface without a window, click menus, and measure where each panel is drawn.
