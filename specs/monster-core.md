# Monster Core -- Mechanism Spec

> Source: `src/mon.c`, `src/makemon.c`, `src/exper.c`, `src/monmove.c`,
> `src/were.c`, `src/dog.c`, `include/permonst.h`, `include/monst.h`,
> `include/monflag.h`, `include/mondata.h`

---

## 1. Monster Data Structures

### 1.1 Species Template (`struct permonst`)

Every monster species has exactly one entry in the global `mons[]` array.
Fields (all read-only during play):

| Field | Type | Meaning |
|-------|------|---------|
| `pmnames[NUM_MGENDERS]` | `const char*[3]` | Display names (male/female/neutral) |
| `pmidx` | `enum monnums` | Index into `mons[]` (also serves as `monsndx()`) |
| `mlet` | `char` | Monster symbol class (S_ANT, S_DOG, ...) |
| `mlevel` | `schar` | Base monster level (0..127) |
| `mmove` | `schar` | Base movement speed |
| `ac` | `schar` | Base armor class |
| `mr` | `schar` | Base magic resistance (0..100) |
| `maligntyp` | `aligntyp` | Monster alignment (-128..127) |
| `geno` | `unsigned short` | Generation/genocide flags (G_UNIQ, G_FREQ, ...) |
| `mattk[6]` | `struct attack` | Up to 6 attacks: `{aatyp, adtyp, damn, damd}` |
| `cwt` | `unsigned short` | Corpse weight |
| `cnutrit` | `unsigned short` | Nutritional value of corpse |
| `msound` | `uchar` | Sound type (MS_SILENT..MS_GROAN) |
| `msize` | `uchar` | Physical size (MZ_TINY..MZ_GIGANTIC) |
| `mresists` | `uchar` | Innate resistance bitmask (MR_FIRE..MR_STONE) |
| `mconveys` | `uchar` | Resistance conveyed by eating (same bitmask) |
| `mflags1` | `unsigned long` | M1_* flags |
| `mflags2` | `unsigned long` | M2_* flags |
| `mflags3` | `unsigned short` | M3_* flags |
| `difficulty` | `uchar` | Pre-computed toughness rating |
| `mcolor` | `uchar` | Display color |

`NORMAL_SPEED` = 12. `NATTK` = 6.

### 1.2 Monster Instance (`struct monst`)

Each live monster on a level is a `struct monst` linked into the `fmon` list.
Key fields:

| Field | Type | Meaning |
|-------|------|---------|
| `nmon` | `struct monst*` | Next monster in level's `fmon` list |
| `data` | `struct permonst*` | Pointer into `mons[]` for *current* form |
| `m_id` | `unsigned` | Unique ID across entire game |
| `mnum` | `short` | Permanent species index (survives polymorph for corpse) |
| `cham` | `short` | If shapeshifter: original `mons[]` index; else `NON_PM` |
| `movement` | `short` | Current movement points this turn |
| `m_lev` | `uchar` | Adjusted difficulty level |
| `malign` | `aligntyp` | Alignment relative to player |
| `mx, my` | `coordxy` | Map position |
| `mux, muy` | `coordxy` | Where monster thinks hero is |
| `mhp, mhpmax` | `int` | Current and max hit points |
| `mtame` | `schar` | Tameness level (0 = not tame, 1..20) |
| `mpeaceful` | `Bitfield(1)` | Does not attack unprovoked |
| `mintrinsics` | `unsigned short` | Acquired intrinsic resistances (low 8 bits = MR_*) |
| `mextrinsics` | `unsigned short` | Equipment-granted resistances |
| `mspeed` | `Bitfield(2)` | 0=normal, MSLOW=1, MFAST=2 |
| `permspeed` | `Bitfield(2)` | Intrinsic speed value |
| `mrevived` | `Bitfield(1)` | Has been revived from dead |
| `mcloned` | `Bitfield(1)` | Is a clone |
| `mflee` | `Bitfield(1)` | Currently fleeing |
| `mfleetim` | `Bitfield(7)` | Flee timeout (0..127) |
| `msleeping` | `Bitfield(1)` | Asleep |
| `mblinded` | `Bitfield(7)` | Temporary blindness counter |
| `mfrozen` | `Bitfield(7)` | Paralysis/busy counter |
| `mcanmove` | `Bitfield(1)` | Can move (cleared during paralysis) |
| `mconf` | `Bitfield(1)` | Confused |
| `mstun` | `Bitfield(1)` | Stunned |
| `mcan` | `Bitfield(1)` | Has been cancelled |
| `minvis` | `Bitfield(1)` | Currently invisible |
| `perminvis` | `Bitfield(1)` | Intrinsic invisibility |
| `female` | `Bitfield(1)` | Is female |
| `mspec_used` | `int` | Special attack cooldown timer |
| `minvent` | `struct obj*` | Inventory list |
| `mw` | `struct obj*` | Wielded weapon |
| `misc_worn_check` | `long` | Worn equipment bitmask |
| `mstrategy` | `unsigned long` | AI strategy flags |
| `mlstmv` | `long` | Last move timestamp |
| `meating` | `int` | Eating timeout |

Relationship: `monst.data` points to `mons[monst.mnum]` under normal
conditions. When polymorphed, `monst.data` points to the new form while
`monst.mnum` retains the original. `monst.cham` stores the innate form for
shapeshifters (chameleon, doppelganger, sandestin, vampires).

Dead monster check: `DEADMONSTER(mon)` is `(mon->mhp < 1)`. Dead monsters
remain on `fmon` until `dmonsfree()` at end of turn.

---

## 2. Monster HP Calculation

### 2.1 Level Adjustment (`adj_lev`)

When a monster is created, its actual level `m_lev` is computed from the
species base `mlevel`:

```
fn adj_lev(ptr) -> u8:
    if ptr == Wizard_of_Yendor:
        return min(ptr.mlevel + times_wizard_died, 49)

    if ptr.mlevel > 49:
        return 50   // "special" demons/devils

    let tmp = ptr.mlevel
    let lev_diff = level_difficulty() - ptr.mlevel

    if lev_diff < 0:
        tmp -= 1                    // harder monster than level: reduce by 1
    else:
        tmp += lev_diff / 5         // easier level: +1 per 5 depth units

    let plyr_diff = player_level - ptr.mlevel
    if plyr_diff > 0:
        tmp += plyr_diff / 4        // additional +1 per 4 player levels above

    let upper = min(3 * ptr.mlevel / 2, 49)   // crude cap at 1.5x base
    return clamp(tmp, 0, upper)
```

