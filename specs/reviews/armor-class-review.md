# Review: armor-class.md

**Reviewer**: Claude Opus 4.6 (1M context)
**Date**: 2026-03-14
**Spec file**: `/Users/hz/Downloads/nethack-babel/specs/armor-class.md`
**Source files reviewed**: `src/do_wear.c`, `src/worn.c`, `include/hack.h`, `include/you.h`

---

## Summary

The armor-class spec is exceptionally thorough and accurate. It covers the core AC formula, ARM_BONUS macro, equipment slots, wearing/removal constraints, conferred properties, magic cancellation, erosion, donning/doffing delays, polymorph effects, spell/divine protection, and monster AC. The spec includes 21 test vectors, detailed AC bonus tables for every armor piece, and multiple boundary condition tests. Only very minor issues were found.

**Verdict**: PASS.

---

## A. Format Compliance

| Check | Status | Notes |
|-------|--------|-------|
| [A1] 10+ test vectors | PASS | 21 test vectors (TV-01 through TV-21), all with worked calculations |
| [A2] 2+ boundary conditions | PASS | TV-06 (erosion capped by a_ac), TV-10/TV-11 (AC cap), TV-12 (eroded zero-AC item), TV-15/TV-16 (AC_VALUE at 0 and -1), TV-17 (ranged vs melee AC), TV-19 (no damage reduction at AC 0), TV-20 (MC intrinsic fallback) |
| [A3] No C code | PASS | No C code blocks; all presented as pseudocode or formulas |
| [A4] No Rust code | PASS | No Rust code present |
| [A5] Pseudocode present | PASS | Key algorithms: find_ac (1.1), ARM_BONUS (1.2), AC_VALUE (1.4), melee hit check (1.4), thitu (1.5), magic_negation (6.1), cast_protection (11.1), decay (11.2) |
| [A6] Exact formulas with constants | PASS | AC_MAX=99, all a_ac/MC/delay values tabulated, ARM_BONUS formula explicit |
| [A7] [疑似 bug] markers | PASS | Two properly marked: thitu() not using AC_VALUE (Section 1.5 / item 7), cloak of protection MC redundancy (item 8) |

---

## B. Content Coverage

| Keyword | Covered | Notes |
|---------|---------|-------|
| AC 计算 (AC calculation) | YES | Section 1 (find_ac, ARM_BONUS, AC_VALUE, AC cap) |
| 装备槽 (equipment slots) | YES | Section 2 (7 armor slots, non-armor equipment) |
| 穿戴限制 (wearing restrictions) | YES | Sections 3-4 (layering, body form, cursed items, removal constraints) |

---

## C. Source Accuracy

### C1. Function Coverage

**Non-trivial public functions in `src/do_wear.c`:**

