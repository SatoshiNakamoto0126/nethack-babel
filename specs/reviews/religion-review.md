# Review: religion.md

**Reviewer**: Claude Opus 4.6 (1M context)
**Date**: 2026-03-14
**Spec file**: `/Users/hz/Downloads/nethack-babel/specs/religion.md`
**Primary source**: `src/pray.c` (rev 1.244), `src/attrib.c`, `include/rm.h`

---

## Tier A: Format Compliance

| Check | Status | Notes |
|-------|--------|-------|
| A1: >= 10 test vectors | PASS | 24 test vectors across sections 11.1-11.5 |
| A2: >= 2 boundary conditions | PASS | TVs 15-20 are explicit boundary conditions (ALIGNLIM, LUCKMAX, critically_low_hp edge cases) |
| A3: No C code blocks | FAIL | Section 10.1 contains a C code block (`if (is_demon(...) && (gp.p_aligntyp == A_LAWFUL || ...))`) with `pray.c:2131-2132` reference. Section 10.2 also contains C code. Must convert to pseudocode. |
| A4: No Rust code | PASS | No Rust code present |
| A5: Pseudocode present | PASS | Extensive pseudocode throughout (adjalign, can_pray, pleased, sacrifice_value, etc.) |
| A6: Exact formulas | PASS | ALIGNLIM, rnz, sacrifice_value, bestow_artifact probability formulas all present |
| A7: [bug] markers | PASS | Two [bug] markers: 10.1 demon prayer condition, 10.2 atheist conduct timing |

**A3 fix required**: Sections 10.1 and 10.2 contain C code blocks. Replace with pseudocode equivalents.

---

## Tier B: Key Content Coverage (v2 spec S15.5)

| Topic | Status | Notes |
|-------|--------|-------|
| Prayer cooldown (ublesscnt) | PASS | Section 2.2 comprehensively covers cooldown conditions, reset values, anti-automation mechanism |
| Piety (alignment record) | PASS | Section 1.2 covers ALIGNLIM formula, adjalign mechanics; Section 1.3 covers thresholds |
| Divine intervention | PASS | Section 2.8 (pleased), 2.9 (angrygods), Section 4 (crowning) |
| Sacrifice mechanics | PASS | Sections 3.1-3.8 cover full sacrifice flow |
| Luck system | PASS | Section 7 covers data structures, timeout, common events |

---

## Tier C: Detailed Verification

### C1: Function Coverage

| Function | Covered | Section |
|----------|---------|---------|
| `can_pray()` | Yes | 2.4 |
| `dopray()` | Yes | 2.5 |
| `prayer_done()` | Yes | 2.6 |
| `in_trouble()` | Yes | 2.7 |
| `pleased()` | Yes | 2.8 |
| `angrygods()` | Yes | 2.9 |
| `god_zaps_you()` | Yes | 2.9 (sub-section) |
| `gcrownu()` | Yes | 4 |
| `dosacrifice()` | Yes | 3.4 |
| `sacrifice_value()` | Yes | 3.2 |
| `eval_offering()` | Yes | 3.3 |
| `offer_corpse()` | Yes | 3.5 |
| `offer_real_amulet()` | Yes | 3.7 |
| `offer_fake_amulet()` | Yes | 3.8 |
| `bestow_artifact()` | Yes | 3.6 |
| `water_prayer()` | Yes | 5.2 |
| `desecrate_altar()` | Yes | 5.4 |
| `altar_wrath()` | Yes | 5.5 |
| `doturn()` | Yes | 9 |
| `critically_low_hp()` | Yes | 2.7 (sub-section) |
| `gods_upset()` | Partial | Referenced but inner logic (ugangr++ for own god, ugangr-- for other god) not fully explained |
| `give_spell()` | No | Missing. `pleased()` case 6 calls `give_spell()`, described briefly but no detail on spell selection algorithm |

**Missing function**: `give_spell()` is only mentioned as "spell book grant" in the pat_on_head table. The actual spell selection logic (preference for unknown/forgotten spells, skill-restricted avoidance, 25% chance of learning directly) is absent.

### C2: Formula Spot-Checks (3)

**Check 1: `angrygods()` maxanger formula (Section 2.9)**

Spec says:
```
if resp_god != ualign.type:
    maxanger = ualign.record / 2 + (Luck>0? -Luck/3 : -Luck)
else:
    maxanger = 3 * ugangr + ((Luck>0 || record>=STRIDENT)? -Luck/3 : -Luck)
```

Source (pray.c:714-719):
```c
if (resp_god != u.ualign.type)
    maxanger = u.ualign.record / 2 + (Luck > 0 ? -Luck / 3 : -Luck);
else
    maxanger = 3 * u.ugangr + ((Luck > 0 || u.ualign.record >= STRIDENT)
                               ? -Luck / 3
                               : -Luck);
```

**Result**: MATCH. The spec accurately reproduces the formula.

**Check 2: `bestow_artifact()` probability (Section 3.6)**

Spec says: `do_bestow = !rn2(6 + 2 * ugifts * nartifacts)`

