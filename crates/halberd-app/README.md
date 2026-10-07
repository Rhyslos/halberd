# halberd-app

**Layer 4 · App**

The Halberd program itself.

## Purpose

Starts everything, wires the crates together and owns the frame loop.

## Must never

- Contain feature logic: features live in the crates they belong to

## Depends on

`halberd-assets`, `halberd-io`, `halberd-compile`, `halberd-render`, `halberd-tools`, `halberd-ui`, `halberd-doc`, `halberd-config`

## Status

Startup implemented: loads settings, finds Garry's Mod, saves what it found, and prints a plain-language report. The window arrives in the "Window and docking panels" milestone.

Command-line options (`halberd --help`):

| Option | Effect |
| --- | --- |
| `--gmod-dir <folder>` | Use this Garry's Mod folder and remember it |
| `--steam-dir <folder>` | Look for Garry's Mod in this Steam folder |
| `--settings <file>` | Use this settings file instead of the standard one |
