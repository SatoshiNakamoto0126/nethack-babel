# Hunger & Eating Mechanism Spec

Source: `src/eat.c`, `include/hack.h`, `include/you.h`, `include/context.h`, `include/objects.h`, `src/pray.c`, `src/allmain.c`, `src/hack.c`

---

## 1. Nutrition Counter

The hero has a single integer counter `u.uhunger` that tracks satiation level. It starts at 900 (`init_uhunger()`) and is modified by:

- **Depletion**: `gethungry()` called every move (and also on melee attacks via `overexertion()`)
- **Gain**: `lesshungry(num)` adds `num` to `u.uhunger`
- **Loss**: `morehungry(num)` subtracts `num` from `u.uhunger`

The counter can go negative (during fainting/starvation). There is no upper clamp; choking logic triggers at `u.uhunger >= 2000`.

---

## 2. Hunger States and Thresholds

Defined in `include/hack.h` as `enum hunger_state_types`:

| State        | Enum Value | Threshold (u.uhunger) |
|-------------|-----------|----------------------|
| `SATIATED`   | 0         | > 1000               |
| `NOT_HUNGRY` | 1         | > 150                |
| `HUNGRY`     | 2         | > 50                 |
| `WEAK`       | 3         | > 0                  |
| `FAINTING`   | 4         | <= 0                 |
| `FAINTED`    | 5         | (set when faint occurs) |
| `STARVED`    | 6         | (death)              |

State computation in `newuhs()`:

```
newhs = if h > 1000 then SATIATED
        else if h > 150 then NOT_HUNGRY
        else if h > 50 then HUNGRY
        else if h > 0 then WEAK
        else FAINTING
```

### Strength Penalty

When transitioning to `WEAK` (newhs >= WEAK and old < WEAK): `ATEMP(A_STR) = -1` (temporary -1 Str).
When recovering from `WEAK` (newhs < WEAK and old >= WEAK): `ATEMP(A_STR) = 0` (restored).

### Fainting

When `newhs == FAINTING` and not already fainted:

```
uhunger_div_by_10 = sgn(u.uhunger) * ((abs(u.uhunger) + 5) / 10)

if u.uhs <= WEAK OR rn2(20 - uhunger_div_by_10) >= 19:
    faint_duration = 10 - uhunger_div_by_10
    hero faints for faint_duration turns (nomul(-faint_duration))
    hero becomes temporarily deaf for faint_duration
    selftouch("Falling, you") if not levitating
    newhs = FAINTED
```

The fainting check probability: `rn2(20 - uhunger_div_by_10) >= 19` means the chance increases as hunger counter goes more negative.

### Starvation Death

```
if u.uhunger < -(100 + 10 * ACURR(A_CON)):
    die from starvation
```

The death threshold depends on Constitution. A Con 18 hero dies at uhunger < -280; a Con 3 hero dies at uhunger < -130.

### Exhaustion Death

After setting `u.uhs = newhs`, if HP (or mHP when polymorphed) < 1:

```
die from "hunger and exhaustion"
```

---

## 3. Per-Turn Hunger Depletion (`gethungry()`)

Called every move in `moveloop()` and additionally on melee attacks via `overexertion()`.

Skipped entirely if `u.uinvulnerable` (praying) or `iflags.debug_hunger`.

### Base Depletion

```
if (!Unaware OR !rn2(10)):       // slow metabolic rate while asleep
    if (carnivorous OR herbivorous OR metallivorous):
        if !Slow_digestion:
            u.uhunger--             // 1 point per move
```

Unaware = `(multi < 0 && (unconscious() || is_fainted()))`. When asleep/fainted, only 10% chance of base depletion per turn.

Polymorph forms that cannot eat (not carnivorous, not herbivorous, not metallivorous) skip base depletion entirely.

### Accessory Hunger

A random number `accessorytime = rn2(20)` is rolled each call (replaced the old `moves % 20` to defeat ring-juggling).

**Odd accessorytime values (1, 3, 5, 7, 9, 11, 13, 15, 17, 19):**

- Regeneration (from non-artifact, non-polyform source): `u.uhunger--`
- Encumbrance > SLT_ENCUMBER (Stressed or worse): `u.uhunger--`

**Even accessorytime values (0, 2, 6, 10, 14, 18):**

