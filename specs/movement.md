# Movement Mechanism Spec

Source: `src/hack.c`, `src/allmain.c`, `src/do.c`, `src/detect.c`, `src/mon.c`, `src/engrave.c`, `src/monmove.c`, `src/lock.c`, `src/dokick.c`, `src/pickup.c`, `src/rnd.c`; headers `include/flag.h`, `include/hack.h`, `include/weight.h`, `include/youprop.h`, `include/permonst.h`

---

## 1. Movement Point System

### 1.1 Constants

```
NORMAL_SPEED = 12        // include/permonst.h:80
```

Each "turn" (game tick), every entity (hero and each monster) accumulates movement points. An entity can act when it has `>= NORMAL_SPEED` movement points. Each action that consumes a move costs `NORMAL_SPEED` points.

### 1.2 Hero Movement Calculation (`u_calc_moveamt`)

Called once per game turn when both hero and all monsters are out of movement points.

```pseudocode
fn u_calc_moveamt(wtcap: EncumbranceLevel):
    if riding_steed AND u.umoved:
        moveamt = mcalcmove(steed, moving=true)
    else:
        moveamt = youmonst.data.mmove   // base speed of current form

        if Very_fast:
            // Very_fast = extrinsic speed (boots, potion, or spell)
            // Defined as: (HFast & ~INTRINSIC) || EFast
            if rn2(3) != 0:   // 2/3 chance
                moveamt += NORMAL_SPEED
        else if Fast:
            // Fast = intrinsic speed (eaten tengu, etc.)
            // Defined as: HFast || EFast
            if rn2(3) == 0:   // 1/3 chance
                moveamt += NORMAL_SPEED

    // Encumbrance penalties (applied after speed bonuses)
    match wtcap:
        UNENCUMBERED => no change
        SLT_ENCUMBER => moveamt -= moveamt / 4           // lose 25%
        MOD_ENCUMBER => moveamt -= moveamt / 2           // lose 50%
        HVY_ENCUMBER => moveamt -= (moveamt * 3) / 4    // lose 75%
        EXT_ENCUMBER => moveamt -= (moveamt * 7) / 8    // lose 87.5%
        OVERLOADED   => no change (but hero can't move at all -- see carrying_too_much)

    u.umovement += moveamt
    if u.umovement < 0:
        u.umovement = 0
```

**Key detail**: The hero's base `mmove` as a human is 12 (NORMAL_SPEED). For polymorph forms, it comes from `mons[].mmove`.

### 1.3 Monster Movement Calculation (`mcalcmove`)

```pseudocode
fn mcalcmove(mon, m_moving: bool) -> int:
    mmove = mon.data.mmove    // base species speed

    if mon.mspeed == MSLOW:
        if mmove < 12:
            mmove = (2 * mmove + 1) / 3      // lose ~1/3
        else:
            mmove = 4 + (mmove / 3)           // lose ~2/3
    else if mon.mspeed == MFAST:
        mmove = (4 * mmove + 2) / 3           // gain ~1/3

    if mon == u.usteed AND u.ugallop AND context.mv:
        // gallop: multiply by ~1.5 with variance
        mmove = ((rn2(2) ? 4 : 5) * mmove) / 3

    if m_moving:
        // Stochastic rounding to multiples of NORMAL_SPEED
        // Prevents player from predicting free turns
        mmove_adj = mmove % NORMAL_SPEED
        mmove -= mmove_adj
        if rn2(NORMAL_SPEED) < mmove_adj:
            mmove += NORMAL_SPEED

    return mmove
```

### 1.4 Initial Movement Points

On new game start: `u.umovement = NORMAL_SPEED` (gives hero first move).

On restore: pending movement points are saved/restored.

---

## 2. moveloop_core() Turn Sequence

The main game loop calls `moveloop_core()` once per iteration. Here is the exact order of operations:

