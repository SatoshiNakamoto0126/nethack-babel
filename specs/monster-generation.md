# Monster Generation Mechanism Spec

> Source: `src/makemon.c`, `src/mkroom.c`, `src/questpgr.c`, `src/dungeon.c`,
> `src/allmain.c`, `include/permonst.h`, `include/monst.h`, `include/monflag.h`,
> `include/global.h`

---

## 1. Dungeon Level Difficulty Calculation

The function `level_difficulty()` returns an effective difficulty rating used
throughout monster generation.

```
fn level_difficulty() -> i16:
    if In_endgame:
        return depth(sanctum_level) + u.ulevel / 2
    if u.uhave.amulet:
        return deepest_lev_reached(include_quest=true)
    res = depth(u.uz)       // floors below surface
    if builds_up(u.uz):     // Sokoban, Vlad's Tower
        res += 2 * (dungeons[u.uz.dnum].entry_lev - u.uz.dlevel + 1)
    return res
```

`depth(lev)` = `dungeons[lev.dnum].depth_start + lev.dlevel - 1`.

The Sokoban adjustment means going *up* inside a builds-up branch increases
difficulty by 2 per level above entry.

---

## 2. Monster Difficulty Window

From the level difficulty, two bounds filter which monsters may be randomly
generated:

```
monmin_difficulty(levdif) = levdif / 6
monmax_difficulty(levdif) = (levdif + u.ulevel) / 2
```

A monster with `mons[mndx].difficulty` outside `[minmlev, maxmlev]` is excluded
from the random selection pool.  `rndmonst_adj(minadj, maxadj)` adds offsets:

```
minmlev = monmin_difficulty(zlevel) + minadj
maxmlev = monmax_difficulty(zlevel) + maxadj
```

Standard `rndmonst()` calls `rndmonst_adj(0, 0)`.

---

## 3. Monster Selection Algorithm (`rndmonst`)

### 3.1 Quest Override

On the quest dungeon branch, with probability 6/7, `qt_montype()` is called
instead of the normal algorithm:

```
fn qt_montype() -> Option<permonst>:
    if rn2(5) != 0:             // 80% chance: enemy1
        if urole.enemy1num != NON_PM && rn2(5) != 0 && not genocided:
            return mons[urole.enemy1num]
        return mkclass(urole.enemy1sym, 0)
    else:                       // 20% chance: enemy2
        if urole.enemy2num != NON_PM && rn2(5) != 0 && not genocided:
            return mons[urole.enemy2num]
        return mkclass(urole.enemy2sym, 0)
```

### 3.2 Core Random Selection (Weighted Reservoir Sampling)

For each monster index `mndx` from `LOW_PM` (0) to `SPECIAL_PM` (exclusive):

**Exclusion filters** (all must pass):
1. `difficulty < minmlev` → skip (too weak)
2. `difficulty > maxmlev` → skip (too strong)
3. Rogue level: skip if symbol is not uppercase
4. Elemental plane: skip if `wrong_elem_type(ptr)` (wrong element)
5. `uncommon(mndx)` → skip:
   - has `G_NOGEN | G_UNIQ` flags → uncommon
   - is genocided or extinct (`G_GONE`) → uncommon
   - In Gehennom: positive alignment (`maligntyp > 0`) → uncommon
   - Not in Gehennom: has `G_HELL` flag → uncommon
6. In Gehennom and has `G_NOHELL` → skip

**Weight calculation** for passing monsters:

```
weight = (ptr.geno & G_FREQ)       // 0..7, the frequency field
       + align_shift(ptr)          // 0..5
       + temperature_shift(ptr)    // 0 or 3
```

Clamped to `[0, 127]`.

**Reservoir sampling**: for each candidate with `weight > 0`:

```
totalweight += weight
if rn2(totalweight) < weight:
    selected_mndx = mndx
```

This is an unbiased single-pass weighted random selection.

### 3.3 Align Shift

```
fn align_shift(ptr) -> int:
    dungeon_align = special_level_align or dungeons[dnum].flags.align
    match dungeon_align:
        AM_NONE    => 0
        AM_LAWFUL  => (ptr.maligntyp + 20) / (2 * ALIGNWEIGHT)
        AM_NEUTRAL => (20 - abs(ptr.maligntyp)) / ALIGNWEIGHT
        AM_CHAOTIC => (-(ptr.maligntyp - 20)) / (2 * ALIGNWEIGHT)
```

