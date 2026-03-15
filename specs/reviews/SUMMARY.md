# Spec Review Summary — NetHack Babel

28 specs reviewed by 11 parallel agents against v2 project spec and original C source code.

## Overall Results

| Verdict | Count | Specs |
|---------|-------|-------|
| GOOD | 24 | melee-combat, monster-attack, ranged-combat, armor-class, spellcasting, potion-effects, status-timeout, item-generation, item-naming, inventory, identification, pickup, item-use, artifact, monster-core, monster-generation, monster-ai, monster-items, pet, trap, shop, dungeon-gen, movement, experience |
| NEEDS_WORK | 3 | scroll-effects, wand-ray, religion |
| MAJOR_ISSUES | 1 | hunger |

## Critical Errors (must fix before implementation)

| # | Spec | Error | Detail |
|---|------|-------|--------|
| 1 | **hunger** | Choking survival inverted | Spec says 95% survive (19/20), actual code gives 5% survive (1/20). `!rn2(20)` = 1-in-20 chance of survival, not 19-in-20. |
| 2 | **wand-ray** | Reflection range cost | Spec claims reflection doesn't consume `range -= 2`. Source (zap.c:4937,4949) shows it IS consumed unconditionally on `zap_hit()` success. |
| 3 | **scroll-effects** | unpunish() BUC scope | Spec places `unpunish()` under "cursed scroll" only. Source (read.c:1547) shows ALL BUC states remove punishment when not confused. |
| 4 | **hunger** | Starvation boundary | TV #10 uses wrong comparison operator. Code uses strict `<`, so uhunger = -(100+10*Con) survives. |
| 5 | **artifact** | BANISH probability | dlord/dprince bonus reversed. Source: `is_dprince` -> `chance += 2`, `is_dlord` -> `chance += 1`. Spec has them swapped. |
| 6 | **religion** | TV #19 incorrect | `critically_low_hp` at ulevel=1, maxhp=30 fails to account for `hplim = 15 * ulevel` cap reducing maxhp to 15. |

## Format Compliance (A3: C Code Leaks)

| Spec | Issue |
|------|-------|
| religion | C code blocks in Sections 10.1 and 10.2 — need conversion to pseudocode |
| monster-attack | Borderline `struct attack { ... }` notation in Section 1 |
| item-naming | GemStone macro uses C `#define` syntax |

## Test Vector Errors

| Spec | TV | Error |
|------|-----|-------|
| item-naming | TV #2 | `"3 +0 rusty long swords"` should be `"3 rusty +0 long swords"` (erosion before enchantment) |
| item-naming | TV #36 | `"an uncursed locked empty large box"` should be `"an empty uncursed locked large box"` |
| item-use | TV #11 | Boundary error: actualcost=15, pen.spe=15 — `15 < 15` is false, scroll IS written |
| hunger | TV #18, #19 | Choking survival probability inverted (see Critical #1) |
| hunger | TV #10 | Starvation boundary off-by-one (see Critical #4) |
| monster-ai | TV #3 | Wrong formula branch for speed=12 MSLOW (result coincidentally correct) |
| movement | TV E5 | Self-contradictory: states "Overtaxed" then corrects to HVY_ENCUMBER inline |
| religion | TV #19 | Incorrect result due to missing hplim cap (see Critical #6) |

## Minor Formula Errors

| Spec | Section | Error |
|------|---------|-------|
| item-use | 2.2 | Blade break probability: says 8/1000=0.8%, correct is 7/1000=0.7% |
| monster-attack | — | Missing `tim_tmp < 0` clamp in digestion timer |
| monster-attack | — | Steed attack: only mentions orcs (50%), omits general 25% for all monsters |
| trap | — | `MINE_TRIGGER_WT` stated as ~40, actually 400 (WT_ELF/2 = 800/2) |
| trap | — | `domagictrap` invisibility toggle: `HInvis = HInvis ? 0 : FROMOUTSIDE`, not XOR |
| pet | 7.2 | Eating threshold: says `quality < MANFOOD`, should be `edible <= CADAVER` |
| monster-items | 3.3 | Wand of digging missing ~7 exclusion conditions from `find_defensive()` |
| shop | — | "Hawaiian shirt visible" — code checks for ANY visible shirt, not specifically Hawaiian |
| ranged-combat | 11 | Monster poison 3% instakill applies only to mon-vs-mon, not mon-vs-hero |

## Function Coverage Summary

| Spec | Coverage | Notes |
|------|----------|-------|
| monster-core | 96% (24/25) | Missing: `set_malign()` |
| monster-ai | 93% (26/28) | |
| monster-generation | 92% (23/25) | Missing: `set_mimic_sym()`, `unmakemon()` |
| hunger | 90% (27/30) | |
| potion-effects | 97% | |
| scroll-effects | 95% | |
| status-timeout | 93% | |
| trap | 98% (39/40) | |
| shop | 84% (27/32) | Missing: internal state mgmt |
| monster-attack | 84% (51/61) | |
| melee-combat | 77% (30/39) | Missing: hmon_* damage handlers |
| wand-ray | 71% | Missing: item-specific effects |
| spellcasting | 71% | Uncovered = UI/display only |
| armor-class | — | All key formulas verified exact |

## Priority Remediation List

### P0: Critical errors (fix immediately)
1. hunger.md — Fix choking survival probability (5% not 95%)
2. wand-ray.md — Fix reflection range consumption
3. scroll-effects.md — Fix unpunish() BUC scope
4. artifact.md — Fix BANISH dlord/dprince probability
5. hunger.md — Fix starvation boundary TV
6. religion.md — Fix TV #19 + remove C code blocks

### P1: Test vector corrections
7. item-naming.md — Fix TV #2 and #36 prefix ordering
8. item-use.md — Fix TV #11 boundary
9. monster-ai.md — Fix TV #3 formula branch annotation
10. movement.md — Fix TV E5 self-contradiction

### P2: Minor formula corrections
11-19. (see Minor Formula Errors table above)

### P3: Coverage improvements (optional)
- melee-combat: add hmon_* damage handler coverage
- wand-ray: add item-specific effect functions
- shop: add internal state management functions
- movement: add running modes (context.run 1-7) detail
