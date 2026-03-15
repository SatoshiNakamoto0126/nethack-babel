# Review: monster-attack.md

## Summary
- Quality: GOOD
- Test vectors: 12 [PASS]
- Boundary conditions: 4 [PASS]
- C/Rust code leaks: 1
- Bug markers: 2

## A. Format Compliance

- [A1] Test vectors: 12 individual test cases (TV-1 through TV-12). PASS (>=10)
- [A2] Boundary conditions: 4 explicit boundary tests (TV-5: negative AC damage clamp to 1, TV-9: helmet 1/8 pass-through, TV-11: new moon stone, TV-12: digestion timer calculation). PASS (>=2)
- [A3] No C code: FAIL (1 instance). Section 1 contains:
  ```
  struct attack {
      aatyp   -- 攻击方式 (AT_CLAW, AT_BITE, ...)
      adtyp   -- 伤害类型 (AD_PHYS, AD_FIRE, ...)
      damn    -- 伤害骰子数
      damd    -- 每个骰子面数
  }
  ```
  While field names use `--` comments instead of C type annotations, the `struct attack { ... }` framing is C-like. This is borderline -- the content is more pseudocode than compilable C, but uses `struct` keyword. Marking as 1 minor leak.
- [A4] No Rust code: PASS
- [A5] Pseudocode present: Sections 2.2, 2.5, 4.1-4.6, and each AD_* subsection include pseudocode-style algorithms. PASS
- [A6] Exact formulas: MC formula (`!(rn2(10) >= 3 * armpro)`), hit formula (`AC_VALUE + 10 + m_lev`), digestion timer, permanent damage formula all have explicit constants. PASS
- [A7] Bug markers: 2 instances of `[疑似 bug]` (section 7.1 digestion timer priority, section 7.1 comment about "shorter time is more dangerous"). PASS

## B. Content Coverage

v2 spec 15.1 keywords:
- **怪物攻击玩家**: Covered extensively in sections 2-5 (mattacku flow, hitmu, damage types). PASS
- **被动攻击**: Covered in section 8 (passiveum) with acid/stone/enchant/phys/plys/cold/stun/fire/elec. PASS
- **特殊效果**: Covered across sections 5-14 (stone, drain, poison, seduce, engulf, gaze, explosion, spells). PASS

All 3 keywords have dedicated coverage.

## C. Source Accuracy

### C1. Function Coverage

Non-trivial functions in mhitu.c:
1. `mattacku` -- COVERED (section 2)
2. `hitmu` -- COVERED (section 4)
3. `gulpmu` -- COVERED (section 7)
4. `explmu` -- COVERED (section 12)
5. `gazemu` -- COVERED (section 11)
6. `getmattk` -- COVERED (section 3)
7. `hitmsg` -- referenced but not detailed (message function)
8. `missmu` -- referenced
9. `wildmiss` -- mentioned (section 17, note 2)
10. `calc_mattacku_vars` -- covered indirectly (section 2.1)
11. `summonmu` -- COVERED (section 14)
12. `diseasemu` -- COVERED (AD_PEST, AD_DISE sections)
13. `u_slip_free` -- COVERED (section 13)
14. `magic_negation` -- COVERED (section 10)
15. `mhitm_mgc_atk_negated` -- COVERED (section 10)
16. `doseduce` -- COVERED (section 6)
17. `mayberem` -- mentioned (section 6)
18. `assess_dmg` -- NOT COVERED (post-damage assessment, minor)
19. `passiveum` -- COVERED (section 8)
20. `could_seduce` -- mentioned (section 6)
21. `mdamageu` -- referenced
22. `ranged_attk_assessed` -- NOT COVERED (minor utility)
23. `mtrapped_in_pit` -- NOT COVERED (minor)
24. `mon_avoiding_this_attack` -- NOT COVERED (minor)
25. `ranged_attk_available` -- NOT COVERED (minor)
26. `cloneu` -- NOT COVERED (minor)

