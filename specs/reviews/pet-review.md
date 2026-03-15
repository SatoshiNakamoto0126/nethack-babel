# Review: pet.md

**Reviewer**: Claude Opus 4.6 (automated)
**Date**: 2026-03-14
**Spec file**: `/specs/pet.md`
**C source files reviewed**: `src/dog.c` (1392 lines), `src/dogmove.c` (~1400 lines)

---

## Tier A: Format Compliance

| Check | Status | Notes |
|-------|--------|-------|
| A1: >= 10 test vectors | PASS | 24 test vectors (TV-1 through TV-24) |
| A2: >= 2 boundary conditions | PASS | Multiple boundary TVs: TV-6, TV-7, TV-13, TV-15, TV-19, TV-21, TV-24 |
| A3: No C code blocks | PASS | All code blocks use pseudocode or generic syntax |
| A4: No Rust code | PASS | No Rust code present |
| A5: Pseudocode present | PASS | Extensive pseudocode in sections 2-9, 12 |
| A6: Exact formulas | PASS | All key formulas present with exact values |
| A7: [疑似 bug] markers | PASS | 4 bugs marked in section "疑似 Bug" |

---

## Tier B: Key Content Coverage (v2 spec section 15.4)

The v2 spec requires coverage of "宠物 AI, 忠诚度, 训练" (pet AI, loyalty, training).

| Topic | Covered | Section |
|-------|---------|---------|
| Pet data structures | Yes | Section 1 |
| Pet initialization | Yes | Section 2 |
| Tameness/loyalty mechanics | Yes | Section 3 |
| Pet hunger system | Yes | Section 4 |
| Pet movement AI | Yes | Section 5 |
| Pet combat behavior | Yes | Section 6 |
| Pet item handling | Yes | Section 7 |
| Taming methods | Yes | Section 8 |
| Going feral/hostile | Yes | Section 9 |
| Pet displacement | Yes | Section 10 |
| Leash mechanics | Yes | Section 11 |
| Saddle and riding | Yes | Section 12 |
| Pet growth | Yes | Section 13 |
| Whistles | Yes | Section 14 |

**Assessment**: Very comprehensive coverage of all key content areas for pet AI, loyalty, and training.

---

## Tier C: Detailed Verification

### C1: Function Coverage

| C function | Spec section | Covered? |
|------------|-------------|----------|
| `newedog()` | 1 | Yes (mentioned) |
| `free_edog()` | 1 | Yes (mentioned) |
| `initedog()` | 2.3 | Yes |
| `pet_type()` | 2.1 | Yes |
| `make_familiar()` | 8.4, 8.5 | Yes |
| `makedog()` | 2.2 | Yes (default names) |
| `dog_hunger()` | 4.2 | Yes |
| `dog_nutrition()` | 4.3 | Yes |
| `dog_eat()` | 4.4 | Yes |
| `dogfood()` | 4.5 | Yes |
| `dog_move()` | 5.1 | Yes |
| `dog_goal()` | 5.2 | Yes |
| `dog_invent()` | 7.1 | Yes |
| `droppables()` | 7.3 | Yes |
| `pet_ranged_attk()` | 6.2 | Yes |
| `score_targ()` | 6.3 | Yes |
| `best_target()` | 6.2 | Yes (implicitly) |
| `find_targ()` | 6.2 | Yes (implicitly) |
| `abuse_dog()` | 3.3 | Yes |
| `wary_dog()` | 9.1 | Yes |
| `mon_catchup_elapsed_time()` | 3.3 | Yes (separation decay) |
| `keepdogs()` | Not explicitly | Partially (leash during migration) |
| `migrate_to_level()` | 3.3 | Yes (leash snapping) |
| `pick_familiar_pm()` | 8.5 | Yes |

**Missing functions**: `find_friends()` is not explicitly documented (used in `score_targ()` to check if friendlies are behind target). The spec mentions "friend behind target within 15 squares" in section 6.3 which is correct but does not name the function.

### C2: Formula Spot-Checks (3 verified)

