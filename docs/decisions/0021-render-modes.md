# 0021. Render modes, and textures before textures exist

- **Status:** Accepted
- **Date:** 2026-10-24

## Problem

The roadmap asks for three ways to draw: wireframe, flat and textured. Halberd can't read Garry's Mod's textures yet (that is the next milestone: reading VPK archives), so "textured" has nothing to show.

## Options

1. **Wait:** ship wireframe and flat now, textured with the VPK milestone. Simple, but the mode switch and texture coordinates would be untested until then.
2. **Textured with a stand-in:** lay out texture coordinates exactly as Source will (each face's saved texture axes and scale), and draw a pattern per material in place of the image. Real textures later replace only the pattern.

## Decision

Option 2.

- **Each view chooses** how it draws from its name menu. Side views start as wireframes, as in Hammer; the 3D view starts flat.
- **Wireframe** skips faces and draws outlines in a light colour, keeping a tint for entities (teal for brush entities such as `func_detail`, purple for point entities), as Hammer's 2D views do.
- **Textured** gives every face texture coordinates from its saved `uaxis`/`vaxis` (or Hammer's world alignment for new faces), laid out for 128-texel textures, and colours each material by a hue from its name. A checker every quarter texture and a line at each texture edge show alignment, scale and rotation. Selected faces keep a red tint.

- **Texture axes that no longer fit a face** (it was turned, or Extrude copied a wall's look onto a top) would smear the texture into streaks in the game. Such faces are shown, and saved, with world-aligned axes keeping their scale, so the view and the game agree. Faces whose axes fit are saved exactly as read.
- Damaged values are ignored: scales under 0.001 units per texel and shifts over a million texels count as unreadable.

## Consequences

- When real textures arrive, the shader samples them with the same coordinates, using each texture's real size.
- Two materials can get similar colours; the pattern is a guide, not the final look.
