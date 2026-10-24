# 0031. The full entity list and keyvalue editing

- **Status:** Accepted
- **Date:** 2026-10-24

## Problem

Phase 1 placed four basic entities and showed every other entity's settings read-only. A real map needs Garry's Mod's whole entity list (triggers, doors, NPCs, logic), with each entity's keyvalues edited by their meaning: a list of choices, ticks for flags, a colour. Hammer gets all of this from the game's FGD files.

## Options

1. **Ship our own entity list.** Out of date with the game, and a lot to maintain.
2. **Read the game's own FGD files**, as Hammer does: `garrysmod.fgd` in Garry's Mod's `bin` folder, which includes Half-Life 2's and the base game's.

## Decision

Option 2.

- **Reading** (`halberd-fgd`): classes (base, point and solid, plus the NPC, filter, key frame and move kinds, which are point entities), their helpers (`size`, `color`, `studio`, `iconsprite`…), keyvalues with type, label, default, description, choices and flags, and inputs and outputs. `@include` is followed beside the file (never outside its folder), included files first. `[ … ]` blocks of other editors' lists (`@MaterialExclusion`, `@AutoVisGroup`) are skipped. A class that can't be read is reported with its file and line and the rest is still read. Input is capped (file size, tokens, items, choices, includes) and fuzz-tested.
  - **Base classes are kept apart** from point and solid classes: Valve's files have a base class `Light` and a point class `light`.
  - **A class with its bases** (`Fgd::resolved`): the bases' keys first, each key once (a later definition replaces an earlier one in place), cycles and missing bases skipped.
- **Finding** (`halberd-assets`): `bin/garrysmod.fgd`, else `halflife2.fgd`, `base.fgd`, else any `.fgd` in `bin`. Read in the background with the game's content. Without one, the Console says so and the basic entities still work.
- **The Library's Entities tab** keeps the basic entities and the sky, and adds **All entities**: a search over every point and solid class. Clicking a point entity places it with its default keyvalues (and default flags) at its defined size. Clicking a brush entity ties the selected brushes to it (Hammer's "Tie to entity"); with nothing selected it waits, saying why.
- **Properties** lists the selected entity's class (changeable), every keyvalue its class has, with the label and a tooltip of the description, edited by type: a list for choices, ticks for flags, a colour button (and brightness), text for the rest. Keys the definitions don't know are listed as text; keys can be added and removed (removed keys fall back to their default). Inputs and outputs are listed (connecting them is the next milestone). A brush entity can move its brushes back to the world. Without definitions, every key is listed as text.
  - **Text goes in on Enter or when the field is left**, as in Hammer, so typing a name is one undo step; Escape puts it back. Lists, ticks and colours change at once.
  - **Hammer's own `id` is hidden**: it numbers objects in the file and isn't a setting.
- **Point entities are shown at their defined size** (`size(…)`), in every view, for picking and the map's size, in any map opened.
- **New edits:** `TieToEntity`, `MoveToWorld` (a brush entity left with no brushes goes, unless it holds brushes Halberd keeps unseen), `SetEntityClass` (letters, digits and `_` only) and `RemoveEntitySettings` (never the class or origin). Each is one undo step.

## Consequences

- Entities show as boxes in their colour; models from `studio(...)` and sprites from `iconsprite(...)` aren't drawn yet.
- Keyvalue types without their own editor yet (sounds, materials, models, target names) are typed as text; pickers for them can come later.
- The FGD files are read once at startup; editing them needs a restart.
