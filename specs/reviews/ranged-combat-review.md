# Review: ranged-combat.md

**Reviewer**: Claude Opus 4.6 (1M context)
**Date**: 2026-03-14
**Spec file**: `/Users/hz/Downloads/nethack-babel/specs/ranged-combat.md`
**Source files reviewed**: `src/dothrow.c`, `src/mthrowu.c`

---

## Summary

The ranged combat spec is thorough, well-structured, and highly accurate. It covers throwing mechanics, launcher/ammo matching, multishot, to-hit formulas, damage computation, breakage, returning weapons, and monster throwing in substantial detail. Format compliance is excellent with 14 test vectors, pseudocode throughout, and proper use of [疑似 bug] markers. Two minor formula inaccuracies and a few missing mechanics were found.

**Verdict**: PASS with minor corrections needed.

---

## A. Format Compliance

| Check | Status | Notes |
|-------|--------|-------|
| [A1] 10+ test vectors | PASS | 14 test vectors (TV-01 through TV-14), all with worked calculations |
| [A2] 2+ boundary conditions | PASS | TV-05 (STR threshold for Mjollnir), TV-08 (crossbow STR penalty boundary), TV-10 (breakage enchantment boundary) |
| [A3] No C code | PASS | No C code blocks; all algorithms in pseudocode |
| [A4] No Rust code | PASS | No Rust code present |
| [A5] Pseudocode present | PASS | All major algorithms have pseudocode: range calc, multishot, to-hit, damage, breakage, returning weapons |
| [A6] Exact formulas with constants | PASS | BOLT_LIM=8, WT_TO_DMG=100, MZ constants, STR tables all explicitly stated |
| [A7] [疑似 bug] markers | PASS | Two properly marked: double-random crossbow penalty (Section 3.8), monster multishot ordering asymmetry (Section 3.9) |

---

## B. Content Coverage

| Keyword | Covered | Notes |
|---------|---------|-------|
| 投掷 (throwing) | YES | Sections 1-3, 10 cover range, stamina, misfire, multishot |
| 射击 (shooting) | YES | Sections 2, 4-5 cover launcher/ammo matching, to-hit, damage |
| 弹药匹配 (ammo matching) | YES | Section 2.1 covers `ammo_and_launcher` / `matching_launcher` with full table |

---

## C. Source Accuracy

### C1. Function Coverage

**Non-trivial public functions in `src/dothrow.c`:**

| # | Function | Covered in Spec | Notes |
|---|----------|----------------|-------|
| 1 | `multishot_class_bonus()` | YES | Section 3.4 |
| 2 | `dothrow()` | Implicit | Entry point, not mechanically relevant |
| 3 | `dofire()` | NO | MINOR: fire-assist launcher swap logic not covered |
| 4 | `endmultishot()` | NO | MINOR: utility function |
| 5 | `hitfloor()` | Partial | Referenced in Section 10.2 |
| 6 | `walk_path()` | NO | MINOR: Bresenham path tracing, infrastructure |
| 7 | `hurtle_jump()` | NO | MINOR: jumping variant |
| 8 | `hurtle_step()` | NO | MINOR: recoil step handler |
| 9 | `hurtle()` | Partial | Referenced in Section 1.3 (recoil) |
| 10 | `mhurtle()` | NO | MINOR: monster knockback |
| 11 | `will_hurtle()` | NO | MINOR: utility |
| 12 | `harmless_missile()` | Partial | Referenced in Section 10.1 |
| 13 | `throwing_weapon()` | YES | Section 2.3 |
| 14 | `throwit()` | YES | Core logic mapped in Sections 1-7 |
| 15 | `throwit_mon_hit()` | Implicit | Delegates to thitmonst |
| 16 | `omon_adj()` | YES | Section 4.4 |
| 17 | `should_mulch_missile()` | YES | Section 8.1 |
| 18 | `thitmonst()` | YES | Sections 4.1-4.9 |
| 19 | `hero_breaks()` | Partial | Referenced in Section 8.2 |
| 20 | `breaks()` | NO | MINOR: non-hero breakage variant |
| 21 | `breaktest()` | YES | Section 8.2 |
| 22 | `breakobj()` | Partial | Section 8.2 context |
| 23 | `release_camera_demon()` | NO | MINOR: special case |

**Non-trivial public functions in `src/mthrowu.c`:**

