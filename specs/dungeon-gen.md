# Dungeon Generation

Source: `src/mklev.c`, `src/mkroom.c`, `src/mkmaze.c`, `src/sp_lev.c`, `src/rect.c`, `src/dungeon.c`, `dat/dungeon.lua`, `include/rm.h`, `include/mkroom.h`, `include/global.h`

## 1. Map Dimensions

```
COLNO = 80   // columns (x axis), column 0 unused for gameplay
ROWNO = 21   // rows (y axis)
```

Actual usable area: x in `[1, 78]`, y in `[0, 20]`. Rooms and corridors are placed within these bounds with additional margins (XLIM=4, YLIM=3 from room edges).

## 2. Dungeon Branch Structure

The dungeon topology is defined in `dat/dungeon.lua`. Each entry specifies base level count, random range, flags, branches, and named special levels.

### 2.1 Branch Definitions

| Branch | Base Levels | Range | Flags | Entry Direction |
|--------|------------|-------|-------|-----------------|
| The Dungeons of Doom | 25 | 5 | (none) | down |
| Gehennom | 20 | 5 | mazelike, hellish | down |
| The Gnomish Mines | 8 | 2 | mazelike | down |
| The Quest | 5 | 2 | (none) | portal |
| Sokoban | 4 | 0 | mazelike | up |
| Fort Ludios | 1 | 0 | mazelike | portal |
| Vlad's Tower | 3 | 0 | mazelike | up |
| The Elemental Planes | 6 | 0 | mazelike | up |
| The Tutorial | 2 | 0 | mazelike, unconnected | down |

### 2.2 Branch Connection Rules

Branches are connected from a parent dungeon to a child. Connection types:

| Connection | Parent | Child | Type | Position in Parent |
|-----------|--------|-------|------|--------------------|
| Doom -> Mines | Doom | Mines | stair (down) | base=2, range=3 from Doom start |
| Doom -> Sokoban | Doom | Sokoban | stair (up) | chained to Oracle level, base=1 above |
| Doom -> Quest | Doom | Quest | portal | chained to Oracle, base=6, range=2 |
| Doom -> Fort Ludios | Doom | Ludios | portal | base=18, range=4; actually placed by `mk_knox_portal()` in a vault, depth > 10 and < Medusa |
| Doom -> Gehennom | Doom | Gehennom | no_down (stair on Castle only) | chained to Castle level, base=0 |
| Doom -> Elemental Planes | Doom | Planes | no_down, up | base=1 |
| Gehennom -> Vlad's Tower | Gehennom | Vlad's | stair (up) | base=9, range=5 |

### 2.3 Named Special Levels in Each Branch

**Dungeons of Doom:**
- `rogue` (dlvl 15 +/- 4, roguelike)
- `oracle` (dlvl 5 +/- 5, neutral)
- `bigrm` (dlvl 10 +/- 3, 40% chance, 13 variants)
- `medusa` (dlvl -5 +/- 4 from bottom, 4 variants, chaotic)
- `castle` (bottom level)

**Gehennom:**
- `valley` (dlvl 1 = top of Gehennom)
- `sanctum` (bottom level)
- `juiblex` (dlvl 4 +/- 4)
- `baalz` (dlvl 6 +/- 4)
- `asmodeus` (dlvl 2 +/- 6)
- `wizard1` (dlvl 11 +/- 6)
- `wizard2` (chained 1 above wizard1)
- `wizard3` (chained 2 above wizard1)
- `orcus` (dlvl 10 +/- 6)
- `fakewiz1` (dlvl -6 +/- 4 from bottom)
- `fakewiz2` (dlvl -6 +/- 4 from bottom)

**Gnomish Mines:**
- `minetn` (Mine Town, dlvl 3 +/- 2, 7 variants, town)
- `minend` (bottom level, 3 variants)

**Quest:**
- `x-strt` (dlvl 1 +/- 1)
- `x-loca` (dlvl 3 +/- 1)
- `x-goal` (bottom level)

**Sokoban:** `soko1`..`soko4` (each has 2 variants), entered from bottom going up.

**Vlad's Tower:** `tower1`..`tower3`, entered from top going down (entry = -1).

**Elemental Planes:** `astral` (top), `water`, `fire`, `air`, `earth`, `dummy` (entry = -2, entering at earth level).

### 2.4 Fort Ludios Portal Placement

The Fort Ludios portal is special. Its branch source dungeon is initially set to an invalid value (`n_dgns`). The function `mk_knox_portal()` corrects this when:

1. The level is in the main dungeon (Doom).
2. Not the Quest entry level.
3. `depth(&u.uz) > 10` and `< depth(&medusa_level)`.
4. A vault exists on the level.
5. No other branch already exists on this level.
6. 1/3 chance (2/3 chance of deferring to a later level).