where `ALIGNWEIGHT = 4`.

For `AM_LAWFUL`: a monster with `maligntyp = +20` gets `(20+20)/8 = 5`;
a monster with `maligntyp = -20` gets `(-20+20)/8 = 0`.

For `AM_NEUTRAL`: a monster with `maligntyp = 0` gets `20/4 = 5`;
a monster with `maligntyp = ±20` gets `0/4 = 0`.

### 3.4 Temperature Shift

```
fn temperature_shift(ptr) -> int:
    if level.flags.temperature != 0:
        if temperature > 0 and ptr resists fire:  return 3
        if temperature < 0 and ptr resists cold:  return 3
    return 0
```

---

## 4. Generation Frequency and Flags

The `permonst.geno` field (unsigned short) encodes:

| Bits    | Mask       | Meaning                                     |
|---------|------------|---------------------------------------------|
| 0..2    | `G_FREQ`=0x0007 | Creation frequency (0..7)              |
| 4       | `G_NOCORPSE`=0x0010 | Never leaves a corpse               |
| 5       | `G_GENO`=0x0020 | Can be genocided                       |
| 6       | `G_LGROUP`=0x0040 | Appears in large groups               |
| 7       | `G_SGROUP`=0x0080 | Appears in small groups               |
| 9       | `G_NOGEN`=0x0200 | Never generated randomly               |
| 10      | `G_HELL`=0x0400 | Generated only in Gehennom              |
| 11      | `G_NOHELL`=0x0800 | Not generated in Gehennom              |
| 12      | `G_UNIQ`=0x1000 | Unique — generated only once            |

Runtime flags in `mvitals[mndx].mvflags`:

| Bits | Mask         | Meaning                           |
|------|--------------|-----------------------------------|
| 0    | `G_EXTINCT`=0x01 | Population limit reached       |
| 1    | `G_GENOD`=0x02 | Genocided by player              |
| 2    | `G_KNOWN`=0x04 | Has been encountered             |
| 3    | `MV_KNOWS_EGG`=0x08 | Player recognizes its egg  |

`G_GONE` = `G_GENOD | G_EXTINCT` (0x03).

---

## 5. Group/Swarm Generation

When `makemon()` is called with a random monster (no specific type requested)
and `MM_NOGRP` is *not* set:

```
if ptr.geno & G_SGROUP and rn2(2):      // 50% chance
    m_initsgrp(mtmp, x, y, mmflags)     // small group
elif ptr.geno & G_LGROUP:
    if rn2(3):                           // 67% chance
        m_initlgrp(mtmp, x, y, mmflags) // large group
    else:                                // 33% chance
        m_initsgrp(mtmp, x, y, mmflags) // small group instead
```

### Group sizes

```
m_initsgrp → m_initgrp(mtmp, x, y, n=3, mmflags)
m_initlgrp → m_initgrp(mtmp, x, y, n=10, mmflags)
```

Inside `m_initgrp`:

```
cnt = rnd(n)                             // 1..n
cnt /= (u.ulevel < 3) ? 4 : (u.ulevel < 5) ? 2 : 1
if cnt == 0: cnt = 1                     // at least 1
```

So effective group sizes after hero level adjustment:

| Group type | n  | Hero lvl < 3 | Hero lvl 3..4 | Hero lvl >= 5 |
|------------|----|---------------|---------------|---------------|
| Small      | 3  | 1 (always)    | 1 (always)    | 1..3          |
| Large      | 10 | 1..2          | 1..5          | 1..10         |

Group members:
- Only **hostile** members are placed — `peace_minded()` check skips peaceful ones
- Each additional monster is created with `MM_NOGRP` to prevent recursive groups
- All group members are forced hostile (`mpeaceful = FALSE`)

---

## 6. Monster Level Adjustment (`adj_lev`)

After selecting a monster type, its actual `m_lev` is adjusted:

