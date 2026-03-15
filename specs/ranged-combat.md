# Ranged Combat Mechanism Spec

Source: `src/dothrow.c`, `src/mthrowu.c`, `src/weapon.c`, `src/uhitm.c`, `src/zap.c` (boomhit),
`include/obj.h`, `include/hack.h`, `include/attrib.h`

---

## 1. Throwing Mechanics

### 1.1 Range Calculation

The base throwing range depends on strength and object weight.

```
crossbowing = ammo_and_launcher(obj, uwep) AND weapon_type(uwep) == P_CROSSBOW

IF crossbowing:
    urange = 18 / 2  # = 9, strength-independent
ELSE:
    urange = ACURRSTR / 2
    # ACURRSTR maps raw A_STR as follows:
    #   STR 3..18        -> 3..18
    #   STR 18/01..18/31 -> 19
    #   STR 18/32..18/81 -> 20
    #   STR 18/82..18/100, 19..21 -> 21
    #   STR 22..25       -> 22..25
    # (integer division: result = 19 + raw_str / 50 for 18/xx range)

IF obj is HEAVY_IRON_BALL:
    range = urange - (obj.owt / 100)
ELSE:
    range = urange - (obj.owt / 40)

# Special ball-and-chain rules
IF obj == uball:
    IF u.ustuck: range = 1
    ELSE IF range >= 5: range = 5

IF range < 1: range = 1
```

### 1.2 Ammo Range Modifiers

```
IF is_ammo(obj):
    IF ammo_and_launcher(obj, uwep):
        IF crossbowing:
            range = BOLT_LIM   # = 8 (overrides computed range entirely)
        ELSE:
            range += 1
    ELSE IF obj.oclass != GEM_CLASS:
        range /= 2   # integer division; penalty for throwing ammo by hand
        # message: "You aren't wielding a <launcher>..."
```

### 1.3 Special Range Overrides (applied after ammo modifiers)

```
IF Is_airlevel OR Levitation:
    urange_recoil = urange - range
    IF urange_recoil < 1: urange_recoil = 1
    range -= urange_recoil
    IF range < 1: range = 1
    # After projectile launch, hero hurtles (-dx, -dy, urange_recoil)

IF obj is BOULDER:       range = 20   # hero must be giant polymorphed
IF obj is Mjollnir:      range = (range + 1) / 2   # heavy; halved rounding up
IF tethered_weapon:      range = min(range, isqrt(arw.range))
    # aklys: arw.range = (BOLT_LIM/2)^2 = 16, so max range = isqrt(16) = 4
IF obj == uball AND u.utrap AND u.utraptype == TT_INFLOOR:
    range = 1
IF Underwater: range = 1
```

### 1.4 Stamina Check

```
IF (dx OR dy OR dz < 1)
   AND calc_capacity(obj.owt) > SLT_ENCUMBER
   AND (Upolyd ? (u.mh < 5 AND u.mh != u.mhmax)
              : (u.uhp < 10 AND u.uhp != u.uhpmax))
   AND obj.owt > (Upolyd ? u.mh : u.uhp) * 2
   AND NOT Is_airlevel:
    # "drops from your grasp" -- forced drop at feet
    dx = dy = 0; dz = 1
```

### 1.5 Cursed/Greased Misfire

```
IF (obj.cursed OR obj.greased) AND (dx OR dy) AND rn2(7) == 0:
    # ~14.3% chance per throw
    IF ammo_and_launcher(obj, uwep):
        # "<obj> misfires!"
    ELSE IF obj.greased OR throwing_weapon(obj):
        # "<obj> slips as you throw it!"
    ELSE:
        slipok = FALSE   # non-throwing items don't slip

    IF slipok:
        dx = rn2(3) - 1   # random -1, 0, +1
        dy = rn2(3) - 1
        IF dx == 0 AND dy == 0: dz = 1  # drops at feet
        impaired = TRUE
```

---

## 2. Launcher Mechanics

### 2.1 Launcher/Ammo Matching

Defined via skill codes in `objects[]` (from `include/obj.h`):

```
is_launcher(obj) = obj.oclass == WEAPON_CLASS
                   AND oc_skill >= P_BOW AND oc_skill <= P_CROSSBOW

is_ammo(obj) = (obj.oclass == WEAPON_CLASS OR GEM_CLASS)
               AND oc_skill >= -P_CROSSBOW AND oc_skill <= -P_BOW

matching_launcher(ammo, launcher) =
    launcher != NULL
    AND objects[ammo.otyp].oc_skill == -objects[launcher.otyp].oc_skill

ammo_and_launcher(ammo, launcher) =
    is_ammo(ammo) AND matching_launcher(ammo, launcher)
```

Concrete pairings (via oc_skill sign inversion):

| Launcher Skill | Launchers                              | Ammo                                                         |
| -------------- | -------------------------------------- | ------------------------------------------------------------ |
| P_BOW          | bow, elven bow, orcish bow, yumi       | arrow, elven arrow, silver arrow, orcish arrow, ya           |
| P_SLING        | sling                                  | rock, flint, gems, glass stones, loadstone, luckstone        |
| P_CROSSBOW     | crossbow                               | crossbow bolt                                                |

### 2.2 Glove Penalty for Bows

When wearing gloves and using a bow (wielded weapon oc_skill == P_BOW), to-hit penalty:

| Glove Type              | To-Hit Penalty |
| ----------------------- | -------------- |
| Gauntlets of power      | -2             |
| Gauntlets of fumbling   | -3             |
| Leather gloves          | 0              |
| Gauntlets of dexterity  | 0              |

### 2.3 Weapon Category Definitions

