# Review: wand-ray.md (魔杖/射线/爆炸系统)

**Reviewer**: Claude Opus 4.6 (1M context)
**Date**: 2026-03-14
**Source verified against**: `zap.c` r1.584 (6337 lines), `spell.c` r1.185

---

## Summary

Overall a thorough and well-structured spec. Coverage of zap.c public functions is very high. The main inaccuracy is the claim that reflection does not consume `range -= 2` -- it does, for both monster and player reflection. Test vector TV-8 contains the error and its own "correction" is also wrong. The spec has 17 test vectors (well above minimum) and good pseudocode coverage.

**Verdict**: PASS with corrections required on reflection/range mechanics.

---

## A. Format Compliance

| Check | Status | Notes |
|-------|--------|-------|
| [A1] >= 10 test vectors | PASS | 17 test vectors (TV-1 through TV-17) |
| [A2] >= 2 boundary conditions | PASS | TV-4 (recharge explosion), TV-5 (wrest), TV-11 (Luck boundary), TV-16 (initial charges) |
| [A3] No C code blocks | PASS | All code blocks are pseudocode |
| [A4] No Rust code | PASS | None found |
| [A5] Pseudocode present | PASS | Sections 2.2, 2.3, 3, 7.3, 7.4, 8.2, 9.1-9.4, 10, 11.2-11.5, 12.2-12.3 |
| [A6] Exact formulas | PASS | All major damage formulas with explicit constants |
| [A7] [疑似 bug] markers | PASS | Used in Appendix C items 1-4 and TV-13 |

---

## B. Content Coverage

| Keyword (from v2 spec S15.2) | Covered | Section |
|------------------------------|---------|---------|
| 魔杖射线 | YES | Sections 1-2, 16 |
| 反射 | YES | Section 4 |
| 爆炸 | YES | Section 12 |
| 效果链 | YES | Sections 5-7, 14-15 |

All four keywords adequately covered.

---

## C. Source Code Accuracy

### C1. Function Coverage

Non-trivial public/staticfn functions in `zap.c`:

| Function | Covered | Notes |
|----------|---------|-------|
| `zaptype()` | Implicit | Used internally, explained via ZT encoding in Appendix A |
| `probe_objchain()` | No | Minor (probing UI helper) |
| `zombie_can_dig()` | No | Minor (dig helper) |
| `polyuse()` | No | Minor (polymorph item helper) |
| `create_polymon()` | No | Minor |
| `stone_to_flesh_obj()` | No | Minor (spell helper) |
| `zap_updown()` | Partial | Mentioned for dig but not fully detailed |
| `zhitu()` | YES | Section 5.3 |
| `revive_egg()` | No | Minor |
| `zap_steed()` | Partial | Section 2.2 mentions steed |
| `skiprange()` | No | Minor (bhit range skip) |
| `maybe_explode_trap()` | No | Minor |
| `zap_map()` | Partial | Mentioned in Section 1.2 |
| `bounce_dir()` | YES | Section 2.3 |
| `zap_hit()` | YES | Section 3 |
| `disintegrate_mon()` | Partial | Via death/disintegrate in Section 5 |
| `adtyp_to_prop()` | No | Minor (mapping helper) |
| `backfire()` | YES | Section 10 |
| `zap_ok()` | No | Minor (UI callback) |
| `boxlock_invent()` | No | Minor |
| `spell_hit_bonus()` | Partial | Referenced in zap_hit, not fully detailed |
| `maybe_destroy_item()` | YES | Section 15 |
| `destroyable()` | Partial | Via Section 15 |
| `spell_damage_bonus()` | YES | Section 5.2 |
| `zapyourself()` | YES | Section 17 |
| `weffects()` | YES | Via Sections 1, 6, 16 |
| `zapnodir()` | YES | Section 7 |
| `dobuzz()` | YES | Section 2 |
| `zhitm()` | YES | Section 5 |
| `zappable()` | YES | Section 8.2 |
| `dozap()` | YES | Sections 8, 10 |
| `do_enlightenment_effect()` | Partial | Via WAN_ENLIGHTENMENT |
| `burn_floor_objects()` | Partial | Via Section 14.1 |
| `buzz() / ubuzz()` | YES | Via Section 2 (dobuzz is the implementation) |
| `zap_over_floor()` | YES | Section 14 |
| `zap_dig()` | YES | Section 18 |
| `bhitm()` | YES | Section 6 |
| `bhito()` | Partial | Referenced but not fully detailed |
| `bhit()` | Partial | Mentioned in Section 1.2 |

**Coverage**: ~25 of ~35 significant functions = ~71%. Minor uncovered functions are mostly UI helpers or very narrow utility routines.

### C2. Formula Verification (3 most important)

**Formula 1: zap_hit() (Section 3)**

Spec says:
```
chance = rn2(20)
IF chance == 0: RETURN rnd(10) < ac + spell_bonus
ac = AC_VALUE(ac)
RETURN (3 - chance < ac + spell_bonus)
```

Source (`zap.c` lines 4691-4707):
```c
int chance = rn2(20);
int spell_bonus = type ? spell_hit_bonus(type) : 0;
if (!chance)
    return rnd(10) < ac + spell_bonus;
ac = AC_VALUE(ac);
return (3 - chance < ac + spell_bonus);
```

**MATCH** -- exact correspondence.

**Formula 2: zhitm() fire damage with cold vulnerability (Section 5)**

Spec says: `d(nd, 6)` base + `+7` if `resists_cold(mon)`.

Source (`zap.c` lines 4252-4263):
```c
tmp = d(nd, 6);
if (spellcaster) tmp = spell_damage_bonus(tmp);
orig_dmg = tmp;
if (resists_cold(mon)) tmp += 7;
```

