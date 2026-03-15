# Review: monster-items.md

**Reviewer**: Claude Opus 4.6 (automated)
**Date**: 2026-03-14
**Spec file**: `/specs/monster-items.md`
**C source files reviewed**: `src/muse.c` (3287 lines), `src/weapon.c`, `src/worn.c`

---

## Tier A: Format Compliance

| Check | Status | Notes |
|-------|--------|-------|
| A1: >= 10 test vectors | PASS | 33 test vectors across sections 14.1-14.6 |
| A2: >= 2 boundary conditions | PASS | 8 boundary conditions (B1-B5 in 14.6, B1-B3 in 14.1) |
| A3: No C code blocks | PASS | All code blocks use generic syntax or pseudocode; the `struct musable` block and `seen_resistance` enum use fenced blocks without `c` language tag |
| A4: No Rust code | PASS | No Rust code present |
| A5: Pseudocode present | PASS | Pseudocode in sections 3.2, 3.4, 4.4, 4.5, 4.6, 5.4, 5.5, 6.1 |
| A6: Exact formulas | PASS | All key formulas present with exact values |
| A7: [疑似 bug] markers | PASS | 5 bugs marked in section 13 plus 1 inline in section 3.3 |

---

## Tier B: Key Content Coverage (v2 spec section 15.4)

The v2 spec requires coverage of "怪物使用物品 AI" (monster item use AI).

| Topic | Covered | Section |
|-------|---------|---------|
| Three-phase decision system | Yes | Section 1 |
| Defensive item selection | Yes | Section 3 |
| Offensive item selection | Yes | Section 4 |
| Miscellaneous item use | Yes | Section 5 |
| Monster equipment/armor | Yes | Section 6 |
| Monster weapon selection | Yes | Section 7 |
| Item generation tables | Yes | Section 8 |
| Counter-petrification/sliming | Yes | Section 9 |
| Floor item pickup (`searches_for_item`) | Yes | Section 10 |
| Reflection system | Yes | Section 11 |
| Seen-resistance tracking | Yes | Section 12 |

**Assessment**: Comprehensive coverage of all key content areas.

---

## Tier C: Detailed Verification

### C1: Function Coverage

| C function | Spec section | Covered? |
|------------|-------------|----------|
| `precheck()` | 2.3 | Yes |
| `find_defensive()` | 3.1-3.3 | Yes |
| `use_defensive()` | 3.4-3.5 | Yes |
| `m_use_healing()` | 3.3 | Yes |
| `find_offensive()` | 4.1-4.3 | Yes |
| `use_offensive()` | 4.4-4.6 | Yes |
| `find_misc()` | 5.1-5.3 | Yes |
| `use_misc()` | 5.3-5.5 | Yes |
| `m_dowear()` | 6.1 | Yes |
| `select_hwep()` | 7.1 | Yes |
| `select_rwep()` | 7.2 | Yes |
| `mon_wield_item()` | 7.3 | Yes |
| `rnd_defensive_item()` | 8.1 | Yes |
| `rnd_offensive_item()` | 8.2 | Yes |
| `rnd_misc_item()` | 8.3 | Yes |
| `munstone()` | 9.1 | Yes |
| `munslime()` | 9.2 | Yes |
| `searches_for_item()` | 10 | Yes |
| `mon_reflects()` | 11 | Yes |
| `muse_newcham_mon()` | 5.4 | Yes |
| `mloot_container()` | 5.3 (item 9) | Yes |

**Missing functions**: None of significance. Minor omissions: `mzapwand()`, `mplayhorn()`, `mreadmsg()`, `mquaffmsg()` are message-only helper functions not critical for Babel reimplementation.

### C2: Formula Spot-Checks (3 verified)

