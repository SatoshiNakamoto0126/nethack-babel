# Review: spellcasting.md (施法系统)

**Reviewer**: Claude Opus 4.6 (1M context)
**Date**: 2026-03-14
**Source verified against**: `spell.c` r1.185 (2427 lines)

---

## Summary

An excellent, highly detailed spec that accurately captures the spellcasting system. The `percent_success()` formula is correctly transcribed and all 10 deterministic test vectors were verified against the source code -- all pass. Coverage of `spell.c` functions is very high. The chain lightning mechanic is well documented. A few minor issues exist: the `learning` formula divisor uses `spell_level` in the spec but the source uses `spellev(spell)` (same thing, but worth noting); the `confused_book` description says "2/3 几率仅浪费时间" which is accurate; and the chain lightning description could be more precise about the diagonal propagation. The "[疑似 bug]" annotations are well-placed and mostly accurate.

**Verdict**: PASS -- no critical issues found.

---

## A. Format Compliance

| Check | Status | Notes |
|-------|--------|-------|
| [A1] >= 10 test vectors | PASS | 10 deterministic test cases (Tests 1-10) + 6 boundary conditions (Boundaries 1-6) = 16 total |
| [A2] >= 2 boundary conditions | PASS | 6 explicit boundary conditions (splcaster=20, read_ability=20, spestudied, Pw, sp_know, difficulty=0) |
| [A3] No C code blocks | PASS | All code blocks are pseudocode or formula notation |
| [A4] No Rust code | PASS | None found |
| [A5] Pseudocode present | PASS | Sections 2.2, 2.3.2, 2.3.4, 2.3.6, 3.1, 4.2-4.5, 5.1-5.7, 6.2, 7.5, 8.3, 9.2-9.4, 10.1 |
| [A6] Exact formulas | PASS | `percent_success()` fully decomposed with explicit constants; SPELL_LEV_PW; age_spells; spell_backfire |
| [A7] [疑似 bug] markers | PASS | 4 items in Appendix section, properly annotated |

---

## B. Content Coverage

| Keyword (from v2 spec S15.2) | Covered | Section |
|------------------------------|---------|---------|
| 施法成功率 | YES | Section 4 (full `percent_success()` decomposition) |
| 能量消耗 | YES | Section 5 (SPELL_LEV_PW, Amulet drain, hunger) |
| 施法惩罚 | YES | Section 6 (full armor penalty table with role data) |

All three keywords thoroughly covered.

---

## C. Source Code Accuracy

### C1. Function Coverage

Non-trivial public/staticfn functions in `spell.c`:

| Function | Covered | Notes |
|----------|---------|-------|
| `spell_let_to_idx()` | No | Minor (UI helper) |
| `cursed_book()` | YES | Section 2.3.4 |
| `confused_book()` | YES | Section 2.3.5 |
| `deadbook_pacify_undead()` | Partial | Via Section 9.1 |
| `deadbook()` | YES | Section 9.1 |
| `learn()` | YES | Section 2.3.6 |
| `rejectcasting()` | YES | Section 9.3 |
| `getspell()` | No | Minor (UI/menu) |
| `study_book()` | YES | Section 2 |
| `age_spells()` | YES | Section 3.1 |
| `cast_chain_lightning()` | YES | Section 8.3 |
| `propagate_chain_lightning()` | YES | Section 8.3 |
| `cast_protection()` | YES | Section 8.7 |
| `spell_backfire()` | YES | Section 9.2 |
| `spelleffects_check()` | YES | Sections 5, 9.3 |
| `spelleffects()` | YES | Section 8 |
| `throwspell()` | YES | Section 9.4 |
| `tport_spell()` | YES | Section 9.5 |
| `losespells()` | YES | Section 10.1 |
| `sortspells()` | No | Minor (UI/sort) |
| `spellsortmenu()` | No | Minor (UI) |
| `dovspell()` | No | Minor (UI, #showspells) |
| `show_spells()` | No | Minor (dumplog) |
| `dospellmenu()` | No | Minor (UI) |
| `percent_success()` | YES | Section 4 (fully decomposed) |
| `spellretention()` | YES | Section 3.3 |
| `initialspell()` | YES | Section 2.5 |
| `known_spell()` | Partial | Implied by memory state system |
| `spell_idx()` | No | Minor (lookup helper) |
| `force_learn_spell()` | YES | Section 2.6 |
| `num_spells()` | No | Minor (count helper) |

**Coverage**: ~22 of ~31 functions = ~71%. Uncovered functions are exclusively UI/display helpers with no gameplay logic.

### C2. Formula Verification (3 most important)

**Formula 1: percent_success() (Section 4)**

Verified the full formula against source lines 2173-2291. Every step matches:

1. `splcaster = urole.spelbase` -- line 2187: MATCH
2. Metal body armor: `splcaster += spelarmr` (or `/2` with robe) -- lines 2191-2195: MATCH
3. Robe without metal: `splcaster -= spelarmr` -- line 2195: MATCH
4. Shield: `splcaster += spelshld` -- lines 2196-2197: MATCH
5. Quarterstaff: `splcaster -= 3` -- lines 2199-2200: MATCH
6. Metal accessories: `+4/+6/+2` -- lines 2202-2208: MATCH
7. `splcaster = min(splcaster, 20)` -- lines 2222-2223: MATCH
8. `chance = 11 * statused / 2` -- line 2230: MATCH
9. `difficulty = (spellev-1)*4 - (skill*6 + ulevel/3 + 1)` -- lines 2239-2240: MATCH
10. Positive difficulty: `chance -= isqrt(900*difficulty + 2000)` -- line 2244: MATCH
11. Negative difficulty: `learning = 15 * -difficulty / spellev; chance += min(learning, 20)` -- lines 2251-2252: MATCH
12. Clamp [0, 120] -- lines 2260-2263: MATCH
13. Heavy shield: `chance /= 4` (or `/2` for spelspec) -- lines 2269-2275: MATCH
14. Final: `chance = chance * (20 - splcaster) / 15 - splcaster` -- line 2283: MATCH
15. Clamp [0, 100] -- lines 2286-2289: MATCH

**EXACT MATCH** on all 15 steps.

**Formula 2: SPELL_LEV_PW (Section 5.1)**

Spec: `energy = spell_level * 5`
Source (`include/spell.h` line 36): `#define SPELL_LEV_PW(lvl) ((lvl) * 5)`

**MATCH**.

**Formula 3: spell_backfire() duration (Section 9.2)**

Spec: `duration = (spell_level + 1) * 3, range 6..24`
Source (line 1183): `long duration = (long) ((spellev(spell) + 1) * 3)`

Spec: 40%/30%/20%/10% distribution of confusion/stun
Source (lines 1194-1214): `switch (rn2(10)): cases 0-3 (40%), 4-6 (30%), 7-8 (20%), 9 (10%)`

**MATCH** on both formula and distribution.

### C3. Missing Mechanics

**CRITICAL**: None found.

**MINOR**:

1. **Chain lightning diagonal propagation details**: The spec says "初始 strength=2, 每步 strength-1, 命中非电抗性怪物: strength 恢复至 3". The source (line 976) confirms `zap.strength = 3` on hitting a non-resistant monster. However, the spec doesn't fully describe the diagonal spreading behavior: after the forward propagation, if `strength >= 2 && u.uen > 0`, the Pw cost applies; but diagonals start with `strength = 0` unless a monster was hit (in which case they inherit the forward zap's strength). The spec's description at lines 691-697 is mostly correct but slightly imprecise about when diagonals get strength.

2. **`confused_book()` can trigger during `learn()` too**: The spec only describes confused_book in the context of `study_book()`, but the source (line 368-376) shows that if confusion occurs while reading (during `learn()` occupation), `confused_book()` is also called. The spec doesn't note this timing.

3. **`cursed_book()` after successful learning**: Source lines 450-457 show that even after successfully learning a spell, if the book is cursed, `cursed_book()` is called again. The spec doesn't mention this post-learning cursed book check.

