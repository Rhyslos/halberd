# 0029. Showing where a map leaks

- **Status:** Accepted
- **Date:** 2026-10-24

## Problem

A Source map must be sealed: no gap may join the inside, where the entities are, to the empty space around the map. When it isn't, vbsp stops and lighting and visibility can't be worked out. vbsp says only "leaked"; finding the gap by eye in a large map is the most frustrating part of compiling in Hammer. Halberd should show where it is.

## Options

1. **Say that it leaked**, as before, and leave the hunt to the user.
2. **Work out the leak ourselves** (flood-fill from the entities through the brushes). Duplicates vbsp, and could disagree with it.
3. **Read vbsp's own leak path.** On a leak vbsp writes a pointfile (`<map>.lin`): a line of points from outside the map, through the gap, to the entity that "sees" outside. Hammer draws the same file.

## Decision

Option 3.

- **Reading:** `halberd-compile` reads the pointfile after a leak (one `x y z` point per line; at most 100,000 points and 16 MB; lines that aren't points are skipped; fewer than two points is no path). A stale pointfile is deleted before each compile, so an old path is never taken for a new one. The path reaches the program as `CompileEvent::LeakPath`, just before the leak error.
- **Showing:** every view draws the line in red with a dark edge (readable over red brushes), on top of the map, with the two ends marked: a square **Outside** and a dot **Leaking entity**. In the 3D view, parts behind the camera are cut off at the camera rather than dropped, so the line still points the way while you fly along it. A banner at the bottom of the viewport says "The map has a leak", with **Go to leak** (points every view at the whole line) and **Hide leak**.
- **Clearing:** the line goes when a compile or leak check succeeds, when another map is opened or a new one started, or when hidden. A failed compile for another reason keeps it.
- **Check for leaks:** a new File menu entry runs vbsp alone with `-leaktest`, which is much quicker than a full compile; nothing is copied to the game or played, and success says "No leaks found: the map is sealed."

## Consequences

- The line is only as good as vbsp's: it shows one leak at a time; after fixing it, the next check may find another.
- The line is drawn over everything (not hidden behind walls), so it can always be followed; where it crosses a wall is the place to look.
- Needs the compile tools (Windows), as compiling does. A Halberd-side seal check without the compilers could come later with the Phase 2 compile work.
