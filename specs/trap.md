# Trap System

Source: `src/trap.c`, `include/trap.h`, `include/you.h`, `src/teleport.c`, `src/mklev.c`, `src/detect.c`, `src/mthrowu.c`

## 1. Trap Data Structure

```
struct trap {
    ttyp:       u5          // trap type enum (0..25)
    tseen:      bool        // has been seen by hero
    once:       bool        // has been triggered at least once (for arrow/dart/rock: deplete check)
    madeby_u:   bool        // placed by hero (affects messages, monster anger, disarm)
    tx, ty:     coord       // map position
    dst:        d_level     // destination level (for portals/holes/trapdoors)
    launch:     coord       // launch point for projectile traps / teledest for TELEP_TRAP
    // union vl:
    //   launch_otyp: i16   // object type for projectile
    //   launch2: coord     // secondary launch point (rolling boulder)
    //   conjoined: u8      // bitmask of adjacent conjoined pits
    //   tnote: i16         // squeaky board note index (0..11)
}
```

Hero trap state (in `struct you`):
- `u.utrap: uint` -- turns remaining trapped (0 = not trapped)
- `u.utraptype: enum utraptypes` -- TT_NONE=0, TT_BEARTRAP=1, TT_PIT=2, TT_WEB=3, TT_LAVA=4, TT_INFLOOR=5, TT_BURIEDBALL=6

## 2. Trap Type Enum

| Value | Name               | Floor-trigger | Hideable | Destroyable |
|------:|--------------------|:---:|:---:|:---:|
|     0 | NO_TRAP            | --  | --  | --  |
|     1 | ARROW_TRAP         | yes | yes | yes |
|     2 | DART_TRAP          | yes | yes | yes |
|     3 | ROCKTRAP           | yes | yes | yes |
|     4 | SQKY_BOARD         | yes | yes | yes |
|     5 | BEAR_TRAP          | yes | yes | yes |
|     6 | LANDMINE           | yes | yes | yes |
|     7 | ROLLING_BOULDER_TRAP | yes | yes | yes |
|     8 | SLP_GAS_TRAP       | yes | yes | yes |
|     9 | RUST_TRAP          | yes | yes | yes |
|    10 | FIRE_TRAP          | yes | yes | yes |
|    11 | PIT                | yes | yes | yes |
|    12 | SPIKED_PIT         | yes | yes | yes |
|    13 | HOLE               | yes | **no** | yes |
|    14 | TRAPDOOR           | yes | yes | yes |
|    15 | TELEP_TRAP         | no  | yes | yes |
|    16 | LEVEL_TELEP        | no  | yes | yes |
|    17 | MAGIC_PORTAL       | no  | yes | **no** |
|    18 | WEB                | no  | yes | yes |
|    19 | STATUE_TRAP        | no  | yes | yes |
|    20 | MAGIC_TRAP         | no  | yes | yes |
|    21 | ANTI_MAGIC         | no  | yes | yes |
|    22 | POLY_TRAP          | no  | yes | yes |
|    23 | VIBRATING_SQUARE   | no  | yes | **no** |
|    24 | TRAPPED_DOOR       | --  | --  | --  |
|    25 | TRAPPED_CHEST      | --  | --  | --  |

Notes:
- `floor_trigger()` returns true for types 1..14 (ARROW_TRAP through TRAPDOOR).
- `unhideable_trap()` -- only HOLE is always visible (`tseen = 1` on creation).
- `undestroyable_trap()` -- MAGIC_PORTAL and VIBRATING_SQUARE cannot be overwritten.
- `is_magical_trap()` -- TELEP_TRAP, LEVEL_TELEP, MAGIC_TRAP, ANTI_MAGIC, POLY_TRAP.
- `is_xport()` -- TELEP_TRAP through MAGIC_PORTAL.
- TRAPPED_DOOR (24) and TRAPPED_CHEST (25) are not placed on the map as traps.

## 3. Trap Avoidance (dotrap)

When hero steps onto a trap, avoidance is checked in `dotrap()` before effects:

### 3.1 Floor Trigger Bypass (Flying/Levitation)

```
if NOT Sokoban:
    if floor_trigger(ttype) AND check_in_air(hero, trflags):
        if trap.tseen:
            message "You <step/fly> over <trap>"
        return  // no effect
```

`check_in_air` returns true if:
- `HURTLING` flag set, OR
- hero is Levitating, OR
- hero is Flying AND not deliberately plunging (`TOOKPLUNGE | VIASITTING` not set)

**Exception:** In Sokoban, pits and holes ignore levitation/flying -- "Air currents pull you down."

### 3.2 DEX-based Avoidance

For seen, non-forced traps (not in Sokoban, not Fumbling, not plunging, not conjoined/adjacent pit):

```
if trap.tseen AND NOT Fumbling AND ttype != ANTI_MAGIC
   AND NOT undestroyable(ttype) AND NOT forcebungle AND NOT plunged
   AND NOT conj_pit AND NOT adj_pit:
    if rn2(5) == 0:    // 20% chance to escape
        message "You escape <trap>"
        return
    // special: clinger at pit has better odds
    if is_pit(ttype) AND is_clinger(hero.data):
        // always escapes (the OR condition above means this branch
        // is reached when rn2(5) != 0 but is_clinger)
        message "You escape <trap>"
        return
```

So base avoidance is **1-in-5 (20%)** for seen traps. Clingers always avoid seen pits (non-Sokoban).

### 3.3 Monster Trap Avoidance (mintrap)