```pseudocode
fn moveloop_core():
    // ---- Housekeeping ----
    get_nh_event()
    dobjsfree()                      // free deferred object list
    if context.bypasses: clear_bypasses()
    if sanity_check: sanity_check()
    if context.resume_wish: makewish()

    // ---- If hero took an action (context.move is true) ----
    if context.move:
        u.umovement -= NORMAL_SPEED      // consume movement points

        do:  // hero-can't-move loop
            encumber_msg()

            // --- Monster movement phase ---
            context.mon_moving = true
            do:
                monscanmove = movemon()      // all monsters take turns
                if u.umovement >= NORMAL_SPEED:
                    break   // hero gets to move again
            while monscanmove
            context.mon_moving = false

            wtcap = near_capacity()

            if !monscanmove AND u.umovement < NORMAL_SPEED:
                // === New game turn boundary ===
                were_changes = 0
                mcalcdistress()              // monster trap/blind/etc adjustments

                // Reallocate monster movement points
                for each mtmp in fmon:
                    mtmp.movement += mcalcmove(mtmp, true)

                // Random monster generation
                if !rn2(demigod ? 25 : depth > stronghold ? 50 : 70):
                    makemon(random)

                u_calc_moveamt(wtcap)        // hero gets movement points
                settrack()
                moves++

                if moves >= 1_000_000_000:
                    "The dungeon capitulates." -> ESCAPED

                hero_seq = moves << 3

                // === Once-per-turn effects ===
                l_nhcore_call(NHCORE_MOVELOOP_TURN)
                if Glib: glibr()
                nh_timeout()
                run_regions()
                if u.ublesscnt: u.ublesscnt--
                saving_grace_turn = false

                // HP regeneration
                if u.uinvulnerable:
                    wtcap = UNENCUMBERED
                else if hp < hpmax:
                    regen_hp(wtcap)

                // Encumbrance HP drain
                if wtcap > MOD_ENCUMBER AND u.umoved:
                    if wtcap < EXT_ENCUMBER:
                        if moves % 30 == 0: overexert_hp()
                    else:
                        if moves % 10 == 0: overexert_hp()

                regen_pw(wtcap)

                // Random teleportation
                if Teleportation AND !rn2(85): tele()

                // Polymorph / lycanthropy
                if Polymorph AND !rn2(100): polyself()
                if lycanthrope AND !Upolyd AND !rn2(80 - 20*night()): you_were()

                // Automatic searching (Searching intrinsic)
                if Searching AND !level.noautosearch AND multi >= 0:
                    dosearch0(1)       // aflag=1 => intrinsic autosearch

                if Warning: warnreveal()
                dosounds()
                do_storms()
                gethungry()
                age_spells()
                exerchk()
                invault()
                if u.have.amulet: amulet()

                // Engraving degradation from standing
                if !rn2(40 + ACURR(A_DEX) * 3):
                    u_wipe_engr(rnd(3))

                // Demigod harassment
                if u.uevent.udemigod AND !u.uinvulnerable:
                    if u.udg_cnt: u.udg_cnt--
                    if !u.udg_cnt: intervene(); u.udg_cnt = rn1(200, 50)

                // Water/air level effects
                if Is_waterlevel OR Is_airlevel: movebubbles()
                else if level.fumaroles: fumaroles()

                // Multi-turn immobility countdown
                if multi < 0:
                    if ++multi == 0: unmul(null)

        while u.umovement < NORMAL_SPEED   // keep looping until hero can act

        // === Once-per-hero-action effects ===
        hero_seq++
        encumber_msg()              // second check for message purposes
        // Clairvoyance check
        if moves >= context.seer_turn:
            if (have_amulet OR Clairvoyant) AND !In_endgame AND !BClairvoyant:
                do_vicinity_map()
            context.seer_turn = moves + rn1(31, 15)   // 15..45

        if u.utrap AND u.utraptype == TT_LAVA: sink_into_lava()
        else if !u.umoved: pooleffects(false)

    // ---- Once-per-player-input ----
    find_ac()
    // Vision updates, hallucination refreshes
    // Bot line updates
    context.move = 1   // assume next input will use a move

    // Handle occupation/running/multi-turn actions
    if multi > 0:
        lookaround()
        if context.mv: domove()       // running
        else: --multi; rhack(cmd_key)  // repeat command
    else if multi == 0:
        rhack(0)                       // get player input

    if u.utotype: deferred_goto()      // level change
```

---

## 3. Directional Movement

### 3.1 Eight Directions + Up/Down

The hero can move in 8 compass directions (N, NE, E, SE, S, SW, W, NW) via `u.dx` and `u.dy` (each -1, 0, or +1). Up (`<`) and down (`>`) are separate commands (`doup()`, `dodown()`).

Grid bugs (`NODIAG(u.umonnum)`) can only move in 4 cardinal directions.

### 3.2 Diagonal Restrictions

**Tight diagonal squeeze**: When moving diagonally and both adjacent orthogonal squares contain obstacles (`bad_rock()`), the hero must pass a squeeze check:

```pseudocode
fn cant_squeeze_thru(mon) -> int:
    if Passes_walls: return 0  // can always pass
    if bigmonst(ptr) AND !amorphous AND !whirly AND !noncorporeal AND !slithy:
        return 1  // too big
    amt = (mon == hero) ? inv_weight() + weight_cap() : curr_mon_load(mon)
    if amt > WT_TOOMUCH_DIAGONAL:  // 600
        return 2  // carrying too much
    if mon == hero AND Sokoban:
        return 3  // Sokoban forbids diagonal squeeze
    return 0
```

**Door diagonal restriction**: Cannot move diagonally into or out of an intact doorway (not `D_NODOOR` or `D_BROKEN`) unless `Passes_walls`.

### 3.3 Impaired Movement

When `Stunned` or (`Confused` AND `!rn2(5)`), the direction is randomized via `confdir()`. Up to 50 attempts are made to find a valid direction; if all fail, the hero loses the move.

---

## 4. Door Interaction

### 4.1 Open (`doopen` / `doopen_indir`)

- Requires hands (`!nohands`)
- Cannot open from a pit
- If door is `D_CLOSED`:
  - Success check: `rnl(20) < (ACURRSTR + ACURR(A_DEX) + ACURR(A_CON)) / 3`
  - On success: door becomes `D_ISOPEN` (or `D_NODOOR` if trapped, triggering `b_trapped()`)
  - On failure: "The door resists!", exercise STR
- If door is `D_LOCKED` and `flags.autounlock` is set:
  - If `AUTOUNLOCK_APPLY_KEY`: auto-use a key/lockpick
  - If `AUTOUNLOCK_KICK`: prompt to kick
- `verysmall()` hero: "You're too small to pull the door open."

### 4.2 Close (`doclose`)

- Requires hands
- Door must be `D_ISOPEN`
- Success check: `rn2(25) < (ACURRSTR + ACURR(A_DEX) + ACURR(A_CON)) / 3` (auto-success if riding)
- Obstructed by monsters or objects at the door position

### 4.3 Kick (`kick_door`)

- Cannot kick while levitating (no leverage)
- Success check: `rnl(35) < avrg_attrib + (martial() ? ACURR(A_DEX) : 0)`
  - Where `avrg_attrib` = average of STR, DEX, CON
