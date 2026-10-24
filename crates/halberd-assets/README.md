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

Materials (`materials.rs`, see [decision 0023](../../docs/decisions/0023-materials.md)):

1. `material_names` lists every `materials/*.vmt` in the archives and loose folders (at most 200,000 loose ones; `GameContent::loose_files`), named as Hammer names them (`BRICK/BRICKWALL001A`), sorted.
2. `load_material` reads a material, follows `patch` materials to what they include (at most 8 deep), and decodes its texture at most `max_size` pixels on each side (`MaterialImage`: the picture, the texture's full size, whether it is a tool material, and a plain-language problem if there is no picture).
3. `MaterialLoader` does that on its own thread: `request` a material at a size (asked once until it arrives), `poll` for finished ones; the `wake` callback lets the window redraw. The biggest size waiting is read first (the views' textures), then the newest request. Material files over 1 MB are refused unread (`GameContent::read_at_most`).

Models (`models.rs`, see [decision 0024](../../docs/decisions/0024-props.md)):

1. `model_names` lists every `models/*.mdl` in the archives and loose folders (at most 200,000 loose ones), as maps name them (`models/props_c17/oildrum001.mdl`, lower case), sorted.
2. `load_model` reads a model's `.mdl`, `.vvd` and `.vtx` (`.dx90.vtx`, else `.vtx`, `.dx80.vtx`, `.sw.vtx`) and finds each of its materials in the model's material folders, in order (`LoadedModel`).
3. `ModelLoader` does that on its own thread, each model asked for once until it arrives.

**Entity definitions** (`fgd.rs`): `find_fgd` looks in Garry's Mod's `bin` folder for `garrysmod.fgd`, then `halflife2.fgd` and `base.fgd`, then any `.fgd`; `load_fgd` reads it with `halberd-fgd` (see [decision 0031](../../docs/decisions/0031-entity-definitions.md)).
