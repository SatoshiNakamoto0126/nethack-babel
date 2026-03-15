# NetHack 3.7 Potion System -- Complete Mechanics Specification

Source: `src/potion.c`, `src/muse.c`, `src/pray.c`, `src/fountain.c`, `src/mon.c`, `src/dothrow.c`, `src/explode.c`, `src/objnam.c`, `include/objects.h`, `include/obj.h`, `include/prop.h`

---

## 0. Conventions

- **bcsign(obj)**: returns `+1` if blessed, `-1` if cursed, `0` otherwise.
- **rn2(n)**: uniform random integer in `[0, n)`.
- **rn1(x, y)**: equivalent to `rn2(x) + y`, i.e. uniform in `[y, y+x)`.
- **rnd(n)**: uniform random integer in `[1, n]`.
- **d(n, s)**: roll `n` dice of `s` sides, i.e. sum of `n` calls to `rnd(s)`.
- **Maybe_Half_Phys(dmg)**: if hero has half physical damage (from artifact or high-level monk), damage is `(dmg + 1) / 2`; otherwise `dmg`.
- **TIMEOUT**: maximum value for an intrinsic timeout (a large constant, effectively unlimited).
- **itimeout_incr(old, incr)**: `min(old_timeout + incr, TIMEOUT)`, floored at 0.
- **Diluted**: potions have an `odiluted` flag. Diluted potions sometimes have reduced effects. Mixing always produces diluted results (unless water).
- **A_MAX = 6** attributes: STR, DEX, CON, INT, WIS, CHA (indices 0-5).

---

## 1. Potion Types (Complete List)

| Enum                   | Name               | Description      | Magic | Base Cost | Base Prob |
|------------------------|--------------------|------------------|-------|-----------|-----------|
| POT_GAIN_ABILITY       | gain ability       | ruby             | yes   | 300       | 40        |
| POT_RESTORE_ABILITY    | restore ability    | pink             | yes   | 100       | 40        |
| POT_CONFUSION          | confusion          | orange           | yes   | 100       | 40        |
| POT_BLINDNESS          | blindness          | yellow           | yes   | 150       | 30        |
| POT_PARALYSIS          | paralysis          | emerald          | yes   | 300       | 40        |
| POT_SPEED              | speed              | dark green       | yes   | 200       | 40        |
| POT_LEVITATION         | levitation         | cyan             | yes   | 200       | 40        |
| POT_HALLUCINATION      | hallucination      | sky blue         | yes   | 100       | 30        |
| POT_INVISIBILITY       | invisibility       | brilliant blue   | yes   | 150       | 40        |
| POT_SEE_INVISIBLE      | see invisible      | magenta          | yes   | 50        | 40        |
| POT_HEALING            | healing            | purple-red       | yes   | 20        | 115       |
| POT_EXTRA_HEALING      | extra healing      | puce             | yes   | 100       | 45        |
| POT_GAIN_LEVEL         | gain level         | milky            | yes   | 300       | 20        |
| POT_ENLIGHTENMENT      | enlightenment      | swirly           | yes   | 200       | 20        |
| POT_MONSTER_DETECTION  | monster detection  | bubbly           | yes   | 150       | 40        |
| POT_OBJECT_DETECTION   | object detection   | smoky            | yes   | 150       | 40        |
| POT_GAIN_ENERGY        | gain energy        | cloudy           | yes   | 150       | 40        |
| POT_SLEEPING           | sleeping           | effervescent     | yes   | 100       | 40        |
| POT_FULL_HEALING       | full healing       | black            | yes   | 200       | 10        |
| POT_POLYMORPH          | polymorph          | golden           | yes   | 200       | 10        |
| POT_BOOZE              | booze              | brown            | no    | 50        | 40        |
| POT_SICKNESS           | sickness           | fizzy            | no    | 50        | 40        |
| POT_FRUIT_JUICE        | fruit juice        | dark             | no    | 50        | 40        |
| POT_ACID               | acid               | white            | no    | 250       | 10        |
| POT_OIL                | oil                | murky            | no    | 250       | 30        |
| POT_WATER              | water              | clear (fixed)    | no    | 100       | 80        |

All potions weigh 20 aum, are made of GLASS material, and have nutrition value 10.

---

## 2. Pre-Quaff Events

Before potion effects are evaluated, two special events may occur based on the **randomized description** of the potion:

### 2.1 "Milky" Potion -- Ghost

If the potion's randomized description is "milky" (not necessarily POT_GAIN_LEVEL):
- Condition: ghosts not genocided, AND `rn2(POTION_OCCUPANT_CHANCE(born)) == 0`
- `POTION_OCCUPANT_CHANCE(n) = 13 + 2 * n` where `n` = number of ghosts previously born
- Effect: a ghost appears; hero is paralyzed for 3 turns (via `nomul(-3)`)
- The potion is consumed; normal effects do NOT apply

### 2.2 "Smoky" Potion -- Djinni

If the potion's randomized description is "smoky":
- Condition: djinn not genocided, AND `rn2(POTION_OCCUPANT_CHANCE(born)) == 0`
- Effect: a djinni appears; outcome determined by `chance = rn2(5)`:
  - **Blessed**: if `chance == 4`, reroll as `rnd(4)` (so 0,1,2,3); otherwise `chance = 0`. Distribution: wish=80%, tame=5%, peaceful=5%, vanish=5%, hostile=5%
  - **Uncursed**: raw roll. Distribution: wish=20%, tame=20%, peaceful=20%, vanish=20%, hostile=20%
  - **Cursed**: if `chance == 0`, reroll as `rn2(4)` (so 0,1,2,3); otherwise `chance = 4`. Distribution: wish=5%, tame=5%, peaceful=5%, vanish=5%, hostile=80%
  - Outcomes by chance value: 0=wish, 1=tame pet, 2=peaceful, 3=vanishes, 4=hostile
- The potion is consumed; normal effects do NOT apply

---

## 3. Quaffing Effects (Per Potion Type)

### 3.1 POT_RESTORE_ABILITY

- **Cursed**: "Ulch! This makes you feel mediocre!" -- no stat restoration.
- **Uncursed**: restores ONE random attribute (ABASE to AMAX) starting from a random attribute index, cycling through all. Additionally, if `u.ulevel < u.ulevelmax`, restores one lost level via `pluslvl()`.
- **Blessed**: restores ALL attributes (ABASE to AMAX for each). Additionally, restores ALL lost levels (loops `pluslvl()` while `u.ulevel < u.ulevelmax`).
- Overrides Fixed_abil (unlike unicorn horn).
- Does NOT recover temporary strength loss from hunger or temporary dex loss from wounded legs.

### 3.2 POT_HALLUCINATION

- If Halluc_resistance: counts as "nothing" effect.
- Duration: `itimeout_incr(HHallucination, rn1(200, 600 - 300 * bcsign(otmp)))`
  - Blessed: `rn1(200, 300)` = `[300, 500)` additional turns
  - Uncursed: `rn1(200, 600)` = `[600, 800)` additional turns
  - Cursed: `rn1(200, 900)` = `[900, 1100)` additional turns
- Enlightenment bonus: if `(blessed AND rn2(3)==0)` OR `(uncursed AND rn2(6)==0)`:
  - Shows enlightenment screen, exercises WIS

### 3.3 POT_WATER

- **Uncursed**: "This tastes like water." `u.uhunger += rnd(10)`. No identification.
- **Blessed (holy water)**:
  - If hero is undead/demon OR chaotic: burns like acid, `losehp(Maybe_Half_Phys(d(2,6)))`, cures lycanthropy, exercise CON (negative)
  - Otherwise: "You feel full of awe." Cures sickness (all types), cures lycanthropy, exercises WIS and CON