`level_difficulty()`:
- Normal dungeon: `depth(&u.uz)` (adjusted for builds-up branches)
- Carrying Amulet of Yendor: `deepest_lev_reached`
- Endgame: `depth(sanctum_level) + player_level / 2`

### 2.2 Initial HP (`newmonhp`)

```
fn newmonhp(mon, mndx):
    mon.m_lev = adj_lev(mons[mndx])

    if is_golem(mndx):
        mon.mhp = mon.mhpmax = golemhp(mndx)  // fixed per type (see table)

    else if is_rider(mndx):   // Death, Famine, Pestilence
        mon.mhp = mon.mhpmax = d(10, 8)  // 10d8 = 10..80

    else if mlevel > 49:      // "special" encoded HP
        mon.mhp = mon.mhpmax = 2 * (mlevel - 6)
        mon.m_lev = mon.mhp / 4

    else if is_adult_dragon(mndx):
        let N = mon.m_lev
        if In_endgame:
            mon.mhp = mon.mhpmax = 8 * N
        else:
            mon.mhp = mon.mhpmax = 4 * N + d(N, 4)

    else if mon.m_lev == 0:
        mon.mhp = mon.mhpmax = d(1, 4)   // 1d4 = 1..4
        basehp = 1

    else:   // standard formula
        let N = mon.m_lev
        mon.mhp = mon.mhpmax = d(N, 8)   // Nd8 = N..8N
        if is_home_elemental:
            mon.mhp *= 3
            mon.mhpmax = mon.mhp
        basehp = N

    // floor boost: if rolled minimum, add 1 (guarantees mhpmax >= 2)
    if mon.mhpmax == basehp:
        mon.mhpmax += 1
        mon.mhp = mon.mhpmax
```

**Golem HP table:**

| Golem | HP |
|-------|----|
| Straw | 20 |
| Paper | 20 |
| Rope | 30 |
| Leather | 40 |
| Flesh | 40 |
| Wood | 50 |
| Gold | 60 |
| Clay | 70 |
| Glass | 80 |
| Stone | 100 |
| Iron | 120 |

### 2.3 HP per Level (drain/gain)

`monhp_per_lvl()` for level drain or Stormbringer energy gain:
- Golem: `golemhp(type) / mlevel`
- mlevel > 49: `4 + rnd(4)` (5..8)
- Adult dragon: `4 + rn2(5)` (4..8)
- Level 0: `rnd(4)` (1..4)
- Default: `rnd(8)` (1..8)

### 2.4 Regeneration (`mon_regen`)

Called once per game turn (regardless of monster speed):

```
fn mon_regen(mon, digest_meal):
    if moves % 20 == 0 OR regenerates(mon.data):  // M1_REGEN
        healmon(mon, 1, 0)   // +1 HP, not exceeding mhpmax
    if mon.mspec_used > 0:
        mon.mspec_used -= 1
    if digest_meal and mon.meating > 0:
        mon.meating -= 1
```

Standard monsters heal 1 HP every 20 turns. Monsters with `M1_REGEN` (trolls,
Vlad, etc.) heal 1 HP every turn.

### 2.5 `healmon` details

```
fn healmon(mon, amt, overheal) -> int:
    if mon.mhp + amt > mon.mhpmax + overheal:
        mon.mhpmax += overheal
        mon.mhp = mon.mhpmax
    else:
        mon.mhp += amt
        if mon.mhp > mon.mhpmax:
            mon.mhpmax = mon.mhp   // mhpmax grows to match
    return mon.mhp - old_mhp
```

Note: when `overheal` is 0 (normal regen), `mhp` is capped at `mhpmax`. But
healing from eating objects uses `amt = weight`, which can raise `mhpmax` since
the second branch updates `mhpmax = mhp` when `mhp` exceeds `mhpmax`.
[疑似 bug] The non-pet healing path `healmon(mtmp, objects[otmp->otyp].oc_weight, 0)` can increase `mhpmax` beyond the creature's natural maximum because the else branch sets `mhpmax = mhp` when `mhp > mhpmax`, which cannot happen with `overheal=0` since the first branch caps it. Actually, re-reading: the first branch fires when `mhp + amt > mhpmax + 0`, so if `amt > mhpmax - mhp`, it sets `mhp = mhpmax`. The else branch fires when `mhp + amt <= mhpmax`, so `mhp` can never exceed `mhpmax` in the else branch when `overheal=0`. This is correct.

---

## 3. Monster Movement

### 3.1 Speed System

The movement system is tick-based:
- Each game turn, every monster accumulates `mcalcmove()` movement points.
- To take an action, a monster spends `NORMAL_SPEED` (12) points.
- If points remain >= 12 after one action, the monster can act again.

### 3.2 Movement Point Calculation (`mcalcmove`)

```
fn mcalcmove(mon, m_moving: bool) -> int:
    let mmove = mon.data.mmove   // base speed from permonst

    // speed adjustments
    if mon.mspeed == MSLOW:
        if mmove < 12:
            mmove = (2 * mmove + 1) / 3     // lose ~1/3 speed
        else:
            mmove = 4 + mmove / 3            // lose ~2/3 speed
    else if mon.mspeed == MFAST:
        mmove = (4 * mmove + 2) / 3          // gain ~1/3 speed

    // galloping steed bonus
    if mon == steed AND galloping AND context.mv:
        mmove = (rn2(2) ? 4 : 5) * mmove / 3    // ~1.5x with variance

    // randomized rounding (anti-kiting)
    if m_moving:
        let remainder = mmove % 12
        mmove -= remainder
        if rn2(12) < remainder:
            mmove += 12

    return mmove
```

**Speed examples:**

| Base speed | Normal | MSLOW | MFAST |
|-----------|--------|-------|-------|
| 6 | 6 | 4 | 8 |
| 12 | 12 | 8 | 16 |
| 15 | 15 | 9 | 20 |
| 18 | 18 | 10 | 24 |
| 24 | 24 | 12 | 32 |

The randomized rounding ensures that a speed-12 monster always gets exactly 1
action per turn, but a speed-15 monster gets 1 action on 75% of turns and 2
actions on 25% of turns (on average).

### 3.3 Movement Execution (`movemon`)

Per-turn flow:
1. `mcalcdistress()` for all monsters: regen, shapeshift check, timeout
   decrements for `mblinded`, `mfrozen`, `mfleetim`.