**Formula 1: Healing threshold** (spec section 3.2, source line 543-546)
- Spec: `fraction = (u.ulevel < 10) ? 5 : (u.ulevel < 14) ? 4 : 3`
- Source: `fraction = u.ulevel < 10 ? 5 : u.ulevel < 14 ? 4 : 3;`
- Spec: heals when `mhp < mhpmax AND (mhp < 10 OR mhp * fraction < mhpmax)`
- Source: does NOT heal when `mtmp->mhp >= mtmp->mhpmax || (mtmp->mhp >= 10 && mtmp->mhp * fraction >= mtmp->mhpmax)`
- **PASS**: Formulas match exactly (spec correctly inverts the condition).

**Formula 2: Flee timer** (spec section 3.4, source line 812)
- Spec: `fleetim = !mtmp->mflee ? (33 - (30 * mtmp->mhp / mtmp->mhpmax)) : 0`
- Source: `fleetim = !mtmp->mflee ? (33 - (30 * mtmp->mhp / mtmp->mhpmax)) : 0;`
- **PASS**: Exact match.

**Formula 3: Offensive item generation switch** (spec section 8.2, source lines 2023-2025)
- Spec: `rn2(9 - (difficulty < 4) + 4 * (difficulty > 6))`
- Source: `rn2(9 - (difficulty < 4) + 4 * (difficulty > 6))`
- Spec: WAN_DEATH chance `difficulty > 7 && !rn2(35)`
- Source: `if (difficulty > 7 && !rn2(35)) return WAN_DEATH;`
- **PASS**: Exact match.

### C3: Missing Mechanics

1. **find_defensive wand of digging -- additional exclusions not fully listed**: The spec says wand of digging "breaks the scan immediately once found" which is correct. However, the spec at section 3.3 point 8 does not fully list the conditions from source line 664-673. Missing conditions:
   - `!Sokoban` (digging completely blocked in Sokoban during defensive use, not just generation)
   - `!(levl[x][y].wall_info & W_NONDIGGABLE)` (non-diggable floor)
   - `!(Is_botlevel(&u.uz) || In_endgame(&u.uz))` (bottom level or endgame)
   - `!(is_ice(x, y) || is_pool(x, y) || is_lava(x, y))` (special terrain)
   - `!(is_Vlad(mtmp) && In_V_tower(&u.uz))` (Vlad in tower)
   - `!t` (no existing trap at location, after pit/web/bear trap filtering)
   - `!stuck` (not stuck)

   These are significant for Babel implementation as they are hard reject conditions.

2. **Nurse check uses `dmgtype(mtmp->data, AD_HEAL)`**: Spec section 4.1 says "nurse monsters won't use items if hero is naked." The actual code at line 1434 checks `dmgtype(mtmp->data, AD_HEAL)` not the monster being specifically a nurse. This is more general -- any monster with AD_HEAL damage type would trigger this check. This matters for correctness if polymorph introduces other AD_HEAL monsters.

3. **Scroll of fire (SCR_FIRE) is disabled**: Spec section 4.3 item 17 lists scroll of earth but does not mention scroll of fire. In the source (lines 1574-1589), `MUSE_SCR_FIRE` is inside `#if 0` ... `#endif`, meaning it is disabled/dead code. The spec correctly omits it from the offensive scan. However, `searches_for_item()` (line 2728-2730) still includes `SCR_FIRE` as a pickupable item. Spec section 10 correctly includes it in the floor pickup table. This is a minor inconsistency in the game itself worth noting.

4. **Container looting details**: Spec section 5.3 item 9 says "monster takes 1-4 items out." Source (lines 2259-2272) uses `rn2(10)` with a distribution:
   - 0-3 (40%): 1 item
   - 4-6 (30%): 2 items
   - 7-8 (20%): 3 items
   - 9 (10%): 4 items

   Additionally, each extraction has a throttle: `!rn2(nitems + 1)` chance to stop early. The spec simplifies this to "1-4 items" which loses the weighted distribution.

5. **Horn ray width**: Spec section 4.4 only covers beam wand execution. Horns are handled differently (source lines 1847-1860): they use `rn1(6, 6)` for ray width (range 6-11) instead of a fixed width. This is not documented in the spec.