## 3. Level Generation Algorithm

Entry point: `mklev()` -> `makelevel()`.

### 3.1 Level Type Decision

```
pseudocode makelevel():
    if Is_special(u.uz) and not Is_rogue_level:
        makemaz(special_level_proto)          // Lua special level
    else if dungeons[dnum].proto[0] != '\0':
        makemaz("")                           // branch has proto pattern (e.g., Gehennom "hellfill")
    else if dungeons[dnum].fill_lvl[0] != '\0':
        makemaz(fill_lvl)                     // branch fill level
    else if In_quest:
        makemaz(role-specific fill)
    else if In_hell or (rn2(5) and depth > medusa_depth):
        makemaz("")                           // random maze
    else:
        // regular rooms+corridors level
        makerooms() -> sort_rooms() -> generate_stairs() -> makecorridors()
        -> make_niches() -> vault -> special_rooms -> fill_rooms
```

### 3.2 Regular Level Generation (Rooms + Corridors)

#### 3.2.1 Rectangle System

Before room placement, the level is divided into a pool of free rectangles. Initially there is one rectangle covering the entire map `[0, COLNO-1] x [0, ROWNO-1]`.

```
n_rects_max = (COLNO * ROWNO) / 30 = (80 * 21) / 30 = 56
```

When a room is placed, its bounding rectangle (including a margin) is removed from the free pool by `split_rects()`, which subdivides the overlapping free rectangle into up to 4 smaller ones.

#### 3.2.2 Room Placement (`makerooms()`)

Rooms are created in a loop until:
- `nroom >= MAXNROFROOMS - 1` (i.e., 39 rooms), OR
- `rnd_rect()` returns NULL (no free rectangles)

```
MAXNROFROOMS = 40
```

**Vault attempt:** When `nroom >= MAXNROFROOMS / 6` (i.e., >= 7) and `rn2(2)` and vault not yet tried, attempt to create a vault (2x2 room).

**Themed rooms:** If a Lua themerms file is loaded for the current dungeon, `themerooms_generate()` is called for each room attempt. If themed room creation fails more than 10 times, or `nroom >= MAXNROFROOMS / 6`, the loop breaks.

**Random room creation** (`create_room(-1, -1, -1, -1, -1, -1, OROOM, -1)`):

1. Pick a random free rectangle via `rnd_rect()`.
2. Generate random room dimensions:
   ```
   width  = 2 + rn2(rect_width > 28 ? 12 : 8)   // range [2, 13] or [2, 9]
   height = 2 + rn2(4)                             // range [2, 5]
   if width * height > 50:
       height = 50 / width
   ```
3. Apply border margins: XLIM=4 on each side horizontally, YLIM=3 vertically (XLIM+1 or YLIM+1 if touching map edge).
4. Check via `check_room()` that the room fits without overlapping existing rooms.
5. If successful, split free rectangles and call `add_room()`.

**Room lighting:** `litstate_rnd(rlit)` where rlit = -1 (random):
```
lit = (rnd(1 + abs(depth)) < 11) && rn2(77)
```
Deeper levels are less likely to be lit. At depth 1, probability ~ 76/77. At depth 10, chance of `rnd(11) < 11` is always true. At depth 20, `rnd(21) < 11` is ~48%.

[疑似 bug] The lighting formula `rnd(1 + abs(depth)) < 11 && rn2(77)` means that at depth 10, `rnd(11) < 11` is always true (rnd returns 1..10), so rooms are lit 76/77 of the time. But at depth >= 11, this starts declining, contradicting the comment "on low levels the room is lit (usually)". The threshold seems oddly placed at depth 10 instead of being a smooth gradient.

#### 3.2.3 Room Construction (`do_room_or_subroom()`)

For non-special rooms:
1. Set HWALL for top and bottom edges (y = ly-1 and y = hy+1).
2. Set VWALL for left and right edges (x = lx-1 and x = hx+1).
3. Set ROOM for interior cells.
4. Set corner types: TLCORNER, TRCORNER, BLCORNER, BRCORNER.

Room coordinates are clamped:
```
if lowx == 0: lowx = 1
if lowy == 0: lowy = 1
if hix >= COLNO - 1: hix = COLNO - 2
if hiy >= ROWNO - 1: hiy = ROWNO - 2
```

#### 3.2.4 Room Sorting

After all rooms are created, `sort_rooms()` sorts them left-to-right by `lx` coordinate using qsort. This ensures `makecorridors()` connects adjacent rooms sequentially.

### 3.3 Staircase Placement (`generate_stairs()`)

Stairs are placed after rooms are created but before corridors:

```
pseudocode generate_stairs():
    if not bottom_level:
        room = generate_stairs_find_room()
        pos = somexyspace(room)
        mkstairs(pos, DOWN)
    if dlevel != 1:
        room = generate_stairs_find_room()
        pos = somexyspace(room)
        mkstairs(pos, UP)
```

**Room selection** (`generate_stairs_find_room()`) uses a multi-phase relaxation:
- Phase 2: OROOM only, no existing stairs, needjoining=true
- Phase 1: Also allows THEMEROOM
- Phase 0: Allows rooms already containing stairs
- Phase -1: Allows unjoined rooms (fallback)

Within each phase, eligible rooms are collected and one is chosen randomly.

### 3.4 Corridor Connection Algorithm (`makecorridors()`)

Three passes ensure full connectivity:

```
pseudocode makecorridors():
    // Pass 1: Connect sequential rooms (may stop early with 1/50 chance)
    for a = 0 to nroom-2:
        join(a, a+1, nxcor=false)
        if !rn2(50): break

    // Pass 2: Connect rooms 2 apart if not yet in same equivalence class
    for a = 0 to nroom-3:
        if smeq[a] != smeq[a+2]:
            join(a, a+2, nxcor=false)

    // Pass 3: Full connectivity - connect any disconnected components
    repeat until no changes:
        for each pair (a, b):
            if smeq[a] != smeq[b]:
                join(a, b, nxcor=false)

    // Pass 4: Extra random corridors (may be blocked)
    if nroom > 2:
        for i = rn2(nroom) + 4 times:
            a = rn2(nroom)
            b = rn2(nroom - 2), adjusted to skip a and a+1
            join(a, b, nxcor=true)  // nxcor=true allows blocking
```

**`join(a, b, nxcor)`**: Determines relative positions of rooms a and b, finds door positions on appropriate walls via `finddpos()`, then calls `dig_corridor()`.

- If `nxcor=true` and the first corridor tile is already non-STONE, the corridor is abandoned.
- Extra corridors (`nxcor=true`) have a 1/35 chance per step of aborting.

**Equivalence classes** (`smeq[]`): When `join()` succeeds, `smeq[a]` and `smeq[b]` are merged (smaller value wins).

### 3.5 Corridor Digging (`dig_corridor()`)

The corridor is dug from origin to destination using a biased random walk:

```
pseudocode dig_corridor(org, dest, nxcor, ftyp=CORR, btyp=STONE):
    determine initial direction dx, dy (towards dest)
    while not at dest:
        if step_count > 500 or (nxcor and !rn2(35)): return FAIL
        advance position by (dx, dy)
        if cell == btyp (STONE):
            if ftyp == CORR and maybe_sdoor(100):
                cell = SCORR   // secret corridor, prob = 1/max(2, 100) if depth > 2
            else:
                cell = ftyp
                if nxcor and !rn2(50): place BOULDER
        else if cell != ftyp and cell != SCORR:
            return FAIL        // hit something unexpected

        // Direction adjustment: biased towards reducing larger distance component
        dix = abs(xx - tx), diy = abs(yy - ty)
        if dix > diy and diy > 0 and !rn2(dix - diy + 1): prefer vertical
        if diy > dix and dix > 0 and !rn2(diy - dix + 1): prefer horizontal
        // Try to turn towards destination if current direction is blocked
```

### 3.6 Door Placement

#### 3.6.1 Door Creation (`dodoor()`)

```
dodoor(x, y, aroom):
    if maybe_sdoor(8):   // depth > 2 and !rn2(max(2, 8)) => 1/8 chance at depth > 2
        type = SDOOR
    else:
        type = DOOR
    dosdoor(x, y, aroom, type)
```

#### 3.6.2 Door State (`dosdoor()`)

For regular DOOR type:
```
if !rn2(3):                              // 1/3 chance of having a door panel
    if !rn2(5):       D_ISOPEN           // 1/15 overall
    else if !rn2(6):  D_LOCKED           // 4/90 = 2/45 overall
    else:             D_CLOSED           // remainder of 1/3
    if (not open) and (not shop door) and level_difficulty >= 5 and !rn2(25):
        add D_TRAPPED                    // trap on closed/locked doors
else:                                    // 2/3 chance
    if shop_door: D_ISOPEN
    else:         D_NODOOR               // empty doorway
```

Overall door state probabilities (non-shop, difficulty >= 5):
| State | Probability |
|-------|------------|
| D_NODOOR | 2/3 |
| D_ISOPEN | 1/15 |
| D_CLOSED (no trap) | ~(1/3)(4/6)(24/25) = ~0.213 |
| D_CLOSED + D_TRAPPED | ~(1/3)(4/6)(1/25) = ~0.009 |
| D_LOCKED (no trap) | ~(1/3)(1/6)(1/5)(24/25) = ~0.011 |
| D_LOCKED + D_TRAPPED | ~(1/3)(1/6)(1/5)(1/25) = ~0.0004 |