- Hunger property active: `u.uhunger--`
- Conflict (from non-artifact source): `u.uhunger--`

**Specific even values:**

| accessorytime | Effect |
|---|---|
| 0 | Slow_digestion from armor (not from ring): `u.uhunger--` |
| 4 | Left ring worn (not meat ring, and has nonzero spe or is uncharged type or is sole source of +0 protection; see note below): `u.uhunger--` |
| 8 | Amulet worn (not fake Amulet of Yendor): `u.uhunger--` |
| 12 | Right ring worn (same criteria as left ring but simpler +0 protection check; see note below): `u.uhunger--` |
| 16 | Carrying real Amulet of Yendor (`u.uhave.amulet`): `u.uhunger--` |

Meat rings and +0 charged rings (where another protection source exists) do NOT cause hunger. The fake Amulet of Yendor does NOT cause hunger when worn.

**Left/right ring asymmetry**: The left ring check (accessorytime=4) has a more complex double-counting guard for two +0 rings of protection. It checks whether the right ring is also a +0 ring of protection and, if so, still counts the left ring as a hunger source (since only one +0 ring should get the free pass). The right ring check (accessorytime=12) uses a simpler test: `(EProtection & ~W_RINGR) == 0L`, which only checks whether the right ring is the sole source of protection. This means if both rings are +0 protection (and no amulet of guarding), the left ring causes hunger but the right ring does not.

### Summary: Maximum Depletion Per Turn

In the worst case (not asleep, no Slow_digestion, wearing two non-meat rings, an amulet, carrying Amulet, regenerating, heavily encumbered, with Hunger and Conflict active), depletion can be up to ~5 points per turn on average.

With `Slow_digestion` active (from a ring): base depletion is suppressed, but ring/amulet hunger still applies on even turns. Slow_digestion from armor (not ring) itself costs hunger on accessorytime==0.

---

## 4. Food Nutrition Values

### Standard Comestibles

| Food Item | Nutrition | Eating Time (oc_delay) |
|---|---|---|
| Tripe ration | 200 | 2 |
| Egg | 80 | 1 |
| Meatball | 5 | 1 |
| Meat stick | 5 | 1 |
| Enormous meatball | 2000 | 20 |
| Meat ring | 5 | 1 |
| Glob (any pudding/ooze/slime) | owt (weight) | 2 |
| Kelp frond | 30 | 1 |
| Eucalyptus leaf | 1 | 1 |
| Apple | 50 | 1 |
| Orange | 80 | 1 |
| Pear | 50 | 1 |
| Melon | 100 | 1 |
| Banana | 80 | 1 |
| Carrot | 50 | 1 |
| Sprig of wolfsbane | 40 | 1 |
| Clove of garlic | 40 | 1 |
| Slime mold (player fruit) | 250 | 1 |
| Lump of royal jelly | 200 | 1 |
| Cream pie | 100 | 1 |
| Candy bar | 100 | 1 |
| Fortune cookie | 40 | 1 |
| Pancake | 200 | 2 |
| Lembas wafer | 800 | 2 |
| Cram ration | 600 | 3 |
| Food ration | 800 | 5 |
| K-ration | 400 | 1 |
| C-ration | 300 | 1 |
| Tin | 0 (special) | 0 (special) |

### Corpse Nutrition

Corpse nutrition = `mons[corpsenm].cnutrit` (per-monster value defined in `monst.c`).

### Racial Modifiers (adj_victual_nutrition)

Applied per-bite during multi-turn eating:

- **Lembas wafer**:
  - Elf (or polymorphed into elf): nutrition increased by 25% (800 -> effective 1000)
  - Orc (or polymorphed into orc): nutrition decreased by 25% (800 -> effective 600)
- **Cram ration**:
  - Dwarf (or polymorphed into dwarf): nutrition increased by ~17% (600 -> effective 700)

---

## 5. Eating Time Calculation

### For Normal Food (non-corpse, non-glob)

Base eating time = `objects[otyp].oc_delay` (the delay column in objects.h table).

### For Corpses

```
reqtime = 3 + (cwt >> 6)    // cwt = monster's corpse weight
```

Where `cwt >> 6` is integer division by 64. So a 400-weight corpse takes `3 + 6 = 9` turns.