```
is_missile(obj) = (WEAPON_CLASS or TOOL_CLASS)
                  AND oc_skill >= -P_BOOMERANG AND oc_skill <= -P_DART
    # Includes: dart, shuriken, boomerang

throwing_weapon(obj) = is_missile(obj) OR is_spear(obj)
    OR (is_blade AND NOT is_sword AND oc_dir has PIERCE)  # daggers, knife
    OR otyp == WAR_HAMMER OR otyp == AKLYS

is_multigen(obj) = WEAPON_CLASS
    AND oc_skill >= -P_SHURIKEN AND oc_skill <= -P_BOW
    # Items generated in stacks: ammo + darts + shuriken
```

---

## 3. Multishot

### 3.1 Eligibility

Multishot requires ALL of:
- `obj.quan > 1` (more than one item in stack)
- Either: ammo with matching wielded launcher, OR: non-ammo weapon class stackable
- NOT Confused, NOT Stunned

### 3.2 Weak Multishot

Certain roles/conditions inhibit multishot (only get bonus at Expert):

```
weakmultishot = Role is Wizard OR Cleric
             OR (Role is Healer AND skill != P_KNIFE)
             OR (Role is Tourist AND skill != -P_DART)
             OR Fumbling
             OR ACURR(A_DEX) <= 6
```

### 3.3 Skill Bonus

```
SWITCH P_SKILL(weapon_type(obj)):
    P_EXPERT:  multishot += 1, THEN fall through to:
    P_SKILLED: IF NOT weakmultishot: multishot += 1
    default:   no bonus
```

Effective skill bonus:
- Expert + not weak: +2
- Expert + weak: +1
- Skilled + not weak: +1
- Skilled + weak: 0
- Basic/Unskilled: 0

### 3.4 Role Bonus (`multishot_class_bonus`)

| Role          | Condition                                  | Bonus |
| ------------- | ------------------------------------------ | ----- |
| Cave Dweller  | skill == -P_SLING OR skill == P_SPEAR     | +1    |
| Monk          | skill == -P_SHURIKEN                       | +1    |
| Ranger        | skill != P_DAGGER                          | +1    |
| Rogue         | skill == P_DAGGER                          | +1    |
| Ninja         | skill == -P_SHURIKEN OR skill == -P_DART  | +1    |
| Ninja/Samurai | ammo is YA AND launcher is YUMI            | +1    |

Note: Ninja falls through to Samurai, so a Ninja can get both the shuriken/dart bonus AND the ya+yumi bonus (if applicable). Samurai only gets ya+yumi.

### 3.5 Racial Bonus (requires NOT weakmultishot)

| Race   | Condition                                        | Bonus |
| ------ | ------------------------------------------------ | ----- |
| Elf    | ELVEN_ARROW + ELVEN_BOW wielded                  | +1    |
| Orc    | ORCISH_ARROW + ORCISH_BOW wielded                | +1    |
| Gnome  | skill == -P_CROSSBOW (any crossbow bolt)          | +1    |

### 3.6 Quest Artifact Bonus (requires NOT weakmultishot)

```
IF uwep is quest artifact AND ammo_and_launcher(obj, uwep):
    multishot += 1
```

### 3.7 Crossbow Strength Penalty

```
IF multishot > 1
   AND skill == -P_CROSSBOW
   AND ammo_and_launcher(obj, uwep)
   AND ACURRSTR < (Race_if(PM_GNOME) ? 16 : 18):
    multishot = rnd(multishot)   # randomize down before general randomization
```

### 3.8 Final Randomization

```
multishot = rnd(multishot)   # always randomized 1..multishot
IF multishot > obj.quan: multishot = obj.quan
IF shotlimit > 0 AND multishot > shotlimit: multishot = shotlimit
```

**[疑似 bug]** The crossbow strength penalty applies `multishot = rnd(multishot)` and then the general randomization applies `multishot = rnd(multishot)` again, double-penalizing crossbow users with low strength. For example, if accumulated multishot is 4, a weak crossbow user gets `rnd(rnd(4))` instead of just `rnd(4)`. The expected value drops from 2.5 to ~1.8. This may be intentional for game balance but the double-random is unusual compared to all other weapon types.

### 3.9 Monster Multishot (`monmulti`)

Different formula from hero multishot:

```
multishot = 1
# Prerequisite: quan > 1, ammo with launcher or stackable weapon, not confused

# Level-based bonus
IF is_prince: multishot += 2
ELIF is_lord: multishot += 1
ELIF is_mplayer: multishot += 1

# Elven craftsmanship
IF ammo is ELVEN_ARROW AND NOT cursed: multishot += 1
IF launcher is ELVEN_BOW AND ammo_and_launcher AND NOT cursed: multishot += 1

# Launcher enchantment (1/3 of spe)
IF ammo_and_launcher AND launcher.spe > 1:
    multishot += rounddiv(launcher.spe, 3)

# Randomize BEFORE class/racial bonuses
multishot = rnd(multishot)

# Class bonus (same function as hero)
multishot += multishot_class_bonus(monsndx, otmp, mwep)

# Racial bonus
IF (elf + elven arrow + elven bow)
   OR (orc + orcish arrow + orcish bow)
   OR (gnome + crossbow bolt + crossbow):
    multishot += 1

multishot = clamp(multishot, 1, otmp.quan)
```

**[疑似 bug]** Monster multishot applies `rnd()` randomization BEFORE class and racial bonuses (which are therefore guaranteed), while hero multishot applies `rnd()` AFTER all bonuses (making them merely probabilistic). This asymmetry means monster racial/class bonuses are always realized, making monsters slightly more lethal at multishot than equivalent heroes.

---

## 4. To-Hit Formula (Hero Throwing at Monster)

### 4.1 Base To-Hit (`thitmonst`)

```
tmp = -1 + Luck + find_mac(mon) + u.uhitinc
      + maybe_polyd(youmonst.data.mlevel, u.ulevel)
```

Where:
- `find_mac(mon)` = monster's effective AC (10 for no armor; lower = harder to hit)
- `u.uhitinc` = accuracy bonus from ring/intrinsic
- Level uses polymorphed form level if applicable