uhitm.c (monster-attack-relevant, AD_* handlers):
27. `mhitm_ad_phys` -- COVERED (AD_PHYS section)
28. `mhitm_ad_fire/cold/elec/acid` -- COVERED (sections 5)
29. `mhitm_ad_drst/drdx/drco` -- COVERED (AD_DRST section)
30. `mhitm_ad_drin` -- COVERED (AD_DRIN section)
31. `mhitm_ad_drli` -- COVERED (AD_DRLI section)
32. `mhitm_ad_dren` -- COVERED (AD_DREN section)
33. `mhitm_ad_stun` -- COVERED (AD_STUN section)
34. `mhitm_ad_slow` -- COVERED (AD_SLOW section)
35. `mhitm_ad_plys` -- COVERED (AD_PLYS section)
36. `mhitm_ad_slee` -- COVERED (AD_SLEE section)
37. `mhitm_ad_conf` -- COVERED (AD_CONF section)
38. `mhitm_ad_blnd` -- COVERED (AD_BLND section)
39. `mhitm_ad_ston` -- COVERED (AD_STON section)
40. `mhitm_ad_stck` -- COVERED (AD_STCK section)
41. `mhitm_ad_wrap` -- COVERED (AD_WRAP section)
42. `mhitm_ad_sgld` -- COVERED (AD_SGLD section)
43. `mhitm_ad_sedu` -- COVERED (AD_SEDU section)
44. `mhitm_ad_ssex` -- COVERED (AD_SSEX section)
45. `mhitm_ad_tlpt` -- COVERED (AD_TLPT section)
46. `mhitm_ad_rust/corr/dcay` -- COVERED
47. `mhitm_ad_ench` -- COVERED (AD_ENCH section)
48. `mhitm_ad_slim` -- COVERED (AD_SLIM section)
49. `mhitm_ad_poly` -- COVERED (AD_POLY section)
50. `mhitm_ad_were` -- COVERED (AD_WERE section)
51. `mhitm_ad_heal` -- COVERED (AD_HEAL section)
52. `mhitm_ad_deth` -- COVERED (AD_DETH section)
53. `mhitm_ad_pest` -- COVERED (AD_PEST section)
54. `mhitm_ad_famn` -- COVERED (AD_FAMN section)
55. `mhitm_ad_dgst` -- COVERED (AD_DGST section)
56. `mhitm_ad_halu` -- COVERED (AD_HALU section)
57. `mhitm_ad_samu` -- COVERED (AD_SAMU section)
58. `mhitm_ad_dise` -- COVERED (AD_DISE section)
59. `mhitm_ad_legs` -- COVERED (AD_LEGS section)
60. `mhitm_knockback` -- mentioned (section 4.3)
61. `do_stone_u` -- COVERED (AD_STON section)

Coverage: 51/61 relevant functions covered = ~84%.

### C2. Formula Spot-Checks (3)

**Formula 1: Monster hit determination (mhitu.c:707-716)**

Spec says:
```
tmp = AC_VALUE(u.uac) + 10 + mtmp.m_lev
    + (multi < 0 ? 4 : 0)
    - (player_invisible_to_monster ? 2 : 0)
    - (mtmp.mtrapped ? 2 : 0)
if tmp <= 0: tmp = 1
```

Source (lines 707-716):
```c
tmp = AC_VALUE(u.uac) + 10;
tmp += mtmp->m_lev;
if (gm.multi < 0) tmp += 4;
if ((Invis && !perceives(mdat)) || !mtmp->mcansee) tmp -= 2;
if (mtmp->mtrapped) tmp -= 2;
if (tmp <= 0) tmp = 1;
```

VERDICT: MATCH. The spec's "player_invisible_to_monster" correctly captures `(Invis && !perceives(mdat)) || !mtmp->mcansee`. All constants correct.

**Note**: The spec says "怪物看不见玩家 -2" which could be misread as only blindness, but the pseudocode above correctly shows the full condition. PASS.

**Formula 2: MC negation probability (mhitu.c:1084-1131, uhitm.c:75-99)**

Spec says:
```
armpro = magic_negation(mdef)   // 0..3
negated = !(rn2(10) >= 3 * armpro)
// armpro=0 -> 0%, armpro=1 -> 30%, armpro=2 -> 60%, armpro=3 -> 90%
```