### For Globs

```
reqtime = 3 + (owt >> 6)    // owt = current glob weight
```

### Adjustment for Partially-Eaten Food

```
reqtime = rounddiv(reqtime * oeaten, basenutrit)
```

Where `oeaten` is the remaining nutrition and `basenutrit` is the full nutrition value.

### Nutrition Per Bite (nmod)

```
if reqtime == 0 OR oeaten == 0:
    nmod = 0
else if oeaten >= reqtime:
    nmod = -(oeaten / reqtime)     // negative: give |nmod| nutrition per turn
else:
    nmod = reqtime % oeaten        // positive: give 1 nutrition every nmod turns
```

When `nmod < 0`: each bite gives `adj_victual_nutrition()` nutrition (which equals `-nmod` plus racial modifier).
When `nmod > 0`: each bite gives 1 nutrition, but only on turns where `usedtime % nmod != 0`.

### Non-Food Items (metallivore, gelatinous cube, etc.)

All non-food items take exactly 1 turn to eat (`reqtime = 1`). Nutrition = item weight (for balls/chains), `oc_nutrition` (for normal objects), or `quan / 100` (for gold coins, capped at 2000).

---

## 6. Choking (Eating While Satiated)

### Trigger

Checked in `bite()` and `lesshungry()`. The `canchoke` flag is set when `u.uhs == SATIATED` at the start of eating.

- During multi-turn eating: if `canchoke && u.uhunger >= 2000`, calls `choke()`.
- From `lesshungry()`: if `u.uhunger >= 2000` and `(!iseating || canchoke)`, calls `choke()`.

### Choke Outcome

```
if u.uhs != SATIATED:
    return (no effect, unless AoS)

if Breathless OR Hunger property OR (!Strangled AND !rn2(20)):
    vomit: morehungry(Hunger ? (u.uhunger - 60) : 1000)
    // with Hunger: reduces to 60; without: loses 1000 nutrition
    vomit() side effects
else:
    death by choking
```

Survival chance: ~5% (1/20) for a normal hero. `!rn2(20)` succeeds with probability 1-in-20. The other 19/20 of the time, the hero chokes to death. Breathless or Hunger property guarantees survival. Being Strangled blocks the `!rn2(20)` escape route (guaranteed death unless Breathless or Hunger).

### "Nearly Full" Warning

When `u.uhunger >= 1500` and not already warned and no Hunger property:

```
"You're having a hard time getting all of it down."
```

If `canchoke` and more than 1 bite remains, prompts "Continue eating?"

### Lawful Knight Penalty

If `Role_if(PM_KNIGHT) && u.ualign.type == A_LAWFUL` and satiated: `adjalign(-1)` and "You feel like a glutton!"

---

## 7. Corpse Eating Effects

### Pre-Eating Effects (cprefx)

Called on first bite of corpse.

1. **Cannibalism check** (see Section 10)
2. **Petrification**: `flesh_petrifies()` (cockatrice, chickatrice, Medusa) -> instant stoning unless Stone_resistance or polymorphable into stone golem
3. **Domestic animals** (dogs, cats): unless `CANNIBAL_ALLOWED()` (Cave Dweller or Orc), "eating the <foo> was a bad idea" -> permanent Aggravate Monster
4. **Lizard**: cures petrification (`fix_petrification()`)
5. **Riders** (Death, Pestilence, Famine): instant death ("Eating that is instantly fatal"); corpse revives if life-saved
6. **Green slime**: unless Unchanging or slimeproof, starts sliming (10 turns)
7. **Acidic monsters**: if Stoned, cures petrification

### Post-Eating Effects (cpostfx)

Called after completely consuming a corpse.

| Monster | Effect |
|---|---|
| Wraith | Gain one experience level |
| Human wererat/werejackal/werewolf | Contract lycanthropy |
| Nurse | Full HP heal, cure blindness |
| Stalker | Temporary or permanent invisibility + see invisible |
| Yellow light, Giant bat | Stun +30 turns |
| Bat | Stun +30 turns |
| Mimics (small/large/giant) | Forced mimicry (20/40/50 turns) |
| Quantum mechanic | Toggle intrinsic speed |
| Lizard | Reduce stun to 2, reduce confusion to 2 |
| Chameleon/Doppelganger/Genetic engineer | Polymorph self (unless Unchanging) |
| Displacer beast | Temporary displacement (d(6,6) turns) |
| Disenchanter | Strip a random intrinsic |
| Mind flayer / Master mind flayer | 50% chance: +1 Int (if below max); otherwise fall through to intrinsic check |