### 4.2 Dexterity Modifier

```
IF DEX < 4:  tmp -= 3
ELIF DEX < 6:  tmp -= 2
ELIF DEX < 8:  tmp -= 1
ELIF DEX >= 14: tmp += (DEX - 14)
# DEX 8..13: no modifier
```

### 4.3 Distance Modifier

```
disttmp = 3 - distmin(hero_x, hero_y, mon_x, mon_y)
IF disttmp < -4: disttmp = -4
tmp += disttmp
```

`distmin` = Chebyshev distance = max(|dx|, |dy|).

| Distance | Modifier |
| -------- | -------- |
| 1        | +2       |
| 2        | +1       |
| 3        | 0        |
| 4        | -1       |
| 5        | -2       |
| 6        | -3       |
| 7+       | -4       |

### 4.4 Monster Adjustments (`omon_adj`)

```
tmp += (mon.data.msize - MZ_MEDIUM)   # size: -2 (tiny) to +5 (gigantic)
IF mon.msleeping: tmp += 2
IF NOT mon.mcanmove OR mon.data.mmove == 0:
    tmp += 4
    # 10% chance to unfreeze on being targeted

SWITCH obj.otyp:
    HEAVY_IRON_BALL (not uball): tmp += 2
    BOULDER: tmp += 6
    default (weapon/weptool/gem): tmp += hitval(obj, mon)
```

### 4.5 `hitval` Detail

```
tmp = 0
IF is_weapon or weptool: tmp += obj.spe
tmp += objects[obj.otyp].oc_hitbon
IF is_weapon AND obj.blessed AND mon_hates_blessings(mon): tmp += 2
IF is_spear AND mon.mlet in {S_XORN, S_DRAGON, S_JABBERWOCK, S_NAGA, S_GIANT}:
    tmp += 2
IF obj is TRIDENT AND is_swimmer(mon):
    IF is_pool(mon.mx, mon.my): tmp += 4
    ELIF mon.mlet is S_EEL or S_SNAKE: tmp += 2
IF is_pick AND passes_walls(mon) AND thick_skinned(mon): tmp += 2
IF obj.oartifact: tmp += spec_abon(obj, mon)
```

### 4.6 Weapon-Type-Specific Modifiers

**For kicked objects:**
```
tmp -= (is_ammo ? 5 : 3)
```

**For ammo (thrown):**
```
IF NOT ammo_and_launcher(obj, uwep):
    tmp -= 4   # mismatched ammo, heavy penalty
ELSE:
    tmp += uwep.spe - greatest_erosion(uwep)
    tmp += weapon_hit_bonus(uwep)   # skill-based, see Section 4.8
    IF uwep.oartifact: tmp += spec_abon(uwep, mon)
    # Elf/Samurai bow expertise:
    IF (Race_if(PM_ELF) OR Role_if(PM_SAMURAI))
       AND (NOT Upolyd OR your_race(youmonst.data))
       AND objects[uwep.otyp].oc_skill == P_BOW:
        tmp += 1
        IF (Elf AND uwep is ELVEN_BOW) OR (Samurai AND uwep is YUMI):
            tmp += 1
```

**For thrown non-ammo or applied polearm:**
```
IF obj is BOOMERANG: tmp += 4
ELIF throwing_weapon(obj): tmp += 2       # designed for throwing
ELIF obj == thrownobj: tmp -= 2           # NOT designed for throwing
tmp += weapon_hit_bonus(obj)
```

### 4.7 Elf vs Orc Bonus

```
IF mon is orc AND hero is elf (including polymorphed): tmp += 1
```

### 4.8 `weapon_hit_bonus` (Skill-Based To-Hit)

| Skill Level    | Weapon Bonus |
| -------------- | ------------ |
| Restricted     | -4           |
| Unskilled      | -4           |
| Basic          | 0            |
| Skilled        | +2           |
| Expert         | +3           |

Additional riding penalty:

| Riding Skill   | Penalty |
| -------------- | ------- |
| Restricted     | -2      |
| Unskilled      | -2      |
| Basic          | -1      |
| Skilled/Expert | 0       |

Two-weapon while riding: additional -2.

### 4.9 Hit Determination

```
dieroll = rnd(20)
IF tmp >= dieroll: HIT
ELSE: MISS
```

If inside engulfer: `tmp += 1000` (guaranteed hit).

### 4.10 Special Object Hit Checks (bypass normal formula)

```
EGG / CREAM_PIE / BLINDING_VENOM / ACID_VENOM:
    hit if: guaranteed_hit OR ACURR(A_DEX) > rnd(25)

Potions:
    hit if: guaranteed_hit OR ACURR(A_DEX) > rnd(25)
```

---

## 5. Damage on Impact

### 5.1 Weapon Dice (`dmgval`)

```
IF bigmonst(target):
    base = rnd(objects[otyp].oc_wldam)   # "large monster" damage die
ELSE:
    base = rnd(objects[otyp].oc_wsdam)   # "small monster" damage die

# Additional dice for specific weapons (hardcoded switch in dmgval):
# Examples for small monsters:
#   CROSSBOW_BOLT, MACE, WAR_HAMMER, FLAIL, SPETUM, TRIDENT: +1
#   BATTLE_AXE, MORNING_STAR, BROADSWORD, RANSEUR, VOULGE: +rnd(4)
#   ACID_VENOM: +rnd(6)
# Examples for large monsters:
#   IRON_CHAIN, CROSSBOW_BOLT, MORNING_STAR, BROADSWORD: +1
#   FLAIL, RANSEUR, VOULGE: +rnd(4)
#   ACID_VENOM, HALBERD, SPETUM: +rnd(6)
#   BATTLE_AXE, BARDICHE, TRIDENT: +d(2,4)
#   TSURUGI, DWARVISH_MATTOCK, TWO_HANDED_SWORD: +d(2,6)

IF is_weapon or weptool:
    base += obj.spe   # enchantment bonus
    IF base < 0: base = 0

# Material resistance
IF obj material <= LEATHER AND thick_skinned(target): base = 0
IF target is Shade AND NOT shade_glare(obj): base = 0
```