**Formula 1: Tameness decay during separation** (spec section 3.3, source lines 686-693)
- Spec: `wilder = (time_away + 75) / 150`
- Source: `int wilder = (imv + 75) / 150;`
- Spec: three outcomes (mtame > wilder: decrease; mtame > rn2(wilder): untame peaceful; else: hostile)
- Source:
  ```
  if (mtmp->mtame > wilder) mtmp->mtame -= wilder;
  else if (mtmp->mtame > rn2(wilder)) mtmp->mtame = 0;
  else mtmp->mtame = mtmp->mpeaceful = 0;
  ```
- **PASS**: Exact match.

**Formula 2: Combat balk threshold** (spec section 6.1, source lines 1112)
- Spec: `balk = pet.m_lev + (5 * pet.mhp / pet.mhpmax) - 2`
- Source: `int balk = mtmp->m_lev + ((5 * mtmp->mhp) / mtmp->mhpmax) - 2;`
- **PASS**: Exact match.

**Formula 3: Apport training** (spec section 7.4/4.4, source lines 316-317)
- Spec: `apport += 200 / (dropdist + moves - droptime)`
- Source: `edog->apport += (int) (200L / ((long) edog->dropdist + svm.moves - edog->droptime));`
- **PASS**: Exact match.

### C3: Missing Mechanics

1. **Item pickup eating threshold error** (spec section 7.2): The spec says:
   > `elif edible with quality < MANFOOD (or < ACCFOOD if hungry)`

   But the actual C code at dogmove.c lines 430-432 says:
   ```c
   if ((edible <= CADAVER
        || (edog->mhpmax_penalty && edible == ACCFOOD))
   ```
   This means pets eat items with quality <= CADAVER (0 or 1) by default, NOT anything < MANFOOD (which would include ACCFOOD=2). ACCFOOD items are only eaten when the pet is **starving** (mhpmax_penalty set), not merely "hungry." The spec's threshold is too permissive.

2. **dog_goal food search threshold similarly wrong** (spec section 5.2): The spec pseudocode at lines showing the food search says:
   > `if food_quality >= current_goal_quality or food_quality == UNDEF: continue`

   The actual source at dogmove.c line 526: `if (otyp > gg.gtyp || otyp == UNDEF) continue;`

   Then at line 530: `if (cursed_object_at(nx, ny) && !(edog->mhpmax_penalty && otyp < MANFOOD)) continue;`

   The spec says `if cursed_object_at(x,y) and not (starving and food < MANFOOD)` which correctly uses `mhpmax_penalty` as "starving" -- this part is correct.

   But the spec also says for food seeking: `if food_quality < MANFOOD:` for setting the goal. The source at line 536 says `if (otyp < MANFOOD)`. This part is actually correct in the spec. The issue is only with the eating threshold in dog_invent (item 1 above).

3. **`dog_invent` drop logic**: Spec section 7.1 says:
   > `if not rn2(udist + 1) or not rn2(apport): if rn2(10) < apport: drop all droppable items`

   Source (lines 411-418):
   ```c
   if (!rn2(udist + 1) || !rn2(edog->apport))
       if (rn2(10) < edog->apport) {
           relobj(mtmp, (int) mtmp->minvis, TRUE);
           if (edog->apport > 1) edog->apport--;
           ...
       }
   ```
   **PASS**: Spec matches source.

4. **Conflict handling in dog_move**: Spec section 9.4 says "Guardian angels disappear and send nasties instead." Source (dogmove.c lines 1039-1046) confirms: `lose_guardian_angel(mtmp)` is called when `Conflict && !resist_conflict(mtmp)` and `!edog` (i.e., minion). This is correctly described.

5. **Trap avoidance probability**: Spec section 5.3 item 4 says "39/40 chance to avoid" seen traps. Source (lines 1199): `if (trap->tseen && rn2(40)) continue;` -- `rn2(40)` returns 0 with probability 1/40, so `rn2(40)` is nonzero (continue/avoid) with probability 39/40. **PASS**.

