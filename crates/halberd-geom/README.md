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
- **Editing by corners** (`brush/edit.rs`): `points` lists a brush's corners once each, `edges` and `face_points` refer to them by number. `with_moved_points` moves some corners and rebuilds the brush as the convex hull of the new points (Quickhull, in double precision): a face that no longer lies flat folds in two, a corner pushed inside disappears, and points all in one plane are refused. Each new face's `source` is the old face it shares most corners with, and a `FaceFate` per face says whether it kept exactly its corners; faces that only slid keep their exact planes. Fuzz-tested with 2,000 random corner moves on every shape.
- **Transforms**: `transformed` (any matrix), `rotated` (around an axis through a pivot) and `scaled` (keeping an anchor in place), worked on the planes in double precision; a quarter turn of a grid-aligned box lands on whole units again.
- **Limits** (`GeomError` explains each in plain words): at most 128 faces, at least 1 unit thick, within ±131072 units, planes must enclose a solid.

Tests include a random test: 2,000 random plane sets either make a valid brush, whose every corner lies behind every plane, or give an error, never a crash.
