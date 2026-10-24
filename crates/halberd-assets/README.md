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

Mounting game content (`content.rs`, `GameContent`):

1. `gameinfo.txt`'s `game` search paths are read in order (`search_paths`); `|gameinfo_path|` and `|all_source_engine_paths|` are filled in, platform conditions such as `[$WIN32]` are skipped, and a trailing `*` (addons) mounts every folder and VPK archive inside (at most 4096 per `*`; the search paths after it are still mounted). Paths that would leave the install are skipped; `.`, `./` and `` name the base folder.
2. Each place is a folder or a VPK archive (`.vpk` paths mount their `_dir.vpk`). Places that don't exist are skipped; archives that can't be read become notes (`problems`).
3. `read` and `contains` look in each place in turn; the first that has the file wins, as in the engine; a copy that can't be read (damaged) gives way to the next place that has it. Archive lists are in memory; folders are only looked into when asked, matching names without regard to case. Paths that would leave the game (`..`, absolute) are refused.
4. `archived_files` lists archived files by folder and extension (for the material browser); `summary` gives a line for the Console.

Indexing loose files and caching decoded assets arrive with the browsers.
