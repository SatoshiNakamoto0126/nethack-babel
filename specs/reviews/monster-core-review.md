# Review: monster-core.md

## Summary
- Quality: GOOD
- Test vectors: 22 (10 HP + 5 speed + 3 XP + 12 boundary) [PASS]
- Boundary conditions: 12 [PASS]
- C/Rust code leaks: NONE
- Bug markers: 3

## A. Format Compliance

- [A1] Test vectors: 22 individual test cases across HP calculation (10), movement speed (5), experience (3), and boundary conditions (12). PASS (>=10)
- [A2] Boundary conditions: 12 explicitly labeled boundary conditions (BC1: life-saved mhpmax, BC2: level 0 HP floor, BC3: Wizard killed 50 times, BC4-5: adj_lev extremes, BC6: extinction at 120, BC7: died counter at 255, BC8: grow_up 400 cap, BC9-10: MSLOW/MFAST on speed 1, BC11: vampire HP revert, BC12: life-saved genocided). PASS (>=2)
- [A3] No C code: all code blocks use pseudocode notation (`fn`, `let`, `if`/`else`, no semicolons). No `int `, `void `, `static `, `struct `, `#include`, `#define`. PASS
- [A4] No Rust code: no `let mut`, `match `, `impl `, `pub fn`, `->` used as Rust return type. Uses `-> u8` and `-> int` which are pseudocode-style return annotations, not Rust. PASS
- [A5] Pseudocode present: sections 2.1, 2.2, 2.4, 2.5, 3.2, 4.2, 4.3, 4.4, 5.1, 10.1, 10.3, 10.5, 13, 14 all contain pseudocode blocks. PASS
- [A6] Exact formulas: all formulas include explicit constants (NORMAL_SPEED=12, NATTK=6, golem HP table, movement speed table with computed values). PASS
- [A7] Bug markers: 3 instances of `[疑似 bug]` (section 2.5 healmon analysis, section 8.3 setmangry FIXME, section 14 mlstmv FIXME). PASS

## B. Content Coverage

v2 spec 15.4 monster-core keywords:
- **移动**: Covered in section 3 (speed system, mcalcmove formula, movement execution flow, speed examples table). PASS
- **死亡**: Covered extensively in sections 4.1-4.7 (death hierarchy, mondead, m_detach, life saving, vampire rising, corpse generation, treasure drop). PASS
- **属性检查**: Covered in sections 6 (M1/M2/M3 flags with hex values), 7 (resistance bitmask, resistance check flow, MR2_* extended properties), 11 (locomotion/sensory/special property tables). PASS

All 3 keywords have dedicated coverage.

## C. Source Accuracy

### C1. Function Coverage

Non-trivial public/static functions in mon.c, makemon.c relevant to monster-core:

1. `adj_lev` -- COVERED (section 2.1, section 6 in monster-generation)
2. `newmonhp` -- COVERED (section 2.2)
3. `monhp_per_lvl` -- COVERED (section 2.3)
4. `mon_regen` -- COVERED (section 2.4)
5. `healmon` -- COVERED (section 2.5)
6. `mcalcmove` -- COVERED (section 3.2)
7. `movemon` -- COVERED (section 3.3)
8. `m_calcdistress` -- COVERED (section 14)
9. `mondead` -- COVERED (section 4.2)
10. `m_detach` -- COVERED (section 4.3)
11. `lifesaved_monster` -- COVERED (section 4.4)
12. `vamprises` -- COVERED (section 4.5)
13. `corpse_chance` -- COVERED (section 4.6)
14. `make_corpse` -- COVERED (section 4.6, species-specific drops)
15. `xkilled` -- COVERED (section 4.7, treasure drop)
16. `experience` -- COVERED (section 5.1)
17. `grow_up` -- COVERED (section 13)
18. `newcham` -- COVERED (section 10.1)
19. `select_newcham_form` -- COVERED (section 10.2)
20. `were_change` -- COVERED (section 10.3)
21. `set_malign` -- NOT COVERED (alignment pre-computation for kill rewards)
22. `minliquid` -- COVERED (section 15)
23. `golemhp` -- COVERED (section 2.2, table)
24. `setmangry` -- COVERED (section 8.3)
25. `wakeup` -- COVERED (section 8.4)

Coverage: 24/25 = 96%. `set_malign()` is not documented (the malign pre-computation formula that determines per-kill alignment impact).

### C2. Formula Spot-Checks

**Spot-check 1: `adj_lev` for Wizard of Yendor**
- Spec (section 2.1): `return min(ptr.mlevel + times_wizard_died, 49)`
- Source (makemon.c:2013-2020): `tmp = ptr->mlevel + svm.mvitals[PM_WIZARD_OF_YENDOR].died; if (tmp > 49) tmp = 49; return tmp;`
- MATCH

**Spot-check 2: `mcalcmove` MSLOW formula**
- Spec (section 3.2): `if mmove < 12: mmove = (2 * mmove + 1) / 3; else: mmove = 4 + mmove / 3`
- Source (mon.c:1119-1125): `if (mmove < 12) mmove = (2 * mmove + 1) / 3; else mmove = 4 + (mmove / 3);`
- MATCH

