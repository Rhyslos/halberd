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

- **Data** (`halberd-doc`): a `Connection` has the output, target name, input, parameter, delay and number of times (-1 for no limit, 1 for "only once"). They are read from each entity's `connections` block, separated by commas or by the ESC character newer Hammers write; written with commas, or ESC when a part holds a comma. Lines that can't be read are kept as they are. At most 1,024 per entity, 256 characters per part; fuzz-tested.
  - `SetConnections` replaces an entity's connections (one undo step).
  - Names: a target matches an entity's `targetname` without regard to case, and a target ending in `*` matches every name starting with the rest, as in the game. Targets starting with `!` (`!player`, `!self`, `!activator`…) are the game's own and name no entity.
- **Properties** shows **Outputs**: for each connection its output (a list of the class's outputs, from the entity definitions), target (text, or picked from the map's named entities), input (a list of the target's inputs), parameter, delay and **Only once**, with a remove button, and **Add output**. Warnings: no target yet, no entity with that name, the target has no such input. **Connected to this** lists the connections reaching the entity; clicking one selects where it comes from. The class's inputs are listed for reference.
- **The views** draw a line from each entity to the entities its outputs reach, with an arrow towards the receiver: for the selection by default (green leaving it, blue reaching it); the View menu can show all connections or none. Lines are worked out again only when the map or selection changes.

## Consequences

- Outputs with wildcard targets draw a line to every match.
- Without the game's entity definitions, outputs and inputs are typed as text, without lists or input warnings.
- A node view, and checking connections before compiling, can build on this later.
