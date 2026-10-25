# halberd-geom

**Layer 1 · Core**

Geometry: planes, convex shapes, booleans, convex splitting.

## Purpose

All the maths of shapes: planes, convex brushes, shape operations (extrude, bevel, split, bridge), booleans, and splitting editable shapes into the convex brushes Source requires.

## Must never

- Know about entities, files, the GPU or UI
- Allocate without bound on degenerate input

## Depends on

none (no other Halberd crates). Outside library: `glam` (vector maths).

## Status

Convex brushes are implemented.

- **`Plane`**: normal (pointing out of the solid) and distance.
- **`Aabb`**: axis-aligned box, for bounds and box drawing.
- **`Brush`**: a convex solid stored as planes, exactly as VMF stores brushes. Each face's polygon is cut from a huge square by the other planes, in double precision, with corners snapped onto whole units when within 0.01. Faces wind counter-clockwise seen from outside. `cuboid`, `from_planes` (planes of any normal length are rescaled; duplicate and unused planes are dropped), `translated`, `bounds`, `ray_hit` (a ray starting inside a brush ignores it), `is_axis_aligned_box`. Each face knows which input plane it came from (`Face::source`), so per-face data such as materials follows faces through moves and rebuilds.
- **Shapes** (`build_shape`): ready-made `Shape`s fitted into a box: box, wedge (a ramp), cylinder and cone (3 to 64 sides), sphere (4 to 16 sides around, within the 128-face limit), arch (a doorway half ring, one brush per side, with a thickness) and stairs (solid steps of about a chosen height, one brush each). Corners are rounded to whole units and kept inside the box, and rounded outlines are kept convex, so shapes compile cleanly; a sphere's corners can fall between units where its bands meet, as in Hammer. A `Heading` says which way a ramp or stairs climb and an arch spans. Fuzz-tested with random boxes and settings.
- **Editing by corners** (`brush/edit.rs`): `points` lists a brush's corners once each, `edges` and `face_points` refer to them by number. `with_moved_points` moves some corners and rebuilds the brush as the convex hull of the new points (Quickhull, in double precision): a face that no longer lies flat folds in two, a corner pushed inside disappears, and points all in one plane are refused. Moving nothing gives back the very same brush. Each new face's `source` is the old face it shares most corners with (faces stay in the old order), and a `FaceFate` per face says whether it kept exactly its corners; faces that only slid keep their exact planes. Rounded corners are handled within 0.02 units: thin leaning slivers are turned to bound the points, and slivers that add nothing are dropped when a shape would go over 128 faces. `topology` gives corners, face corners and edges in one go. Fuzz-tested with 600 random corner moves across shapes and side counts, checking that every point stays inside and no corner bulges out.
- **Booleans** (`brush/boolean.rs`): `subtract` (this brush minus a cutter, as convex pieces cut one cutter face at a time; `Subtracted::Untouched`, `Gone` or `Pieces`), `intersection` (the common part, or `None`), `overlaps`. Faces made by the cutter get sources one past this brush's faces. Fuzz-tested with boxes on whole units: what is left plus the common part fills the brush. Pieces thinner than a unit can't be brushes and are left out (Halberd's Subtract says how much space that was).
- **Floor plans** (`polygon.rs`): `clean_outline` tidies a drawn outline (drops repeated corners and corners on a straight line, turns it counter-clockwise) and drops corners within a quarter unit of straight, and refuses one that crosses itself (`CrossesItself`), is smaller than a square unit, or has more than 256 corners; `convex_parts` splits an outline with dents into few convex parts: straight cuts that continue a dent's wall first (`polygon/cuts.rs`; an L becomes two rectangles), otherwise ear clipping, then Hertel–Mehlhorn merging (at most four times the fewest possible); `prism_brushes` stands each part up between two heights as a brush, and refuses (`TooSharp`) a part whose brush corners would drift from the outline (very sharp corners make nearly parallel sides). Ears are clipped widest first, to avoid slivers. Tested with 200 random star-shaped outlines (every part convex, areas add up, never more parts than triangles) and 400 random outlines with dents up to 8,000 units out (the brushes fill exactly the outline, or the outline is refused as too sharp), plus a 63-tooth comb, and L, U and plus shapes that split into rectangles.
- **Shape operations** (`brush/ops.rs`): `volume`, `extruded_face` (a new brush standing on a face), `bevelled` (edges cut off a set distance back, refused with `BevelTooWide` if a cut would reach another corner), `cut` (the parts behind and in front of a plane; a plane that only grazes cuts nothing), `mirrored` (about a position on an axis, faces in order), `without_faces` (neighbours grow to close the gap, or `WouldOpen`), and in `brush/edit.rs`, `without_points` and `hull` (the convex brush around any points). New faces' sources are one past the old faces; fates say which old faces kept exactly their corners.
- **Transforms**: `transformed` (any matrix), `rotated` (around an axis through a pivot) and `scaled` (keeping an anchor in place), worked on the planes in double precision; a quarter turn of a grid-aligned box lands on whole units again.
- **Limits** (`GeomError` explains each in plain words): at most 128 faces, at least 1 unit thick, within ±131072 units, planes must enclose a solid.

Tests include a random test: 2,000 random plane sets either make a valid brush, whose every corner lies behind every plane, or give an error, never a crash.
