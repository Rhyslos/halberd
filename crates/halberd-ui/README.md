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

`halberd-doc`, `halberd-geom`, `halberd-config`, `halberd-tools`. Outside libraries: `egui`, `egui_dock`, `glam`, `serde`.

## Status

The workbench is implemented: a menu bar (File, Edit, View, Help) and six panels in the standard layout from the feature spec. Scene and Layers share one area as tabs.

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
- **The workbench owns the open map** (`halberd_doc::Document`) and carries out what the tools ask.
- **Edit menu**: Undo and Redo (named after the edit, such as "Undo Create brush"), Delete. Shortcuts: Ctrl+Z, Ctrl+Y or Ctrl+Shift+Z, Delete. Ignored while typing in a text box.
- **Scene** lists every object ("Brush #3"); click to select, Ctrl+click to add or remove.
- **Properties**: for one box, number fields for its width, depth, height and lowest corner. Drag them sideways or click to type; a whole drag is one undo step; values are stored in whole Hammer units. For several objects, a summary.
- **View menu**: show lengths in Hammer units or metres (everywhere: Properties, the Box tool's height field, the player label), and show or hide the player figure. Both are saved in the settings file by `halberd-app`.
- **Player figure**: a 72-unit player outline beside the box being drawn or the selection, on the side facing the camera (at the origin otherwise), labelled with its height.
- The Console shows the startup report, and anything a tool could not do. Library and Layers show placeholders until their milestones.
- **Viewport** (`ViewportPanel`): turns mouse and keyboard input into a `ViewportInput` for the camera controller and a `ToolInput` for the active tool (switcher in the top-left corner with the Box tool's height field, B toggles Box, Escape cancels or deselects), shows the image from a `ViewportRenderer` (supplied by the program; `NoRenderer` shows a reason instead), draws the pivot marker while orbiting, and has the camera mode switcher in its bottom-left corner. Fly mode keeps redrawing only while a movement key is held.
- Tab buttons and the viewport are labelled for screen readers.

Tests use `egui_kittest` to run the real interface without a window: click menus, measure where each panel is drawn, and drive the viewport with simulated right-drags, middle-drags, scrolling and WASD, draw boxes with left-drags, click to select, and use Delete, Ctrl+Z and Ctrl+Y.
