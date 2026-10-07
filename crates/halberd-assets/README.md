# halberd-assets

**Layer 2 · Services**

Find, mount, index and cache game content.

## Purpose

Finds the GMod install, mounts base, mounted-game and Workshop content, indexes what is available, and keeps decoded assets in caches that respect the memory budget.

## Must never

- Block the interface thread
- Run Lua or any code found in addons
- Write into the game or Workshop folders

## Depends on

`halberd-config`, `halberd-vpk`, `halberd-gma`, `halberd-mdl`, `halberd-vtf`

## Status

Skeleton only. The roadmap milestone that fills this crate in will replace this line.