| # | Function | Covered | Notes |
|---|----------|---------|-------|
| 1 | `find_ac()` | YES | Section 1.1, core formula |
| 2 | `Boots_on()` / `Boots_off()` | YES | Section 5.1 (properties), Section 8 (delay) |
| 3 | `Cloak_on()` / `Cloak_off()` | YES | Section 5.1 (properties) |
| 4 | `Helmet_on()` / `Helmet_off()` | YES | Section 5.3 (attribute modifiers) |
| 5 | `Gloves_on()` / `Gloves_off()` | YES | Section 5.3 |
| 6 | `Shield_on()` / `Shield_off()` | YES | Section 5.1 |
| 7 | `Shirt_on()` / `Shirt_off()` | Implicit | No special effects, noted via delay table |
| 8 | `Armor_on()` / `Armor_off()` | YES | Section 5.2 (dragon armor) |
| 9 | `Amulet_on()` / `Amulet_off()` | Partial | Amulet of guarding AC contribution covered |
| 10 | `Ring_on()` / `Ring_off()` | Partial | Ring of protection covered |
| 11 | `canwearobj()` | YES | Section 3.3 (body form restrictions) |
| 12 | `accessory_or_armor_on()` | YES | Section 8.2 (donning delay) |
| 13 | `dowear()` / `doputon()` | Implicit | Entry points, not mechanically relevant |
| 14 | `armoroff()` | YES | Section 8.3 (doffing delay) |
| 15 | `select_off()` | YES | Section 4.2 (removal constraints) |
| 16 | `take_off()` | YES | Section 8.4 (A-command delays) |
| 17 | `doddoremarm()` | Implicit | Entry point for A command |
| 18 | `destroy_arm()` | Partial | Not explicitly covered; armor destruction by scroll/breath |
| 19 | `adj_abon()` | YES | Section 5.3 |
| 20 | `inaccessible_equipment()` | NO | MINOR: covering armor accessibility check |
| 21 | `glibr()` | NO | MINOR: slippery fingers weapon/ring dropping |
| 22 | `some_armor()` | NO | MINOR: random armor selection for enchant/destroy |
| 23 | `stuck_ring()` | NO | MINOR: prayer-related ring check |
| 24 | `toggle_stealth()` | Implicit | Covered via oc_oprop table |
| 25 | `toggle_displacement()` | Implicit | Covered via oc_oprop table |
| 26 | `dragon_armor_handling()` | YES | Section 5.2 |
| 27 | `hard_helmet()` | Partial | Referenced in ranged-combat spec |
| 28 | `donning()` / `doffing()` | NO | MINOR: in-progress don/doff check |
| 29 | `cancel_don()` / `cancel_doff()` | NO | MINOR: interrupt handlers |
| 30 | `stop_donning()` | NO | MINOR: interrupt handler |
| 31 | `count_worn_armor()` | NO | MINOR: utility |

**Non-trivial public functions in `src/worn.c`:**

| # | Function | Covered | Notes |
|---|----------|---------|-------|
| 1 | `setworn()` | Implicit | Property application mechanism |
| 2 | `setnotworn()` | Implicit | Property removal |
| 3 | `find_mac()` | YES | Section 13 |
| 4 | `m_dowear()` | NO | MINOR: monster armor selection |
| 5 | `which_armor()` | Implicit | Helper function |
| 6 | `mon_break_armor()` | Partial | Section 9 covers hero polymorph; monster polymorph not separately detailed |
| 7 | `racial_exception()` | YES | Section 3.4 |
| 8 | `update_mon_extrinsics()` | NO | MINOR: monster property management |
| 9 | `mon_set_minvis()` | NO | MINOR: monster invisibility |
| 10 | `mon_adjust_speed()` | NO | MINOR: monster speed |
| 11 | `wearslot()` | Implicit | Slot determination |
| 12 | `extra_pref()` | NO | MINOR: monster armor preference scoring |

**Coverage: ~55% of public functions are explicitly covered. All core AC-related functions are covered. Uncovered functions are primarily utility/UI functions or monster AI.**

### C2. Formula Verification (3 most important)

**Formula 1: `find_ac()` (Section 1.1 vs do_wear.c:2467-2520)**

Spec says:
```
uac = base_ac
    - ARM_BONUS(body_armor) - ARM_BONUS(cloak) - ARM_BONUS(helmet)
    - ARM_BONUS(boots) - ARM_BONUS(shield) - ARM_BONUS(gloves)
    - ARM_BONUS(shirt)
    - ring_of_protection_left.spe - ring_of_protection_right.spe
    - 2 (if AMULET_OF_GUARDING)
    - u.ublessed (if HProtection INTRINSIC)
    - u.uspellprot
if abs(uac) > 99: uac = sign(uac) * 99
```

Source (do_wear.c:2470-2501):
```c
int uac = mons[u.umonnum].ac;
if (uarm)  uac -= ARM_BONUS(uarm);
if (uarmc) uac -= ARM_BONUS(uarmc);
if (uarmh) uac -= ARM_BONUS(uarmh);
if (uarmf) uac -= ARM_BONUS(uarmf);
if (uarms) uac -= ARM_BONUS(uarms);
if (uarmg) uac -= ARM_BONUS(uarmg);
if (uarmu) uac -= ARM_BONUS(uarmu);
if (uleft && uleft->otyp == RIN_PROTECTION)  uac -= uleft->spe;
if (uright && uright->otyp == RIN_PROTECTION) uac -= uright->spe;
if (uamul && uamul->otyp == AMULET_OF_GUARDING) uac -= 2;
if (HProtection & INTRINSIC) uac -= u.ublessed;
uac -= u.uspellprot;
if (abs(uac) > AC_MAX) uac = sgn(uac) * AC_MAX;
```