For SDOOR type:
```
if shop_door or !rn2(5): D_LOCKED       // shop doors always locked; otherwise 1/5
else:                     D_CLOSED       // 4/5
if not shop_door and level_difficulty >= 4 and !rn2(20):
    add D_TRAPPED
```

**Mimic replacement:** If a door is D_TRAPPED, difficulty >= 9, and 1/5 chance, and mimics not genocided, the door is replaced by D_NODOOR with a mimic monster.

#### 3.6.3 Secret Doors

`maybe_sdoor(chance)`: returns true if `depth > 2` and `!rn2(max(2, chance))`.

- At corridor doors: `maybe_sdoor(8)` => 1/8 chance at depth > 2.
- In niches: `maybe_sdoor(100)` for secret corridors (SCORR), very rare (1/100).

### 3.7 Niches (`make_niches()`)

```
count = rnd((nroom / 2) + 1)
```

Each niche attempt:
1. Pick a random room (must be OROOM; if 1 door, 4/5 chance of skipping).
2. Find a door position on the north or south wall.
3. The cell beyond the door (into stone) becomes either SCORR (with trap) or CORR.

Niche types:
- Level teleporter niche: if `depth > 15` and `!rn2(6)` (at most one per level)
- Trapdoor niche: if `5 < depth < 25` and `!rn2(6)` (at most one)
- Normal niche: SCORR (1/4 chance) or CORR with optional secret door (`rn2(7)` ? `rn2(5) ? SDOOR : DOOR` : iron bars)

## 4. Maze Level Generation

### 4.1 Random Maze (`makemaz("")`)

When no special level proto is found:

```
pseudocode makemaz(""):
    level.flags.is_maze_lev = 1
    level.flags.corrmaze = !rn2(3)           // 1/3 chance corridors, 2/3 rooms

    if not Invocation_level and rn2(2):
        create_maze(-1, -1, !rn2(5))         // random corridor/wall widths, 1/5 deadend removal
    else:
        create_maze(1, 1, false)             // standard 1-wide corridors, 1-wide walls

    if not corrmaze: wallification(2, 2, x_maze_max, y_maze_max)

    place up stairs, down stairs (or vibrating square on Invocation level)
    place_branch()
    populate_maze()
```

### 4.2 Maze Grid Construction (`create_maze()`)

```
pseudocode create_maze(corrwid, wallthick, rmdeadends):
    if corrwid == -1: corrwid = rnd(4)          // [1, 4]
    if wallthick == -1: wallthick = rnd(4) - corrwid  // may be negative
    clamp wallthick to [1, 5], corrwid to [1, 5]

    scale = corrwid + wallthick
    rdx = x_maze_max / scale
    rdy = y_maze_max / scale

    // Initialize grid
    if corrmaze:
        fill [2..rdx*2) x [2..rdy*2) with STONE
    else:
        fill: odd cells = STONE, even cells = HWALL

    // Generate maze using recursive backtracking (walkfrom)
    start = random odd position in [3, x_maze_max] x [3, y_maze_max]
    walkfrom(start.x, start.y, typ=0)

    if rmdeadends: maze_remove_deadends(CORR or ROOM)

    // Scale up if scale > 2
    if scale > 2: expand each maze cell
```

### 4.3 Maze Walk Algorithm (`walkfrom()`)

Recursive backtracking maze generation:

```
pseudocode walkfrom(x, y, typ):
    if typ == 0:
        typ = corrmaze ? CORR : ROOM
    set levl[x][y].typ = typ (unless it's a DOOR)

    loop:
        collect valid directions (where 2 steps away is STONE and in bounds)
        if none: return
        pick random valid direction
        carve 1 step in that direction (set to typ)
        carve 2nd step (set to typ)
        recurse walkfrom(new_x, new_y, typ)
```

The `okay(x, y, dir)` check ensures the target (2 cells away) is STONE and within bounds `[3, x_maze_max] x [3, y_maze_max]`.

### 4.4 Maze Population (`populate_maze()`)

```
objects:  rn1(8, 11) = [11, 18] random objects (GEM_CLASS or RANDOM_CLASS)
boulders: rn1(10, 2) = [2, 11] boulders
minotaurs: rn2(3) = [0, 2]
monsters:  rn1(5, 7) = [7, 11] random monsters
gold:      rn1(6, 7) = [7, 12] gold piles
traps:     rn1(6, 7) = [7, 12] traps
```

## 5. Special Level Loading

Special levels are defined as Lua scripts in `dat/`. The loading path:

```
makemaz(protoname) -> load_special(protoname.lua)
```

If the special level has random variants (rndlevs > 0), the actual file loaded is `protoname-N.lua` where N = `rnd(rndlevs)`.

