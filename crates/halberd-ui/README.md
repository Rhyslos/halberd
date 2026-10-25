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
- **Shape menu** (`viewport/shape_ops.rs`, between Edit and View): every shape operation with its key (E, Ctrl+B, F, K, X, Ctrl+J, M, Ctrl+M, Ctrl+Shift+U / C / I for union, subtract and intersect, and Ctrl+Shift+B to make brushes from floor plan shapes), greyed out with the reason when it can't run; the same keys work in the viewport and say why in the Console when they can't. While picking parts the viewport draws the box being dragged.
- **Delete in Vertex or Face mode** (Select tool, brushes selected) deletes the picked corners or faces (Edge mode: refused with a note), never the whole brush.
- **Inside entities** (Ctrl+W, or the button in the viewport's toolbar): clicks pick single brushes inside brush entities such as `func_detail` instead of the whole entity (Hammer's "Ignore groups"). Shortcut keys use the modifiers held when the key went down, so Ctrl+W never counts as W.
- **Properties**: for an entity, its class, settings (read-only until Phase 2) and brush count. For a brush inside an entity, which entity it is part of. For a brush, its material(s). For one box, number fields for its width, depth, height and lowest corner. Drag them sideways or click to type; a whole drag is one undo step; values are stored in whole Hammer units. Resizing pushes each face out or in, so every face keeps its own material and file data. For several objects, a summary.
- **View menu**: show lengths in Hammer units or metres (everywhere: Properties, the Draw tool's fields, the player label), and show or hide the player figure. Both are saved in the settings file by `halberd-app`.
- **Player figure**: a 72-unit player outline beside the box being drawn or the selection, on the side facing the camera (at the origin otherwise), labelled with its height.
- **Keybinds** (`keymap.rs`, `keybinds.rs`): one list of every key binding (keys, action, when it applies), shown by the Keybinds window from the menu bar: a drawn US keyboard with bound keys lit, hover or click a key for its actions, search to light up matching keys, and the mouse controls below.
- **Console** (`console.rs`): every message with its time and level (information, warning, error; warnings and errors coloured). Buttons show or hide each level, a box searches, Copy puts the shown messages on the clipboard, Clear empties it. The same message repeated is shown once with a count. Keeps the last 5,000 messages; each takes one line (hover a cut-short one to read it all), and only the lines on screen are drawn. When problems arrive while it is hidden behind another tab, the tab shows how many. A mirror passes every message on (the program writes them to its log file); the program also adds messages from its logger. Library and Layers show placeholders until their milestones.
- **Views** (`viewport/layout.rs`, `viewport/panes.rs`): the viewport panel shows up to `MAX_VIEWS` (six) views in a `ViewLayout` (one, two side by side or stacked, three with one large, four as in Hammer, six), picked from **Views** in its top-right corner or the View menu. Each view's name in its top-left corner is a menu of what it shows (`PaneView`: Perspective or a side view: Top, Front, Right, Bottom, Back, Left; the first four views start as Hammer's). Each view keeps its own 3D camera and side camera. The camera controls go to the view under the pointer (kept while the right or middle button is held); the tools go to the view under the pointer (outlined when there are several), kept while the left button is held or a drag is in progress, so a drag never jumps views; a floor plan being drawn follows the pointer into any view it can be drawn in. The player figure is placed once, from a 3D camera, and shown the same in every view. After drawing, the renderer is told how many views are shown (`ViewportRenderer::release_unused`) so it can let go of the others' images. Tools are shared; the toolbar rows sit in the first view, the camera mode switcher in each 3D view. Shapes are drawn in 3D, Top and Bottom views; a side view says so. Each view's menu also sets how it draws (`RenderMode`: Wireframe, Flat, Textured; side views start as wireframes), passed to the renderer in `ViewportView::render_mode` and saved with the views. `view_setup` and `with_view_setup` (`ViewSetup`) save and restore the views. Each view's image is asked of the renderer with its own `ViewportView::id` and grid plane.
- **Viewport** (`ViewportPanel`): turns mouse and keyboard input into a `ViewportInput` for the camera controller and a `ToolInput` for the active tool (switcher in the top-left corner with the Select and Draw tools and the Move W / Rotate R / Scale S / All T gizmo buttons; B picks Draw and B again opens its shape list (arrow keys, Enter or Space; Escape closes), Q picks Select; Draw's second toolbar row (`viewport/draw_options.rs`) picks the shape and its height, sides, arch thickness or step height; R while drawing turns the shape, Shift and Alt draw it uniformly from the corner or around the start; W, R, S and T toggle gizmo modes, with Shift+W and Shift+S while the right button is held; 1, 2, 3 and 4 (and the Pick buttons in the Select tool's second toolbar row) choose Object, Vertex, Edge or Face mode; Escape cancels, lets go of picked parts, or deselects), draws the floor plan being drawn and a hint saying what to do next (`viewport/plan.rs`; Enter finishes, Backspace takes back a corner), the transform gizmo (`viewport/gizmo.rs`) and the corners, edges and faces that can be picked (`viewport/elements.rs`) over the image, shows the image from a `ViewportRenderer` (supplied by the program; `NoRenderer` shows a reason instead), draws the pivot marker while orbiting, and has the camera mode switcher in its bottom-left corner. Fly mode keeps redrawing only while a movement key is held.
- Tab buttons and the viewport are labelled for screen readers.

