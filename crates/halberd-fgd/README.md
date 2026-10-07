# halberd-fgd

**Layer 0 · File formats**

Read FGD entity definition files.

## Purpose

FGD files list every entity a game supports, with its keyvalues, inputs and outputs. This crate parses them so the editor can offer GMod's real entity list.

## Must never

- Know about the editor's document, GPU or UI
- Panic on malformed input

## Depends on

none (no other Halberd crates)

## Status

Skeleton only. The roadmap milestone that fills this crate in will replace this line.