- **Cursed (unholy water)**:
  - If hero is undead/demon OR chaotic: heals `d(2,6)` HP, may trigger lycanthropic transformation, exercises CON
  - If hero is lawful (not undead/demon): burns like acid, `losehp(Maybe_Half_Phys(d(2,6)))`
  - If hero is neutral (not undead/demon): "You feel full of dread." May trigger lycanthropic transformation, exercises CON (negative)

### 3.4 POT_BOOZE

- Confusion (unless blessed): `itimeout_incr(HConfusion, d(2 + u.uhs, 8))`
  - `u.uhs` is hunger state (0=satiated, 1=not hungry, ..., 4=fainting); higher hunger = more confusion
- Heals 1 HP (unless diluted)
- Nutrition: `10 * (2 + bcsign(otmp))` = blessed 30, uncursed 20, cursed 10
- Exercises WIS (negative)
- **Cursed**: hero passes out for `rnd(15)` turns

### 3.5 POT_ENLIGHTENMENT

- **Cursed**: "You have an uneasy feeling..." -- exercises WIS (negative), counts as unknown
- **Uncursed**: shows enlightenment screen, exercises WIS
- **Blessed**: +1 INT, +1 WIS, then shows enlightenment screen, exercises WIS

### 3.6 POT_INVISIBILITY

- If already Invis, Blind, or BInvis: counts as "nothing" (no message), but timeout still increased
- Duration:
  - Blessed: `!rn2(HInvis ? 15 : 30)` chance of permanent intrinsic (`FROMOUTSIDE`). If HInvis is already set (any timeout/intrinsic): 1/15 chance. If HInvis is 0: 1/30 chance. If permanent is granted, no additional temporary duration is added.
  - Non-permanent: `d(6 - 3 * bcsign(otmp), 100) + 100`
    - Blessed: `d(3, 100) + 100` = `[103, 400]`
    - Uncursed: `d(6, 100) + 100` = `[106, 700]`
    - Cursed: `d(9, 100) + 100` = `[109, 1000]`
- **Cursed** additional effect: aggravates monsters, removes permanent invisibility (`FROMOUTSIDE`)

### 3.7 POT_SEE_INVISIBLE / POT_FRUIT_JUICE

These share the same handler:

**POT_FRUIT_JUICE** path (if `otmp->otyp == POT_FRUIT_JUICE`):
- Nutrition: `(odiluted ? 5 : 10) * (2 + bcsign(otmp))`
  - Blessed, not diluted: 30; cursed, diluted: 5
- Returns after nutrition; no other effects.

**POT_SEE_INVISIBLE** path:
- Taste message (uses fruit name)
- **Non-cursed**: cures blindness via `make_blinded(0L, TRUE)`
- **Blessed**: grants permanent See_invisible (`FROMOUTSIDE`)
- **Uncursed**: temporary See_invisible `rn1(100, 750)` = `[750, 850)` turns
- **Cursed**: "Yecch! This tastes rotten." -- no see invisible granted

### 3.8 POT_PARALYSIS

- If Free_action: "You stiffen momentarily." No paralysis.
- Otherwise: paralyzed for `rn1(10, 25 - 12 * bcsign(otmp))` turns
  - Blessed: `rn1(10, 13)` = `[13, 23)` turns
  - Uncursed: `rn1(10, 25)` = `[25, 35)` turns
  - Cursed: `rn1(10, 37)` = `[37, 47)` turns
- Exercises DEX (negative)

### 3.9 POT_SLEEPING

- If Sleep_resistance OR Free_action: "You yawn." No sleep.
- Otherwise: fall asleep for `rn1(10, 25 - 12 * bcsign(otmp))` turns (same formula as paralysis)
  - Blessed: `[13, 23)`, Uncursed: `[25, 35)`, Cursed: `[37, 47)`

### 3.10 POT_MONSTER_DETECTION

- **Blessed**: grants Detect_monsters intrinsic.
  - Duration increment: if current timeout >= 300, increment by 1; otherwise `rn2(100) + 100` = `[100, 200)` turns
  - Clears invisible monster glyphs; if swallowed/underwater, falls through to normal detection
  - If no monsters on level: "You feel lonely."
- **Uncursed/Cursed**: calls `monster_detect()` to show map of monsters

### 3.11 POT_OBJECT_DETECTION

- Calls `object_detect()` to show map of objects
- Exercises WIS on success

### 3.12 POT_SICKNESS

- "Yecch! This stuff tastes like poison."
- **Blessed**: "mildly stale fruit juice" -- loses 1 HP (unless Healer role)
- **Uncursed/Cursed, non-Healer**:
  - If not Poison_resistance: lose a random attribute by `rn1(4, 3)` = `[3, 7)` points; lose `rnd(10) + 5 * (cursed ? 1 : 0)` HP
  - If Poison_resistance: lose 1 point from random attribute; lose `1 + rn2(2)` HP
  - Healer role: "Fortunately, you have been immunized." (no damage)
- **All BUC**: if Hallucinating, cures hallucination
- Exercises CON (negative, unless blessed+Healer)

### 3.13 POT_CONFUSION

- Duration: `itimeout_incr(HConfusion, rn1(7, 16 - 8 * bcsign(otmp)))`
  - Blessed: `rn1(7, 8)` = `[8, 15)` turns
  - Uncursed: `rn1(7, 16)` = `[16, 23)` turns
  - Cursed: `rn1(7, 24)` = `[24, 31)` turns

### 3.14 POT_GAIN_ABILITY

- **Cursed**: "Ulch! That potion tasted foul!" -- no effect
- **Uncursed**: tries up to 6 random attributes; increases first one that can be raised by 1 point via `adjattrib()`
- **Blessed**: increases ALL 6 attributes by 1 point each
- If Fixed_abil: counts as "nothing"

### 3.15 POT_SPEED

- Temporary speed duration: `rn1(10, 100 + 60 * bcsign(otmp))`
  - Blessed: `rn1(10, 160)` = `[160, 170)`
  - Uncursed: `rn1(10, 100)` = `[100, 110)`
  - Cursed: `rn1(10, 40)` = `[40, 50)`
- **Non-cursed**: if hero lacks intrinsic speed, grants permanent speed (`FROMOUTSIDE`)
- If hero has Wounded_legs and non-cursed and not riding: heals legs (consumes potion, no speed effect)
- Exercises DEX

### 3.16 POT_BLINDNESS

- Duration: `itimeout_incr(BlindedTimeout, rn1(200, 250 - 125 * bcsign(otmp)))`
  - Blessed: `rn1(200, 125)` = `[125, 325)` turns
  - Uncursed: `rn1(200, 250)` = `[250, 450)` turns
  - Cursed: `rn1(200, 375)` = `[375, 575)` turns

### 3.17 POT_GAIN_LEVEL