### Intrinsic Gain from Corpses

After `cpostfx()`, if `check_intrinsics` is true, the system:

1. Checks for hallucination-inducing corpses: `dmgtype(AD_STUN)`, `dmgtype(AD_HALU)`, or violet fungus -> +200 turns hallucination
2. Checks for magical energy: `attacktype(AT_MAGC)` or newt -> `eye_of_newt_buzz()` (rnd(3) energy, 1/3 chance of +1 max energy if at max)
3. Selects ONE intrinsic from the corpse's `mconveys` flags and strength (for giants):

#### Intrinsic Selection (`corpse_intrinsic()`)

Builds a list of all conveyable intrinsics from `mconveys` flags plus strength-from-giants. Selects one uniformly at random (reservoir sampling). If strength is the only candidate, 50% chance of nothing.

#### Conveyance Probability (`should_givit()`)

For permanent intrinsics:

```
chance = case type of
    POISON_RES:
        if (killer bee or scorpion) AND rn2(4) == 0: chance = 1
        else: chance = 15
    TELEPORT:      chance = 10
    TELEPORT_CONTROL: chance = 12
    TELEPAT:       chance = 1
    default:       chance = 15

succeed if: monster_level > rn2(chance)
```

So a level-15 monster always conveys (for chance=15), while a level-1 monster has 1/15 chance for most intrinsics, but always succeeds for telepathy (chance=1).

#### Temporary Resistance (`temp_givit()`)

For STONE_RES and ACID_RES, even if `should_givit()` fails:

```
STONE_RES: succeed if monster_level > rn2(6)
ACID_RES:  succeed if monster_level > rn2(3)
```

These grant timed resistance: `incr_itimeout(d(3,6))` turns (3-18 turns).

#### Conveyable Intrinsics (via `mconveys` flags)

| Flag | Intrinsic | Type |
|---|---|---|
| MR_FIRE | Fire resistance | Permanent |
| MR_SLEEP | Sleep resistance | Permanent |
| MR_COLD | Cold resistance | Permanent |
| MR_DISINT | Disintegration resistance | Permanent |
| MR_ELEC | Shock resistance | Permanent |
| MR_POISON | Poison resistance | Permanent |
| MR_ACID | Acid resistance | Temporary (d(3,6) turns) |
| MR_STONE | Stoning resistance | Temporary (d(3,6) turns) |
| can_teleport | Teleportation | Permanent |
| control_teleport | Teleport control | Permanent |
| telepathic | Telepathy | Permanent |

#### Strength from Giants

`is_giant(ptr)` -> 50% chance if only candidate, otherwise competes with other intrinsics. Calls `gainstr()`.

### Poisonous Corpses

```
if poisonous(mons[mnum]) AND rn2(5):   // 80% chance of poison effect
    if !Poison_resistance:
        lose rnd(4) Str, take rnd(15) damage
    else:
        "You seem unaffected by the poison."
```

### Acidic Corpses

```
if acidic(mons[mnum]) AND !Acid_resistance:
    take rnd(15) acid damage
```

### Tainted (Rotten) Corpses

Rot calculation:

```
rotted = (moves - corpse_age) / (10 + rn2(20))
if cursed: rotted += 2
if blessed: rotted -= 2
```

Non-rotting corpses: lizard, lichen, Riders, acid blob.

| rotted | Effect |
|---|---|
| > 5 | "tainted" -> food poisoning (rn1(10,10) = 10-19 turns to die) unless Sick_resistance |
| > 5 OR (> 3 AND rn2(5)) | "mildly ill" -> rnd(8) damage (unless tainted already triggered) |

### Rotten Food (non-tainted, non-poisonous, non-acidic)

If `!nonrotting_corpse` and (`orotten` flag or `!rn2(7)`):

25% chance each of:
- Confusion: d(2,4) turns
- Blindness: d(2,10) turns
- Unconscious: rnd(10) turns + deafness
- Nothing (25% implicit from the `!rn2(4)` / `!rn2(4)` / `!rn2(3)` chain)

