# 0023. Materials: reading textures, the browser and per-face materials

- **Status:** Accepted
- **Date:** 2026-10-24

## Problem

With the game's content mounted ([0022](0022-game-content.md)), Halberd can show and apply real materials. A material is a VMT text file naming a VTF texture; GMod and Half-Life 2 have tens of thousands of them. The browser must list them all at once, show pictures without freezing the window, and the Textured view must draw the real textures without running out of GPU memory.

## Options

1. **A VTF library from crates.io** (`vtf` and similar). Small and rarely updated; some panic on damaged files, and they pull in an image library for formats Halberd decodes in a few lines.
2. **Our own reader** in `halberd-vtf`: the header, the resource table of newer versions, and the decoders for the formats materials use (DXT1, DXT3, DXT5 and the uncompressed formats).

For textures on the GPU:

1. **One GPU texture per material**, each with its own bind group. Many draw calls, one per material.
2. **One texture array** of fixed-size layers (256 × 256, with mipmaps), each material's texture resized into a layer; faces say which layer to read and the texture's real size.

## Decision

Option 2 for both.

- **Reading:** `Vtf::parse` checks every size and offset before reading; `decode` picks the largest stored size that fits what is asked for; a texture stored only at full size is shrunk to fit (or refused when it is over 4096 × 4096), so a picture is never bigger than asked for. Unsupported formats (palettes, HDR) give a reason, never a crash; both readers are fuzz-tested. `Material::parse` reads a VMT with the KeyValues reader, follows `patch` materials to the material they include (at most 8 deep), laying each patch's parameters over it, and spots tool materials. Material files over 1 MB are refused before they are read.
- **Listing:** every `materials/*.vmt` in the archives and loose folders, named as Hammer names them (`BRICK/BRICKWALL001A`), listed once after mounting, on the background thread.
- **Pictures** are decoded on a background thread (`MaterialLoader`), only for tiles on screen, the views' textures first and then the newest requests, so what is on screen now comes before what scrolled past; the browser keeps the 600 shown most recently.
- **Choosing and applying:** a click picks the material new brushes and shapes get; a double-click or **Apply to selection** puts it on every face of the selected brushes, shapes and brush entities, or only on the picked faces in Face mode. Applying is one undo step ("Apply material to 3 faces").
- **Textured view:** up to 256 materials get a layer; the texture is laid out with its real size, so alignment matches the game. A material the map no longer shows gives its layer back, so the limit counts what is on screen now. Materials still loading, missing, or past the 256th keep the coloured pattern from [0021](0021-render-modes.md).

## Consequences

- Textures are shown at most 256 pixels across in the view; detail on large textures is softer than in the game.
- Animated, cube-map and blended (two-texture) materials show their first frame, face or texture only.
- A map with more than 256 materials shows patterns for the rest; a later milestone can grow the array.
- The array takes about 90 MB of graphics memory once the first texture arrives, whatever the number of materials; growing it in steps would help small graphics cards.