2. `movemon()` iterates `fmon`:
   - Skip dead, off-map, or migrating monsters.
   - If `mtmp->movement < NORMAL_SPEED`: skip (not enough points).
   - `mtmp->movement -= NORMAL_SPEED` (spend action).
   - If movement still >= NORMAL_SPEED: set `somebody_can_move = TRUE`.
   - Check `minliquid()`, equipping, hiding, Conflict.
   - Call `dochugw()` for actual AI move.
3. `dmonsfree()` removes truly dead monsters from `fmon`.
4. Return `somebody_can_move` to allow additional passes.

---

## 4. Monster Death Processing

### 4.1 Death Call Hierarchy

```
killed(mtmp)              -- hero killed; calls xkilled(XKILL_GIVEMSG)
xkilled(mtmp, flags)      -- hero killed, full processing (XP, alignment, corpse)
  |-> mondead(mtmp)        -- core death: life-save, vamp-rise, stats, detach
  |     |-> lifesaved_monster()
  |     |-> vamprises()    -- shifted vampire reverts instead of dying
  |     |-> m_detach()     -- remove from map, drop inventory
  |-> make_corpse()        -- generate corpse object (if applicable)

mondied(mtmp)              -- monster-initiated death; calls mondead() + corpse
monkilled(mtmp, txt, how)  -- killed by another monster
mongone(mtmp)              -- removed from game (not died; genocide, dismiss)
monstone(mtmp)             -- petrification death -> statue
```

### 4.2 `mondead` Detail

```
fn mondead(mtmp):
    mtmp.mhp = 0
    lifesaved_monster(mtmp)
    if !DEADMONSTER(mtmp): return   // life-saved

    // vampire shapeshifter reverts to vampire instead of dying
    if is_vampshifter(mtmp) AND vamprises(mtmp): return

    // optional sad feeling for pet death
    if was_pet: "You have a sad feeling..."

    // steam vortex creates gas cloud on death
    if mtmp.data == PM_STEAM_VORTEX:
        create_gas_cloud(mx, my, rn2(10)+5, 0)

    // vault guard special handling
    if mtmp.isgd AND !grddead(mtmp): return

    // restore shapeshifter to true form for death bookkeeping
    if ismnum(mtmp.cham):
        set_mon_data(mtmp, mons[mtmp.cham])
        mtmp.cham = NON_PM
    else if were in animal form:
        set_mon_data to human_were form

    // increment kill count (max 255)
    let mndx = monsndx(mtmp.data)
    if mvitals[mndx].died < 255:
        mvitals[mndx].died++

    // quest leader tracking
    if mtmp.m_id == quest_leader_id:
        quest_status.leader_is_dead = TRUE

    // Kops may respawn (2/5 chance, near stairs or random)
    if mtmp.data.mlet == S_KOP:
        if rnd(5) <= 2: makemon(same_type)

    m_detach(mtmp, mptr, TRUE)
```

### 4.3 `m_detach` Detail

```
fn m_detach(mtmp, mptr, due_to_death):
    unleash if leashed
    remove light source if emitting
    mon_leaving_level(mtmp)   // remove from map grid
    mtmp.mhp = 0
    if mtmp.iswiz: wizdeadorgone()
    if due_to_death:
        handle nemesis/leader death messages
        relobj(mtmp)   // drop all inventory onto map
    handle shopkeeper/worm cleanup
    mtmp.mstate |= MON_DETACH
    iflags.purge_monsters++
    dismount if steed
```

### 4.4 Life Saving

```
fn lifesaved_monster(mtmp):
    let lifesave = worn AMULET_OF_LIFE_SAVING
    if !lifesave: return

    // nonliving monsters can't be life-saved (except vampshifters)
    display messages
    consume the amulet
    mtmp.mcanmove = 1
    mtmp.mfrozen = 0
    mtmp.mhpmax = max(mtmp.m_lev + 1, 10)
    mtmp.mhp = mtmp.mhpmax

    if genocided:
        "Unfortunately still genocided..."
        mtmp.mhp = 0   // dies anyway
```

### 4.5 Vampire Rising

When a shapeshifted vampire (bat/wolf/fog cloud) dies:
- If not in native vampire form AND native form not genocided:
  - Restore full HP: `mhpmax = max(m_lev + 1, 10); mhp = mhpmax`
  - `newcham()` back to vampire form
  - Display transformation message
  - If on closed door, smash it (possibly booby-trapped)

### 4.6 Corpse Generation

`corpse_chance()` determines if a corpse is dropped:
- Vlad / Liches: never (body crumbles to dust)
- AT_BOOM monsters (gas spore): explode instead, no corpse
- Level-specific: no corpse on Rogue level, no-deathdrops levels, or
  graveyard undead (2/3 chance no corpse)
- Always drop: big monsters, lizards (non-cloned), golems, riders,
  player-type monsters, shopkeepers
- Otherwise: `!rn2(2 + (G_FREQ < 2) + verysmall)` chance
  - Common monsters: 1/2
  - Rare monsters: 1/3
  - Very small rare: 1/4

`make_corpse()` creates species-specific drops:
- **Dragons**: 1/3 chance of scales (1/20 if revived)
- **Unicorns**: horn (degraded if revived; crumbles if revived + rn2(2))
- **Long worms**: worm tooth
- **Vampires/Mummies/Zombies**: corpse of living counterpart, pre-aged
- **Golems**: material items instead of corpse (iron chains, glass gems, etc.)
- **Puddings/Oozes**: globs instead of corpses (merge with adjacent globs)

### 4.7 Treasure Drop

When hero kills a monster (in `xkilled`):
- 1/6 chance of random treasure item (further filtered)
- No treasure from: swallowed monsters, steeds, Kops, cloned monsters
- Food items dropped only if monster has M2_COLLECT
- Large items not dropped by small monsters

---

## 5. Experience Award

### 5.1 Base Experience (`experience()`)

