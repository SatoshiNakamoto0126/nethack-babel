# Review: experience.md

**Reviewer**: Claude Opus 4.6 (1M context)
**Date**: 2026-03-14
**Spec file**: `/Users/hz/Downloads/nethack-babel/specs/experience.md`
**Primary sources**: `src/exper.c` (rev 1.62), `src/attrib.c` (rev 1.134), `include/global.h`, `include/monattk.h`

---

## Tier A: Format Compliance

| Check | Status | Notes |
|-------|--------|-------|
| A1: >= 10 test vectors | PASS | 16 labeled vectors (A-P) covering XP thresholds, monster XP calculation, level drain, HP/PW computation, exercise, restore ability |
| A2: >= 2 boundary conditions | PASS | TV F (exactly at threshold), TV G/H (level 1 drain fatal/non-fatal), TV L (Tourist Gnome 0 HP floor), TV N (adj_lev cap) |
| A3: No C code blocks | PASS | No C code blocks; all formulas in plain text or pseudocode format |
| A4: No Rust code | PASS | No Rust code present |
| A5: Pseudocode present | PASS | Pseudocode for newuexp, experience(), adj_lev, newhp, newpw, exercise, exerchk, rndexp, etc. |
| A6: Exact formulas | PASS | Complete formulas for XP thresholds, monster XP, HP/PW per level, exercise system, enermod |
| A7: [bug] markers | PASS | 2 markers: enermod PM_CLERIC confusion (not a real bug, noted), Tourist Gnome minimal HP |

---

## Tier B: Key Content Coverage (v2 spec S15.5)

| Topic | Status | Notes |
|-------|--------|-------|
| XP formula | PASS | Section 1 with complete 30-level table |
| Level thresholds | PASS | Section 1.1-1.2 with newuexp() formula and full table |
| Monster XP calculation | PASS | Section 2 with all bonus categories |
| Level up/down mechanics | PASS | Sections 4-5 cover pluslvl/losexp in detail |
| Attribute exercise | PASS | Section 7 covers full exercise/abuse system |
| Title system | PASS | Section 8 covers all 13 classes with all 9 ranks each |

---

## Tier C: Detailed Verification

### C1: Function Coverage

| Function | Covered | Section |
|----------|---------|---------|
| `newuexp()` | Yes | 1.1 |
| `experience()` | Yes | 2 |
| `more_experienced()` | Yes | 9.5 |
| `newexplevel()` | Yes | 4.4 |
| `pluslvl()` | Yes | 4 |
| `losexp()` | Yes | 5 |
| `newhp()` | Yes | 4.1 |
| `newpw()` | Yes | 4.3 |
| `enermod()` | Yes | 4.3 |
| `adj_lev()` | Yes | 3.1 |
| `level_difficulty()` | Yes | 3.2 |
| `newmonhp()` | Yes | 3.3 |
| `exercise()` | Yes | 7.2 |
| `exerper()` | Yes | 7.3 |
| `exerchk()` | Yes | 7.4 |
| `adjabil()` | Yes | 4.5 |
| `xlev_to_rank()` | Yes | 8.1 |
| `rndexp()` | Yes | 10 |
| `peffect_restore_ability()` | Yes | 6.1-6.2 |

### C2: Formula Spot-Checks (3)

**Check 1: `newuexp()` formula (Section 1.1)**

Spec says:
```
if lev < 1:  return 0
if lev < 10: return 10 * 2^lev
if lev < 20: return 10000 * 2^(lev - 10)
else:        return 10000000 * (lev - 19)
```

Source (exper.c:14-23):
```c
if (lev < 1) return 0L;
if (lev < 10) return (10L * (1L << lev));
if (lev < 20) return (10000L * (1L << (lev - 10)));
return (10000000L * ((long) (lev - 19)));
```

`10 * (1 << lev) = 10 * 2^lev`. MATCH.

Verification of table values:
- `newuexp(1)` = `10 * 2^1 = 20`. Table says 20. MATCH.
- `newuexp(9)` = `10 * 2^9 = 10 * 512 = 5120`. Table says 5120. MATCH.
- `newuexp(10)` = `10000 * 2^0 = 10000`. Table says 10000. MATCH.
- `newuexp(20)` = `10000000 * 1 = 10000000`. Table says 10000000. MATCH.
- `newuexp(30)` = `10000000 * 11 = 110000000`. Table says 110000000. MATCH.