The Lua level scripts use an API of functions like:
- `des.level_init()` - initialize level type (maze/rooms/cavern)
- `des.level_flags()` - set level flags
- `des.map()` - place ASCII art map sections
- `des.room()` - define rooms
- `des.monster()`, `des.object()`, `des.trap()` - place entities
- `des.stair()`, `des.ladder()`, `des.door()` - place connections
- `des.mazewalk()` - generate maze from a starting point
- `des.terrain()` - set terrain types
- `des.levregion()` - define teleport/stair regions

After loading, `fixup_special()` handles post-processing (placing branches, Medusa statues, etc.).

## 6. Terrain Types

All 37 terrain types from `include/rm.h`:

| Value | Name | Description |
|-------|------|-------------|
| 0 | STONE | Solid rock (default) |
| 1 | VWALL | Vertical wall |
| 2 | HWALL | Horizontal wall |
| 3 | TLCORNER | Top-left corner |
| 4 | TRCORNER | Top-right corner |
| 5 | BLCORNER | Bottom-left corner |
| 6 | BRCORNER | Bottom-right corner |
| 7 | CROSSWALL | Cross wall (+) |
| 8 | TUWALL | T-wall pointing up |
| 9 | TDWALL | T-wall pointing down |
| 10 | TLWALL | T-wall pointing left |
| 11 | TRWALL | T-wall pointing right |
| 12 | DBWALL | Drawbridge wall (closed drawbridge) |
| 13 | TREE | Tree |
| 14 | SDOOR | Secret door |
| 15 | SCORR | Secret corridor |
| 16 | POOL | Pool of water |
| 17 | MOAT | Moat (non-boiling pool) |
| 18 | WATER | Deep water (wall-like) |
| 19 | DRAWBRIDGE_UP | Drawbridge span location (when closed) |
| 20 | LAVAPOOL | Lava pool |
| 21 | LAVAWALL | Lava wall |
| 22 | IRONBARS | Iron bars |
| 23 | DOOR | Door |
| 24 | CORR | Corridor |
| 25 | ROOM | Room floor |
| 26 | STAIRS | Staircase |
| 27 | LADDER | Ladder |
| 28 | FOUNTAIN | Fountain |
| 29 | THRONE | Throne |
| 30 | SINK | Kitchen sink |
| 31 | GRAVE | Grave |
| 32 | ALTAR | Altar |
| 33 | ICE | Ice |
| 34 | DRAWBRIDGE_DOWN | Open drawbridge |
| 35 | AIR | Open air (Plane of Air) |
| 36 | CLOUD | Cloud (Plane of Air) |

Key classification macros:
- `IS_WALL(typ)`: 1..12 (VWALL through DBWALL)
- `IS_STWALL(typ)`: 0..12 (STONE through DBWALL)
- `IS_OBSTRUCTED(typ)`: 0..15 (anything below POOL)
- `ACCESSIBLE(typ)`: >= 23 (DOOR and above)
- `IS_ROOM(typ)`: >= 25 (ROOM, STAIRS, furniture...)
- `IS_POOL(typ)`: 16..19 (POOL through DRAWBRIDGE_UP)
- `IS_FURNITURE(typ)`: 26..32 (STAIRS through ALTAR)
- `SPACE_POS(typ)`: > 23 (strictly above DOOR)

## 7. Special Room Types

### 7.1 Room Type Enum

| Value | Type | Min Depth | Probability (per-level) |
|-------|------|-----------|------------------------|
| 0 | OROOM | -- | default |
| 1 | THEMEROOM | -- | from Lua themes |
| 2 | COURT | > 4 | `!rn2(6)` = 1/6 |
| 3 | SWAMP | > 15 | `!rn2(6)` = 1/6 |
| 4 | VAULT | -- | special creation |
| 5 | BEEHIVE | > 9 | `!rn2(5)` = 1/5 (if bees not genocided) |
| 6 | MORGUE | > 11 | `!rn2(6)` = 1/6 |
| 7 | BARRACKS | > 14 | `!rn2(4)` = 1/4 (if soldiers not genocided) |
| 8 | ZOO | > 6 | `!rn2(7)` = 1/7 |
| 9 | DELPHI | -- | special level only |
| 10 | TEMPLE | > 8 | `!rn2(5)` = 1/5 |
| 11 | LEPREHALL | > 5 | `!rn2(8)` = 1/8 (if leprechauns not genocided) |
| 12 | COCKNEST | > 16 | `!rn2(8)` = 1/8 (if cockatrices not genocided) |
| 13 | ANTHOLE | > 12 | `!rn2(8)` = 1/8 (if ant types available) |
| 14+ | SHOPBASE+ | > 1, < medusa | see below |