| # | Function | Covered in Spec | Notes |
|---|----------|----------------|-------|
| 1 | `m_has_launcher_and_ammo()` | NO | MINOR: check function |
| 2 | `thitu()` | YES | Section 9.2 |
| 3 | `ohitmon()` | YES | Section 9.3 |
| 4 | `m_throw()` | YES | Sections 9, 11 |
| 5 | `thrwmm()` | NO | MINOR: monster-vs-monster throwing entry |
| 6 | `spitmm()` | NO | MINOR: spit attack |
| 7 | `breamm()` | NO | MINOR: breath attack |
| 8 | `m_useupall()` / `m_useup()` | NO | MINOR: inventory management |
| 9 | `thrwmu()` | Partial | Entry point for monster throwing at hero; `select_rwep` covered in 9.1 |
| 10 | `linedup()` / `lined_up()` | Partial | Referenced in Section 9.4 |
| 11 | `hit_bars()` / `hits_bars()` | NO | MINOR: iron bars interaction |

**Coverage: ~65% of public functions are covered. All core gameplay-affecting functions are covered.**

### C2. Formula Verification (3 most important)

**Formula 1: Range calculation (Section 1.1 vs `throwit()` at dothrow.c:1614-1672)**

Spec says:
```
crossbowing = ammo_and_launcher(obj, uwep) AND weapon_type(uwep) == P_CROSSBOW
IF crossbowing: urange = 18 / 2
ELSE: urange = ACURRSTR / 2
```

Source (line 1614-1616):
```c
crossbowing = (ammo_and_launcher(obj, uwep)
               && weapon_type(uwep) == P_CROSSBOW);
urange = (crossbowing ? 18 : (int) ACURRSTR) / 2;
```

**MATCH.** All subsequent range modifiers (ammo, air level, boulder, Mjollnir, tether, underwater) verified correct.

**Formula 2: Hero multishot (Section 3 vs `throw_obj()` at dothrow.c:158-238)**

Spec's multishot accumulation order:
1. Start at 1
2. Skill bonus (Expert: +1 fall-through Skilled: +1 if not weak)
3. Role bonus (`multishot_class_bonus`)
4. Racial bonus (if not weakmultishot)
5. Quest artifact bonus (if not weakmultishot)
6. Crossbow STR penalty
7. Final rnd()

Source verification: Lines 161-237 confirm this exact order.

**MATCH.** The `weakmultishot` conditions (lines 170-174) match Section 3.2 exactly. The crossbow penalty threshold (line 228-231) with `Race_if(PM_GNOME) ? 16 : 18` matches Section 3.7.

**Formula 3: thitmonst to-hit (Section 4.1 vs dothrow.c:2036-2046)**

Spec says:
```
tmp = -1 + Luck + find_mac(mon) + u.uhitinc + maybe_polyd(youmonst.data.mlevel, u.ulevel)
```

Source (line 2036-2037):
```c
tmp = -1 + Luck + find_mac(mon) + u.uhitinc
      + maybe_polyd(gy.youmonst.data->mlevel, u.ulevel);
```

**MATCH.** DEX modifiers (lines 2038-2045) and distance modifier (lines 2051-2054) also verified correct.

### C3. Missing Mechanics

**CRITICAL:**
- None found.

**MINOR:**
1. **`dofire()` fire-assist logic** (dothrow.c:557-580): The spec does not cover the fire-assist system where the game automatically swaps to a matching launcher when firing quivered ammo. This is primarily a UI convenience feature rather than a combat mechanic.

2. **Gem-to-unicorn special interaction** (dothrow.c:2087-2098): The spec does not describe the special case where throwing gems at unicorns bypasses normal combat and triggers luck changes. This is a significant gameplay mechanic but is more of a "special interaction" than a ranged combat formula.

3. **Iron bars interaction** (`hits_bars()` in mthrowu.c:1471): Projectiles can be stopped by iron bars. Not covered.

4. **Shopkeeper catching thrown pickaxes** (dothrow.c:1809-1817): Shopkeepers can snatch pick-axes that land in their shop. Not covered.