Monsters that have previously encountered a trap type (`mon_knows_traps`) or mindless monsters at holes:
```
if already_seen AND rn2(4) AND NOT forcebungle:
    return  // 75% chance to avoid
```

Flying/levitating/clinging monsters skip floor-trigger traps (same `check_in_air` logic).

## 4. Individual Trap Effects

### 4.1 Arrow Trap (ARROW_TRAP = 1)

**Depletion:** If `trap.once` is true AND `trap.tseen` AND `rn2(15) == 0` (1/15 chance): trap is deleted, "You hear a loud click!" No projectile.

**Projectile:** Creates 1 arrow. Sets `trap.once = 1`.

**Hit check (hero):** `thitu(8, dmgval(arrow, hero), ...)`
- Hit formula: `u.uac + 8 <= rnd(20)` => hit
- Damage: `dmgval(arrow, hero)` -- standard weapon damage for arrow vs. hero

**Hit check (monster):** `thitm(8, mon, arrow, 0, false)`
- Hit formula: `find_mac(mon) + 8 + arrow.spe <= rnd(20)` => miss; otherwise hit
- Damage: `dmgval(arrow, mon)`

**Steed:** 50% chance (`!rn2(2)`) steed is hit instead of hero.

### 4.2 Dart Trap (DART_TRAP = 2)

Same depletion logic as arrow trap (1/15).

**Projectile:** Creates 1 dart. `!rn2(6)` => **1/6 chance dart is poisoned**.

**Hit check (hero):** `thitu(7, dmgval(dart, hero), ...)` -- attack level 7 (not 8).

**Poison (hero):** If dart is poisoned and hero is hit:
```
poisoned("dart", A_CON, "little dart", strength_10_or_0, true)
// strength: 10 normally, 0 if life-saving triggered during damage
```

**Hit check (monster):** `thitm(7, mon, dart, 0, false)` -- same poison logic not applied to monsters in thitm (poison only via obj properties).

**Steed:** 50% chance steed is hit.

### 4.3 Rock Trap (ROCKTRAP = 3)

**Depletion:** If `trap.once AND trap.tseen AND !rn2(15)`: "A trap door in the ceiling opens, but nothing falls out!" Trap deleted.

**Damage formula:**
```
base_dmg = d(2, 6)     // 2..12

if wearing_hard_helmet:
    dmg = 2
elif passes_rocks(hero) AND wearing_any_helmet:
    dmg = 2             // helmet prevents phasing
elif passes_rocks(hero) AND NOT wearing_helmet:
    dmg = 0             // "passes harmlessly through you"
else:
    dmg = base_dmg      // soft helmet gives no protection

losehp(Maybe_Half_Phys(dmg))
exercise(A_STR, FALSE)
```

**Monster:** `thitm(0, mon, rock, d(2, 6), false)` -- forced hit (d_override nonzero).

### 4.4 Squeaky Board (SQKY_BOARD = 4)

No damage. Musical note selected per-board (12 chromatic notes C through B).

**Hero (flying/levitating without force):** Just notices "a loose board below you" -- no sound.

**Hero (grounded):** Board squeaks, `wake_nearby(FALSE)` wakes all nearby monsters.

**Monster:** If flying, no effect. Otherwise wakes monsters: `wake_nearto(mx, my, 40)`.

### 4.5 Bear Trap (BEAR_TRAP = 5)

**Avoidance:** Levitation/Flying skip (floor trigger). Also:
- Amorphous/whirly/unsolid hero: "closes harmlessly through you"
- Hero `msize <= MZ_SMALL` (no steed): "closes harmlessly over you"

**Damage:** `d(2, 4)` => 2..8

**Trapping duration:** `rn1(4, 4)` => 4..7 turns

**Wounded legs:** One random side, duration `rn1(10, 10)` => 10..19 turns.

```
set_utrap(rn1(4, 4), TT_BEARTRAP)       // 4..7 turns
set_wounded_legs(random_side, rn1(10, 10))  // 10..19 turns
losehp(Maybe_Half_Phys(d(2, 4)))
exercise(A_DEX, FALSE)
```

**Steed:** Steed takes the damage instead.

**Monster trapping:** Monster is trapped if `msize > MZ_SMALL AND NOT amorphous AND NOT in_air AND NOT whirly AND NOT unsolid`. Damage: `thitm(0, mon, NULL, d(2, 4), false)`.

### 4.6 Land Mine (LANDMINE = 6)

**Flying/Levitating hero:**
- If not yet seen AND `rn2(3)`: no effect at all (returns).
- Otherwise, discovers the trigger. If already seen AND `rn2(3)`: no further effect.
- Air currents set it off: damage applies.

**Grounded hero:**
```
set_wounded_legs(LEFT_SIDE, rn1(35, 41))    // 41..75 turns
set_wounded_legs(RIGHT_SIDE, rn1(35, 41))   // 41..75 turns
exercise(A_DEX, FALSE)
losehp(Maybe_Half_Phys(rnd(16)))            // 1..16
```

After explosion: trap becomes PIT, `blow_up_landmine()` scatters objects (`scatter(x, y, 4, ...)`), wakes monsters within radius 20 (`wake_nearto(x, y, 400)`). Hero then recursively falls into the newly-created pit.

**Monster trigger weight check:**
```
#define MINE_TRIGGER_WT (WT_ELF / 2)
if rn2(mon.data.cwt + 1) < MINE_TRIGGER_WT:
    return  // too light to trigger
```

**Monster damage:** `thitm(0, mon, NULL, rnd(16), false)`.