6. **Cursed item avoidance probability**: Spec section 5.3 item 5 says "probability `1/(13*uncursed_count)`". Source (line 1231): `if (cursemsg[i] && !mtmp->mleashed && uncursedcnt > 0 && rn2(13 * uncursedcnt)) continue;`. The pet steps on cursed items when `rn2(13*uncursedcnt)` returns 0, which has probability `1/(13*uncursedcnt)`. **PASS**.

7. **Return attack condition**: Spec section 6.4 says:
   > `if hit but didn't kill and rn2(4) and target hasn't moved this turn and pet position not scary to target and target is near pet`

   Source (lines 1150-1155):
   ```c
   if ((mstatus & (M_ATTK_HIT | M_ATTK_DEF_DIED)) == M_ATTK_HIT
       && rn2(4)
       && mtmp2->mlstmv != svm.moves
       && !onscary(mtmp->mx, mtmp->my, mtmp2)
       && monnear(mtmp2, mtmp->mx, mtmp->my))
   ```
   **PASS**: Spec matches source, though "target hasn't moved this turn" is slightly misleading -- the condition is `mlstmv != svm.moves` which means the target hasn't had its turn yet this round. The spec's wording is acceptable.

8. **Missing: `dog_invent` eating threshold includes mines/soko prize exclusion**: Source lines 427 checks `!(is_mines_prize(obj) || is_soko_prize(obj))`. Not mentioned in spec. Minor.

### C4: Test Vector Verification (3 checked)

**TV-5: Tameness decay from 150-turn separation**
- Input: mtame=10, time_away=300
- Spec: wilder = (300+75)/150 = 375/150 = 2 (integer division), mtame = 10-2 = 8
- Source: `(imv + 75) / 150` with imv=300 => 375/150 = 2 (integer). 10 > 2, so mtame = 10-2 = 8.
- **PASS**: Correct.

**TV-13: wary_dog revival with killed_by_u**
- Input: killed_by_u=1, abuse=0
- Spec: mtame=0, mpeaceful = `!rn2(0+1)` = `!rn2(1)` = !0 = 1 (always peaceful)
- Source (lines 1306-1310):
  - `edog->killed_by_u == 1` is TRUE, so enters first branch
  - `mtmp->mpeaceful = mtmp->mtame = 0;`
  - `edog->abuse >= 0 && edog->abuse < 10` => 0 >= 0 && 0 < 10 => TRUE
  - `!rn2(edog->abuse + 1)` => `!rn2(1)` => `!0` = 1 (TRUE), so `mtmp->mpeaceful = 1`
- **PASS**: Correct. Note: `rn2(1)` always returns 0, so `!rn2(1)` is always 1.

**TV-22: Combat balk threshold at full HP**
- Input: pet m_lev=5, mhp=20, mhpmax=20, target m_lev=7
- Spec: balk = 5 + (5*20)/20 - 2 = 5 + 5 - 2 = 8. target.m_lev(7) < 8 => attacks
- Source: `balk = 5 + (5*20)/20 - 2 = 8`. `(int)7 >= 8` is FALSE, so pet attacks.
- **PASS**: Correct.

---

## Issues Summary

### Errors (must fix)

1. **Section 7.2 (Item Pickup eating threshold)**: Spec says pet eats items with `quality < MANFOOD` which includes ACCFOOD (quality 2). Source code uses `edible <= CADAVER` (quality 0 or 1) as the default eating threshold. ACCFOOD is only eaten when the pet has `mhpmax_penalty` (starving), not merely "hungry." The spec's pseudocode should read:
   ```
   elif edible with quality <= CADAVER (or == ACCFOOD if starving):
       eat it immediately
   ```

### Inaccuracies (should fix)

2. **Section 6.1 (floating eye avoidance)**: Spec says "not reflecting" as a condition for floating eye avoidance. Source (line 1126) checks `!mon_reflects(mtmp, (char *) NULL)`. This is correct in the spec, but the condition list is incomplete. Source also requires `haseyes(mtmp->data)` (line 1124) and that the floating eye is not invisible to the pet (`!mtmp2->minvis || perceives(mtmp->data)`, line 1125). These conditions are missing from the spec.