### 7.2 Special Room Selection Priority

The checks are evaluated in order; exactly zero or one special room is created per level:

```
pseudocode (u_depth = depth(&u.uz)):
    if wizard and SHOPTYPE env: shop
    else if u_depth > 1 and u_depth < medusa_depth and nroom >= threshold and rn2(u_depth) < 3: SHOP
    else if u_depth > 4 and !rn2(6): COURT
    else if u_depth > 5 and !rn2(8) and leprechauns exist: LEPREHALL
    else if u_depth > 6 and !rn2(7): ZOO
    else if u_depth > 8 and !rn2(5): TEMPLE
    else if u_depth > 9 and !rn2(5) and bees exist: BEEHIVE
    else if u_depth > 11 and !rn2(6): MORGUE
    else if u_depth > 12 and !rn2(8) and ant types available: ANTHOLE
    else if u_depth > 14 and !rn2(4) and soldiers exist: BARRACKS
    else if u_depth > 15 and !rn2(6): SWAMP
    else if u_depth > 16 and !rn2(8) and cockatrices exist: COCKNEST
```

The `room_threshold` is 3 normally, 4 if a branch exists on this level.

### 7.3 Room Selection for Special Rooms

**`pick_room(strict)`**: Picks a random OROOM without stairs:
- Start at random index, iterate all rooms.
- Skip rooms with `rtype != OROOM`.
- If `strict=false`: skip rooms with upstairs; skip rooms with downstairs 2/3 of time.
- If `strict=true`: skip rooms with any stairs.
- Accept if `doorct == 1` OR `!rn2(5)` OR wizard mode.

**Shop requirements** (additional): Must have exactly 1 door (or wizard override), no stairs, and `invalid_shop_shape()` must be false. The room is lit upon becoming a shop.

**`isbig(room)`**: area > 20 cells. Big rooms cannot be wand or book shops (forced to general store).

### 7.4 Special Room Stocking

All special rooms are stocked after room/corridor generation via `fill_special_room()`.

**Zoo/Court/Morgue/etc.** (`fill_zoo()`): For each cell in the room:
- Skip cells adjacent to the first door.
- Place a sleeping monster appropriate to room type.
- Place room-specific items.

**Monster types by room:**

| Room Type | Monster Selection |
|-----------|------------------|
| COURT | `courtmon()`: rn2(60)+rn2(3*difficulty); >100 dragon, >95 giant, >85 troll, >75 centaur, >60 orc, >45 bugbear, >30 hobgoblin, >15 gnome, else kobold |
| ZOO | Random (NULL) |
| BEEHIVE | Killer bees (queen bee at center) |
| MORGUE | `morguemon()`: rn2(100); if hd>10 & i<10 demon; if hd>8 & i>85 vampire; <20 ghost, <40 wraith, else zombie |
| BARRACKS | `squadmon()`: rnd(80+difficulty); soldier(80), sergeant(15), lieutenant(4), captain(1) |
| LEPREHALL | Leprechaun |
| COCKNEST | Cockatrice |
| ANTHOLE | `antholemon()`: soldier ant, fire ant, or giant ant (based on birthday % 3 + difficulty) |
| SWAMP | Giant eel (rn2(5)), piranha, electric eel; fungus on dry cells |

**Court throne monster** (`mk_zoo_thronemon()`):
```
i = rnd(level_difficulty())
if i > 9: Ogre Tyrant
else if i > 5: Elven Monarch
else if i > 2: Dwarf Ruler
else: Gnome Ruler
```

**Swamp** (`mkswamp()`): Converts up to 5 existing OROOM rooms. Cells where `(x+y) % 2 == 1` become POOL with aquatic monsters; other cells may get fungi (`!rn2(4)`).

## 8. Level Features

### 8.1 Feature Placement Probabilities

In `fill_ordinary_room()` for each OROOM/THEMEROOM:

| Feature | Probability | Notes |
|---------|------------|-------|
| Sleeping monster | 1/3 (or always if have Amulet) | Random type; spider gets web |
| Traps | While `!rn2(x)` where `x = max(2, 8 - difficulty/6)` | Up to 1000 attempts |
| Gold pile | 1/3 | |
| Fountain | 1/10 | `mkfount()` |
| Sink | 1/60 | `mksink()` |
| Altar | 1/60 | `mkaltar()`, random alignment |
| Grave | `1/max(2, 80 - depth*2)` | More common deeper |
| Statue | 1/20 | Random monster type |
| Chest/box | `1/(nroom * 5 / 2)` | 2/3 large box, 1/3 chest |
| Random object | 1/3, then while `!rn2(5)` more | |

### 8.2 Fountain Details

