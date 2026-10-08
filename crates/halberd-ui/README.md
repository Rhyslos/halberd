# halberd-ui

**Layer 3 · Front end**

The interface panels.

## Purpose

The egui panels: Library, Scene, Layers, Properties, Console and Settings, docked around the viewports.

## Must never

- Change the document except through commands
- Do slow work on the interface thread
- Open windows or own the frame loop: that is `halberd-app`'s job

## Depends on

`halberd-doc`, `halberd-config`, `halberd-tools`. Outside libraries: `egui`, `egui_dock`, `glam`, `serde`.

## Status

The workbench is implemented: a menu bar (File, View, Help) and six panels in the standard layout from the feature spec. Scene and Layers share one area as tabs.

```
┌─────────┬────────────────────┬────────────────┐
│ Library │      Viewport      │ Scene · Layers │
│         │                    ├────────────────┤
├─────────┴────────────────────┤   Properties   │
│            Console           │                │
└──────────────────────────────┴────────────────┘
```

- Panels can be dragged, split, grouped and resized, but not closed or floated, so none can be lost.
- **View → Reset panel layout** restores the standard arrangement. **Ctrl+Q** quits.
- A saved layout that is missing a panel is replaced by the standard one, with one exception: a layout from before Layers existed keeps the user's arrangement and gains Layers as a tab beside Scene. The panel once called "Browsers" loads as Library.
- The Console shows the startup report. Library, Scene, Layers and Properties show placeholders until their milestones.
- **Viewport** (`ViewportPanel`): turns mouse and keyboard input into a `ViewportInput` for the camera controller, shows the image from a `ViewportRenderer` (supplied by the program; `NoRenderer` shows a reason instead), draws the pivot marker while orbiting, and has the camera mode switcher in its bottom-left corner. Fly mode keeps redrawing only while a movement key is held.
- Tab buttons and the viewport are labelled for screen readers.

Tests use `egui_kittest` to run the real interface without a window: click menus, measure where each panel is drawn, and drive the viewport with simulated right-drags, middle-drags, scrolling and WASD.