[疑似 bug] The probability chain is: 1/4 confusion, 3/4 * 1/4 = 3/16 blindness, 3/4 * 3/4 * 1/3 = 3/16 unconscious, remainder (~6/16) nothing. This is not exactly 25% each.

---

## 8. Tin Eating

### Tin Contents

Tin variety is stored in `obj->spe`:
- `spe == 1`: Spinach tin (SPINACH_TIN)
- `spe < 0`: `-spe - 1` indexes into `tintxts[]` array (0=rotten, 1=homemade, ...)
- `spe == 0`: Variety determined randomly at open time

If `obj->cursed`: always rotten.

### Tin Varieties and Nutrition

| Index | Variety | Nutrition | Fodder | Greasy |
|---|---|---|---|---|
| 0 | rotten | -50 (vomiting) | no | no |
| 1 | homemade | 50 | yes | no |
| 2 | soup made from | 20 | yes | no |
| 3 | french fried | 40 | no | yes |
| 4 | pickled | 40 | yes | no |
| 5 | boiled | 50 | yes | no |
| 6 | smoked | 50 | yes | no |
| 7 | dried | 55 | yes | no |
| 8 | deep fried | 60 | no | yes |
| 9 | szechuan | 70 | yes | no |
| 10 | broiled | 80 | no | no |
| 11 | stir fried | 80 | no | yes |
| 12 | sauteed | 95 | no | no |
| 13 | candied | 100 | yes | no |
| 14 | pureed | 500 | yes | no |

Special: homemade tin nutrition is capped at the monster's `cnutrit` value.

### Spinach Tin Nutrition

```
blessed: 600
uncursed: 400 + rnd(200) = 401..600
cursed: 200 + rnd(400) = 201..600
```

Spinach also calls `gainstr()`.

### Rotten Tin

Nutrition = -50, causes `make_vomiting(rn1(15, 10))` = 10..24 turns of vomiting.

Homemade tins have 1/7 chance of becoming rotten when opened (unless blessed). Non-rotting monster tins (lizard, lichen, Riders, acid blob) cannot be rotten.

### Tin Opening Time

Depends on wielded tool:

| Tool | Opening Time (turns) |
|---|---|
| Metallivore form | 0 (instant) |
| Blessed tin (no blessed tin opener) | rn2(2) = 0 or 1 |
| Blessed tin + blessed tin opener | 0 (instant) |
| Tin opener (uncursed) | rn2(2) = 0 or 1 |
| Tin opener (blessed) | 0 (instant, via rn2(1)) |
| Tin opener (cursed) | rn2(3) = 0, 1, or 2 |
| Dagger/knife/athame/crysknife/stiletto | 3 |
| Pick-axe/axe | 6 |
| Bare hands (no tool) | rn1(1 + 500/(DEX+STR), 10) |
| Blessed tin (always) | 0 or 1 as above |

Maximum opening time is 50 turns (`opentin()` gives up after that).

### Tin Effects

Tins apply the same `cprefx()`/`cpostfx()` effects as corpses (petrification, Rider death, intrinsic gain, etc.). Additionally:

- Greasy tins cause slippery fingers: `make_glib(rn1(11, 5))` = 5..15 turns
- Cursed tins have 1/8 chance of being booby-trapped (explodes)
- Metallivore hero eating a tin also gains +5 nutrition from the tin itself

---

## 9. Vegan/Vegetarian Conduct Tracking

Three conduct counters in `u.uconduct`:

| Counter | Meaning |
|---|---|
| `food` | Any comestible consumed (foodless conduct) |
| `unvegan` | Animal product consumed (vegan conduct broken) |
| `unvegetarian` | Meat consumed (vegetarian conduct broken) |

### What Breaks Which Conduct

**Foodless** (`food`): Any eating at all, including non-food items (metallivore eating weapons, etc.), eating brains as mind flayer, drinking potions via eating.

**Vegan** (`unvegan`): Eating non-vegan corpses/tins, eggs, flesh-material food items, leather/bone/dragon_hide/wax items, pancakes, fortune cookies, cream pies, candy bars, lump of royal jelly.

