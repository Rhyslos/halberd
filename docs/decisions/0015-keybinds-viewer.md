# 0015. The Keybinds viewer

- **Status:** Accepted
- **Date:** 2026-10-09

## Problem

Halberd has grown many keys, and several do different things in different situations: R rotates, but turns a shape while drawing; W moves, but flies in the Fly camera while the right mouse button is held. People need one place to see them all and look one up. The project lead asked for a drawn keyboard: hover a key to see what it does, and search for an action to light up its keys.

## Options

1. **A plain list** of shortcuts in a help window. Simple, but finding "what does this key do?" means reading the whole list.
2. **A drawn keyboard plus search**, fed by one list of every binding. Answers both questions: what a key does (hover or click it) and which key does something (search).

## Decision

Option 2.

- **One list** (`halberd-ui`'s `keymap`) holds every binding: its keys, what it does in plain words, and when it applies ("Anywhere", "Viewport", "While drawing", "Shape list open", "Holding right mouse", "Fly camera, holding right mouse", "\"Save changes?\" showing", "Message box showing"). Tests check that the menu shortcuts the code uses are in it, that every listed key is on the drawn keyboard, and that no combination means two things in the same situation.
- **The window** opens from **Keybinds** in the menu bar, between View and Help:
  - a US-layout keyboard where keys that do something are lit;
  - hovering a key lists what it does in every situation, and clicking it keeps that list under the keyboard;
  - the search box finds actions, situations or exact combinations ("save", "while drawing", "ctrl+s") and highlights the keys involved. A single letter or a key's name ("w", "shift") asks about that key only; Ctrl and Cmd, Alt and Option are the same key;
  - the list under the keyboard has a fixed height (it scrolls), so the keyboard never moves while searching or clicking keys;
  - a short list of what the mouse does sits underneath.
- **Modifier keys** (Shift, Ctrl, Alt) are keys on the drawing, so combinations light up together and Shift's own jobs (draw uniformly, fly faster) show when hovering it.
- **Read-only for now:** changing bindings comes with the Settings page milestone, which will edit this same list.

## Consequences

- A new key binding must be added to the list, or the viewer won't show it. The tests catch the menu shortcuts; other keys rely on review.
- Keyboards with other layouts (AZERTY, QWERTZ) still show US positions; Halberd reads keys by their meaning, so the labels are right, but their places on the drawing may differ from the physical keyboard.