Source (uhitm.c:87):
```c
negated = !(rn2(10) >= 3 * armpro);
```

Verification of probabilities:
- armpro=0: `!(rn2(10) >= 0)` = `!(TRUE)` = FALSE always -> 0% negation. CORRECT.
- armpro=1: `!(rn2(10) >= 3)` = TRUE when rn2(10) in {0,1,2} -> 3/10 = 30%. CORRECT.
- armpro=2: `!(rn2(10) >= 6)` = TRUE when rn2(10) in {0..5} -> 6/10 = 60%. CORRECT.
- armpro=3: `!(rn2(10) >= 9)` = TRUE when rn2(10) in {0..8} -> 9/10 = 90%. CORRECT.

magic_negation() formula also verified (mhitu.c:1084-1131):
- mc = max of armor a_can values
- extrinsic Protection: mc += (Amulet_of_Guarding ? 2 : 1), capped at 3
- if no extrinsic and mc < 1: intrinsic Protection sets mc = 1

VERDICT: EXACT MATCH.

**Formula 3: Digestion timer (mhitu.c:1376-1390)**

Spec says:
```
if AD_DGST:
    tim_tmp = (ACURR(A_CON) + 10 - u.uac + rn2(20)) / mtmp.m_lev + 3
elif other:
    tim_tmp = rnd(mtmp.m_lev + 10 / 2)
    // [疑似 bug: 运算优先级问题, 10/2=5, 实际是 rnd(m_lev + 5)]
u.uswldtim = max(tim_tmp, 2)
```

Source (lines 1376-1390):
```c
if (mattk->adtyp == AD_DGST) {
    tim_tmp = (int)ACURR(A_CON) + 10 - (int)u.uac + rn2(20);
    if (tim_tmp < 0) tim_tmp = 0;
    tim_tmp /= (int) mtmp->m_lev;
    tim_tmp += 3;
} else {
    tim_tmp = rnd((int) mtmp->m_lev + 10 / 2);
}
u.uswldtim = (unsigned) ((tim_tmp < 2) ? 2 : tim_tmp);
```

Discrepancy found: The spec's AD_DGST formula writes `(CON + 10 - uac + rn2(20)) / m_lev + 3` as a single expression, but the source has a `tim_tmp < 0` clamp to 0 before dividing. **The spec omits the `if (tim_tmp < 0) tim_tmp = 0` guard** (line 1381-1382). This could matter when u.uac is very high (very bad AC).

The non-DGST formula analysis is correct: `10 / 2` is computed first due to C operator precedence, yielding 5, so it's `rnd(m_lev + 5)`.

VERDICT: MINOR DISCREPANCY. Missing `< 0` clamp in AD_DGST formula. The priority bug annotation is correct.

### C3. Missing Mechanics

**CRITICAL**: None.

**MINOR**:
1. **Steed attack probability incomplete**: Section 2.1 says "兽人有 50% 概率攻击坐骑代替玩家" but omits that ALL monsters have a 25% chance (`!rn2(4)`) when riding -- orcs just have it elevated to 50% (`!rn2(2)`). The `m_next2u(mtmp)` adjacency check is also omitted.

2. **AD_DGST digestion timer negative clamp**: As noted in C2, the `if (tim_tmp < 0) tim_tmp = 0` guard before the division is missing from the spec formula.

3. **AT_KICK in pit**: Source line 799 shows `if (mattk->aatyp == AT_KICK && mtrapped_in_pit(mtmp)) continue;` -- monsters trapped in pits cannot kick. Not mentioned.

4. **mhitu_dieroll save**: The spec mentions this in note 4 but doesn't explain that it's specifically saved via `gm.mhitu_dieroll = rnd(20 + i)` at line 909, only for AT_WEAP attacks. Other attack types don't save the dieroll this way.

5. **Medusa gaze in mattacku**: Source line 833 shows `if (mdat != &mons[PM_MEDUSA]) sum[i] = gazemu(mtmp, mattk)` -- Medusa's gaze is handled separately through `m_respond` in `dochug()`, not through `mattacku`. The spec doesn't note this exception explicitly.