```
fn adj_lev(ptr) -> int:
    if ptr == Wizard_of_Yendor:
        return min(ptr.mlevel + times_wizard_died, 49)

    if ptr.mlevel > 49:
        return 50               // "special" demon lords with encoded HP

    tmp = ptr.mlevel
    diff = level_difficulty() - tmp
    if diff < 0:
        tmp -= 1                // monster stronger than level: decrement
    else:
        tmp += diff / 5         // weaker: increment 1 per 5 diff

    player_diff = u.ulevel - ptr.mlevel
    if player_diff > 0:
        tmp += player_diff / 4  // player stronger: boost monster

    upper = min(3 * ptr.mlevel / 2, 49)   // crude upper limit
    return clamp(tmp, 0, upper)
```

---

## 7. Hit Point Calculation (`newmonhp`)

After `m_lev` is set via `adj_lev`:

```
fn newmonhp(mon, mndx):
    mon.m_lev = adj_lev(ptr)

    if is_golem(ptr):
        hp = golemhp(mndx)          // fixed: 20..120 by type

    elif is_rider(ptr):
        hp = d(10, 8)               // 10d8 (10..80)

    elif ptr.mlevel > 49:
        hp = 2 * (ptr.mlevel - 6)   // encoded HP
        mon.m_lev = hp / 4

    elif S_DRAGON and mndx >= PM_GRAY_DRAGON:  // adult dragons
        N = mon.m_lev
        if In_endgame:
            hp = 8 * N
        else:
            hp = 4 * N + d(N, 4)

    elif mon.m_lev == 0:
        hp = rnd(4)                  // 1d4

    else:
        hp = d(mon.m_lev, 8)        // Nd8
        if is_home_elemental(ptr):
            hp *= 3

    // minimum HP boost: if hp == base (all 1s rolled), add 1
    if hp == basehp:
        hp += 1
    mon.mhp = mon.mhpmax = hp
```

### Golem HP Table

| Golem type   | HP  |
|-------------|-----|
| Straw       | 20  |
| Paper       | 20  |
| Rope        | 30  |
| Leather     | 40  |
| Flesh       | 40  |
| Wood        | 50  |
| Gold        | 60  |
| Clay        | 70  |
| Glass       | 80  |
| Stone       | 100 |
| Iron        | 120 |

---

## 8. Genocide and Extinction Checks

### 8.1 Genocide (`G_GENOD`)

If a specific monster type is requested and `mvitals[mndx].mvflags & G_GENOD`,
`makemon()` returns NULL immediately.  Genocided monsters are always excluded
from random selection via `uncommon()`.

### 8.2 Extinction

The `propagate()` function is called for every successful `makemon()`:

```
fn propagate(mndx, tally, ghostly) -> bool:
    lim = mbirth_limit(mndx)
    gone = mvitals[mndx].mvflags & G_GONE

    result = (mvitals[mndx].born < lim) && !gone

    // unique monsters: mark extinct after first creation
    if (mons[mndx].geno & G_UNIQ) && mndx != PM_HIGH_CLERIC:
        mvitals[mndx].mvflags |= G_EXTINCT

    // increment birth counter (cap at 255)
    if mvitals[mndx].born < 255 && tally && (!ghostly || result):
        mvitals[mndx].born += 1

    // auto-extinction when birth limit reached
    if mvitals[mndx].born >= lim && !(geno & G_NOGEN) && !already_extinct:
        mvitals[mndx].mvflags |= G_EXTINCT

    return result
```

### Birth Limits

```
fn mbirth_limit(mndx) -> int:
    PM_NAZGUL  => 9
    PM_ERINYS  => 3
    _          => MAXMONNO (120)
```

`MAXMONNO = 120` — after 120 of the same species are *created* (not killed),
the species goes extinct.

---

## 9. Spontaneous Monster Generation Timing

In the main game loop (`allmain.c`), once per game turn (when the clock advances):

```
if !rn2(spawn_rate):
    makemon(NULL, 0, 0, NO_MM_FLAGS)   // random type, random location
```

Where `spawn_rate` depends on game state:

| Condition                                              | Rate  | Expected turns between spawns |
|--------------------------------------------------------|-------|-------------------------------|
| Wizard of Yendor killed (`udemigod`)                   | 1/25  | 25                            |
| Deeper than Castle (`depth > depth(stronghold_level)`) | 1/50  | 50                            |
| Otherwise (normal dungeon)                             | 1/70  | 70                            |

The new monster's location is chosen by `makemon_rnd_goodpos()`:
- 50 random attempts, rejecting positions visible to the hero (unless during
  level creation or hero is blind)
- Fallback: systematic scan of all positions, first round skipping visible ones,
  second round including them
- Near a stairway as last resort

**Suppression**: spontaneous generation is disabled when
`iflags.debug_mongen` is true or `level.flags.rndmongen` is false.

---

## 10. Class-Specific Room Generation

Special rooms stock specific monster types at level creation time:

### 10.1 Throne Room (`COURT`)

```
fn courtmon() -> permonst:
    i = rn2(60) + rn2(3 * level_difficulty())
    if i > 100: mkclass(S_DRAGON)
    elif i > 95: mkclass(S_GIANT)
    elif i > 85: mkclass(S_TROLL)
    elif i > 75: mkclass(S_CENTAUR)
    elif i > 60: mkclass(S_ORC)
    elif i > 45: PM_BUGBEAR
    elif i > 30: PM_HOBGOBLIN
    elif i > 15: mkclass(S_GNOME)
    else:        mkclass(S_KOBOLD)
```

A throne is placed; a throne-room ruler is selected by difficulty:

```
ruler_roll = rnd(level_difficulty())
if roll > 9:  PM_OGRE_TYRANT
elif roll > 5: PM_ELVEN_MONARCH
elif roll > 2: PM_DWARF_RULER
else:          PM_GNOME_RULER
```

### 10.2 Barracks (`BARRACKS`)

```
fn squadmon() -> permonst:
    sel_prob = rnd(80 + level_difficulty())
    cumulative over:
        PM_SOLDIER:    80
        PM_SERGEANT:   15
        PM_LIEUTENANT:  4
        PM_CAPTAIN:     1
    // total = 100; higher sel_prob rolls past 100 → fallback to last
```

At higher level_difficulty, the roll has more headroom to exceed cumulative
thresholds, producing more officers.

### 10.3 Graveyard / Morgue (`MORGUE`)

```
fn morguemon() -> permonst:
    i = rn2(100)
    hd = rn2(level_difficulty())

    if hd > 10 && i < 10:
        if Inhell or endgame: mkclass(S_DEMON)
        else: ndemon(A_NONE)    // named demon, any alignment
    if hd > 8 && i > 85:
        mkclass(S_VAMPIRE)
    if i < 20: PM_GHOST
    elif i < 40: PM_WRAITH
    else: mkclass(S_ZOMBIE)
```

### 10.4 Beehive (`BEEHIVE`)

- Center tile: `PM_QUEEN_BEE`
- All other tiles: `PM_KILLER_BEE`
- 1/3 chance of `LUMP_OF_ROYAL_JELLY` per tile

### 10.5 Ant Hole (`ANTHOLE`)

```
fn antholemon() -> permonst:
    indx = (ubirthday % 3) + level_difficulty()
    match (indx + trycnt) % 3:
        0 => PM_SOLDIER_ANT
        1 => PM_FIRE_ANT
        _ => PM_GIANT_ANT
```

Same ant species throughout a level; varies between levels.  Retries up to 3
times if chosen species is genocided/extinct.

### 10.6 Cockatrice Nest (`COCKNEST`)

All monsters: `PM_COCKATRICE`. 1/3 chance of a statue with random items per tile.

### 10.7 Leprechaun Hall (`LEPREHALL`)

All monsters: `PM_LEPRECHAUN`. Gold piles placed proportional to distance from
door.

### 10.8 Zoo (`ZOO`)

Random monsters (`makemon(NULL, ...)`). Gold piles placed.

### 10.9 Swamp (`SWAMP`)

Random monsters. Up to one electric eel per tile in water.

---

## 11. mkclass — Class-Constrained Selection

`mkclass(class, spc)` selects a monster of a given symbol class:

```
fn mkclass(class, spc) -> Option<permonst>:
    maxmlev = level_difficulty() / 2     // note: halved, not same as rndmonst

    // iterate monsters of this class (sorted by difficulty)
    for each candidate in class (ascending difficulty):
        // filter: not genocided/extinct (unless G_IGNORE passed)
        // filter: not G_NOGEN, not G_UNIQ (unless spc masks them)
        // Gehennom/hell filter (8/9 of the time): skip G_NOHELL in Gehennom,
        //                                         skip G_HELL outside it
        //   Exception: S_LICH always respects hell restrictions

        if num > 0 && too_strong && stronger_than_previous && rn2(2):
            break                        // stop adding stronger candidates

        k = geno & G_FREQ               // 0..7
        if k == 0 and entire class has freq 0: k = 1
        // skew: subtract 1 if adj_lev(mon) > 2 * u.ulevel
        nums[mndx] = k + 1 - (adj_lev(mon) > u.ulevel * 2 ? 1 : 0)
        num += nums[mndx]

    // weighted random pick from accumulated candidates
    pick = rnd(num)
    iterate: subtract nums[mndx] until pick <= 0
    return selected monster
```

Key differences from `rndmonst()`:
- Uses `level_difficulty() / 2` as the strength cap (halved)
- Iterates only within the requested class
- Has a 50% chance to stop adding candidates once one exceeds the difficulty cap
- Skews toward weaker monsters when hero level is low

---

## 12. Starting Inventory for Generated Monsters

### 12.1 Weapons (`m_initweap`)

Called only if `is_armed(ptr)` (monster has `AT_WEAP` attack).
Not called on the Rogue level.

**By monster class:**

| Class         | Weapons                                                |
|---------------|--------------------------------------------------------|
| `S_GIANT`     | 50%: boulder (or club if ettin); non-ettin 20%: two-handed sword or battle-axe |
| `S_HUMAN` (mercenary) | Varies by rank — see below                   |
| `S_HUMAN` (elf) | Elven gear loadout with 3 variants               |
| `S_HUMAN` (priest) | Mace (+1..+3, 50% cursed)                      |
| `S_ANGEL`     | Blessed erodeproof long sword or silver mace (+0..+6); shield of reflection or large shield |
| `S_KOBOLD`    | 25%: darts ×(3..14)                                   |
| `S_CENTAUR`   | 50%: bow+arrows or crossbow+bolts                     |
| `S_ORC`       | Orcish helm + weapon varies by sub-type                |
| `S_OGRE`      | Battle-axe or club (better chance for leaders)         |
| `S_TROLL`     | 50%: random polearm                                    |
| `S_WRAITH`    | Knife + long sword                                     |
| `S_ZOMBIE`    | 25%: leather armor; 25%: knife or short sword          |
| `S_KOP`       | 25%: cream pies; 33%: club or rubber hose              |
| `S_DEMON`     | Specific weapons per named demon                       |

**Mercenary weapons by rank:**

| Rank          | Primary weapon                          | Secondary |
|---------------|-----------------------------------------|-----------|
| Soldier/Watchman | 33%: random polearm; else spear or short sword | Dagger/Knife |
| Sergeant      | Flail or mace                           | —         |
| Lieutenant    | Broadsword or long sword                | —         |
| Captain/Watch Captain | Long sword or silver saber       | —         |

**General default** (demons who fall through, other armed monsters):

```
bias = is_lord + 2*is_prince + extra_nasty   // 0..4
roll = rnd(14 - 2*bias)                      // higher bias → better odds
match roll:
    1 => strong: battle-axe; weak: darts
    2 => strong: two-handed sword; weak: crossbow+bolts
    3 => bow + arrows
    4 => strong: long sword; weak: daggers
    5 => strong: lucern hammer; weak: aklys
    _ => nothing
```

**Offensive item bonus**: if `m_lev > rn2(75)`, gets a random offensive item.

### 12.2 Other Inventory (`m_initinv`)

Not called on the Rogue level.