- **Cursed**: hero physically rises through the ceiling to the level above (if possible). If on level 1 with the Amulet, goes to the Plane of Earth. Otherwise, "You have an uneasy feeling."
- **Uncursed**: `pluslvl(FALSE)` -- gains one experience level
- **Blessed**: `pluslvl(FALSE)` then sets `u.uexp = rndexp(TRUE)` (random position within the new level's XP range)

### 3.18 POT_HEALING

- `healup(8 + d(4 + 2 * bcsign(otmp), 4), ...)`
  - Blessed: `8 + d(6, 4)` = `[14, 32]` HP
  - Uncursed: `8 + d(4, 4)` = `[12, 24]` HP
  - Cursed: `8 + d(2, 4)` = `[10, 16]` HP
- Max HP increase: uncursed/blessed: +1 max HP; cursed: +0
- Cures sickness: blessed only
- Cures blindness: uncursed and blessed
- Exercises CON

### 3.19 POT_EXTRA_HEALING

- `healup(16 + d(4 + 2 * bcsign(otmp), 8), ...)`
  - Blessed: `16 + d(6, 8)` = `[22, 64]` HP
  - Uncursed: `16 + d(4, 8)` = `[20, 48]` HP
  - Cursed: `16 + d(2, 8)` = `[18, 32]` HP
- Max HP increase: blessed +5, uncursed +2, cursed +0
- Cures sickness: uncursed and blessed
- Cures blindness (and deafness): always
- Cures hallucination: always
- Exercises CON and STR
- Blessed + not riding: heals wounded legs

### 3.20 POT_FULL_HEALING

- `healup(400, 4 + 4 * bcsign(otmp), ...)`
  - Always heals 400 HP (effectively full heal)
  - Max HP increase: blessed +8, uncursed +4, cursed +0
- Cures sickness: uncursed and blessed
- Cures blindness (and deafness): always
- Cures hallucination: always
- **Blessed**: restores one lost level (decrements `u.ulevelmax` by 1, then `pluslvl()`)
  - Note: "multiple potions will only get half of them back" -- each potion restores at most one level, but `ulevelmax` is decremented, so the net gain approaches half
- Exercises STR and CON
- Heals wounded legs: blessed always; uncursed if not riding; cursed never

### 3.21 POT_LEVITATION

- If not already levitating and not blocked: initiates levitation (timeout starts at 1, then `float_up()`)
- **Cursed**:
  - Clears `I_SPECIAL` (can't voluntarily descend)
  - If on upstairs: goes up a level
  - If under a ceiling: head bonk damage `rnd(no_helmet ? 10 : soft_helmet ? 6 : 3)`, `Maybe_Half_Phys`
- **Blessed**: duration `rn1(50, 250)` = `[250, 300)` turns; sets `I_SPECIAL` (can descend via `>`)
- **Uncursed**: duration `rn1(140, 10)` = `[10, 150)` turns; no `I_SPECIAL`

### 3.22 POT_GAIN_ENERGY

- Mana max change: `num = d(N, 6)` where N = 3 (blessed), 2 (uncursed), 1 (cursed)
  - Blessed: `d(3, 6)` = `[3, 18]`
  - Uncursed: `d(2, 6)` = `[2, 12]`
  - Cursed: `d(1, 6)` = `[1, 6]`, then negated: `[-6, -1]`
- `u.uenmax += num` (clamped to >= 0)
- `u.uen += 3 * num` (clamped to `[0, u.uenmax]`)
- Exercises WIS

### 3.23 POT_OIL

- **Lit oil**:
  - If hero `likes_fire()`: "refreshing drink"
  - Otherwise: `d(vulnerable ? 4 : 2, 4)` fire damage
    - `vulnerable = !Fire_resistance || Cold_resistance` [note: Cold_resistance makes fire hurt MORE]
  - Burns away slime
- **Unlit, cursed**: "This tastes like castor oil." -- no mechanical effect
- **Unlit, non-cursed**: "That was smooth!" -- no mechanical effect
- Exercises WIS (positive only if likes_fire and lit)

### 3.24 POT_ACID

- If Acid_resistance: "This tastes sour/tangy." No damage.
- Otherwise: `dmg = d(cursed ? 2 : 1, blessed ? 4 : 8)`
  - Blessed: `d(1, 4)` = `[1, 4]`
  - Uncursed: `d(1, 8)` = `[1, 8]`
  - Cursed: `d(2, 8)` = `[2, 16]`
  - `losehp(Maybe_Half_Phys(dmg))`
  - Exercises CON (negative)
- **All BUC**: if Stoned, cures petrification
- Counts as unknown (because holy/unholy water can also burn)

### 3.25 POT_POLYMORPH

- If Unchanging: no transformation
- **Blessed**: if currently in own natural form (`u.umonnum == u.umonster`), grants controlled polymorph with `POLY_LOW_CTRL`. If transformed into something other than natural form, the transformation duration is capped at `min(u.mtimedone, rn2(15) + 10)` = `[10, 25)`.
- **Non-blessed** (or blessed when already polymorphed): uncontrolled polymorph (`POLY_NOFLAGS`)

---

## 4. healup() Detailed Semantics

```
healup(nhp, nxtra, curesick, cureblind)
```

- Adds `nhp` to current HP
- If current HP exceeds max HP after addition: sets current HP to `max HP + nxtra` (i.e., increases max HP by `nxtra` and sets current to new max)
- `cureblind`: clears blindness (including cream on face), cures deafness
- `curesick`: cures vomiting and all types of sickness

---

## 5. Potion Throwing and Shattering Effects (potionhit)

When a potion hits a target (via `potionhit()`):

### 5.1 Bottle Impact Damage

- **On hero**: `Maybe_Half_Phys(rnd(2))` = 1-2 physical damage
- **On monster**: 80% chance (`rn2(5)`) of 1 HP damage (if monster has > 1 HP)

### 5.2 Saddle Hit

When the potion hits a saddled monster, there is a chance it hits the saddle instead:
- Base: `!rn2(10)` (10% chance)
- POT_WATER has enhanced saddle-targeting: also hits if `(rnl(10) > 7 AND cursed)` OR `(rnl(10) < 4 AND blessed)` OR `!rn2(3)`

If saddle hit by POT_WATER: applies H2Opotion_dip BUC logic (see Dip section).

### 5.3 Effects on Monsters (non-saddle hit)

| Potion | Effect on Monster |
|--------|-------------------|
| POT_HEALING | Heals to full HP. If blessed: cures blindness. Pestilence: treated as POT_SICKNESS instead. |
| POT_EXTRA_HEALING | Heals to full HP. If non-cursed: cures blindness. Pestilence: treated as POT_SICKNESS instead. |
| POT_FULL_HEALING | Heals to full HP. Always cures blindness. Pestilence: treated as POT_SICKNESS instead. |
| POT_RESTORE_ABILITY | Heals to full HP (same as healing). Does not anger. |
| POT_GAIN_ABILITY | Heals to full HP (same as healing). Does not anger. |
| POT_SICKNESS | If Pestilence: heals instead. If poison-resistant or disease-type: no effect. Otherwise: halves monster HP (if HP > 2). |
| POT_CONFUSION | If monster fails resistance check: confuses monster. |
| POT_BOOZE | If monster fails resistance check: confuses monster. |
| POT_INVISIBILITY | Makes monster invisible. Cursed: temporarily makes visible then invisible? Actually: `mon_set_minvis(mon, cursed_potion)` -- if cursed, the invisibility is "blocked" style. |
| POT_SLEEPING | `sleep_monst(mon, rnd(12), POTION_CLASS)` -- sleep for 1-12 turns if fails resistance |
| POT_PARALYSIS | If monster can move: paralyze for `rnd(25)` turns |
| POT_SPEED | Speeds up monster by one step. Does not anger. |
| POT_BLINDNESS | If monster has eyes and not permanently blind: blind for `64 + rn2(32) + rn2(32) * !resist(...)` turns, added to existing blindness, capped at 127. |
| POT_WATER (holy) | Against undead/demon/were/vampshifter: `d(2, 6)` damage. Kills if HP drops to 0. Reverts were to human form. |
| POT_WATER (unholy) | Against undead/demon/were/vampshifter: heals `d(2, 6)`. Triggers were transformation to beast. Does not anger. |
| POT_WATER | Gremlin: splits. Iron golem: `d(1, 6)` rust damage. |
| POT_OIL (lit) | Explodes as burning oil: `d(diluted ? 3 : 4, 4)` fire damage in explosion. |
| POT_ACID | If not acid-resistant and fails resistance: `d(cursed ? 2 : 1, blessed ? 4 : 8)` damage. |
| POT_POLYMORPH | Calls `bhitm()` -- standard polymorph-other effect. |
| POT_GAIN_LEVEL, POT_LEVITATION, POT_FRUIT_JUICE, POT_MONSTER_DETECTION, POT_OBJECT_DETECTION | No effect on monster. |

### 5.4 Effects on Hero (thrown at self)

Only three potions have special effects when thrown at the hero (beyond bottle damage):
- **POT_OIL (lit)**: explosion at hero's position
- **POT_POLYMORPH**: uncontrolled polymorph (only if not Unchanging AND not Antimagic)
- **POT_ACID**: same damage formula as quaffing acid

---

## 6. Vapor/Breathing Effects (potionbreathe)

Vapors from broken/thrown potions can be inhaled. Triggered when:
- Distance == 0 (potion broke at hero's location), OR
- Distance < 3 AND `rn2((1 + ACURR(A_DEX)) / 2) == 0`
- AND hero is not both breathless and eyeless

A wet towel (`Half_gas_damage`) blocks ALL vapor effects.

| Potion | Vapor Effect |
|--------|-------------|
| POT_RESTORE_ABILITY / POT_GAIN_ABILITY | Cursed: stinging message only. Non-cursed: restores 1 point to ONE random lowered attribute (blessed: restores 1 point to ALL lowered attributes). |
| POT_FULL_HEALING | +1 HP (poly and normal). Cures blindness. Falls through to extra healing. |
| POT_EXTRA_HEALING | +1 HP (poly and normal). Non-cursed: cures blindness. Falls through to healing. |
| POT_HEALING | +1 HP (poly and normal). Blessed: cures blindness. Exercises CON. |
| POT_SICKNESS | Non-Healer: lose 5 HP (or set to 1 if HP <= 5). Exercises CON (negative). |
| POT_HALLUCINATION | "You have a momentary vision." No hallucination applied. |
| POT_CONFUSION / POT_BOOZE | Confusion for `rnd(5)` turns (incremental). |
| POT_INVISIBILITY | Brief flash of invisibility (visual only, no actual invisibility). |
| POT_PARALYSIS | If no Free_action: paralyzed `rnd(5)` turns. |
| POT_SLEEPING | If no Free_action and no Sleep_resistance: sleep `rnd(5)` turns. |
| POT_SPEED | Speed boost: `rnd(5)` turns of Fast. Exercises DEX. |
| POT_BLINDNESS | Blind for `rnd(5)` turns (incremental). |
| POT_WATER | Gremlin (poly'd): split. Lycanthrope: blessed triggers revert, cursed triggers transformation. |
| POT_ACID / POT_POLYMORPH | Exercise CON (negative). |
| POT_GAIN_LEVEL, POT_GAIN_ENERGY, POT_LEVITATION, POT_FRUIT_JUICE, POT_MONSTER_DETECTION, POT_OBJECT_DETECTION, POT_OIL | No vapor effect. |

Note: Full Healing vapors give a total of +3 HP (from all three fallthrough cases), Extra Healing gives +2, Healing gives +1.

---

## 7. Dipping Effects (potion_dip)

### 7.1 Dipping into POT_WATER

Uses `H2Opotion_dip()`:

- **Blessed water**:
  - Cursed item -> uncurse (amber glow)
  - Uncursed item -> bless (light blue aura)
  - Blessed item -> no effect
- **Cursed water**:
  - Blessed item -> unbless (brown glow)
  - Uncursed item -> curse (black aura)
  - Cursed item -> no effect
- **Uncursed water**:
  - Applies `water_damage()` to the dipped item (may rust, dilute, etc.)

Special: dipping a towel into water wets it ("The towel soaks it up!").

### 7.2 Dipping into/with POT_POLYMORPH

If either the dipped item or the potion is POT_POLYMORPH:
- If the target object is unpolyable (wand/potion/spellbook of polymorph, or amulet of unchanging): nothing happens
- Otherwise: `poly_obj()` -- polymorphs the item into a random object of the same class
- Counts as a polypile conduct violation

### 7.3 Potion-into-Potion Mixing (Alchemy)

When dipping a potion into a different potion:

**Stack size limit**:
- If diluted: only 2 potions are affected
- If result is magic: `rnd(min(amt, 8) - 2) + 2` = `[3, 8]` potions
- If result is non-magic: `rnd(amt - 6) + 6` = `[7, N]` potions

**Explosion risk**: potions explode if ANY of:
- The dipped potion stack is cursed
- The dipped potion is POT_ACID
- The dipped potion is lit POT_OIL
- Random: `!rn2(wearing_alchemy_smock ? 30 : 10)` (3.3% or 10% chance)
- Explosion damage: `stack_count + rnd(9)`

**Known recipes** (via `mixtype()`):

| Dip Item (o1) | Into Potion (o2) | Result |
|---------------|-----------------|--------|
| POT_HEALING | POT_SPEED | POT_EXTRA_HEALING |
| POT_HEALING | POT_GAIN_LEVEL or POT_GAIN_ENERGY | POT_EXTRA_HEALING |
| POT_EXTRA_HEALING | POT_GAIN_LEVEL or POT_GAIN_ENERGY | POT_FULL_HEALING |
| POT_FULL_HEALING | POT_GAIN_LEVEL or POT_GAIN_ENERGY | POT_GAIN_ABILITY |
| Any healing/full_healing/extra_healing or UNICORN_HORN | POT_SICKNESS | POT_FRUIT_JUICE |
| Any healing/full_healing/extra_healing or UNICORN_HORN | POT_HALLUCINATION, POT_BLINDNESS, or POT_CONFUSION | POT_WATER |
| AMETHYST | POT_BOOZE | POT_FRUIT_JUICE |
| POT_GAIN_LEVEL or POT_GAIN_ENERGY | POT_CONFUSION | POT_BOOZE (2/3) or POT_ENLIGHTENMENT (1/3) |
| POT_GAIN_LEVEL or POT_GAIN_ENERGY | POT_HEALING | POT_EXTRA_HEALING |
| POT_GAIN_LEVEL or POT_GAIN_ENERGY | POT_EXTRA_HEALING | POT_FULL_HEALING |
| POT_GAIN_LEVEL or POT_GAIN_ENERGY | POT_FULL_HEALING | POT_GAIN_ABILITY |
| POT_GAIN_LEVEL or POT_GAIN_ENERGY | POT_FRUIT_JUICE | POT_SEE_INVISIBLE |
| POT_GAIN_LEVEL or POT_GAIN_ENERGY | POT_BOOZE | POT_HALLUCINATION |
| POT_FRUIT_JUICE | POT_SICKNESS | POT_SICKNESS |
| POT_FRUIT_JUICE | POT_ENLIGHTENMENT or POT_SPEED | POT_BOOZE |
| POT_FRUIT_JUICE | POT_GAIN_LEVEL or POT_GAIN_ENERGY | POT_SEE_INVISIBLE |
| POT_ENLIGHTENMENT | POT_LEVITATION | POT_GAIN_LEVEL (2/3 chance; 1/3 no recipe) |
| POT_ENLIGHTENMENT | POT_FRUIT_JUICE | POT_BOOZE |
| POT_ENLIGHTENMENT | POT_BOOZE | POT_CONFUSION |

Note: the function normalizes by swapping o1/o2 if o1 is a potion and o2 is one of gain_level, gain_energy, healing, extra_healing, full_healing, enlightenment, or fruit_juice. This means the recipe table is symmetric for those pairs.

**Failed alchemy** (no matching recipe, and did not explode):
- If diluted: always becomes POT_WATER
- Otherwise `rnd(8)`:
  - 1: POT_WATER
  - 2-3: POT_SICKNESS
  - 4: random potion type
  - 5-8: mixture evaporates (potion destroyed, "glows brightly")

**Result properties**:
- BUC: always uncursed
- Diluted: always yes (unless result is POT_WATER)
- dknown: cleared if Blind or Hallucinating

### 7.4 Unicorn Horn / Amethyst Dipping

Dipping a unicorn horn or amethyst into a potion uses `mixtype()`:
- If a recipe exists: transforms the single potion into the result
- Result BUC: if result is POT_WATER, uncursed+undiluted; otherwise, cursed state = unicorn horn's cursed state
- Result is always `bknown = FALSE`

### 7.5 Dipping Lichen Corpse in Acid

- Cosmetic only: corpse "turns red/orange/wrinkled around the edges"
- Potion NOT consumed

### 7.6 Poisoning Weapons

- Dipping a poisonable weapon into POT_SICKNESS: applies poison coating (`opoisoned = TRUE`)
- Dipping a poisoned weapon into POT_HEALING/EXTRA_HEALING/FULL_HEALING: removes poison coating

### 7.7 Acid Corrosion

- Dipping anything into POT_ACID: attempts to corrode the item via `erode_obj()`

### 7.8 Oil Dipping

- **Lit oil**: fire damage to the dipped item
- **Cursed oil**: spills on hands, causes Glib for `d(2, 10)` additional turns
- **Non-cursed oil on weapon/weptool**:
  - If not rustprone/corrodeable, or is ammo, or has no erosion: cosmetic "oily sheen" message
  - Otherwise: reduces rust (`oeroded`) and/or corrosion (`oeroded2`) by 1 each
  - Always identifies POT_OIL

### 7.9 Lamp Filling

- Dipping an oil lamp or magic lamp into POT_OIL:
  - If either is lit: explosion `d(6, 6)` damage
  - Magic lamp with `spe == 0` (empty): converts to oil lamp
  - If lamp age > 1000: "lamp is full", potion not consumed
  - Otherwise: `lamp.age += (diluted ? 3 : 4) * potion.age / 2`, capped at 1500
  - Identifies POT_OIL

---

## 8. Potion Identification Rules

### 8.1 Automatic Identification (makeknown)

A potion is identified if:
- The potion has been `dknown` (seen up close) AND
- `potion_unkn == 0` (the effect was clearly perceptible)

If `potion_unkn > 0`: only `trycall()` is invoked (allows the player to name but not formally discover).

### 8.2 Per-Potion Identification Behavior

Many potions increment `potion_unkn` to prevent auto-identification:
- **Always unknown**: restore_ability (cursed still increments unkn), hallucination (if resistant), see_invisible (always), gain_ability (cursed), gain_level (cursed), monster_detection (blessed with no monsters), speed (if healing legs), acid (always, because holy water can mimic)
- **Identifiable by effect**: healing/extra_healing/full_healing (always clear effects), paralysis, sleeping, blindness, confusion, sickness, booze, enlightenment (non-cursed), polymorph, levitation, gain_energy, oil (when lit)

### 8.3 Vapor Identification

From `potionbreathe()`:
- `kn` flag is set for: paralysis, sleeping, blindness, invisibility
- If `kn` is set AND `obj->dknown`: `makeknown(obj->otyp)`
- Otherwise: `trycall(obj)`

---

## 9. Hallucination Effects on Potion Descriptions

Hallucination affects potions in the following ways:

### 9.1 Bottle Name

When a potion bottle is mentioned (e.g., "The bottle crashes on your head"):
- Normal: randomly chosen from {"bottle", "phial", "flagon", "carafe", "flask", "jar", "vial"}
- Hallucinating: randomly chosen from {"jug", "pitcher", "barrel", "tin", "bag", "box", "glass", "beaker", "tumbler", "vase", "flowerpot", "pan", "thingy", "mug", "teacup", "teapot", "keg", "bucket", "thermos", "amphora", "wineskin", "parcel", "bowl", "ampoule"}

### 9.2 Object Naming

The randomized description (e.g., "ruby", "milky") is NOT changed by hallucination. The base description `dn = OBJ_DESCR(objects[typ])` is always the true description. However, `dknown` may be cleared when mixing potions while hallucinating, preventing association.

### 9.3 In-Message Flavor Text

Various quaff messages change under hallucination:
- Booze: "dandelion wine" instead of "liquid fire"
- Ghost from bottle: random monster name instead of "ghost"
- Speed: "feeling normal" instead of "feeling strange" (polymorph)
- Acid: "tangy" instead of "sour"
- Confusion: "What a trippy feeling!" instead of "Huh, What?"
- Various other substitutions in make_confused, make_blinded, etc.

---

## 10. Special Potion Behaviors -- Precise Numerical Details

### 10.1 Healing Potions Summary Table

| Potion | HP Healed | Max HP Increase | Cure Sick | Cure Blind/Deaf |
|--------|-----------|----------------|-----------|-----------------|
| Healing (B) | 8 + d(6,4) = [14,32] | +1 | Yes | Yes |
| Healing (U) | 8 + d(4,4) = [12,24] | +1 | No | Yes |
| Healing (C) | 8 + d(2,4) = [10,16] | +0 | No | No |
| Extra Healing (B) | 16 + d(6,8) = [22,64] | +5 | Yes | Yes |
| Extra Healing (U) | 16 + d(4,8) = [20,48] | +2 | Yes | Yes |
| Extra Healing (C) | 16 + d(2,8) = [18,32] | +0 | No | Yes |
| Full Healing (any) | 400 | B:+8, U:+4, C:+0 | B/U:Yes, C:No | Yes |

Extra Healing and Full Healing always cure hallucination. Full Healing (blessed) restores one lost level.

### 10.2 Gain Energy Precise Formulas

```
num = d(blessed ? 3 : uncursed ? 2 : 1, 6)
if cursed: num = -num
u.uenmax += num         (clamped >= 0)
u.uen += 3 * num        (clamped to [0, u.uenmax])
```

### 10.3 Levitation Duration

```
Cursed:   timeout stays at 1 (set during float_up), no I_SPECIAL
Uncursed: incr_itimeout(&HLevitation, rn1(140, 10))  -- [10, 150) turns
Blessed:  incr_itimeout(&HLevitation, rn1(50, 250))  -- [250, 300) turns, I_SPECIAL
```

### 10.4 Invisibility Duration

```
Permanent chance (blessed only):
  if HInvis already set: 1/15 chance
  if HInvis not set:     1/30 chance

Temporary (when not getting permanent):
  Blessed:  d(3, 100) + 100  = [103, 400]
  Uncursed: d(6, 100) + 100  = [106, 700]
  Cursed:   d(9, 100) + 100  = [109, 1000]
```

---

## 11. Potion Effects on Objects (Dip Summary)

| Dip Into | Object Type | Effect |
|----------|-------------|--------|
| Holy water (blessed) | Cursed item | Uncurse |
| Holy water (blessed) | Uncursed item | Bless |
| Unholy water (cursed) | Blessed item | Unbless |
| Unholy water (cursed) | Uncursed item | Curse |
| Uncursed water | Any carried item | `water_damage()` (rust, dilute, etc.) |
| POT_POLYMORPH | Non-unpolyable item | Polymorphs to random item of same class |
| POT_SICKNESS | Poisonable weapon | Applies poison coating |
| POT_HEALING/EXTRA/FULL | Poisoned weapon | Removes poison coating |
| POT_ACID | Any | Corrodes if corrodeable |
| POT_OIL (unlit, non-cursed) | Rusty/corroded weapon | Reduces erosion by 1 |
| POT_OIL (unlit, cursed) | Any | Glib hands for d(2,10) turns |
| POT_OIL (lit) | Any | Fire damage |
| POT_OIL | Lamp | Refuels (see section 7.9) |
| Any potion | Another potion | Alchemy (see section 7.3) |

---

## 12. Breakage and Shattering

### 12.1 Break Chance (breaktest)

For potions (`POTION_CLASS`):
- First: `obj_resists(obj, 1, 99)` -- artifacts have a chance to resist
- If not artifact and glass: always breaks
- All potions return TRUE from `breaktest()` (they always break on hard impact)

### 12.2 breakobj() for Potions

When a potion breaks on the ground:
- If POT_OIL and lit: triggers `explode_oil()` -> `splatter_burning_oil()` -> explosion `d(diluted ? 3 : 4, 4)` fire damage
- If hero is adjacent (`next2u(x, y)`):
  - Not water and not wearing wet towel: smell/eye-watering message
  - Calls `potionbreathe(obj)` for vapor effects
- Monster breathing from broken potions is NOT implemented

---

## 13. Test Vectors

### TV1: Blessed Healing Potion (normal case)
- Input: quaff blessed POT_HEALING, hero HP=10/20, not sick, not blind
- dice roll d(6,4) = 15 (example)
- HP restored: 8 + 15 = 23
- New HP: min(10 + 23, 20) = 20, since 33 > 20, max HP increases: max = 20 + 1 = 21, current = 21
- Cures sickness: yes (blessed)
- Cures blindness: yes (blessed or uncursed)
- Output: HP = 21/21

### TV2: Cursed Healing Potion (boundary: max HP not increased)
- Input: quaff cursed POT_HEALING, hero HP=18/20, not sick, blind
- dice roll d(2,4) = 3 (example)
- HP restored: 8 + 3 = 11
- New HP: 18 + 11 = 29, exceeds max 20, so max HP += 0 (cursed), current = 20
- Cures blindness: no (cursed)
- Output: HP = 20/20, still blind

### TV3: Full Healing Blessed with Lost Levels
- Input: quaff blessed POT_FULL_HEALING, hero level 8, ulevelmax = 12
- HP healed: 400 (full heal)
- Max HP increase: +8
- Lost level restoration: ulevelmax becomes 11, pluslvl() brings hero to level 9
- Output: level = 9, ulevelmax = 11, HP fully healed with +8 max

### TV4: Gain Energy Cursed (edge: energy goes to 0)
- Input: quaff cursed POT_GAIN_ENERGY, u.uen = 3, u.uenmax = 5
- dice roll d(1,6) = 6
- num = -6
- u.uenmax = 5 + (-6) = -1, clamped to 0
- u.uen = 3 + 3*(-6) = 3 - 18 = -15, clamped to 0
- Output: u.uen = 0, u.uenmax = 0

### TV5: Djinni from Blessed Smoky Potion
- Input: quaff blessed potion with "smoky" description, djinn not genocided, 0 djinn born, rn2(13) = 0
- POTION_OCCUPANT_CHANCE(0) = 13, so 1/13 chance
- rn2(5) = 4 (initial roll)
- Blessed reroll: chance 4 -> rnd(4) = 2
- Outcome: case 2 = peaceful djinni
- Output: peaceful djinni appears, potion consumed, normal effects skipped

### TV6: Alchemy -- Healing + Speed = Extra Healing
- Input: dip 5 uncursed POT_HEALING into POT_SPEED
- mixtype returns POT_EXTRA_HEALING (magic = true)
- Stack limit: amt = rnd(min(5,8) - 2) + 2 = rnd(3) + 2 = [3, 5]
- Say amt = 4: 4 potions transform
- Explosion check: not cursed, not acid, not lit oil, rn2(10) = 3 (no explosion)
- Result: 4 diluted uncursed POT_EXTRA_HEALING
- Output: 4 diluted potions of extra healing (if not hallucinating/blind: dknown)

### TV7: Potion Paralysis with Free_action (boundary: resistance)
- Input: quaff cursed POT_PARALYSIS, hero has Free_action
- Output: "You stiffen momentarily." No paralysis, no DEX exercise

### TV8: Acid Cures Petrification (boundary)
- Input: quaff cursed POT_ACID while Stoned, hero has Acid_resistance
- Acid damage: 0 (acid resistant)
- fix_petrification(): Stoned is cured
- Output: petrification cured, no HP damage, potion_unkn incremented (not auto-identified)

### TV9: Holy Water on Undead Hero (damage case)
- Input: quaff blessed POT_WATER, hero is undead (vampire polymorph), has lycanthropy
- Effect: burns like acid, d(2,6) = 7, losehp(Maybe_Half_Phys(7))
- Lycanthropy cured (set_ulycn(NON_PM))
- Exercise CON (negative)
- Output: hero takes 7 damage (or 4 with half phys), lycanthropy cured

### TV10: Vapor from Full Healing (HP boundary)
- Input: POT_FULL_HEALING breaks at hero's feet, hero HP = 50/50 (poly HP = 10/10), not wearing wet towel
- Full Healing vapor: +1 poly HP (now 11/10 -> no, capped at mhmax; stays 10), +1 normal HP (51/50 -> stays 50)
- Wait -- the code says `if (u.mh < u.mhmax) u.mh++` and `if (u.uhp < u.uhpmax) u.uhp++`
- Since hero is at max: neither increments
- Falls through to extra healing: same check, still at max, no increment
- Falls through to healing: same check, still at max, no increment
- Cures blindness: yes (full healing always cures)
- Output: HP unchanged (already at max), blindness cured

### TV11: Alchemy Explosion (boundary: acid potion)
- Input: dip 1 uncursed POT_FRUIT_JUICE into POT_ACID
- dip_potion_explosion: obj->cursed=false, obj->otyp=POT_FRUIT_JUICE (not acid), not lit oil
- But potion being mixed INTO doesn't matter -- the explosion check is on `obj` (the dipped stack)
- Wait: re-reading code -- the useup(potion) happens BEFORE dip_potion_explosion(obj, ...)
- Actually: `dip_potion_explosion(obj, amt + rnd(9))` checks `obj->cursed || obj->otyp == POT_ACID || (obj->otyp == POT_OIL && obj->lamplit) || !rn2(10)`
- obj is POT_FRUIT_JUICE, not cursed, not acid, not oil -> falls to !rn2(10) = 10% chance
- If rn2(10) = 0: explosion, damage = 1 + rnd(9)
- Output: 10% chance of explosion for 2-10 damage; otherwise normal alchemy

### TV12: Diluted Alchemy Stack Limit (boundary)
- Input: dip 10 diluted POT_HEALING into POT_GAIN_LEVEL
- Diluted: amt forced to 2
- Only 2 potions transform into POT_EXTRA_HEALING (diluted)
- The remaining 8 stay as POT_HEALING
- Output: 2 diluted POT_EXTRA_HEALING, 8 diluted POT_HEALING remain

### TV13: Holy Water Creation via Prayer
- Input: pray on coaligned altar, prayer succeeds (p_type >= 3), 3 uncursed POT_WATER on tile, hero is not Blind
- water_prayer(TRUE) iterates all objects on tile
- All 3 potions: blessed=true, cursed=false, bknown=true
- Output: 3 blessed (holy) potions of water, hero knows BUC status

### TV14: Monster Quaffs Blessed POT_HEALING (boundary: overheal)
- Input: monster with mhp=10, mhpmax=15, quaffs blessed POT_HEALING
- dice roll d(8, 4) = 20
- healmon(mtmp, 20, 1): mhp + 20 = 30 > mhpmax + 1 = 16
- So: mhpmax = 15 + 1 = 16, mhp = 16
- Non-cursed: blindness cured
- Output: monster HP = 16/16, blindness cured

### TV15: Monster Quaffs Cursed POT_GAIN_LEVEL on Top Floor (boundary)
- Input: monster on dungeon level 1, no Amulet, quaffs cursed POT_GAIN_LEVEL
- Can_rise_up returns false (nothing above level 1 without Amulet)
- Monster "looks uneasy" (if visible)
- Potion consumed, no level change, no migration
- Output: potion consumed, monster unchanged

### TV16: Blessed Restore Ability with Multiple Lost Levels (boundary)
- Input: quaff blessed POT_RESTORE_ABILITY, u.ulevel = 5, u.ulevelmax = 14, all stats at max
- Stats: all ABASE == AMAX, no restoration needed
- Level restoration loop: do { pluslvl() } while (ulevel < ulevelmax && blessed)
- Iterations: 5->6, 6->7, ..., 13->14 (9 iterations)
- Output: hero level = 14, ulevelmax = 14, all 9 lost levels restored in one potion

### TV17: Levitation Cursed Head Bonk (boundary: no helmet)
- Input: quaff cursed POT_LEVITATION, not already levitating, has ceiling, no helmet (uarmh == NULL)
- float_up() triggers
- I_SPECIAL cleared
- Not on upstairs -> ceiling check
- dmg = rnd(10), say rnd(10) = 8
- losehp(Maybe_Half_Phys(8)) -- without half phys: 8 damage
- Output: hero levitating (timeout=1, no I_SPECIAL), takes 8 damage from ceiling, "You hit your head on the ceiling."

### TV18: Speed Potion Heals Wounded Legs (boundary: early return)
- Input: quaff uncursed POT_SPEED, hero has Wounded_legs, not cursed, not riding
- heal_legs(0) called
- potion_unkn incremented (effect not clearly "speed")
- Return immediately -- no speed_up() called, no intrinsic speed granted
- Output: legs healed, no speed boost, potion possibly not auto-identified

---

## 14. Holy Water Creation (Prayer on Altar)

Source: `src/pray.c`, function `water_prayer()`.

Holy (blessed) and unholy (cursed) water are created by praying while standing on a coaligned altar with potions of water placed on the altar tile.

### 14.1 Mechanism

The function `water_prayer(bless_water)` iterates over all objects on the hero's tile (`level.objects[u.ux][u.uy]`):

```
for each obj on tile:
    if obj.otyp == POT_WATER:
        if bless_water AND not already blessed:
            obj.blessed = true
            obj.cursed = false
        elif !bless_water AND not already cursed:
            obj.blessed = false
            obj.cursed = true
        obj.bknown = !(Blind || Hallucination)
```

Key properties:
- Converts the ENTIRE stack of water potions at once (all `quan` units)
- Multiple stacks of POT_WATER on the same tile are all affected
- Only POT_WATER is affected; other potions on the tile are ignored
- If the hero is not Blind and not Hallucinating, the bknown flag is set (player knows BUC status)

### 14.2 When water_prayer(TRUE) is Called (Blessing)

Called during a **successful coaligned prayer** when standing on a coaligned altar:

```
if p_type >= 3 (coaligned, pleased god):
    if on_altar():
        water_prayer(TRUE)   // bless water
```

### 14.3 When water_prayer(FALSE) is Called (Cursing)

Called during **failed or hostile prayer** scenarios:

- `p_type == 0` (god is angry, on cross-aligned altar): `water_prayer(FALSE)`
- `p_type == 1` (naughty, on cross-aligned altar): `water_prayer(FALSE)`
- `p_type == 2` (attempted positive prayer on cross-aligned altar): `water_prayer(FALSE)`

In these cases, water on the altar becomes unholy (cursed).

### 14.4 Visual Feedback

```
if !Blind AND changed > 0:
    "The potion(s) on the altar glow(s) light blue for a moment."   // blessed
    "The potion(s) on the altar glow(s) black for a moment."        // cursed
```

If other (non-water) potions are also on the tile, the message adjusts: "Some of the potions" or "One of the potions" to distinguish.

---

## 15. Monster Quaffing Behavior

Source: `src/muse.c`, functions `use_defensive()`, `use_misc()`, and related.

Monsters use potions in two contexts: defensive use (when injured) and miscellaneous use (tactical advantage).

### 15.1 Defensive Potion Use

Monsters check for healing potions when their HP is low. Priority order (highest first):

1. **POT_FULL_HEALING** -- checked first
2. **POT_EXTRA_HEALING** -- checked second
3. **POT_HEALING** -- checked third

Additionally, during general item scanning (later pass), the same potions are re-checked if earlier checks missed them.

#### Healing Formulas (Monster)

| Potion | HP Restored | Max HP Increase |
|--------|------------|-----------------|
| POT_HEALING | `d(6 + 2 * bcsign, 4)` | +1 |
| POT_EXTRA_HEALING | `d(6 + 2 * bcsign, 8)` | blessed: +5, else: +2 |
| POT_FULL_HEALING | `mhpmax` (full heal) | blessed: +8, else: +4 |

The `healmon(mtmp, amt, overheal)` function works as follows:
```
if mtmp.mhp + amt > mtmp.mhpmax + overheal:
    mtmp.mhpmax += overheal
    mtmp.mhp = mtmp.mhpmax
else:
    mtmp.mhp += amt
    if mtmp.mhp > mtmp.mhpmax:
        mtmp.mhpmax = mtmp.mhp
```

#### Blindness Curing (Monster)

- POT_HEALING: cures monster blindness if non-cursed
- POT_EXTRA_HEALING: always cures monster blindness
- POT_FULL_HEALING: always cures monster blindness (unless it is actually POT_SICKNESS used by Pestilence)

#### Pestilence Special Case

Pestilence treats POT_FULL_HEALING the same as other monsters, but uses POT_SICKNESS as its "full healing" equivalent (the potion is unblessed before use). This mirrors the player mechanic where healing harms Pestilence and sickness heals it.

### 15.2 Miscellaneous Potion Use

Monsters use these potions for tactical advantage:

#### POT_GAIN_LEVEL

- **Cursed**: if monster can rise through ceiling (`Can_rise_up`), migrates to level above
- **Non-cursed**: monster levels up via `grow_up()` (evolves to next form if applicable)
- Triggers: monsters use this proactively when able

#### POT_INVISIBILITY

- Monster quaffs to become invisible: `mon_set_minvis(mtmp, !cursed ? FALSE : TRUE)`
- **Cursed**: invisibility attempt fails (mon stays visible); aggravates nearby monsters
- Only used when monster is not already invisible

#### POT_SPEED

- Monster quaffs to gain permanent speed increase: `mon_adjust_speed(mtmp, 1, otmp)`
- Note: unlike the player (who gains temporary "very fast" plus potential permanent speed), monsters gain a permanent one-step speed increase
- Only used when monster is not already MFAST

#### POT_POLYMORPH

- Monster quaffs to polymorph: `newcham(mtmp, muse_newcham_mon(mtmp), NC_SHOW_MSG)`
- Used tactically when the monster decides transformation is advantageous

### 15.3 Monster Decision Logic

Monsters decide which items to use based on:
- **Exclusions**: animals, exploding attackers, mindless creatures, ghosts, and Keystone Kops cannot use items
- **Difficulty scaling**: higher-difficulty monsters have access to more varied defensive items
- Monsters prefer more powerful healing potions over weaker ones
- Monsters use `precheck()` to validate the item still exists before use

### 15.4 Monsters Throwing Potions at Hero

Monsters can throw potions at the hero via the standard ranged attack system (`thrwmu()` in `mthrowu.c`). When a thrown potion hits:
- `potionhit()` handles all effects (same function used for player-thrown potions)
- The `how` parameter is set to `POTHIT_MONST_THROW`
- Hero takes bottle impact damage + potion-specific effects + possible vapor inhalation

### 15.5 Random Defensive Item Generation

When monsters are generated with defensive items, potions are chosen by `rnd_defensive_item()`:

| Roll | Item |
|------|------|
| 3 | POT_HEALING |
| 4 | POT_EXTRA_HEALING |
| 5 | POT_FULL_HEALING (difficulty > 8) |

---

## 16. Pre-Quaff Preconditions

### 16.1 Strangulation Check

If the hero is Strangled: "If you can't breathe air, how can you drink liquid?" -- quaffing is blocked entirely.

### 16.2 Dungeon Feature Prompts

Before selecting an inventory potion, the game offers to interact with dungeon features (unless preceded by `m` prefix):
- **Fountain**: "Drink from the fountain?" -> `drinkfountain()`
- **Sink**: "Drink from the sink?" -> `drinksink()`
- **Underwater**: "Drink the water around you?" -> cosmetic only ("Do you know what lives in this water?")

### 16.3 Worn Potion Handling

If the selected potion is worn (wielded):
- If `quan > 1`: split off one unit, clear `owornmask` on it
- If `quan == 1`: `remove_worn_item()` first

The potion's `in_use` flag is set to prevent premature deallocation during polymorph-related item drops.

---

## 17. Dipping into Dungeon Features

### 17.1 Dipping into Fountains

Source: `src/fountain.c`, function `dipfountain()`.

Blocked if Levitating. Special cases first:

**Excalibur**: if dipping a long sword, hero level >= 5, `quan == 1`, not an artifact, and Excalibur doesn't already exist:
- Probability: Knight 1/6, others 1/30
- **Lawful**: sword becomes Excalibur (blessed, erodeproof, erosion cleared)
- **Non-lawful**: sword cursed, possible spe decrease, erodeproof removed

**Hands/gloves**: calls `wash_hands()`.

**Other items**: `water_damage()` then random effect (if not destroyed):

| Roll (rnd(30)) | Effect |
|-----------------|--------|
| 16 | Curse the item (if not already cursed) |
| 17-20 | Uncurse the item (if cursed); otherwise "feeling of loss" |
| 21 | Water demon appears |
| 22 | Water nymph appears |
| 23 | Snakes appear |
| 24 | Find a gem (if fountain not looted); else water gushes |
| 25 | Water gushes forth |
| 26-27 | Strange feeling (cosmetic) |
| 28 | Lose gold (bathing urge) |
| 1-15, 29-30 | No additional effect |

---

## 18. Notes and Anomalies

1. **POT_OIL fire vulnerability logic** [Possible surprise, not a bug]: `vulnerable = !Fire_resistance || Cold_resistance`. This means Cold_resistance makes you take MORE damage from drinking lit oil (4d4 instead of 2d4). This appears intentional -- cold-adapted creatures are extra-vulnerable to fire.

2. **Blessed polymorph potion control**: The control is `POLY_LOW_CTRL` which is a limited form of control. If the blessed potion transforms into something other than natural form, duration is capped at `rn2(15) + 10` = `[10, 25)` turns, much shorter than normal polymorph.

3. **Healing vapor fallthrough**: POT_FULL_HEALING falls through POT_EXTRA_HEALING falls through POT_HEALING in the vapor code. This means full healing vapors give +3 HP total (1 from each tier), extra healing gives +2, and plain healing gives +1. The blindness curing also cascades: full always cures, extra cures if non-cursed, healing cures if blessed.

4. **Monster paralysis duration**: Comment in code says "really should be rnd(5) for consistency with players breathing potions, but..." -- the actual duration is `rnd(25)`, much longer than the vapor paralysis of `rnd(5)` on the player. [Intentional gameplay choice, noted as inconsistency in source]

5. **Sickness potion cures hallucination**: Drinking POT_SICKNESS always clears hallucination regardless of BUC status. This is a useful identification trick.

6. **Monster detection blessed timeout diminishing returns**: Once the Detect_monsters timeout reaches 300+, blessed potions only add 1 turn instead of the normal `rn2(100) + 100`.

7. **Alchemy smock explosion protection**: Wearing an alchemy smock reduces the random explosion chance from 10% (`!rn2(10)`) to 3.3% (`!rn2(30)`), but does NOT prevent explosions from cursed potions, acid, or lit oil.

8. **Restore ability vs. unicorn horn**: Potion of restore ability overrides Fixed_abil; unicorn horn does not. Additionally, the potion (but not the spell) can restore lost experience levels.

9. **Monster healing formula differs from player** [疑似 bug]: When a monster quaffs POT_HEALING, the formula is `d(6 + 2*bcsign, 4)` (blessed: d(8,4)=[8,32], uncursed: d(6,4)=[6,24], cursed: d(4,4)=[4,16]). The player formula is `8 + d(4 + 2*bcsign, 4)` (blessed: 8+d(6,4)=[14,32]). The constant +8 base is absent from the monster version, making monster healing strictly weaker for non-blessed potions. The same discrepancy exists for POT_EXTRA_HEALING. This may be intentional (monsters have different HP scales) or an oversight from when the formulas diverged.

10. **Cursed invisibility removes permanent** [疑似 bug]: Quaffing a cursed potion of invisibility first grants temporary invisibility (adding to the timeout), then specifically removes `FROMOUTSIDE` permanent invisibility. This means a cursed potion can permanently downgrade a player who had gained permanent invisibility from a blessed potion. The granting-then-removing sequence in the same function seems potentially unintended -- the temporary duration is added before the permanent flag is stripped.

11. **POT_SICKNESS damage includes `fromsink` check**: The damage message checks `otmp->fromsink` to determine the kill message prefix (KILLED_BY vs KILLED_BY_AN). The `fromsink` field is an alias for `corpsenm`. For a normal potion this would be 0, but if a potion were somehow created from a sink interaction with `fromsink` set, the damage formula is identical. The `5 * !!(otmp->cursed)` bonus damage applies in both cases.

12. **Blessed restore ability level restoration is unbounded**: A single blessed potion of restore ability restores ALL lost levels via a `do { pluslvl() } while (ulevel < ulevelmax)` loop. In contrast, blessed full healing restores only ONE level. This makes blessed restore ability strictly superior for level drain recovery, which may be a deliberate design choice to differentiate the potions.

13. **Alchemy dip stack splitting can orphan potions**: When dipping a large stack of potions, the code splits off `amt` potions, transforms them, then calls `freeinv(obj); hold_potion(obj, ...)`. If the hero is Fumbling, `hold_potion()` may drop the transformed potions. The remaining untransformed stack stays in inventory normally. This interaction is documented in comments but could surprise Rust reimplementors.