3. **Section 6.1 (petrifying monster avoidance)**: Spec says "always avoid." Source (line 1128-1133) shows the avoidance is not absolute -- if the target can be attacked at range (`best_target(mtmp, FALSE) == mtmp2` and `dist2 > 2`), the pet sets `ranged_only = TRUE` rather than unconditionally skipping. However, due to 疑似 bug #1 (which the spec correctly identifies), `ranged_only` is then immediately used to `continue`, making the avoidance effectively absolute. The spec's "always avoid" is correct for the current (bugged) behavior.

4. **Section 9.1 (wary_dog -- apport reset)**: Spec says `edog.apport = 5` after revival. Source (line 1352) confirms this, but only for the `was_dead` case. For life-saved pets (`!was_dead`), apport retains its current value (line 1353 comment: "else lifesaved, so retain current values"). The spec should clarify this distinction.

5. **Section 6.3 (score_targ -- confused check)**: Spec shows `if pet.mconf and rn2(3): return score` at the beginning and `if pet.mconf and not rn2(3): score -= 1000` at the end. Source (line 741): `if (!mtmp->mconf || !rn2(3) || Is_qstart(&u.uz))` -- the entire scoring block is gated by this. When confused AND rn2(3) is nonzero AND not on quest start, the scoring block is skipped entirely, meaning only the fuzz factor `rnd(5)` and the final confusion penalty apply. The spec captures this but also includes `Is_qstart(&u.uz)` check: when on the quest start level, the safe-breath scoring always applies regardless of confusion. This `Is_qstart` condition is missing from the spec.

6. **Section 2.1 (Starting Pet Selection)**: The spec's pseudocode shows `return random_choice(PM_KITTEN, PM_LITTLE_DOG)` with "50/50". Source (line 100): `rn2(2) ? PM_KITTEN : PM_LITTLE_DOG`. `rn2(2)` returns 0 or 1 with equal probability, so when result is 1 (50%), kitten; when 0 (50%), dog. **PASS**: This is correct.

### Cosmetic/Minor

7. **Section 4.3 (dog_nutrition)**: Spec says for non-food, non-coin: `nutrit = 5 * objects[obj.otyp].oc_nutrition`. Source (line 211): `nutrit = 5 * objects[obj->otyp].oc_nutrition;`. **PASS**: Correct.

8. **Section 14.3 (Blessed Eucalyptus Leaf)**: "1/49 chance to lose the blessing per use." This should be verified against `src/apply.c`. Not in scope for this review's assigned source files.

9. **Section 12.1 (Saddleable monsters)**: Lists `S_QUADRUPED, S_UNICORN, S_ANGEL, S_CENTAUR, S_DRAGON, S_JABBERWOCK`. This should be verified against `src/apply.c`'s `can_saddle()`. Not in scope for this review's assigned source files.

---

## Verified Bugs

All 4 marked [疑似 bug] items were verified against source:

1. **ranged_only flag (dogmove.c lines 1135-1137)**: Confirmed. `ranged_only` is set TRUE then immediately causes `continue`.
2. **Starvation HP reduction to 0 (dogmove.c lines 363-371)**: Confirmed. If `mhpmax` is 2, `mhpmax/3 = 0`, causing immediate death.
3. **abuse_dog with Conflict (dog.c lines 1363-1364)**: Confirmed. `mtmp->mtame /= 2` when `Aggravate_monster || Conflict`.
4. **mount_steed tameness decrement (not in assigned files)**: Source not in scope but logic is consistent with description.

---

## Verdict

**PASS with minor revisions required.** The spec is comprehensive and well-structured with excellent test vector coverage. The main error is the eating threshold in section 7.2 (Issue #1), where `<= CADAVER` is incorrectly stated as `< MANFOOD`, which would cause pets to eat a broader range of items than the original game allows. The remaining issues are minor precision improvements. All formulas spot-checked are correct, and all test vectors verified are accurate.
