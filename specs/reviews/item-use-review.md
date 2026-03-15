# Review: item-use.md

**Reviewer**: Claude Opus 4.6
**Date**: 2026-03-14
**Spec file**: `/Users/hz/Downloads/nethack-babel/specs/item-use.md`
**C sources verified**: `src/apply.c`, `src/lock.c` (referenced), `src/music.c` (referenced), `src/write.c` (referenced), `src/detect.c` (referenced)

---

## Tier A: Format Compliance

| Check | Status | Notes |
|-------|--------|-------|
| A1: >=10 test vectors | PASS | 15 test vectors (TV1--TV15) |
| A2: >=2 boundary conditions | PASS | Multiple boundary TVs: TV9 (INT=20 never backfires), TV11 (exact minimum ink), TV13 (DEX+level=25 never charms), TV14 (break probability over 50 turns) |
| A3: No C code blocks | PASS | All formulas use pseudocode-style blocks, not compilable C |
| A4: No Rust code | PASS | No Rust code present |
| A5: Pseudocode present | PASS | Formulas throughout use pseudocode notation |
| A6: Exact formulas | PASS | Lock pick chances, crystal ball backfire, marker ink, unicorn horn distributions all given with exact formulas |
| A7: [suspicious bug] markers | PASS | Two markers: one for cream pie ECMD_OK (section 16), one for wooden flute charm threshold (TV13) |

**Tier A verdict**: PASS (7/7)

---

## Tier B: Key Content Coverage (v2 spec S15.3)

v2 spec S15.3 covers: tools, instruments, containers.

| Topic | Covered | Section |
|-------|---------|---------|
| Tool dispatch (doapply) | Yes | S1 |
| Lock picking / forcing | Yes | S2 |
| Musical instruments | Yes | S3 |
| Camera | Yes | S4 |
| Mirror | Yes | S5 |
| Lamps / lanterns / candles | Yes | S6 |
| Crystal ball | Yes | S7 |
| Magic marker | Yes | S8 |
| Unicorn horn | Yes | S9 |
| Figurines | Yes | S10 |
| Tinning kit | Yes | S11 |
| Leash | Yes | S12 |
| Saddle | Yes | S13 |
| Stethoscope | Yes | S14 |
| Bullwhip | Yes | S15 |
| Containers | Yes | S20 |
| Wand breaking | Yes | S24 |
| Polearms | Yes | S22 |
| Grappling hook | Yes | S23 |

**Tier B verdict**: PASS -- comprehensive coverage of all key item-use mechanics.

---

## Tier C: Deep Verification

### C1: Function Coverage

| Function | Covered | Notes |
|----------|---------|-------|
| `doapply` | Yes | Dispatch table in S1 |
| `use_camera` | Yes | S4 |
| `use_towel` | Yes | S18 |
| `use_stethoscope` | Yes | S14 |
| `use_whistle` | Yes | S25.1 |
| `use_magic_whistle` | Yes | S25.2 |
| `use_mirror` | Yes | S5 |
| `use_bell` | Yes | S26 |
| `use_candelabrum` | Yes | S6.4 |
| `use_candle` | Yes | S6.5 |
| `use_lamp` | Yes | S6.1 |
| `light_cocktail` | Yes | S6.6 |
| `use_cream_pie` | Yes | S16 |
| `use_royal_jelly` | Yes | S27 |
| `use_grease` | Yes | S17 |
| `use_figurine` | Yes | S10 |
| `use_whip` | Yes | S15 |
| `use_grapple` | Yes | S23 |
| `use_tinning_kit` | Yes | S11 |
| `use_leash` | Yes | S12 |
| `use_stone` | Yes | S19 |
| `use_trap` | Yes | S21 |
| `do_break_wand` | Yes | S24 |
| `flip_through_book` | Yes | S33 |
| `flip_coin` | Yes | S34 |
| `use_unicorn_horn` | Yes | S9 |
| `do_play_instrument` | Yes | S3 |
| `bagotricks` | Yes | S30 |
| `hornoplenty` | Yes | S29 |
| `pick_lock` | Yes | S2.1 |
| `doforce`/`forcelock` | Yes | S2.2 |
| `use_crystal_ball` | Yes (in detect.c) | S7 |
| `dowrite` | Yes (in write.c) | S8 |

**Coverage**: Excellent. All major `doapply` dispatch targets are covered.

### C2: Formula Spot-Checks (3)

**Spot-Check 1: Camera cursed behavior (S4 vs `use_camera` at apply.c:94)**

Spec says: "Cursed camera: 50% chance (`!rn2(2)`) of blinding self via `zapyourself()`"

Source (apply.c:94):
```
if (obj->cursed && !rn2(2)) {
    (void) zapyourself(obj, TRUE);
}
```

**MATCH**: Correct.

Spec also says: "Self-target (dx=0, dy=0): blinds self via `zapyourself()`"

Source (apply.c:102-104):
```
} else if (!u.dx && !u.dy) {
    (void) zapyourself(obj, TRUE);
}
```

**MATCH**: Correct.

**Spot-Check 2: Cream pie return value (S16 vs `use_cream_pie` at apply.c:3564-3598)**

Spec says: "Returns `ECMD_OK` (does **not** cost a turn)"

Source (apply.c:3598): `return ECMD_OK;`

**MATCH**: Correct. The spec correctly identifies this as a [suspicious bug] and notes it may be intentional.

**Spot-Check 3: Wand breaking damage formula (S24 vs `do_break_wand` at apply.c:3967)**