**EXACT MATCH.** Every term, ordering, and condition verified.

**Formula 2: ARM_BONUS macro (Section 1.2 vs hack.h:1532-1534)**

Spec says:
```
ARM_BONUS(obj) = a_ac + obj.spe - min(greatest_erosion(obj), a_ac)
```

Source (hack.h:1532-1534):
```c
#define ARM_BONUS(obj) \
    (objects[(obj)->otyp].a_ac + (obj)->spe \
     - min((int) greatest_erosion(obj), objects[(obj)->otyp].a_ac))
```

**EXACT MATCH.**

**Formula 3: `find_mac()` for monsters (Section 13 vs worn.c:709-728)**

Spec says:
```
base = mon.data->ac
for each item in mon.minvent:
    if item is worn:
        if item == AMULET_OF_GUARDING: base -= 2
        else: base -= ARM_BONUS(item)
if abs(base) > 99: base = sign(base) * 99
```

Source (worn.c:709-728):
```c
int base = mon->data->ac;
long mwflags = mon->misc_worn_check;
for (obj = mon->minvent; obj; obj = obj->nobj) {
    if (obj->owornmask & mwflags) {
        if (obj->otyp == AMULET_OF_GUARDING)
            base -= 2;
        else
            base -= ARM_BONUS(obj);
    }
}
if (abs(base) > AC_MAX) base = sgn(base) * AC_MAX;
```

**EXACT MATCH.**

### C3. Missing Mechanics

**CRITICAL:**
- None found.

**MINOR:**

1. **`destroy_arm()` function** (do_wear.c:3195-3251): The spec does not describe the mechanics of the "destroy armor" scroll, black dragon breath, or monster spell that can destroy worn armor. The function has a specific priority order (cloak > suit > shirt > helmet > gloves > boots > shield) and resistance checks (`obj_resists(armor, 0, 90)`). This is relevant to understanding armor durability.

2. **`glibr()` slippery fingers** (do_wear.c:2522-2621): When hero has "Glib" (slippery fingers, e.g., from grease), weapons and rings can slip off. The spec mentions `Glib` blocks glove removal (Section 3.3) but does not describe the ring/weapon dropping behavior.

3. **Armor interrupt/cancel mechanics** (`donning()`, `cancel_don()`, `stop_donning()`): The spec describes delays but not the mechanics of interrupting the donning process (e.g., being attacked while putting on armor).

4. **`inaccessible_equipment()`** (do_wear.c:3276): The logic for determining whether armor is covered and inaccessible for dipping or greasing is not described.

5. **Monster armor equipping AI** (`m_dowear()`, `m_dowear_type()`, `extra_pref()`): The spec does not cover how monsters choose which armor to wear. While this is primarily AI rather than AC mechanics, it affects what armor monsters have equipped and thus their AC.

6. **Alchemy smock acid resistance**: Section 5.1 lists alchemy smock's `oc_oprop` as `POISON_RES`, which is correct. However, the spec does not note that alchemy smock also grants acid resistance via special handling in `Cloak_on()` (do_wear.c:369: `EAcid_resistance |= WORN_CLOAK`) rather than via `oc_oprop`. The spec's Section 5.1 table could be misleading as it might suggest the smock only gives poison resistance.

    **Correction**: Re-reading the spec, the table header says "oc_oprop" and the entry correctly says `POISON_RES`. But the text in Section 5.1 says "When armor is worn, its `oc_oprop` is applied as an extrinsic property" -- this is accurate but incomplete for alchemy smock, which has an additional property not in `oc_oprop`. This is actually a good thing to note but not an error.