| Monster / Class    | Inventory                                           |
|--------------------|-----------------------------------------------------|
| Mercenaries        | Armor scaled to target AC; rations for soldiers     |
| Watchmen           | 67%: tin whistle                                    |
| Guard              | Cursed tin whistle                                  |
| Shopkeeper         | Skeleton key + wands/potions (cascading)            |
| Priest             | Robe (or magic cloak) + small shield + 20..29 gold  |
| Nymph              | 50%: mirror; 50%: potion of object detection        |
| Giant (non-minotaur) | rn2(m_lev/2) gems                                |
| Minotaur           | 33%: wand of digging                                |
| Nazgul             | Cursed ring of invisibility                         |
| Lich (master)      | 1/13: athame or wand of nothing                     |
| Lich (arch)        | 1/3: athame or quarterstaff                         |
| Mummy              | 6/7: mummy wrapping                                 |
| Quantum Mechanic   | 1/20: large box with Schroedinger's cat corpse      |
| Leprechaun         | d(level_difficulty(), 30) gold                      |
| Ice Devil          | 25%: spear                                          |
| Asmodeus           | Wand of cold + wand of fire                         |
| Gnome              | Small chance of candle (higher in Mines)             |

**General bonuses** (after class-specific; soldiers mostly skip these):

```
if m_lev > rn2(50):  random defensive item
if m_lev > rn2(100): random misc item
if likes_gold && no gold && rn2(5) == 0:
    d(level_difficulty(), has_inventory ? 5 : 10) gold
```

**Saddle**: 1/100 chance for domestic monsters that can be saddled.

### 12.3 Special Named Monster Items

| Monster           | Item                         |
|-------------------|------------------------------|
| Vlad the Impaler  | Candelabrum of Invocation    |
| Wizard of Yendor  | Spellbook of Dig (first time, Earth level) |
| Croesus           | Two-handed sword             |
| Quest Nemesis     | Bell of Opening              |
| Pestilence        | Potion of Sickness           |

---

## 13. Peaceful Generation Rules

`peace_minded(ptr)` determines initial disposition:

```
fn peace_minded(ptr) -> bool:
    if always_peaceful(ptr):  return true    // M2_PEACEFUL
    if always_hostile(ptr):   return false   // M2_HOSTILE
    if ptr.msound == MS_LEADER or MS_GUARDIAN: return true
    if ptr.msound == MS_NEMESIS: return false
    if ptr == PM_ERINYS: return !u.ualign.abuse  // peaceful if no alignment abuse
    if race_peaceful(ptr): return true       // matches urace.lovemask
    if race_hostile(ptr):  return false      // matches urace.hatemask

    // alignment check
    if sgn(ptr.maligntyp) != sgn(u.ualign.type): return false
    if ptr.maligntyp < 0 && u.uhave.amulet: return false
    if is_minion(ptr): return u.ualign.record >= 0

    // co-aligned: small chance of hostility
    return rn2(16 + clamp(u.ualign.record, -15, ...)) != 0
           && rn2(2 + abs(ptr.maligntyp)) != 0
```

### Special Overrides in `makemon()`

After `peace_minded()` is called, `makemon()` applies overrides:

| Condition                      | Result           |
|--------------------------------|------------------|
| Orcs when hero is Elf          | Forced hostile   |
| Co-aligned unicorn             | Forced peaceful  |
| Demon prince with `MS_BRIBE`   | Forced peaceful + invisible (unless hero wields Excalibur or Demonbane) |
| Raven when hero wields bec-de-corbin | Forced peaceful |
| `MM_ANGRY` flag passed         | Forced hostile   |

---

## 14. Unique Monster Generation

Monsters with `G_UNIQ` set:
- Never appear in random selection pool (`uncommon()` rejects them)
- Created only by explicit `makemon(&mons[PM_FOO], ...)` calls
- After creation, `propagate()` immediately sets `G_EXTINCT`
- Exception: `PM_HIGH_CLERIC` is flagged `G_UNIQ` but *not* auto-extincted
  (aligned priests can grow into high priests)

If a unique monster is requested but already genocided, `makemon()` returns NULL.
If extinct (but not genocided), creation proceeds in wizard mode with a debug
message.

---

## 15. Elemental Level Restrictions

On elemental planes (endgame, non-astral):

```
fn wrong_elem_type(ptr) -> bool:
    if ptr is elemental: must match plane (air/fire/earth/water)
    elif Water level: must be swimmer
    elif Fire level: must resist fire
    elif Air level: must be flyer/floater/amorphous/noncorporeal/whirly
    // Earth level: no restrictions
```