### 5.2 Special Damage Bonuses (applied within `dmgval`)

```
IF (weapon OR gem OR ball OR chain):
    bonus = 0
    IF obj.blessed AND mon_hates_blessings: bonus += rnd(4)
    IF is_axe AND target is_wooden: bonus += rnd(4)
    IF material == SILVER AND mon_hates_silver: bonus += rnd(20)
    IF artifact_light AND lamplit AND hates_light: bonus += rnd(8)

    # If artifact double-damage applies, halve bonus to compensate
    IF bonus > 1 AND obj.oartifact AND spec_dbon(obj, mon, 25) >= 25:
        bonus = (bonus + 1) / 2

    base += bonus

# Erosion penalty
IF base > 0:
    base -= greatest_erosion(obj)
    IF base < 1: base = 1
```

### 5.3 Strength Bonus (`dbon`)

Applied to ALL thrown/melee attacks EXCEPT when shooting ammo through a matching launcher:

```
dbon():
    IF Upolyd: RETURN 0
    STR < 6:      RETURN -1
    STR 6..15:    RETURN 0
    STR 16..17:   RETURN 1
    STR 18:       RETURN 2
    STR 18/01..18/75:  RETURN 3
    STR 18/76..18/90:  RETURN 4
    STR 18/91..18/99:  RETURN 5
    STR >= 18/100:     RETURN 6
```

Application logic (from `hmon_hitmon_dmg_recalc`):
```
IF thrown != HMON_THROWN OR NOT ammo_and_launcher(obj, uwep):
    strbonus = dbon()
    # Two-weapon adjustment: strbonus = (3 * abs(strbonus) + 2) / 4 * sign
    # Two-handed melee: strbonus = (3 * abs(strbonus) + 1) / 2 * sign
    dmg += strbonus
ELSE:
    # Launcher-shot ammo: NO strength bonus
    dmg += u.udaminc only
```

### 5.4 Skill Damage Bonus (`weapon_dam_bonus`)

Applied when using weapon skill properly (melee, thrown missile, or shot ammo with launcher):

| Skill Level | Damage Bonus |
| ----------- | ------------ |
| Restricted  | -2           |
| Unskilled   | -2           |
| Basic       | 0            |
| Skilled     | +1           |
| Expert      | +2           |

### 5.5 Increase Damage Ring

```
dmg += u.udaminc   # always applied
```

### 5.6 Minimum Damage

```
IF dmg < 1: dmg = 1   # combined bonuses cannot reduce a hit below 1
```

### 5.7 Elven/Samurai Extra Damage (Shot Ammo)

When properly shot through matching launcher:

```
IF Role_if(PM_SAMURAI) AND obj is YA AND uwep is YUMI: dmg += 1
ELIF Race_if(PM_ELF) AND obj is ELVEN_ARROW AND uwep is ELVEN_BOW: dmg += 1
```

### 5.8 Poison Damage (Hero Attacks)

For thrown/shot poisoned ammo:

```
IF obj.opoisoned AND is_poisonable(obj):
    IF resists_poison(mon): no extra damage (message only)
    ELIF rn2(10) != 0: dmg += rnd(6)    # 90% chance
    ELSE: instant kill (poiskilled)      # 10% chance

# Poison wears off with probability 1/max(2, 10 - obj.owt/10)
# Permanently poisoned weapons (orcish dagger/arrow) never lose poison
```

### 5.9 Non-Weapon/Non-Gem Damage (hitting with launcher, ammo without launcher, etc.)

When a weapon-class object is used improperly (e.g., bashing with a bow, throwing ammo without launcher):

```
# hmon_hitmon_weapon_ranged path:
dmg = rnd(2)   # only 1-2 points base damage
IF material == SILVER AND mon_hates_silver:
    dmg += rnd(dmg > 0 ? 20 : 10)
# No skill bonus, no strength bonus applied
```

### 5.10 Non-Weapon Object Damage

For throwing objects that aren't weapons/weptools/gems:

```
dmg = (obj.owt + 99) / 100   # WT_TO_DMG = 100
IF dmg <= 1: dmg = 1
ELSE: dmg = rnd(dmg)
IF dmg > 6: dmg = 6

# Silver and blessed bonuses applied separately
IF obj.blessed AND mon_hates_blessings: dmg += rnd(4)
IF material == SILVER AND mon_hates_silver: dmg += rnd(20)
```

---

## 6. Special Projectiles

### 6.1 Daggers for Rogues

- Role multishot bonus: +1 for daggers (`skill == P_DAGGER`)
- `throwing_weapon(dagger)` returns TRUE, so thrown daggers get +2 to-hit bonus
- Backstab bonus (`dmg += rnd(u.ulevel)`) applies only to melee (`hand_to_hand`), NOT to thrown daggers
- To-hit: `weapon_hit_bonus` uses dagger skill level

### 6.2 Shuriken for Samurai/Ninja/Monk

- Ninja: +1 multishot for shuriken and darts
- Monk: +1 multishot for shuriken only
- `is_missile(shuriken)` is TRUE, so thrown shuriken get +2 to-hit
- `is_poisonable(shuriken)` is TRUE
- `oc_hitbon` for shuriken = +2 (stacks with +2 missile bonus = +4 total weapon bonus)

### 6.3 Boomerang

Boomerangs follow a unique curved flight path (not straight line). Handled by `boomhit()` in `zap.c`.

**Flight path:**
- 10 steps maximum along a curved trajectory
- Direction alternates; counterclockwise if hero is right-handed (URIGHTY), clockwise if left-handed
- Pattern rotates: at each step (except ct%5==0), direction index rotates left or right
- Can hit multiple monsters: `nhits = max(1, obj.spe + 1)` before stopping

