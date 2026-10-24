# 0032. Entity inputs and outputs

- **Status:** Accepted
- **Date:** 2026-10-24

## Problem

Map logic in Source is built from connections: when one entity fires an output ("the button was pressed"), another entity gets an input ("door, open"), with an optional parameter, delay and limit. Halberd kept connections from opened maps but couldn't show or edit them, and nothing showed which entities talk to which.

## Options

1. **A separate node editor** (boxes and wires, like a visual script).
2. **Hammer's way, plus lines in the views:** an Outputs list in the entity's properties, a list of what reaches it, and lines between connected entities where they are in the map.

## Decision

Option 2 first. A node editor can come later on top of the same data.

- **Data** (`halberd-doc`): a `Connection` has the output, target name, input, parameter, delay and number of times (-1 for no limit, 1 for "only once"). They are read from each entity's `connections` block, separated by commas or by the ESC character newer Hammers write; written with commas, or ESC when a part holds a comma (Garry's Mod's tools read only commas, so a comma in a parameter is warned about). Delays are written in the shortest form that reads back exactly (`0.001` stays `0.001`).
  - **Untouched lines are kept exactly**: an edit rewrites only the lines that changed; the others keep their spacing, separators and place, as do lines that can't be read, readable lines past the limit (1,024 per entity), and the block's name. At most 256 characters per part; fuzz-tested, including that writing what was read gives back the same file data.
  - `SetConnections` replaces an entity's connections (one undo step).
  - Names: a target matches an entity's `targetname` without regard to case, and a `*` anywhere matches everything from there on, as the game compares names (`do*r` matches every name starting with "do"). Targets starting with `!` (`!player`, `!self`, `!activator`…) are the game's own and name no entity.
- **Properties** shows **Outputs**: for each connection its output, target and input, each typed freely or picked from a list (the class's outputs from the entity definitions, the map's named entities, the target's inputs), parameter, delay and **Only once**, with a remove button, and **Add output**. Warnings: no target yet, no entity with that name (or class), the target has no such input in the game's list, a comma in the parameter. Showing an entity never changes it (a delay outside the field's range is shown as it is); dragging a delay is one undo step. **Connected to this** lists the connections reaching the entity; clicking one selects where it comes from. The class's inputs are listed for reference.
- **The views** draw a line from each entity to the entities its outputs reach, with an arrow towards the receiver: for the selection by default (green leaving it, blue reaching it); the View menu can show all connections or none. Which entities are joined is worked out again only when names or connections change (`Document::logic_revision`); where the lines run, when something moves. A brush picked inside a brush entity shows the entity's lines. The outputs editor's map-wide lists are worked out on the same terms.

## Consequences

- Outputs with wildcard targets draw a line to every match.
- Without the game's entity definitions, outputs and inputs are typed as text, without lists or input warnings.
- A node view, and checking connections before compiling, can build on this later.
