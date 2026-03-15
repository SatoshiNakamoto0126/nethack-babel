# Scroll Effects Mechanism Spec

Source: `src/read.c`, `src/wield.c` (chwepon), `src/pickup.c` (scare monster pickup),
`src/teleport.c` (scrolltele/level_tele), `src/do_wear.c` (some_armor),
`src/invent.c` (identify_pack), `src/region.c` (create_gas_cloud),
`include/objects.h` (scroll definitions), `include/obj.h` (SPE_LIM, spe semantics)

---

## 1. Scroll Types and Base Probabilities

| Scroll               | Prob | Cost | Enum                  |
|----------------------|------|------|-----------------------|
| enchant armor        |   63 |   80 | SCR_ENCHANT_ARMOR     |
| destroy armor        |   45 |  100 | SCR_DESTROY_ARMOR     |
| confuse monster      |   53 |  100 | SCR_CONFUSE_MONSTER   |
| scare monster        |   35 |  100 | SCR_SCARE_MONSTER     |
| remove curse         |   65 |   80 | SCR_REMOVE_CURSE      |
| enchant weapon       |   80 |   60 | SCR_ENCHANT_WEAPON    |
| create monster       |   45 |  200 | SCR_CREATE_MONSTER    |
| taming               |   15 |  200 | SCR_TAMING            |
| genocide             |   15 |  300 | SCR_GENOCIDE          |
| light                |   90 |   50 | SCR_LIGHT             |
| teleportation        |   55 |  100 | SCR_TELEPORTATION     |
| gold detection       |   33 |  100 | SCR_GOLD_DETECTION    |
| food detection       |   25 |  100 | SCR_FOOD_DETECTION    |
| identify             |  180 |   20 | SCR_IDENTIFY          |
| magic mapping        |   45 |  100 | SCR_MAGIC_MAPPING     |
| amnesia              |   35 |  200 | SCR_AMNESIA           |
| fire                 |   30 |  100 | SCR_FIRE              |
| earth                |   18 |  200 | SCR_EARTH             |
| punishment           |   15 |  300 | SCR_PUNISHMENT        |
| charging             |   15 |  300 | SCR_CHARGING          |
| stinking cloud       |   15 |  300 | SCR_STINKING_CLOUD    |
| mail                 |    0 |    0 | SCR_MAIL              |
| blank paper          |   28 |   60 | SCR_BLANK_PAPER       |

All scrolls weigh 5 aum, material PAPER, `oc_magic=1` except mail and blank paper.

---

## 2. Reading Prerequisites and Literacy

### 2.1 Literacy Conduct

Reading a scroll (except blank paper and Book of the Dead) increments
`u.uconduct.literate`. First violation triggers a livelog entry.

### 2.2 Blindness

When blind, the hero can still read scrolls IF `scroll->dknown` is true (the
description has been seen previously). The hero "pronounces/cogitates the
formula" from memory. Spellbooks (except Book of the Dead) cannot be read blind.

```
if Blind and scroll.oclass == SCROLL_CLASS:
    if not scroll.dknown:
        "Being blind, you cannot read the formula on the scroll."
        -> no effect, no time consumed
    else:
        allowed (recite from memory)
```

### 2.3 Confusion

Reading while confused is always allowed. It produces alternate effects for many
scrolls. The hero "mispronounces" (or "misunderstands" if mute) the magic words.

### 2.4 Can-Chant Check

If the hero cannot chant (e.g., polymorphed into a form without speech), the
word "pronounce" is replaced with "cogitate" in messages. This is cosmetic only;
reading still works.

### 2.5 Scroll Consumption

Most scrolls are consumed (`useup()`) after reading. Exceptions where the scroll
is consumed early or handled specially:
- SCR_FIRE: consumed before explosion
- SCR_CHARGING: consumed before item selection prompt
- SCR_IDENTIFY (scroll form): consumed before identification
- SCR_REMOVE_CURSE (cursed): "disintegrates" message, the `nodisappear` flag
  suppresses "it disappears" for this case and for SCR_FIRE

### 2.6 Wisdom Exercise

Reading any magical scroll (`oc_magic == 1`) exercises Wisdom: `exercise(A_WIS, TRUE)`.

---

## 3. Individual Scroll Effects

### 3.1 SCR_ENCHANT_ARMOR

**Target Selection**: `some_armor(&youmonst)` selects a random worn armor piece.
Priority order: cloak > body armor > shirt, then 1/4 chance each to switch to
helm, gloves, boots, or shield.

#### 3.1.1 Normal (uncursed, not confused)

Enchantment amount calculation:

```
s = spe (if cursed scroll: -spe)

// Evaporation check (before enchanting)
if s > (special_armor ? 5 : 3) and rn2(s) != 0:
    armor evaporates (destroyed)
    return

// Clamp extreme negative
if s < -100: s = -100

// Base enchantment power
s = (4 - s) / 2    // integer division

// Bonuses
if special_armor: s += 1    // elven armor or wizard's cornuthaum
if not oc_magic:  s += 1    // mundane armor is easier to enchant
if blessed:       s += 1

// Minimum guarantee
if s <= 0:
    s = 0
    if spe > 0 and rn2(spe) == 0: s = 1
else:
    s = rnd(s)    // 1..s

// Cap
if s > 11: s = 11

if cursed_scroll: s = -s
```

Additional effects of the scroll's BUC:
- Blessed scroll: `bless(armor)` if not already blessed
- Cursed scroll: `curse(armor)` if not already cursed
- Uncursed scroll: `uncurse(armor)` if currently cursed