```
mkfount():
    find unoccupied, non-bydoor position in room (up to 200 tries)
    set typ = FOUNTAIN
    if !rn2(7): blessedftn = 1  // 1/7 chance of blessed fountain
    level.flags.nfountains++
```

### 8.3 Altar Details

```
mkaltar():
    if room.rtype != OROOM: return
    find position
    set typ = ALTAR
    alignment = rn2(A_LAWFUL + 2) - 1  // uniform over {-1, 0, 1} = {chaotic, neutral, lawful}
    altarmask = Align2amask(alignment)
```

### 8.4 Temple Altar

```
mktemple():
    room = pick_room(strict=true)  // no stairs at all
    shrine position = center of room (with rounding jitter for even dimensions)
    typ = ALTAR
    altarmask = induced_align(80) | AM_SHRINE
    place priest/priestess
```

### 8.5 Grave Details

```
mkgrave():
    if room.rtype != OROOM: return
    1/10 chance of "Saved by the bell!" inscription (+ bell object)
    1/3 chance of buried gold: rnd(20) + difficulty * rnd(5) pieces
    rn2(5) random cursed buried objects
```

### 8.6 Sink Details

Simple placement: `set typ = SINK`, increment `level.flags.nsinks`.

## 9. Vault Generation

### 9.1 Vault Creation

A vault is a special 2x2 room (or 2x2 interior + walls = 4x4 total) that is:
- Not connected to corridors.
- Reachable only via teleport traps.
- Filled with gold.

```
pseudocode vault_generation:
    if do_vault():
        w = h = 1   // interior size = 2x2
        if check_room(vault_x, w, vault_y, h, vault=true):
            add_room(vault_x, vault_y, vault_x+w, vault_y+h, lit=TRUE, VAULT)
            level.flags.has_vault = 1
            fill vault with gold: rn1(abs(depth) * 100, 51) per cell
            mk_knox_portal(vault_corner)   // may place Ludios portal
            if !noteleport and !rn2(3):
                makevtele()                // teleport trap in a niche
```

### 9.2 Vault Gold

Each cell in the vault gets:
```
gold_amount = rn1(abs(depth) * 100, 51) = 51 + rn2(abs(depth) * 100)
```
For a 2x2 vault at depth 10, that's 4 piles of 51..1050 gold each.

## 10. Mineralization

After level topology is finalized, `mineralize()` seeds the rock with gold and gems:

```
goldprob = 20 + depth / 3
gemprob = goldprob / 4
if In_mines: goldprob *= 2, gemprob *= 3
if In_quest: goldprob /= 4, gemprob /= 6
```

For each eligible STONE cell (surrounded by STONE on all sides, not W_NONDIGGABLE):
- Gold: `rn2(1000) < goldprob`, amount = `1 + rnd(goldprob * 3)`, 1/3 buried
- Gems: `rn2(1000) < gemprob`, count = `rnd(2 + dunlev/3)`, 1/3 buried each

Kelp is placed in water/moat:
- Pool/water: 1/10 chance per cell
- Moat: 1/30 chance per cell

## 11. Branch Type Mechanics

```c
enum branch_type {
    BR_STAIR,    // bidirectional stair
    BR_NO_END1,  // stair on end2 only
    BR_NO_END2,  // stair on end1 only
    BR_PORTAL    // magic portal (bidirectional)
};
```

- `BR_STAIR`: Normal bidirectional stairs between levels (e.g., Doom <-> Mines).
- `BR_NO_END1` / `BR_NO_END2`: One-way connection (e.g., Doom -> Gehennom has `no_down`, meaning the stair down from Castle is the only connection).
- `BR_PORTAL`: Magic portal trap (e.g., Quest, Fort Ludios).

`place_branch()` creates either a portal trap or a stairway depending on `br->type`.

## 12. Wall Spine Fix-up

After room/corridor placement, `wallification()` performs two passes:

1. **`wall_cleanup()`**: Remove walls surrounded entirely by stone (convert to STONE).
2. **`fix_wall_spines()`**: Determine correct wall type (VWALL, HWALL, corners, T-walls, cross-walls) based on neighboring walls using a 4-bit encoding:
   ```
   bits = (N_spine << 3) | (S_spine << 2) | (E_spine << 1) | W_spine
   spine_array[bits] gives the correct wall type
   ```

## 13. Level Finalization (`level_finalize_topology()`)

```
1. bound_digging()      // mark outer stone as W_NONDIGGABLE
2. mineralize()         // place gold/gems in rock, kelp in water
3. topologize() rooms   // assign roomno to cells
4. set_wall_state()     // finalize wall display states
5. copy rtype -> orig_rtype for all rooms
```

---

## 测试向量

### Room Dimensions