```
fn experience(mtmp, nk: int) -> int:
    let ptr = mtmp.data
    let tmp = 1 + mtmp.m_lev^2

    // AC bonus: for AC < 3
    let ac = find_mac(mtmp)
    if ac < 3:
        if ac < 0: tmp += (7 - ac) * 2
        else:      tmp += (7 - ac) * 1

    // speed bonus
    if ptr.mmove > 18: tmp += 5    // very fast (> 1.5 * NORMAL_SPEED)
    else if ptr.mmove > 12: tmp += 3

    // attack type bonus (for each of 6 attack slots)
    for each attack:
        if aatyp > AT_BUTT:
            if aatyp == AT_WEAP: tmp += 5
            else if aatyp == AT_MAGC: tmp += 10
            else: tmp += 3

    // damage type bonus (for each attack)
    for each attack:
        if adtyp > AD_PHYS and adtyp < AD_BLND:
            tmp += 2 * mtmp.m_lev
        else if adtyp in {AD_DRLI, AD_STON, AD_SLIM}:
            tmp += 50
        else if adtyp != AD_PHYS:
            tmp += mtmp.m_lev
        // heavy damage bonus
        if damn * damd > 23:
            tmp += mtmp.m_lev
        // drowning eel special
        if adtyp == AD_WRAP and mlet == S_EEL and !Amphibious:
            tmp += 1000

    // extra nasty bonus (M2_NASTY)
    if extra_nasty(ptr): tmp += 7 * mtmp.m_lev

    // high level bonus
    if mtmp.m_lev > 8: tmp += 50

    // mail daemon override
    if ptr == PM_MAIL_DAEMON: tmp = 1

    // revived/cloned diminishing returns
    if mtmp.mrevived or mtmp.mcloned:
        // nk = total killed of this species (including this one)
        // kills 1..20: full XP
        // kills 21..40: XP/2
        // kills 41..80: XP/4
        // kills 81..120: XP/8
        // kills 121..180: XP/16
        // kills 181..240: XP/32
        // kills 241..255+: XP/64
        let bracket_size = 20
        for i in 0..:
            if nk <= bracket_size or tmp <= 1: break
            tmp = (tmp + 1) / 2
            nk -= bracket_size
            if i is odd: bracket_size += 20

    return tmp
```

### 5.2 Alignment Adjustments on Kill

| Condition | Alignment change |
|-----------|-----------------|
| Killed quest leader | `-(record + ALIGNLIM/2)`, +7 god anger, -20 luck |
| Killed quest nemesis | `+ALIGNLIM/4` |
| Killed quest guardian | `-ALIGNLIM/8`, +1 anger, -4 luck |
| Killed co-aligned priest | -2, lose divine protection |
| Killed cross-aligned priest | +2 |
| Killed Moloch priest | `+ALIGNLIM/4` |
| Killed own pet | -15 alignment |
| Killed peaceful monster | -5 |
| Killed peaceful (50%) or pet | -1 luck |
| Murder (non-hostile human, L/N alignment) | lose telepathy, -2 luck |
| Same-aligned unicorn | -5 luck, "feel guilty" |
| Plus `mtmp->malign` (pre-computed) | varies |

---

## 6. Monster Flags

### 6.1 M1 Flags (Body/Movement Properties)

