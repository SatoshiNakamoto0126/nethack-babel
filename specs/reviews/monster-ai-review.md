# Review: monster-ai.md

## Summary
- Quality: GOOD
- Test vectors: 18 (TV1-TV18) [PASS]
- Boundary conditions: 4 (TV9-TV12) [PASS]
- C/Rust code leaks: NONE
- Bug markers: 2

## A. Format Compliance

- [A1] Test vectors: 18 individual test cases (TV1-TV4: movement, TV5-TV8: fear/flee, TV9-TV12: boundary conditions, TV13-TV15: target perception, TV16-TV18: ranged attack). PASS (>=10)
- [A2] Boundary conditions: TV9 (covetous HP exactly mhpmax/3), TV10 (covetous HP=mhpmax/3+1), TV11 (covetous full HP), TV12 (mfleetim cap at 127). PASS (>=2)
- [A3] No C code: all code blocks use pseudocode notation. Chinese section headers and comments used throughout. No `int `, `void `, `static `, `struct `, `#include`, `#define`. PASS
- [A4] No Rust code: no `let mut`, `match `, `impl `, `pub fn`. PASS
- [A5] Pseudocode present: sections 1.1, 2 (Phase 1-4), 3, 4.1-4.5, 5, 6.1-6.6, 7.1-7.4, 8.2-8.5, 9.4, 10.4-10.5, 11.2-11.6, 12.1-12.3, 13.1-13.6, 14.1-14.2, 15 all contain pseudocode blocks. PASS
- [A6] Exact formulas: includes constants (NORMAL_SPEED=12, BOLT_LIM=8, MON_POLE_DIST=5, MTSZ=4, UTSZ=100, SQSRCHRADIUS=5, mfleetim cap=127). Flee time formula, courage recovery probability (1/25), confusion recovery (1/50), stun recovery (1/10) all exact. PASS
- [A7] Bug markers: 2 instances -- one `[疑似 bug]` in section 4.5 (Wizard case 1 fallthrough) and one in section 11.5 (move+shoot asymmetry). PASS

## B. Content Coverage

v2 spec 15.4 monster-ai keywords:
- **追踪算法**: Covered in section 6 (appr direction system, hero track buffer UTSZ=100, monster self-track MTSZ=4, mfndpos position search with flag table, movement selection algorithm, shortsighted levels). PASS
- **逃跑阈值**: Covered in sections 4.1-4.4 (distfleeck scare detection, monflee flee timer, onscary Elbereth/scroll immunity lists, courage recovery conditions). Also section 4.5 (covetous HP thresholds for STRAT_HEAL). PASS
- **目标选择**: Covered in section 5 (set_apparxy with displacement/invisibility/Xorn), section 8 (covetous target priority for artifacts), section 6.1 (appr computation with multiple override conditions). PASS

All 3 keywords have dedicated coverage.

## C. Source Accuracy

### C1. Function Coverage

Non-trivial public/static functions in monmove.c, wizard.c relevant to monster-ai:

1. `dochug` -- COVERED (section 2, four-phase decomposition)
2. `m_move` -- COVERED (sections 6.5, 12-13, referenced throughout)
3. `distfleeck` -- COVERED (section 4.1)
4. `monflee` -- COVERED (section 4.2)
5. `onscary` -- COVERED (section 4.3, immunity lists)
6. `set_apparxy` -- COVERED (section 5)
7. `disturb` -- COVERED (section 3)
8. `m_balks_at_approaching` -- COVERED (section 11.4)
9. `mind_blast` -- COVERED (section 2 Phase 2, 1/20 probability)
10. `strategy` -- COVERED (section 4.5, 8.2)
11. `tactics` -- COVERED (section 8.3)
12. `choose_stairs` -- mentioned (section 8.3)
13. `intervene` -- COVERED (section 8.4)
14. `mon_regen` -- COVERED (section 15)
15. `watch_on_duty` -- COVERED (section 9.4)
16. `postmov` -- COVERED (section 7.2 door handling)
17. `maybe_spin_web` -- COVERED (section 13.2)
18. `leppie_stash` -- COVERED (section 13.1)
19. `leppie_avoidance` -- COVERED (section 6.1)
20. `mon_allowflags` -- COVERED (section 6.4 flag table)
21. `should_displace` -- COVERED (section 14.1)
22. `undesirable_disp` -- COVERED (section 14.2)
23. `m_avoid_kicked_loc` -- mentioned (section 6.5)
24. `vamp_shift` -- COVERED (section 7.3)
25. `m_arrival` -- mentioned (section 2 Phase 1)
26. `m_respond` -- COVERED (section 2 Phase 1)
27. `m_search_items` -- partially covered (section 12)
28. `release_hero` -- COVERED (section 4.2)