---

## 16. Shapeshifter Handling at Creation

If `Protection_from_shape_changers` is not active and the monster is a
shapeshifter (`pm_to_cham(mndx) != NON_PM`):

- `mtmp.cham` is set to the base form
- `newcham()` is called to pick a random disguise form
- Exception: Vlad the Impaler stays in true form (to carry the Candelabrum)
- If shapeshifted successfully, initial inventory generation is skipped

---

## 17. Monster Placement Rules

`makemon(ptr, x, y, mmflags)` position handling:

- `(0, 0)` → random location via `makemon_rnd_goodpos()`
- `(u.ux, u.uy)` → near player via `enexto_core()`
- Otherwise → use exact position; if occupied and `MM_ADJACENTOK`, find adjacent

`makemon_rnd_goodpos()`:
1. Try 50 random positions; reject if hero can see it (unless creating level or blind)
2. Fallback: scan all positions, two passes (first: not in sight; second: any)
3. Between passes: try stairway positions

---

## 18. Post-Creation Setup

After monster is placed:

- Mimics: set mimic appearance (`set_mimic_sym`)
- Spiders/snakes: during level creation, place object underneath and hide
- Eels: hide during level creation
- Stalkers/black lights: permanent invisibility
- Leprechauns: always sleeping
- Jabberwocks/nymphs: 80% sleeping if hero has no Amulet
- Bats in Gehennom: double speed
- Light-emitting monsters: create light source

---

## 19. Sokoban Special Rule

When placing random monsters in Sokoban, the first attempt rejects
boulder-throwing monsters (`throws_rocks`). After the first try (up to 50
total), they become acceptable.

---

## 20. `rndmongen` and `debug_mongen` Guards

Two flags suppress random monster generation entirely:

- `level.flags.rndmongen == false`: per-level flag (set by some special levels)
- `iflags.debug_mongen == true`: wizard mode debug option

If either is active and `ptr == NULL` (random selection), `makemon()` returns
NULL immediately.

---

## 疑似 Bug

### [疑似 bug] align_shift 可返回负值但未被裁剪

`align_shift()` 对 `AM_LAWFUL` 分支计算 `(ptr.maligntyp + 20) / (2*4)`。
当 `maligntyp = -20` 时结果为 0，正常。但 C 的整数除法对负数是 toward-zero 的，
若 `maligntyp < -20`（理论上 `maligntyp` 是 `aligntyp`/`schar`，范围 -128..127），
结果可能为负。虽然 `rndmonst` 中 `weight` 有 `< 0` 的 impossible 检查，
但负的 `align_shift` 加上 `G_FREQ`(1..7) 仍可能产生正权重，
使得该怪物被选中的概率低于预期。实际游戏中 `maligntyp` 不会低于 -20，
所以这是理论性的。

### [疑似 bug] mkclass 中 difficulty 截断用 level_difficulty()/2 而非 rndmonst 的 (level_difficulty()+ulevel)/2

`mkclass()` 使用 `maxmlev = level_difficulty() >> 1`，
而 `rndmonst()` 使用 `(level_difficulty() + u.ulevel) / 2`。
这意味着通过 `mkclass()` 生成（如坟场的 `morguemon()` → `mkclass(S_ZOMBIE)`）
产生的怪物比直接 `rndmonst()` 产生的偏弱。这可能是有意设计（
特殊房间本身已有挑战），但两处不一致容易引起困惑。

### [疑似 bug] m_initgrp 中 peace_minded 双重检查

`m_initgrp()` 对每个群体成员调用 `peace_minded()` 来跳过和平怪物。
但 `makemon()` 本身也会调用 `peace_minded()` 来设置 `mpeaceful`。
代码注释承认了这一点（"Undo the second peace_minded() check"），
但实际上在第二次检查时并没有"撤销"——它只是强制设为 hostile。
如果 `peace_minded()` 有随机成分（它有），则群体大小取决于
两次独立调用的结果，可能导致群体比预期更小。

---

## 测试向量

### T1: level_difficulty 基本计算