Dragon scales special case: if `s >= 0` and armor is dragon scales (not mail),
the scales merge into dragon scale mail. Blessed scroll also increments spe by 1
and blesses the mail.

**Warning vibration**: After enchanting, if `spe > (special_armor ? 5 : 3)` and
`(special_armor || !rn2(7))`, the armor "suddenly vibrates" as a warning.

#### 3.1.2 Confused Reading

Sets or removes `oerodeproof` on the armor piece:
- Blessed/uncursed scroll: sets `oerodeproof = 1` (rustproofing/fireproofing),
  also repairs all erosion (`oeroded = oeroded2 = 0`)
- Cursed scroll: sets `oerodeproof = 0` (removes protection)

#### 3.1.3 No Armor

If no armor worn: `strange_feeling()` -> "Your skin glows then fades."
(or "warm" if blind). Exercises CON and STR (positive if not cursed).

---

### 3.2 SCR_ENCHANT_WEAPON

**Target**: `uwep` (wielded weapon). Must be weapon class or weptool.

#### 3.2.1 Normal Reading

```
amount =
    if cursed_scroll: -1
    else if no uwep:  1
    else if uwep.spe >= 9: (rn2(uwep.spe) == 0) ? 1 : 0
    else if blessed:  rnd(3 - uwep.spe / 3)
    else:             1
```

This amount is passed to `chwepon()`.

**chwepon() logic**:

1. No weapon or non-weapon wielded:
   - If `amount >= 0` and wielding cursed item that welds: uncurse it
   - Otherwise: `strange_feeling()`, "Your hands twitch/itch"
   - Return 0 (scroll not used up in the normal sense)

2. Worm tooth + positive amount: transforms to crysknife, uncurses

3. Crysknife + negative amount: transforms to worm tooth

4. Artifact protection: if `amount < 0` and artifact with name restriction,
   only "faintly glows" -- no actual change

5. **Evaporation check**:
   ```
   if (spe > 5 and amount >= 0) or (spe < -5 and amount < 0):
       if rn2(3) != 0:   // 2/3 chance to evaporate
           weapon evaporates (destroyed with useupall)
           return
   ```

6. Apply enchantment: `uwep->spe += amount`
   - If `amount > 0` and cursed: uncurse weapon

7. Magicbane warning: if wielding Magicbane and `spe >= 0`, hand "itches/flinches"

8. **Warning vibration**: if `spe > 5` and `(is_elven_weapon || oartifact || !rn2(7))`:
   "suddenly vibrates unexpectedly"

#### 3.2.2 Confused Reading

Sets or removes `oerodeproof` on `uwep` (same logic as enchant armor confused):
- Blessed/uncursed scroll: `oerodeproof = 1`, repairs erosion
- Cursed scroll: `oerodeproof = 0`

Requires `uwep` to exist and `erosion_matters(uwep)` and not armor class.

---

### 3.3 SCR_IDENTIFY

#### 3.3.1 Scroll Form

The scroll is consumed (`useup()`) **before** identification proceeds.

**Confused or (cursed and not already known)**:
- "You identify this as an identify scroll." -- no items identified.
- If not already known, learn the scroll type.
- Return immediately.

**Normal identification count**:
```
cval = 1                                    // default: identify 1 item

if blessed or (!cursed and rn2(5) == 0):    // blessed always; uncursed 20% chance
    cval = rn2(5)                           // 0..4; 0 means identify ALL
    if cval == 1 and blessed and Luck > 0:
        cval = 2                            // blessed with positive luck: at least 2

identify_pack(cval)
```

Where `identify_pack(0)` means identify everything, `identify_pack(n)` identifies
up to n items (player chooses which).

#### Summary Table

| BUC     | Confused | Items Identified                    |
|---------|----------|-------------------------------------|
| Blessed | No       | rn2(5): 0=all, 1->2 if Luck>0, 2-4 |
| Uncursed| No       | 80%: 1 item; 20%: rn2(5) as above   |
| Cursed  | No       | 0 items (identifies self if unknown) |
| Any     | Yes      | 0 items (identifies self)            |

[疑似 bug] Cursed scroll when already known: the code checks
`scursed && !already_known` for the self-identification-only path. If the scroll
is already known AND cursed AND not confused, it falls through to the normal
identification logic with `cval = 1`. This means a known cursed scroll still
identifies 1 item, which seems unintended -- the cursed effect should arguably
always be weaker.

---

### 3.4 SCR_REMOVE_CURSE

#### 3.4.1 Cursed Scroll

"The scroll disintegrates." No curse removal. The `nodisappear` flag is set so
the earlier "it disappears" message is suppressed.

#### 3.4.2 Uncursed Scroll (not confused)

Uncurses **worn/wielded** items only:
- Items with non-zero `owornmask` (excluding W_BALL, W_ART, W_ARTI)
- Special handling for uswapwep (only if twoweaponing) and uquiver (only if
  mergeable weapon ammo, or gem when slinging)
- Also: loadstones, leashed pets
- Steed's saddle (handled separately from inventory; see 3.4.4)

#### 3.4.3 Blessed Scroll (not confused)

Uncurses **ALL** inventory items (same conditions as uncursed, but `sblessed`
flag bypasses the worn-only requirement).

#### 3.4.4 Saddle Handling (non-cursed scrolls)

When riding, the steed's saddle is treated as if part of the hero's inventory
for remove curse purposes. The saddle has its own branch of logic
(read.c:1529-1545), separate from the main inventory loop:

- **Confused**: `blessorcurse(saddle, 2)`, `bknown = 0` (same as inventory items)
- **Not confused, saddle is cursed**: `uncurse(saddle)`, saddle glows "amber"
  (visible only if not Blind; `bknown` set to 1 unless Hallucinating)

This saddle handling only applies to non-cursed scrolls (it is inside the
`else` branch of the `if (scursed)` check).

#### 3.4.5 Confused Reading

For each eligible item (same scope as blessed/uncursed):
```
blessorcurse(obj, 2)
```
This function: if object is already blessed or cursed, no change. If uncursed:
50% chance to trigger, then 50/50 bless/curse.

Also sets `obj->bknown = 0` for each affected item (lose BUC knowledge).

#### 3.4.6 Punishment Removal (all BUC states, not confused)

After the cursed/non-cursed item processing, the following checks apply
regardless of BUC status (read.c:1547-1552):

```
if Punished and not confused:
    unpunish()       // remove ball and chain
if trapped in buried ball:
    buried_ball_to_freedom()
    "The clasp on your <leg> vanishes."
```

This means **all three BUC states** (blessed, uncursed, AND cursed) remove
punishment when the scroll is read without confusion. Even a cursed scroll
that "disintegrates" without uncursing any items will still free the hero
from punishment.

---

### 3.5 SCR_TELEPORTATION

| Condition              | Effect                                       |
|------------------------|----------------------------------------------|
| Confused OR Cursed     | `level_tele()` -- level teleportation          |
| Blessed (not confused) | Controlled teleportation (choose destination)  |
| Uncursed, not confused | Random teleportation; blessed gives control    |

Teleport control (`Teleport_control` intrinsic or blessed scroll, and not stunned)
allows choosing destination.