**Spot-check 3: `experience` revived/cloned diminishing returns**
- Spec (section 5.1): bracket sizes 20, 20, 40, 40, 60, 60, ... with `if i is odd: bracket_size += 20`
- Source (exper.c:157-162): `for (i = 0, tmp2 = 20; nk > tmp2 && tmp > 1; ++i) { tmp = (tmp + 1) / 2; nk -= tmp2; if (i & 1) tmp2 += 20; }`
- MATCH. The bracket progression is: i=0 (bracket=20), i=1 (bracket becomes 40), i=2 (bracket=40), i=3 (bracket becomes 60), etc. This gives 20, 40, 40, 60, 60, 80... Wait, the spec says "kills 21..40: XP/2, 41..80: XP/4". Let me verify: at i=0, tmp2=20 (covers kills 1-20), at i=1 tmp2 becomes 40 (covers kills 21-60), at i=2 tmp2=40 (covers kills 61-100)... Actually, let me re-check: `nk -= tmp2` then `if (i & 1) tmp2 += 20`. So i=0: subtract 20, tmp2 stays 20. i=1: subtract 20, tmp2 becomes 40. i=2: subtract 40, tmp2 stays 40. i=3: subtract 40, tmp2 becomes 60. The spec comment says kills 21..40 (size 20), 41..80 (size 40), 81..120 (size 40), 121..180 (size 60), 181..240 (size 60). But the code is: bracket 0=20, bracket 1=20, bracket 2=40, bracket 3=40, bracket 4=60, bracket 5=60. This matches the spec's table exactly: 1-20 full, 21-40 half, 41-80 quarter, 81-120 eighth, etc.

### C3. Missing Mechanics

1. **`set_malign()` formula**: The spec covers alignment adjustments in section 5.2 as a table, but the detailed `set_malign()` pre-computation formula (which determines `mtmp->malign` based on alignment, peacefulness, and always_peaceful/always_hostile status) is not documented. The function in mon.c:2313-2358 has nuanced case logic that differs from the simplified table.

2. **Monster eating mechanics**: Section 2.5 mentions `healmon` from eating objects but the full `mpickeat`/`dog_eat` pathways and how monsters decide to eat items are not covered (likely belongs in a separate pet/dog AI spec).

3. **`m_respond()` details**: Section 14 mentions `m_respond` only briefly in `dochug` flow. The Shrieker/Medusa gaze/Erinys aggravate behaviors are covered in monster-ai, not here. Acceptable cross-reference.

### C4. Test Vector Verification

**TV1 (Newt, mlevel=0, depth=1, plvl=1):**
- adj_lev: mlevel=0, tmp=0, diff=1-0=1 >= 0, tmp += 1/5 = 0, player_diff=1-0=1>0, tmp += 1/4 = 0, upper=min(0,49)=0, result=0. CORRECT.
- HP: d(1,4) = 1..4, basehp=1. If roll=1, mhpmax boosted to 2. "min 2" annotation CORRECT.

**TV8 (Demogorgon, mlevel=106):**
- adj_lev: mlevel > 49, returns 50. But spec says `m_lev = 2*(106-6)/4 = 50`.
- newmonhp: hp = 2*(106-6) = 200, m_lev = 200/4 = 50. CORRECT.
- Source (makemon.c:1031-1032): `mon->mhpmax = mon->mhp = 2 * (ptr->mlevel - 6); mon->m_lev = mon->mhp / 4;` MATCHES.

**TV speed 3 (Dog, base=16, normal):**
- Spec says "12 or 24 (random rounding)", "1 (2/3) or 2 (1/3)"
- mcalcmove: remainder = 16 % 12 = 4, mmove = 12, rn2(12) < 4 with prob 4/12 = 1/3 adds 12 making 24. So 2/3 chance of 12, 1/3 chance of 24. CORRECT.

## D. Issues Found

### D1. Minor: Speed table MSLOW formula branch label

Section 3.2 speed examples table shows MSLOW speed 12 = 8, which is correct. However, for speed 12, the formula used should be the `else` branch (`4 + mmove/3 = 4 + 4 = 8`), not the `if mmove < 12` branch (`(2*12+1)/3 = 8`). The result is coincidentally the same, but the spec's pseudocode correctly distinguishes the branches, so the table values are accurate.

### D2. Minor: Kops respawn probability description

Section 4.2 states "if rnd(5) <= 2: makemon(same_type)". The actual code uses `switch(rnd(5))` with case 1 (near stairs, with fallthrough to case 2 if no stairs) and case 2 (random position). This means:
- Case 1 with stairs: near stairs (probability 1/5)
- Case 1 without stairs: falls through to case 2 behavior (random)
- Case 2: random position (probability 1/5)
- Cases 3-5: no respawn (probability 3/5)
The total probability (2/5) matches the spec, but the location logic (stairs fallthrough) is slightly simplified.

### D3. Minor: `corpse_chance` simplification

Section 4.6 states "lizards (non-cloned)" always drop corpses via `bigmonst || lizard`. The actual code in mon.c:3225 checks `(bigmonst(mdat) || mdat == &mons[PM_LIZARD]) && !mon->mcloned`. The `!mcloned` condition applies to both `bigmonst` and `lizard`, not just lizard. The spec's parenthetical "(non-cloned)" is ambiguous about whether it applies to all conditions in that line.

### D4. Informational: healmon [疑似 bug] self-correction

The `[疑似 bug]` in section 2.5 correctly identifies a potential concern with `healmon` and then correctly self-corrects, concluding the behavior is actually correct. This is good analysis. The self-correction is accurate: with `overheal=0`, the first branch fires when `mhp + amt > mhpmax`, setting `mhp = mhpmax`; the else branch fires when `mhp + amt <= mhpmax`, so `mhp` never exceeds `mhpmax` after addition.

## E. Final Assessment

PASS. The spec is comprehensive, accurately reflects the C source code, covers all three v2 keywords thoroughly, and includes sufficient test vectors with boundary conditions. The three `[疑似 bug]` markers are appropriate: two correspond to real source FIXMEs and one is a correctly-resolved analysis. Minor issues are cosmetic or involve acceptable simplifications.