**Return catch (at step ct==9, hero's position):**
```
IF Fumbling OR rn2(20) >= ACURR(A_DEX):
    # Hit self
    thitu(10 + obj.spe, dmgval(obj, &youmonst), "boomerang")
ELSE:
    # Caught successfully, returned to inventory
    exercise(A_DEX, TRUE)
```

**Boomerang as melee weapon (bashing, not thrown):**
```
dmg = rnd(2)
IF rnl(4) == 3:
    # Breaks into splinters (rnl is luck-adjusted)
    dmg += 1 (if target is not Shade)
    boomerang destroyed
```

**To-hit bonus:** +4 when thrown (highest among non-artifact weapons)

**Breakage immunity:** Boomerangs are explicitly exempted from `should_mulch_missile()`.

### 6.4 Aklys (Tethered Weapon)

- Must be wielded as primary weapon (W_WEP) to return
- Range limited by tether: `range = min(range, isqrt(16)) = min(range, 4)`
- Uses DISP_TETHER display (visible tether line, reels back on return)
- Return mechanics identical to Mjollnir (99% return, 99% catch if not impaired)
- Does NOT require special role or minimum strength

---

## 7. Returning Items

### 7.1 AutoReturn Macro

```
AutoReturn(obj, wmask) =
    (wmask & W_WEP) != 0
    AND (obj.otyp == AKLYS
         OR (obj.oartifact == ART_MJOLLNIR AND Role_if(PM_VALKYRIE)))
    OR obj.otyp == BOOMERANG
```

### 7.2 Mjollnir Return

Requirements:
- Must be wielded (W_WEP)
- Hero must be Valkyrie
- Hero must have STR >= STR19(25) (raw STR >= 125)

Range: `(range + 1) / 2` (halved, rounding up)

**Throwing upward (dz < 0):**
```
IF NOT impaired:
    # Instant return -- hits ceiling, comes back to hand
    # Re-wielded, twoweap state restored
```

**Horizontal throw return:**
```
IF rn2(100):  # 99% chance of returning
    IF NOT impaired AND rn2(100):  # ~98% overall catch rate
        "returns to your hand!"
        # Re-wielded, twoweap restored
    ELSE:
        IF rn2(2) == 0:
            # Lands at feet, 0 damage
        ELSE:
            dmg = rn2(2) + rnd(3)   # 0..1 + 1..3 = 1..4 base
            # artifact_hit() may add more
            losehp(Maybe_Half_Phys(dmg))
        # Object dropped on ground
ELSE:  # 1% chance
    "fails to return!"
    # Object stays at target location
```

**Impaired** = Confusion OR Stunned OR Blind OR Hallucination OR Fumbling

### 7.3 Aklys Return

Same mechanics as Mjollnir return (99% return, 99% catch if not impaired) but:
- Uses tethered display (DISP_TETHER with BACKTRACK)
- No role or strength requirement
- Must be wielded as primary weapon
- Range capped at 4 by tether length

### 7.4 Boomerang Return

See Section 6.3. Uses unique curved flight path. Return happens at step 9. DEX check to catch: `rn2(20) < DEX` succeeds.

### 7.5 Monster Throw-and-Return (aklys)

```
made_it_back = rn2(100)   # 99% returns
IF made_it_back:
    IF NOT impaired AND rn2(100):
        # Caught, re-wielded
    ELSE:
        IF rn2(2) == 0: lands at feet
        ELSE: hits thrower for rn2(2) + rnd(3) damage
            # Can kill the monster if artifact_hit applies
ELSE:
    # "loud snap!" -- tether breaks
    # Object stays at target location, not returned
```

---

## 8. Breakage / Mulching of Projectiles

### 8.1 `should_mulch_missile` (Projectile Destruction on Hit)

Called when a thrown/shot projectile hits a monster successfully.

**Eligibility**: must be `is_ammo(obj)` or `is_missile(obj)`, AND:
- NOT a boomerang
- NOT a magic item (`objects[].oc_magic`)

```
chance = 3 + greatest_erosion(obj) - obj.spe

IF chance > 1:
    broken = (rn2(chance) != 0)   # probability = (chance-1)/chance
ELSE:
    broken = (rn2(4) == 0)       # flat 25%

# Blessed save
IF obj.blessed:
    IF context.mon_moving (monster threw it):
        IF rn2(3) == 0: broken = FALSE     # 33% save
    ELSE (hero threw it):
        IF rnl(4) == 0: broken = FALSE     # luck-adjusted ~25% save

# Hard material save
IF (gem class AND oc_tough) OR obj.otyp == FLINT:
    IF rn2(2) == 0: broken = FALSE          # 50% save
```

**Typical break rates (on hit):**

| Enchantment | Erosion | chance | Base Break Rate |
| ----------- | ------- | ------ | --------------- |
| +0          | 0       | 3      | 67%             |
| +0          | 2       | 5      | 80%             |
| +3          | 0       | 0      | 25% (flat)      |
| +5          | 0       | -2     | 25% (flat)      |
| +0          | 0, blessed | 3   | ~50% (67% * ~75%) |

### 8.2 `breaktest` (Glass/Fragile Object Shatters on Surface)

When projectile hits a non-soft surface without hitting a monster:

```
IF obj_resists(obj, nonbreakchance, 99): FALSE  # artifacts/magical items resist
    # nonbreakchance = 90 for glass armor, 1 for everything else

IF material == GLASS AND NOT artifact AND NOT GEM_CLASS: TRUE

SWITCH (otyp or POTION_CLASS):
    EXPENSIVE_CAMERA, all potions, EGG, CREAM_PIE, MELON,
    ACID_VENOM, BLINDING_VENOM: TRUE
    default: FALSE
```

### 8.3 Monster-Thrown Projectile Destruction (`drop_throw`)

```
IF obj is CREAM_PIE or VENOM_CLASS: always destroyed
IF hit AND obj is EGG: always destroyed
IF hit: destroyed if should_mulch_missile(obj) returns TRUE
IF miss: object placed at landing spot (not destroyed)
```

---

## 9. Monster Throwing/Shooting at Player

### 9.1 Monster Weapon Selection Priority (`select_rwep`)

1. Cockatrice eggs (any monster)
2. Cream pies (Kops only)
3. Boulders (giants only)
4. Polearms (if dist2 <= 13 and couldsee; also Art_Snickersnee)
5. Throw-and-return weapons (aklys; if dist2 <= arw.range and couldsee)
6. Gems via sling (checked before darts in rwep list)
7. Ranged weapons from `rwep[]`:
   spears (dwarvish/silver/elven/orcish), javelin, shuriken, ya,
   arrows (silver/elven/orcish), crossbow bolts,
   daggers (silver/elven/orcish), knife, flint, rock,
   loadstone (only if not cursed), luckstone, dart, cream pie

Monsters search inventory for matching launchers (bow variants, sling, crossbow).

### 9.2 Monster To-Hit vs Player (`m_throw` -> `thitu`)

```
hitv = 3 - distmin(hero, monster)
IF hitv < -4: hitv = -4
IF is_elf(monster.data) AND objects[obj.otyp].oc_skill == -P_BOW:
    hitv += 1
    IF MON_WEP(monster) is ELVEN_BOW: hitv += 1
IF bigmonst(hero): hitv += 1
hitv += 8 + obj.spe
IF dam < 1: dam = 1

# thitu check:
dieroll = rnd(20)
IF u.uac + hitv <= dieroll: MISS
ELSE: HIT
```

Monster elf archers shooting elven arrows get +1 damage bonus.

For special items (egg, cream pie, blinding venom): `hitv = 8, dam = 0`.

### 9.3 Monster-on-Monster To-Hit (`ohitmon`)

```
tmp = 5 + find_mac(target) + omon_adj(target, obj, FALSE)
IF archer exists AND target == intended target:
    IF archer.m_lev > 5: tmp += archer.m_lev - 5
    IF launcher.oartifact: tmp += spec_abon(launcher, target)
IF tmp < rnd(20): MISS
ELSE: HIT
```

### 9.4 Engagement Conditions

- `BOLT_LIM = 8` -- maximum ranged attack distance
- `MON_POLE_DIST = 5` (dist2) -- monster polearm reach (knight's move)
- `PET_MISSILE_RANGE2 = 36` (dist2) -- max range for pet shooting at monsters
- Monster must have line of sight (`lined_up` -> `linedup`)
- Retreating player: monster shoots with probability `rn2(BOLT_LIM - distance)` failing, so closer = more likely to throw

---

## 10. Throwing Upward/Downward

### 10.1 Throwing Upward (dz < 0)

```
IF returning_missile AND NOT impaired:
    # Hits ceiling and returns to hand (auto-caught)

ELSE:
    toss_up(obj, hitsroof)
    # hitsroof = rn2(5) != 0 AND NOT Underwater  (80% chance if has ceiling)

    IF no ceiling: "flies up into the sky" (object lost)
    IF hitsroof AND breaktest(obj): may shatter on ceiling
    ELSE: falls back on hero's head

    # Damage from falling:
    IF potion: potionhit effect
    IF breaktest: may break on hero (egg->petrify, cream pie->blind, etc.)
    IF harmless_missile: "It doesn't hurt." (scrolls, cloth, etc.)
    ELSE:
        dmg = dmgval(obj, &youmonst)
        IF dmg == 0:  # non-weapon
            dmg = (obj.owt + 99) / 100
            dmg = max(1, dmg <= 1 ? 1 : rnd(dmg))
            IF dmg > 6: dmg = 6
        IF hard_helmet AND (NOT silver OR NOT Hate_silver):
            dmg = 1   # helmet protects
        dmg += u.udaminc
        IF dmg < 0: dmg = 0
        dmg = Maybe_Half_Phys(dmg)
```

### 10.2 Throwing Downward (dz > 0)

```
IF riding steed AND obj is potion AND rn2(6):  # 83% chance
    potionhit(steed, obj)   # splash steed
ELSE:
    hitfloor(obj)   # object hits floor, may break or fall into trap
```

---

## 11. Monster Throwing Poison Damage

### 11.1 Monster-on-Monster (`ohitmon`, mthrowu.c)

When a monster's poisoned projectile hits another monster:

```
IF obj.opoisoned AND is_poisonable:
    IF resists_poison(target): no extra damage
    ELIF rn2(30):     damage += rnd(6)     # ~97% chance
    ELSE:             damage = target.mhp  # ~3% instakill
```

### 11.2 Monster-on-Hero (`m_throw` -> `poisoned()`, mthrowu.c)

When a monster's poisoned projectile hits the hero, the `poisoned()` function is called with instakill chance parameter 10:

```
IF obj.opoisoned AND is_poisonable:
    poisoned(name, A_STR, killer, chance=10)
    # Inside poisoned(): instakill if rn2(chance) == 0 => 10% instakill
    # Same rate as hero-on-monster (Section 5.8)
    # If life-saving triggered from the hit damage, chance is set to 0 (no instakill, attribute loss only)
```

Note: Monster-on-monster poison has a 3% instakill rate (`rn2(30)`), while monster-on-hero poison has a 10% instakill rate (`poisoned()` with chance=10), the same as hero-on-monster.

---

## 12. Additional Mechanics

### 12.1 Fire-Assist (`dofire`, dothrow.c)

When the hero uses the `f` (fire) command with quivered ammo and `iflags.fireassist` is set, the game automatically locates a matching launcher:

```
IF uquiver is ammo AND fireassist AND NOT skip_fireassist:
    IF uwep matches uquiver: use uwep (no swap needed)
    ELIF uswapwep matches uquiver: swap weapons, then retry fire
    ELIF find_launcher(uquiver) in inventory: wield that launcher, then retry fire
```

This is a UI convenience -- the actual combat mechanics are unchanged.

### 12.2 Gem-to-Unicorn Interaction (dothrow.c)

Throwing gems at unicorns bypasses normal ranged combat entirely. When `obj.oclass == GEM_CLASS` and target is a unicorn and material is not MINERAL and hero is not using a sling:

```
IF target is helpless: automatic miss (tmiss)
ELIF target is tame: catches and drops the gem (no effect)
ELSE: catches the gem -> gem_accept(mon, obj)
    # gem_accept adjusts Luck based on gem value and alignment:
    #   co-aligned unicorn + valuable gem: Luck boost
    #   cross-aligned unicorn: random Luck change
    #   worthless glass: no Luck effect, no anger
```

Rocks and gray stones (MINERAL material) are treated as attacks, not gifts. Gems shot via sling (`uslinging()`) are also treated as attacks.

### 12.3 Iron Bars Blocking (`hits_bars`, mthrowu.c)

Projectiles can be stopped by iron bars in the flight path. The `hits_bars()` function determines whether a missile passes through or is blocked based on object size and type:

```
# Objects that always pass through: whips, gold, coins
# Objects that always hit bars: boulders, large weapons (including polearms)
# Small objects (ammo, gems, darts): hit only if always_hit OR random chance

IF hits:
    hit_bars(obj) -> may break the object on impact
```

At point-blank range (one square away), the random chance for small objects hitting bars is skipped.

### 12.4 Shopkeeper Catching Pick-Axes (dothrow.c)

When a thrown object lands on a shopkeeper's square and the shopkeeper is present:

```
IF mon.isshk AND is_pick(obj):
    shopkeeper snatches the pick-axe
    # "Snigglenose snatches up the pick-axe."
    check_shop_obj for payment tracking
    mpickobj(mon, obj)   # shopkeeper takes possession
```

### 12.5 Donning Interrupts

Unlike melee attacks (which call `stop_donning()` in `mhitu.c` to interrupt armor donning/doffing when the hero is hit), ranged projectile hits via `thitu()` do NOT interrupt donning. This means a hero can continue putting on or removing armor while being pelted by projectiles, as long as the damage does not kill or incapacitate.

---

## 13. Key Constants

| Constant            | Value | Source       |
| ------------------- | ----- | ------------ |
| BOLT_LIM            | 8     | hack.h:50    |
| WT_TO_DMG           | 100   | weight.h:17  |
| WT_SPLASH_THRESHOLD | 9     | weight.h:11  |
| AKLYS_LIM           | 4     | weapon.c:512 |
| MON_POLE_DIST       | 5     | hack.h:1441  |
| PET_MISSILE_RANGE2  | 36    | hack.h:1442  |
| MZ_TINY             | 0     |              |
| MZ_SMALL            | 1     |              |
| MZ_MEDIUM           | 2     |              |
| MZ_LARGE            | 3     |              |
| MZ_HUGE             | 4     |              |
| MZ_GIGANTIC         | 7     |              |
| STR18(x)            | 18+x  | attrib.h:36  |
| STR19(x)            | 100+x | attrib.h:37  |

---

## 测试向量

### TV-01: Basic bow+arrow range

```
Input:  ACURRSTR = 18 (STR 18), arrow (owt=1), wielding bow
Output: crossbowing = FALSE
        urange = 18/2 = 9
        range = 9 - (1/40) = 9 - 0 = 9
        ammo_and_launcher: TRUE, not crossbow
        range += 1 = 10
        Final range = 10
```

### TV-02: Crossbow bolt range (strength-independent)

```
Input:  ACURRSTR = 10, crossbow bolt, wielding crossbow
Output: crossbowing = TRUE
        urange = 18/2 = 9 (hardcoded 18 for crossbow)
        range = 9 - (1/40) = 9
        ammo_and_launcher + crossbow: range = BOLT_LIM = 8
        Final range = 8
```

### TV-03: Arrow thrown by hand (no bow)

```
Input:  ACURRSTR = 18, arrow (owt=1), uwep = long sword
Output: urange = 18/2 = 9
        range = 9 - 0 = 9
        is_ammo but NOT ammo_and_launcher, obj.oclass != GEM_CLASS
        range /= 2 = 4
        Final range = 4
```

### TV-04: Mjollnir range (STR 25 Valkyrie)

```
Input:  ACURRSTR = 25, Mjollnir (owt=50), Valkyrie, STR >= STR19(25)
Output: urange = 25/2 = 12
        range = 12 - (50/40) = 12 - 1 = 11
        Mjollnir override: range = (11+1)/2 = 6
        Final range = 6
```

### TV-05: Boundary -- STR threshold for Mjollnir throwing

```
Input A: Raw A_STR = STR18(100) = 118
         STR19(25) = 125
         118 < 125: CANNOT throw Mjollnir ("It's too heavy.")

Input B: Raw A_STR = STR19(25) = 125
         125 >= 125: CAN throw Mjollnir
```

### TV-06: Multishot -- Elven Ranger, Expert bow, max setup

```
Input:  Role=Ranger, Race=Elf, skill=Expert, DEX=18
        ELVEN_ARROW (quan=20) + ELVEN_BOW wielded
        Not weakmultishot (Ranger, DEX>6, not Fumbling)

Calculation:
    multishot = 1
    Expert: +1, fall-through Skilled (not weak): +1  => 3
    Role (Ranger, not dagger): +1                     => 4
    Race (Elf, elven arrow+elven bow): +1             => 5
    Quest artifact: not applicable (bow is elven bow, not Longbow of Diana)
    No crossbow penalty
    Final: rnd(5) => 1..5

Output: multishot range = [1, 5]
```

### TV-07: Multishot -- Ranger with Longbow of Diana

```
Input:  Role=Ranger, Race=Elf, skill=Expert, DEX=18
        ELVEN_ARROW (quan=20) + Longbow of Diana (quest artifact bow) wielded

Calculation:
    multishot = 1
    Expert: +1, Skilled (not weak): +1                => 3
    Role (Ranger, not dagger): +1                     => 4
    Race (Elf, elven arrow + NOT elven bow): +0       => 4
    Quest artifact + ammo matches: +1                 => 5
    Final: rnd(5) => 1..5

Output: multishot range = [1, 5]
        (Same max as TV-06, racial bow bonus traded for quest artifact bonus)
```

### TV-08: Boundary -- crossbow multishot STR penalty

```
Input A: Race=Human, ACURRSTR=18, crossbow bolt+crossbow, accumulated multishot=4
         Threshold: 18 (non-Gnome)
         18 < 18? NO -- no penalty
         Final: rnd(4) => 1..4

Input B: ACURRSTR=17, same setup, accumulated multishot=4
         17 < 18? YES
         Penalty: multishot = rnd(4), then general rnd(result)
         E.g., rnd(4)=3, then rnd(3)=2
         Expected value: sum_{k=1}^{4} (1/4) * sum_{j=1}^{k} (1/k) * j
                       = sum_{k=1}^{4} (1/4) * (k+1)/2
                       ≈ 1.875
         vs. no-penalty expected value of rnd(4) ≈ 2.5

Output A: range [1,4], expected 2.5
Output B: range [1,4], expected ~1.875 (double-random penalty)
```

### TV-09: To-hit -- thrown dagger at sleeping medium monster

```
Input:  Level=10, Luck=3, find_mac=5, uhitinc=0, DEX=16
        Target: sleeping, MZ_MEDIUM, can't move
        Dagger (throwing_weapon, spe=+0, oc_hitbon=+2), distance=2
        Basic dagger skill

Calculation:
    tmp = -1 + 3 + 5 + 0 + 10 = 17
    DEX 16: +(16-14) = +2                            => 19
    Distance 2: disttmp = 3-2 = +1                   => 20
    omon_adj: size +0, sleeping +2, immobile +4,
              hitval(+0 spe + +2 hitbon) = +2         => +8
    Thrown non-ammo, throwing_weapon: +2               => 30
    weapon_hit_bonus(Basic dagger): +0                 => 30

    dieroll = rnd(20), max 20
    30 >= any dieroll: guaranteed hit

Output: tmp = 30, hit rate = 100%
```

### TV-10: Breakage -- +0 uneroded arrow vs +5 blessed arrow

```
Input A: +0 arrow, erosion 0, not blessed, hit target
         chance = 3 + 0 - 0 = 3
         broken = rn2(3) != 0 => 67% break

Input B: +5 blessed arrow, erosion 0, hit target (hero throwing)
         chance = 3 + 0 - 5 = -2, chance <= 1
         broken = rn2(4) == 0 => 25% break
         Blessed save: rnl(4) == 0 saves (~25% at Luck 0)
         Effective: 25% * 75% = ~19% break

Output A: break rate ~67%
Output B: break rate ~19%
```

### TV-11: Boomerang catch vs self-hit

```
Input A: DEX=18, not Fumbling, +0 boomerang
         Catch check: rn2(20) >= 18? Probability = 2/20 = 10% fail
         Catch rate: 90%

Input B: DEX=10, not Fumbling
         rn2(20) >= 10? Probability = 10/20 = 50% fail
         Catch rate: 50%

Input C: Fumbling, any DEX
         Always fails catch, always hits self
         Self-hit: thitu(10+spe, dmgval(boomerang, hero))
```

### TV-12: Monster elf lord shooting elven arrows

```
Input:  Elf lord, ELVEN_ARROW (quan=30, not cursed),
        ELVEN_BOW (spe=+3, not cursed)

monmulti calculation:
    multishot = 1
    is_lord: +1                                       => 2
    ELVEN_ARROW not cursed: +1                        => 3
    ELVEN_BOW + ammo matches + not cursed: +1         => 4
    spe=3 > 1: +rounddiv(3,3) = +1                   => 5
    rnd(5) => 1..5
    multishot_class_bonus: elf lord is not a hero role => +0
    racial: elf + elven arrow + elven bow: +1
    Final: rnd(5) + 1 => 2..6

Output: multishot range = [2, 6]
```

### TV-13: Monster throw to-hit vs player (AC -10)

```
Input:  Player AC = -10, monster throws +0 orcish arrow, distance 5
        Monster: orc, not elf, not special

Calculation:
    hitv = 3 - 5 = -2
    Not elf, not bigmonst
    hitv += 8 + 0 = 6
    Total hitv = 6

    thitu: u.uac + hitv = -10 + 6 = -4
    Hit if -4 > rnd(20)?  Never (rnd(20) >= 1)

Output: hitv = 6, hit rate = 0% (impossible to hit AC -10 with hitv 6)
```

### TV-14: Damage -- silver arrow shot through bow at vampire

```
Input:  +2 silver arrow, +1 bow (Expert), STR 18 (dbon=+2)
        Target: vampire (hates silver, small-sized)
        u.udaminc = +1 (ring of increase damage)

dmgval calculation:
    rnd(6) (arrow oc_wsdam=6)
    +2 (spe)
    silver vs hates_silver: +rnd(20)
    -0 (erosion)
    min 1

hmon_hitmon_dmg_recalc:
    u.udaminc = +1
    Strength: SKIPPED (ammo_and_launcher)
    weapon_dam_bonus(bow, Expert) = +2
    dmgbonus = 1 + 2 = 3

No elf/samurai racial bonus (not elf race in this example)

Total: rnd(6) + 2 + rnd(20) + 3
Range: 1+2+1+3 = 7 to 6+2+20+3 = 31

Output: damage range = [7, 31] (before minimum-1 check, which is irrelevant here)
```