| Flag | Hex | Effect |
|------|-----|--------|
| `M1_FLY` | 0x00000001 | Can fly; avoids pits, water, lava |
| `M1_SWIM` | 0x00000002 | Can traverse water without drowning |
| `M1_AMORPHOUS` | 0x00000004 | Can flow under doors |
| `M1_WALLWALK` | 0x00000008 | Can phase through solid rock |
| `M1_CLING` | 0x00000010 | Clings to ceiling; avoids pits |
| `M1_TUNNEL` | 0x00000020 | Can dig through rock |
| `M1_NEEDPICK` | 0x00000040 | Needs pick-axe to tunnel |
| `M1_CONCEAL` | 0x00000080 | Hides under objects on floor |
| `M1_HIDE` | 0x00000100 | Mimics/blends with ceiling/floor |
| `M1_AMPHIBIOUS` | 0x00000200 | Survives underwater |
| `M1_BREATHLESS` | 0x00000400 | Does not need to breathe |
| `M1_NOTAKE` | 0x00000800 | Cannot pick up objects |
| `M1_NOEYES` | 0x00001000 | No eyes (immune to gaze, blinding) |
| `M1_NOHANDS` | 0x00002000 | No hands (can't open doors, handle items) |
| `M1_NOLIMBS` | 0x00006000 | No arms/legs (superset of NOHANDS) |
| `M1_NOHEAD` | 0x00008000 | No head (immune to beheading) |
| `M1_MINDLESS` | 0x00010000 | No mind (immune to mind effects) |
| `M1_HUMANOID` | 0x00020000 | Humanoid body plan |
| `M1_ANIMAL` | 0x00040000 | Animal body plan |
| `M1_SLITHY` | 0x00080000 | Serpentine body |
| `M1_UNSOLID` | 0x00100000 | No solid/liquid body |
| `M1_THICK_HIDE` | 0x00200000 | Thick hide or scales |
| `M1_OVIPAROUS` | 0x00400000 | Can lay eggs |
| `M1_REGEN` | 0x00800000 | Regenerates 1 HP per turn (vs 1/20 turns) |
| `M1_SEE_INVIS` | 0x01000000 | Can see invisible creatures |
| `M1_TPORT` | 0x02000000 | Can teleport |
| `M1_TPORT_CNTRL` | 0x04000000 | Controls teleport destination |
| `M1_ACID` | 0x08000000 | Acidic to eat |
| `M1_POIS` | 0x10000000 | Poisonous to eat |
| `M1_CARNIVORE` | 0x20000000 | Eats corpses |
| `M1_HERBIVORE` | 0x40000000 | Eats fruits |
| `M1_OMNIVORE` | 0x60000000 | Eats both (CARNIVORE | HERBIVORE) |
| `M1_METALLIVORE` | 0x80000000 | Eats metal |

### 6.2 M2 Flags (Type/Behavior Properties)

| Flag | Hex | Effect |
|------|-----|--------|
| `M2_NOPOLY` | 0x00000001 | Players may not polymorph into this |
| `M2_UNDEAD` | 0x00000002 | Is walking dead |
| `M2_WERE` | 0x00000004 | Is a lycanthrope |
| `M2_HUMAN` | 0x00000008 | Is a human |
| `M2_ELF` | 0x00000010 | Is an elf |
| `M2_DWARF` | 0x00000020 | Is a dwarf |
| `M2_GNOME` | 0x00000040 | Is a gnome |
| `M2_ORC` | 0x00000080 | Is an orc |
| `M2_DEMON` | 0x00000100 | Is a demon |
| `M2_MERC` | 0x00000200 | Is a guard or soldier |
| `M2_LORD` | 0x00000400 | Is a lord to its kind |
| `M2_PRINCE` | 0x00000800 | Is an overlord to its kind |
| `M2_MINION` | 0x00001000 | Is a deity's minion |
| `M2_GIANT` | 0x00002000 | Is a giant |
| `M2_SHAPESHIFTER` | 0x00004000 | Shapeshifting species |
| `M2_MALE` | 0x00010000 | Always male |
| `M2_FEMALE` | 0x00020000 | Always female |
| `M2_NEUTER` | 0x00040000 | Neither male nor female |
| `M2_PNAME` | 0x00080000 | Name is a proper name |
| `M2_HOSTILE` | 0x00100000 | Always starts hostile |
| `M2_PEACEFUL` | 0x00200000 | Always starts peaceful |
| `M2_DOMESTIC` | 0x00400000 | Can be tamed by feeding |
| `M2_WANDER` | 0x00800000 | Wanders randomly |
| `M2_STALK` | 0x01000000 | Follows to other levels |
| `M2_NASTY` | 0x02000000 | Extra nasty (more XP) |
| `M2_STRONG` | 0x04000000 | Strong/big monster |
| `M2_ROCKTHROW` | 0x08000000 | Throws boulders |
| `M2_GREEDY` | 0x10000000 | Likes gold |
| `M2_JEWELS` | 0x20000000 | Likes gems |
| `M2_COLLECT` | 0x40000000 | Picks up weapons and food |
| `M2_MAGIC` | 0x80000000 | Picks up magic items |

### 6.3 M3 Flags (Strategy/Special)

| Flag | Hex | Effect |
|------|-----|--------|
| `M3_WANTSAMUL` | 0x0001 | Wants the Amulet of Yendor |
| `M3_WANTSBELL` | 0x0002 | Wants the Bell of Opening |
| `M3_WANTSBOOK` | 0x0004 | Wants the Book of the Dead |
| `M3_WANTSCAND` | 0x0008 | Wants the Candelabrum |
| `M3_WANTSARTI` | 0x0010 | Wants the quest artifact |
| `M3_WANTSALL` | 0x001f | Wants any major artifact |
| `M3_WAITFORU` | 0x0040 | Waits until sees you or attacked |
| `M3_CLOSE` | 0x0080 | Lets you approach unless attacked |
| `M3_COVETOUS` | 0x001f | = WANTSALL; wants something |
| `M3_INFRAVISION` | 0x0100 | Has infravision |
| `M3_INFRAVISIBLE` | 0x0200 | Visible by infravision |
| `M3_DISPLACES` | 0x0400 | Pushes other monsters out of its way |

### 6.4 Generation Flags (`geno` field)

| Flag | Hex | Meaning |
|------|-----|---------|
| `G_UNIQ` | 0x1000 | Generated only once |
| `G_NOHELL` | 0x0800 | Not generated in Gehennom |
| `G_HELL` | 0x0400 | Generated only in Gehennom |
| `G_NOGEN` | 0x0200 | Only generated specially |
| `G_SGROUP` | 0x0080 | Appears in small groups |
| `G_LGROUP` | 0x0040 | Appears in large groups |
| `G_GENO` | 0x0020 | Can be genocided |
| `G_NOCORPSE` | 0x0010 | Never leaves a corpse |
| `G_FREQ` | 0x0007 | Creation frequency (0..7) |

---

## 7. Monster Resistances and Vulnerabilities

### 7.1 Resistance Bitmask (8 bits, stored in `mresists`, `mintrinsics`, `mextrinsics`)

| Bit | Constant | Resistance |
|-----|----------|------------|
| 0x01 | `MR_FIRE` | Fire |
| 0x02 | `MR_COLD` | Cold |
| 0x04 | `MR_SLEEP` | Sleep |
| 0x08 | `MR_DISINT` | Disintegration |
| 0x10 | `MR_ELEC` | Electricity |
| 0x20 | `MR_POISON` | Poison |
| 0x40 | `MR_ACID` | Acid |
| 0x80 | `MR_STONE` | Petrification |

### 7.2 Resistance Check

`resists_fire(mon)` etc. call `Resists_Elem(mon, prop_index)` which checks:
1. Species innate: `mon->data->mresists & MR_mask`
2. Intrinsic (gained from eating): `mon->mintrinsics & MR_mask`
3. Extrinsic (from equipment): `mon->mextrinsics & MR_mask`
4. Equipment-carried artifact effects (e.g., Fire Brand gives fire resistance)

Shorthand: `mon_resistancebits(mon) = data->mresists | mextrinsics | mintrinsics`

### 7.3 Extended Properties (MR2_*)

Stored in the upper 8 bits of `mintrinsics`/`mextrinsics`:

| Constant | Hex | Property |
|----------|-----|----------|
| `MR2_SEE_INVIS` | 0x0100 | See invisible |
| `MR2_LEVITATE` | 0x0200 | Levitation |
| `MR2_WATERWALK` | 0x0400 | Water walking |
| `MR2_MAGBREATH` | 0x0800 | Magical breathing |
| `MR2_DISPLACED` | 0x1000 | Displacement |
| `MR2_STRENGTH` | 0x2000 | Gauntlets of power |
| `MR2_FUMBLING` | 0x4000 | Fumbling |

### 7.4 Seen Resistance Tracking

`mon->seen_resistance` tracks what resistances the monster has observed the
*hero* demonstrate. Used for AI decisions. Flags: `M_SEEN_MAGR`, `M_SEEN_FIRE`,
`M_SEEN_COLD`, `M_SEEN_SLEEP`, `M_SEEN_DISINT`, `M_SEEN_ELEC`, `M_SEEN_POISON`,
`M_SEEN_ACID`, `M_SEEN_REFL`.

---

## 8. Tame / Peaceful / Hostile State Transitions

### 8.1 State Model

```
     +-- mtame > 0, mpeaceful = 1 --+
     |         TAME                  |
     +-------------------------------+
                |  (anger / abuse)
                v
     +-- mtame = 0, mpeaceful = 1 --+
     |       PEACEFUL                |
     +-------------------------------+
                |  (attack / setmangry)
                v
     +-- mtame = 0, mpeaceful = 0 --+
     |        HOSTILE                |
     +-------------------------------+
```

Invariant: `mtame > 0` implies `mpeaceful = 1` (enforced by sanity checks).

### 8.2 Taming (`tamedog`)

Conditions that prevent taming:
- Wizard of Yendor, Medusa, quest nemesis (M3_WANTSARTI): always rejected
- Shopkeepers: made peaceful only (via `make_happy_shk`)
- Already tame: feeding raises tameness (max 20 via eating, max 10 via magic)

On successful taming:
- `mtmp->mpeaceful = 1`
- Set initial tameness (via `initedog()` which sets `mtame = 10` typically)
- Allocate `edog` structure for pet AI
- Blessed scroll of taming: +2 to existing low tameness
- Full moon + night + dog: 1/6 chance of failure

Tameness range: 1..20 (eating can raise above 10; magic caps at 10).

### 8.3 Angering (`setmangry`)

```
fn setmangry(mtmp, via_attack):
    if via_attack and standing_on_Elbereth:
        "feel like a hypocrite"
        alignment penalty, erase Elbereth

    mtmp.mstrategy &= ~STRAT_WAITMASK
    if !mtmp.mpeaceful: return
    if mtmp.mtame: return   // tame monsters don't get angry this way

    mtmp.mpeaceful = 0
    alignment adjustments for priest
    display anger message
    if attacked quest leader: anger all guardians
    other peacefuls may react
```

[疑似 bug] `setmangry()` returns immediately if `mtmp->mtame` is set, meaning
tame monsters cannot be made hostile via this path. The comment in source says
"[FIXME: this logic seems wrong; peaceful humanoids gasp or exclaim when they
see you attack a peaceful monster but they just casually look the other way
when you attack a pet?]"

