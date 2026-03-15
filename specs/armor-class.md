# NetHack 3.7 -- Armor & AC System Specification

Source: `src/do_wear.c`, `src/worn.c`, `src/mhitu.c`, `src/polyself.c`,
`src/spell.c`, `src/timeout.c`, `src/trap.c`, `include/objclass.h`,
`include/obj.h`, `include/prop.h`, `include/hack.h`, `include/objects.h`

---

## 1. AC Calculation Formula

### 1.1 Core Formula (`find_ac()` in `do_wear.c:2468`)

```
uac = base_ac
    - ARM_BONUS(body_armor)
    - ARM_BONUS(cloak)
    - ARM_BONUS(helmet)
    - ARM_BONUS(boots)
    - ARM_BONUS(shield)
    - ARM_BONUS(gloves)
    - ARM_BONUS(shirt)
    - ring_of_protection_left.spe       (only if otyp == RIN_PROTECTION)
    - ring_of_protection_right.spe      (only if otyp == RIN_PROTECTION)
    - 2                                 (if amulet == AMULET_OF_GUARDING)
    - u.ublessed                        (if HProtection has INTRINSIC flag)
    - u.uspellprot                      (from cast_protection spell)

if abs(uac) > 99:
    uac = sign(uac) * 99
```

- **base_ac** = `mons[u.umonnum].ac` -- the base AC of the hero's current form.
  For an unpolymorphed human, this is **10**. Other monster forms have different
  base ACs (e.g., a silver dragon has AC -1).
- Absent armor slots contribute nothing (null pointer check before each subtraction).

### 1.2 ARM_BONUS Macro (`hack.h:1532`)

```
ARM_BONUS(obj) = a_ac + obj.spe - min(greatest_erosion(obj), a_ac)
```

Where:
- `a_ac` = `objects[obj.otyp].a_ac` -- the base armor bonus stored in the object class definition.
  In `objects.h`, the ARMOR macro stores `10 - display_ac` as the `a_ac` value.
  Example: plate mail's display AC is 3, so `a_ac` = 10 - 3 = **7**.
- `obj.spe` = enchantment level (can be negative).
- `greatest_erosion(obj)` = `max(obj.oeroded, obj.oeroded2)` -- the worse of the
  two erosion counters (range 0..3 each).
- The `min(greatest_erosion, a_ac)` ensures erosion penalty never exceeds the base
  armor bonus. Erosion cannot make the base contribution negative; at worst it
  zeroes it out. Enchantment (`spe`) is NOT capped by erosion.

### 1.3 What Does NOT Affect AC

- **Dexterity**: Unlike D&D, dexterity has **no direct effect** on AC in NetHack.
  Gauntlets of dexterity modify `ABON(A_DEX)` which affects to-hit, but NOT AC.
- **Strength, luck, etc.**: No effect on AC.
- **Wielded weapons/weptools**: Do not contribute to AC.

### 1.4 AC_VALUE -- Randomized Negative AC (`hack.h:1544`)

When negative AC is used for hit determination, it is weakened randomly:

```
AC_VALUE(AC) = if AC >= 0: AC
               else:        -rnd(-AC)    // random 1..-AC, then negated
```

This means AC of -20 provides a random -1 to -20 contribution to the hit
check, NOT a flat -20. The hit check in `hitmu()` is:

```
tmp = AC_VALUE(u.uac) + 10 + monster_level
if hero is multi-turn-busy: tmp += 4
if hero is invisible and monster cannot perceive: tmp -= 2
if monster is trapped: tmp -= 2
if tmp <= 0: tmp = 1      // always at least 1% chance to hit
```

Then the monster rolls `rnd(20+i)` where `i` is the attack index (0 for first):
```
if tmp > rnd(20 + i):
    hit
else:
    miss
```

For weapon attacks, `hitval(weapon, &youmonst)` is added to `tmp` before the roll
and subtracted after.

### 1.5 Ranged Hit Determination (`thitu()` in `mthrowu.c:74`)

```
dieroll = rnd(20)
if u.uac + tlev <= dieroll:
    miss
else:
    hit
```

> [疑似 bug] `thitu()` uses the raw `u.uac` value, NOT `AC_VALUE(u.uac)`.
> This means negative AC provides **deterministic** protection against ranged
> attacks (every point of negative AC fully counts), whereas melee hit
> determination uses `AC_VALUE()` which randomly weakens negative AC to a value
> between -1 and -|AC|. As a result, AC -20 always provides a full -20 modifier
> against ranged attacks but only -1 to -20 (uniform random) against melee.
> This may be intentional (ranged attacks bypass some defense so AC is compensated)
> or it may be a long-standing inconsistency.

### 1.6 Spell/Ray Hit Determination (`zhitm_pre()` in `zap.c:4694`)

```
chance = rn2(20)                        // 0..19
if chance == 0:                         // 5% edge case
    return rnd(10) < ac + spell_bonus
ac = AC_VALUE(ac)                       // negative AC randomized
return (3 - chance) < ac + spell_bonus
```

### 1.7 Negative AC Damage Reduction (`mhitu.c:1200`)

After a melee hit lands, negative AC also reduces damage:

```
if damage > 0 and u.uac < 0:
    damage -= rnd(-u.uac)     // reduce by random 1..|u.uac|
    if damage < 1:
        damage = 1             // minimum 1 damage
```

Key details:
- Uses raw `u.uac`, **not** `AC_VALUE(u.uac)` -- the reduction is deterministic
  in the sense that it always uses the full AC value as the range ceiling.
