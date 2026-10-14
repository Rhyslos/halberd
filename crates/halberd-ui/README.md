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

`halberd-doc`, `halberd-geom`, `halberd-config`, `halberd-tools`. Outside libraries: `egui`, `egui_dock`, `glam`, `serde`, `chrono` (message times).

## Status

The workbench is implemented: a menu bar (File, Edit, View, Keybinds, Help) and six panels in the standard layout from the feature spec. Scene and Layers share one area as tabs.

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
- **File menu**: New (Ctrl+N), Open… (Ctrl+O), Save (Ctrl+S), Save As… (Ctrl+Shift+S), Quit (Ctrl+Q). The workbench never touches files: it returns `WorkbenchAction`s (Open, Save, Save As, Quit) for the program to carry out, and New, Open and Quit first ask "Save changes?" when the map has unsaved changes (Enter saves, N doesn't, Escape cancels). Problems are shown in a message box. `window_title` names the map with a dot when unsaved.
- **The workbench owns the open map** (`halberd_doc::Document`) and carries out what the tools ask.
- **Edit menu**: Undo and Redo (named after the edit, such as "Undo Create brush"), Delete. Shortcuts: Ctrl+Z, Ctrl+Y or Ctrl+Shift+Z, Delete. Ignored while typing in a text box.
- **Scene** lists every object ("Brush #3", "light #21"); click to select, Ctrl+click to add or remove.
- **Scene** lists a brush entity's brushes under it, folded away until its arrow is clicked or one of them is selected; clicking a brush there selects that brush alone.
- **Delete in Vertex, Edge or Face mode** (Select tool, brushes selected) does nothing, greys out Edit → Delete, and says so in the Console (deleting parts comes later; deleting the whole brush would surprise).
- **Inside entities** (Ctrl+W, or the button in the viewport's toolbar): clicks pick single brushes inside brush entities such as `func_detail` instead of the whole entity (Hammer's "Ignore groups"). Shortcut keys use the modifiers held when the key went down, so Ctrl+W never counts as W.
- **Properties**: for an entity, its class, settings (read-only until Phase 2) and brush count. For a brush inside an entity, which entity it is part of. For a brush, its material(s). For one box, number fields for its width, depth, height and lowest corner. Drag them sideways or click to type; a whole drag is one undo step; values are stored in whole Hammer units. Resizing pushes each face out or in, so every face keeps its own material and file data. For several objects, a summary.
- **View menu**: show lengths in Hammer units or metres (everywhere: Properties, the Draw tool's fields, the player label), and show or hide the player figure. Both are saved in the settings file by `halberd-app`.
- **Player figure**: a 72-unit player outline beside the box being drawn or the selection, on the side facing the camera (at the origin otherwise), labelled with its height.
- **Keybinds** (`keymap.rs`, `keybinds.rs`): one list of every key binding (keys, action, when it applies), shown by the Keybinds window from the menu bar: a drawn US keyboard with bound keys lit, hover or click a key for its actions, search to light up matching keys, and the mouse controls below.
- **Console** (`console.rs`): every message with its time and level (information, warning, error; warnings and errors coloured). Buttons show or hide each level, a box searches, Copy puts the shown messages on the clipboard, Clear empties it. The same message repeated is shown once with a count. Keeps the last 5,000 messages; each takes one line (hover a cut-short one to read it all), and only the lines on screen are drawn. When problems arrive while it is hidden behind another tab, the tab shows how many. A mirror passes every message on (the program writes them to its log file); the program also adds messages from its logger. Library and Layers show placeholders until their milestones.
- **Viewport** (`ViewportPanel`): turns mouse and keyboard input into a `ViewportInput` for the camera controller and a `ToolInput` for the active tool (switcher in the top-left corner with the Select and Draw tools and the Move W / Rotate R / Scale S / All T gizmo buttons; B picks Draw and B again opens its shape list (arrow keys, Enter or Space; Escape closes), Q picks Select; Draw's second toolbar row (`viewport/draw_options.rs`) picks the shape and its height, sides, arch thickness or step height; R while drawing turns the shape, Shift and Alt draw it uniformly from the corner or around the start; W, R, S and T toggle gizmo modes, with Shift+W and Shift+S while the right button is held; 1, 2, 3 and 4 (and the Pick buttons in the Select tool's second toolbar row) choose Object, Vertex, Edge or Face mode; Escape cancels, lets go of picked parts, or deselects), draws the transform gizmo (`viewport/gizmo.rs`) and the corners, edges and faces that can be picked (`viewport/elements.rs`) over the image, shows the image from a `ViewportRenderer` (supplied by the program; `NoRenderer` shows a reason instead), draws the pivot marker while orbiting, and has the camera mode switcher in its bottom-left corner. Fly mode keeps redrawing only while a movement key is held.
- Tab buttons and the viewport are labelled for screen readers.

Tests use `egui_kittest` to run the real interface without a window: click menus, measure where each panel is drawn, and drive the viewport with simulated right-drags, middle-drags, scrolling and WASD, draw boxes with left-drags, click to select, and use Delete, Ctrl+Z and Ctrl+Y.
