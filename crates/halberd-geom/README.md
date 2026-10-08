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
- **`Brush`**: a convex solid stored as planes, exactly as VMF stores brushes. Each face's polygon is cut from a huge square by the other planes, in double precision, with corners snapped onto whole units when within 0.001. Faces wind counter-clockwise seen from outside. `cuboid`, `from_planes` (planes of any normal length are rescaled; duplicate and unused planes are dropped), `translated`, `bounds`, `ray_hit` (a ray starting inside a brush ignores it), `is_axis_aligned_box`.
- **Limits** (`GeomError` explains each in plain words): at most 128 faces, at least 1 unit thick, within ±131072 units, planes must enclose a solid.

Tests include a random test: 2,000 random plane sets either make a valid brush, whose every corner lies behind every plane, or give an error, never a crash.