- Giant polyform (`is_giant`): always succeeds (`doorbuster`)
- On success:
  - If `D_TRAPPED`: door becomes `D_NODOOR`, trap triggers
  - Else if `ACURR(A_STR) > 18 AND !rn2(5) AND !shopdoor`: door shatters (`D_NODOOR`)
  - Else: door crashes open (`D_BROKEN`)
- On failure: "Whammm!!" / "Thwack!!", exercises STR

### 4.4 Lock/Unlock (`pick_lock`)

Uses key, lockpick, or credit card. Separate from movement but can be triggered via `autounlock`.

### 4.5 Trapped Doors

When a trapped door (`D_TRAPPED`) is opened or kicked open, `b_trapped()` fires, dealing damage. The door becomes `D_NODOOR`.

### 4.6 Autoopen

When `flags.autoopen` is set and the hero walks into a closed door (not confused, stunned, or fumbling), `doopen_indir()` is called automatically. If the hero is blind, stunned, has low DEX (<10), or is fumbling, they bump into the door instead ("Ouch! You bump into a door.") and exercise DEX negatively.

---

## 5. Collision Handling

### 5.1 Attacking a Monster

When moving onto a square containing a hostile (non-safe) monster:

1. If running and the monster is visible, stop running (`nomul(0)`, `context.move = 0`)
2. If `context.forcefight` or monster is detected: call `do_attack(mtmp)`
3. Displacer beast special: 50% chance of swapping places with hero (requires specific conditions: not helpless, not eating, not trapped, hero not trapped/stuck/riding, valid position)

### 5.2 Displacing a Pet

When moving onto a tame monster's square (and it's not a ceiling hider):

```pseudocode
fn domove_swap_with_pet(mtmp, x, y) -> bool:
    // Prevent swap if:
    // - pet pinned in pit by boulder
    // - diagonal move and pet is NODIAG
    // - boulder at hero's old pos and pet too big
    // - diagonal squeeze impossible for pet (bigmonst or load > 600)
    // - peaceful pet trapped (can't move out)
    // - displacing into unsafe pos / mundisplaceable

    // On success:
    remove_monster(x, y)
    place_monster(mtmp, u.ux0, u.uy0)
    // Check if pet lands in liquid or trap (can anger god if pet dies)
    // Pet death: alignment -15, possible guilt message
```

### 5.3 Pushing a Boulder (`moverock`)

When walking into a boulder:

1. If `context.nopick` (m-prefix): step onto boulder's spot if giant or small enough (squeeze)
2. If levitating or on air level: "don't have enough leverage to push"
3. If `verysmall()`: "too small to push"
4. Calculate destination `(rx, ry) = (u.ux + 2*u.dx, u.uy + 2*u.dy)`
5. Destination must be: in bounds, not obstructed, not iron bars, not another boulder, not diagonal-through-doorway, no non-corporeal monster blocking
6. Sokoban: no diagonal boulder pushes
7. Boulder pushed into traps: fills pits/holes, triggers landmines (90% chance), teleported by teleport traps
8. Boulder into water/lava: `boulder_hits_pool()` (fills pool, creates stepping stone)

For giants (`throws_rocks`): can step over boulders with m-prefix, or maneuver over them when push fails (incurs Sokoban guilt).

---

## 6. Swimming, Levitation, Flying Movement Rules

### 6.1 Swimming / Underwater

- `water_turbulence()`: When `u.uinwater`, `water_friction()` may alter direction
- Exiting water while encumbered: blocked if `near_capacity() > wtmod`
  - `wtmod = Swimming ? MOD_ENCUMBER : SLT_ENCUMBER`
  - i.e., swimmers can exit while Stressed; non-swimmers only while Burdened
- On entering pool without protection: `drown()` called
- `Wwalking` (water walking boots): walk on water surface; waterwall still blocks
- `Swimming` / `Amphibious` / `Breathless`: survive in water without drowning

### 6.2 Levitation

- Cannot push boulders (no leverage)
- Cannot kick doors (no leverage)
- Cannot go down stairs (floating above them)
  - `>` while levitating: can dismiss controlled levitation, otherwise "You are floating above the stairs"
- Going up stairs: allowed (but `near_capacity() > SLT_ENCUMBER` blocks regardless)
- Entering solid terrain: `BLevitation |= FROMOUTSIDE` (levitation suppressed)
- Sinks: `dosinkfall()` -- lose levitation items, crash damage `rn1(8, 25 - CON)`
- Carrying capacity: `MAX_CARR_CAP` (1000) when levitating

### 6.3 Flying

- Similar to levitation for terrain traversal (above pools/lava)
- Can push boulders (unlike levitation)
- Can kick doors (unlike levitation)
- Suppressed in solid terrain just like levitation
- Doesn't prevent using stairs
- Does not boost carrying capacity (unlike levitation)

### 6.4 Air Level

- `air_turbulence()`: When `Is_airlevel AND !Levitation AND !Flying`:
  - 75% chance (`rn2(4)`) of losing the move entirely:
    - 1/3: "You tumble in place." (exercise DEX down)
    - 1/3: "You can't control your movements very well."
    - 1/3: "It's hard to walk in thin air." (exercise DEX up)

### 6.5 Ice

- `slippery_ice_fumbling()`: On ice (not levitating):
  - Immune if: snow boots, cold resistant (steed or self), flying, floater, clinger, whirly form
  - Otherwise: `!rn2(Cold_resistance ? 3 : 2)` chance of triggering fumbling on next move
    - Without cold resistance: 50% per step
    - With cold resistance: 33% per step

