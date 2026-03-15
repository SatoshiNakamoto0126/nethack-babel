# Review: melee-combat.md

## Summary
- Quality: GOOD
- Test vectors: 15 [PASS]
- Boundary conditions: 6 [PASS]
- C/Rust code leaks: NONE
- Bug markers: 3

## A. Format Compliance

- [A1] Test vectors: 15 individual test cases (TV1-TV15). PASS (>=10)
- [A2] Boundary conditions: 6 explicitly labeled boundary tests (TV5: negative enchantment clamp, TV6: Shade immunity, TV9: overloaded+trapped can't hit, TV12: STR 18 exact boundary, TV14: Luck -13 extreme, TV15: unskilled two-weapon extreme). PASS (>=2)
- [A3] No C code: all code blocks use pseudocode notation. No `int `, `void `, `static `, `return `, `if (`, `struct `, `#include`, `#define` as actual code. PASS
- [A4] No Rust code: no `fn `, `let mut`, `match `, `impl `, `pub fn`. PASS
- [A5] Pseudocode present: Section 8 provides full flow pseudocode for `do_attack`, `hitum`, and `hmon_hitmon`. PASS
- [A6] Exact formulas: all formulas include explicit constants (e.g., spelarmr=20, luck formula `sgn(Luck)*((abs(Luck)+2)/3)`, strength tables with internal encoding). PASS
- [A7] Bug markers: 3 instances of `[疑似 bug，如实记录原始行为]` (sections 5.1/TV5, note 3, note 8/10). PASS

## B. Content Coverage

v2 spec 15.1 keywords:
- **thac0**: Covered extensively in sections 2-4 (THAC0/roll_to_hit formula, find_roll_to_hit decomposition). PASS
- **伤害骰**: Covered in section 5.1 (dmgval, weapon damage dice tables for small/large). PASS
- **力量加成**: Covered in section 5.4 (dbon table) and 5.6.1 (two-hand/two-weapon adjustments). PASS
- **附魔**: Covered in section 3 (hitval spe bonus) and 5.1 (dmgval spe bonus, negative enchantment clamp). PASS
- **特殊伤害**: Covered in sections 5.2 (silver, blessed, axe vs wood, artifact light), 7.1-7.7 (backstab, jousting, poison, artifact, weapon shatter, Cleaver). PASS

All 5 keywords have dedicated coverage.

## C. Source Accuracy

### C1. Function Coverage

Non-trivial public/static functions in uhitm.c (melee-combat relevant):
1. `find_roll_to_hit` -- COVERED (section 2)
2. `do_attack` -- COVERED (section 8)
3. `hitum` -- COVERED (sections 8-9)
4. `hitum_cleave` -- COVERED (section 7.5)
5. `double_punch` -- COVERED (section 6.4)
6. `known_hitum` -- partially covered (in flow pseudocode)
7. `hmon` / `hmon_hitmon` -- COVERED (section 8 pseudocode)
8. `hmon_hitmon_barehands` -- COVERED (section 6.1-6.2)
9. `hmon_hitmon_weapon_ranged` -- mentioned but not main focus (melee spec)
10. `hmon_hitmon_weapon_melee` -- COVERED (sections 7.1-7.4)
11. `hmon_hitmon_dmg_recalc` -- COVERED (sections 5.4-5.6)
12. `hmon_hitmon_poison` -- COVERED (section 7.6)
13. `hmon_hitmon_jousting` / `joust` -- COVERED (section 7.3)
14. `hmon_hitmon_stagger` -- COVERED (section 6.5)
15. `hmon_hitmon_splitmon` -- NOT COVERED (pudding splitting)
16. `backstabbable` -- COVERED (section 7.1)
17. `passive` -- COVERED (section 11)
18. `passive_obj` -- partially covered (in section 11)
19. `hmonas` -- COVERED (section 9.2)
20. `shade_miss` / `shade_aware` -- COVERED (section 5.1, TV6)
21. `missum` -- COVERED (TV11 miss message analysis)
22. `mhitm_knockback` -- mentioned (section 8 pseudocode)
23. `attack_checks` -- mentioned (section 8 pseudocode)
24. `check_caitiff` -- NOT COVERED
25. `mon_maybe_unparalyze` -- COVERED (note 10)
26. `m_is_steadfast` -- NOT COVERED

weapon.c (melee-combat relevant):
27. `hitval` -- COVERED (section 3)
28. `dmgval` -- COVERED (section 5.1)
29. `special_dmgval` -- COVERED (section 6.2)
30. `abon` -- COVERED (section 2.1.1)
31. `dbon` -- COVERED (section 5.4)
32. `weapon_hit_bonus` -- COVERED (section 4)
33. `weapon_dam_bonus` -- COVERED (section 5.6)
34. `skill_init` -- COVERED (section 10.3)
35. `use_skill` -- COVERED (section 10.2)
36. `can_advance` -- COVERED (section 10.2)
37. `slots_required` -- COVERED (section 10.2)
38. `weapon_type` -- referenced
39. `enhance_weapon_skill` -- NOT COVERED (UI function, minor)

Coverage: 30/39 functions covered = ~77%. Missing functions are mostly minor (pudding splitting, steadfast check, caitiff check, UI).

### C2. Formula Spot-Checks (3)

**Formula 1: find_roll_to_hit (uhitm.c:376-427)**

Spec says:
```
roll_to_hit = 1 + abon() + find_mac(target) + u.uhitinc
  + sgn(Luck)*((abs(Luck)+2)/3) + maybe_polyd(mlevel, ulevel)
  + monster_state + role_race + encumbrance + trap + weapon
```

Source (line 376-424):
```
tmp = 1 + abon() + find_mac(mtmp) + u.uhitinc
      + (sgn(Luck) * ((abs(Luck) + 2) / 3))
      + maybe_polyd(youmonst.data->mlevel, u.ulevel);
```
Then adds state bonuses (+2/+2/+2/+4), monk adjustments, elf vs orc, encumbrance `(tmp2*2)-1`, trap -3, weapon bonuses.

VERDICT: EXACT MATCH. All constants and operators verified.

**Formula 2: dbon() (weapon.c:988-1011)**

Spec table:
| STR range | bonus |
|-----------|-------|
| < 6 | -1 |
| 6-15 | 0 |
| 16-17 | +1 |
| 18 exact | +2 |
| 18/01-18/75 | +3 |
| 18/76-18/90 | +4 |
| 18/91-18/99 | +5 |
| >= 18/100 | +6 |

Source:
- `str < 6` -> -1
- `str < 16` -> 0
- `str < 18` -> 1
- `str == 18` -> 2
- `str <= STR18(75)` -> 3
- `str <= STR18(90)` -> 4
- `str < STR18(100)` -> 5
- else -> 6

VERDICT: EXACT MATCH. All thresholds and values correct.

**Formula 3: Two-hit strength bonus (uhitm.c:1461-1468)**

Spec says:
```
if twohits:
    strbonus = ((3 * absbonus + 2) / 4) * sgn(strbonus)
elif melee AND uwep AND bimanual(uwep):
    strbonus = ((3 * absbonus + 1) / 2) * sgn(strbonus)
```

Source (lines 1465-1468):
```c
if (hmd->twohits)
    strbonus = ((3 * absbonus + 2) / 4) * sgn(strbonus);
else if (hmd->thrown == HMON_MELEE && uwep && bimanual(uwep))
    strbonus = ((3 * absbonus + 1) / 2) * sgn(strbonus);
```

VERDICT: EXACT MATCH. Constants, operators, and integer division all verified.

### C3. Missing Mechanics

**CRITICAL**: None identified. All core gameplay mechanics (hit determination, damage calculation, special attacks, skill system) are covered.

**MINOR**:
1. Pudding splitting (hmon_hitmon_splitmon): Iron/metal weapon hits cause pudding to divide. Not documented.
2. `check_caitiff`: Knights get alignment penalty for attacking fleeing monsters. Not mentioned.
3. `m_is_steadfast`: Monsters that resist knockback. Not documented (though knockback is mentioned).
4. First weapon hit conduct tracking (`first_weapon_hit`): Minor conduct detail.
5. Boomerang breakage in `hmon_hitmon_weapon_ranged`: Edge case for ranged-as-melee.

### C4. Test Vector Verification

**TV1 Verification** (Basic hit calculation):
```
STR=16, DEX=10, ulevel=1, Luck=0, AC=10, long sword +0 Basic, Unencumbered
abon(): STR=16 -> str < 17 -> sbon=0; ulevel < 3 -> sbon=1; DEX=10 -> sbon (no adjustment) -> return 1. CORRECT.
find_mac: 10. CORRECT.
luck: sgn(0)*((0+2)/3) = 0. CORRECT.
level: 1. CORRECT.
hitval: spe=0 + oc_hitbon=0 -> 0. CORRECT.
weapon_hit_bonus: type <= P_LAST_WEAPON, P_SKILL=P_BASIC -> 0. CORRECT.
Total: 1 + 1 + 10 + 0 + 0 + 1 + 0 + 0 = 13. CORRECT.
Hit condition: dieroll <= 12 (60%). CORRECT (tmp > dieroll means 13 > 1..12).
```
PASS.

**TV5 Verification** (Negative enchantment clamp):
```
short sword -5, Basic.
dmgval: rnd(6)=1, is_weapon -> tmp += spe=-5 -> tmp=-4; tmp < 0 -> tmp=0. CORRECT.
hmon_hitmon: hmd.dmg=0, test (hmd.dmg > 0) fails -> skip dmg_recalc. CORRECT.
hmd.dmg < 1: get_dmg_bonus=TRUE, !Shade -> dmg=1. CORRECT.
```
Source lines 1806-1817 confirm: `if (hmd.dmg > 0) hmon_hitmon_dmg_recalc(...)` then `if (hmd.dmg < 1) ... hmd.dmg = (hmd.get_dmg_bonus && !mon_is_shade) ? 1 : 0`.
PASS.

**TV13 Verification** (Two-handed strength bonus):
```
dbon()=6, bimanual, melee.
absbonus=6, strbonus = ((3*6+1)/2)*1 = 19/2 = 9. CORRECT.

Mapping check for dbon=4:
((3*4+1)/2)*1 = 13/2 = 6. Spec says 6. CORRECT.

Mapping check for dbon=2:
((3*2+1)/2)*1 = 7/2 = 3. Spec says 3. CORRECT.
```
PASS.

## D. Recommendations

1. **Add pudding splitting documentation**: The iron/metal weapon causing pudding division (hmon_hitmon_splitmon) is a well-known gameplay mechanic. Add a brief section in "Special Attack Mechanisms" covering the material check (IRON or METAL), the HP > 1 condition, and the exclusion of ammo/missiles.

2. **Add check_caitiff note**: Knights/Samurai alignment penalties for attacking fleeing/weak monsters are relevant to melee combat flow. Consider a brief mention in section 8 or as a note.

3. **Minor: Healer anatomy knowledge condition**: The spec correctly lists the condition (`oc_skill == P_KNIFE`) and the formula (`min(3, mvitals[].died / 6)`), but should explicitly note the `obj->oclass == WEAPON_CLASS` guard in addition to `oc_skill == P_KNIFE` (source line 950-951), since weptools with P_KNIFE skill would not qualify.

4. **Minor: Weapon shatter flee probability**: The spec says "75% probability to flee" which matches `rn2(4)` (line 1009), but the flee duration `d(2,3)` is not mentioned. Consider adding it.

5. **No action needed on test vectors**: All verified test vectors produce correct results. The 15 test vectors with 6 boundary conditions exceed minimum requirements.