Source (pray.c:1792): `do_bestow = !rn2(6 + (2 * u.ugifts * nartifacts));`

**Result**: MATCH.

**Check 3: `pleased()` action calculation (Section 2.8)**

Spec says: `action = rn1(prayer_luck + (on_altar? 3 + on_shrine : 2), 1)`

Source (pray.c:1126): `action = rn1(prayer_luck + (on_altar() ? 3 + on_shrine() : 2), 1);`

**Result**: MATCH.

### C3: Missing Mechanics

1. **`gods_upset()` asymmetric behavior**: The spec (Section 1.4) says `gods_upset()` does `ugangr++ (own god)`, but the actual implementation (pray.c:1435-1443) is: if `g_align == ualign.type`, then `ugangr++`; else if `ugangr > 0`, then `ugangr--`. The spec does not mention the `ugangr--` decrement when the upset god is *not* your own god. This is a significant behavioral detail for reimplementation.

2. **`prayer_done()` Inhell pathway**: The spec (Section 2.6) says `p_type == -2` in non-Gehennom prints "Nothing else happens" and returns. But the code also flows into the Inhell check if `Inhell` is true (i.e., `-2` on a Moloch altar IN Gehennom proceeds to the Inhell case). The spec captures this correctly but the flow could be clearer.

3. **`sacrifice_your_race()` missing `change_luck` for chaotic**: Section 3.5 correctly notes that non-chaotic sacrifice loses luck (-5), but the chaotic case on chaotic/none altars has `change_luck(altaralign == A_NONE ? -2 : 2)` which IS captured in Section 7.4. Cross-reference is implicit.

4. **`offer_different_alignment_altar()` rejection branch**: When the player has already converted once (`ualignbase[A_CURRENT] != ualignbase[A_ORIGINAL]`), the spec only says "non-conversion case" but the source shows: `ugangr += 3, adjalign(-5), change_luck(-5), adjattrib(WIS, -2)`, then `angrygods` if not in hell. This rejection penalty is missing from the spec.

### C4: Test Vector Verification (3)

**TV #7 (acid blob sacrifice value)**:
Input: corpsenm=PM_ACID_BLOB, difficulty=1, moves_since_death=999, oeaten=false
Source check (pray.c:1843): `otmp->corpsenm == PM_ACID_BLOB` bypasses the age check.
`value = mons[PM_ACID_BLOB].difficulty + 1 = 1 + 1 = 2`
**Result**: CORRECT (expected: 2)

**TV #13 (artifact bestow probability)**:
Input: ulevel=10, uluck=3, ugifts=0, nartifacts=2
`do_bestow = ulevel > 2 (true) && uluck >= 0 (true)`
`do_bestow = !rn2(6 + 2*0*2) = !rn2(6)` => probability = 1/6
**Result**: CORRECT (expected: 1/6 = 16.7%)

**TV #19 (critically_low_hp boundary)**:
Input: curhp=6, maxhp=30, ulevel=1
`hplim = 15*1 = 15; maxhp = min(30, 15) = 15`
`xlev_to_rank(1) = 0 => divisor = 5`
`6 <= 5? NO. 6*5=30 <= 15? NO.`
Wait -- spec says maxhp=30, but the function caps it: `if (maxhp > hplim) maxhp = hplim` where hplim=15. So maxhp becomes 15.
Then `6*5 = 30 > 15`? No, `30 <= 15` is false. And `6 <= 5` is false.
So the function returns FALSE.

Spec says: "6*5=30 <= 30 -> true (boundary hit)". This is WRONG because the spec forgot the `hplim = 15*ulevel` cap. With ulevel=1, maxhp gets capped to 15, not 30.

**Result**: TV #19 is INCORRECT. The spec fails to account for the `hplim` cap. Correct result: `curhp(6) * divisor(5) = 30 > hplim-capped-maxhp(15)` and `6 > 5`, so result is FALSE, not TRUE.

---

## Summary

| Category | Result |
|----------|--------|
| Tier A | 6/7 PASS (A3 fail: C code in bug section) |
| Tier B | PASS (all key topics covered) |
| Tier C1 | Good coverage; `give_spell()` detail missing |
| Tier C2 | 3/3 formula matches |
| Tier C3 | 2 missing mechanics (gods_upset decrement, rejection penalty detail) |
| Tier C4 | 2/3 TVs correct; TV #19 INCORRECT (hplim cap not applied) |

### Required Fixes

1. **[A3]** Convert C code blocks in Sections 10.1 and 10.2 to pseudocode
2. **[C3]** Add `gods_upset()` full logic: when `g_align != ualign.type && ugangr > 0`, decrement ugangr
3. **[C3]** Add rejection path penalties in `offer_different_alignment_altar()` (ugangr+=3, adjalign(-5), change_luck(-5), adjattrib(WIS,-2))
4. **[C4]** Fix TV #19: with ulevel=1 and maxhp=30, hplim=15 caps maxhp to 15. Result should be FALSE (30 > 15 and 6 > 5)
5. **[C1]** Add `give_spell()` selection algorithm (unknown/forgotten preference, 25% learn-direct chance)