### 4.7 Rolling Boulder Trap (ROLLING_BOULDER_TRAP = 7)

Triggers `launch_obj(BOULDER, launch_x, launch_y, launch2_x, launch2_y, ROLL)`.

Boulder rolls from launch point toward launch2 point (through the trap location).

**Hit check (hero):** `thitu(9 + boulder.spe, dmgval(boulder, hero), ...)`.

**Boulder interactions during roll:**
- Rock-throwing monsters: 2/3 chance to snatch boulder
- Other boulders: chain reaction, transfers `otrapped` flag
- Pits/holes: boulder fills them
- Landmines: 70% (`rn2(10) > 2`) chance to trigger explosion
- Teleport traps: boulder teleported/level-teleported
- Doors: boulder crashes through
- Walls/trees: "Thump!" -- stops
- Iron bars: `hits_bars()` check

### 4.8 Sleeping Gas Trap (SLP_GAS_TRAP = 8)

**Hero:**
```
if Sleep_resistance OR breathless(hero.data):
    "You are enveloped in a cloud of gas!"  // no effect
else:
    "A cloud of gas puts you to sleep!"
    fall_asleep(-rnd(25), TRUE)             // sleep 1..25 turns
```

**Monster:**
```
if NOT resists_sleep(mon) AND NOT breathless(mon.data) AND NOT helpless(mon):
    sleep_monst(mon, rnd(25), -1)           // sleep 1..25 turns
```

**Steed:** Separate check via `steedintrap()` -- same formula.

### 4.9 Rust Trap (RUST_TRAP = 9)

A gush of water hits a random body part (equal 1/5 chance each):
- 0: head (helmet)
- 1: left arm (shield, then two-weapon/bimanual offhand, then gloves)
- 2: right arm (weapon, then gloves)
- 3-4: body (lit items splashed; cloak, or armor, or shirt)

Each hit item gets `water_damage()` which rusts rustable items, dilutes potions, blanks scrolls/spellbooks.

**Special monster interactions:**
- Iron Golem: `dmg = mhmax` -- "covered with rust!" (usually fatal)
- Gremlin: `rn2(3)` chance (2/3) to split via `split_mon()`

**Hero as Iron Golem:** `losehp(Maybe_Half_Phys(u.mhmax))`.
**Hero as Gremlin:** 2/3 chance to split.

### 4.10 Fire Trap (FIRE_TRAP = 10)

**Hero (`dofiretrap`):**
```
base_dmg = d(2, 4)                     // 2..8

if Underwater:
    losehp(rnd(3), "boiling water")     // reduced
    return

if Fire_resistance:
    num = rn2(2)                        // 0 or 1 damage
else if Upolyd:
    // golem vulnerability
    alt = match u.umonnum:
        PM_PAPER_GOLEM  => u.mhmax
        PM_STRAW_GOLEM  => u.mhmax / 2
        PM_WOOD_GOLEM   => u.mhmax / 4
        PM_LEATHER_GOLEM => u.mhmax / 8
        _               => 0
    num = max(base_dmg, alt)
    // also reduces mhmax by rn2(min(mhmax, num + 1))
else:
    num = d(2, 4)
    // reduces uhpmax by rn2(min(uhpmax, num + 1))
    // may trigger level drain if uhpmax drops below minimum

losehp(num)
burn_away_slime()
burnarmor(hero)     // random armor piece eroded by fire
destroy_items(hero, AD_FIRE, base_dmg)   // scrolls, potions, spellbooks
```

**Monster:** Same d(2, 4) base, golem multipliers. Fire-resistant monsters uninjured. Also: `mtmp->mhpmax -= rn2(num + 1)` (permanent max HP reduction).

### 4.11 Pit (PIT = 11)

**Avoidance:** Levitation/Flying (unless plunged/Sokoban). Clingers avoid (non-Sokoban).

**Trapping:** `set_utrap(rn1(6, 2), TT_PIT)` => **2..7 turns** trapped.

**Damage:**
```
if conjoined_pit:
    // moving between adjacent conjoined pits
    dmg = 0
elif deliberate (menu_requested AND already_known):
    dmg = 0
elif plunged AND (Flying OR clinger):
    dmg = 0                             // flying plungers take no pit damage
elif adjacent_nonconjoined_pit:
    dmg = rnd(3)                        // 1..3
else:
    dmg = rnd(6)                        // 1..6

losehp(Maybe_Half_Phys(dmg))
selftouch("Falling, you")               // cockatrice corpse check
exercise(A_STR, FALSE)
exercise(A_DEX, FALSE)
```

**Monster:** `thitm(0, mon, NULL, rnd(6), false)` for PIT; `rnd(10)` for SPIKED_PIT.

Monsters with `passes_walls` are NOT trapped (mtrapped stays 0).

### 4.12 Spiked Pit (SPIKED_PIT = 12)

Same trapping as PIT (2..7 turns). Additional spike damage:

```
spike_dmg = match:
    conjoined_pit   => rnd(4)           // 1..4
    adjacent_pit    => rnd(6)           // 1..6
    normal          => rnd(10)          // 1..10

losehp(Maybe_Half_Phys(spike_dmg))

if !rn2(6):                             // 1/6 chance
    poisoned("spikes", A_STR, ..., strength_8_or_0, FALSE)
    // strength: 8 normally, 0 if life-saving triggered
```

### 4.13 Hole (HOLE = 13)

Always visible (`tseen = 1` on creation). Falls through to lower level.