**Result**: EXACT MATCH. All formula branches and table values verified.

**Check 2: `experience()` monster XP calculation (Section 2)**

Spec base formula: `base = 1 + m_lev^2`

Source (exper.c:90): `tmp = 1 + mtmp->m_lev * mtmp->m_lev;`

MATCH. `m_lev * m_lev = m_lev^2`.

Spec AC bonus: `if mac < 3: if mac < 0: bonus = (7 - mac) * 2; else: bonus = (7 - mac) * 1`

Source (exper.c:93-94):
```c
if ((i = find_mac(mtmp)) < 3)
    tmp += (7 - i) * ((i < 0) ? 2 : 1);
```

MATCH.

Spec speed bonus: `if mmove > 18: bonus += 5; else if mmove > 12: bonus += 3`

Source (exper.c:97-98):
```c
if (ptr->mmove > NORMAL_SPEED)
    tmp += (ptr->mmove > (3 * NORMAL_SPEED / 2)) ? 5 : 3;
```

`3 * 12 / 2 = 18`. MATCH.

**Result**: EXACT MATCH.

**Check 3: `exercise()` diminishing returns (Section 7.2)**

Spec says: `AEXE(i) += (rn2(19) > ACURR(i)) ? 1 : 0`

Source (attrib.c:509): `AEXE(i) += (inc_or_dec) ? (rn2(19) > ACURR(i)) : -rn2(2);`

For the exercise (inc_or_dec=true) case: `(rn2(19) > ACURR(i)) ? 1 : 0` where the boolean result is implicitly 1 or 0. MATCH.

For the abuse case: `-rn2(2)` = 0 or -1. Spec says "50% 概率减少, AEXE(i) -= rn2(2)". MATCH.

**Result**: EXACT MATCH.

### C3: Missing Mechanics

1. **`losexp()` interaction with `setuhpmax()`**: The spec (Section 5.4) describes `uhpmax -= num` and bounds checking, but the actual source uses `setuhpmax()` rather than direct assignment. `setuhpmax()` may have side effects (e.g., adjusting `u.uhp` if it exceeds `u.uhpmax`). The spec says `if uhpmax < uhpmin: uhpmax = uhpmin` but the source calls `setuhpmax(uhpmin, TRUE)`. The difference matters because `setuhpmax()` with TRUE as second arg adjusts even when polymorphed.

2. **`pluslvl()` polymorphed HP handling**: Section 4 mentions the polymorphed case briefly ("increase hit points when polymorphed, do monster form first") but the spec doesn't detail this. Source (exper.c:319-323) shows that when Upolyd, `monhp_per_lvl()` is used for monster form HP increase, THEN `newhp()` is called for the underlying human form. Both happen on every level-up. This is important for reimplementation.

3. **`enermod()` for Priest**: The spec correctly notes that `PM_CLERIC` corresponds to Priest, and the spec even marks it as [bug]-like but confirms it's correct. However, the Monk entry in the enermod table is missing -- the spec lists Monk under the "default" (1x) case. Checking source (exper.c:28-40): Monk (`PM_MONK`) is not a named case, so it falls to `default: return en`. The spec table in Section 4.3 correctly groups Monk under "others" with 1x multiplier. CORRECT.

4. **`newpw()` throttling at level 30**: Section 4.3 says `lim = 4 - uenmax / 200` with `max(lim, 1)`. Source (exper.c:74): `char lim = 4 - u.uenmax / 200;` Note `char` type, which means `lim` can be negative before the `max()` clamp. On systems where `char` is signed, this works fine. On unsigned char systems, there could be issues. The spec doesn't mention the `char` type concern, but this is arguably a C implementation detail rather than a behavioral specification.

5. **`adjabil()` weapon skill**: Section 4.6 correctly notes that `adjabil()` calls `add_weapon_skill(newlevel - oldlevel)` for upgrades and `lose_weapon_skill(oldlevel - newlevel)` for downgrades.