### 8.4 Waking (`wakeup`)

```
fn wakeup(mtmp, via_attack):
    wake_msg(mtmp, via_attack)
    mtmp.msleeping = 0
    reveal mimics
    finish_meating()
    if via_attack:
        growl(mtmp) if was sleeping
        setmangry(mtmp, TRUE)
        priest retribution if in temple
        shopkeeper pursuit if outside shop
```

### 8.5 Pet Abuse

Pets track `killed_by_u` in `edog`. Killing a pet triggers:
- -15 alignment
- -1 luck
- Livelog entry
- Thunder sound effect

---

## 9. Monster Level Adjustment by Dungeon Depth

See section 2.1 (`adj_lev`). Summary:

- Base level from `permonst.mlevel`
- Adjusted up by `(level_difficulty - mlevel) / 5` if dungeon harder
- Adjusted up by `(player_level - mlevel) / 4` if player higher level
- Adjusted down by 1 if mlevel > level_difficulty
- Capped at `min(3 * mlevel / 2, 49)`
- Floor of 0

Special cases:
- Wizard of Yendor: `mlevel + times_killed`, cap 49
- `mlevel > 49`: fixed at 50 (special demons)

### 9.1 Difficulty Filtering for Generation

```
monmax_difficulty(levdif) = (levdif + u.ulevel) / 2
monmin_difficulty(levdif) = levdif / 6
montoostrong(monindx, lev) = mons[monindx].difficulty > lev
montooweak(monindx, lev)   = mons[monindx].difficulty < lev
```

---

## 10. Polymorph (Monster)

### 10.1 `newcham` -- Core Polymorph Function

```
fn newcham(mtmp, mdat, ncflags) -> bool:
    // immunity checks
    if mtmp.cham == NON_PM:   // not a natural shapechanger
        if is_rider: return 0  // Riders immune
        if mbirth_limit < MAXMONNO: return 0  // Nazgul, Erinyes immune
        // cancelled shapeshifters become uncancelled
        if mtmp.mcan and !Protection_from_shape_changers:
            mtmp.cham = pm_to_cham(mtmp.data)
            if mtmp.cham != NON_PM: mtmp.mcan = 0

    // choose form if none specified
    if mdat == NULL:
        mdat = try select_newcham_form() up to 20 times
        if failed: return 0

    if genocided(mdat): return 0
    if mdat == current_form: return 0

    // HP proportional transfer
    let hpn = mtmp.mhp, hpd = mtmp.mhpmax
    newmonhp(mtmp, monsndx(mdat))    // sets new m_lev and mhpmax
    mtmp.mhp = hpn * mtmp.mhp / hpd  // same fraction of max
    if mtmp.mhp <= 0: mtmp.mhp = 1

    set_mon_data(mtmp, mdat)
    // update visibility, leash, light source, equipment
    // gender: 10% chance of change (except vampires)
    // drop boulders if no longer a boulder-thrower
    // handle engulfment release if new form can't engulf
    // new vampires become full shapeshifters

    return 1
```

### 10.2 Shapeshift Form Selection (`select_newcham_form`)

| Shapeshifter type | Form selection |
|-------------------|----------------|
| Sandestin | 6/7: `pick_nasty` (difficulty < Archon); 1/7: random |
| Doppelganger | 1/7: nasty (difficulty < Jabberwock); 3/7: role monster; 1/7: quest guardian; 2/7: random humanoid |
| Chameleon | 1/3: random animal; 2/3: random |
| Vampire | `pickvampshape`: fog cloud, wolf, or vampire bat |
| Regular (NON_PM) | Dragon form if wearing dragon scales/mail; else random |

Random form: index in `LOW_PM..SPECIAL_PM` range, filtered by `polyok()`.

### 10.3 Werewolf Transformation (`were_change`)

Called each turn for lycanthropes:

```
fn were_change(mon):
    if !is_were(mon.data): return

    if is_human(mon.data):    // human form -> animal
        chance = night ? (full_moon ? 1/3 : 1/30)
                       : (full_moon ? 1/10 : 1/50)
        if !Protection_from_shape_changers AND rn2(denom) == 0:
            new_were(mon)     // transform to animal

    else:                     // animal form -> human
        if rn2(30) == 0 OR Protection_from_shape_changers:
            new_were(mon)     // transform to human
```

### 10.4 Vampire Shape Selection

Vampires in good health (>= 90% HP) shift to fog cloud, wolf, or vampire bat.
When low on HP (<= 1/6 max), shift back to vampire form.
Fog clouds at full HP may shift to wolf/bat if unseen by hero.

### 10.5 Gender on Polymorph

```
fn mgender_from_permonst(mtmp, mdat):
    if is_male(mdat): mtmp.female = FALSE
    else if is_female(mdat): mtmp.female = TRUE
    else if !is_neuter(mdat):
        // 10% chance of gender swap (not for vampires)
        if !rn2(10) and !is_vampire and !is_vampshifter:
            mtmp.female = !mtmp.female
```

---

## 11. Monster Properties

### 11.1 Locomotion