Coverage: 26/28 = 93%.

### C2. Formula Spot-Checks

**Spot-check 1: `distfleeck` flee time formula**
- Spec (section 4.1): `flee_time = rnd(rn2(7) ? 10 : 100)` -- 6/7 probability of rnd(10)=1..10, 1/7 probability of rnd(100)=1..100.
- Source (monmove.c:564): `monflee(mtmp, rnd(rn2(7) ? 10 : 100), TRUE, TRUE);`
- MATCH exactly.

**Spot-check 2: `set_apparxy` displacement logic**
- Spec (section 5): `displ = couldsee(mux, muy) ? 2 : 1` when displaced. `gotu = !rn2(4)` for displaced (1/4 chance of seeing through). Random offset `u.ux - displ + rn2(2*displ+1)`.
- Source (monmove.c:2225-2250): `displ = couldsee(mx, my) ? 2 : 1;` ... `gotu = notseen ? !rn2(3) : notthere ? !rn2(4) : FALSE;` ... `mx = u.ux - displ + rn2(2 * displ + 1);`
- MATCH. Note the spec uses `mux, muy` for the `couldsee` check (the monster's *previous* guess of where the hero is), which matches the source's local `mx = mtmp->mux, my = mtmp->muy`.

**Spot-check 3: `m_balks_at_approaching` distance threshold**
- Spec (section 11.4): "non-peaceful + distance < 5 squares (dist2 < 25) + can see player"
- Source (monmove.c:1195): `if (mtmp->mpeaceful || (edist >= 5 * 5) || !m_canseeu(mtmp)) return oldappr;`
- The source uses `edist >= 25` (i.e., `dist2 >= 25`), which means the function returns early for distances >= 5. The spec says "dist2 < 25", which is equivalent -- only monsters within 5 squares (exclusive) are checked. MATCH.
- Note: spec says "distance < 5 格" but `dist2 < 25` means Euclidean distance < 5, not Chebyshev. This is a dist2 check, so a monster at position (4,0) has dist2=16 < 25 (checked), while (4,3) has dist2=25 (not checked). Correct.

### C3. Missing Mechanics

1. **`m_move()` full flow**: The `m_move()` function is ~500 lines and is the core movement execution. The spec covers the inputs (appr, mfndpos results) and the selection logic, but the full internal flow -- including Tengu teleport (section 10.4), unicorn avoidance (section 10.5), hiding behavior (section 13.4), and item search (section 12) -- is distributed across multiple sections rather than presented as a unified flow. This is an acceptable decomposition for readability.

2. **`shk_move()` / `pri_move()` / `gd_move()`**: These are separate movement functions for shopkeepers, priests, and vault guards that bypass the normal `m_move()` path. The spec acknowledges this (section 9) but does not detail their internals. This is appropriate scope exclusion.

3. **Conflict resistance**: Section 9.5 mentions "checks `resist_conflict`" but doesn't detail the resistance formula. `resist_conflict()` checks `mtmp->mpeaceful && !Conflict` and magic resistance. Minor omission.

4. **`gelatinous_cube_digests()` and `bee_eat_jelly()`**: These Phase 3 pre-movement actions (dochug lines 868-878) are not covered. Killer bee -> queen bee transformation via royal jelly and gelatinous cube digestion are special dochug actions.

### C4. Test Vector Verification

**TV3 (speed=12, MSLOW):**
- Spec: `mmove = (24+1)/3 = 8`
- Source: `if (mmove < 12)` -- 12 is NOT < 12, so the `else` branch is used: `mmove = 4 + 12/3 = 4 + 4 = 8`.
- The spec uses the WRONG formula branch (the `< 12` branch gives `(2*12+1)/3 = 25/3 = 8` by coincidence). The result (8) is correct, but the formula shown is from the wrong code path. **INACCURACY** -- the comment should note this uses the `else` branch, not the `if mmove < 12` branch. The final value is correct by numerical coincidence.

**TV9 (covetous HP = mhpmax/3, e.g. max=30, hp=10):**
- Spec: `(10*3)/30 = 1` -> non-Wizard returns STRAT_HEAL, Wizard falls through.
- Source (wizard.c:283-296): `switch ((mtmp->mhp * 3) / mtmp->mhpmax)` where `(10*3)/30 = 30/30 = 1`. Case 1: non-Wizard returns STRAT_HEAL, Wizard falls through to case 2 setting `dstrat = STRAT_HEAL`. CORRECT.

**TV15 (Xorn vs invisible player with gold):**
- Spec: `displ = 0`, exact positioning (smells gold).
- Source (monmove.c:2224): `displ = (mtmp->data == &mons[PM_XORN] && umoney) ? 0 : 1;`
- CORRECT -- Xorn with `umoney > 0` gets `displ = 0`.

## D. Issues Found

### D1. Error: Wizard harassment repeat countdown

Section 8.4 states the countdown is `u.udg_cnt = rn1(250, 50) = 50..299 turns`. This is the INITIAL value set when the Wizard first dies (wizard.c:814). However, the REPEAT value after each `intervene()` call is `rn1(200, 50) = 50..249` (allmain.c:366). The spec does not distinguish between initial and repeat countdowns.

- Initial (wizard.c:814): `u.udg_cnt = rn1(250, 50);` -- range 50..299
- Repeat (allmain.c:366): `u.udg_cnt = rn1(200, 50);` -- range 50..249

**Recommendation**: Document both values. The repeat countdown is shorter, meaning harassment accelerates over time.

### D2. Minor: TV3 formula branch error

As noted in C4, TV3 shows `mmove = (24+1)/3 = 8` for speed=12 MSLOW. This uses the `mmove < 12` formula, but speed 12 is NOT < 12 -- the code takes the `else` branch: `mmove = 4 + 12/3 = 8`. The result is correct by coincidence (both formulas yield 8 for input 12), but the shown calculation path is wrong.

### D3. Minor: Gremlin fear description

Section 4.1 states "Gremlin 怕光（4/5 概率）" with code `flees_light AND !rn2(5)`. The `!rn2(5)` means `rn2(5) == 0`, which is a 1/5 probability of returning TRUE. So the gremlin has a 1/5 chance of being brave (bravegremlin), meaning it fears light 4/5 of the time. The spec parenthetical says "4/5 概率" which is correct, but the negation in the code `!rn2(5)` makes this the probability of NOT being brave, i.e., the probability of being scared. Let me verify: `bravegremlin = (rn2(5) == 0)` = 1/5 chance of being brave. `flees_light(mtmp) && !bravegremlin` = flees_light AND 4/5. So 4/5 probability of being scared IF flees_light is true. CORRECT.

### D4. Informational: [疑似 bug] quality

1. Section 4.5 Wizard fallthrough: correctly identifies that the `case 1` fallthrough for Wizard means 33-66% HP Wizards will pursue targets instead of healing. Correctly notes this appears intentional ("the wiz is less cautious"). Well-analyzed.

2. Section 11.5 move+shoot asymmetry: correctly quotes the source comment about monsters being able to move and shoot on the same turn while the hero cannot. This is a known design decision, not a bug.

## E. Final Assessment

PASS. The spec provides a thorough decomposition of the monster AI decision loop, covering all three v2 keywords with exact formulas and pseudocode. The four-phase dochug breakdown is clear and matches the source structure. Two issues to address:

1. **Fix**: Section 8.4 should distinguish initial (rn1(250,50)=50..299) vs repeat (rn1(200,50)=50..249) Wizard harassment countdown.
2. **Fix**: TV3 should show the correct formula branch for speed=12 MSLOW (`4 + 12/3 = 8`, not `(2*12+1)/3 = 8`).

Neither issue affects correctness of final values, but both represent documentation inaccuracies that could mislead an implementer.
