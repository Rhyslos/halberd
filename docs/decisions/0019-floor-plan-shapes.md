# 0019. Floor plan shapes, split into convex brushes on save

- **Status:** Accepted
- **Date:** 2026-10-23

## Problem

Source compiles only convex brushes, so in Hammer an L-shaped room or a winding corridor has to be built from several boxes by hand, and every later change means moving several of them. The roadmap asks for automatic convex splitting on save and export: builders should be able to make shapes with dents and let Halberd cut them up. This needs a kind of object that may be concave, and a way to save it that Hammer and the compiler still understand.

## Options

1. **Any concave solid**, split in 3D on save. The most general, but splitting arbitrary solids is hard to do well (many thin pieces), and there is no simple way to draw or edit one yet.
2. **Floor plans**: an outline that may have dents, stood up between two heights. Most concave shapes in a blockout are exactly this (rooms, corridors, platforms, walls that turn corners), splitting a flat outline into few convex parts is a solved problem, and drawing one is a matter of clicking corners.
3. **No new object**: split the outline into brushes as soon as it is drawn. Simple, but the shape is lost straight away, which is what the milestone wants to avoid.

## Decision

Option 2. A floor plan is drawn with the Draw tool (Floor plan in its shape list) by clicking corners; it stays one object in the editor and is split only when saved.

- **Splitting:** the outline is cut into triangles (ear clipping, widest ears first so thin slivers are avoided), then neighbouring triangles are merged while the result stays convex (Hertel–Mehlhorn). This gives at most four times the fewest convex parts possible, usually the fewest; an L gives two. Each part becomes one brush.
- **Clean brushes only:** a drawn corner within a quarter unit of straight is dropped, and every brush is checked to have its corners exactly where the outline puts them. An outline with a corner so sharp that its brush would drift (and leave gaps that leak) is refused with a reason rather than saved wrong.
- **Saving:** the parts are written as ordinary world brushes, so Hammer, the compiler and any other tool see normal brushes. Each carries two extra keys in its `editor` block, `halberd_shape` (which shape it belongs to) and `halberd_outline` (the outline and heights), and an orange editor colour. Hammer keeps unknown editor keys.
- **Opening:** brushes with the same shape number become the shape again only if nothing about them changed: the same pieces face for face, one material, Halberd's own texture alignment and editor settings. If someone moved, reshaped, retextured, deleted, copied or grouped a piece in Hammer, the pieces open as ordinary brushes with those changes kept, and lose Halberd's marks so they can never be mixed up with a later shape. The Console says so. Rebuilding stops after 100,000 outline corners, so a damaged file can't make opening hang.
- **Ids:** a shape that was read from a file keeps its pieces' solid and side ids when saved again, as brushes do, so overlays and decals that point at its faces stay correct.
- **Look:** a shape has one material for all its faces; textures are world-aligned on each face when saved.
- **Editing:** a shape can be selected, moved, scaled and turned about the upright axis (it always stands upright). **Make brushes from shape** (Ctrl+Shift+B) turns it into ordinary brushes for anything else.

## Consequences

- Shapes are world brushes only for now; putting one into a `func_detail` means making brushes from it first.
- Corners cannot yet be moved after drawing; to change the outline, draw it again or make brushes from it. Editing the outline is a natural later step.
- Shape operations (extrude, booleans and the rest) work on brushes, not shapes; a shape among the selection is left alone by them.
- The same save-and-regroup approach can carry other editor-only shapes later (curved walls, terrain pieces).
