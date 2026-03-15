# Review: hunger.md

## Summary
- Quality: GOOD
- Test vectors: 32 [PASS]
- Boundary conditions: 6 [PASS]
- C/Rust code leaks: NONE
- Bug markers: 1

## A. Format Compliance

- [A1] Test vectors: 32 individual test cases (TV1-TV32 across Hunger States, Eating Time, Choking, Intrinsic Gain, Tin, Cannibalism, Starvation Boundary). PASS (>=10)
- [A2] Boundary conditions: 6 explicitly labeled boundary tests (TV2: exactly 1000 not satiated, TV4: exactly 150 is HUNGRY, TV6: exactly 50 is WEAK, TV8: exactly 0 is FAINTING, TV10/TV31: starvation death threshold at various Con, TV20: canchoke=false). PASS (>=2)
- [A3] No C code: all code blocks use pseudocode notation. No `int `, `static `, `#include` as actual code. Uses `#define` once in section 16 for the nonrotting_corpse macro but it's documenting the macro, not C code. Borderline PASS.
- [A4] No Rust code: no `fn `, `let mut`, `impl `, `pub fn`. PASS
- [A5] Pseudocode present: sections 2 (state computation), 3 (depletion), 6 (choking), 7 (corpse effects), 10 (cannibalism). PASS
- [A6] Exact formulas: all formulas include explicit constants and ranges (nutrition tables, eating time `3 + (cwt >> 6)`, nmod formula, starvation threshold `-(100 + 10 * CON)`). PASS
- [A7] Bug markers: 1 instance of `[疑似 bug]` (section 7: rottenfood probability chain). PASS

## B. Content Coverage

v2 spec 15.5 hunger keywords:
- **营养消耗率**: Covered in section 3 (per-turn depletion, accessory hunger, slow digestion interaction, maximum depletion). PASS
- **尸体效果**: Covered extensively in section 7 (cprefx, cpostfx, intrinsic gain, poisonous, acidic, tainted, rotten). PASS
- **Conduct**: Covered in sections 9, 13 (vegan, vegetarian, foodless conduct tracking, what breaks each). PASS

All 3 keywords have dedicated coverage.

## C. Source Accuracy

### C1. Function Coverage

Non-trivial functions in eat.c:
1. `gethungry` -- COVERED (section 3)
2. `newuhs` -- COVERED (section 2)
3. `lesshungry` -- COVERED (section 6)
4. `morehungry` -- COVERED (section 1)
5. `choke` -- COVERED (section 6)
6. `cprefx` -- COVERED (section 7)
7. `cpostfx` -- COVERED (section 7)
8. `corpse_intrinsic` -- COVERED (section 7)
9. `should_givit` -- COVERED (section 7)
10. `temp_givit` -- COVERED (section 7)
11. `givit` -- COVERED (section 7 intrinsic table)
12. `eatcorpse` -- partially covered (via cprefx/cpostfx)
13. `start_eating` -- partially covered (via eating time section 5)
14. `bite` -- COVERED (section 5 nmod, section 6 canchoke)
15. `fprefx` -- COVERED (section 15)
16. `fpostfx` -- COVERED (section 15)
17. `rottenfood` -- COVERED (section 7, section 14)
18. `adj_victual_nutrition` -- COVERED (section 4)
19. `consume_tin` / `start_tin` / `opentin` -- COVERED (section 8)
20. `violated_vegetarian` -- COVERED (section 9)
21. `eataccessory` -- COVERED (section 11)
22. `bounded_increase` -- COVERED (section 11)
23. `doeat_nonfood` -- COVERED (section 11)
24. `edibility_prompts` -- COVERED (section 17)
25. `init_uhunger` -- COVERED (section 12)
26. `maybe_cannibal` -- COVERED (section 10)
27. `eye_of_newt_buzz` -- COVERED (section 7)
28. `recalc_wt` -- NOT COVERED (weight recalculation during eating)
29. `done_eating` -- NOT COVERED (eating completion cleanup)
30. `touchfood` -- NOT COVERED (food handling)

Coverage: 27/30 functions covered = 90%. Good coverage.

### C2. Formula Spot-Checks (3)

**Check 1: Starvation death threshold**
- Spec (section 2): `u.uhunger < -(100 + 10 * ACURR(A_CON))`. Con 18 => death at < -280, Con 3 => death at < -130.
- Source (eat.c line 3437): `u.uhunger < -(100 + 10 * (int) ACURR(A_CON))`
- MATCH

**Check 2: `should_givit` chance values**
- Spec (section 7): POISON_RES (killer bee/scorpion: 1/4 chance of chance=1, else chance=15), TELEPORT (chance=10), TELEPORT_CONTROL (chance=12), TELEPAT (chance=1), default (chance=15). Succeed if `monster_level > rn2(chance)`.
- Source (eat.c lines 961-988): Exact same values and formula.
- MATCH

**Check 3: Hunger state thresholds**
- Spec (section 2): SATIATED > 1000, NOT_HUNGRY > 150, HUNGRY > 50, WEAK > 0, FAINTING <= 0.
- Source (eat.c lines 3369-3372): `(h > 1000) ? SATIATED : (h > 150) ? NOT_HUNGRY : (h > 50) ? HUNGRY : (h > 0) ? WEAK : FAINTING`
- MATCH

### C3. Missing Mechanics