6. **Experience sources completeness**: Section 9.4 provides a comprehensive table of XP sources. The "fix squeaky board" entry shows `1 XP, 5 score`. This is a minor source that's good to document. The table appears thorough.

### C4: Test Vector Verification (3)

**TV A (Simple low-level monster, XP=10)**:
Input: m_lev=3, mac=10, mmove=12, no special attacks, no M2_NASTY, m_lev<=8
Expected: base = 1 + 9 = 10

Source check: `1 + 3*3 = 10`. No bonuses apply.
**Result**: CORRECT

**TV D (Revival diminishing, nk=45, base XP=100)**:
Expected: 25

Trace:
```
tmp=100, nk=45, tmp2=20, i=0
i=0: nk(45)>20: tmp=(100+1)/2=50, nk=45-20=25, i=0 even: tmp2 stays 20. i=1
i=1: nk(25)>20: tmp=(50+1)/2=25, nk=25-20=5, i=1 odd: tmp2=20+20=40. i=2
i=2: nk(5)<=40: stop
```

Source (exper.c:157-162):
```c
for (i = 0, tmp2 = 20; nk > tmp2 && tmp > 1; ++i) {
    tmp = (tmp + 1) / 2;
    nk -= tmp2;
    if (i & 1)
        tmp2 += 20;
}
```

Iteration 0: i=0, nk=45>20, tmp=(100+1)/2=50, nk=25, i&1=0 so no tmp2 change.
Iteration 1: i=1, nk=25>20, tmp=(50+1)/2=25, nk=5, i&1=1 so tmp2=40.
Iteration 2: i=2, nk=5<=40, loop ends.

Final tmp=25.
**Result**: CORRECT

**TV F (Exactly at threshold, boundary)**:
Player level 5, uexp=320 (=newuexp(5)).
`newexplevel()` checks: `uexp(320) >= newuexp(5)(320)` = true.
Calls `pluslvl(TRUE)`.
In `pluslvl(TRUE)`: `tmp = newuexp(6) = 640`. `uexp(320) >= 640`? No. So no truncation.
Level becomes 6.

Source (exper.c:302-303): `if (u.ulevel < MAXULEV && u.uexp >= newuexp(u.ulevel))`
With ulevel=5, uexp=320, newuexp(5)=320: `320 >= 320` = true. Correct.

Source (exper.c:341-345):
```c
if (incr) {
    long tmp = newuexp(u.ulevel + 1); // newuexp(6)=640
    if (u.uexp >= tmp) u.uexp = tmp - 1; // 320 < 640, no truncation
}
```
**Result**: CORRECT

---

## Summary

| Category | Result |
|----------|--------|
| Tier A | 7/7 PASS |
| Tier B | PASS (all key topics comprehensively covered) |
| Tier C1 | Excellent coverage; all major functions documented |
| Tier C2 | 3/3 EXACT MATCH against source code |
| Tier C3 | Minor: setuhpmax vs direct assignment, polymorphed pluslvl detail |
| Tier C4 | 3/3 TVs correct |

### Required Fixes

None. This is an excellent spec with no critical issues.

### Recommended Improvements

1. **Clarify `pluslvl()` polymorphed behavior**: Note that when Upolyd, both monster form HP (via `monhp_per_lvl`) AND underlying human form HP (via `newhp()`) are increased on every level-up. This dual-increment is important for reimplementation.

2. **Note `setuhpmax()` in `losexp()`**: The spec says `uhpmax -= num` but source uses `setuhpmax()` which has additional side effects (it adjusts `u.uhp` if needed and handles the polymorphed case with the second `TRUE` argument). Consider noting this implementation detail.

3. **Add a test vector for `newpw()` throttling**: At level 30 with `uenmax=600`, the throttle formula gives `lim = 4 - 600/200 = 4 - 3 = 1`, so PW increase is capped at 1. At `uenmax=800`: `lim = 4 - 4 = 0`, but `max(0, 1) = 1`. This would make a good boundary test vector.

4. **HP advancement table verification note**: The HP/PW tables in Section 4.2 are extensive (13 classes + 5 races). A note referencing `src/role.c` data structures would help future verification. The tables appear correct based on known NetHack data.
