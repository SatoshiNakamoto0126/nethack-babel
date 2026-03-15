# Review: monster-generation.md

## Summary
- Quality: GOOD
- Test vectors: 17 (T1-T17) [PASS]
- Boundary conditions: 3 (T5, T6, T13/Nazgul) [PASS]
- C/Rust code leaks: NONE
- Bug markers: 3

## A. Format Compliance

- [A1] Test vectors: 17 individual test cases (T1-T17) covering level_difficulty, difficulty windows, adj_lev, group sizes, propagate, align_shift, spawn rates, and courtmon. PASS (>=10)
- [A2] Boundary conditions: T5 (zlevel=1, ulevel=1 minimal window), T6 (zlevel=0 edge case), T13 (Nazgul at birth limit boundary 8/9). PASS (>=2)
- [A3] No C code: all code blocks use pseudocode notation (`fn`, `if`/`elif`/`else`, `match`, `for each`). No `int `, `void `, `static `, `struct `, `#include`. PASS
- [A4] No Rust code: uses `-> i16`, `-> int`, `-> Option<permonst>`, `-> bool` as pseudocode annotations, not Rust syntax. No `let mut`, `impl`, `pub fn`. PASS
- [A5] Pseudocode present: sections 1, 2, 3.1-3.4, 5, 6, 7, 8.2, 9, 10.1-10.9, 11, 13, 15 all contain pseudocode. PASS
- [A6] Exact formulas: includes constants (ALIGNWEIGHT=4, MAXMONNO=120, G_FREQ mask 0x0007, spawn rates 25/50/70), frequency/alignment/temperature shift formulas, group size divisors. PASS
- [A7] Bug markers: 3 instances of `[疑似 bug]` (align_shift negative values, mkclass vs rndmonst difficulty caps, m_initgrp peace_minded double check). PASS

## B. Content Coverage

v2 spec 15.4 monster-generation keywords:
- **难度-频率**: Covered in sections 1-3 (level_difficulty calculation, difficulty window with monmin/monmax, weighted reservoir sampling with G_FREQ + align_shift + temperature_shift). Also section 11 (mkclass halved difficulty). PASS
- **群体生成**: Covered in section 5 (G_SGROUP/G_LGROUP flags, group sizes via m_initgrp, hero level adjustment divisors, effective group size table). PASS

Both keywords have dedicated coverage.

## C. Source Accuracy

### C1. Function Coverage

Non-trivial public/static functions in makemon.c relevant to monster-generation:

1. `level_difficulty` -- COVERED (section 1)
2. `rndmonst` / `rndmonst_adj` -- COVERED (section 3.2)
3. `uncommon` -- COVERED (section 3.2 exclusion filter 5)
4. `align_shift` -- COVERED (section 3.3)
5. `temperature_shift` -- COVERED (section 3.4)
6. `mkclass` / `mkclass_aligned` -- COVERED (section 11)
7. `m_initgrp` -- COVERED (section 5)
8. `m_initsgrp` / `m_initlgrp` -- COVERED (section 5, macros with n=3, n=10)
9. `adj_lev` -- COVERED (section 6)
10. `newmonhp` -- COVERED (section 7)
11. `golemhp` -- COVERED (section 7 table)
12. `propagate` -- COVERED (section 8.2)
13. `mbirth_limit` -- COVERED (section 8 birth limits)
14. `makemon` -- COVERED (sections 16-18, group triggering, placement)
15. `makemon_rnd_goodpos` -- COVERED (section 9, 17)
16. `peace_minded` -- COVERED (section 13)
17. `wrong_elem_type` -- COVERED (section 15)
18. `set_mimic_sym` -- NOT COVERED (mimic appearance selection)
19. `m_initweap` -- COVERED (section 12.1)
20. `m_initinv` -- COVERED (section 12.2)
21. `qt_montype` -- COVERED (section 3.1)
22. `courtmon` -- COVERED (section 10.1)
23. `squadmon` -- COVERED (section 10.2)
24. `morguemon` -- COVERED (section 10.3)
25. `mk_gen_ok` -- partially covered (used internally by mkclass)

Coverage: 23/25 = 92%. `set_mimic_sym()` is not documented here (complex mimic appearance logic based on room type, terrain, and objects). This is arguably not "generation" per se, but is part of post-creation setup (section 18 mentions mimics briefly).

### C2. Formula Spot-Checks

**Spot-check 1: `uncommon()` Gehennom filter**
- Spec (section 3.2 filter 5): "In Gehennom: positive alignment (maligntyp > 0) -> uncommon"
- Source (makemon.c:1595-1596): `if (Inhell) return (boolean) (mons[mndx].maligntyp > A_NEUTRAL);`
- `A_NEUTRAL` is 0 (defined in align.h). So `maligntyp > 0` = `maligntyp > A_NEUTRAL`. MATCH.