6. **`failed_grab` check**: Several attack types (AT_HUGS, AT_ENGL, and general melee in 3.7) now check `failed_grab(mtmp, &youmonst, mattk)` for unsolid targets. This newer mechanic is not mentioned.

### C4. Test Vector Verification

**TV-1 Verification** (Basic hit):
```
u.uac=0, m_lev=5, multi>=0, visible, not trapped, i=0
tmp = AC_VALUE(0) + 10 + 5 = 0 + 10 + 5 = 15
Hit: tmp > rnd(20) -> 15 > rnd(20)
rnd(20) returns 1..14: hit (14 values out of 20 = 70%)
rnd(20) returns 15: near miss (tmp == j)
rnd(20) returns 16..20: miss
```
Source lines 707-708, 804: `tmp = AC_VALUE(u.uac) + 10; tmp += mtmp->m_lev;` then `if (tmp > (j = rnd(20 + i)))`.
For i=0, rnd(20+0)=rnd(20). Result: 14/20 = 70% hit rate.
PASS.

**TV-6 Verification** (MC 3 negation):
```
armpro=3
negated = !(rn2(10) >= 9)
rn2(10) returns 0..8: negated=TRUE (9/10 = 90%)
rn2(10) returns 9: negated=FALSE (1/10 = 10%)
```
Source line 87: `negated = !(rn2(10) >= 3 * armpro)` with armpro=3 -> `!(rn2(10) >= 9)`.
PASS.

**TV-12 Verification** (Digestion timer):
```
CON=18, u.uac=-5, rn2(20)=10, m_lev=15
tim_tmp = 18 + 10 - (-5) + 10 = 43
tim_tmp < 0? No.
tim_tmp /= 15 -> 43/15 = 2 (C integer division)
tim_tmp += 3 -> 5
u.uswldtim = max(5, 2) = 5
```
Source lines 1380-1384:
```c
tim_tmp = (int)ACURR(A_CON) + 10 - (int)u.uac + rn2(20);
// = 18 + 10 - (-5) + 10 = 43
if (tim_tmp < 0) tim_tmp = 0; // 43 >= 0, no change
tim_tmp /= (int) mtmp->m_lev; // 43 / 15 = 2
tim_tmp += 3; // 5
```
`u.uswldtim = (unsigned)((5 < 2) ? 2 : 5) = 5`.
PASS.

## D. Recommendations

1. **Fix steed attack probability**: Change section 2.1 item 5 to: "骑乘中, 所有怪物有 25% 概率 (`!rn2(4)`) 攻击坐骑代替玩家; 兽人提升至 50% (`!rn2(2)`)." Also add the `m_next2u(mtmp)` adjacency requirement.

2. **Fix struct attack notation in section 1**: Replace the C-like `struct attack { ... }` with plain pseudocode description or a table format to fully comply with A3.

3. **Add digestion timer negative clamp**: In section 7.1, add `if tim_tmp < 0: tim_tmp = 0` before the division step in the AD_DGST formula.

4. **Add Medusa gaze exception note**: In section 11 or section 2.5, note that Medusa's gaze is handled via `m_respond` in `dochug()`, not in `mattacku`'s gaze loop.

5. **Minor: AD_DRLI probability**: The spec says `!rn2(3)` for 1/3 probability which matches source. However, the spec's `&& !negated` should be clarified as using `mhitm_mgc_atk_negated()` check, not the inline `!negated` pattern.

6. **Consider adding failed_grab mechanic**: The `failed_grab()` check for unsolid defender targets is a 3.7 addition that affects AT_HUGS, AT_ENGL, and standard melee attacks. Worth a brief note in section 2.

7. **Test vectors are solid**: All 12 verified test vectors produce correct results. The 4 boundary conditions meet the minimum requirement. Consider adding a boundary test for the hit formula `tmp <= 0` clamp to 1 (e.g., very low level monster vs very negative AC player with invisibility and trapped attacker).
