# Review: scroll-effects.md

**Reviewer**: Claude Opus 4.6 (1M context)
**Date**: 2026-03-14
**Source verified against**: `src/read.c` (3357 lines, $NHDT-Date: 2025/11/07)
**Spec version reviewed**: current HEAD

---

## Tier A: Format Compliance

| Check | Status | Notes |
|-------|--------|-------|
| [A1] >= 10 test vectors | PASS | 42 individual test cases across sections 9.1-9.7 |
| [A2] >= 2 boundary conditions | PASS | Section 9.5 has 8 boundary tests (#25-#32) |
| [A3] No C code blocks | PASS | All code blocks are pseudocode |
| [A4] No Rust code | PASS | No Rust code present |
| [A5] Pseudocode present | PASS | Pseudocode provided for enchant armor formula (sec 3.1.1), identify (sec 3.3.1), scare monster pickup (sec 3.7.3), fire damage (sec 3.12), etc. |
| [A6] Exact formulas | PASS | Formulas with explicit constants throughout; enchant armor, fire damage, stinking cloud, charging, etc. all have exact formulas |
| [A7] [疑似 bug] markers | PASS | 4 markers: identify cursed-known (#3.3), scare monster pickup reset (#3.7.4), magic mapping cursed+confused (#3.11.2), destroy armor confused+cursed (#3.17.2) |

**Tier A Score**: 7/7

---

## Tier B: Content Coverage

### Keywords from v2 spec section 15.2:

| Keyword | Covered? | Section | Notes |
|---------|----------|---------|-------|
| 卷轴效果 (scroll effects) | YES | Sections 3.1-3.23 | All 23 scroll types covered individually |
| 混淆阅读 (confused reading) | YES | Section 6 | Complete summary table of all confused effects |

**Tier B Score**: 2/2

---

## Tier C: Source Code Accuracy

### [C1] Function Coverage

Functions in `src/read.c` relevant to scroll effects:

| Function | Covered in Spec | Notes |
|----------|----------------|-------|
| `doread()` | YES | Sections 2.1-2.6 (prerequisites, literacy, blindness) |
| `seffect_enchant_armor()` | YES | Section 3.1 |
| `seffect_enchant_weapon()` | YES | Section 3.2 |
| `seffect_identify()` | YES | Section 3.3 |
| `seffect_remove_curse()` | PARTIAL | Section 3.4 -- see error below |
| `seffect_teleportation()` | YES | Section 3.5 |
| `seffect_genocide()` | YES | Section 3.6 |
| `seffect_scare_monster()` | YES | Section 3.7 |
| `seffect_confuse_monster()` | YES | Section 3.8 |
| `seffect_light()` | YES | Section 3.9 |
| `seffect_charging()` | YES | Section 3.10 |
| `seffect_magic_mapping()` | YES | Section 3.11 |
| `seffect_fire()` | YES | Section 3.12 |
| `seffect_earth()` | YES | Section 3.13 |
| `seffect_punishment()` | YES | Section 3.14 |
| `seffect_taming()` | YES | Section 3.15 |
| `seffect_create_monster()` | YES | Section 3.16 |
| `seffect_destroy_armor()` | YES | Section 3.17 |
| `seffect_amnesia()` | YES | Section 3.18 |
| `seffect_gold_detection()` | YES | Section 3.19 |
| `seffect_food_detection()` | YES | Section 3.20 |
| `seffect_stinking_cloud()` | YES | Section 3.21 |
| `seffect_blank_paper()` | YES | Section 3.22 |
| `seffect_mail()` | YES | Section 3.23 |
| `recharge()` | YES | Section 3.10 (wand/ring/tool details) |
| `punish()` | YES | Section 3.14 |
| `do_genocide()` | YES | Section 3.6 |
| `do_class_genocide()` | YES | Section 3.6.1 |
| `forget()` | YES | Section 3.18 |
| `maybe_tame()` | YES | Section 3.15 |
| `some_armor()` | YES | Section 7 |
| `litroom()` | PARTIAL | Section 3.9 -- radius values referenced but `litroom()` details not fully expanded |
| `chwepon()` | YES | Section 3.2.1 (referenced as external, logic documented) |

**Coverage: ~95%** -- all 23 scroll effect functions are covered. Minor gaps in helper functions (`litroom` internals, `gold_detect`/`trap_detect`/`food_detect` internals are deferred to their own files).

### [C2] Formula Verification (3 most important)

**Formula 1: Enchant Armor evaporation check (sec 3.1.1)**

Spec says:
```
s = spe (if cursed scroll: -spe)
if s > (special_armor ? 5 : 3) and rn2(s) != 0:
    armor evaporates
```

Source (read.c:1177-1188):
```c
s = scursed ? -otmp->spe : otmp->spe;
if (s > (special_armor ? 5 : 3) && rn2(s)) {
    // evaporates
```

**MATCH** -- formula is correct.

**Formula 2: Identify item count (sec 3.3.1)**

Spec says:
```
cval = 1
if blessed or (!cursed and rn2(5) == 0):
    cval = rn2(5)
    if cval == 1 and blessed and Luck > 0:
        cval = 2
```

Source (read.c:2032-2038):
```c
int cval = 1;
if (sblessed || (!scursed && !rn2(5))) {
    cval = rn2(5);
    if (cval == 1 && sblessed && Luck > 0)
        ++cval;
}
```

**MATCH** -- formula is correct.

**Formula 3: Fire scroll damage (sec 3.12.1)**

Spec says:
```
cval = bcsign(sobj)
dam = (2 * (rn1(3, 3) + 2 * cval) + 1) / 3
```

Source (read.c:1810-1811):
```c
cval = bcsign(sobj);
dam = (2 * (rn1(3, 3) + 2 * cval) + 1) / 3;
```

**MATCH** -- formula is exact.

### [C3] Missing Mechanics

**CRITICAL**:

1. **SCR_REMOVE_CURSE unpunish scope is wrong**: Section 3.4.1 states "Cursed: ...the `nodisappear` flag is set... Also: if Punished and not confused, removes punishment (`unpunish()`)." This implies unpunish only happens with cursed scrolls. However, in the source code (read.c:1547), `unpunish()` is called outside the `if (scursed) { ... } else { ... }` block:
   ```c
   if (Punished && !confused)
       unpunish();
   ```
   This means **all BUC states** (blessed, uncursed, AND cursed) remove punishment when not confused. The spec's placement of this under section 3.4.1 (cursed scroll) is misleading. The same applies to the buried ball freedom check (read.c:1549-1552). This is CRITICAL because it misrepresents which scroll states can free you from punishment.

**MINOR**:

2. **Scare monster read effect scope**: Section 3.7.1 says "For each visible monster on the level" but the source (read.c:1415) actually checks `cansee(mtmp->mx, mtmp->my)` which is "can see the monster's position" -- this is slightly different from "visible monster" since a cansee check tests line-of-sight to the square, not monster visibility per se.

3. **Confused remove curse saddle handling**: The spec mentions saddle in section 3.4.2 but does not explicitly note that the saddle is handled separately from inventory with its own confused/uncurse logic (read.c:1529-1545).

4. **Punishment initial weight for non-cursed**: Section 3.14 mentions the cursed scroll adds `WT_IRON_BALL_INCR * 2` extra weight, but the code uses `WT_IRON_BALL_INCR * (1 + cursed_levy)` -- for uncursed, `cursed_levy=0`, so extra weight = `WT_IRON_BALL_INCR * 1`. The spec says "standard punishment" for uncursed but doesn't fully describe the initial ball weight formula (which is in `mkobj` for BALL_CLASS, not in `punish()`).

### [C4] Test Vector Verification

**TV #5 (Identify, Cursed, Not Known)**: "Self-identify only, learn scroll type"
- Source (read.c:2021-2028): `if (confused || (scursed && !already_known))` -> self-identify only. `if (!already_known) learnscrolltyp(SCR_IDENTIFY)`. Then returns.
- **CORRECT**.

**TV #20 (Scare Monster, blessed spe=0, pickup)**: "Becomes uncursed, spe=0"
- Source pickup.c scare monster logic: blessed -> `unbless(obj)`, no spe change.
- **CORRECT**.

**TV #31 (Fire blessed, rn1(3,3)=5)**: "dam=(2*(5+2)+1)/3=5, *5=25"
- Calculation: rn1(3,3)=5, cval=+1, inner = 5 + 2*1 = 7, dam = (2*7+1)/3 = 15/3 = 5, *5 = 25.
- **CORRECT**.

---

## Summary

| Category | Score | Notes |
|----------|-------|-------|
| Tier A (Format) | 7/7 | Fully compliant |
| Tier B (Content) | 2/2 | Both keywords covered |
| Tier C (Accuracy) | Good | ~95% function coverage, 1 CRITICAL error (remove_curse unpunish scope), 3 MINOR issues |

### Required Changes

1. **CRITICAL**: Fix section 3.4 (SCR_REMOVE_CURSE) to move the unpunish/buried-ball logic out of section 3.4.1 (cursed) into a new section "3.4.5 Punishment Removal (all BUC, not confused)" or equivalent. The current placement falsely implies only a cursed scroll removes punishment. All BUC states remove punishment when not confused.

### Suggested Improvements

2. **MINOR**: In section 3.7.1, change "visible monster" to "monster at a position the hero can see (cansee check)" for precision.

3. **MINOR**: Add a note about the saddle's separate confused/uncurse handling in section 3.4.4 (confused reading).

4. **MINOR**: Section 3.14 could benefit from noting that `cursed_levy` is 0 for uncursed scrolls, making the "already punished" weight increase formula `WT_IRON_BALL_INCR * 1` for uncursed vs `WT_IRON_BALL_INCR * 2` for cursed.
