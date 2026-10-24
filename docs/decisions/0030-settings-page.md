# 0030. The Settings page: keys, camera and memory

- **Status:** Accepted
- **Date:** 2026-10-24

## Problem

Halberd had a settings file but nowhere in the editor to change it. People coming from Hammer or Blender want their own keys; some want WASD flying and some don't (the file even had a "not asked yet" answer waiting for this); camera speed is personal; and a laptop needs a smaller memory budget than a workstation.

## Options

1. **Keep editing the file by hand.** Not for a non-programmer, and nothing checks the keys.
2. **A Settings window** that changes the same settings, applies them at once, and saves them.

## Decision

Option 2. A **Settings** button in the menu bar (beside Keybinds) opens a window with three tabs.

- **Keys.** Every command a key starts is listed by when it works (anywhere, in the viewport, while drawing, flying). Click a key, then press the new one (with Ctrl, Shift or Alt if wanted); Esc cancels. **Clear** leaves a command without a key, **Reset** puts one back, **Reset all keys** puts them all back.
  - **Shared keys are allowed but pointed out** ("Also: Save") where both work at once. Refusing would make swapping two keys impossible without a spare key.
  - **Two commands acting on one key press are pointed out**, including a tool key (Q, B) used for turning a shape while drawing, and a flying key that something else also uses (only the gizmo keys step aside while flying: they need Shift then).
  - **Some keys stay fixed** because they mean the same everywhere: Escape (cancel, close, and cancelling a key change), Enter and Backspace for confirming and taking back, the arrows in lists, Shift and Alt while drawing, and Ctrl with C, X or V (the window system keeps them for copying; only Ctrl+Shift+C and X can be told apart, and they work). Flying keys must be plain keys, since Shift flies faster. Second keys that always work (Ctrl+Shift+Z for redo, Backspace for delete) stay as they are.
  - **The settings file keeps only the changes**, as text by each command's stable name (`"file.save" = "Ctrl+Alt+S"`, `""` for no key). A newer Halberd can add commands or change defaults without losing them. Changes to commands this Halberd doesn't have are kept word for word (so opening an older Halberd doesn't lose them); keys that can't be used are left out; both with a Console note.
  - The menus, the toolbar ("Move G") and the Keybinds window show the keys in effect.
- **Camera.** Fly with WASD on or off (off hides the Fly camera); flying speed when Halberd starts (the wheel still changes it while flying); turning speed and wheel zoom speed, from 10 to 400 %. Turning speed is for the 3D view only; a side view's drag keeps following the pointer.
- **First launch** asks about WASD once, in a small window in the corner that never covers the map. Until answered, WASD is on, as before.
- **Performance.** The memory budget (512 MB to 256 GB) decides how many pictures each browser keeps (600 at the standard 2 GB, from 150 to 4,000) and how many material textures stay on the graphics card (256 at 2 GB, at least 64). It takes effect the next time Halberd starts, because the texture store is made once.
- **Applying and saving.** Changes take effect at once and are saved straight away (the same way View menu choices already were), except while a slider is being dragged: then once it is let go.
- **Typing.** While a text box has the keyboard, keys without Ctrl or Alt (other than F1 to F12) are text, never commands, whatever they are bound to.

## Consequences

- While the window waits for a key, every other key waits too, so pressing Ctrl+S there changes the key instead of saving.
- Worker threads and the autosave interval are in the settings file but not on the page yet; autosave arrives in Phase 2.
- The memory budget covers pictures and textures, the parts that grow with use. The map itself and the game's file lists don't depend on it.