| Property | Test | Source |
|----------|------|--------|
| Flying | `is_flyer(ptr)` | M1_FLY |
| Floating | `is_floater(ptr)` | `mlet == S_EYE or S_LIGHT` |
| Clinging | `is_clinger(ptr)` | M1_CLING |
| Swimming | `is_swimmer(ptr)` | M1_SWIM |
| Amphibious | `amphibious(ptr)` | M1_AMPHIBIOUS |
| Can't drown | `cant_drown(ptr)` | swim OR amphibious OR breathless |
| Passes walls | `passes_walls(ptr)` | M1_WALLWALK |
| Amorphous | `amorphous(ptr)` | M1_AMORPHOUS |
| Tunneling | `tunnels(ptr)` | M1_TUNNEL |
| Needs pick | `needspick(ptr)` | M1_NEEDPICK |
| Grounded | `grounded(ptr)` | !flyer AND !floater AND (!clinger OR no ceiling) |

### 11.2 Sensory

| Property | Test | Source |
|----------|------|--------|
| See invisible | `perceives(ptr)` | M1_SEE_INVIS |
| Infravision | `infravision(ptr)` | M3_INFRAVISION |
| No eyes | `!haseyes(ptr)` | M1_NOEYES |
| Telepathic | `telepathic(ptr)` | floating eye, mind flayer, master mind flayer |
| Infravisible | `infravisible(ptr)` | M3_INFRAVISIBLE |

### 11.3 Special Properties

| Property | Test | Source |
|----------|------|--------|
| Regeneration | `regenerates(ptr)` | M1_REGEN -- heals every turn not every 20 |
| Teleport | `can_teleport(ptr)` | M1_TPORT |
| Teleport control | `control_teleport(ptr)` | M1_TPORT_CNTRL |
| Breathless | `breathless(ptr)` | M1_BREATHLESS |
| Noncorporeal | `noncorporeal(ptr)` | `mlet == S_GHOST` |
| Nonliving | `nonliving(ptr)` | undead OR manes OR golem OR vortex |
| Acidic body | `acidic(ptr)` | M1_ACID |
| Poisonous body | `poisonous(ptr)` | M1_POIS |
| Thick hide | `thick_skinned(ptr)` | M1_THICK_HIDE |
| Mindless | `mindless(ptr)` | M1_MINDLESS |
| Shapeshifter | `is_shapeshifter(ptr)` | M2_SHAPESHIFTER |
| Covetous | `is_covetous(ptr)` | M3_COVETOUS |
| Displaces | `is_displacer(ptr)` | M3_DISPLACES |
| Light emitter | `emits_light(ptr)` | lights, flaming/shocking sphere, fire vortex, gold dragon |

### 11.4 Air Status

```
fn m_in_air(mtmp) -> bool:
    return is_flyer(data)
        OR is_floater(data)
        OR (is_clinger(data) AND has_ceiling AND mundetected)
```

---

## 12. Genocided / Extinct Tracking

### 12.1 Data Structure

`svm.mvitals[NUMMONS]` -- per-species tracking:

| Field | Type | Meaning |
|-------|------|---------|
| `born` | `uchar` | Count of this species created (0..255) |
| `died` | `uchar` | Count that have died (0..255) |
| `mvflags` | `uchar` | G_GENOD, G_EXTINCT, G_KNOWN, MV_KNOWS_EGG |

### 12.2 Genocide (`G_GENOD`)

Set by scroll of genocide or wizard mode. Genocided monsters:
- Cannot be created
- Existing instances are killed
- Life-saving fails for genocided species
- `G_GENOD` flag persists for the entire game

### 12.3 Extinction (`G_EXTINCT`)

Automatic population control:

```
fn propagate(mndx, tally, ghostly) -> bool:
    let lim = mbirth_limit(mndx)
    let gone = mvitals[mndx].mvflags & G_GONE  // genocided or extinct

    result = (mvitals[mndx].born < lim) AND !gone

    // unique monsters: extinct after first creation
    if G_UNIQ and mndx != PM_HIGH_CLERIC:
        mvitals[mndx].mvflags |= G_EXTINCT

    if tally and (born < 255):
        mvitals[mndx].born++

    // auto-extinction at birth limit
    if born >= lim AND !G_NOGEN AND !G_EXTINCT:
        mvitals[mndx].mvflags |= G_EXTINCT

    return result
```

Birth limits (`mbirth_limit`):
- Nazgul: 9
- Erinyes: 3
- All others: `MAXMONNO` = 120

Unlike genocide, extinction can be reversed: `mvitals[mndx].mvflags &= ~G_EXTINCT`
is used in `makemon()` to allow creation of extinct monsters in wizard mode.

### 12.4 `G_GONE` macro

`G_GONE = G_GENOD | G_EXTINCT` -- used to check if a species is unavailable
for generation regardless of reason.

---

## 13. Monster Growth (`grow_up`)

When a monster kills another monster (or consumes a wraith corpse / gain level
potion), it may gain HP and level up:

```
fn grow_up(mtmp, victim) -> permonst*:
    if DEADMONSTER(mtmp): return NULL

    oldtype = monsndx(mtmp.data)
    newtype = little_to_big(oldtype)
    // special: killer bee + no victim (potion) -> queen bee

    if victim:  // killed a monster
        hp_threshold = m_lev * 8       // normal
        if m_lev == 0: hp_threshold = 4
        if is_golem: hp_threshold = (mhpmax / 10 + 1) * 10 - 1
        if is_home_elemental: hp_threshold *= 3

        lev_limit = 3 * ptr.mlevel / 2  // same as adj_lev upper bound
        if oldtype != newtype and mons[newtype].mlevel > lev_limit:
            lev_limit = mons[newtype].mlevel

        max_increase = rnd(victim.m_lev + 1)
        if mhpmax + max_increase > hp_threshold + 1:
            max_increase = max(hp_threshold + 1 - mhpmax, 0)
        cur_increase = max_increase > 1 ? rn2(max_increase) : 0

    else:  // wraith corpse or gain level potion
        max_increase = cur_increase = rnd(8)
        hp_threshold = 0       // always levels up
        lev_limit = 50

    mtmp.mhpmax += max_increase
    mtmp.mhp += cur_increase
    if mhpmax <= hp_threshold: return ptr  // no level gain

    // level limit adjustments
    if is_mplayer: lev_limit = 30
    else if lev_limit < 5: lev_limit = 5
    else if lev_limit > 49: lev_limit = min(ptr.mlevel > 49 ? 50 : 49)

    mtmp.m_lev++
    if m_lev >= mons[newtype].mlevel AND newtype != oldtype:
        // grow into new form
        if genocided(newtype): die
        else: set_mon_data(mtmp, mons[newtype])
        handle gender change

    // sanity caps
    if m_lev > lev_limit: m_lev--
    if mhpmax > 400: mhpmax = 400   // 50 * 8 absolute cap
    if mhp > mhpmax: mhp = mhpmax

    return ptr
```