**Destination:** Stored in `trap.dst` (set by `hole_destination()`):
```
dst.dnum = current_dungeon
dst.dlevel = current_depth
while dst.dlevel < bottom:
    dst.dlevel++
    if rn2(4):     // 75% chance to stop at each level
        break
```

**Avoidance:** Levitation, ustuck, `!Can_fall_thru`, Flying/clinger (unless plunged), `msize >= MZ_HUGE`, pet jerks you back.

**Sokoban exception:** Cannot avoid in Sokoban.

**Monster:** Huge or larger monsters, non-grounded monsters, long worms (>5 segments) don't fall through. In Sokoban, they are "yanked down."

### 4.14 Trapdoor (TRAPDOOR = 14)

Same mechanics as HOLE but starts hidden. "A trap door opens up under you!"

### 4.15 Teleport Trap (TELEP_TRAP = 15)

NOT a floor trigger. Hero stepped on it:

```
tele_trap(trap):
    if In_endgame OR Antimagic:
        "You feel a wrenching sensation."   // no teleport
    elif !next_to_u():                      // pet adjacent
        shudder
    elif trap.once:                         // vault teleporter
        delete trap
        vault_tele()
    elif fixed_destination(trap):           // teledest set
        teleport to trap.teledest (displacing any monster there)
    else:
        tele()                              // random teleport on level
```

**Monster:** `mtele_trap()` -- teleports monster randomly on level.

### 4.16 Level Teleporter (LEVEL_TELEP = 16)

```
level_tele_trap(trap, trflags):
    if Antimagic AND NOT intentional:
        shieldeff; "wrenching sensation"    // blocked
        return
    if In_endgame:
        "wrenching sensation"               // blocked
        return

    delete trap
    level_tele()                            // interactive level selection

    if NOT Teleport_control:
        make_confused(HConfusion_timeout + 3, FALSE)
```

**Monster:** `mlevel_tele_trap()` -- sends monster to random level.

### 4.17 Magic Portal (MAGIC_PORTAL = 17)

Indestructible. Transports hero to linked destination (`domagicportal(trap)`).

**Monster:** Treated as level teleport.

### 4.18 Web (WEB = 18)

**Immunity:**
- Webmakers (e.g., spiders): walk freely
- Amorphous/whirly/unsolid/gelatinous cube: flow through
- Flaming/acidic: destroy the web and pass through

**Trapping duration (hero):** Based on STR:

| STR Range | Duration Formula |
|-----------|-----------------|
| <= 3      | rn1(6, 6) = 6..11 |
| 4..5      | rn1(6, 4) = 4..9  |
| 6..8      | rn1(4, 4) = 4..7  |
| 9..11     | rn1(4, 2) = 2..5  |
| 12..14    | rn1(2, 2) = 2..3  |
| 15..17    | rnd(2) = 1..2     |
| 18..68    | 1                 |
| >= 69 (18/**) | 0 (tear through, web destroyed) |

**Mounted:** If steed is strong (`strongmonst`), STR treated as 17.

**Monster trapping:** Most monsters trapped. Specific large monsters tear through:
- Explicitly listed: Titanothere, Baluchitherium, Purple Worm, Jabberwock, Iron Golem, Balrog, Kraken, Mastodon, Orion, Norn, Cyclops, Lord Surtur
- By class: S_GIANT, adult dragons (`extra_nasty`), long worms (>5 segments)
- Owlbear/Bugbear: trapped but roar

### 4.19 Statue Trap (STATUE_TRAP = 19)

**Hero only** (monsters do NOT trigger).

Searching adjacent to the statue or stepping on it activates the trap. The trap is deleted, and the first valid statue at the location is animated via `animate_statue()`:
- Monster created is always hostile (mtame=0, mpeaceful=0)
- Statue contents transferred to monster inventory
- If archaeologist and statue is historic: -1 alignment

### 4.20 Magic Trap (MAGIC_TRAP = 20)

**Hero:**
```
if !rn2(30):    // 1/30 chance
    // Magical explosion
    delete trap
    losehp(rnd(10))                 // 1..10
    u.uenmax += 2; u.uen = u.uenmax    // gain 2 max energy, fully restore
    return
else:
    domagictrap()                   // random effect (see below)
```

**domagictrap() effects** (fate = rnd(20)):

| fate   | Effect |
|--------|--------|
| 1..9   | Flash of light: blinded rn1(5,10)=10..14 turns; deafened rn1(20,30)=30..49 turns (or rn1(5,15)=15..19 if already deaf); spawn rnd(4)=1..4 random monsters; wake_nearto(7*7=49) |
| 10     | Nothing happens |
| 11     | Toggle intrinsic invisibility: `HInvis = HInvis ? 0 : FROMOUTSIDE` (if any HInvis bits set, clears ALL; otherwise sets FROMOUTSIDE) |
| 12     | Fire trap effect (dofiretrap) |
| 13..18 | Flavor messages only (shiver, howling, yearning, pack shakes, smell, tiredness) |
| 19     | +1 CHA; tame all adjacent monsters |
| 20     | Uncurse inventory (as if uncursed scroll of remove curse / SPE_REMOVE_CURSE) |

**Monster:** Usually immune. 1/21 chance (`!rn2(21)`) treated as fire trap.

### 4.21 Anti-Magic Trap (ANTI_MAGIC = 21)

**Hero with Antimagic:**
```
dmgval2 = rnd(4)                            // 1..4 base
if Half_physical_damage OR Half_spell_damage:
    dmgval2 += rnd(4)
if wielding Magicbane:
    dmgval2 += rnd(4)
if carrying non-quest artifact conferring MR:
    dmgval2 += rnd(4)
if Passes_walls:
    dmgval2 = (dmgval2 + 3) / 4            // quarter damage

losehp(dmgval2, "anti-magic implosion")
```

**Energy drain (always, hero):**
```
drain = d(2, 6)                             // 2..12
halfd = rnd(drain / 2)                      // 1..drain/2
if u.uenmax > drain:
    u.uenmax -= halfd
    drain -= halfd
drain_en(drain)
```

**Monster without magic resistance:** Increases `mspec_used` by d(2, 6) (delays spell usage), but ONLY if the monster is not cancelled (`!mtmp->mcan`) AND has a magic attack (`attacktype(AT_MAGC)`) or breath attack (`attacktype(AT_BREA)`). Monsters without these attack types are unaffected.

**Monster with magic resistance:** Takes physical damage (same rnd(4) + bonus formula as hero, reduced by `passes_walls`).

### 4.22 Polymorph Trap (POLY_TRAP = 22)

**Hero:**
```
if Antimagic OR Unchanging:
    shieldeff; "momentarily different"  // no polymorph; trap NOT consumed
else:
    delete trap
    polyself(POLY_NOFLAGS)
```

**Monster:**
```
if resists_magm(mon):
    shieldeff                           // blocked
elif NOT resist(mon, WAND_CLASS, 0, NOTELL):
    newcham(mon, NULL, NC_SHOW_MSG)     // random polymorph
```

Trap is NOT deleted for monsters (they can trigger it repeatedly).

### 4.23 Vibrating Square (VIBRATING_SQUARE = 23)

Not a real trap. Marker for the invocation site. No damage, no trapping. Hero feels it. Indestructible.

### 4.24 Trapped Door (TRAPPED_DOOR = 24)

Not placed on the map. Part of the door structure (`doormask & D_TRAPPED`). Triggered when opening/breaking the door: `b_trapped("door", ...)`.

### 4.25 Trapped Chest (TRAPPED_CHEST = 25)

Not placed on the map. Part of object (`obj->otrapped`). Triggered by `chest_trap()`.

**Luck-based save:** If `Luck > -13 AND rn2(13 + Luck) > 7`: trap fizzles (various flavor messages). At max luck (13): `rn2(26) > 7` => ~69% fizzle.

**Actual trap effects** (weighted by luck):
```
effect_index = rn2(20) ? ((Luck >= 13) ? 0 : rn2(13 - Luck)) : rn2(26)
```

| Index   | Effect |
|---------|--------|
| 25..21  | Explosion: d(6,6) damage, destroys all objects at location |
| 20..17  | Poison gas: poisoned("gas cloud", A_STR, ..., 15) or gas cloud |
| 16..13  | Poisoned needle: poisoned("needle", A_CON, ..., 10) |
| 12..9   | Fire trap (dofiretrap on box) |
| 8..6    | Electric shock: d(4,4) damage (0 if Shock_resistance) |
| 5..3    | Paralysis: -d(5,6) turns frozen (blocked by Free_action) |
| 2..0    | Stunning gas: stun rn1(7,16)=16..22, hallucinate rn1(5,16)=16..20 |

## 5. Trap Detection

### 5.1 Searching (#search command)

`dosearch0()` checks all 8 adjacent squares:

```
for each adjacent (x, y):
    // secret doors
    if levl[x][y].typ == SDOOR:
        if NOT rnl(7 - fund):    // fund = artifact_search_bonus + lenses_bonus
            convert to door

    // traps
    if trap at (x,y) AND NOT trap.tseen:
        if NOT rnl(8):           // base ~1/8 chance (adjusted by luck)
            if trap.ttyp == STATUE_TRAP:
                activate_statue_trap()
            else:
                find_trap(trap)  // sets tseen, exercises WIS
```

**Search bonus (`fund`):** Capped at 5.
- Wielding artifact with SPFX_SEARCH: `+uwep.spe`
- Wearing lenses (non-blind): `+2`

`rnl(N)` returns a luck-biased random: for positive luck, result tends lower (better chance). Base probability: roughly `1/8` to find a trap, improved by luck.

### 5.2 Trap Detection Spells/Items

- Wand/scroll/spell of detect traps: reveals all traps on level
- Trap detection results in `tseen = 1` for found traps

### 5.3 Monster Trap Awareness

When a monster triggers a trap, it learns that trap type: `mon_learns_traps(mon, ttype)`. Monsters that see a trap being triggered also learn: `mons_see_trap(trap)`.

Known traps give 75% avoidance (`rn2(4)` check).

## 6. Trap Escape

### 6.1 Bear Trap Escape

Hero: `u.utrap` decrements each turn via normal movement. When it reaches 0, hero is free. No explicit strength check; it's purely turn-based (4..7 turns).

Levitation while trapped: blocked (BLevitation set to I_SPECIAL). Message: "you float up slightly, but your leg is still stuck."

Monster: 1/40 chance per turn (`!rn2(40)`) to pull free. Metallivorous monsters can eat the bear trap.

### 6.2 Pit Escape (`climb_pit`)

```
if Passes_walls:
    instant escape
elif boulder_present AND !rn2(2):
    leg stuck in crevice (no escape this turn)
elif (Flying OR clinger) AND NOT Sokoban:
    climb out
elif (--u.utrap == 0) OR m_easy_escape_pit(hero):
    // m_easy_escape_pit: pit fiend OR msize >= MZ_HUGE
    crawl to edge
else:
    still trapped, utrap decremented
```

Typical pit duration: 2..7 turns (from `rn1(6, 2)`).

Levitation while in pit: instant escape via `float_up()`.

Monster: 1/40 chance per turn. Pit Fiends and MZ_HUGE+ escape easily. Boulders in pit: 50% chance to pull free AND fill pit.

### 6.3 Web Escape

Turn-based: `u.utrap` decrements. Duration depends on STR (see section 4.18).

STR >= 69 (gauntlets of power + 18/**): instant tear-through.

Monster: 1/40 chance per turn to pull free.

### 6.4 Lava Escape

```
u.utrap -= (1 << 8)                        // subtract 256 per turn
if u.utrap < (1 << 8):                     // below 256
    "You sink below the surface and die."   // DISSOLVED
    // life-saving: reset_utrap, safe_teleds
elif !u.umoved:
    u.utrap += rnd(4)                       // 1..4 added back
```

Lava trap time is stored in units of 256 per "tick". Without fire resistance, `u.uhp = (u.uhp + 2) / 3` each turn (66% HP loss).

## 7. Trap Generation

### 7.1 Random Trap Selection (`traptype_rnd`)

`rnd(TRAPNUM - 1)` = rnd(25) = 1..25. Then filtered by level difficulty:

| Trap Type | Min Difficulty | Other Restriction |
|-----------|:-:|---|
| ARROW_TRAP, DART_TRAP, ROCKTRAP, SQKY_BOARD, BEAR_TRAP, PIT, RUST_TRAP, TELEP_TRAP | 1 | TELEP_TRAP: not on no-teleport levels |
| SLP_GAS_TRAP, ROLLING_BOULDER_TRAP | 2 | -- |
| SPIKED_PIT, LEVEL_TELEP | 5 | LEVEL_TELEP: not no-teleport, not single-level branch |
| LANDMINE | 6 | -- |
| WEB | 7 | (lower if MKTRAP_NOSPIDERONWEB) |
| STATUE_TRAP, POLY_TRAP | 8 | -- |
| FIRE_TRAP | -- | **Gehennom only** (In_hell) |
| HOLE | 1 | Only 1/7 chance (`!rn2(7)` to keep) |
| TRAPDOOR | 1 | -- |
| MAGIC_PORTAL | -- | Never random |
| VIBRATING_SQUARE | -- | Never random |
| TRAPPED_DOOR | -- | Never random |
| TRAPPED_CHEST | -- | Never random |

If the random type fails its filter, `kind = NO_TRAP` and no trap is placed.

### 7.2 Rogue Level Traps

Special selection: `rn2(7)` =>
0: BEAR_TRAP, 1: ARROW_TRAP, 2: DART_TRAP, 3: TRAPDOOR, 4: PIT, 5: SLP_GAS_TRAP, 6: RUST_TRAP

### 7.3 Trap Placement

`mktrap()`:
- In rooms: random room location via `somexyspace()`
- In mazes: maze corridor location
- WEB traps: a giant spider is placed on the web (unless MKTRAP_NOSPIDERONWEB)
- Rolling boulder traps: boulder placed 4..8 squares away via `mkroll_launch()`
- Statue traps: a "living" statue created with random monster and its inventory

### 7.4 Hole Destination Calculation

```
hole_destination(dst):
    bottom = dng_bottom(u.uz)       // deepest reachable level
    dst.dnum = u.uz.dnum
    dst.dlevel = current_depth
    while dst.dlevel < bottom:
        dst.dlevel++
        if rn2(4):                  // 75% chance to stop
            break
```

In Quest: cannot fall past qlocate level if not yet reached.
In Gehennom: cannot reach sanctum until invocation performed.

## 8. Setting Traps

### 8.1 Trap Items

Hero can set traps using:
- **BEARTRAP** item: creates BEAR_TRAP
- **LAND_MINE** item: creates LANDMINE

(Applied via the 'a'pply command on these objects.)

### 8.2 Disarming Traps (#untrap)

**Disarmable types:**
- BEAR_TRAP: yields 1 BEARTRAP item
- WEB: destroyed (blade weapon helps; Sting/fire artifacts auto-succeed)
- LANDMINE: yields 1 LAND_MINE item
- SQKY_BOARD: requires oil or grease
- ARROW_TRAP: yields `50 - rnl(50)` arrows (1..50, luck-biased)
- DART_TRAP: yields `50 - rnl(50)` darts
- PIT/SPIKED_PIT: can help trapped monster out

**Not disarmable:** HOLE, TRAPDOOR, TELEP_TRAP, LEVEL_TELEP, MAGIC_PORTAL, FIRE_TRAP, SLP_GAS_TRAP, RUST_TRAP, MAGIC_TRAP, ANTI_MAGIC, POLY_TRAP, VIBRATING_SQUARE, STATUE_TRAP, ROCKTRAP, ROLLING_BOULDER_TRAP.

### 8.3 Untrap Probability (`untrap_prob`)

Base chance to fail = 3. Modifiers:

```
chance = 3
if WEB:
    if wielding blade (not Sting/fire artifact):
        chance = 3          // blade helps
    elif wielding Sting OR fire artifact blade:
        chance = 1          // guaranteed
    elif NOT webmaker:
        chance = 7
if Confusion OR Hallucination: chance++
if Blind: chance++
if Stunned: chance += 2
if Fumbling: chance *= 2
if trap.madeby_u: chance--
if Ranger AND BEAR_TRAP AND chance <= 3: return 0  // always succeed
if Rogue:
    if rn2(2 * MAXULEV) < u.ulevel: chance--
    if has_questart AND chance > 1: chance--
elif Ranger AND chance > 1: chance--
chance = max(chance, 1)
return rn2(chance)          // 0 = success
```

### 8.4 Box Trap Disarm

```
ch = ACURR(A_DEX) + u.ulevel
if Rogue: ch *= 2
if confused OR Fumbling OR rnd(75 + level_difficulty()/2) > ch:
    chest_trap triggers
else:
    "You disarm it!" -- +8 experience
```

### 8.5 Door Trap Detection

```
if (doormask & D_TRAPPED) AND (force OR (!confused AND rn2(MAXULEV - u.ulevel + 11) < 10)):
    find trap
```

Door trap disarm:
```
ch = 15 + (Rogue ? u.ulevel * 3 : u.ulevel)
if confused OR Fumbling OR rnd(75 + level_difficulty()/2) > ch:
    "You set it off!" -- door destroyed
else:
    "You disarm it!" -- +8 experience
```

## 9. Monster Interaction with Traps

### 9.1 Harmless Trap Check (`m_harmless_trap`)

Before applying effects, the system checks if a trap is harmless to a particular monster:

| Trap | Immune If |
|------|-----------|
| BEAR_TRAP | msize <= MZ_SMALL, amorphous, whirly, unsolid |
| SLP_GAS_TRAP | resists_sleep, defended(AD_SLEE) |
| RUST_TRAP | NOT Iron Golem (all others immune) |
| FIRE_TRAP | resists_fire, defended(AD_FIRE) |
| PIT/SPIKED_PIT/HOLE/TRAPDOOR | is_clinger (non-Sokoban) |
| WEB | amorphous, webmaker, whirly, unsolid |
| STATUE_TRAP | always harmless to monsters |
| MAGIC_TRAP | always harmless (usually) |
| ANTI_MAGIC | resists_magm, defended(AD_MAGM) |
| VIBRATING_SQUARE | always harmless |
| All floor-triggers | flying/levitating/clinging (non-Sokoban) |

### 9.2 Monster Trap Escape

Already-trapped monsters try to escape each turn in `mintrap()`:

```
if !rn2(40):        // 1/40 = 2.5% per turn
    // special: pit with boulder: additional !rn2(2) check
    if boulder_in_pit AND !rn2(2):
        pull free, fill_pit
    else:
        escape (climb out of pit / pull free of bear trap or web)

// Pit Fiend or MZ_HUGE+: m_easy_escape_pit => always passes the !rn2(40) equivalent
```

**Metallivorous monsters:** Can eat bear traps (trap deleted, `meating = 5`) or munch spiked pit spikes (converts to regular pit).

### 9.3 Monster Anger

If hero placed the trap (`madeby_u`) and a monster triggers it:
```
if rnl(5):      // luck-adjusted; roughly 80% at neutral luck
    setmangry(mon)
```

## 10. Hit Formulas

### 10.1 Hero Hit by Trap Missile (`thitu`)

```
hit = (u.uac + tlev <= rnd(20))
// if u.uac + tlev <= dieroll - 2: "misses"
// if u.uac + tlev == dieroll - 1: "almost hit"
// if u.uac + tlev >= dieroll: hit

// tlev values: arrow=8, dart=7, rolling_boulder=9+spe
```

### 10.2 Monster Hit by Trap Missile (`thitm`)

```
if d_override > 0:
    always hits, damage = d_override
elif obj:
    hit = (find_mac(mon) + tlev + obj.spe <= rnd(20))
    damage = dmgval(obj, mon)
else:
    hit = (find_mac(mon) + tlev <= rnd(20))
    damage = 1
```

## 11. Special Interactions

### 11.1 Sokoban Rules

In Sokoban (`level.flags.sokoban_rules`):
- Pits and holes **cannot be avoided** by levitation, flying, or clinging
- "Air currents pull you down"
- Falling through hole at stronghold entrance goes to Valley of the Dead
- Filling all pits/holes solves the puzzle (`maybe_finish_sokoban`)

### 11.2 Fixed-Destination Teleport Traps

`fixed_tele_trap(t)`: true if `ttyp == TELEP_TRAP` and `teledest.x, teledest.y` are valid coordinates. These traps are **always forced** (FORCETRAP), bypassing avoidance.

### 11.3 Steed Interaction

When riding, several traps affect the steed instead of (or in addition to) the hero:
- Arrow/Dart: 50% chance steed is hit
- Sleep gas: steed may fall asleep
- Landmine: steed takes rnd(16) damage
- Pit/Spiked Pit: steed takes rnd(6)/rnd(10) damage
- Poly trap: steed may polymorph
- Bear trap: steed takes d(2,4) damage, hero still trapped

### 11.4 Trap Interaction with Terrain

- Landmine explosion: can destroy drawbridges, break doors
- Fire trap on ice: melts ice
- Pit/hole on drawbridge: changes to floor
- Landmine creates PIT after explosion
- Boulder in pit: fills pit
- Ice melting under landmine/bear trap: trap falls into water as item

## 12. Suspected Bugs

1. **[疑似 bug]** `dofiretrap` comment at line 4147: "Bug: for box case, the equivalent of burn_floor_objects() ought to be done upon its contents." -- Fire trap triggered from a chest does not damage the chest's other contents.

2. **[疑似 bug]** `trapeffect_pit` at line 4894-4898: "FIXME: if hero gets killed here, setting u.utrap in advance will show 'you were trapped in a pit' during disclosure's display of enlightenment, but hero is dying *before* becoming trapped." -- `set_utrap` is called before `losehp`, so death message references trap state that wasn't reached.

3. **[疑似 bug]** The rock trap damage comment at line 1328 says `"should be std ROCK dmg?"` suggesting the d(2,6) value may not match the intended standard rock damage.

4. **[疑似 bug]** In `chest_trap`, the effect selection formula `rn2(20) ? ((Luck >= 13) ? 0 : rn2(13 - Luck)) : rn2(26)` means that at Luck=13, 95% of the time the effect index is 0 (the mildest: stun gas), but 5% of the time it's `rn2(26)` which can include the explosion (index 21-25). This seems intentional but the 5% bypass of max-luck protection is undocumented.

5. **[疑似 bug]** `float_up` at line 3870: checks `u.utraptype == WEB` but WEB's enum value is 18 (trap type), while `u.utraptype` should be `TT_WEB` which is 3. The code uses the literal `WEB` (=18) when it should use `TT_WEB` (=3). This means the web message in `float_up` is **never displayed** -- the `else` branch (bear trap message) fires instead.

## 13. 测试向量

### 13.1 Bear Trap Damage

| Input | Expected |
|-------|----------|
| d(2,4) with dice=(1,1) | dmg=2, `Maybe_Half_Phys(2)` = 2 (no half phys) or 1 (half phys) |
| d(2,4) with dice=(4,4) | dmg=8, `Maybe_Half_Phys(8)` = 8 or 4 |
| hero msize=MZ_SMALL, no steed | "closes harmlessly over you", dmg=0 |
| hero amorphous form | "closes harmlessly through you", dmg=0 |

### 13.2 Pit Trapping Duration

| Input | Expected `u.utrap` |
|-------|-------------------|
| rn1(6,2) min | 2 |
| rn1(6,2) max | 7 |

### 13.3 Web Duration by STR

| STR | Duration Range | Notes |
|-----|---------------|-------|
| 3   | 6..11 (rn1(6,6)) | minimum STR |
| 18  | 1 | standard 18 |
| 69+ | 0 (instant tear) | gauntlets + 18/** |

### 13.4 Arrow Trap Hit Check (Hero)

| u.uac | tlev | rnd(20) result | Hit? |
|-----:|-----:|-----:|:---:|
| 10   | 8    | 18  | yes (10+8=18 <= 18) |
| 10   | 8    | 17  | no  (18 > 17) |
| -5   | 8    | 3   | yes (3 <= 3) |
| -5   | 8    | 2   | no  (3 > 2) |

### 13.5 Trap Depletion (Arrow/Dart/Rock)

| `trap.once` | `trap.tseen` | rn2(15) | Result |
|:-----------:|:------------:|:-------:|--------|
| false | any | any | Normal fire (sets once=true) |
| true  | false | any | Normal fire |
| true  | true  | 0   | Trap deleted, no projectile |
| true  | true  | 1..14 | Normal fire |

### 13.6 Sleeping Gas Duration

| Input | Expected |
|-------|----------|
| rnd(25) = 1 | fall_asleep(-1) => 1 turn |
| rnd(25) = 25 | fall_asleep(-25) => 25 turns |
| Sleep_resistance | no sleep, gas cloud message only |
| breathless form | no sleep, gas cloud message only |

### 13.7 Landmine Wounded Legs Duration

| Input | Expected |
|-------|----------|
| rn1(35,41) min | 41 turns (both legs) |
| rn1(35,41) max | 75 turns (both legs) |

### 13.8 Boundary: Spiked Pit Poison

| rn2(6) result | Poisoned? |
|:---:|:---:|
| 0 | yes (1/6 chance) |
| 1..5 | no |

### 13.9 Magic Trap Explosion

| rn2(30) result | Effect |
|:---:|---------|
| 0 | Magical explosion: rnd(10) damage, +2 max energy, trap deleted |
| 1..29 | domagictrap() random effects |

### 13.10 Anti-Magic Energy Drain

| d(2,6) roll | halfd | uenmax change | drain_en amount |
|:-----------:|:-----:|:-------------:|:---------------:|
| 2 (min)     | rnd(1)=1 | -1 (if uenmax > 2) | 1 |
| 12 (max)    | rnd(6)=1..6 | -(1..6) (if uenmax > 12) | 12-(1..6)=6..11 |

### 13.11 Boundary: Monster Landmine Weight Check

| mon.cwt | MINE_TRIGGER_WT (WT_ELF/2 = 400) | rn2(cwt+1) < trigger_wt? |
|:-------:|:--------------------------------:|:------------------------:|
| 0 (tiny) | 400 | rn2(1)=0 < 400 => always triggers |
| 200 (heavy) | 400 | rn2(201) range [0,200]: ALL values < 400 => always triggers |
| 800 (WT_ELF) | 400 | rn2(801) range [0,800]: ~50% chance value >= 400 => ~50% skips |

### 13.12 Boundary: Hero Avoidance of Seen Trap

| Seen? | Fumbling? | rn2(5) | Clinger at pit? | Result |
|:-----:|:---------:|:------:|:---------------:|--------|
| yes | no | 0 | no | escape |
| yes | no | 1..4 | no | trigger |
| yes | no | any | yes (pit) | escape |
| yes | yes | any | any | trigger (Fumbling blocks) |
| no  | no | any | any | trigger (unseen) |