**MATCH** -- correct. The `+7` cold-vulnerable bonus is exactly right.

**Formula 3: dobuzz() range and reflection range cost (Section 2.1, 4.3)**

Spec says (Section 4.3): "反射不消耗额外射程 (只有实际命中才 `range -= 2`)"
Spec says (Appendix C item 4): same claim.

Source (`zap.c` lines 4858-4937 for monsters):
```c
if (zap_hit(find_mac(mon), spell_type)) {
    if (mon_reflects(mon, ...)) {
        dx = -dx; dy = -dy;   // reflection
    } else {
        ... zhitm() ...       // actual hit
    }
    range -= 2;               // <-- line 4937: OUTSIDE the if/else!
}
```

Source (`zap.c` lines 4948-4968 for player):
```c
} else if (zap_hit((int) u.uac, 0)) {
    range -= 2;               // <-- line 4949: BEFORE Reflecting check
    if (Reflecting) {
        dx = -dx; dy = -dy;
    } else {
        zhitu(...);
    }
}
```

**MISMATCH (CRITICAL)**: `range -= 2` applies unconditionally when `zap_hit()` succeeds, whether or not the target reflects. The spec incorrectly claims reflection does not consume extra range. This affects TV-8 as well.

### C3. Missing Mechanics

**CRITICAL**:
1. **Reflection DOES consume `range -= 2`**: The spec claims it does not (Sections 4.3, Appendix C item 4, TV-8). This is factually wrong. Both monster reflection and player reflection consume 2 range. This fundamentally changes the tactical analysis of reflection.

2. **`spell_hit_bonus()` omitted**: Section 3 references `spell_bonus` in the zap_hit formula but does not provide the full `spell_hit_bonus()` function. This function (lines 3497-3533) uses skill level and DEX to compute hit bonus: Restricted/Unskilled=-4, Basic=0, Skilled=+2, Expert=+3, plus DEX modifier (-3 to +DEX-14). This is important for combat calculations.

**MINOR**:
1. **Lightning flashburn on miss/whiz-by**: The source (line 4974-4975) shows that lightning causes `flashburn(d(nd, 50), TRUE)` even when the player is NOT hit (just being in the same square). The spec (Section 5.3) mentions flashburn for hits but doesn't note it happens unconditionally for lightning.

2. **Rider resurrection mechanic**: When a disintegration breath hits a Rider, the Rider disintegrates and immediately reintegrates (lines 4873-4886), with HP restored to max. The spec doesn't mention Riders specifically.

3. **Death monster absorption**: PM_DEATH absorbs death rays and breaks the loop (lines 4888-4897). The spec mentions Death in Section 5 but doesn't detail the loop-breaking behavior.

4. **`gas_hit` deferred `zap_over_floor`**: Poison gas and fireball defer `zap_over_floor()` until after reflection check (lines 4843, 4981-4982). The spec doesn't explain this deferral pattern.

### C4. Test Vector Verification

**TV-1: Fire wand basic damage**

Spec: `nd=6, damage = d(6,6), range 6..36; cold-resistant monster: d(6,6) + 7`

Source verification (`zap.c` lines 4252-4257): `tmp = d(nd, 6); ... if (resists_cold(mon)) tmp += 7;`

Also verified from `weffects()` (line 3454): `WAN_MAGIC_MISSILE ? 2 : 6` confirms nd=6 for WAN_FIRE.

**PASS** -- correct.

**TV-8: Reflection range**

Spec claims: "range -= 2 (不会! 反射时只消耗正常的 range--, 不额外 -2)"

Source: `range -= 2` at line 4937 is OUTSIDE the if/else branches, applying to BOTH reflection and actual hit for monsters. For player, `range -= 2` at line 4949 is BEFORE the Reflecting check.

**FAIL** -- the spec is wrong. `range -= 2` always applies when `zap_hit()` succeeds, regardless of reflection.

**TV-4: Recharge explosion probability**

Spec: `n=0: never explodes; n=7: 343/343 = always; WAN_WISHING n>0: always`

This requires checking `recharge()` which is in `zap.c`. Let me verify the formula is in this file.

The spec correctly describes the `n^3 > rn2(343)` formula. For n=0 the condition `n > 0` is false so no explosion. For n=7: `343 > rn2(343)` is always true. For WAN_WISHING with n>0: the `||` short-circuits.

**PASS** -- correct.

---

## D. Specific Issues

### D1. CRITICAL: Reflection range consumption (Sections 4.3, Appendix C #4, TV-8)

The spec states in three places that reflection does not consume `range -= 2`. This is wrong. The source code clearly shows `range -= 2` is executed for any successful `zap_hit()`, including when the target reflects. This should be corrected in:
- Section 4.3 bullet "反射不消耗额外射程"
- Appendix C item 4
- TV-8 (the "correction" note is itself incorrect)

### D2. MINOR: Missing `spell_hit_bonus()` details

Section 3 uses `spell_bonus` without providing the full formula. The function applies skill-based bonus (-4 to +3) and DEX-based modifier (-3 to +DEX-14). This is relevant for understanding zap accuracy.

### D3. MINOR: Lightning flashburn applies even on miss

Source line 4974-4975 shows `flashburn()` is called after the hit/miss/reflect block, unconditionally for lightning. The spec's Section 5.3 only mentions it for hits. A player dodging a lightning bolt can still be blinded.

### D4. Cosmetic: TV-8 self-contradictory

TV-8 contains an initial claim, then a "[修正: ...]" that tries to correct it but is still wrong. The whole test vector needs rewriting with the correct behavior: `range -= 2` applies on any successful `zap_hit()`.