### C4: Test Vector Verification (3 checked)

**TV 14.1 #10** (Healing threshold boundary):
- Input: ulevel=15, hp=10, maxhp=31, fraction=3
- Spec says: `hp*frac = 30 < 31 = TRUE` => DOES seek healing
- Source check: `mtmp->mhp >= 10` (TRUE) AND `mtmp->mhp * 3 >= 31` => `30 >= 31` is FALSE
- So condition `(mhp >= 10 && mhp*fraction >= mhpmax)` is FALSE, the healing check is NOT short-circuited.
- Therefore monster DOES seek healing.
- **PASS**: TV is correct.

**TV 14.3 #5** (Scroll of earth):
- Input: hostile, adjacent (dist2=1 which is <=2), has hard helmet
- Spec expects: MUSE_SCR_EARTH
- Source check (lines 1553-1563): `dist2 <= 2` (1<=2 TRUE), `hard_helmet` (TRUE), `mcansee && haseyes` (assumed TRUE), `!Is_rogue_level` (assumed TRUE)
- **PASS**: TV is correct.

**TV 14.3 #6** (Scroll of earth distance):
- Input: hostile, dist=3
- Spec expects: Skip (dist2 > 2)
- Spec says "dist2 > 2" but actually dist2 for distance 3 is 9, and the check is `dist2(mtmp->mx, mtmp->my, mtmp->mux, mtmp->muy) <= 2`
- If "dist=3" means the monster is 3 squares away in any direction, the minimum dist2 would be 9, which is > 2.
- **PASS**: TV is correct, though "dist=3" should more precisely say "dist2=9" or "any position with dist2 > 2."

---

## Issues Summary

### Errors (must fix)

1. **Section 3.3 item 8 (wand of digging)**: Missing several hard exclusion conditions from source lines 664-673. Must add: `!Sokoban`, `!(W_NONDIGGABLE)`, `!(Is_botlevel || In_endgame)`, `!(ice/pool/lava)`, `!(Vlad in V_tower)`, `!stuck`, `!existing_trap`. Without these, a Babel implementation would allow digging in contexts the original forbids.

### Inaccuracies (should fix)

2. **Section 4.1**: "nurse monsters" should be "monsters with AD_HEAL damage type (`dmgtype(mdat, AD_HEAL)`)" for precision.

3. **Section 4.4 (Horn execution)**: Horn ray width is `rn1(6, 6)` (range 6-11), not fixed at 6 like beam wands. This applies to both fire horn and frost horn when used offensively (source lines 1853-1856).

4. **Section 5.3 item 9 (Container looting)**: "monster takes 1-4 items out" should document the weighted distribution (40/30/20/10% for 1/2/3/4 items) and the per-item throttle `!rn2(nitems + 1)`.

5. **Section 8.1 (Defensive item generation)**: Wand of digging row says "Skip if floater/shk/guard/priest; retry in Sokoban 3/4." Should note the 3/4 means `rn2(4)` is nonzero (3 out of 4 cases cause retry, so 1/4 chance of keeping the wand).

### Cosmetic/Minor

6. **Section 12 (Seen Resistance)**: Block uses `c` enum syntax with `enum m_seen_resistance { ... };`. While not tagged as C, it uses C enum syntax. Consider converting to a table or pseudocode for A3 compliance purity.

7. **Section 13 bug #5 (Camera returns 1)**: Verified correct. Source line 1934 shows `return 1;` for MUSE_CAMERA.

---

## Verdict

**PASS with minor revisions required.** The spec is thorough, well-organized, and substantially accurate. The main issue is the incomplete list of exclusion conditions for wand of digging in `find_defensive()` (Issue #1), which would cause incorrect behavior in a Babel reimplementation. The remaining issues are minor precision improvements. Test vectors are comprehensive and correctly calculated.
