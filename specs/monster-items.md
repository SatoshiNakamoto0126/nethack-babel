# Monster Items Mechanism Specification

> Source: `src/muse.c`, `src/weapon.c`, `src/worn.c`, `src/mthrowu.c`, `include/monst.h`

## 1. Overview

Monsters use items via a three-phase decision system called every monster turn from `monmove.c` and `mhitu.c`:

1. **Defensive phase** (`find_defensive` + `use_defensive`) -- checked during monster movement, before attacking.
2. **Miscellaneous phase** (`find_misc` + `use_misc`) -- checked only if no defensive item found.
3. **Offensive phase** (`find_offensive` + `use_offensive`) -- checked during attack resolution; if used, monster does NOT also melee.

Each phase populates a global `struct musable gm.m` with one chosen item and an action code. The `find_*` functions scan the monster's inventory and nearby terrain; the `use_*` functions execute the action.

```
struct musable {
    struct obj *offensive;
    struct obj *defensive;
    struct obj *misc;
    int has_offense, has_defense, has_misc;
};
```

---

## 2. Universal Restrictions

### 2.1 Monsters Excluded from All Item Use

| Condition | Check | Affects |
|---|---|---|
| Animal body | `is_animal(mdat)` | All three phases |
| Mindless | `mindless(mdat)` | All three phases |
| No hands | `nohands(mdat)` | Offensive, inventory items in defensive/misc |
| Ghost (letter S_GHOST) | `pm->mlet == S_GHOST` | `rnd_*_item()` only |
| Kop (letter S_KOP) | `pm->mlet == S_KOP` | `rnd_*_item()` only |
| Self-destruct attack | `attacktype(pm, AT_EXPL)` | `rnd_*_item()` only |

### 2.2 Additional Constraints

- Scrolls require `mtmp->mcansee && haseyes(mtmp->data)` to read.
- Monsters **cannot** recognize cursed items (comment in source: "Monsters shouldn't recognize cursed items").
- Monsters **know** when wands have 0 charges (will not select empty wands).
- Confused monsters do not know to avoid reading scrolls.
- **Peaceful monsters** only use healing potions defensively (no teleportation or fleeing).

### 2.3 Precheck (cursed wand backfire, potion occupants)

Before any item use, `precheck()` runs:

- **Milky potion**: if `!(mvitals[PM_GHOST].mvflags & G_GONE)`, chance `1 / POTION_OCCUPANT_CHANCE(born)` where `POTION_OCCUPANT_CHANCE(n) = 13 + 2*n`. Ghost emerges, monster paralyzed 3 turns.
- **Smoky potion**: same formula with `PM_DJINNI`. Djinni emerges; 50% peaceful, 50% vanishes.
- **Cursed wand**: `1/WAND_BACKFIRE_CHANCE` = `1/100` chance of backfire. Damage = `d(spe+2, 6)`. Can kill the monster. On backfire, all three `has_defense/offense/misc` reset to 0.

---

## 3. Defensive Item Use

### 3.1 When Defensive Search Triggers

`find_defensive(mtmp, tryescape)` is called:
- During normal movement with `tryescape=FALSE`
- When monster has no valid moves with `tryescape=TRUE`