On no-teleport levels (Stronghold, Vlad's Tower): "A mysterious force prevents
you from teleporting!" -- scroll is identified.

Amulet of Yendor or on Tower level: 1/3 chance "You feel disoriented" and
teleport fails.

---

### 3.6 SCR_GENOCIDE

```
if blessed:
    do_class_genocide()    // genocide an entire monster class
else:
    do_genocide(flags)
    // flags = (!cursed) | (2 * !!Confusion)
    // bit 0 (REALLY): set if uncursed (actual genocide)
    // bit 1 (PLAYER): set if confused (forced self-genocide)
```

#### 3.6.1 Blessed: Class Genocide

Player names a monster class (letter symbol). All genocidable members of that
class are wiped out. Self-genocide occurs if the class matches hero's role or
race monster.

#### 3.6.2 Uncursed: Species Genocide

Player names a specific monster type. That species is genocided (G_GENOD flag
set, corpse generation suppressed). Self-genocide if own role/race.

Immune monsters (no G_GENO flag): "No, mortal! That will not be done."

#### 3.6.3 Cursed: Reverse Genocide

`how & REALLY` is 0 (bit 0 not set), so instead of genociding, **creates**
monsters of the named type:

```
count = rn1(3, 4)    // 4..6 monsters
for i in 0..count:
    makemon(ptr, u.ux, u.uy, NO_MINVENT | MM_NOMSG)
```

If player tries to decline (escape/none) with a cursed scroll, a random monster
is chosen instead (`rndmonst()`).

#### 3.6.4 Confused + Uncursed

`flags = REALLY | PLAYER` (bits 0 and 1 both set). This forces genocide of the
hero's own monster type. Death message: "genocidal confusion".

#### 3.6.5 Confused + Cursed

`flags = 0 | PLAYER` = `PLAYER` only. The PLAYER bit is set (confused) but
REALLY is not (cursed). The code path: `how & PLAYER` is true, so hero's own
type is selected, but `how & REALLY` is false, so it creates monsters of the
hero's type rather than genociding. Result: reverse-genocides your own species.

#### 3.6.6 Confused + Blessed

`do_class_genocide()` -- confusion does not affect blessed genocide. Class
genocide proceeds normally.

The scroll always identifies itself: "You have found a scroll of genocide!"

---

### 3.7 SCR_SCARE_MONSTER

#### 3.7.1 Read from Inventory

For each monster at a position the hero can see (`cansee(mtmp->mx, mtmp->my)`):
- **Confused or cursed**: wakes the monster, removes flee/frozen/sleeping
- **Normal**: if monster fails magic resistance check, `monflee(mtmp, 0, FALSE, FALSE)`

Message: "You hear maniacal laughter" (normal) or "sad wailing" (confused/cursed).

#### 3.7.2 On the Floor (passive effect)

When a scare monster scroll is on the floor, monsters will not step on that
square (checked in movement AI). This is the primary tactical use.

#### 3.7.3 Pick-Up Degradation (src/pickup.c)

The scare monster scroll has a unique degradation system using `spe`:

```
on pickup attempt:
    if blessed:
        unbless(obj)           // becomes uncursed, spe unchanged
    else if spe == 0 and not cursed:
        obj.spe = 1            // mark as "has been picked up"
    else:
        // spe != 0 (was picked up before) OR cursed
        "The scroll turns to dust as you pick it up."
        scroll is destroyed
```

The `spe` field tracks pickup history:
- `spe = 0`: never picked up (fresh)
- `spe = 1`: picked up once (will crumble on next pickup)

Blessed scrolls get one extra pickup: first pickup removes blessed status, second
sets `spe = 1`, third crumbles.

**Cursed scrolls**: always crumble on first pickup attempt (fall through to the
else branch because `cursed` is true).

#### 3.7.4 Lift Attempt Interaction

In `lift_object()` (line 1792), if the player declines to pick up (answers 'n'
to weight warning), `obj->spe` is set to 0:

```c
if (obj->otyp == SCR_SCARE_MONSTER && result <= 0 && !container)
    obj->spe = 0;
```

[疑似 bug] This means that answering 'n' to "Continue?" when the scroll is too
heavy will set `spe = 0`, which actually *refreshes* a scroll that had `spe = 1`
(previously picked up), making it survive one more pickup cycle. A scroll that
was already picked up once and dropped can be "renewed" by attempting to pick it
up when overburdened and declining.

---

### 3.8 SCR_CONFUSE_MONSTER

#### 3.8.1 Non-human or Cursed

Confuses the reader: `make_confused(HConfusion + rnd(100), FALSE)`.

#### 3.8.2 Confused Reading (not blessed, human)

Purple glow on hands + confusion: `HConfusion + rnd(100)`.

#### 3.8.3 Confused Reading (blessed, human)

Red glow around head + **cures** confusion: `make_confused(0, TRUE)`.

#### 3.8.4 Normal (human, not confused, not cursed)

Enchants hands to confuse the next monster touched:

```
incr = 3 (scroll) or 0 (spell)

if not blessed:
    incr += rnd(2)          // 4-5 total for scroll
else:
    incr += rn1(8, 2)       // 5-12 total for scroll

if u.umconf >= 40:
    incr = 1                // diminishing returns

u.umconf += incr
```

The `u.umconf` counter decrements by 1 each time a monster is confused by touch.

---

### 3.9 SCR_LIGHT

#### 3.9.1 Normal (not confused)

- **Blessed/uncursed**: `litroom(TRUE, sobj)` -- lights the area.
  Blessed uses radius 9, uncursed uses radius 5.
  Also calls `lightdamage(sobj, TRUE, 5)` for gremlin self-damage if hero
  is polymorphed into gremlin.
- **Cursed**: `litroom(FALSE, sobj)` -- darkens the area.
  Snuffs carried light sources. May curse artifact lights (e.g. Sunsword).

#### 3.9.2 Confused

Creates tame, cancelled light monsters around the hero:

```
type = cursed ? PM_BLACK_LIGHT : PM_YELLOW_LIGHT
count = rn1(2, 3) + (blessed * 2)    // 3-4 normal, 5-6 blessed
```

If that monster type is extinct/genocided: "Tiny lights sparkle in the air
momentarily."

The lights are created with `MM_EDOG` (tame), `NO_MINVENT`, and `mcan = TRUE`
(cancelled, so they won't explode).

---

### 3.10 SCR_CHARGING

#### 3.10.1 Normal (not confused)

Identifies itself ("This is a charging scroll."), is consumed, then prompts
player to select an item to charge.

Calls `recharge(obj, bcsign)`:
- Cursed scroll: `curse_bless = -1`
- Uncursed scroll: `curse_bless = 0`
- Blessed scroll: `curse_bless = +1`

**Wand recharging** (see `recharge()` in read.c):

Charge limit by type:
- Wand of wishing: lim = 1
- Directional wands: lim = 8
- Non-directional wands: lim = 15

Explosion risk:
```
n = recharged_count
if n > 0 and (wand_of_wishing or n^3 > rn2(343)):
    wand explodes, damage = d(spe + rnd(lim), die_size_by_wand_type)
```

Cumulative explosion odds by recharge count:
0->0%, 1->0.29%, 2->2.33%, 3->7.87%, 4->18.66%, 5->36.44%, 6->62.97%, 7->100%.

Charge amount (if no explosion):
```
if cursed:
    stripspe(obj)   // set charges to 0 (blessed wands resist: nothing happens)
else:
    n = (lim == 1) ? 1 : rn1(5, lim + 1 - 5)    // rn2(5) + (lim - 4)
    if not blessed:
        n = rnd(n)            // 1..n
    if spe < n: spe = n      // set to n
    else: spe += 1            // increment by 1
```

Wand of wishing special: if spe > 3 after recharge, wand explodes.

**Ring recharging**:
```
s = blessed ? rnd(3) : cursed ? -rnd(2) : 1

// Explosion check (before applying s)
if spe > rn2(7) or spe <= -5:
    ring explodes, damage = rnd(3 * abs(spe))
else:
    spe += s
```

**Tool recharging** varies by tool type; see `recharge()` for full details.

#### 3.10.2 Confused Reading

Recharges the hero's energy (Pw):
```
if cursed:
    u.uen = 0                          // drain all energy
else:
    gain = d(blessed ? 6 : 4, 4)      // 6d4 or 4d4
    u.uen += gain
    if u.uen > u.uenmax:
        u.uenmax = u.uen               // raise maximum
    else:
        u.uen = u.uenmax               // restore to max
```

---

### 3.11 SCR_MAGIC_MAPPING

#### 3.11.1 Normal (scroll form)

On a no-map level (`level.flags.nommap`): "Your mind is filled with crazy lines!"
Causes confusion for `rnd(30)` turns. No mapping.

Otherwise:
- **Blessed**: also reveals secret doors (converts SDOOR to DOOR). Then
  `do_mapping()` which also reveals secret passages.
- **Uncursed**: standard `do_mapping()`.
- **Cursed** (not confused): maps the level but with scrambled details --
  temporarily sets `HConfusion = 1` during mapping, then restores. "Unfortunately,
  you can't grasp the details."

#### 3.11.2 Cursed + Confused Interaction

The scramble condition is: `cval = (scursed && !confused)`.

[疑似 bug] When cursed AND confused, `cval` is FALSE (confused negates the
cursed scramble), so mapping is NOT scrambled. A cursed confused scroll gives a
**perfect** map. A cursed non-confused scroll gives a scrambled map. The confused
state helps rather than hurts, which seems unintended.

---

### 3.12 SCR_FIRE

The scroll is consumed (`useup()`) before the effect.

#### 3.12.1 Normal

Damage formula:
```
cval = bcsign(sobj)     // -1, 0, or +1
dam = (2 * (rn1(3, 3) + 2 * cval) + 1) / 3
// rn1(3,3) = rn2(3) + 3 = 3..5
// blessed: (2*(5..7)+1)/3 = (11..15)/3 = 3..5
// uncursed: (2*(3..5)+1)/3 = (7..11)/3 = 2..3
// cursed: (2*(1..3)+1)/3 = (3..7)/3 = 1..2
```

- **Blessed**: `dam *= 5`, player chooses explosion center (within visible range,
  `distu(x,y) < 32` from hero). If chosen spot is out of range, center defaults
  to hero's position.
- **Uncursed/Cursed**: explosion centered on hero.

Explosion type: `ZT_SPELL_O_FIRE` (type 11), `EXPL_FIERY`. Fire damage in area.

Underwater: "The water around you vaporizes violently!" (still explodes).

#### 3.12.2 Confused

- Fire resistant: no damage, "pretty fire in your hands"
- Not fire resistant: 1 HP damage, "The scroll catches fire and you burn your hands"
- Underwater: "A little water around you vaporizes" (no damage)

---

### 3.13 SCR_EARTH

Requires: not Rogue level, has ceiling, not most Endgame levels (Earth level OK).

#### 3.13.1 Normal

- **Blessed**: boulders fall in all 8 surrounding squares (not on hero). If no
  boulders land (all squares obstructed), "But nothing else happens."
- **Uncursed**: boulders fall in 8 surrounding squares AND on hero.
- **Cursed**: boulder falls ONLY on hero (no surrounding boulders, since the
  surrounding-squares loop is guarded by `!scursed`).

Boulder eligibility per square: `isok(x,y) && !closed_door(x,y) &&
!IS_OBSTRUCTED(typ) && !IS_AIR(typ) && (x != u.ux || y != u.uy)`.

#### 3.13.2 Confused

Drops **rocks** instead of boulders:
- Rock quantity: `rn1(5, 2)` = 2..6 rocks per drop
- Boulder quantity: 1 per drop (non-confused)

Helmet protection: hard helmet caps damage at 2 for both boulders and rocks.

#### 3.13.3 Swallowed

"You hear rumbling." No actual boulders/rocks.

---

### 3.14 SCR_PUNISHMENT

- **Blessed or Confused**: "You feel guilty." No punishment.
- **Uncursed**: standard punishment -- ball and chain attached.
- **Cursed**: standard punishment + ball is heavier (see weight formula below).

Weight increase formula uses `cursed_levy = (sobj && sobj->cursed) ? 1 : 0`:

If already punished: "Your iron ball gets heavier."
`uball->owt += WT_IRON_BALL_INCR * (1 + cursed_levy)`
- Uncursed scroll (`cursed_levy=0`): weight increases by `WT_IRON_BALL_INCR * 1`
- Cursed scroll (`cursed_levy=1`): weight increases by `WT_IRON_BALL_INCR * 2`

If not yet punished: a new iron ball is created via `mksobj(HEAVY_IRON_BALL)`.
The initial ball weight is set by `mkobj` (not in `punish()`). The cursed scroll
does not affect the initial ball weight -- it only matters for the
already-punished case above.

If amorphous/whirly/unsolid form: ball and chain appear then fall away.

---

### 3.15 SCR_TAMING

#### 3.15.1 Normal

Affects monsters within distance 1 (adjacent + same square):
```
for each monster in radius 1 of hero (or u.ustuck if swallowed):
    maybe_tame(mtmp, sobj)
```

`maybe_tame()`:
- **Cursed scroll**: `setmangry(mtmp, FALSE)` -- angers the monster
- **Blessed/uncursed**: if monster fails `resist(mtmp, SCROLL_CLASS, 0, NOTELL)`,
  attempt `tamedog(mtmp, sobj, FALSE)`.
  Shopkeepers: `make_happy_shk()` called but not actually tamed.

#### 3.15.2 Confused

Same as normal but radius increases to 5:
```
bd = confused ? 5 : 1
for i in -bd..bd, j in -bd..bd:
    check (u.ux + i, u.uy + j)
```

This means the affected area is an 11x11 square (not circular).

---

### 3.16 SCR_CREATE_MONSTER

```
count = 1 + ((confused || cursed) ? 12 : 0)
           + ((blessed || rn2(73)) ? 0 : rnd(4))

type = confused ? PM_ACID_BLOB : random

create_critters(count, type, FALSE)
```

| Condition          | Count       | Type       |
|--------------------|-------------|------------|
| Normal uncursed    | 1 (rarely 2-5)| random   |
| Blessed            | 1           | random     |
| Cursed             | 13 (rarely more) | random |
| Confused           | 13 (rarely more) | acid blob |
| Confused + cursed  | 13 (rarely more) | acid blob |

The "rarely more" is `rnd(4)` with probability `1/73` (only when not blessed).

---

### 3.17 SCR_DESTROY_ARMOR

**Target**: `some_armor(&youmonst)` (same selection as enchant armor).

#### 3.17.1 Normal (not confused)

- **Blessed**: if wearing >1 armor piece, player chooses which to destroy.
  Identifies the scroll first: "This is a scroll of destroy armor!"
- **Uncursed**: destroys the randomly selected armor piece (`destroy_arm()`).
- **Cursed + armor is cursed**: does NOT destroy. Instead:
  `spe -= 1` (minimum -6), `adj_abon(otmp, -1)`, causes Stun for
  `(HStun & TIMEOUT) + rn1(10, 10)` turns.

If no armor: `strange_feeling()` -> "Your skin itches."

#### 3.17.2 Confused

Sets or removes `oerodeproof`:
- **Cursed scroll**: sets `oerodeproof = 1` (protects armor)
- **Blessed/uncursed scroll**: sets `oerodeproof = 0` (removes protection)

Note: this is the INVERSE of enchant armor's confused effect!

[疑似 bug] The confused reading of destroy armor with a cursed scroll *protects*
your armor (sets erodeproof), while the same scroll without confusion would
try to disenchant it. The cursed confused effect is strictly beneficial, which
seems unintended.

---

### 3.18 SCR_AMNESIA

Always sets `known = TRUE`.

```
forget(blessed ? 0 : ALL_SPELLS)
```

The `forget()` function always:
1. Resets felt ball & chain (`u.bc_felt = 0`)
2. Drains weapon skill training: `drain_weapon_skill(rnd(howmuch ? 5 : 3))`
   - Blessed: `rnd(3)` skills drained (no spells forgotten)
   - Non-blessed: `rnd(5)` skills drained + ALL spells forgotten
3. Resets `meverseen` for all monsters (affects sound-based identification)

BUC does NOT distinguish uncursed from cursed: both forget all spells and drain
`rnd(5)` skills.

Exercises Wisdom negatively: `exercise(A_WIS, FALSE)`.

---

### 3.19 SCR_GOLD_DETECTION

- **Normal (not confused, not cursed)**: `gold_detect(sobj)` -- reveals gold
  on the level.
- **Confused or Cursed**: `trap_detect(sobj)` -- reveals traps instead.

If detection finds nothing: `strange_feeling()` consumes the scroll.

---

### 3.20 SCR_FOOD_DETECTION

Always calls `food_detect(sobj)` regardless of BUC or confusion.

If detection finds nothing: `strange_feeling()` consumes the scroll.

Confusion/BUC effects are handled inside `food_detect()` itself (not in read.c).

---

### 3.21 SCR_STINKING_CLOUD

Always identifies itself: "You have found a scroll of stinking cloud!"

Player chooses center position. Must be visible and within squared distance < 32
from hero (`cansee(x,y) && distu(x,y) < 32`).

If position invalid: "The scroll crumbles with a whiff of rotten eggs."

Cloud creation:
```
cloudsize = 15 + 10 * bcsign(sobj)    // blessed=25, uncursed=15, cursed=5
damage = 8 + 4 * bcsign(sobj)         // blessed=12, uncursed=8, cursed=4
create_gas_cloud(x, y, cloudsize, damage)
```

| BUC     | Cloud Size | Damage per Turn |
|---------|-----------|-----------------|
| Blessed | 25        | 12              |
| Uncursed| 15        | 8               |
| Cursed  | 5         | 4               |

---

### 3.22 SCR_BLANK_PAPER

No effect. "This scroll seems to be blank." (or equivalent if blind).
Sets `known = TRUE`. Does not exercise Wisdom (not magical, `oc_magic = 0`).

---

### 3.23 SCR_MAIL

Not magical (`oc_magic = 0`), probability 0 (not randomly generated).
Conditional compilation: requires `MAIL_STRUCTURES` defined.

Behavior depends on `sobj->spe`:
- `spe = 0`: actual mail delivery (reads system mail via `readmail()`)
- `spe = 1`: bones/wished mail -- "junk mail" or "chain letter"
- `spe = 2`: written via magic marker -- "Postage Due" or "Return to Sender"

Odd/even of `o_id` determines which variant message appears.

Confused flag is forced to FALSE for mail scrolls. Reading mail while maintaining
illiterate conduct prompts for confirmation (if `spe == 0`).

---

## 4. Enchantment Limits Summary

| Item Type      | Soft Limit | Evaporation Trigger                          | Hard Limit  |
|----------------|-----------|----------------------------------------------|-------------|
| Weapon         | +5 / -5  | `(spe>5 && amt>=0) or (spe<-5 && amt<0)`: 2/3 | SPE_LIM=99 |
| Armor (normal) | +3        | `spe > 3 && rn2(spe) != 0`: (spe-1)/spe      | SPE_LIM=99 |
| Armor (special)| +5        | `spe > 5 && rn2(spe) != 0`: (spe-1)/spe      | SPE_LIM=99 |

"Special armor" = elven armor, or cornuthaum when hero is Wizard.

Evaporation probability for weapons at any spe beyond soft limit:
```
P(evaporate) = 2/3    // constant, independent of spe value
```

Evaporation probability for armor at spe `s` (where `s > threshold`):
```
P(evaporate) = (s-1)/s
spe=4 (normal armor): 3/4 = 75%
spe=5 (normal): 4/5 = 80%
spe=6 (special or normal): 5/6 = 83.3%
spe=7: 6/7 = 85.7%
...approaches 100% as spe increases
```

---

## 5. BUC Summary Table

| Scroll           | Blessed                    | Uncursed                 | Cursed                      |
|------------------|----------------------------|--------------------------|-----------------------------|
| Enchant Armor    | Bigger enchant, bless armor| Normal enchant, uncurse  | Disenchant, curse armor     |
| Enchant Weapon   | Bigger enchant             | +1 enchant               | -1 enchant                  |
| Identify         | rn2(5) items (0=all)       | 1 item (20%: rn2(5))    | Self-identify only (if new) |
| Remove Curse     | Uncurse ALL + unpunish     | Uncurse worn + unpunish  | Disintegrates + unpunish    |
| Teleportation    | Controlled teleport        | Random teleport          | Level teleport              |
| Genocide         | Class genocide             | Species genocide         | Reverse (create monsters)   |
| Light            | Radius 9 lit area          | Radius 5 lit area        | Darken area                 |
| Charging         | Best recharge              | Normal recharge          | Strip charges               |
| Magic Mapping    | Map + reveal secret doors  | Map level                | Scrambled map               |
| Fire             | 5x damage, choose center   | Normal damage at self    | Less damage at self         |
| Earth            | Boulders around (not on)   | Boulders around + on     | Boulder on hero only        |
| Punishment       | "You feel guilty" (safe)   | Standard punishment      | Heavier ball                |
| Scare Monster    | Standard scare             | Standard scare           | Wakes monsters instead      |
| Taming           | Standard tame attempt      | Standard tame attempt    | Angers monsters             |
| Create Monster   | 1 creature                 | 1 creature               | 13 creatures                |
| Destroy Armor    | Choose which to destroy    | Random armor destroyed   | Disenchant cursed armor     |
| Amnesia          | Skill drain only           | Spells + skill drain     | Spells + skill drain        |
| Confuse Monster  | Better hand enchantment    | Normal hand enchantment  | Confuse self                |
| Stinking Cloud   | Size 25, dmg 12            | Size 15, dmg 8           | Size 5, dmg 4               |
| Gold Detection   | Detect gold                | Detect gold              | Detect traps                |
| Food Detection   | Detect food                | Detect food              | Detect food                 |

---

## 6. Confused Reading Summary

| Scroll           | Confused Effect                                         |
|------------------|---------------------------------------------------------|
| Enchant Armor    | Toggle erodeproof (blessed/uncursed=set, cursed=remove) |
| Enchant Weapon   | Toggle erodeproof on weapon (same logic)                |
| Identify         | Self-identify only                                      |
| Remove Curse     | Randomly bless/curse worn items, lose BUC knowledge; NO unpunish (confused blocks it) |
| Teleportation    | Level teleport (same as cursed)                         |
| Genocide         | Forced self-genocide (with uncursed/blessed); reverse-self with cursed |
| Light            | Create tame cancelled light monsters                    |
| Charging         | Recharge hero's energy (Pw)                             |
| Magic Mapping    | (No special confused effect; but cancels cursed scramble)|
| Fire             | 1 HP self-damage (or no damage if fire resistant)       |
| Earth            | Rocks instead of boulders                               |
| Punishment       | "You feel guilty" (safe, same as blessed)               |
| Scare Monster    | Wake/embolden monsters (same as cursed)                 |
| Taming           | Larger radius (5 instead of 1)                          |
| Create Monster   | 13 acid blobs                                           |
| Destroy Armor    | Toggle erodeproof (cursed=set, uncursed/blessed=remove) |
| Amnesia          | (No special confused effect)                            |
| Gold Detection   | Detect traps (same as cursed)                           |
| Food Detection   | (No special confused effect in read.c)                  |
| Stinking Cloud   | (No special confused effect)                            |

---

## 7. Armor Selection Algorithm (`some_armor`)

```
fn some_armor(victim) -> Option<Obj>:
    // Priority: cloak > body armor > shirt
    result = uarmc  // cloak
    if result is None: result = uarm   // body armor
    if result is None: result = uarmu  // shirt

    // Each remaining slot has 1/4 chance to override
    if uarmh and (result is None or rn2(4) == 0): result = uarmh  // helm
    if uarmg and (result is None or rn2(4) == 0): result = uarmg  // gloves
    if uarmf and (result is None or rn2(4) == 0): result = uarmf  // boots
    if uarms and (result is None or rn2(4) == 0): result = uarms  // shield

    return result
```

If no armor piece is selected, functions that need armor call
`strange_feeling()`.

---

## 8. Random Number Functions Reference

- `rn2(x)`: uniform random in `[0, x)` (0-indexed)
- `rnd(x)`: uniform random in `[1, x]` (1-indexed)
- `rn1(x, y)`: `rn2(x) + y` = uniform random in `[y, x+y-1]`
- `d(n, s)`: sum of `n` rolls of `rnd(s)` (standard n-dice-of-s-sides)
- `rne(x)`: random with exponential-like distribution
- `bcsign(obj)`: -1 if cursed, +1 if blessed, 0 if uncursed

---

## 9. 测试向量

### 9.1 Scroll of Identify BUC Variations

| # | BUC     | Confused | already_known | Luck | Expected Behavior                              |
|---|---------|----------|---------------|------|------------------------------------------------|
| 1 | Blessed | No       | No            | +3   | Identify rn2(5) items; if rn2(5)==1, upgrade to 2 |
| 2 | Blessed | No       | Yes           | 0    | Identify rn2(5) items; cval=1 stays 1          |
| 3 | Blessed | No       | No            | -1   | Identify rn2(5) items; cval=1 stays 1 (Luck<=0)|
| 4 | Uncursed| No       | No            | 0    | 80%: identify 1; 20%: rn2(5) items             |
| 5 | Cursed  | No       | No            | 0    | Self-identify only, learn scroll type           |
| 6 | Cursed  | No       | Yes           | 0    | Identify 1 item (falls through!) [疑似 bug]     |
| 7 | Any     | Yes      | No            | 0    | Self-identify only                              |
| 8 | Blessed | Yes      | Yes           | 0    | Self-identify only (confused overrides)         |

### 9.2 Scroll of Enchant Armor Evaporation

| #  | spe | special_armor | Scroll BUC | rn2(s) result | Outcome                   |
|----|-----|---------------|------------|---------------|---------------------------|
| 9  | +3  | No (normal)   | Uncursed   | N/A           | No evap risk (s=3, not >3)|
| 10 | +4  | No            | Uncursed   | 0             | Survives (rn2(4)==0)      |
| 11 | +4  | No            | Uncursed   | 3             | Evaporates                |
| 12 | +5  | Yes (elven)   | Uncursed   | N/A           | No evap risk (s=5, not >5)|
| 13 | +6  | Yes           | Uncursed   | 0             | Survives (rn2(6)==0)      |
| 14 | +6  | Yes           | Uncursed   | 5             | Evaporates                |

### 9.3 Scroll of Enchant Weapon Evaporation

| #  | spe | Scroll BUC | rn2(3) result | Outcome       |
|----|-----|------------|---------------|---------------|
| 15 | +5  | Uncursed   | N/A           | No evap risk  |
| 16 | +6  | Uncursed   | 0             | Survives      |
| 17 | +6  | Uncursed   | 1             | Evaporates    |
| 18 | -6  | Cursed     | 0             | Survives      |
| 19 | -6  | Cursed     | 2             | Evaporates    |

### 9.4 Scare Monster Pickup Sequence

| #  | Initial State            | Action    | Result                       |
|----|--------------------------|-----------|------------------------------|
| 20 | blessed, spe=0           | pickup    | Becomes uncursed, spe=0      |
| 21 | uncursed, spe=0          | pickup    | spe becomes 1                |
| 22 | uncursed, spe=1          | pickup    | Turns to dust                |
| 23 | cursed, spe=0            | pickup    | Turns to dust                |
| 24 | blessed, spe=0           | pickup x3 | 1st: uncursed spe=0; 2nd: spe=1; 3rd: dust |

### 9.5 Boundary Conditions

| #  | Scenario                                             | Expected                              |
|----|------------------------------------------------------|---------------------------------------|
| 25 | Enchant armor, spe=99 (SPE_LIM)                     | cap_spe() prevents increase past 99   |
| 26 | Enchant armor, spe=-100, cursed scroll               | s clamped at -100 before formula      |
| 27 | Blessed identify, rn2(5)==0                           | cval=0, identify ALL items            |
| 28 | Cursed genocide, escape all 5 prompts                | rndmonst() chosen, those monsters created |
| 29 | Stinking cloud blessed: cloudsize=25, damage=12      | Exact values from formula             |
| 30 | Stinking cloud cursed: cloudsize=5, damage=4         | Exact values from formula             |
| 31 | Fire blessed, rn1(3,3)=5: dam=(2*(5+2)+1)/3=5, *5=25| dam=25                               |
| 32 | Fire cursed, rn1(3,3)=3: dam=(2*(3-2)+1)/3=1        | dam=1                                 |

### 9.6 Genocide Mode Flags

| #  | BUC     | Confused | flags value          | Effect                            |
|----|---------|----------|----------------------|-----------------------------------|
| 33 | Blessed | No       | (class genocide)     | Wipe entire monster class         |
| 34 | Blessed | Yes      | (class genocide)     | Same -- confusion doesn't affect  |
| 35 | Uncursed| No       | REALLY (=1)          | Species genocide                  |
| 36 | Uncursed| Yes      | REALLY\|PLAYER (=3)  | Self-genocide (kill hero)         |
| 37 | Cursed  | No       | 0                    | Reverse genocide (create monsters)|
| 38 | Cursed  | Yes      | PLAYER (=2)          | Reverse-genocide hero's own type  |

### 9.7 Remove Curse Punishment Removal

| #  | BUC     | Confused | Punished | Expected                                       |
|----|---------|----------|----------|------------------------------------------------|
| 43 | Blessed | No       | Yes      | Uncurse all inventory + unpunish               |
| 44 | Uncursed| No       | Yes      | Uncurse worn items + unpunish                  |
| 45 | Cursed  | No       | Yes      | Scroll disintegrates, no uncursing, but unpunish |
| 46 | Blessed | Yes      | Yes      | Randomly bless/curse items, NO unpunish (confused blocks) |
| 47 | Cursed  | Yes      | Yes      | Scroll disintegrates, NO unpunish (confused blocks) |
| 48 | Uncursed| No       | No       | Uncurse worn items, no punishment to remove     |

### 9.8 Confused Charging Energy Recharge

| #  | BUC     | Confused | u.uen before | u.uenmax | Expected After                  |
|----|---------|----------|-------------|----------|---------------------------------|
| 49 | Blessed | Yes      | 10          | 50       | uen=50 (restored to max)        |
| 50 | Blessed | Yes      | 45          | 50       | uen += 6d4; if >50 then uenmax raised |
| 51 | Cursed  | Yes      | 30          | 50       | uen=0                           |
| 52 | Uncursed| Yes      | 10          | 50       | uen=50 (restored to max)        |
