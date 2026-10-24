# 0026. GMod scale tools: player clearance and size readouts

- **Status:** Accepted
- **Date:** 2026-10-24

## Problem

Blockouts go wrong at scale: doorways a player can't walk through, ceilings they bump, rooms that feel huge or cramped once in the game. The player figure (a 72-unit box beside what you work on) shows size, but not whether a player actually fits somewhere, and the sizes of what is drawn are only visible in Properties.

## Options

1. **A map-wide check** that finds every passage too low or narrow. Hard to define (what is a passage?), slow, and full of false alarms.
2. **Check where the player figure stands**, and let the figure follow the pointer, so you can sweep it through doorways and under ceilings and see at once; plus size readouts in the views.

## Decision

Option 2.

- **Clearance** (`halberd_tools::clearance`): the space a standing player takes (32 × 32 × 72 units), then a crouching one (36 units tall), is tested against the map's brushes (world brushes, brush entities and floor plan shapes; touching a wall or the floor doesn't count). The figure is blue where a player stands, orange where one could only crouch, red where one wouldn't fit. Its label says so, with the room left above the head.
- **Following the pointer** (View → **Player follows the pointer**): in a 3D view the figure stands on whatever is under the pointer.
- **Readouts:** while drawing, the box's width × depth × height shows above it; with something selected, its size shows in the first view's corner. Both use the chosen unit.
- **Feet** joins Hammer units and metres (a unit is an inch, as Source scales characters).

## Consequences

- Props and point entities don't block the figure (their exact shapes aren't solid in Halberd's model of the map); props' collision models may differ from their looks anyway.
- A slanted brush passing close to a corner of the player's box may count as touching it: the check errs towards "doesn't fit".
- Jump and step heights (18-unit steps) are not checked yet.