**Spot-check 2: `align_shift` AM_CHAOTIC formula**
- Spec (section 3.3): `AM_CHAOTIC => (-(ptr.maligntyp - 20)) / (2 * ALIGNWEIGHT)`
- Source (makemon.c:1628-1629): `case AM_CHAOTIC: alshift = (-(ptr->maligntyp - 20)) / (2 * ALIGNWEIGHT);`
- MATCH exactly. For maligntyp=-20: `(-(-20-20))/8 = 40/8 = 5`. For maligntyp=+20: `(-(20-20))/8 = 0`. Correct.

**Spot-check 3: `mkclass` difficulty cap and bias**
- Spec (section 11): `maxmlev = level_difficulty() / 2` (note: halved). Bias: `nums[mndx] = k + 1 - (adj_lev(mon) > u.ulevel * 2 ? 1 : 0)`.
- Source (makemon.c:1883): `maxmlev = level_difficulty() >> 1;` (same as /2 for non-negative). Source (makemon.c:1952): `nums[MONSi(last)] = k + 1 - (adj_lev(&mons[MONSi(last)]) > (u.ulevel * 2));`
- MATCH. The spec correctly uses `/2` and the bias formula is exact.

### C3. Missing Mechanics

1. **`set_mimic_sym()` detail**: The mimic appearance selection based on room type (zoo=gold, delphi=statue/fountain, temple=altar, shop=shop items, etc.) is a substantial function (makemon.c:2385-2543) that is only briefly mentioned in section 18 ("Mimics: set mimic appearance"). This is a non-trivial piece of monster generation behavior.

2. **Monster naming/gender at creation**: The `makemon()` function assigns gender based on `M2_MALE`/`M2_FEMALE`/`M2_NEUTER` flags and random chance. This is partially covered in monster-core (section 10.5) but not in this generation spec.

3. **`unmakemon()`**: The function to undo a monster creation (decrement birth counter, clear extinction for uniques) is not documented. This is relevant for callers that reject `makemon()`'s result.

4. **Rogue level uppercase-only restriction**: Section 3.2 filter 3 mentions "skip if symbol is not uppercase" but doesn't explain the gameplay rationale (Rogue level uses traditional ASCII roguelike display where monsters are uppercase letters).

### C4. Test Vector Verification

**T7 (adj_lev, mlevel=5, depth=15, ulevel=10):**
- Spec calculation: diff=15-5=10, tmp=5+10/5=7, player_diff=10-5=5, tmp=7+5/4=8, upper=min(3*5/2, 49)=min(7,49)=7, result=min(8,7)=7.
- Source verification: tmp=5, tmp2=15-5=10>=0 so tmp+=10/5=2, tmp=7. tmp2=10-5=5>0 so tmp+=5/4=1, tmp=8. tmp2=3*5/2=7, 7<49 so tmp2=7. return min(8,7)=7. CORRECT.

**T12 (propagate, killer bee at born=119, lim=120):**
- Spec: result=true (119<120), born becomes 120, 120>=120 so G_EXTINCT set.
- Source: `result = (119 < 120) && !gone` = true. `born < 255 && tally`: born++ to 120. `born >= lim && !G_NOGEN && !G_EXTINCT`: set G_EXTINCT. CORRECT.

**T14 (align_shift AM_LAWFUL, maligntyp=0):**
- Spec: (0+20)/(2*4) = 20/8 = 2 (integer division). CORRECT.

## D. Issues Found

### D1. Error: Spawn rate repeat countdown value

Section 9 correctly states the initial `u.udg_cnt` implicitly (via the spawn rate table), but the Wizard harassment details are in monster-ai. However, the spawn rate for `udemigod` (1/25) matches the source (allmain.c:232). No error in this spec.

### D2. Minor: `m_initgrp` group size table row for "Small, Hero lvl 3..4"

The spec says small group (n=3) at hero lvl 3..4 gives `cnt /= 2`, so `rnd(3)/2`. rnd(3) gives 1, 2, or 3. Dividing by 2 (integer): 0, 1, 1. Then `if (!cnt) cnt++` gives 1, 1, 1. So always 1. The table says "1 (always)". CORRECT.

### D3. Minor: `uncommon()` description could note A_NEUTRAL = 0

The spec says "positive alignment (maligntyp > 0)" for the Gehennom filter. The source actually tests `maligntyp > A_NEUTRAL`. Since `A_NEUTRAL` is defined as 0 in NetHack, these are equivalent, but the spec could note this for clarity.

### D4. Informational: [疑似 bug] quality

All three bug markers are well-reasoned:
1. `align_shift` negative value: correctly identifies that maligntyp can theoretically be < -20 due to schar range, but notes this doesn't happen in practice. Valid theoretical concern.
2. `mkclass` vs `rndmonst` difficulty: correctly identifies the `level_difficulty()/2` vs `(level_difficulty()+ulevel)/2` inconsistency. Notes it may be intentional.
3. `m_initgrp` double `peace_minded()` check: correctly identifies the race condition between the pre-filter and makemon's own call. Source comment confirms awareness.

## E. Final Assessment

PASS. The spec thoroughly covers the monster generation pipeline from difficulty calculation through species selection, group spawning, inventory assignment, and placement. All formulas verified against source code are accurate. The three `[疑似 bug]` markers identify real source code design questions. Minor gaps (set_mimic_sym detail, unmakemon) do not affect the core generation mechanics.