Spec says:
- `dmg = spe * 4`
- WAN_DEATH/WAN_LIGHTNING: `dmg * 4`, EXPL_MAGICAL
- WAN_FIRE: `dmg * 2`, EXPL_FIERY

Source:
- apply.c:3967: `dmg = obj->spe * 4;`
- apply.c:3992: `broken_wand_explode(obj, dmg * 4, EXPL_MAGICAL);` (DEATH/LIGHTNING)
- apply.c:3995: `broken_wand_explode(obj, dmg * 2, EXPL_FIERY);` (FIRE)

**MATCH**: All correct.

### C3: Missing Mechanics

1. **Towel wet/dry mechanic**: The spec mentions normal towel use but does not discuss `is_wet_towel()` / `dry_a_towel()` mechanics. Using a wet towel calls `dry_a_towel(obj, -1, drying_feedback)` (visible at apply.c:131-132, 158-159, 170-171, 185-186). The wetness affects the towel's age field and is relevant for blindfolding. **LOW priority** -- this is secondary behavior.

2. **Stethoscope Healer detection detail**: The spec says Healers can detect revivers ("mostly dead") but doesn't note the exact mechanic: `obj_has_timer(corpse, REVIVE_MON)` check (apply.c:270). **VERY LOW priority** -- the spec captures the user-facing behavior correctly.

3. **Banana Easter egg**: Not mentioned in the spec. At apply.c:4395-4399, applying a banana while hallucinating produces "It rings! ... But no-one answers." **VERY LOW priority** -- purely cosmetic.

4. **Flip through book -- MAX_SPELL_STUDY cap**: The spec says `spestudied` maps 0-4+ to fadeness levels but doesn't mention `MAX_SPELL_STUDY` cap used at apply.c:4511 (`int findx = min(obj->spestudied, MAX_SPELL_STUDY)`). **LOW priority** -- the spec captures the intent (4+ = "barely visible").

### C4: Test Vector Verification (3)

**TV3: Skeleton Key on Cursed Container, DEX 15**

Spec: `chance = (75 + 15) / 2 = 45` -> 45% per-turn.

Formula from S2.1: skeleton key on container = `75 + ACURR(A_DEX)` = 90. Cursed container modifier: `chance /= 2` = 45.

**VERIFIED**: Correct.

**TV11: Magic Marker Ink -- Writing SCR_GENOCIDE (boundary)**

Spec: pen.spe=15, basecost=30, min check = 30/2 = 15. pen.spe=15 >= 15 passes. actualcost = rn1(15,15) = [15,29]. Since pen.spe=15 and actualcost >= 15 always, pen always dries out.

**VERIFIED**: Correct boundary analysis. When actualcost == pen.spe exactly, `pen.spe < actualcost` is false, but the spec says "if pen.spe < actualcost, marker dries out". So when actualcost = 15 and pen.spe = 15, it would NOT dry out (15 < 15 is false), and pen.spe becomes 0. The spec's text at TV11 says "If actualcost = 15: pen.spe = 0, dries out mid-write! Scroll disappears." This is slightly misleading -- pen.spe becomes 0 via subtraction, not via the drying-out check. The scroll would still be created successfully. **MINOR INACCURACY** in TV11 analysis.

**TV14: Force Lock -- Blade Break Probability**

Spec: `rn2(1000 - 0) > (992 - 0*10)` = `rn2(1000) > 992` = values {993..999} = 7 values. P = 7/1000 = 0.7%.

Source (referenced from lock.c): This matches the documented formula `rn2(1000 - spe) > (992 - greatest_erosion * 10)`.

**VERIFIED**: Correct. (Note the spec earlier at S2.2 says "P per attempt = 8/1000 = 0.8%" then at TV14 refines to 7/1000 = 0.7%. The discrepancy is because {993..999} is 7 values, not 8. The S2.2 text "8/1000" is the incorrect one.)

---

## Issues Summary

| # | Severity | Section | Description |
|---|----------|---------|-------------|
| 1 | MINOR | S2.2 | Break probability stated as "8/1000 = 0.8%" but correct value is 7/1000 = 0.7% (TV14 gets it right). rn2(1000) > 992 yields {993..999} = 7 values. Fix S2.2 text to say 7/1000 |
| 2 | MINOR | TV11 | States "pen.spe = 0, dries out mid-write! Scroll disappears" when actualcost=15 and pen.spe=15, but `pen.spe < actualcost` is `15 < 15` = false, so the marker does NOT dry out; it succeeds with pen.spe = 15-15 = 0. TV11's output conclusion ("100% chance of drying out") is wrong -- pen.spe=15 is the exact boundary where the scroll IS successfully written (barely). |
| 3 | INFO | S18 | Missing wet towel drying mechanic (`dry_a_towel` called on use). Low priority. |
| 4 | NITPICK | S2.2 | "P(survive 50 attempts) = (0.992)^50 = ~0.67 (33% cumulative break chance)" should use 0.993^50 = ~0.703 to match the 7/1000 figure. TV14 has the correct calculation. |

---

## Overall Assessment

**PASS** -- High quality spec with comprehensive coverage of all item-use mechanics. The dispatch table is complete and verified against source. Formulas are accurate throughout with only minor discrepancies in two spots. Test vectors are excellent with good boundary coverage. The two [suspicious bug] markers are appropriately placed and correctly analyzed.

Recommended action: Fix Issues 1, 2, and 4 (all minor arithmetic corrections). Issue 2 is the most impactful as it reverses the boundary conclusion.