---

## 7. Searching

### 7.1 Explicit Search (`dosearch` -> `dosearch0(0)`)

- Takes one turn
- Checks all 8 adjacent squares (3x3 minus center)
- When not autosearch (`aflag=0`): also calls `feel_location()` if Blind or in visible region

### 7.2 Automatic Search (Searching intrinsic)

- Called once per game turn in `moveloop_core()` as `dosearch0(1)` (aflag=1)
- Only if `Searching AND !level.flags.noautosearch AND multi >= 0`

### 7.3 Search Skill Check Formula

```pseudocode
fn dosearch0(aflag):
    fund = 0
    if uwep AND uwep.oartifact AND spec_ability(uwep, SPFX_SEARCH):
        fund = uwep.spe
    if ublindf AND ublindf.otyp == LENSES AND !Blind:
        fund += 2
    fund = min(fund, 5)

    for each adjacent (x, y):
        // Hidden door detection
        if levl[x][y].typ == SDOOR:
            if rnl(7 - fund):    // nonzero => failed
                continue         // failed to find
            // SUCCESS (rnl returned 0): convert secret door to door
            cvt_sdoor_to_door()
            exercise(A_WIS, true)

        // Hidden corridor detection
        else if levl[x][y].typ == SCORR:
            if rnl(7 - fund):
                continue
            // SUCCESS: convert to corridor
            levl[x][y].typ = CORR
            exercise(A_WIS, true)

        // Other squares
        else:
            // Hidden monster detection (if !aflag, i.e. explicit search only)
            if !aflag AND mtmp at (x,y): mfind0(mtmp, 0)

            // Trap detection
            if trap at (x,y) AND !trap.tseen AND !rnl(8):
                // SUCCESS: reveal trap
                if trap.ttyp == STATUE_TRAP:
                    activate_statue_trap()
                else:
                    find_trap(trap)
```

**`rnl(x)` formula** (luck-adjusted random):

```pseudocode
fn rnl(x) -> int:
    // Returns value in [0, x-1]
    // Good luck biases toward 0 (success for "rnl(n) == 0" checks)
    adjustment = Luck
    if x <= 15:
        adjustment = (abs(adjustment) + 1) / 3 * sgn(adjustment)
        //   Luck  11..13 -> adj  4
        //   Luck   8..10 -> adj  3
        //   Luck   5.. 7 -> adj  2
        //   Luck   2.. 4 -> adj  1
        //   Luck  -1,0,1 -> adj  0
        //   Luck  -4..-2 -> adj -1
        //   etc.
    i = RND(x)    // uniform [1, x]  (then conceptually -1 for 0-based, but impl differs)
    if adjustment AND rn2(37 + abs(adjustment)):
        i -= adjustment
        i = clamp(i, 0, x-1)
    return i
```

So for hidden doors: `rnl(7 - fund)` must return 0 to succeed. Base chance (no luck, fund=0) = 1/7 ~= 14.3%. With `fund=5`: `rnl(2)` -> 50% base chance.

For traps: `!rnl(8)` must be true (rnl returns 0). Base chance = 1/8 = 12.5%.

---

## 8. Elbereth Mechanics

### 8.1 Engraving

"Elbereth" can be engraved on the floor via the `#engrave` command. The engraving type matters:

| Type | Method | Degradation Rate |
|------|--------|-----------------|
| `DUST` | Writing in dust (finger) | Fast -- full character count wiped per `wipe_engr_at()` call |
| `ENGRAVE` | Engraving (hard tool/weapon) | Slow -- `rn2(1 + 50/(cnt+1))` chance per wipe attempt (usually 0 or 1 char) |
| `BURN` | Burning (wand of fire, etc.) | Very slow -- only on ice, or magical wipe with 50% chance |
| `MARK` | Marker pen | Same as DUST for wipe purposes |
| `ENGR_BLOOD` | Blood | Same as DUST for wipe purposes |
| `HEADSTONE` | Level feature | Never degrades, never counts for Elbereth |

**Weapon dulling for ENGRAVE**: Costs -1 enchantment per 2 characters engraved (deducted on 1st, 3rd, 5th, ... action). A +0 weapon allows 7 characters -- not enough for "Elbereth" (8 characters) in one go. Can engrave in segments: "Elb" + "ere" + "th".

**Wisdom exercise**: Engraving "Elbereth" by a player (not in `in_mklev`) exercises Wisdom.

### 8.2 Monster Fear (`onscary`)

```pseudocode
fn onscary(x, y, mtmp) -> bool:
    // Universally immune:
    if mtmp.iswiz OR is_lminion(mtmp) OR mtmp == Angel OR is_rider(mtmp):
        return false

    // Immune to location-based scaring:
    if mtmp.data.mlet == S_HUMAN OR unique_corpstat(mtmp):
        return false

    // Immune in own domain:
    if (mtmp.isshk AND inhishop) OR (mtmp.ispriest AND inhistemple):
        return false

    // Scare monster scroll on ground: always works (for eligible monsters)
    if sobj_at(SCR_SCARE_MONSTER, x, y):
        return true

    // Altar scares vampires
    if IS_ALTAR(levl[x][y]) AND (vampire OR vampshifter):
        return true

    // Elbereth check:
    ep = sengr_at("Elbereth", x, y, strict=true)
    return ep != null
        AND (hero_at(x,y)
             OR (Displaced AND mtmp.mux==x AND mtmp.muy==y)
             OR (ep.guardobjects AND vobj_at(x,y)))
        AND !mtmp.isshk
        AND !mtmp.isgd           // vault guard
        AND mtmp.mcansee          // blind monsters ignore it
        AND !mtmp.mpeaceful       // peaceful monsters ignore it
        AND mtmp.data != PM_MINOTAUR
        AND !Inhell               // doesn't work in Gehennom
        AND !In_endgame           // doesn't work on Astral/Elemental Planes
```

