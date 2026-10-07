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

`halberd-config`, `halberd-vpk`, `halberd-gma`, `halberd-mdl`, `halberd-vtf`. Outside library: `steamlocate` (finds Steam and its libraries).

## Status

GMod install detection implemented:

1. A folder chosen by the user or saved in settings is used if it is a valid install.
2. Otherwise every Steam installation on the system is searched, including extra Steam libraries on other drives.
3. A valid install has `garrysmod/gameinfo.txt`. The user may pick either the `GarrysMod` folder or the `garrysmod` folder inside it.
4. The compile tools (vbsp, vvis, vrad, bspzip) are looked up in `bin/win64` first, then `bin`.
5. The Workshop content folder is found next to the install when it exists.

Mounting, indexing and caching arrive in Phase 1.