5. **`poisoned()` function details**: Section 5.8 describes poison damage for hero attacks, but the `poisoned()` function called for monster-thrown poison hitting the hero (mthrowu.c:718-726) works differently from the spec's description of monster poison. The `poisoned()` function's instakill chance parameter is 10 (same as hero), not the `rn2(30)` formula described in Section 11. The spec correctly describes the monster-on-monster path (ohitmon, lines 402-415) using `rn2(30)`, but the monster-on-hero path uses `poisoned()` with parameter 10, which gives `rn2(10)==0` = 10% instakill, same as hero-on-monster.

### C4. Test Vector Verification

**TV-02: Crossbow bolt range**

Spec claims:
```
crossbowing = TRUE
urange = 18/2 = 9
range = 9 - (1/40) = 9
ammo_and_launcher + crossbow: range = BOLT_LIM = 8
Final range = 8
```

Source (dothrow.c:1614-1638):
- `crossbowing = TRUE` -- correct (ammo_and_launcher + weapon_type == P_CROSSBOW)
- `urange = (crossbowing ? 18 : ACURRSTR) / 2 = 9` -- correct
- `range = 9 - (1/40) = 9 - 0 = 9` (integer division) -- correct
- `if (crossbowing) range = BOLT_LIM;` -> `range = 8` -- correct

**VERIFIED.** Output matches.

**TV-06: Elven Ranger multishot**

Spec claims accumulated multishot = 5, then rnd(5).

Source trace (dothrow.c:161-237):
1. `multishot = 1` (line 161)
2. Expert: `multishot++` -> 2 (line 179), fall through to Skilled, not weak: `multishot++` -> 3 (line 183-184)
3. `multishot_class_bonus(PM_RANGER, ...)`: skill is -P_BOW (not P_DAGGER), so +1 -> 4 (line 190, multishot_class_bonus at line 59)
4. Race PM_ELF, ELVEN_ARROW + ELVEN_BOW: +1 -> 5 (lines 196-198)
5. Quest artifact: "not applicable" -- correct, elven bow is not quest artifact
6. No crossbow penalty
7. `multishot = rnd(5)` -> range [1, 5]

**VERIFIED.** Output matches.

**TV-13: Monster throw to-hit vs AC -10 player**

Spec claims:
```
hitv = 3 - 5 = -2
hitv += 8 + 0 = 6
Total hitv = 6
thitu: u.uac + hitv = -10 + 6 = -4
Hit if -4 > rnd(20)? Never
```

Source (mthrowu.c:698-715):
- `hitv = 3 - distmin(u.ux, u.uy, mon->mx, mon->my)` = `3 - 5` = `-2` -- correct
- Not elf, not bigmonst: no adjustment
- `hitv += 8 + singleobj->spe` = `hitv += 8 + 0 = 6` -- correct (total hitv = 6)
- `thitu(6, dam, ...)`: line 105: `if (u.uac + tlev <= rnd(20))` -> `-10 + 6 = -4 <= rnd(20)` is always true -> always MISS

**VERIFIED.** Output matches.

---

## D. Recommendations

1. **Section 11 (Monster Throwing Poison Damage)**: Clarify that the `rn2(30)` / 3% instakill rate applies to monster-on-monster combat (`ohitmon` in mthrowu.c:408). When a monster throws a poisoned weapon at the *hero*, the `poisoned()` function is called with chance parameter 10, which gives the same 10% instakill as hero-on-monster. Add a note distinguishing the two paths.

2. **Section 6.3 (Boomerang)**: The spec says `nhits = max(1, obj.spe + 1)` but this should be verified against `boomhit()` in `zap.c`. Consider adding a note that this function is in `zap.c`, not in the two primary source files.

3. **Add a brief note about gem-to-unicorn interaction**: Even a one-line note in Section 6 or a separate subsection would help, since it's a well-known gameplay mechanic that bypasses normal ranged combat entirely.

4. **Minor correction in Section 9.2**: The formula shows `hitv += 8 + obj.spe` but does not explicitly note that `dam` is computed by `dmgval()` before calling `thitu()`. The `if dam < 1: dam = 1` clamping (mthrowu.c:713-714) should be noted as happening before `thitu()` is called, not inside it.

5. **Section 5.3 (Strength Bonus)**: The two-weapon and two-handed adjustments to `dbon()` are mentioned but the formulas reference `hmon_hitmon_dmg_recalc` without full detail. Consider either expanding or noting that these adjustments are melee-specific and don't apply to ranged attacks (since thrown weapons are HMON_THROWN, not two-weapon or two-handed contexts).
