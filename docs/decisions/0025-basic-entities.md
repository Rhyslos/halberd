# 0025. Basic entities: spawn points, lights, the sun and the sky

- **Status:** Accepted
- **Date:** 2026-10-24

## Problem

A playable blockout needs a spawn point, light and a sky. Garry's Mod reads these from entities (`info_player_start`, `light`, `light_spot`, `light_environment`) and from the world's `skyname` setting. Full entity editing (every class, from FGD files) is a Phase 2 milestone; Phase 1 needs just enough to light and spawn into a map.

## Options

1. **Wait for the FGD entity list** and its general editor. Simple now, but Phase 1 maps would compile dark and without spawns.
2. **A short list of basic entities** with sensible settings, placed with the Place tool from props, plus a few editable fields for them in Properties, and a sky picker.

## Decision

Option 2.

- **The Library's Entities tab** lists Player spawn, Light, Spotlight and Sun. A click turns on the Place tool, as for props; it works without the game.
- **Settings they start with:** lights white at brightness 200 and the spotlight pointing down (`pitch -90`) with a 45° cone, as in Hammer; the sun warmer and brighter than Hammer's, tilted down (`pitch -50`) with blue-grey sky light (`_ambient`) and soft shadows (`SunSpreadAngle 5`), so a first test looks right.
- **A map has one sun:** placing it ends placing, and picking Sun again selects the one already there, with a note.
- **Shown as:** spawn points as player-sized green boxes (their origin at the feet, as in the game), lights yellow, the sun orange; spawns, spotlights and the sun with a line showing which way they face. A light's pitch is read as the compiler reads it: its `pitch` setting (or the pitch in `angles` when that is missing or 0), negative pointing down. Turning a light with the gizmo writes `pitch` and `angles` together, as Hammer keeps them.
- **Properties** edits a light's colour and brightness, a spotlight's cone, the pitch of spotlights and the sun, and the facing of spawns, spotlights and the sun (one undo step per drag, `SetEntitySettings`). Values outside a field's range are shown as they are and never changed just by selecting. Narrowing a spotlight's cone narrows its inner cone with it, since the compiler would otherwise widen the cone again. Other settings stay read-only, listed.
- **Sky:** a picker in the Entities tab sets the world's `skyname` (`SetWorldSetting`, undoable), from the skies in the game's materials (`skybox/<name>up`), or Half-Life 2's when the game isn't found. A new map saves Hammer's usual world settings with the chosen sky in place; a map read from a file keeps its world as read (`MapFileData::from_file`).

## Consequences

- The FGD milestone will replace the fixed list and the few fields with every class and setting; the commands stay.
- Sky brushes are made by putting `tools/toolsskybox` on brushes with the material browser; Halberd doesn't build a sky box for you yet.