```
输入: depth(u.uz) = 10, not endgame, no amulet, not builds_up
预期: level_difficulty() = 10
```

### T2: level_difficulty builds_up 分支 (Sokoban)

```
输入: depth(u.uz) = 7, builds_up, entry_lev = 8, dlevel = 7
预期: level_difficulty() = 7 + 2*(8 - 7 + 1) = 7 + 4 = 11
```

### T3: level_difficulty endgame

```
输入: In_endgame, depth(sanctum_level) = 50, u.ulevel = 14
预期: level_difficulty() = 50 + 14/2 = 57
```

### T4: monmin/monmax difficulty 窗口

```
输入: zlevel = 12, u.ulevel = 8
预期: minmlev = 12/6 = 2, maxmlev = (12+8)/2 = 10
```

### T5: 边界条件 — zlevel = 1, ulevel = 1

```
输入: zlevel = 1, u.ulevel = 1
预期: minmlev = 0, maxmlev = (1+1)/2 = 1
// 只有 difficulty 0 或 1 的怪物可被随机生成
```

### T6: 边界条件 — zlevel = 0 (surface, 理论值)

```
输入: zlevel = 0, u.ulevel = 1
预期: minmlev = 0, maxmlev = (0+1)/2 = 0
// 只有 difficulty 0 的怪物可被随机生成
```

### T7: adj_lev 普通怪物

```
输入: ptr.mlevel = 5, level_difficulty() = 15, u.ulevel = 10
计算:
    diff = 15 - 5 = 10 → tmp = 5 + 10/5 = 7
    player_diff = 10 - 5 = 5 → tmp = 7 + 5/4 = 8
    upper = min(3*5/2, 49) = 7
预期: adj_lev = min(8, 7) = 7
```

### T8: adj_lev Wizard of Yendor (died 3 times)

```
输入: ptr = PM_WIZARD_OF_YENDOR, mlevel = 30, times_died = 3
预期: adj_lev = min(30 + 3, 49) = 33
```

### T9: group size — small group, hero level 1

```
输入: n = 3, u.ulevel = 1 (< 3)
计算: cnt = rnd(3) → 1..3; cnt /= 4 → always 0; cnt = max(0,1) = 1
预期: 1 additional monster (always)
```

### T10: group size — large group, hero level 10

```
输入: n = 10, u.ulevel = 10 (>= 5)
计算: cnt = rnd(10) → 1..10; cnt /= 1 = 1..10
预期: 1..10 additional monsters
```

### T11: propagate — unique monster

```
输入: mndx = PM_MEDUSA, geno has G_UNIQ, born = 0
预期:
    result = true (born < 120 and not gone)
    G_EXTINCT set on mvitals (unique)
    born incremented to 1
```

### T12: propagate — extinction at limit

```
输入: mndx = PM_KILLER_BEE, born = 119, lim = 120
预期:
    result = true (119 < 120)
    born incremented to 120
    120 >= 120 → G_EXTINCT set automatically
```

### T13: propagate — Nazgul birth limit

```
输入: mndx = PM_NAZGUL, born = 8, lim = mbirth_limit(PM_NAZGUL) = 9
预期:
    result = true (8 < 9)
    born = 9
    9 >= 9 → G_EXTINCT set (but Nazgul is also G_UNIQ, so already extinct)
```

### T14: 边界条件 — align_shift AM_LAWFUL with maligntyp = 0

```
输入: dungeon_align = AM_LAWFUL, ptr.maligntyp = 0
预期: (0 + 20) / 8 = 2
```

### T15: spontaneous generation rate — post-Wizard

```
输入: u.uevent.udemigod = true
预期: spawn_rate = 25 (1/25 chance per turn)
```

### T16: courtmon — low difficulty level

```
输入: level_difficulty() = 5
计算: i = rn2(60) + rn2(15) → range [0, 74]
预期: never exceeds 74, so never dragons or giants
      most likely kobolds, gnomes, hobgoblins, bugbears
```

### T17: courtmon — high difficulty level

```
输入: level_difficulty() = 30
计算: i = rn2(60) + rn2(90) → range [0, 149]
预期: can produce dragons (i > 100 → ~30% of range)
```