### C4. Test Vector Verification

**TV-04: Full Loadout, All +0**

Spec claims:
```
Total ARM_BONUS = 7 + 3 + 2 + 2 + 2 + 1 + 0 = 17
AC = 10 - 17 = -7
```

Source verification (do_wear.c:2470-2486):
- `uarm` plate mail: `objects[PLATE_MAIL].a_ac = 7`, spe=0, erosion=0 -> ARM_BONUS = 7
- `uarmc` cloak of protection: `a_ac = 3` -> ARM_BONUS = 3
- `uarmh` dwarvish iron helm: `a_ac = 2` -> ARM_BONUS = 2
- `uarmf` iron shoes: `a_ac = 2` -> ARM_BONUS = 2
- `uarms` large shield: `a_ac = 2` -> ARM_BONUS = 2
- `uarmg` leather gloves: `a_ac = 1` -> ARM_BONUS = 1
- `uarmu` T-shirt: `a_ac = 0` -> ARM_BONUS = 0
- Sum = 17, AC = 10 - 17 = -7

**VERIFIED.** Output matches.

**TV-06: Erosion Capped by a_ac (Boundary)**

Spec claims:
```
uarm = leather jacket (a_ac=1, spe=+3, oeroded=3, oeroded2=2)
greatest_erosion = max(3, 2) = 3
ARM_BONUS = 1 + 3 - min(3, 1) = 1 + 3 - 1 = 3
AC = 10 - 3 = 7
```

Source verification (hack.h:1532-1534):
- `objects[LEATHER_JACKET].a_ac = 1`
- `greatest_erosion = max(3, 2) = 3`
- `ARM_BONUS = 1 + 3 - min(3, 1) = 1 + 3 - 1 = 3` -- correct, erosion capped at a_ac value
- AC = 10 - 3 = 7

**VERIFIED.** Output matches. This correctly demonstrates the key boundary behavior where erosion cannot exceed `a_ac`.

**TV-17: thitu Ranged Hit -- Negative AC (Boundary)**

Spec claims:
```
u.uac = -20, tlev = 5
u.uac + tlev = -15 <= rnd(20) is always true -> always MISS
```

Source (mthrowu.c:105):
```c
if (u.uac + tlev <= (dieroll = rnd(20)))
```

`-20 + 5 = -15`, `rnd(20)` returns 1..20, so `-15 <= 1` is always true -> always miss.

**VERIFIED.** Output matches. This also confirms the [疑似 bug] about `thitu()` using raw `u.uac` instead of `AC_VALUE(u.uac)`.

---

## D. Recommendations

1. **Add a note about alchemy smock's dual properties**: The smock grants both `POISON_RES` (via `oc_oprop`) and `ACID_RES` (via special handling in `Cloak_on/Cloak_off`). Consider adding a note in Section 5.1 or a separate row in the property table to make this explicit. A reader relying solely on the `oc_oprop` table might miss the acid resistance.

2. **Consider adding `destroy_arm()` mechanics**: While not strictly AC calculation, understanding how armor can be destroyed (priority order, resistance checks) is important for a Rust reimplementation.

3. **Section 8.1 delay table minor clarification**: The table says "Body armor (none) | 0 | Leather jacket, dwarvish/elven mithril-coat (delay=1)". This is slightly confusing because it groups delay=0 and delay=1 items together. The mithril-coats have delay=1, not 0. Consider splitting the row or adding parenthetical clarification.

4. **Section 6.1 MC calculation**: The pseudocode says `mc += 2 if AMULET_OF_GUARDING, else mc += 1`. This should be clearer about what the "else" condition is. Looking at the source (`magic_negation()` in mhitu.c), the `+2` is for amulet of guarding specifically, and `+1` is for other sources of extrinsic Protection. The spec's pseudocode captures this but the English explanation could be more explicit.

5. **Section 10 tables are comprehensive**: The AC bonus tables for all armor items are extensive and appear accurate based on spot-checking against `objects.h`. These tables will be very valuable for the Rust reimplementation. No corrections needed.
