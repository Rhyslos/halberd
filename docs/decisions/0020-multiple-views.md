# 0020. Several views in one viewport panel

- **Status:** Accepted
- **Date:** 2026-10-24

## Problem

Hammer users work in four views: 3D, and flat Top, Front and Side views for placing things exactly. The roadmap asks for up to six views with preset layouts and all six side views (Top, Bottom, Front, Back, Left, Right).

## Options

1. **One dock panel per view.** Each view a separate panel that can be dragged anywhere. Flexible, but six panels crowd the layout and the panel system, and the views would need to share tools across panels.
2. **One viewport panel divided into views**, with preset layouts. Simple to use (one menu), keeps the dockable panels as they are, and the views share the tools naturally.

## Decision

Option 2.

- **Layouts:** one view, two side by side, two stacked, three (one large), four (as in Hammer), six. Picked from **Views** in the viewport's corner or the View menu.
- **What a view shows:** its name in its corner is a menu: Perspective, or a side view. Side views follow Hammer's axes (Top X/Y, Front Y/Z, Right X/Z); Bottom, Back and Left look the other way. The first four views start as Hammer's four.
- **Side views are flat** (no perspective): right or middle drag moves them, scrolling zooms around the pointer. Their grid is drawn flat behind the map, without fading.
- **Who gets the mouse:** the camera controls (right and middle button, wheel, flying keys) go to the view under the pointer, kept while one of those buttons is held. The tools (left button, tool keys) go to the view under the pointer too, kept while the left button is held or a drag is in progress, so a drag never jumps to another view. A floor plan being drawn moves with the pointer to any view it can be drawn in (3D, Top, Bottom), so corners can be placed from several views.
- **The player figure** stands in one place, worked out from the 3D view, and shows there in every view.
- **Shared tools:** one selection, one gizmo, one drag, shown in every view.
- **Drawing shapes** needs a floor to stand them on, so for now it works in the 3D, Top and Bottom views; the other side views say so. Drawing in side views (as in Hammer's 2D views) can come later.
- Views and their cameras are remembered between sessions.

## Consequences

- Each view is drawn every frame it is shown, so six views cost about six times the drawing of one; fine for blockout maps, worth watching for very large ones.
- The toolbars sit in the first view; in small views they can cover part of it.
- Side-view rays start at the edge of a standard map (±16384), so things beyond it can't be clicked in a side view. Side views can be moved up to ±32768 from the origin.
- Images of views a layout no longer shows are let go, to save graphics memory.
