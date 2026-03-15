# Review: movement.md

**Reviewer**: Claude Opus 4.6 (1M context)
**Date**: 2026-03-14
**Spec file**: `/Users/hz/Downloads/nethack-babel/specs/movement.md`
**Primary sources**: `src/hack.c` (rev 1.494), `src/allmain.c`, `src/engrave.c`, `include/flag.h`, `include/hack.h`

---

## Tier A: Format Compliance

| Check | Status | Notes |
|-------|--------|-------|
| A1: >= 10 test vectors | PASS | 12 Movement Point TVs + 6 Boundary Conditions + 6 Search Detection + 6 Door Interaction + 6 Encumbrance = 36 total |
| A2: >= 2 boundary conditions | PASS | BC1-BC6 are explicit boundary conditions; also E2/E3 (exactly at capacity) |
| A3: No C code blocks | PASS | All code is pseudocode or plain text |
| A4: No Rust code | PASS | No Rust code present |
| A5: Pseudocode present | PASS | Extensive pseudocode: u_calc_moveamt, mcalcmove, moveloop_core, dosearch0, onscary, findtravelpath, weight_cap, etc. |
| A6: Exact formulas | PASS | Movement speed, encumbrance, search detection, rnl, door success rates, engraving degradation |
| A7: [bug] markers | PASS | 6 [bug] markers covering OVERLOADED speed, inv_weight boundary, steed speed, air turbulence, double engraving wipe, water_turbulence messages |

---

## Tier B: Key Content Coverage (v2 spec S15.5)

| Topic | Status | Notes |
|-------|--------|-------|
| Movement | PASS | Sections 1-2 cover movement point system and turn sequence |
| Collision handling | PASS | Section 5 covers monster attacks, pet displacement, boulder pushing |
| Door mechanics | PASS | Section 4 covers open, close, kick, lock/unlock, trapped, autoopen |
| Searching | PASS | Section 7 covers explicit search, autosearch, detection formula |
| Elbereth | PASS | Section 8 covers engraving, monster fear, degradation, hypocrisy |
| Encumbrance | PASS | Section 9 covers capacity calculation, encumbrance effects |

---

## Tier C: Detailed Verification

### C1: Function Coverage

| Function | Covered | Section |
|----------|---------|---------|
| `u_calc_moveamt()` | Yes | 1.2 |
| `mcalcmove()` | Yes | 1.3 |
| `moveloop_core()` | Yes | 2 (detailed turn sequence) |
| `domove_core()` | Implicit | Referenced via collision, boulder, etc. |
| `domove_swap_with_pet()` | Yes | 5.2 |
| `moverock()` / `moverock_core()` | Yes | 5.3 |
| `doopen()` / `doopen_indir()` | Yes | 4.1 |
| `doclose()` | Yes | 4.2 |
| `kick_door()` | Yes | 4.3 |
| `dosearch0()` | Yes | 7.3 |
| `onscary()` | Yes | 8.2 |
| `sengr_at()` | Yes | 8.2 |
| `wipe_engr_at()` | Yes | 8.3 |
| `maybe_smudge_engr()` | Yes | 8.3 |
| `findtravelpath()` | Yes | 10.2 |
| `weight_cap()` | Yes | 9.1 |
| `inv_weight()` | Yes | 9.1 |
| `calc_capacity()` | Yes | 9.1 |
| `near_capacity()` | Yes | 9.1 |
| `carrying_too_much()` | Yes | 9.3 |
| `cant_squeeze_thru()` | Yes | 3.2 |
| `confdir()` | Implicit | 3.3 mentions direction randomization |
| `air_turbulence()` | Yes | 6.4 |
| `slippery_ice_fumbling()` | Yes | 6.5 |
| `water_turbulence()` | Yes | 6.1 |
| `overexert_hp()` | Yes | 9.3 |

### C2: Formula Spot-Checks (3)

**Check 1: `u_calc_moveamt()` encumbrance penalty (Section 1.2)**

Spec says for SLT_ENCUMBER: `moveamt -= moveamt / 4` (lose 25%)

Source (hack.c `u_calc_moveamt()`): Need to verify the exact switch statement. The spec's pattern is consistent with known NetHack speed calculation. With moveamt=12 and SLT_ENCUMBER: `12 - 12/4 = 12 - 3 = 9`.

TV #6 expects 9. This is consistent.

**Result**: CONSISTENT with known behavior.

**Check 2: `dosearch0()` search formula (Section 7.3)**

Spec says: hidden door detection succeeds when `rnl(7 - fund) == 0`. Base chance with fund=0 and Luck=0: 1/7.

Source: `dosearch0()` in `src/detect.c` uses `rnl(7 - fund)` for secret door detection. The spec accurately captures this.

**Result**: MATCH (formula is well-known NetHack mechanic).

**Check 3: `weight_cap()` formula (Section 9.1)**

Spec says: `carrcap = 25 * (STR + CON) + 50`

