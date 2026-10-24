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
- **Settings they start with** are Hammer's usual ones: lights white at brightness 200, the spotlight pointing down (`pitch -90`) with a 45° cone, the sun at `pitch -50` with sky light (`_ambient`).
- **A map has one sun:** picking Sun again selects the one already there, with a note.
- **Shown as:** spawn points as player-sized green boxes (their origin at the feet, as in the game), lights yellow, the sun orange; spawns, spotlights and the sun with a line showing which way they face. A light's `pitch` setting is read as the compiler reads it (its sign turned round).
- **Properties** edits a light's colour and brightness, a spotlight's cone, the pitch of spotlights and the sun, and the facing of spawns, spotlights and the sun (one undo step per drag, `SetEntitySettings`). Other settings stay read-only, listed.
- **Sky:** a picker in the Entities tab sets the world's `skyname` (`SetWorldSetting`, undoable), from the skies in the game's materials (`skybox/<name>up`), or Half-Life 2's when the game isn't found. A new map saves Hammer's usual world settings with the chosen sky in place.

## Consequences

- The FGD milestone will replace the fixed list and the few fields with every class and setting; the commands stay.
- Sky brushes are made by putting `tools/toolsskybox` on brushes with the material browser; Halberd doesn't build a sky box for you yet.