**Vegetarian** (`unvegetarian`): Eating non-vegetarian corpses/tins, flesh-material food (except eggs), leather/bone/dragon_hide items (but NOT wax).

`violated_vegetarian()` also penalizes Monks: "You feel guilty" + `adjalign(-1)`.

### Monster Classification

- `vegan(ptr)`: monster has no animal products (determined by `M2_MEATY` and other flags)
- `vegetarian(ptr)`: monster is not meat (no `M2_MEATY` flag but could have animal byproducts)

---

## 10. Cannibalism

### Definition

Eating a corpse/tin/egg of same race triggers cannibalism. Checked in `maybe_cannibal()`:

```
if !CANNIBAL_ALLOWED():   // Cave Dweller or Orc are exempt
    if your_race(food_monster)
       OR (Upolyd AND same_race(current_form, food_monster))
       OR (lycanthrope AND were_beastie(pm) == u.ulycn):
        cannibalism triggered
```

### Penalties

```
HAggravate_monster |= FROMOUTSIDE   // permanent aggravate monster
change_luck(-rn1(4, 2))             // luck penalty: -2 to -5
```

Messages: "You have a bad feeling deep inside." (if poly'd human eating human), "You cannibal! You will regret this!"

### Domestic Animal Penalty

Eating dogs/cats (PM_LITTLE_DOG through PM_LARGE_CAT) without CANNIBAL_ALLOWED:

```
HAggravate_monster |= FROMOUTSIDE   // permanent aggravate monster
```

No luck penalty (unlike true cannibalism).

### Once-Per-Turn Guard

`maybe_cannibal()` uses a static `ate_brains` variable to prevent multiple penalties from multi-tentacle mind flayer attacks in a single turn.

---

## 11. Ring/Accessory Eating

### Who Can Eat Rings

Metallivorous polymorph forms (rust monster, rock mole, xorn) can eat metallic items. Gelatinous cube form can eat organic items. Fire elemental form can eat flammable items. Ghoul form can eat non-veggy corpses and eggs.

### Ring Eating Effects

When eating a ring or amulet, 1/3 chance (rings) or 1/5 chance (amulets) of gaining the item's property. Effects by type:

| Item | Effect |
|---|---|
| Default (has oc_oprop) | Gain intrinsic (FROMOUTSIDE) |
| Ring of adornment | adjattrib(A_CHA, spe) |
| Ring of gain strength | adjattrib(A_STR, spe) |
| Ring of gain constitution | adjattrib(A_CON, spe) |
| Ring of increase accuracy | bounded_increase to u.uhitinc |
| Ring of increase damage | bounded_increase to u.udaminc |
| Ring of protection / Amulet of guarding | HProtection + bounded_increase to u.ublessed |
| Ring of free action | Sleep resistance (not free action!) |
| Ring of levitation | Temporary levitation d(10,20) turns (NOT permanent) |
| Amulet of change | Sex change |
| Amulet of unchanging | Rehumanize if polymorphed |
| Amulet of strangulation | Choke (potentially fatal) |
| Amulet of restful sleep | Sleepy intrinsic + rnd(100) timeout |
| Ring of slow digestion | Indigestible! Causes rottenfood effect instead |

### Bounded Increase

For accuracy, damage, and protection from ring eating:

```
if abs(old) + abs(inc) < 10: full increase
if abs(old) + abs(inc) < 20: rnd(inc), minimum to reach 10
if abs(old) + abs(inc) < 40: 0 or 1 (rn2), minimum to reach 20
if abs(old) + abs(inc) >= 40: no increase
```

---

## 12. Prayer for Food

### Major Trouble: TROUBLE_STARVING (priority 9)

Triggered when `u.uhs >= WEAK` (hunger counter <= 50).

### Minor Trouble: TROUBLE_HUNGRY (priority -8)

Triggered when `u.uhs >= HUNGRY` (hunger counter <= 150).

### Fix

Both call `init_uhunger()`:

```
u.uhunger = 900
u.uhs = NOT_HUNGRY
ATEMP(A_STR) = 0 (if was negative)
```

### Golden Glow Boon

When prayer succeeds with no troubles to fix and `u.uhunger < 900`:

```
init_uhunger()    // sets u.uhunger = 900
```

---

## 13. Foodless Conduct

Tracked by `u.uconduct.food`. Incremented on ANY food consumption:

- Eating normal food (`doeat()`)
- Eating corpses/tins
- Eating non-food items (metallivore)
- Eating brains as mind flayer (`eat_brains()`)
- Eating spinach from tin
- Drinking potions via eating (polymorphed)
- Force-feeding by throne sitting (`src/hack.c` line 702)

NOT broken by: prayer food fixes, `init_uhunger()` calls, gaining nutrition from rings of slow digestion, Amulet of life saving revive.

---

## 14. Rotten Food (Non-Corpse)

Non-corpse food items (except fortune cookies, lembas wafers, cram rations) can rot:

```
if cursed:
    always rotten
else if not nonrotting_food(otyp):
    if (moves - age) > (blessed ? 50 : 30):
        if orotten flag OR !rn2(7):
            rotten
```

Non-rotting foods: lembas wafer, cram ration (but if cursed, they DO rot).

Rotten food effect (`rottenfood()`): same as rotten corpse effects (confusion/blindness/unconscious, see Section 7).

Additionally, rotten food halves remaining nutrition: `consume_oeaten(otmp, 1)` which does `oeaten >>= 1`.

---

## 15. Special Food Effects (fpostfx)

| Food | Effect |
|---|---|
| Sprig of wolfsbane | Cure lycanthropy |
| Carrot | Cure blindness (unless engulfed by blinding engulfer) |
| Fortune cookie | Random rumor + breaks literate conduct |
| Lump of royal jelly | Gain strength, rnd(20) HP heal (or -rnd(20) if cursed), 1/17 chance +1 max HP, cure wounded legs. If polymorphed into killer bee, polymorph into queen bee |
| Eucalyptus leaf | Cure sickness and vomiting (unless cursed) |
| Cursed apple | Fall asleep rn1(11, 20) = 20..30 turns (unless sleep resistant) |
| Cockatrice/chickatrice egg | Stoning (5 turns) unless Stone_resistance |
| Pyrolisk egg | Explosion d(3,6) fire damage (consumes egg immediately) |
| Stale egg | Vomiting d(10,4) turns |

---

## 16. Non-Rotting Corpses

```
#define nonrotting_corpse(mnum) \
    (mnum == PM_LIZARD || mnum == PM_LICHEN \
     || is_rider(&mons[mnum])               \
     || mnum == PM_ACID_BLOB)
```

These corpses never become tainted. The rot check is skipped entirely.

Lizard and lichen corpses also have the special property that they fix petrification (lizard) and have 0 nutrition that causes the corpse to rot away completely on eating (lichen, if no nutrition).

---

## 17. Food Detection (Blessed)

`u.uedibility` flag (set by blessed food detection spell) grants a one-time smell-based warning about dangerous food before eating. Checks (in priority order):

1. Tainted meat (rotted > 5, no Sick_resistance)
2. Stoning/sliming danger
3. Tainted meat with Sick_resistance
4. Rotten (rotted > 3 or orotten flag)
5. Poisonous (no Poison_resistance)
6. Cursed apple (no Sleep_resistance)
7. Monk eating meat
8. Acidic (no Acid_resistance)
9. Rustproofed metal (rust monster form)
10. Vegan conduct break
11. Vegetarian conduct break

---

## 测试向量

### Hunger State Transitions

| # | Input | Expected Output | Notes |
|---|---|---|---|
| 1 | `u.uhunger = 1001` | `newhs = SATIATED` | Just above threshold |
| 2 | `u.uhunger = 1000` | `newhs = NOT_HUNGRY` | Boundary: exactly 1000 is NOT satiated |
| 3 | `u.uhunger = 151` | `newhs = NOT_HUNGRY` | Just above HUNGRY threshold |
| 4 | `u.uhunger = 150` | `newhs = HUNGRY` | Boundary: exactly 150 is HUNGRY |
| 5 | `u.uhunger = 51` | `newhs = HUNGRY` | Just above WEAK threshold |
| 6 | `u.uhunger = 50` | `newhs = WEAK` | Boundary: exactly 50 is WEAK |
| 7 | `u.uhunger = 1` | `newhs = WEAK` | Just above FAINTING threshold |
| 8 | `u.uhunger = 0` | `newhs = FAINTING` | Boundary: exactly 0 is FAINTING |
| 9 | `u.uhunger = -1` | `newhs = FAINTING` | Negative hunger |
| 10 | `u.uhunger = -280, ACURR(A_CON) = 18` | Alive: `-280 < -280` is false (strict `<`) | Boundary: exactly at threshold survives |
| 11 | `u.uhunger = -281, ACURR(A_CON) = 18` | Dead: `-281 < -280` is true | One below threshold: dies |
| 12 | `u.uhunger = -131, ACURR(A_CON) = 3` | Death: `-(100 + 30) = -130`, -131 < -130 | Starvation death at Con 3 |

### Eating Time

| # | Input | Expected Output | Notes |
|---|---|---|---|
| 13 | Food ration (nutrition=800, delay=5), full | `reqtime = 5`, `nmod = -(800/5) = -160` | 160 nutrition per bite |
| 14 | Corpse, cwt=400 | `reqtime = 3 + (400 >> 6) = 3 + 6 = 9` | Corpse eating time |
| 15 | Corpse, cwt=0 | `reqtime = 3 + 0 = 3` | Minimum corpse eating time |
| 16 | Lembas wafer as Elf, full | Base nutrition 800, adj_victual_nutrition adds 25% per bite -> effective ~1000 | Racial modifier |
| 17 | Lembas wafer as Orc, full | Base nutrition 800, adj_victual_nutrition subtracts 25% per bite -> effective ~600 | Racial modifier |

### Choking

| # | Input | Expected Output | Notes |
|---|---|---|---|
| 18 | Eating while SATIATED, uhunger reaches 2000, Breathless | Always vomit, survive. morehungry(1000) (or u.uhunger - 60 if Hunger property) | Breathless guarantees survival |
| 19 | Eating while SATIATED, uhunger reaches 2000, no Breathless, no Strangled | 1/20 chance vomit+survive, 19/20 chance choke death | `!rn2(20)` = 5% survival |
| 20 | Eating while NOT_HUNGRY (canchoke=false), uhunger reaches 2000 | No choking (canchoke was not set) | canchoke only set at meal start |

### Intrinsic Gain

| # | Input | Expected Output | Notes |
|---|---|---|---|
| 21 | Eat floating eye (level 3, telepathic, chance=1) | `3 > rn2(1)` always true -> always gain telepathy | Telepathy is easy to get |
| 22 | Eat killer bee corpse (level 1, MR_POISON) | Special: 1/4 chance of chance=1 (always succeed), 3/4 chance of chance=15 (1/15 at level 1) | Net ~31% chance |
| 23 | Eat fire ant (level 3, MR_FIRE, chance=15) | `3 > rn2(15)` -> 3/15 = 20% chance | Standard intrinsic |
| 24 | Eat lizard corpse (level 5, MR_STONE, temp_givit chance=6) | `5 > rn2(6)` -> 5/6 chance of temporary stoning resistance d(3,6) turns | Temporary resistance |

### Tin

| # | Input | Expected Output | Notes |
|---|---|---|---|
| 25 | Blessed spinach tin | Nutrition = 600, gainstr() | Fixed blessed value |
| 26 | Uncursed spinach tin | Nutrition = 400 + rnd(200) = 401..600 | Random range |
| 27 | Homemade tin of monster with cnutrit=30 | Nutrition = min(50, 30) = 30 | Capped at monster nutrition |
| 28 | Rotten tin | No nutrition, vomiting rn1(15, 10) = 10..24 turns | Negative nutrition mapped to vomiting |

### Cannibalism

| # | Input | Expected Output | Notes |
|---|---|---|---|
| 29 | Human eating human corpse | Aggravate Monster (permanent), luck -2..-5 | Standard cannibalism |
| 30 | Orc eating orc corpse | No penalty (CANNIBAL_ALLOWED) | Orcs are exempt |

### Starvation Boundary

| # | Input | Expected Output | Notes |
|---|---|---|---|
| 31 | `u.uhunger = -130, ACURR(A_CON) = 3` | Alive: `-130 < -(100 + 30) = -130` is false (`<` not `<=`) | Exactly at threshold: survives |
| 32 | `u.uhunger = -131, ACURR(A_CON) = 3` | Dead: `-131 < -130` is true | One below threshold: dies |