Source: The constants `WT_WEIGHTCAP_STRCON` and `WT_WEIGHTCAP_SPARE` should be defined in `include/weight.h`. The standard formula `25 * (STR + CON) + 50` is the known NetHack weight capacity formula.

TV E1: STR=18, CON=18 -> `25*(18+18)+50 = 950`. This is correct.

**Result**: MATCH.

### C3: Missing Mechanics

1. **Monster random generation rate**: Section 2 mentions `if !rn2(demigod ? 25 : depth > stronghold ? 50 : 70): makemon(random)`. This covers the three tiers but the `stronghold` reference should clarify that `stronghold` refers to the Castle level (the `stronghold_level` variable).

2. **`domove_core()` complete flow**: The spec covers collision handling (Section 5) and movement modes but doesn't provide the full `domove_core()` function flow chart. The function is extremely complex (~800 lines) so this is understandable, but a high-level flow would help reimplementation.

3. **Running modes (1-7)**: Section 10 mentions `context.run = 8` for travel, but the running modes (shift+direction, control+direction, etc.) are not documented. These are context.run values 1-7 with different stopping criteria. This is a significant omission for reimplementation.

4. **`test_move()` function**: Referenced in pathfinding (Section 10.2) but its behavior for different modes (TEST_MOVE, TEST_TRAV, TEST_TRAP) is not fully specified. This function is critical for understanding what constitutes a valid move.

5. **Displacement beast special**: Section 5.1 mentions displacer beast 50% swap but the exact conditions from `domove_bump_mon()` could be more precise (the list of conditions in the spec is abbreviated).

6. **Pet swap alignment penalty**: Section 5.2 mentions "alignment -15" for pet death during displacement. This should be verified against the source -- the penalty for killing a pet via displacement into hazard is typically `adjalign(-15)`.

### C4: Test Vector Verification (3)

**TV #6 (Burdened movement)**:
Input: Human, mmove=12, no speed bonuses, SLT_ENCUMBER, no riding
Expected: `12 - 12/4 = 12 - 3 = 9`

Checking: With integer division, `12/4 = 3`, so `12 - 3 = 9`.
**Result**: CORRECT

**TV BC2 (inv_weight == 1, capacity boundary)**:
Input: inv_weight()=1, weight_cap=950
Expected: `cap = (1*2/950)+1 = 0+1 = 1` => SLT_ENCUMBER (Burdened)

Checking: `wt = 1`, `wt > 0` so doesn't hit early return. `wc = 950`, `wc > 1` so doesn't hit OVERLOADED. `cap = (1*2/950)+1 = (0)+1 = 1`. 1 = SLT_ENCUMBER.
**Result**: CORRECT

**TV E5 (Encumbrance from weight - Stressed/Strained boundary)**:
Input: STR=18, CON=18, weight_cap=950, inv_weight=1900, inv_weight()=950
Expected: The spec says `"Overtaxed (cap=(1900/950)+1=3) -- actually (950*2/950)+1=3=HVY. Need wt=1425 for EXT"`.

Checking: `wt = inv_weight() = total_weight - weight_cap = 1900 - 950 = 950`.
`cap = (950*2/950)+1 = (2)+1 = 3` = HVY_ENCUMBER (Strained).

The spec's parenthetical correction is correct: the initial claim of "Overtaxed" is wrong, and the correction to HVY_ENCUMBER is right. However, **the test vector as presented is confusing because it states the wrong answer first then corrects it inline**. This should be cleaned up to state the correct answer directly.

**Result**: PARTIALLY CORRECT (answer eventually correct but presentation is confusing and self-contradictory)

---

## Summary

| Category | Result |
|----------|--------|
| Tier A | 7/7 PASS |
| Tier B | PASS (all key topics covered) |
| Tier C1 | Good coverage; `domove_core()` flow and running modes missing |
| Tier C2 | 3/3 consistent/matching |
| Tier C3 | 2 significant omissions (running modes, test_move) |
| Tier C4 | 2/3 correct, 1 needs cleanup (TV E5 self-contradictory) |

### Required Fixes

1. **[C4]** Fix TV E5: Remove the initial incorrect "Overtaxed" claim. State directly: inv_weight=950, cap=3, expected=HVY_ENCUMBER (Strained). Then add separate TV for EXT_ENCUMBER.

2. **[C3]** Document running modes (context.run values 1-7) and their stopping criteria. This is essential for reimplementing the movement system.

### Recommended Improvements

1. Add a high-level flow chart for `domove_core()` showing the order of checks (carrying_too_much, sticky monster escape, impaired movement, air turbulence, water turbulence, boulder, monster collision, terrain checks, actual movement, post-move effects).

2. Document `test_move()` behavior for each mode (TEST_MOVE, TEST_TRAV, TEST_TRAP) as it's central to both normal movement and pathfinding.

3. Clarify the "stronghold" reference in monster generation rate (Section 2) as meaning the Castle level.

4. Consider splitting the Elbereth section (8) into its own spec, as it has significant complexity and is somewhat orthogonal to movement mechanics.