1. **Choking survival probability is INVERTED**: The spec (section 6) states "Survival chance: ~95% (19/20) normally." The actual code (eat.c line 258) has `if (Breathless || Hunger || (!Strangled && !rn2(20)))` as the SURVIVAL condition. For a normal hero (not Breathless, not Hunger, not Strangled), the survival condition is `!rn2(20)` which is true with probability 1/20 = **5%**. The death rate is 95%, not the survival rate. **CRITICAL ERROR** in the spec.

2. **`gethungry` accessory hunger detail**: Section 3 documents the ring protection hunger logic but does not mention the special case at accessorytime=12 (right ring) where the `(EProtection & ~W_RINGR) == 0L` check does NOT include the left-ring-is-also-+0-protection guard that exists at accessorytime=4. This asymmetry (eat.c lines 3260-3266 vs 3237-3254) is not documented. The left ring check is more complex to avoid double-counting.

3. **Eating non-food items with metallivore/gelatinous cube**: Section 11 covers ring eating effects but the general non-food eating framework (what materials each polymorph form can eat) is only briefly mentioned. The `doeat_nonfood()` function's full material checking logic is not documented.

4. **`recalc_wt()`**: Weight recalculation during multi-turn eating (adjusting owt as food is consumed) is not documented. Minor.

5. **Fainting formula nuance**: The spec correctly documents `uhunger_div_by_10 = sgn(u.uhunger) * ((abs(u.uhunger) + 5) / 10)` and the fainting check `u.uhs <= WEAK OR rn2(20 - uhunger_div_by_10) >= 19`. However, the spec says "the chance increases as hunger counter goes more negative" -- this needs clarification. When uhunger is -5, uhunger_div_by_10 = -1, so `rn2(21) >= 19` is about 2/21. When uhunger is -50, uhunger_div_by_10 = -6 (approximately), so `rn2(26) >= 19` is about 7/26. So yes, the chance does increase, but only modestly. The key trigger is that once `u.uhs <= WEAK` (which happens when uhunger drops to 50 or below), the faint always happens (bypasses the rn2 check). This is already in the spec's condition but could be clearer.

### C4. Test Vector Verification (3)

**TV #10: Starvation at Con 18**
- Spec: death threshold `-(100 + 180) = -280`, uhunger=-280 triggers death
- Source: `u.uhunger < -(100 + 10 * 18)` = `u.uhunger < -280`
- At uhunger = -280: `-280 < -280` is FALSE => survives
- At uhunger = -281: `-281 < -280` is TRUE => dies
- FAIL -- The spec says uhunger = -280 is the death threshold ("Death threshold: -(100+180) = -280"), which implies death at -280. But the code uses `<` (strict less than), so -280 exactly survives. The companion TV #11 says -279 "still alive" and implies -280 is death, which is also wrong. The correct boundary is: survives at -280, dies at -281.

**TV #19: Choking survival**
- Spec: "19/20 chance vomit+survive, 1/20 chance choke death"
- Source (eat.c line 258): `if (Breathless || Hunger || (!Strangled && !rn2(20)))`
- For normal hero: survival condition is `!rn2(20)` = 1/20 = 5%
- FAIL -- Probabilities are inverted. The survival chance is 1/20 (5%), not 19/20 (95%). Death chance is 19/20 (95%).

**TV #21: Floating eye telepathy**
- Spec: "level 3, chance=1, 3 > rn2(1) always true -> always gain telepathy"
- Source: `should_givit(TELEPAT, ptr)` with `chance = 1`, `ptr->mlevel > rn2(1)`. `rn2(1)` always returns 0, so `3 > 0` is always true.
- PASS

## D. Issues

### D1. Errors

1. **CRITICAL: Choking survival probability inverted** (section 6): The spec states "Survival chance: ~95% (19/20) normally. Breathless or Hunger property guarantees survival." The actual survival chance for a normal hero is **5% (1/20)**. The `!rn2(20)` in the survival condition means only a 1-in-20 chance of vomiting instead of dying. The `Breathless || Hunger` guarantees are correct.

2. **Starvation boundary test vector error** (TV #10): Claims uhunger = -280 is death with Con 18, but the code uses strict `<` comparison, so -280 exactly survives. Death occurs at -281 or below.

3. **TV #19 probability mismatch**: States "19/20 chance vomit+survive" but the actual probability is 1/20 survival. This flows from error #1.

### D2. Imprecisions

1. **Section 3 accessory hunger asymmetry**: The left ring (accessorytime=4) has a complex double-counting guard for +0 protection rings that the right ring (accessorytime=12) lacks. This asymmetry is not noted.

2. **Section 16 nonrotting_corpse macro**: Uses C `#define` syntax directly. While this is documenting the macro rather than writing C code, it borders on A3 violation.

3. **Section 7 rotten food probability chain**: The [疑似 bug] correctly identifies the non-uniform probabilities (1/4, 3/16, 3/16, 6/16) as differing from the apparent 25% each intent. Good documentation.

### D3. Missing Content

1. **`recalc_wt()` weight adjustment** during eating not documented.
2. **`doeat_nonfood()` material check** framework not fully documented (only ring/amulet eating covered).
3. **Accessory hunger left/right ring asymmetry** not documented.

## E. Verdict

CONDITIONAL PASS -- requires fixing the critical choking probability error. The survival rate is 5%, not 95%. This error affects TV #18, TV #19, and the prose in section 6. The starvation boundary TV #10 also needs correction (strict `<` means -280 survives, -281 dies).

After these fixes, the spec is comprehensive with excellent coverage of nutrition tables, eating time formulas, corpse effects, intrinsic gain mechanics, tin varieties, conduct tracking, and food detection. The accessory hunger system documentation is thorough and accurate.