| # | rect_width | rn2(8 or 12) result | rn2(4) result | Expected width | Expected height | area > 50? | Adjusted height |
|---|-----------|---------------------|---------------|----------------|-----------------|------------|-----------------|
| 1 | 20 (<=28) | 0 | 0 | 2 | 2 | no | 2 |
| 2 | 20 | 7 | 3 | 9 | 5 | no | 5 |
| 3 | 30 (>28) | 11 | 3 | 13 | 5 | 13*5=65>50 | 50/13=3 |
| 4 | 30 | 0 | 0 | 2 | 2 | no | 2 |
| 5 | 20 | 7 | 2 | 9 | 4 | no | 4 |

### Door State (non-shop, difficulty >= 5)

| # | rn2(3) | rn2(5) | rn2(6) | rn2(25) | Expected state |
|---|--------|--------|--------|---------|---------------|
| 1 | 0 (has panel) | 0 (open) | -- | -- | D_ISOPEN |
| 2 | 0 | 1 | 0 (locked) | 0 (trapped) | D_LOCKED \| D_TRAPPED |
| 3 | 0 | 1 | 1 | 24 (no trap) | D_CLOSED |
| 4 | 1 (no panel) | -- | -- | -- | D_NODOOR |
| 5 | 2 (no panel) | -- | -- | -- | D_NODOOR |

### Special Room Selection

| # | u_depth | rn2 results | nroom | threshold | Expected room |
|---|---------|-------------|-------|-----------|---------------|
| 1 | 2 | rn2(2)=1 | 4 | 3 | SHOP (depth>1, <medusa, nroom>=3, rn2(2)<3) |
| 2 | 5 | rn2(5)=3, rn2(6)=0 | 5 | 3 | COURT (depth>4, rn2(6)=0) |
| 3 | 7 | rn2(7)=..., rn2(6)=3, rn2(8)=2, rn2(7)=0 | 5 | 3 | ZOO (depth>6, rn2(7)=0) |
| 4 | 10 | all fail until rn2(5)=0 for BEEHIVE | 5 | 3 | BEEHIVE (depth>9, bees exist) |
| 5 | 1 | -- | 5 | 3 | none (depth=1, shop requires >1) |

### Lighting

| # | depth | rnd(1+abs(depth)) | rn2(77) | Expected lit |
|---|-------|-------------------|---------|-------------|
| 1 | 1 | rnd(2) in [1,2]; <11 always | 0 | FALSE (rn2(77)=0) |
| 2 | 1 | rnd(2)=1 | 50 | TRUE |
| 3 | 10 | rnd(11) in [1,10]; <11 always | 76 | TRUE |
| 4 | 20 | rnd(21); say 15; 15<11 is false | -- | FALSE |
| 5 | 20 | rnd(21); say 5; 5<11 is true | 40 | TRUE |

### Boundary: Maximum rooms

| # | Scenario | Expected |
|---|----------|----------|
| 1 | nroom = 38 (< MAXNROFROOMS-1=39), rnd_rect returns valid | create another room |
| 2 | nroom = 39 (= MAXNROFROOMS-1) | stop loop, no more rooms |

### Boundary: Vault gold

| # | depth | rn2 result | Expected gold per cell |
|---|-------|-----------|----------------------|
| 1 | 1 | 0 | 51 (minimum: 51 + 0) |
| 2 | 1 | 99 | 150 (51 + 99) |
| 3 | 30 | 2999 | 3050 (51 + 2999, max at depth 30) |

### Maze: corridor width and wall thickness

| # | rnd(4) for corrwid | rnd(4) for wallthick calc | wallthick raw | clamped | scale |
|---|-------------------|--------------------------|---------------|---------|-------|
| 1 | 1 | rnd(4)=1; 1-1=0 | 0 | 1 | 2 |
| 2 | 4 | rnd(4)=1; 1-4=-3 | -3 | 1 | 5 |
| 3 | 2 | rnd(4)=4; 4-2=2 | 2 | 2 | 4 |
| 4 | 3 | rnd(4)=3; 3-3=0 | 0 | 1 | 4 |

### Corridor: secret corridor in dig_corridor

| # | depth | rn2(max(2,100)) | Expected |
|---|-------|-----------------|----------|
| 1 | 1 | -- | No secret (depth <= 2) |
| 2 | 2 | -- | No secret (depth <= 2) |
| 3 | 3 | 0 | SCORR (depth>2, rn2(100)=0) |
| 4 | 3 | 50 | CORR (rn2(100)!=0) |

### Niche count

| # | nroom | rnd result | Expected niche count |
|---|-------|-----------|---------------------|
| 1 | 2 | rnd(2)=1 | 1 |
| 2 | 10 | rnd(6)=6 | 6 |
| 3 | 1 | rnd(1)=1 | 1 (minimum) |
| 4 | 39 | rnd(20)=20 | 20 (maximum practical) |