4. **Lenses speed bonus in `learn()` vs `study_book()`**: The spec (Section 2.2) mentions lenses give a speed bonus during reading with the formula `if delay > 0 && ublindf && ublindf->otyp == LENSES && rn2(2): delay++`. However, the source (lines 365-367) shows this check is in `learn()` (the occupation callback), not `study_book()`. The distinction matters because the lenses check happens every turn during the reading process, not just at the start. The spec's pseudocode placement at Section 2.2 could be misleading but the formula itself is correct.

5. **`capacity_check` returns ECMD_TIME not ECMD_OK**: Source line 1281 shows `*res = ECMD_TIME` for the overloaded case. The spec says it blocks casting but doesn't specify whether a turn is consumed. A turn IS consumed when overloaded.

### C4. Test Vector Verification

**Test 1: Wizard, naked, magic missile, Expert, INT 18, level 14**

Hand-verified all steps:
- `splcaster = 1 + (-4) = -3`
- `chance = 11*18/2 = 99`
- `difficulty = 4 - 23 = -19`
- `learning = 15*19/2 = 142; min(142,20) = 20`
- `chance = 99 + 20 = 119; clamp = 119`
- `final = 119*23/15 + 3 = 182 + 3 = 185; clamp = 100`

Source lines 2187-2291 confirm each step. **PASS**.

**Test 2: Barbarian, naked, haste self, Basic escape, INT 7, level 1**

- `splcaster = 14 + (-4) = 10`
- `chance = 11*7/2 = 38`
- `difficulty = 8 - 7 = 1`
- `chance -= isqrt(2900) = 53; chance = -15; clamp = 0`
- `final = 0*(20-10)/15 - 10 = -10; clamp = 0`

**PASS**.

**Test 5: Wizard, full metal, Expert, INT 18, level 20**

- `splcaster = 1 + 10 + 4 + 6 + 2 + (-4) = 19`
- `chance = 119` (same as Test 1)
- `final = 119*(20-19)/15 - 19 = 119/15 - 19 = 7 - 19 = -12; clamp = 0`

**PASS**.

All 10 test vectors and 6 boundary conditions verified -- **all correct**.

---

## D. Specific Issues

### D1. MINOR: Post-learning cursed book effect omitted

Source lines 450-457 in `learn()` show that after successfully learning a spell from a cursed book, `cursed_book()` is called. If it returns TRUE (explosion), the book is destroyed. This is separate from the pre-learning failure pathway and is not documented in the spec.

### D2. MINOR: Chain lightning diagonal propagation precision

The spec states "链接传播时如果 `strength >= 2` 且 `u.uen > 0`: 额外消耗 1 Pw" (Section 8.3, line 695). Source lines 1084-1087 show the check is `if (zap.strength < 2) zap.strength = 0; else if (u.uen > 0) u.uen--;`. The condition is that the Pw cost applies to the FORWARD propagation direction when strength >= 2, not to the diagonal directions. The diagonal directions are propagated with potentially 0 strength unless a monster was hit. The spec's description is not wrong but could be clearer.

### D3. MINOR: Capacity check consumes a turn

Source line 1281 sets `*res = ECMD_TIME` when capacity check fails, meaning a turn is consumed. The spec (Section 9.3 item 6) mentions capacity blocking casting but doesn't specify turn consumption.

### D4. Cosmetic: [疑似 bug] #4 self-correction

The spec's [疑似 bug] item 4 about `study_book()` using only INT (not spelstat) for `read_ability` is correctly identified as a potential design question. The analysis is accurate -- the source (line 582) uses `ACURR(A_INT)` unconditionally regardless of role's `spelstat`. This is a legitimate observation.

### D5. Cosmetic: [疑似 bug] #3 about division by zero

The spec correctly notes that `spellev(spell)` as a divisor could theoretically be zero for `SPE_BLANK_PAPER`. The source confirms `SPE_BLANK_PAPER` has level 0 in objects.h. However, as the spec notes, blank paper won't appear in `spl_book[]` under normal circumstances. This is a valid defensive-programming observation.