Tests use `egui_kittest` to run the real interface without a window: click menus, measure where each panel is drawn, and drive the viewport with simulated right-drags, middle-drags, scrolling and WASD, draw boxes with left-drags, click to select, and use Delete, Ctrl+Z and Ctrl+Y.

**Material browser** (`materials.rs`, `MaterialBrowser`, in the Library panel once names are given; `workbench/library.rs`): a search box (every word must match), the count shown, and a grid of tiles drawn only where visible. Tiles on screen ask for their picture once (`take_wanted`; pictures arrive through `add_picture`, the 600 shown most recently kept; pictures over 512 pixels are refused). A click picks (`BrowserAction::Pick`), a double-click or **Apply to selection** applies (`BrowserAction::Apply`); the workbench then sets the tools' material and runs the edit, warning in the Console when nothing is selected. Before names arrive the panel says how far reading the game's content has got (`ContentStatus`).

**Library tabs** (`workbench/library.rs`): **Materials** and **Props** (`MATERIALS_TAB`, `PROPS_TAB`), each an `AssetBrowser` (`AssetBrowser::materials`, `AssetBrowser::props`; prop names in lower case, without an Apply button). Clicking a prop chooses it for the Place tool once its model is known (`Workbench::add_model_info`); one picked while its model is being read is chosen when it arrives. While placing, the viewport's second toolbar row says what is being placed (`PLACING_LABEL`) with a **Stop** button; Escape only stops placing.

**Entities tab** (`workbench/entity_list.rs`, `ENTITIES_TAB`): Player spawn, Light, Spotlight and Sun, each starting the Place tool (a second sun selects the first); a sky picker (`SKY_PICKER_NAME`) setting the world's `skyname` from `set_sky_names` (`sky_names` finds them in material names; Half-Life 2's when unknown). **Properties** edits basic entities' colour, brightness, cone, pitch and facing (`panels/entity_settings.rs`).

**Scale** (`viewport/player.rs`, `viewport/readout.rs`): the player figure stands beside what you work on, or under the pointer in a 3D view (`set_player_follows`, View → `PLAYER_FOLLOW_LABEL`); each frame its clearance is checked (cached while nothing changes), coloured (`ViewportView::player_fit`) and labelled ("Crouch only · 36 u, 12 u above"). While drawing, the box's size shows above it; the selection's size shows in the first view's corner.
