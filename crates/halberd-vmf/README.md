# halberd-vmf

**Layer 0 · File formats**

Parse and write VMF map files losslessly.

## Purpose

VMF is the text map format Hammer saves. This crate reads a VMF into plain data (solids, sides, entities, groups, visgroups) and writes it back without losing anything, which is what makes Export to Hammer and import possible.

## Must never

- Know about the editor's document, GPU or UI
- Read or write files itself: it works on text it is given
- Silently drop data it does not understand

## Depends on

`halberd-kv`

## Status

Skeleton only. The roadmap milestone that fills this crate in will replace this line.