Early exit conditions:
- `is_animal || mindless` => FALSE
- `tryescape=FALSE` and `dist2(mon, target) > 25` => FALSE (won't defend if hero is far)
- `tryescape=TRUE` on Fort Knox and monster is next to another monster but not next to hero => FALSE
- Monster is engulfing hero => FALSE

### 3.2 Healing Threshold

```
fraction = (u.ulevel < 10) ? 5 : (u.ulevel < 14) ? 4 : 3;
// Monster seeks healing if:
//   mhp < mhpmax AND (mhp < 10 OR mhp * fraction < mhpmax)
```

In pseudocode:
```
should_heal =
    mhp < mhpmax
    AND (mhp < 10 OR mhp / mhpmax < 1/fraction)
```

| Hero Level | fraction | HP threshold |
|---|---|---|
| 1-9 | 5 | hp < 20% of max (or hp < 10) |
| 10-13 | 4 | hp < 25% of max (or hp < 10) |
| 14+ | 3 | hp < 33% of max (or hp < 10) |

### 3.3 Defensive Priority Order

The defensive search has a **fixed priority** implemented via code flow order. Higher items are checked/used first:

1. **Unicorn horn** (cures confusion, stun, blindness) -- checked first if `mconf || mstun || !mcansee`. Skips cursed horns. Unicorns/Ki-rin use innate horn (no object needed).

2. **Lizard corpse/tin** (cures confusion, stun) -- checked if `mconf || mstun`. Lizard tin requires `mcould_eat_tin()` and succeeds with probability 2/3.

3. **Healing potions** (when blind) -- if `!mcansee && !nohands && not Pestilence`, search for pot of full/extra/healing.

4. **Wand of undead turning** (against cockatrice-corpse-wielding hero) -- if hero wields a cockatrice corpse, monster is not poly_when_stoned, not stone-resistant, and `lined_up()`.

5. **Healing decision gate** -- if `tryescape=FALSE`, check healing threshold (section 3.2). Peaceful monsters only try healing here, then return.

6. **Physical escape** (stairs, ladders, trap doors, teleport traps) -- checked for the spot monster is standing on and 8 adjacent squares. Priority: trap door > teleport trap. Stairs/ladders checked directly on monster's square.

7. **Bugle** -- mercenary monsters wake sleeping soldiers within 3-square radius.

8. **Inventory scan** (wands, scrolls, potions, create monster) -- iterates inventory with `nomore()` macro and `!rn2(3)` early-break randomization:

   Priority within scan (first match checked first, but randomization can cause later items to override):
   - **Wand of digging** (escape downward) -- breaks the scan immediately once found. Excluded if ANY of these conditions hold:
     - Monster is stuck (e.g. held by hero)
     - There is an existing trap at monster's position (after pit/web/bear trap filtering)
     - Monster is shopkeeper, vault guard, or priest
     - Monster is a floater (`is_floater`)
     - In Sokoban (`Sokoban`)
     - Floor is non-diggable (`W_NONDIGGABLE`)
     - On the bottom level (`Is_botlevel`) or in the endgame (`In_endgame`)
     - Standing on ice, pool, or lava
     - Vlad in Vlad's Tower (`is_Vlad(mtmp) && In_V_tower`)
   - **Wand of teleportation** (self or at hero depending on Amulet possession)
   - **Scroll of teleportation** -- requires `mcansee && haseyes`
   - **Potion of full healing** (Pestilence uses potion of sickness instead)
   - **Potion of extra healing**
   - **Wand of create monster**
   - **Potion of healing**
   - **Scroll of create monster**

   The `!rn2(3)` check means: once any defensive item is found, there is a 2/3 chance per subsequent item that the scan stops early. [疑似 bug] This means a monster with both wand of teleportation and potion of full healing will non-deterministically choose between them, even though the code structure suggests teleportation should have priority.

### 3.4 Flee Timer on Defensive Use

When a monster uses a defensive item to flee:
```
fleetim = !mtmp->mflee ? (33 - (30 * mtmp->mhp / mtmp->mhpmax)) : 0;
```
At 100% hp => 3 turns; at 50% hp => 18 turns; at 10% hp => 30 turns. The Wizard of Yendor never flees.

### 3.5 Healing Potion Effects

| Potion | Heal Amount | Max HP Increase | Cures Blindness |
|---|---|---|---|
| Healing | `d(6 + 2*bcsign, 4)` | +1 | if not cursed |
| Extra Healing | `d(6 + 2*bcsign, 8)` | +2 (blessed: +5) | always |
| Full Healing | `mhpmax` (full) | +4 (blessed: +8) | always (not for pot_sickness) |
| Sickness (Pestilence) | `mhpmax` (full) | +4 (blessed: +8) | no |

Where `bcsign` = +1 blessed, 0 uncursed, -1 cursed. Note: `healmon(mtmp, amount, maxhp_increase)`.

---

## 4. Offensive Item Use

### 4.1 When Offensive Search Triggers

`find_offensive(mtmp)` is called during `mattacku()` (monster attack on hero), before melee. If an offensive item is used, the monster does **not** also melee that turn.

Preconditions:
- Not peaceful, not animal, not mindless, not nohands
- Hero not swallowed
- Not in hero's sanctuary
- `lined_up(mtmp)` returns TRUE (orthogonal or diagonal line of sight, distance < `BOLT_LIM`)
- Special: monsters with `AD_HEAL` damage type (e.g. nurses, checked via `dmgtype(mdat, AD_HEAL)`) won't use items if hero is naked (`!uwep && !uarmu && !uarm && !uarmh && !uarms && !uarmg && !uarmc && !uarmf`)

### 4.2 Reflection Awareness

```
reflection_skip = (m_seenres(mtmp, M_SEEN_REFL) || monnear(mtmp, mtmp->mux, mtmp->muy));
```

If the monster has seen the hero reflect, OR the monster is adjacent to where it thinks the hero is, it **skips** all beam weapons (death, sleep, fire, cold, lightning, magic missile, fire/frost horn). It can still use striking, teleportation, undead turning, and thrown items.

### 4.3 Offensive Item Priority (Scan Order)

Items checked with `nomore()` -- effectively last-found-wins within the scan, subject to `reflection_skip`:

**Beam weapons** (skipped if `reflection_skip`):
1. Wand of death -- unless hero has M_SEEN_MAGR (antimagic)
2. Wand of sleep -- unless hero is already paralyzed (`multi >= 0` check) or M_SEEN_SLEEP
3. Wand of fire -- unless M_SEEN_FIRE
4. Fire horn -- same + `can_blow(mtmp)`
5. Wand of cold -- unless M_SEEN_COLD
6. Frost horn -- same + `can_blow(mtmp)`
7. Wand of lightning -- unless M_SEEN_ELEC
8. Wand of magic missile -- unless M_SEEN_MAGR

**Non-beam weapons** (always checked):
9. Wand of undead turning -- if hero carries corpse or corpse on floor in line
10. Wand of striking -- unless M_SEEN_MAGR
11. Wand of teleportation -- unless hero has Teleport_control; only used if hero is on scary square, behind chokepoint with friends, on desirable item pile, or on stairs
12. Potion of paralysis -- unless hero already paralyzed
13. Potion of blindness -- unless monster has gaze attack
14. Potion of confusion
15. Potion of sleeping -- unless M_SEEN_SLEEP
16. Potion of acid -- unless M_SEEN_ACID
17. Scroll of earth -- requires `dist2 <= 2` (adjacent), `mcansee && haseyes`, not Rogue level, protected if wearing hard helmet or amorphous/passes_walls/noncorporeal/unsolid, otherwise 1/10 chance
18. Expensive camera -- requires `dist2 <= 2`, hero not blind and not resistant to blindness (or hates light), spe > 0, 1/6 chance

### 4.4 Beam Wand Execution

```
buzz(BZ_M_WAND(BZ_OFS_WAN(otyp)),
     (otyp == WAN_MAGIC_MISSILE) ? 2 : 6,
     mx, my, sgn(mux - mx), sgn(muy - my));
```

Magic missile ray width = 2, other beam wands = 6. **Horns are different**: fire horn and frost horn use `rn1(6, 6)` for ray width (range 6-11), not a fixed width.

### 4.5 Thrown Potions

Offensive potions (paralysis, blindness, confusion, sleeping, acid) are **thrown**, not drunk:
```
m_throw(mtmp, mx, my, sgn(mux - mx), sgn(muy - my),
        distmin(mx, my, mux, muy), otmp);
```

### 4.6 Scroll of Earth Mechanics

- Drops boulders on 3x3 area centered on monster.
- Blessed: boulders on monster's square, not on surrounding squares.
- Cursed: boulders on surrounding squares, not on monster's square.
- Confused: random boulder types (acid blob instead of boulder).
- Monster's square: `!is_blessed`, surrounding: `!is_cursed`.
- Hero hit if `distmin == 1` and `!is_cursed`.

---

## 5. Miscellaneous Item Use

### 5.1 When Misc Search Triggers

`find_misc(mtmp)` is called only if `find_defensive()` returned FALSE. Distance gate: `dist2(mon, target) > 36` => skip.

### 5.2 Polymorph Trap

Low-difficulty monsters (`difficulty < 6`) that are not shapechangers can deliberately jump onto a nearby polymorph trap.

### 5.3 Misc Item Priority (Inventory Scan)

1. **Potion of gain level** -- not if cursed (unless not shopkeeper/guard/priest)
2. **Bullwhip** (disarm hero) -- must be wielded, 1/5 chance per turn, hero adjacent, hero wielding a droppable weapon
3. **Wand of make invisible** -- if not already invisible, not invis_blocked; peaceful only if hero has See_invisible; not if has gaze attack (unless cancelled)
4. **Potion of invisibility** -- same conditions
5. **Wand of speed monster** -- if not already MFAST, not vault guard
6. **Potion of speed** -- same conditions
7. **Wand of polymorph** -- if not shapechanger, difficulty < 6
8. **Potion of polymorph** -- same conditions
9. **Container (bag)** -- if not bag of tricks, not Schroedinger's box, not locked, not cursed mbag, has contents, 1/5 chance; monster takes items out with weighted distribution based on `rn2(10)`:
     - Cases 0-3 (40%): 1 item
     - Cases 4-6 (30%): 2 items
     - Cases 7-8 (20%): 3 items
     - Case 9 (10%): 4 items
     Additionally, each extraction has a `!rn2(nitems + 1)` early-stop throttle where `nitems` is the number of items remaining in the container.

### 5.4 Polymorph Target Selection

When a monster polymorphs itself:
```
fn muse_newcham_mon(mon):
    if wearing dragon scales => polymorph into corresponding dragon
    if wearing dragon scale mail => polymorph into corresponding dragon
    else => rndmonst() (random monster appropriate to level)
```

### 5.5 Bullwhip Disarm Mechanics

```
where_to = rn2(4)   // 0: whip slips free, 1: floor under mon, 2: floor under hero, 3: mon's inventory
```
- If weapon is welded: `where_to = 0` (always fails)
- If `where_to == 3` and monster hates silver and weapon is silver: redirect to `where_to = 2`
- Heavy iron ball: always fails

---

## 6. Monster Equipment (Armor)

### 6.1 Armor Wearing (`m_dowear`)

Monsters equip armor via `m_dowear()`, called at creation and when picking up items.

**Cannot wear armor at all:**
- `verysmall(mdat)` -- too small
- `nohands(mdat)` -- no hands
- `is_animal(mdat)` -- animal form
- `mindless(mdat)` -- except mummies and skeletons during creation

**Armor slots checked in order:**
1. Amulet (W_AMUL) -- only life saving, reflection, or guarding
2. Shirt (W_ARMU) -- only if no suit already worn
3. Cloak (W_ARMC) -- large/huge monsters only wear mummy wrapping; invisible monsters avoid mummy wrapping (blocks invisibility) unless at creation
4. Helmet (W_ARMH) -- horned monsters only wear flimsy helmets; priests/minions skip helm of opposite alignment
5. Shield (W_ARMS) -- only if not wielding two-handed weapon
6. Gloves (W_ARMG)
7. Boots (W_ARMF) -- not for slithy or centaur monsters
8. Suit (W_ARM) -- requires `!cantweararm(mdat)`, with racial exception for small player races

**Selection criterion:**
```
ARM_BONUS(obj) + extra_pref(mon, obj) > ARM_BONUS(current) + extra_pref(mon, current)
```
Where `extra_pref` adds +20 for speed boots if monster is not already MFAST.

Monsters know `obj->spe` for comparison purposes (source comment acknowledges this is not ideal).

### 6.2 Amulet Priority

```
Life Saving > Reflection > Guarding
```
Once wearing life saving or reflection, monster will not switch. Guarding can be replaced by life saving or reflection.

### 6.3 Armor Wearing Delay

Non-creation equipping costs time:
```
m_delay = objects[best->otyp].oc_delay;
if replacing old: m_delay += objects[old->otyp].oc_delay;
if (suit or shirt) and wearing cloak: m_delay += 2;
```
Monster is frozen for `m_delay` turns.

---

## 7. Monster Weapon Selection

### 7.1 Hand-to-Hand Weapon (`select_hwep`)

Priority:
1. **Artifacts first** -- any weapon-class artifact the monster can touch, checking `touch_artifact()` and bimanual constraints.
2. **Race-specific**: giants prefer clubs; Balrogs prefer bullwhip (if hero is wielding something).
3. **Static preference list** (checked in order):
   ```
   CORPSE (cockatrice, requires gloves or stone-resist),
   TSURUGI, RUNESWORD, DWARVISH_MATTOCK, TWO_HANDED_SWORD, BATTLE_AXE,
   KATANA, UNICORN_HORN, CRYSKNIFE, TRIDENT, LONG_SWORD, ELVEN_BROADSWORD,
   BROADSWORD, SCIMITAR, SILVER_SABER, MORNING_STAR, ELVEN_SHORT_SWORD,
   DWARVISH_SHORT_SWORD, SHORT_SWORD, ORCISH_SHORT_SWORD, SILVER_MACE, MACE,
   AXE, DWARVISH_SPEAR, SILVER_SPEAR, ELVEN_SPEAR, SPEAR, ORCISH_SPEAR, FLAIL,
   BULLWHIP, QUARTERSTAFF, JAVELIN, AKLYS, CLUB, PICK_AXE, RUBBER_HOSE,
   WAR_HAMMER, SILVER_DAGGER, ELVEN_DAGGER, DAGGER, ORCISH_DAGGER, ATHAME,
   SCALPEL, KNIFE, WORM_TOOTH
   ```

Constraints:
- Two-handed weapons (`oc_bimanual`) require `strongmonst` AND not wearing a shield.
- Silver weapons skipped if `mon_hates_silver(mtmp)`.
- Cockatrice corpse requires gloves (`W_ARMG` worn) or stone resistance.

### 7.2 Ranged Weapon (`select_rwep`)

Priority:
1. **Cockatrice eggs** (always first)
2. **Cream pies** (Kops only)
3. **Boulders** (rock-throwing monsters only)
4. **Polearms** (if `dist2 <= 13`, i.e., within 5 squares range):
   ```
   HALBERD, BARDICHE, SPETUM, BILL_GUISARME, VOULGE, RANSEUR,
   GUISARME, GLAIVE, LUCERN_HAMMER, BEC_DE_CORBIN, FAUCHARD, PARTISAN, LANCE
   ```
   Snickersnee artifact takes highest polearm priority.
5. **Throw-and-return weapons**: AKLYS (range limit `BOLT_LIM/2` squared)
6. **Standard ranged list** (gems via sling checked before darts):
   ```
   DWARVISH_SPEAR, SILVER_SPEAR, ELVEN_SPEAR, SPEAR, ORCISH_SPEAR, JAVELIN,
   SHURIKEN, YA, SILVER_ARROW, ELVEN_ARROW, ARROW, ORCISH_ARROW,
   CROSSBOW_BOLT, SILVER_DAGGER, ELVEN_DAGGER, DAGGER, ORCISH_DAGGER, KNIFE,
   FLINT, ROCK, LOADSTONE, LUCKSTONE, DART, CREAM_PIE
   ```
   Ammo requires matching launcher (bow/sling/crossbow). Don't throw artifacts. Don't throw cursed wielded weapon.

### 7.3 Weapon Switch (`mon_wield_item`)

`weapon_check` flags trigger wielding:

| Flag | Action |
|---|---|
| `NO_WEAPON_WANTED` (0) | Do nothing |
| `NEED_WEAPON` (1) | Not used directly in switch; triggers on weapon loss |
| `NEED_RANGED_WEAPON` (2) | `select_rwep()` -> wield propellor |
| `NEED_HTH_WEAPON` (3) | `select_hwep()` |
| `NEED_PICK_AXE` (4) | Find pick-axe or dwarvish mattock |
| `NEED_AXE` (5) | Find battle axe or axe |
| `NEED_PICK_OR_AXE` (6) | Find any digging tool |

Wielding takes 1 turn. Cursed wielded weapons cannot be switched (monster sets `weapon_check = NO_WEAPON_WANTED`).

---

## 8. Item Generation for Monsters

Monsters receive items at creation via `rnd_defensive_item`, `rnd_offensive_item`, `rnd_misc_item`.

### 8.1 Defensive Item Generation

```
switch rn2(8 + (difficulty > 3) + (difficulty > 6) + (difficulty > 8)):
  // difficulty <= 3: rn2(8), cases 0-7
  // difficulty 4-6:  rn2(9), cases 0-8
  // difficulty 7-8:  rn2(10), cases 0-9
  // difficulty 9+:   rn2(11), cases 0-10
```

| Case | Item | Notes |
|---|---|---|
| 0, 1 | SCR_TELEPORTATION | |
| 2 | SCR_CREATE_MONSTER | |
| 3 | POT_HEALING | |
| 4 | POT_EXTRA_HEALING | |
| 5 | POT_FULL_HEALING (or POT_SICKNESS for Pestilence) | |
| 6, 9 | WAN_TELEPORTATION (1/3) or SCR_TELEPORTATION (2/3) | Retry if no-teleport level |
| 7 | WAN_DIGGING | Skip if floater/shk/guard/priest; retry in Sokoban 3/4 |
| 8, 10 | WAN_CREATE_MONSTER (1/3) or SCR_CREATE_MONSTER (2/3) | |

### 8.2 Offensive Item Generation

```
if difficulty > 7 && !rn2(35): WAN_DEATH  // ~2.86% chance
switch rn2(9 - (difficulty < 4) + 4 * (difficulty > 6)):
  // difficulty < 4:  rn2(8)
  // difficulty 4-6:  rn2(9)
  // difficulty > 6:  rn2(13)
```

| Case | Item |
|---|---|
| 0 | SCR_EARTH (if hard helmet/amorphous/etc) else fall through |
| 1 | WAN_STRIKING |
| 2 | POT_ACID |
| 3 | POT_CONFUSION |
| 4 | POT_BLINDNESS |
| 5 | POT_SLEEPING |
| 6 | POT_PARALYSIS |
| 7, 8 | WAN_MAGIC_MISSILE |
| 9 | WAN_SLEEP |
| 10 | WAN_FIRE |
| 11 | WAN_COLD |
| 12 | WAN_LIGHTNING |

### 8.3 Misc Item Generation

```
if difficulty < 6 && !rn2(30): POT_POLYMORPH (5/6) or WAN_POLYMORPH (1/6)
if !rn2(40) && !nonliving && !vampshifter: AMULET_OF_LIFE_SAVING
switch rn2(3):
  0: POT_SPEED (5/6) or WAN_SPEED_MONSTER (1/6) -- not vault guard
  1: POT_INVISIBILITY (5/6) or WAN_MAKE_INVISIBLE (1/6) -- peaceful only if See_invisible
  2: POT_GAIN_LEVEL
```

---

## 9. Counter-Petrification and Counter-Sliming

### 9.1 Unstone (`munstone`)

When a monster is being petrified, it scans inventory for:
- **Potion of acid** (always works)
- **Glob of green slime** (if slimeproof)
- **Lizard corpse**
- **Acidic corpse** (any `acidic(&mons[corpsenm])`)
- **Tin of lizard/acidic monster** (requires `mcould_eat_tin()`)

`mcould_eat_tin` requires: not animal, and having a tin opener OR dagger/knife in inventory (not necessarily wielded, but cursed wielded weapon blocks other items).

Side effects of eating: acidic (non-tin) corpse deals `rnd(15)` damage (can kill); lizard cures confusion/stun; slowing down effect.

### 9.2 Unslime (`munslime`)

When turning into green slime, monster tries:
1. **Fire breath** on self (if has AT_BREA/AD_FIRE, not cancelled, `mspec_used == 0`)
2. **Scroll of fire** (requires eyes, sight, hands)
3. **Potion of oil** (requires hands; lit then drunk, `d(3,4)` damage)
4. **Wand of fire** or **fire horn** (spe > 0)
5. **Fire trap** -- move to adjacent fire trap

---

## 10. Item Searching (`searches_for_item`)

Determines which items on the floor a monster will pick up:

| Class | Items Sought | Conditions |
|---|---|---|
| Wand | Digging (if not floater), Polymorph (if difficulty < 6), all rays, striking, undead turning, teleportation, create monster | spe > 0 |
| Wand | Make invisible | Not invisible, not invis_blocked, not gaze attacker |
| Wand | Speed monster | Not already MFAST |
| Potion | Healing, Extra Healing, Full Healing, Polymorph, Gain Level, Paralysis, Sleeping, Acid, Confusion | |
| Potion | Blindness | Not if has gaze attack |
| Potion | Speed | Not if MFAST |
| Potion | Invisibility | Not invisible, not invis_blocked |
| Scroll | Teleportation, Create Monster, Earth, Fire | |
| Amulet | Life Saving | Not nonliving, not vampshifter |
| Amulet | Reflection, Guarding | Always |
| Tool | Pick-axe | If `needspick` |
| Tool | Unicorn horn | Not cursed, not unicorn/Ki-rin |
| Tool | Frost/fire horn | spe > 0, can_blow |
| Tool | Container | Not cursed mbag, not locked |
| Tool | Expensive camera | spe > 0 |
| Food | Cockatrice corpse | If wearing gloves |
| Food | Lizard/acidic corpse | If not stone-resistant, for curing stoning |
| Food | Cockatrice egg | If `touch_petrifies` |

---

## 11. Reflection System (`mon_reflects`)

Monster reflection is checked in order:
1. Shield of Reflection (W_ARMS)
2. Wielded artifact with reflection
3. Amulet of Reflection (W_AMUL)
4. Silver dragon scales / silver dragon scale mail (W_ARM)
5. Innate: silver dragon or chromatic dragon permonst

---

## 12. Seen-Resistance Tracking

Monsters track hero resistances via `seen_resistance` bitmask:

```c
enum m_seen_resistance {
    M_SEEN_MAGR  = 0x0001,  // Antimagic
    M_SEEN_FIRE  = 0x0002,
    M_SEEN_COLD  = 0x0004,
    M_SEEN_SLEEP = 0x0008,
    M_SEEN_DISINT= 0x0010,
    M_SEEN_ELEC  = 0x0020,
    M_SEEN_POISON= 0x0040,
    M_SEEN_ACID  = 0x0080,
    M_SEEN_REFL  = 0x0100,
};
```

When the hero resists, `monstseesu(mask)` is called. When the hero loses a resistance, `monstunseesu(mask)` clears it from all monsters.

A monster will not use an offensive item against a resistance it has observed, e.g., will not zap wand of fire if `M_SEEN_FIRE` is set.

---

## 13. Suspected Bugs

1. **[疑似 bug] Defensive item randomization**: In `find_defensive`, the `!rn2(3)` break after finding any item means the scan stops with 2/3 probability per subsequent iteration. This makes the "priority" of items unstable -- a wand of digging found early might be overridden by a later healing potion, or vice versa, depending on inventory order and randomness. The comment says "selection could be improved by collecting all possibilities into an array and then picking one at random."

2. **[疑似 bug] Offensive item is last-found-wins**: `find_offensive` scans the entire inventory. The `nomore()` macro only skips re-checking a type already selected, but a later item of a different type will override an earlier one. Comment in source: "this picks the last viable item rather than prioritizing choices." For example, a wand of death can be overridden by a potion of acid if the potion appears later in inventory.

3. **[疑似 bug] Misc item selection not prioritized**: Comment in `find_misc`: "[bug?] Choice of item is not prioritized; the last viable one in the monster's inventory will be chosen."

4. **[疑似 bug] Typos in panic strings**: `use_defensive` has "potioh of healing", "potioh of extra healing", "potioh of full healing" instead of "potion". These are in `panic()` strings so they only appear in impossible-condition crashes.

5. **[疑似 bug] Camera use_offensive returns 1 instead of 2**: Most `use_offensive` cases return 2 (monster used turn but survived), but `MUSE_CAMERA` returns 1 (which means "monster died"). Since the camera flash doesn't kill the monster, this causes the caller (`mattacku`) to think the monster died. In practice this means the monster's turn ends correctly but the return value is semantically wrong.

---

## 14. 测试向量

### 14.1 Healing Threshold

The condition to NOT heal (return FALSE) is:
```
mhp >= mhpmax || (mhp >= 10 && mhp * fraction >= mhpmax)
```
So monster heals when: `mhp < mhpmax AND (mhp < 10 OR mhp * fraction < mhpmax)`.

| # | Hero Level | Mon HP | Mon MaxHP | fraction | `hp<max` | `hp<10` | `hp*frac<max` | Heals? |
|---|---|---|---|---|---|---|---|---|
| 1 | 5 | 3 | 20 | 5 | T | T | - | YES |
| 2 | 5 | 10 | 20 | 5 | T | F | 50>=20 F | NO |
| 3 | 5 | 4 | 21 | 5 | T | T | - | YES |
| 4 | 5 | 4 | 20 | 5 | T | T | - | YES |
| 5 | 12 | 6 | 25 | 4 | T | T | - | YES |
| 6 | 12 | 10 | 25 | 4 | T | F | 40>=25 F | NO |
| 7 | 12 | 6 | 24 | 4 | T | T | - | YES |
| 8 | 15 | 9 | 30 | 3 | T | T | - | YES |
| 9 | 15 | 10 | 30 | 3 | T | F | 30>=30 F | NO |
| 10 | 15 | 10 | 31 | 3 | T | F | 30<31 T | YES |
| 11 | 15 | 11 | 31 | 3 | T | F | 33>=31 F | NO |
| 12 | 15 | 30 | 30 | 3 | F | - | - | NO (full hp) |

**Boundary conditions:**
- B1: `hp == 10, maxhp == 50, ulevel == 5, fraction == 5`: `10*5=50 >= 50` => does NOT seek healing (hp>=10 and hp*frac >= max; exact boundary).
- B2: `hp == 9, maxhp == 50, ulevel == 5, fraction == 5`: `hp < 10` => DOES seek healing (absolute threshold overrides).
- B3: `hp == mhpmax`: never seeks healing (`mhp >= mhpmax` short-circuits to return FALSE).

### 14.2 Defensive Item Selection

| # | Monster State | Inventory | Expected Action |
|---|---|---|---|
| 1 | Confused, has hands | Uncursed unicorn horn | MUSE_UNICORN_HORN |
| 2 | Confused, has hands | Cursed unicorn horn only | Check lizard corpse |
| 3 | Confused, Ki-rin | No inventory | MUSE_UNICORN_HORN (innate) |
| 4 | Blind, not Pestilence | POT_FULL_HEALING | MUSE_POT_FULL_HEALING |
| 5 | Blind, Pestilence | POT_FULL_HEALING | Does NOT use (Pestilence excluded) |
| 6 | Hero wields cockatrice corpse, lined up | WAN_UNDEAD_TURNING (spe>0) | MUSE_WAN_UNDEAD_TURNING |
| 7 | Low HP, peaceful | POT_HEALING, SCR_TELEPORTATION | MUSE_POT_HEALING (peaceful skips escape) |
| 8 | Low HP, hostile, on stairs | nothing | MUSE_DOWNSTAIRS or MUSE_UPSTAIRS |
| 9 | Mindless monster | anything | FALSE (excluded from all defense) |
| 10 | Animal form | anything | FALSE (excluded from all defense) |

### 14.3 Offensive Item Selection

| # | Monster State | Inventory | Hero Resistance | Expected |
|---|---|---|---|---|
| 1 | Hostile, lined up | WAN_DEATH (spe>0) | None seen | MUSE_WAN_DEATH |
| 2 | Hostile, lined up | WAN_DEATH (spe>0) | M_SEEN_MAGR | Skips death, tries next |
| 3 | Hostile, lined up, adjacent | WAN_FIRE (spe>0) | M_SEEN_REFL | Skips (reflection_skip when adjacent) |
| 4 | Hostile, lined up, not adjacent | WAN_FIRE (spe>0) | M_SEEN_REFL | Skips (reflection_skip) |
| 5 | Hostile, adjacent | SCR_EARTH, hard helmet | None | MUSE_SCR_EARTH |
| 6 | Hostile, dist=3 | SCR_EARTH | None | Skip (dist2 > 2) |
| 7 | Hostile, lined up | POT_PARALYSIS | multi < 0 (hero paralyzed) | Skip (multi >= 0 check fails) |
| 8 | Peaceful | WAN_DEATH (spe>0) | None | FALSE (peaceful excluded) |
| 9 | Has gaze attack | POT_BLINDNESS | None | Skip (gaze attacker won't blind target) |
| 10 | Hostile, nohands | WAN_FIRE (spe>0) | None | FALSE (nohands excluded) |

### 14.4 Weapon Selection

| # | Monster | Inventory | Expected Weapon |
|---|---|---|---|
| 1 | Strong, no shield | Artifact long sword, tsurugi | Artifact long sword |
| 2 | Giant | Club, long sword | Club (giant preference) |
| 3 | Weak, wearing shield | Two-handed sword, mace | Mace (bimanual blocked by shield) |
| 4 | Silver-hating | Silver saber, iron long sword | Iron long sword |
| 5 | No gloves, no stone resist | Cockatrice corpse, dagger | Dagger (corpse requires gloves) |
| 6 | Wearing gloves | Cockatrice corpse, tsurugi | Cockatrice corpse (highest in hwep) |

### 14.5 Equipment Selection

| # | Monster | Current Armor | New Armor | Expected |
|---|---|---|---|---|
| 1 | Not MFAST | Regular boots (+1) | Speed boots (+1) | Equip speed boots (+20 pref bonus) |
| 2 | Already MFAST | Speed boots (+1) | Regular boots (+3) | Equip regular boots (+3 > +1+0) |
| 3 | Has horns | Iron helm | Leather cap | Equip leather cap (flimsy ok for horns) |
| 4 | Wearing cursed helm | Better helm | - | Keep cursed (can't remove) |
| 5 | Has life saving amulet | Amulet of reflection | - | Keep life saving (no swap) |

### 14.6 Boundary Conditions

- **B1 (Flee timer at full HP)**: `fleetim = 33 - 30*mhp/mhpmax` = `33 - 30` = 3 turns.
- **B2 (Flee timer at 1 HP, maxhp=100)**: `fleetim = 33 - 30*1/100` = `33 - 0` = 33 turns. (capped by mfleetim 7-bit field = max 127).
- **B3 (Wand backfire exactly kills)**: Monster with `mhp == d(spe+2, 6)`. E.g., spe=0, mhp=6, roll `d(2,6)=12` => dead; roll `d(2,6)=2` => survives with 4 hp.
- **B4 (POTION_OCCUPANT_CHANCE with 0 births)**: `13 + 2*0 = 13`, so 1/13 chance. With 10 births: `13 + 20 = 33`, so 1/33 chance.
- **B5 (Healing potion dice)**: Cursed healing: `d(6-2, 4) = d(4,4)` range 4-16. Blessed healing: `d(6+2, 4) = d(8,4)` range 8-32.