- Applied **after** the hit has already landed (separate benefit from to-hit).
- Damage floor is 1 -- negative AC can never fully negate physical damage.
- After this reduction, `Half_physical_damage` may further halve the result.
- This reduction does NOT apply to `permdmg` (Death's life force drain).

Example: AC = -20, incoming damage = 8.
  Reduction = rnd(20), range 1..20.
  Result = max(8 - rnd(20), 1).
  With roll of 1: 7 damage. With roll >= 8: 1 damage.

### 1.8 AC Cap

```
AC_MAX = 99
```

Both positive and negative AC are capped at absolute value 99. Applied in
both `find_ac()` for the hero and `find_mac()` for monsters.

---

## 2. Equipment Slot System

### 2.1 Seven Armor Slots

| Slot        | Enum          | Bit Mask   | Global Pointer | Notes                           |
|-------------|---------------|------------|----------------|---------------------------------|
| Body armor  | `ARM_SUIT`=0  | `W_ARM`    | `uarm`         | Outermost "suit"                |
| Shield      | `ARM_SHIELD`=1| `W_ARMS`   | `uarms`        | Occupies one hand               |
| Helmet      | `ARM_HELM`=2  | `W_ARMH`   | `uarmh`        |                                 |
| Gloves      | `ARM_GLOVES`=3| `W_ARMG`   | `uarmg`        |                                 |
| Boots       | `ARM_BOOTS`=4 | `W_ARMF`   | `uarmf`        |                                 |
| Cloak       | `ARM_CLOAK`=5 | `W_ARMC`   | `uarmc`        | Covers suit and shirt           |
| Shirt       | `ARM_SHIRT`=6 | `W_ARMU`   | `uarmu`        | Under suit; Hawaiian / T-shirt  |

### 2.2 Non-Armor Equipment That Affects AC

| Item                  | AC Contribution           |
|-----------------------|---------------------------|
| Ring of protection    | `-spe` (each ring)        |
| Amulet of guarding    | `-2` (fixed, no erosion)  |

### 2.3 Other AC Sources

| Source                 | AC Contribution            | Details                             |
|------------------------|----------------------------|-------------------------------------|
| Divine protection      | `-u.ublessed`              | Only if `HProtection & INTRINSIC`   |
| Spell protection       | `-u.uspellprot`            | From `SPE_PROTECTION` spell         |

---

## 3. Wearing Constraints

### 3.1 Layering Order (Putting On)

The game enforces a strict inside-to-outside layering for three slots:

1. **Shirt** (`ARM_SHIRT`) -- innermost layer.
   Cannot be put on if **any** of: suit, cloak, or shirt already worn.
2. **Body armor** (`ARM_SUIT`) -- middle layer.
   Cannot be put on if cloak worn. Cannot be put on if another suit worn.
3. **Cloak** (`ARM_CLOAK`) -- outermost layer of the three.
   Cannot be put on if another cloak worn.

Other slots (helmet, gloves, boots, shield) have no layering dependencies
with each other or with the above three.

### 3.2 Single-Slot Occupancy

Each slot holds at most one item. Trying to wear a second item of the same
category results in "You are already wearing ___."

### 3.3 Body Form Restrictions (`canwearobj()`, `do_wear.c:2025`)

| Condition                           | Blocked Slots                          |
|-------------------------------------|----------------------------------------|
| `verysmall()` or `nohands()`        | All armor                              |
| `cantweararm()` -- breakarm/sliparm | Suit, cloak, shirt (with exceptions)   |
| Polymorphed with horns              | Non-flimsy helmets cannot be worn      |
| `slithy()` (serpentine body)        | Boots                                  |
| Centaur form                        | Boots                                  |
| Trapped (bear trap, in floor, etc.) | Boots                                  |
| `Glib` (slippery fingers)           | Gloves                                 |
| Wielding two-handed weapon          | Shield (cannot wear)                   |
| Two-weapon fighting active          | Shield (cannot wear)                   |
| Welded weapon (cursed, bimanual)    | Suit, shirt (cannot put on/take off)   |
| Welded weapon                       | Gloves (cannot put on)                 |

**Mummy wrapping exception**: `WrappingAllowed(ptr)` allows mummy wrapping
as a cloak for humanoids of size SMALL through HUGE, excluding noncorporeal,
centaurs, winged gargoyles, and mariliths.

### 3.4 Race-Based Restrictions

`racial_exception()` in `worn.c:1352`:
- Hobbits may wear elven armor (return 1 = acceptable).
- Otherwise, `cantweararm()` monsters that are not of matching race
  cannot wear suits.

---

## 4. Removal Constraints

### 4.1 Cursed Equipment

Cursed armor **cannot be voluntarily removed**. The `cursed()` function
(`do_wear.c:1888`) checks `otmp->cursed` for armor and `welded(otmp)` for
weapons. If cursed, the game prints "You can't. It is cursed." and the
removal fails. BUC status becomes known (`bknown = 1`).

### 4.2 Layering Order (Removal)

To remove an inner layer, outer layers must be removable first:

- **Removing suit**: If cloak is worn and **cursed**, cannot remove suit.
- **Removing shirt**: If cloak is worn and cursed, cannot remove. If suit is
  worn and cursed, cannot remove (even if cloak is not worn or removable).
- **Removing gloves**: If weapon is welded, cannot remove.
- **Removing ring**: If gloves are cursed, cannot remove. If weapon is
  welded on the ring's hand side (or bimanual), cannot remove.

### 4.3 Automated Takeoff Order (`takeoff_order[]`)

When using the `A` (take-off-all) command, items are removed in this
fixed priority order:

```
WORN_BLINDF, W_WEP, WORN_SHIELD, WORN_GLOVES, LEFT_RING,
RIGHT_RING, WORN_CLOAK, WORN_HELMET, WORN_AMUL, WORN_ARMOR,
WORN_SHIRT, WORN_BOOTS, W_SWAPWEP, W_QUIVER
```

This order ensures outer layers are removed before inner layers (cloak
before armor, armor before shirt).

---

## 5. Conferred Properties (oc_oprop and Special Handling)

### 5.1 Standard oc_oprop Properties

When armor is worn, its `oc_oprop` is applied as an extrinsic property
via `setworn()`. When removed, the property is cleared.

| Armor Item                     | oc_oprop                    |
|--------------------------------|-----------------------------|
| Elven cloak                    | `STEALTH`                   |
| Alchemy smock                  | `POISON_RES`                |
| Cloak of protection            | `PROTECTION` (extrinsic)    |
| Cloak of invisibility          | `INVIS`                     |
| Cloak of magic resistance      | `ANTIMAGIC`                 |
| Cloak of displacement          | `DISPLACED`                 |
| Cornuthaum (wizard)            | `CLAIRVOYANT`               |
| Helm of caution                | `WARNING`                   |
| Helm of telepathy              | `TELEPAT`                   |
| Speed boots                    | `FAST`                      |
| Water walking boots            | `WWALKING`                  |
| Jumping boots                  | `JUMPING`                   |
| Elven boots                    | `STEALTH`                   |
| Levitation boots               | `LEVITATION`                |
| Gauntlets of fumbling          | `FUMBLING`                  |
| Fumble boots                   | `FUMBLING`                  |
| Shield of reflection           | `REFLECTING`                |
| Gray DSM / gray dragon scales  | `ANTIMAGIC`                 |
| Silver DSM / silver scales     | `REFLECTING`                |
| Red DSM / red scales           | `FIRE_RES`                  |
| White DSM / white scales       | `COLD_RES`                  |
| Orange DSM / orange scales     | `SLEEP_RES`                 |
| Black DSM / black scales       | `DISINT_RES`                |
| Blue DSM / blue scales         | `SHOCK_RES`                 |
| Green DSM / green scales       | `POISON_RES`                |
| Yellow DSM / yellow scales     | `ACID_RES`                  |
| Gold DSM / gold scales         | 0 (light source; see below) |

### 5.2 Dragon Armor Special Handling (`dragon_armor_handling()`)

Dragon armor has additional effects beyond `oc_oprop`, applied via
`dragon_armor_handling()` in `do_wear.c:793`:

| Dragon Color | Extra Effect (beyond oc_oprop)           |
|--------------|------------------------------------------|
| Black        | Drain resistance (`EDrain_resistance`)   |
| Blue         | Speed (`EFast`)                          |
| Green        | Sickness resistance (`ESick_resistance`)  |
| Red          | Infravision (`EInfravision`)             |
| Gold         | Hallucination resistance (toggle)        |
| Orange       | Free action (`Free_action`)              |
| Yellow       | Stoning resistance (`EStone_resistance`) |
| White        | Slow digestion (`ESlow_digestion`)       |
| Gray         | (none beyond antimagic)                  |
| Silver       | (none beyond reflection)                 |

### 5.3 Attribute-Modifying Armor (`adj_abon()`)

| Item                      | Effect                                |
|---------------------------|---------------------------------------|
| Helm of brilliance        | `ABON(A_INT) += spe; ABON(A_WIS) += spe` |
| Gauntlets of dexterity    | `ABON(A_DEX) += spe`                 |
| Cornuthaum                | `ABON(A_CHA) += 1` if Wizard role, else `-1` |
| Gauntlets of power        | Modifies strength (handled in `attrib.c`) |

These attribute changes affect to-hit and other derived stats but NOT AC.

### 5.4 Property Blocking (`w_blocks()`)

Some worn items **block** properties:
- **Mummy wrapping** (as cloak): blocks `INVIS`
- **Cornuthaum** (as helmet, non-wizard): blocks `CLAIRVOYANT`
- **Eyes of the Overworld** artifact (as eyewear): blocks `BLINDED`

---

## 6. Magic Cancellation (MC)

### 6.1 MC Calculation (`magic_negation()` in `mhitu.c:1084`)

MC is determined by the **highest** `a_can` value among all worn armor pieces,
not their sum:

```
mc = 0
for each item in inventory:
    if item is worn armor:
        armpro = objects[item.otyp].a_can
        if armpro > mc:
            mc = armpro

if entity has extrinsic Protection (from artifact, item, etc.):
    mc += 2 if AMULET_OF_GUARDING, else mc += 1
    mc = min(mc, 3)
else if mc < 1:
    if entity has intrinsic Protection (prayer blessing > 0 or spell protection):
        mc = 1
```

### 6.2 MC Values by Armor

| MC Value | Armor Items                                                    |
|----------|----------------------------------------------------------------|
| 0        | Leather jacket, Hawaiian shirt, T-shirt, fedora, dunce cap,    |
|          | dented pot, small shield, elven shield, Uruk-hai shield,       |
|          | orcish shield, large shield, dwarvish roundshield,             |
|          | shield of reflection, all gloves, all boots,                   |
|          | all dragon scales/mail                                         |
| 1        | Most body armor (leather armor, ring mail, orcish ring mail,   |
|          | studded leather armor, scale mail, chain mail, orcish chain,   |
|          | splint mail, banded mail, bronze plate mail),                  |
|          | elven cloak, alchemy smock, leather cloak, mummy wrapping,     |
|          | orcish cloak, dwarvish cloak,                                  |
|          | cloak of invisibility, cloak of magic resistance,              |
|          | cloak of displacement, cornuthaum                              |
| 2        | Plate mail, crystal plate mail, dwarvish mithril-coat,         |
|          | elven mithril-coat, oilskin cloak, robe                        |
| 3        | Cloak of protection (unique: only item with MC 3)              |

### 6.3 MC Effect on Magic Attack Negation (`uhitm.c:86`)

```
armpro = magic_negation(defender)
negated = !(rn2(10) >= 3 * armpro)
```

Probability that a magic attack is **negated** (blocked):

| MC | Negation probability | Formula              |
|----|---------------------|----------------------|
| 0  | 0%                  | rn2(10) >= 0 always  |
| 1  | 30%                 | rn2(10) < 3          |
| 2  | 60%                 | rn2(10) < 6          |
| 3  | 90%                 | rn2(10) < 9          |

Note: `rn2(10)` returns 0..9. `rn2(10) >= 3*armpro` is the check for
the attack to succeed. Negation occurs when this check **fails**.

---

## 7. Erosion and Its Effect on AC

### 7.1 Erosion Counters

Each object has two erosion counters:
- `oeroded` (0..3): rust (iron) or burn (flammable) -- "primary" damage
- `oeroded2` (0..3): corrosion (copper/iron) or rot (organic) -- "secondary" damage

`MAX_ERODE = 3`. At erosion level 3, the item is "completely" eroded and may
be destroyed on the next erosion event (if `EF_DESTROY` flag set).

Additionally, crystal (glass) armor can **crack** (`ERODE_CRACK`), using
the primary erosion counter.

### 7.2 Erosion Effect on ARM_BONUS

From the `ARM_BONUS` macro:

```
ARM_BONUS(obj) = a_ac + spe - min(greatest_erosion(obj), a_ac)
```

Where `greatest_erosion(obj) = max(oeroded, oeroded2)`.

The erosion penalty is `min(greatest_erosion, a_ac)`, meaning:
- Erosion reduces the **base** armor contribution, never below zero for the
  base portion.
- Enchantment (`spe`) is **not** reduced by erosion.
- If `a_ac = 7` (plate mail) and `greatest_erosion = 2`, penalty is 2,
  giving `ARM_BONUS = 7 + spe - 2 = 5 + spe`.
- If `a_ac = 1` (elven leather helm) and `greatest_erosion = 3`, penalty
  is `min(3, 1) = 1`, giving `ARM_BONUS = 1 + spe - 1 = spe`.

### 7.3 Erosion Protection

- `oerodeproof` flag: item is erodeproof (immune to erosion).
- Blessed items have a `!rnl(4)` chance (~25% base, luck-adjusted) of
  resisting erosion even when not erodeproof.
- Greased items may resist if `EF_GREASE` flag is checked.

### 7.4 Erosion Types by Material

| Material       | Vulnerable to        | Erosion counter |
|----------------|----------------------|-----------------|
| Iron/steel     | Rust, corrosion      | oeroded, oeroded2 |
| Copper/brass   | Corrosion            | oeroded2        |
| Leather/cloth  | Burn, rot            | oeroded, oeroded2 |
| Wood           | Burn, rot            | oeroded, oeroded2 |
| Dragon hide    | (not damageable)     | N/A             |
| Mithril        | (not damageable)     | N/A             |
| Glass (armor)  | Crack                | oeroded         |

---

## 8. Donning/Doffing Delay (Turn Cost)

### 8.1 Delay Values (`oc_delay` Field)

The `oc_delay` field in `objects.h` specifies the multi-turn delay for
putting on or taking off armor. The delay is applied as `nomul(-oc_delay)`.

**From `objects.h` data:**

| Armor Category     | Delay | Specific Items                              |
|--------------------|-------|---------------------------------------------|
| Body armor (heavy) | 5     | Plate mail, crystal plate mail, bronze plate, splint, banded, chain, orcish chain, scale, ring, orcish ring, dragon scale mail/scales |
| Body armor (light) | 3     | Studded leather armor, leather armor         |
| Body armor (none)  | 0     | Leather jacket, dwarvish/elven mithril-coat (delay=1) |
| Shirts             | 0     | Hawaiian shirt, T-shirt                      |
| Cloaks             | 0     | All cloaks                                   |
| Helmets            | 1     | Most helmets; fedora=0, dented pot=0         |
| Shields            | 0     | All shields                                  |
| Gloves             | 1     | All gloves/gauntlets                         |
| Boots              | 2     | All boots                                    |

### 8.2 Putting On (`accessory_or_armor_on()`)

For armor items:
```
delay = objects[obj.otyp].oc_delay    // the raw value
nomul(-delay)                          // spend that many turns
```

If delay is 0, the item is donned immediately with no turn loss (the
`afternmv` callback fires at once via `unmul("")`).

### 8.3 Taking Off (Single Item via `armoroff()`)

```
delay = objects[otmp.otyp].oc_delay
nomul(-delay)
```

Same delay for removing as for donning.

### 8.4 Taking Off via 'A' Command (`take_off()`)

When using the 'A' command to remove multiple items, additional delays
apply for items that require removing and re-donning covering armor:

- **Removing suit when cloak worn**: `delay += 2 * oc_delay(cloak) + 1`
  (The `+1` is explicitly noted as a kludge since all known cloaks have
  delay=0, making `2*0+1=1`.)
- **Removing shirt when suit and/or cloak worn**:
  - If suit worn: `delay += 2 * oc_delay(suit)`
  - If cloak worn: `delay += 2 * oc_delay(cloak) + 1`

After computing total delay for an item: `if delay > 0: delay -= 1`
(to account for the occupation counter starting on the next move).

---

## 9. Polymorph Effects on Equipment (`break_armor()`)

### 9.1 breakarm Forms

If the new form satisfies `breakarm()` (large non-sliparm; `bigmonst()`,
or medium+ non-humanoid, or marilith/winged gargoyle):

| Slot          | Effect                                              |
|---------------|-----------------------------------------------------|
| Body armor    | **Destroyed** ("You break out of your armor!")      |
| Cloak         | **Dropped** (clasp breaks); mummy wrapping exempt if `WrappingAllowed` |
| Shirt         | **Destroyed** ("Your shirt rips to shreds!")        |

### 9.2 sliparm Forms

If `sliparm()` (whirly, very small, or noncorporeal):

| Slot          | Effect                                              |
|---------------|-----------------------------------------------------|
| Body armor    | **Dropped** ("Your armor falls around you!")        |
| Cloak         | **Dropped** (fall/shrink); mummy wrapping exempt    |
| Shirt         | **Dropped** ("You become much too small!")          |

### 9.3 Specific Body Part Checks

| Condition                      | Affected Slot | Effect                   |
|--------------------------------|---------------|--------------------------|
| `has_horns()` + non-flimsy helm| Helmet       | **Dropped** (falls off)  |
| `has_horns()` + flimsy helm    | Helmet       | Horn pierces through, stays worn |
| `nohands()` or `verysmall()`   | Gloves       | **Dropped** (with weapon) |
| `nohands()` or `verysmall()`   | Shield       | **Dropped**              |
| `nohands()` or `verysmall()`   | Helmet       | **Dropped**              |
| `nohands()`, `verysmall()`, `slithy()`, or centaur | Boots | **Dropped** |

### 9.4 Non-Armor Items

- **Eyewear** (blindfold, lenses, towel): falls off if form has no head.
- **Amulet**: stays worn regardless of form.

---

## 10. Detailed AC Bonus Table

The `a_ac` field stores `10 - display_AC`. The ARM_BONUS at +0 enchantment
with no erosion equals `a_ac`.

### 10.1 Helmets

| Item                        | Display AC | a_ac | MC | Delay | Material     | oc_oprop     |
|-----------------------------|-----------|------|-----|-------|-------------|--------------|
| Elven leather helm          | 9         | 1    | 0   | 1     | Leather     | 0            |
| Orcish helm                 | 9         | 1    | 0   | 1     | Iron        | 0            |
| Dwarvish iron helm          | 8         | 2    | 0   | 1     | Iron        | 0            |
| Fedora                      | 10        | 0    | 0   | 0     | Cloth       | 0            |
| Cornuthaum                  | 10        | 0    | 1   | 1     | Cloth       | CLAIRVOYANT  |
| Dunce cap                   | 10        | 0    | 0   | 1     | Cloth       | 0            |
| Dented pot                  | 9         | 1    | 0   | 0     | Iron        | 0            |
| Helm of brilliance          | 9         | 1    | 0   | 1     | Glass       | 0            |
| Helmet (plumed)             | 9         | 1    | 0   | 1     | Iron        | 0            |
| Helm of caution             | 9         | 1    | 0   | 1     | Iron        | WARNING      |
| Helm of opposite alignment  | 9         | 1    | 0   | 1     | Iron        | 0            |
| Helm of telepathy           | 9         | 1    | 0   | 1     | Iron        | TELEPAT      |

### 10.2 Body Armor (Suits)

| Item                        | Display AC | a_ac | MC | Delay | Material     | oc_oprop     |
|-----------------------------|-----------|------|-----|-------|-------------|--------------|
| Gray dragon scale mail      | 1         | 9    | 0   | 5     | Dragon hide | ANTIMAGIC    |
| Gold dragon scale mail      | 1         | 9    | 0   | 5     | Dragon hide | 0 (light)    |
| Silver dragon scale mail    | 1         | 9    | 0   | 5     | Dragon hide | REFLECTING   |
| Red dragon scale mail       | 1         | 9    | 0   | 5     | Dragon hide | FIRE_RES     |
| White dragon scale mail     | 1         | 9    | 0   | 5     | Dragon hide | COLD_RES     |
| Orange dragon scale mail    | 1         | 9    | 0   | 5     | Dragon hide | SLEEP_RES    |
| Black dragon scale mail     | 1         | 9    | 0   | 5     | Dragon hide | DISINT_RES   |
| Blue dragon scale mail      | 1         | 9    | 0   | 5     | Dragon hide | SHOCK_RES    |
| Green dragon scale mail     | 1         | 9    | 0   | 5     | Dragon hide | POISON_RES   |
| Yellow dragon scale mail    | 1         | 9    | 0   | 5     | Dragon hide | ACID_RES     |
| Gray dragon scales          | 7         | 3    | 0   | 5     | Dragon hide | ANTIMAGIC    |
| Gold dragon scales          | 7         | 3    | 0   | 5     | Dragon hide | 0 (light)    |
| Silver dragon scales        | 7         | 3    | 0   | 5     | Dragon hide | REFLECTING   |
| Red dragon scales           | 7         | 3    | 0   | 5     | Dragon hide | FIRE_RES     |
| White dragon scales         | 7         | 3    | 0   | 5     | Dragon hide | COLD_RES     |
| Orange dragon scales        | 7         | 3    | 0   | 5     | Dragon hide | SLEEP_RES    |
| Black dragon scales         | 7         | 3    | 0   | 5     | Dragon hide | DISINT_RES   |
| Blue dragon scales          | 7         | 3    | 0   | 5     | Dragon hide | SHOCK_RES    |
| Green dragon scales         | 7         | 3    | 0   | 5     | Dragon hide | POISON_RES   |
| Yellow dragon scales        | 7         | 3    | 0   | 5     | Dragon hide | ACID_RES     |
| Plate mail                  | 3         | 7    | 2   | 5     | Iron        | 0            |
| Crystal plate mail          | 3         | 7    | 2   | 5     | Glass       | 0            |
| Bronze plate mail           | 4         | 6    | 1   | 5     | Copper      | 0            |
| Splint mail                 | 4         | 6    | 1   | 5     | Iron        | 0            |
| Banded mail                 | 4         | 6    | 1   | 5     | Iron        | 0            |
| Dwarvish mithril-coat       | 4         | 6    | 2   | 1     | Mithril     | 0            |
| Elven mithril-coat          | 5         | 5    | 2   | 1     | Mithril     | 0            |
| Chain mail                  | 5         | 5    | 1   | 5     | Iron        | 0            |
| Orcish chain mail           | 6         | 4    | 1   | 5     | Iron        | 0            |
| Scale mail                  | 6         | 4    | 1   | 5     | Iron        | 0            |
| Studded leather armor       | 7         | 3    | 1   | 3     | Leather     | 0            |
| Ring mail                   | 7         | 3    | 1   | 5     | Iron        | 0            |
| Orcish ring mail            | 8         | 2    | 1   | 5     | Iron        | 0            |
| Leather armor               | 8         | 2    | 1   | 3     | Leather     | 0            |
| Leather jacket              | 9         | 1    | 0   | 0     | Leather     | 0            |

### 10.3 Shirts

| Item                        | Display AC | a_ac | MC | Delay | Material | oc_oprop |
|-----------------------------|-----------|------|-----|-------|----------|----------|
| Hawaiian shirt              | 10        | 0    | 0   | 0     | Cloth    | 0        |
| T-shirt                     | 10        | 0    | 0   | 0     | Cloth    | 0        |

### 10.4 Cloaks

| Item                        | Display AC | a_ac | MC | Delay | Material | oc_oprop     |
|-----------------------------|-----------|------|-----|-------|----------|--------------|
| Mummy wrapping              | 10        | 0    | 1   | 0     | Cloth    | 0            |
| Elven cloak                 | 9         | 1    | 1   | 0     | Cloth    | STEALTH      |
| Orcish cloak                | 10        | 0    | 1   | 0     | Cloth    | 0            |
| Dwarvish cloak              | 10        | 0    | 1   | 0     | Cloth    | 0            |
| Oilskin cloak               | 9         | 1    | 2   | 0     | Cloth    | 0            |
| Robe                        | 8         | 2    | 2   | 0     | Cloth    | 0            |
| Alchemy smock               | 9         | 1    | 1   | 0     | Cloth    | POISON_RES   |
| Leather cloak               | 9         | 1    | 1   | 0     | Leather  | 0            |
| Cloak of protection         | 7         | 3    | 3   | 0     | Cloth    | PROTECTION   |
| Cloak of invisibility       | 9         | 1    | 1   | 0     | Cloth    | INVIS        |
| Cloak of magic resistance   | 9         | 1    | 1   | 0     | Cloth    | ANTIMAGIC    |
| Cloak of displacement       | 9         | 1    | 1   | 0     | Cloth    | DISPLACED    |

### 10.5 Shields

| Item                        | Display AC | a_ac | MC | Delay | Material | oc_oprop     |
|-----------------------------|-----------|------|-----|-------|----------|--------------|
| Small shield                | 9         | 1    | 0   | 0     | Wood     | 0            |
| Elven shield                | 8         | 2    | 0   | 0     | Wood     | 0            |
| Uruk-hai shield             | 9         | 1    | 0   | 0     | Iron     | 0            |
| Orcish shield               | 9         | 1    | 0   | 0     | Iron     | 0            |
| Large shield                | 8         | 2    | 0   | 0     | Iron     | 0            |
| Dwarvish roundshield        | 8         | 2    | 0   | 0     | Iron     | 0            |
| Shield of reflection        | 8         | 2    | 0   | 0     | Silver   | REFLECTING   |

### 10.6 Gloves

| Item                        | Display AC | a_ac | MC | Delay | Material | oc_oprop     |
|-----------------------------|-----------|------|-----|-------|----------|--------------|
| Leather gloves              | 9         | 1    | 0   | 1     | Leather  | 0            |
| Gauntlets of fumbling       | 9         | 1    | 0   | 1     | Leather  | FUMBLING     |
| Gauntlets of power          | 9         | 1    | 0   | 1     | Iron     | 0            |
| Gauntlets of dexterity      | 9         | 1    | 0   | 1     | Leather  | 0            |

### 10.7 Boots

| Item                        | Display AC | a_ac | MC | Delay | Material | oc_oprop     |
|-----------------------------|-----------|------|-----|-------|----------|--------------|
| Low boots                   | 9         | 1    | 0   | 2     | Leather  | 0            |
| Iron shoes                  | 8         | 2    | 0   | 2     | Iron     | 0            |
| High boots                  | 8         | 2    | 0   | 2     | Leather  | 0            |
| Speed boots                 | 9         | 1    | 0   | 2     | Leather  | FAST         |
| Water walking boots         | 9         | 1    | 0   | 2     | Leather  | WWALKING     |
| Jumping boots               | 9         | 1    | 0   | 2     | Leather  | JUMPING      |
| Elven boots                 | 9         | 1    | 0   | 2     | Leather  | STEALTH      |
| Kicking boots               | 9         | 1    | 0   | 2     | Iron     | 0            |
| Fumble boots                | 9         | 1    | 0   | 2     | Leather  | FUMBLING     |
| Levitation boots             | 9         | 1    | 0   | 2     | Leather  | LEVITATION   |

---

## 11. Spell Protection Details

### 11.1 `cast_protection()` in `spell.c:1104`

```
natac = u.uac + u.uspellprot    // factor out current spell protection
loglev = floor(log2(u.ulevel)) + 1   // 1..5 for levels 1..30
natac_scaled = (10 - natac) / 10      // integer division; convert to positive and scale
gain = loglev - u.uspellprot / (4 - min(3, natac_scaled))

if gain > 0:
    u.uspellprot += gain
    u.uspmtime = 20 if expert in protection skill, else 10
    if u.usptime == 0:
        u.usptime = u.uspmtime
    find_ac()
```

### 11.2 Spell Protection Decay (`timeout.c:652`)

Each game turn:
```
if u.usptime > 0:
    u.usptime -= 1
    if u.usptime == 0 and u.uspellprot > 0:
        u.usptime = u.uspmtime     // reset timer
        u.uspellprot -= 1          // lose 1 point
        find_ac()
```

Protection decays 1 point every `uspmtime` turns (10 or 20 turns per point
depending on skill).

### 11.3 Divine Protection (`u.ublessed`)

Gained via prayer:
- First grant: `u.ublessed = rn1(3, 2)` = random 2..4.
- Subsequent grants: `u.ublessed += 1`.
- Lost entirely when angering a deity: `u.ublessed = 0`.
- Requires `HProtection & INTRINSIC` flag (set by `HProtection |= FROMOUTSIDE`).

---

## 12. Monk Body Armor Penalty

When a Monk wears body armor (`uarm` is non-null), a to-hit penalty is applied:

```
iflags.tux_penalty = (uarm && Role_if(PM_MONK) && urole.spelarmr)
```

The penalty value is `urole.spelarmr` (role-specific spell armor penalty).
This affects **to-hit** (not AC). The penalty does not apply when polymorphed.

---

## 13. Monster AC Calculation (`find_mac()` in `worn.c:709`)

For monsters, AC follows the same formula:

```
base = mon.data->ac          // monster species base AC
for each item in mon.minvent:
    if item is worn:
        if item == AMULET_OF_GUARDING:
            base -= 2
        else:
            base -= ARM_BONUS(item)
if abs(base) > 99:
    base = sign(base) * 99
```

---

## 14. Test Vectors

All test vectors assume an **unpolymorphed human** (base AC = 10).

### TV-01: Naked Human

```
Input:  No armor, no rings, no amulet, no protection
Output: AC = 10
```

### TV-02: Single Piece -- Plate Mail +0, No Erosion

```
Input:  uarm = plate mail (a_ac=7, spe=0, erosion=0)
ARM_BONUS = 7 + 0 - min(0, 7) = 7
Output: AC = 10 - 7 = 3
```

### TV-03: Plate Mail +3, No Erosion

```
Input:  uarm = plate mail (a_ac=7, spe=3, erosion=0)
ARM_BONUS = 7 + 3 - 0 = 10
Output: AC = 10 - 10 = 0
```

### TV-04: Full Loadout, All +0

```
Input:  uarm  = plate mail       (a_ac=7)
        uarmc = cloak of protection (a_ac=3)
        uarmh = dwarvish iron helm  (a_ac=2)
        uarmf = iron shoes          (a_ac=2)
        uarms = large shield        (a_ac=2)
        uarmg = leather gloves      (a_ac=1)
        uarmu = T-shirt             (a_ac=0)
        All spe=0, no erosion
Total ARM_BONUS = 7 + 3 + 2 + 2 + 2 + 1 + 0 = 17
Output: AC = 10 - 17 = -7
```

### TV-05: Erosion Reduces Base Only

```
Input:  uarm = plate mail (a_ac=7, spe=+2, oeroded=3, oeroded2=0)
greatest_erosion = max(3, 0) = 3
ARM_BONUS = 7 + 2 - min(3, 7) = 7 + 2 - 3 = 6
Output: AC = 10 - 6 = 4
```

### TV-06: Erosion Capped by a_ac (Boundary)

```
Input:  uarm = leather jacket (a_ac=1, spe=+3, oeroded=3, oeroded2=2)
greatest_erosion = max(3, 2) = 3
ARM_BONUS = 1 + 3 - min(3, 1) = 1 + 3 - 1 = 3
Output: AC = 10 - 3 = 7
Note: erosion penalty is 1 (not 3), because a_ac is only 1.
```

### TV-07: Negative Enchantment

```
Input:  uarm = leather armor (a_ac=2, spe=-3, erosion=0)
ARM_BONUS = 2 + (-3) - 0 = -1
Output: AC = 10 - (-1) = 11
Note: Negative ARM_BONUS worsens AC beyond base 10.
```

### TV-08: Rings of Protection + Amulet of Guarding

```
Input:  No armor
        uleft = ring of protection (spe=+3)
        uright = ring of protection (spe=+5)
        uamul = amulet of guarding
Output: AC = 10 - 3 - 5 - 2 = 0
```

### TV-09: Divine + Spell Protection

```
Input:  No armor
        HProtection has INTRINSIC flag, u.ublessed = 4
        u.uspellprot = 3
Output: AC = 10 - 4 - 3 = 3
```

### TV-10: AC Cap (Boundary)

```
Input:  uarm = gray dragon scale mail (a_ac=9, spe=+7)
        uarmc = cloak of protection (a_ac=3, spe=+5)
        uarmh = dwarvish iron helm (a_ac=2, spe=+5)
        uarmf = speed boots (a_ac=1, spe=+5)
        uarms = shield of reflection (a_ac=2, spe=+5)
        uarmg = gauntlets of dexterity (a_ac=1, spe=+5)
        uarmu = T-shirt (a_ac=0, spe=+5)
        uleft = ring of protection (spe=+7)
        uright = ring of protection (spe=+7)
        uamul = amulet of guarding
        HProtection INTRINSIC, u.ublessed=4
        u.uspellprot=5
        No erosion.

ARM_BONUS(uarm)  = 9 + 7 = 16
ARM_BONUS(uarmc) = 3 + 5 = 8
ARM_BONUS(uarmh) = 2 + 5 = 7
ARM_BONUS(uarmf) = 1 + 5 = 6
ARM_BONUS(uarms) = 2 + 5 = 7
ARM_BONUS(uarmg) = 1 + 5 = 6
ARM_BONUS(uarmu) = 0 + 5 = 5
rings: 7 + 7 = 14
amulet: 2
divine: 4
spell: 5

raw_uac = 10 - 16 - 8 - 7 - 6 - 7 - 6 - 5 - 14 - 2 - 4 - 5 = -70
abs(-70) <= 99, so no capping.
Output: AC = -70
```

### TV-11: AC Cap Actually Triggered (Boundary)

```
Input: Same as TV-10 but spe values pushed to +99 limit for each slot.
       uarm (a_ac=9, spe=+99): ARM_BONUS = 108
       uarmc (a_ac=3, spe=+99): ARM_BONUS = 102
       Sum of all bonuses vastly exceeds 109.
       raw_uac = 10 - (very large number) << -99

Output: AC = -99 (capped by AC_MAX)
```

### TV-12: Eroded Zero-AC Item (Boundary)

```
Input:  uarmu = Hawaiian shirt (a_ac=0, spe=0, oeroded=3)
greatest_erosion = 3
ARM_BONUS = 0 + 0 - min(3, 0) = 0
Output: AC = 10 - 0 = 10
Note: Erosion has no effect when a_ac is already 0.
```

### TV-13: MC Calculation -- Mixed Sources

```
Input:  uarm  = plate mail (a_can=2)
        uarmc = orcish cloak (a_can=1)
        uamul = amulet of guarding
        HProtection INTRINSIC with u.ublessed > 0 (extrinsic Protection)

mc starts at 0.
Iterate: plate mail a_can=2, mc becomes 2.
         orcish cloak a_can=1, already < mc, no change.
gotprot = TRUE (from amulet of guarding's Protection check)
via_amul = TRUE
mc += 2 -> 4, capped to 3.
Output: MC = 3
Negation probability = 90%
```

### TV-14: MC Without Extrinsic Protection

```
Input:  uarm = leather armor (a_can=1)
        No amulet of guarding, no Protection extrinsic.
        HProtection INTRINSIC, u.ublessed > 0 (prayer protection only).

mc starts at 0.
Iterate: leather armor a_can=1, mc becomes 1.
gotprot = FALSE (no extrinsic Protection).
mc >= 1, so intrinsic protection fallback not triggered.
Output: MC = 1
Negation probability = 30%
```

### TV-15: AC_VALUE Boundary -- AC = 0

```
Input:  AC = 0
AC_VALUE(0) = 0   (non-negative path)
Output: 0 (deterministic)
```

### TV-16: AC_VALUE Boundary -- AC = -1

```
Input:  AC = -1
AC_VALUE(-1) = -rnd(1) = -1 (always)
Output: -1 (deterministic, since rnd(1) = 1)
```

### TV-17: thitu Ranged Hit -- Negative AC (Boundary)

```
Input:  u.uac = -20, tlev = 5
Condition: u.uac + tlev <= rnd(20)  =>  -20 + 5 <= rnd(20)  =>  -15 <= 1..20
This is ALWAYS true, so the attack always MISSES.

Compare melee: AC_VALUE(-20) + 10 + 5 = (-1..-20) + 15 = -5..14
  rnd(20) range 1..20
  Hit if tmp > dieroll. With tmp=-5: never hits. With tmp=14: hits on 1..13.

Note: At AC -20, ranged attacks with tlev <= 19 ALWAYS miss (deterministic),
while melee attacks from a level 5 monster still hit sometimes.
```

### TV-18: Damage Reduction -- AC = -5, Damage = 3

```
Input:  u.uac = -5, damage = 3
damage -= rnd(5)   // 1..5
Result = max(3 - rnd(5), 1)
  rnd(5)=1: 2 damage
  rnd(5)=2: 1 damage
  rnd(5)=3..5: 1 damage (capped)
Output: 1 or 2 damage (60% chance of 1, 20% chance of 2)
```

### TV-19: Damage Reduction -- AC = 0 (No Reduction)

```
Input:  u.uac = 0, damage = 10
Condition: u.uac < 0 is FALSE, so no reduction.
Output: 10 damage (unchanged)
```

### TV-20: MC Intrinsic-Only Fallback (Boundary)

```
Input:  No armor worn at all.
        No amulet of guarding.
        u.uspellprot = 5 (from protection spell).

mc starts at 0 (no worn armor).
gotprot = FALSE (no extrinsic Protection).
mc < 1, so check intrinsic: uspellprot > 0 => mc = 1.
Output: MC = 1
Negation probability = 30%
```

### TV-21: Polymorphed Form Base AC

```
Input:  Hero polymorphed into xorn (mons[PM_XORN].ac = -2).
        No armor worn (xorn cannot wear suits).
Output: AC = -2
```

---

## 15. Uncertain / Noteworthy Items

1. **Gold dragon scales/mail `oc_oprop = 0`**: The light-emitting property
   is handled specially, not via `oc_oprop`. The hallucination resistance
   toggle in `dragon_armor_handling()` uses `make_hallucinated()`.

2. **Shimmering dragon scales/mail**: Defined in source but guarded by
   `#if 0 /* DEFERRED */`. Not available in the current game.

3. **Dwarvish mithril-coat delay = 1**: Despite being a suit, it has
   the unusually low delay of 1 (vs. 5 for most suits), consistent with
   its non-bulky flag (`blk=0`).

4. **Elven mithril-coat delay = 1**: Same as dwarvish, also non-bulky.

5. **Dented pot delay = 0**: Unlike other helmets (delay=1), the dented
   pot has delay=0.

6. **`AC_VALUE` randomization**: Negative AC's benefit to evading hits is
   randomized via `-rnd(-AC)`, not flat. This is a deliberate design choice
   to prevent invulnerability at very low AC values.

7. **[疑似 bug] `thitu()` does not use `AC_VALUE()`**: Ranged hit determination
   in `thitu()` (`mthrowu.c:105`) uses `u.uac` directly, while melee hit
   determination in `mattacku()` (`mhitu.c:707`) uses `AC_VALUE(u.uac)`.
   This makes negative AC strictly more effective against ranged attacks than
   against melee attacks. See TV-17 for a concrete example.

8. **[疑似 bug] Cloak of protection MC redundancy**: The cloak of protection
   has `a_can=3` (the highest possible MC from armor alone). It also confers
   extrinsic `PROTECTION`, which triggers `gotprot=true` in `magic_negation()`,
   adding +1 to MC (capped to 3). Since `a_can=3` already achieves MC 3, the
   +1 from PROTECTION is redundant when this cloak is the MC-providing armor.
   The PROTECTION property's MC bonus only has practical effect when combined
   with a *different* armor piece that provides a lower `a_can` value.

9. **Mithril is metallic**: `is_metallic()` checks `IRON <= material <= MITHRIL`.
   Both dwarvish and elven mithril-coats count as metallic armor, triggering
   spell casting penalties (`spelarmr`) for roles that have them. This may
   surprise players who expect mithril to be "light" armor without penalties.

10. **Monsters do not benefit from rings, prayers, or spells for AC**:
    `find_mac()` only iterates worn inventory items. Ring of protection,
    divine protection, and spell protection have no effect on monster AC.
    This is an intentional simplification.