**`sengr_at` strict mode**: The engraving text must be *exactly* "Elbereth" (case-insensitive), not a substring. Headstones are excluded. The engraving's `engr_time` must be `<= moves` (it becomes active only after the turn it was created).

**`guardobjects` flag**: Set on level-generation Elbereths (`in_mklev`). These deter monsters when *any objects* are on the same square (even if hero isn't there). Player-engraved Elbereths only work when the hero (or displaced image) is on the square.

### 8.3 Degradation

Engravings degrade in three ways:

**1. Walking over (`maybe_smudge_engr`)**: Called after `domove()` succeeds. If `can_reach_floor(true)`, wipes `rnd(5)` characters (1-5) from engravings at both the source and destination squares.

**2. Standing idle per turn (`u_wipe_engr`)**: Once per turn in moveloop, there is a `1 / (40 + ACURR(A_DEX) * 3)` chance of wiping `rnd(3)` characters (1-3).

**3. `wipe_engr_at` actual removal logic**:

```pseudocode
fn wipe_engr_at(x, y, cnt, magical):
    ep = engr_at(x, y)
    if ep AND ep.engr_type != HEADSTONE AND !ep.nowipeout:
        if ep.engr_type == BURN AND !is_ice(x,y) AND !(magical AND !rn2(2)):
            return  // burned engravings resist non-magical wiping (except on ice)
        if ep.engr_type != DUST AND ep.engr_type != ENGR_BLOOD:
            // Hard engravings (ENGRAVE, MARK) resist:
            cnt = rn2(1 + 50 / (cnt + 1)) ? 0 : 1
        wipeout_text(ep.engr_txt, cnt, 0)
        // Remove leading spaces; delete engraving if empty
```

For DUST/BLOOD: full `cnt` characters are wiped. For ENGRAVE/MARK: only 1 character is wiped, and only with probability `1 / (1 + 50/(cnt+1))`. For BURN: almost immune to non-magical wiping.

### 8.4 Hypocrisy Penalty

Attacking a monster while standing on Elbereth (if that monster would be affected by Elbereth) incurs an alignment penalty and may anger your god. Checked via `sengr_at("Elbereth", u.ux, u.uy, TRUE)` in `mon.c:anger_guards()`.

---

## 9. Encumbrance Levels and Effects

### 9.1 Capacity Calculation

```pseudocode
fn weight_cap() -> int:
    carrcap = WT_WEIGHTCAP_STRCON * (ACURRSTR + ACURR(A_CON)) + WT_WEIGHTCAP_SPARE
    //      = 25 * (STR + CON) + 50

    if Upolyd:
        if youmonst.data.mlet == S_NYMPH: carrcap = MAX_CARR_CAP  // 1000
        else if !youmonst.data.cwt: carrcap = carrcap * youmonst.data.msize / MZ_HUMAN
        else if !strongmonst OR (strongmonst AND cwt > WT_HUMAN):
            carrcap = carrcap * youmonst.data.cwt / WT_HUMAN  // 1450

    if Levitation OR Is_airlevel OR (riding AND strongmonst(steed)):
        carrcap = MAX_CARR_CAP   // 1000
    else:
        carrcap = min(carrcap, MAX_CARR_CAP)
        if !Flying:
            if wounded_left_leg:  carrcap -= WT_WOUNDEDLEG_REDUCT  // 100
            if wounded_right_leg: carrcap -= WT_WOUNDEDLEG_REDUCT  // 100

    return max(carrcap, 1)

fn inv_weight() -> int:
    // Returns (total_inventory_weight - weight_cap)
    // Negative means under capacity
    wt = sum of all inventory weights
         // coins: (quan + 50) / 100 per stack
         // boulders: free weight for giants (throws_rocks)
    wc = weight_cap()
    return wt - wc

fn calc_capacity(xtra_wt) -> EncumbranceLevel:
    wt = inv_weight() + xtra_wt
    if wt <= 0: return UNENCUMBERED     // 0
    if wc <= 1: return OVERLOADED       // 5
    cap = (wt * 2 / wc) + 1
    return min(cap, OVERLOADED)          // clamp to 1..5

fn near_capacity() -> EncumbranceLevel:
    return calc_capacity(0)
```

### 9.2 Encumbrance Levels

| Level | Value | Name | `wt` Range (where wt = inv_weight(), wc = weight_cap()) |
|-------|-------|------|----------------------------------------------------------|
| `UNENCUMBERED` | 0 | Unencumbered | wt <= 0 |
| `SLT_ENCUMBER` | 1 | Burdened | 0 < wt, cap formula = 1 when wt <= wc/2 |
| `MOD_ENCUMBER` | 2 | Stressed | cap formula = 2 when wc/2 < wt <= wc |
| `HVY_ENCUMBER` | 3 | Strained | cap formula = 3 when wc < wt <= 3*wc/2 |
| `EXT_ENCUMBER` | 4 | Overtaxed | cap formula = 4 when 3*wc/2 < wt <= 2*wc |
| `OVERLOADED` | 5 | Overloaded | wt > 2*wc (or wc <= 1) |

(Integer division: `cap = (wt * 2 / wc) + 1`)

### 9.3 Encumbrance Effects Summary

| Level | Speed Penalty | HP Regen | Pw Regen | Climb Stairs Up | Can Move? | HP Drain |
|-------|--------------|----------|----------|-----------------|-----------|----------|
| Unencumbered | None | Normal | Normal | Yes | Yes | No |
| Burdened | -25% | Normal | Normal | No | Yes | No |
| Stressed | -50% | Only if !moved | Only if !moved | No | Yes | No |
| Strained | -75% | Only if !moved | Blocked | No | Yes* | Every 30 turns if moved |
| Overtaxed | -87.5% | Only if !moved | Blocked | No | Yes* | Every 10 turns if moved |
| Overloaded | N/A | Only if !moved | Blocked | No | **No** | N/A (can't move) |

*\* At Strained/Overtaxed with `hp < 10` (or `mh < 5` poly'd) AND `hp != hpmax`, hero also cannot move (`carrying_too_much()`).*

**Climbing stairs up**: Requires `near_capacity() <= SLT_ENCUMBER` (i.e., at most Burdened). Otherwise: "Your load is too heavy to climb the stairs."

**HP drain from overexertion** (`overexert_hp`):
- If `hp > 1`: lose 1 hp
- If `hp == 1`: "You pass out from exertion!" -- fall asleep for 10 turns, exercise CON down

**Pw regen blocked**: When `wtcap >= MOD_ENCUMBER`, power regeneration is skipped (the modulus check `moves % period == 0` is within the `wtcap < MOD_ENCUMBER` condition).

**HP regen reduced**: `encumbrance_ok = (wtcap < MOD_ENCUMBER || !u.umoved)`. At Stressed or above, HP only regenerates when the hero did not move on that turn (or has Regeneration/Sleepy).

---

## 10. Travel Command and Pathfinding

### 10.1 Overview

The `_` (travel) command sets `context.travel = 1` and a destination `(u.tx, u.ty)`. Each step, `findtravelpath()` is called to compute the next move direction.

`context.run = 8` during travel (distinct from running modes 1-7).

### 10.2 Pathfinding Algorithm (`findtravelpath`)

Three modes:
- `TRAVP_TRAVEL (0)`: Normal pathfinding
- `TRAVP_GUESS (1)`: When direct path fails, find closest reachable point
- `TRAVP_VALID (2)`: Validate a travel destination

**Algorithm**: BFS (breadth-first search) from destination back to hero position.

```pseudocode
fn findtravelpath(mode):
    // Special case: adjacent destination
    if (TRAVEL or VALID) AND travel1 AND next2u(tx,ty) AND crawl_destination(tx,ty):
        if test_move(ux, uy, tx-ux, ty-uy, TEST_MOVE):
            u.dx = tx - ux; u.dy = ty - uy
            return true

    // BFS from destination (tx,ty) toward hero (ux,uy)
    // For GUESS mode: swap roles (BFS from hero toward dest to find couldsee squares)
    dirmax = NODIAG(u.umonnum) ? 4 : 8

    travel[COLNO][ROWNO] = 0   // visited markers
    queue = [(tx, ty)]
    radius = 1

    while queue not empty:
        next_queue = []
        for each (x, y) in queue:
            for each direction dir (0..dirmax-1):
                (nx, ny) = (x + xdir[dir], y + ydir[dir])
                if !isok(nx, ny): continue
                if GUESS mode AND !couldsee(nx, ny): continue

                // Penalize obstacles (closed doors, boulders, traps)
                // by re-enqueuing without advancing, effectively adding 3 cost
                if closed_door(x,y) OR (boulder(x,y) AND !could_move_onto)
                   OR test_move(x,y,nx-x,ny-y, TEST_TRAP):
                    if travel[x][y] > radius - 3:
                        re-enqueue (x,y)
                        continue

                if test_move(x, y, nx-x, ny-y, TEST_TRAV)
                   AND (seenv OR (!Blind AND couldsee)):
                    if (nx, ny) == (ux, uy):
                        // Found path! Set u.dx = x - ux, u.dy = y - uy
                        // Track visited in travelmap to detect loops
                        return true
                    else if !travel[nx][ny]:
                        next_queue.append((nx, ny))
                        travel[nx][ny] = radius

        queue = next_queue
        radius++

    // If GUESS mode: find closest reachable couldsee square to target
    // Pick by distmin, tiebreak by dist2, then by travel cost (lower better)
    // Re-run as TRAVEL mode toward that guessed target

    return false
```

### 10.3 Travel Stops

Travel automatically stops when:
- Hero reaches destination
- A hostile monster becomes visible nearby (via `lookaround()`)
- Hero enters a door, furniture, or obstructed square (when `run < 8`)
- `travelmap` detects visiting the same square twice ("You stop, unsure which way to go.")
- An interrupted action occurs (trap, etc.)

### 10.4 Travel and Traps/Liquid

During pathfinding (`TEST_TRAV` / `TEST_TRAP` modes), travel avoids:
- Seen traps (except `VIBRATING_SQUARE`)
- Known pools/lava (unless flying/levitating/water-walking/lava-walking)
- Boulders in Sokoban (never path through)
- Waterwalls and lavawalls (always avoid, even if flying)

---

## 11. Running Modes (`context.run`)

### 11.1 Run Values

The `context.run` field controls automatic repeated movement. Different input methods set different values:

| Value | Name | Input Method | Description |
|-------|------|-------------|-------------|
| 0 | Walk | Normal direction key | Single step; no automatic continuation |
| 1 | Run | Shift + direction (or `do_run_*`) | Follow corridors, stop at branches/interesting features |
| 2 | Rush prefix | `G` prefix (or `do_rush`) | Go until something interesting, stop at corridor branch |
| 3 | Rush | Shift-direction rush (`do_rush_*`) | Same as 2, but directly with direction |
| 8 | Travel | `_` command (`findtravelpath`) | BFS pathfinding to destination |

Note: Values 4-7 are not used in current NetHack 3.7.

### 11.2 Stopping Criteria (`lookaround()`)

`lookaround()` is called each step during running/rushing/travel. Behavior differs by `context.run` value:

- **run=0**: `lookaround()` returns immediately (not running)
- **run=1** (corridor run): Follows corridors, turns at bends (up to two consecutive turns). Stops when:
  - A hostile visible monster is in front (blocks path)
  - A corridor branches (multiple exits)
  - Treats closed doors as corridor segments
  - Treats objects/traps/stairs as corridor segments (does not stop for them)
- **run=2** (rush prefix): Stops when:
  - A hostile visible monster is seen (any adjacent square, not just in front; unless traveling)
  - Corridor widens (corrct > 1)
  - Closed door in front (not diagonal)
  - Objects, traps, stairs, or other interesting features in front
- **run=3** (rush): Like run=1 for corridor-following (turns at bends), but stops for interesting features like run=2
- **run=8** (travel): Like run=1/3 for corridor following. Ignores objects/traps/stairs that are not in front. Stops for visible hostile monsters. Uses `travelmap` to detect loops.

All running modes stop for:
- Visible traps in front (via `avoid_moving_on_trap()`)
- Liquid (pool/lava) in front (via `avoid_moving_on_liquid()`)
- Grid bug diagonal movement

### 11.3 `test_move()` Function

`test_move(ux, uy, dx, dy, mode)` checks whether moving from `(ux,uy)` by `(dx,dy)` is valid. The `mode` parameter controls behavior:

| Mode | Value | Purpose |
|------|-------|---------|
| `DO_MOVE` | 0 | Actually performing the move; generates messages, triggers effects |
| `TEST_MOVE` | 1 | Test whether a normal move is valid; no side effects |
| `TEST_TRAV` | 2 | Test for travel pathfinding; more permissive (allows closed doors, boulders) |
| `TEST_TRAP` | 3 | Check if a future travel location is a trap; returns TRUE if trap present |

Key checks in `test_move()`:
- Out of bounds => FALSE
- Obstructed terrain (walls, iron bars) => FALSE unless `Passes_walls` or can tunnel/eat rock
- Closed/locked doors => FALSE for `TEST_MOVE` (unless `autoopen`); `DO_MOVE` may auto-open or auto-dig
- Boulders => handled via `moverock()` in `DO_MOVE`; FALSE for `TEST_MOVE` if not squeezable; special Sokoban handling for `TEST_TRAV`
- Travel mode (`run=8`): checks for seen traps, known liquid, waterwalls/lavawalls
- Diagonal restrictions: door diagonal rule, tight squeeze check

---

## 疑似 Bug

1. **[疑似 bug] `u_calc_moveamt` OVERLOADED case**: The switch statement has no case for `OVERLOADED` (value 5); it falls through to `default: break`, applying no speed penalty. However, the hero can never actually move when overloaded because `carrying_too_much()` blocks movement at the top of `domove_core()`. So this is harmless, but the speed reduction table is incomplete -- if `carrying_too_much()` were ever bypassed, the hero would get full speed while overloaded.

2. **[疑似 bug] Encumbrance threshold: `inv_weight() == 0` is Burdened**: The `calc_capacity()` formula `cap = (wt * 2 / wc) + 1` with `wt = 0` returns `UNENCUMBERED` (caught by the `wt <= 0` early return). But `inv_weight()` returns `total_weight - weight_cap()`, so carrying *exactly* your capacity yields `wt = 0` which is `UNENCUMBERED`. Carrying capacity + 1 yields `wt = 1`, so `cap = (1*2/wc)+1 = 1` = `SLT_ENCUMBER`. This means the boundary is exclusive (weight must *exceed* capacity to become Burdened), which is internally consistent but may surprise Rust reimplementers who assume inclusive thresholds.

3. **[疑似 bug] Steed speed ignores hero speed bonuses**: In `u_calc_moveamt()`, when riding (`u.usteed && u.umoved`), the code uses `mcalcmove(steed, true)` and completely skips the hero's `Very_fast`/`Fast` bonus. The comment says "your speed doesn't augment steed's speed" -- this is intentional design, but notably speed boots become useless while mounted, which may surprise players.

4. **[疑似 bug] Air turbulence wastes resting moves**: On the air level, `air_turbulence()` consumes the hero's move 75% of the time. It fires before checking whether `u.dx == 0 && u.dy == 0` (resting/searching), so resting on the air level without flying/levitation is very slow -- the hero effectively loses 75% of their rest turns to turbulence.

5. **[疑似 bug] Double engraving wipe on movement**: `maybe_smudge_engr()` wipes engravings at both the source AND destination squares when moving. If the hero moves from one Elbereth to an adjacent Elbereth, both get degraded. More notably, the destination wipe means a protective engraving starts degrading the moment the hero arrives, before getting any monster-turn protection from it.

6. **[疑似 bug] `water_turbulence` same message for different thresholds**: When underwater and too encumbered to climb out, the message is always "You are carrying too much to climb out of the water" regardless of whether the hero is `Swimming` (threshold MOD_ENCUMBER) or not (threshold SLT_ENCUMBER). The different thresholds are correct but the feedback doesn't reflect which standard applies.

---

## 测试向量

### Movement Point Calculation

| # | Form | mmove | Speed | Encumbrance | Riding | Expected moveamt per turn |
|---|------|-------|-------|-------------|--------|--------------------------|
| 1 | Human | 12 | None | Unencumbered | No | 12 |
| 2 | Human | 12 | Very_fast (rn2(3)!=0) | Unencumbered | No | 12 + 12 = **24** |
| 3 | Human | 12 | Very_fast (rn2(3)==0) | Unencumbered | No | **12** (no bonus this turn) |
| 4 | Human | 12 | Fast (rn2(3)==0) | Unencumbered | No | 12 + 12 = **24** |
| 5 | Human | 12 | Fast (rn2(3)!=0) | Unencumbered | No | **12** (no bonus) |
| 6 | Human | 12 | None | Burdened (SLT) | No | 12 - 12/4 = 12 - 3 = **9** |
| 7 | Human | 12 | None | Stressed (MOD) | No | 12 - 12/2 = 12 - 6 = **6** |
| 8 | Human | 12 | None | Strained (HVY) | No | 12 - (12*3)/4 = 12 - 9 = **3** |
| 9 | Human | 12 | None | Overtaxed (EXT) | No | 12 - (12*7)/8 = 12 - 10 = **2** |
| 10 | Human | 12 | Very_fast + bonus | Stressed (MOD) | No | (12+12)*50% = 24 - 12 = **12** |
| 11 | Steam vortex | 24 | None | Unencumbered | No | **24** |
| 12 | Grid bug | 12 | None | Unencumbered | No | **12** (4 dirs only, same speed) |

### Boundary Conditions

| # | Scenario | Expected |
|---|----------|----------|
| BC1 | `inv_weight() == 0` exactly (inventory weight == weight_cap) | `wt <= 0` path => **UNENCUMBERED** |
| BC2 | `inv_weight() == 1` (1 unit over capacity) | `cap = (1*2/wc)+1`; for wc=950: `(2/950)+1 = 0+1 = 1` => **SLT_ENCUMBER (Burdened)** |
| BC3 | `weight_cap() returns 1` (minimum), `inv_weight() == 1` | `wc <= 1` early return => **OVERLOADED** |
| BC4 | `moves == 999_999_999` | Next increment => `moves >= 1_000_000_000` => "The dungeon capitulates." => game ends (ESCAPED) |
| BC5 | Hero DEX 3, standing Elbereth idle degradation chance | `1 / (40 + 3*3) = 1/49` ~= **2.04% per turn** |
| BC6 | Hero DEX 25, standing Elbereth idle degradation chance | `1 / (40 + 25*3) = 1/115` ~= **0.87% per turn** |

### Search Detection

| # | fund | Target | rnl argument | Base chance (Luck=0) |
|---|------|--------|-------------|---------------------|
| S1 | 0 | Secret door (SDOOR) | rnl(7) == 0 | 1/7 = **14.3%** |
| S2 | 2 (lenses only) | Secret door | rnl(5) == 0 | 1/5 = **20.0%** |
| S3 | 5 (max fund) | Secret door | rnl(2) == 0 | 1/2 = **50.0%** |
| S4 | 0 | Hidden trap | !rnl(8) | 1/8 = **12.5%** |
| S5 | 5 | Hidden trap | !rnl(8) | 1/8 = **12.5%** (fund does NOT affect trap detection) |
| S6 | 0, Luck=13 | Secret door | rnl(7) with adj=4 | Significantly better than 14.3% (biased toward 0) |

### Door Interaction

| # | Action | STR+DEX+CON | Formula | Approx Success Rate |
|---|--------|-------------|---------|-------------------|
| D1 | Open door | sum=30 | rnl(20) < 30/3=10 | ~50% base |
| D2 | Open door | sum=54 (max realistic) | rnl(20) < 54/3=18 | ~90% base |
| D3 | Close door | sum=30 | rn2(25) < 30/3=10 | 10/25 = **40%** |
| D4 | Close door while riding | N/A | auto-success | **100%** |
| D5 | Kick door | avrg_attrib=12, non-martial | rnl(35) < 12 | ~34% base |
| D6 | Kick door, giant polyform | N/A | doorbuster=true | **100%** |

### Encumbrance from Weight

| # | STR | CON | weight_cap | Inv Weight | inv_weight() | Expected Level |
|---|-----|-----|-----------|------------|-------------|---------------|
| E1 | 18 | 18 | 25*(18+18)+50=950 | 0 | -950 | **Unencumbered** |
| E2 | 18 | 18 | 950 | 950 | 0 | **Unencumbered** (boundary: exactly at cap) |
| E3 | 18 | 18 | 950 | 951 | 1 | **Burdened** (cap=(2/950)+1=1) |
| E4 | 18 | 18 | 950 | 1425 | 475 | **Stressed** (cap=(950/950)+1=2) |
| E5 | 18 | 18 | 950 | 1900 | 950 | **Strained** (wt=950, cap=(950*2/950)+1=2+1=3=HVY_ENCUMBER) |
| E5b | 18 | 18 | 950 | 2375 | 1425 | **Overtaxed** (wt=1425, cap=(1425*2/950)+1=(2850/950)+1=3+1=4=EXT_ENCUMBER) |
| E6 | 3 | 3 | 25*(3+3)+50=200 | 600 | 400 | **OVERLOADED** (cap=(400*2/200)+1=5) |