---

## 14. Distress Processing (`m_calcdistress`)

Called once per turn for every monster (including immobile ones):

```
fn m_calcdistress(mtmp):
    // immobile monsters: check liquid
    if mtmp.data.mmove == 0:
        if minliquid(mtmp): return  // may have drowned/burned

    // regeneration
    mon_regen(mtmp, FALSE)

    // shapeshift check
    if ismnum(mtmp.cham):
        decide_to_shapeshift(mtmp)
    were_change(mtmp)

    // timeout decrements
    if mtmp.mblinded > 0:
        mtmp.mblinded--
        if mtmp.mblinded == 0: mtmp.mcansee = 1
    if mtmp.mfrozen > 0:
        mtmp.mfrozen--
        if mtmp.mfrozen == 0: mtmp.mcanmove = 1
    if mtmp.mfleetim > 0:
        mtmp.mfleetim--
        if mtmp.mfleetim == 0: mtmp.mflee = 0
```

[疑似 bug] Source comment: "FIXME: mtmp->mlstmv ought to be updated here"
-- `mlstmv` is not updated in `m_calcdistress`, which means immobile monsters
(mmove == 0) never update their `mlstmv` timestamp. This could cause issues
with catch-up logic for monsters that were temporarily off-level.

---

## 15. Liquid Hazards (`minliquid`)

Monsters in water or lava face hazards each turn:

- **Gremlins in water/fountain**: split (clone) with 2/3 probability, dry up
  fountain
- **Iron golems in water**: 1/5 chance of 2d6 rust damage per turn
- **Lava**: non-clingers, non-flyers, non-lava-likers:
  - Try teleport escape first
  - Non-fire-resistant: die (burns/boils/melts message)
  - Fire-resistant: 1 HP/turn until dead
- **Water**: non-clingers that can't survive underwater:
  - Try teleport escape first
  - Drown message and death
- **Eels out of water**: lose HP probabilistically
  (`mhp > 1 AND rn2(mhp) > rn2(8)` -> `mhp--`), flee

---

## 测试向量

### HP Calculation

| # | Species | Base mlevel | Dungeon depth | Player level | Expected m_lev | HP Formula |
|---|---------|-------------|---------------|-------------|----------------|------------|
| 1 | Newt | 0 | 1 | 1 | 0 | 1d4 (1..4), min 2 |
| 2 | Grid bug | 0 | 1 | 1 | 0 | 1d4 (1..4), min 2 |
| 3 | Kobold | 0 | 3 | 1 | 0 | 1d4 (1..4), min 2 |
| 4 | Gnome | 1 | 5 | 1 | 1 | 1d8 (1..8), min 2 |
| 5 | Hill giant | 6 | 10 | 10 | 7 | 7d8 (7..56), min 8 |
| 6 | Iron golem | 18 | 25 | 15 | adj_lev(18) | Fixed 120 HP |
| 7 | Death (Rider) | 30 | sanctum | 20 | adj_lev(30) | 10d8 (10..80) |
| 8 | Demogorgon | 106 (special) | any | any | `2*(106-6)/4 = 50` | `2*(106-6) = 200` HP |
| 9 | Gray dragon | 15 | 30 | 15 | adj_lev(15) | Endgame: 8N, else 4N+Nd4 |
| 10 | Straw golem | 3 | 5 | 5 | adj_lev(3) | Fixed 20 HP |

### Movement Speed

| # | Species | Base mmove | mspeed | Effective mmove | Actions per hero turn |
|---|---------|-----------|--------|----------------|----------------------|
| 1 | Newt | 6 | normal | 6 | 50% chance of 0, 50% of 1 |
| 2 | Grid bug | 12 | normal | 12 | exactly 1 |
| 3 | Dog | 16 | normal | 12 or 24 (random rounding) | 1 (2/3) or 2 (1/3) |
| 4 | Dog | 16 | MFAST | (4*16+2)/3 = 22 | 12 or 24 | 1 (5/6) or 2 (1/6) |
| 5 | Dog | 16 | MSLOW | 4+16/3 = 9 | 0 or 12 | 0 (1/4) or 1 (3/4) |

### Experience

| # | Monster | m_lev | AC | Speed | Attacks | Expected XP components |
|---|---------|-------|-----|-------|---------|----------------------|
| 1 | Newt (level 0) | 0 | 8 | 6 | 1d2 bite | 1+0=1 base, no AC/speed/attack bonuses => ~1 |
| 2 | Grid bug (level 0) | 0 | 9 | 12 | 1d1 elec | 1+0+0+0+0=1 base, +0 m_lev for elec => 1 |
| 3 | Archon (level 19) | 19 | -6 | 16 | multiple magic | 1+361+26+3+5+10+... very high |

### Boundary Conditions

| # | Scenario | Expected behavior |
|---|----------|-------------------|
| 1 | Monster with mhpmax=1 life-saved | mhpmax set to max(m_lev+1, 10) = 10 (for level <9) |
| 2 | Level 0 monster HP roll of all 1s | basehp=1, mhpmax=1, boost to 2 |
| 3 | Wizard of Yendor killed 50 times | adj_lev = min(mlevel + 50, 49) = 49 |
| 4 | adj_lev with mlevel=1, depth=1, plvl=1 | tmp=1, diff=0, +0 depth, +0 player, cap=1 => 1 |
| 5 | adj_lev with mlevel=1, depth=50, plvl=30 | tmp=1+49/5+29/4=1+9+7=17, cap=3*1/2=1, return min(17,1)=1 |
| 6 | Born count reaches 120 for common monster | G_EXTINCT set automatically |
| 7 | Revived monster killed 255 times | mvitals.died stays at 255 (no overflow) |
| 8 | grow_up mhpmax reaching 400 cap | mhpmax clamped to 400 (50*8) |
| 9 | MSLOW on speed 1 monster | (2*1+1)/3 = 1 (not reduced to 0) |
| 10 | MFAST on speed 1 monster | (4*1+2)/3 = 2 (not a no-op) |
| 11 | Vampire at 1/6 HP in bat form | reverts to vampire form, full HP restore |
| 12 | Life-saved genocided monster | life-save triggers, then immediately dies anyway |
