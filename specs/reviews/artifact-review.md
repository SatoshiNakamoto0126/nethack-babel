# Review: artifact.md

**Reviewer**: Claude Opus 4.6
**Date**: 2026-03-14
**Spec file**: `/Users/hz/Downloads/nethack-babel/specs/artifact.md`
**C sources verified**: `src/artifact.c` (full file, 2850 lines), `include/artilist.h` (full file)

---

## Tier A: Format Compliance

| Check | Status | Notes |
|-------|--------|-------|
| A1: >=10 test vectors | PASS | 14 test vectors (TV-1 through TV-14) |
| A2: >=2 boundary conditions | PASS | TV-9 (wish with N=3), TV-10 (wish with N=2), TV-11 (Excalibur level boundary), TV-12 (Magicbane spe=3 scare_dieroll boundary), TV-13 (DRLI level-0 boundary), TV-14 (gift probability first time) |
| A3: No C code blocks | PASS | All code-fenced blocks use pseudocode notation |
| A4: No Rust code | PASS | No Rust code present |
| A5: Pseudocode present | PASS | Pseudocode for spec_abon, spec_dbon, spec_applies, touch_artifact, mk_artifact, hack_artifacts, restrict_name, DRLI mechanics, Magicbane effects, Vorpal/Tsurugi, invoke cooldown |
| A6: Exact formulas | PASS | All combat formulas, probability distributions, and cooldown mechanics given precisely |
| A7: [suspicious bug] markers | PASS | One marker: TV-13 documents level-0 DRLI edge case with detailed analysis |

**Tier A verdict**: PASS (7/7)

---

## Tier B: Key Content Coverage (v2 spec S15.3)

v2 spec S15.3 for artifacts covers: artifact effects, alignment checks, invoke.

| Topic | Covered | Section |
|-------|---------|---------|
| Artifact definition structure | Yes | S1.1 |
| SPFX flag bits | Yes | S1.2 |
| Complete artifact table | Yes | S1.3 |
| Attack bonus formulas (spec_abon/spec_dbon) | Yes | S2.1, S2.2 |
| spec_applies matching logic | Yes | S2.3 |
| Per-artifact attack details | Yes | S2.4 |
| Level drain (DRLI) mechanics | Yes | S2.5 |
| Cleaver cleave mechanics | Yes | S2.6 |
| Magicbane special effects | Yes | S2.7 |
| Vorpal Blade beheading | Yes | S2.8 |
| Tsurugi bisection | Yes | S2.9 |
| Defense attributes (defn/cary/spfx/cspfx) | Yes | S3 |
| Invoke abilities | Yes | S4 |
| Touch/blast damage | Yes | S5 |
| Stormbringer bloodthirst | Yes | S6 |
| Quest artifact assignments | Yes | S7 |
| Wishing for artifacts | Yes | S8 |
| Altar gift system | Yes | S9 |
| Excalibur fountain | Yes | S10 |
| Naming rules | Yes | S11 |
| Destruction resistance | Yes | S12 |
| Mjollnir throwing | Yes | S13 |

**Tier B verdict**: PASS -- all key content areas thoroughly covered.

---

## Tier C: Deep Verification

### C1: Function Coverage

| Function | Covered | Location |
|----------|---------|----------|
| `hack_artifacts` | Yes | S10.4 |
| `mk_artifact` | Yes | S9.2 |
| `artifact_name` | Yes | S11.2 (fuzzy matching) |
| `exist_artifact` | Yes | S8 |
| `restrict_name` | Yes | S11.1 |
| `spec_abon` | Yes | S2.1 |
| `spec_dbon` | Yes | S2.2 |
| `spec_applies` | Yes | S2.3 |
| `artifact_hit` | Yes | S2.4--S2.9 |
| `Mb_hit` | Yes | S2.7 |
| `touch_artifact` | Yes | S5 |
| `arti_immune` | Yes | S3.6 |
| `bane_applies` | Yes | S5.1 (bane check in touch) |
| `set_artifact_intrinsic` | Yes | S3.3, S3.4 |
| `arti_invoke` | Yes | S4 |
| `arti_invoke_cost` | Yes | S4.1 |
| `arti_invoke_cost_pw` | Yes | S4.1 |
| `invoke_healing` | Yes | S4.2 |
| `invoke_energy_boost` | Yes | S4.2 |
| `invoke_charge_obj` | Yes | S4.2 |
| `invoke_create_portal` | Yes | S4.2 |
| `invoke_create_ammo` | Yes | S4.2 |
| `invoke_banish` | Yes | S4.2 |
| `invoke_fling_poison` | Yes | S4.2 |
| `invoke_storm_spell` | Yes | S4.2 |
| `invoke_blinding_ray` | Yes | S4.2 |
| `artifact_light` | Yes | S3.5 |
| `retouch_object` | Yes | S12.3 |
| `retouch_equipment` | Yes | S12.3 |
| `is_magic_key` | Partial | MKoT magic key conditions mentioned in S2 but full blessed/cursed/rogue logic not detailed |
| `permapoisoned` | Yes | S2.4 (Grimtooth permanently poisoned) |
| `nartifact_exist` | Yes | S8, S9 |

**Coverage**: Excellent -- all significant artifact.c functions are addressed.

### C2: Formula Spot-Checks (3)

**Spot-Check 1: spec_dbon with damd=0 (Frost Brand example, S2.2 vs artifact.c:1113-1114)**

Spec says: "When `damd == 0` and `spec_dbon_applies`, return `max(tmp, 1)` where `tmp` is the base weapon damage. This effectively doubles damage."

Source (artifact.c:1113-1114):
```c
if (gs.spec_dbon_applies)
    return weap->attk.damd ? rnd((int) weap->attk.damd) : max(tmp, 1);
```

**MATCH**: Correct. The `tmp` parameter is the already-calculated base damage passed in from `artifact_hit` (artifact.c:1477: `*dmgptr += spec_dbon(otmp, mdef, *dmgptr)`).

**Spot-Check 2: Magicbane scare_dieroll formula (S2.7 vs artifact.c:1269-1274)**

Spec says:
```
scare_dieroll = MB_MAX_DIEROLL / 2 = 4
IF spe >= 3: scare_dieroll /= 2^(spe/3)
```

Source (artifact.c:1269-1274):
```c
int attack_indx, fakeidx, scare_dieroll = MB_MAX_DIEROLL / 2;
...
if (mb->spe >= 3)
    scare_dieroll /= (1 << (mb->spe / 3));
```

**MATCH**: `1 << (mb->spe / 3)` is exactly `2^(spe/3)` (integer division). Correct.

**Spot-Check 3: Touch artifact blast damage (S5.2 vs artifact.c:960)**

Spec says:
```
dmg = d(Antimagic ? 2 : 4, self_willed ? 10 : 4)
```

Source (artifact.c:960):
```c
dmg = d((Antimagic ? 2 : 4), (self_willed ? 10 : 4));
```

**MATCH**: Exact match.

Silver damage:
Spec: "IF 物品材质是银 AND 玩家厌恶银: tmp = rnd(10), dmg += Maybe_Half_Phys(tmp)"

Source (artifact.c:962-963):
```c
if (objects[obj->otyp].oc_material == SILVER && Hate_silver)
    tmp = rnd(10), dmg += Maybe_Half_Phys(tmp);
```

**MATCH**: Exact match.

### C3: Missing Mechanics

1. **SPFX_REFLECT for Longbow of Diana in wielded-properties table (S3.3)**: The spec section 3.3 lists SPFX_REFLECT artifacts as only "Dragonbane". However, Longbow of Diana also has SPFX_REFLECT in its spfx field (artilist.h line 272). The code at artifact.c:874 gates SPFX_REFLECT behind `wp_mask & W_WEP`, so it only works when wielded. This is correct behavior for Dragonbane but also applies to Longbow of Diana when wielded as a melee weapon. **MEDIUM priority** -- should be listed.

2. **Banish: behavior when already in Gehennom**: The spec mentions banishing demons to Gehennom but doesn't describe the in-Gehennom branch. Source (artifact.c:2000-2007): when `!Inhell`, demons are migrated to a random Gehennom level; when `Inhell`, they are simply teleported locally via `u_teleport_mon()`. **LOW priority** -- the spec focuses on the typical case.

3. **MKoT `is_magic_key` blessing/curse conditions**: The spec mentions MKoT's trap-finding and lock-picking bonus in item-use.md but the artifact spec doesn't detail the exact blessed/cursed conditions for the Key to function as a "magic key" (artifact.c:2781-2793: Rogues need non-cursed; non-Rogues need blessed). **LOW priority** -- partially covered.

4. **Artifact speaking (arti_speak)**: The spec mentions SPFX_SPEAK in S1.2 ("能说话") but doesn't detail the `arti_speak()` function (artifact.c:2285-2303): speaking artifacts whisper rumors from the rumors file, with BUC status affecting rumor truthfulness. **LOW priority** -- flavor mechanic.

5. **Artifact discovery list**: `discover_artifact()` / `undiscovered_artifact()` / `disp_artifact_discoveries()` tracking not mentioned. **VERY LOW priority** -- UI/display concern.

### C4: Test Vector Verification (3)

**TV-4: Frost Brand vs no-cold-resist, base damage = 7**

Spec: spec_applies = TRUE (no cold resist), spec_dbon = damd==0 -> max(7, 1) = 7. Effectively doubles.

Source logic: `spec_dbon(otmp, mdef, 7)` with `weap->attk.damd == 0` and `gs.spec_dbon_applies = TRUE` -> returns `max(7, 1) = 7`.

**VERIFIED**: Correct.

**TV-9: Wish artifact with N=3 (boundary)**

Spec: `rn2(3) > 1` -> {0,1,2}, only 2 > 1 -> 1/3 failure, 2/3 success.

Source (objnam.c, referenced): `rn2(nartifact_exist()) > 1` where nartifact_exist includes the newly created artifact.

**VERIFIED**: Correct boundary analysis.

**TV-12: Magicbane spe=3, dieroll=2**

Spec: `scare_dieroll = 4 / 2^(3/3) = 4 / 2 = 2`. `dieroll(2) <= scare_dieroll(2)` -> enters scare. `dieroll(2) <= scare_dieroll/2 = 1`? No -> no cancel.

Source: `scare_dieroll = 4`, `scare_dieroll /= (1 << (3/3)) = (1 << 1) = 2`, so scare_dieroll = 2. `dieroll=2 <= 2` -> MB_INDEX_SCARE. `dieroll=2 <= 2/2=1`? No -> not MB_INDEX_CANCEL.

**VERIFIED**: Correct.

---

## Issues Summary

| # | Severity | Section | Description |
|---|----------|---------|-------------|
| 1 | **ERROR** | S4.2 (BANISH) | Spec says "boss(dlord+2概率, dprince+1概率)" but source has them reversed: `is_dprince` -> `chance += 2`, `is_dlord` -> `chance += 1` (artifact.c:1993-1996). Fix: swap to "dprince+2概率, dlord+1概率". |
| 2 | MINOR | S3.3 | SPFX_REFLECT table lists only "Dragonbane" but Longbow of Diana also has SPFX_REFLECT in spfx (artilist.h:272). Add "Longbow of Diana" to the SPFX_REFLECT row. |
| 3 | MINOR | S2.7 | Magicbane confusion is described as "1/12 概率" -- correct (`!rn2(12)` at artifact.c:1407), but the spec says "额外 1/12 概率造成混乱" without specifying the exact confusion duration. Source: player gets `(HConfusion & TIMEOUT) + 4L` turns (artifact.c:1410), not +3 as for stun. Add "+4 turns confusion" for precision. |
| 4 | MINOR | S1.3 | Excalibur's spfx column shows "NOGEN,RESTR,SEEK,DEFN,INTEL,SEARCH" -- this is correct per artilist.h, but SEEK and SEARCH are separate flags (0x10 and 0x200). The spec correctly notes SEEK is unused but listing both might confuse implementers. Consider annotating SEEK as `SEEK(unused)` in the table. |
| 5 | INFO | S4.2 (BANISH) | Missing detail: when already in Gehennom (`Inhell`), demons are teleported locally rather than migrated. Spec only describes the non-Gehennom case. |
| 6 | INFO | S3.3 | SPFX_PROTECT table says "Tsurugi of Muramasa, Mitre of Holiness" -- this is correct for spfx. But note that `protects()` at artifact.c:714 also checks `cspfx & SPFX_PROTECT`, which no artifact currently uses. No action needed, just documenting. |
| 7 | NITPICK | TV-13 | The note says "攻击者不回血 (drain=0 因为 mhpmax <= m_lev 条件限制)" -- more precisely, the m_lev==0 branch sets `*dmgptr = 2*mhp+200` and skips the drain/heal calculation entirely. The drain variable isn't even used in this path; the healing happens only after the else branch (artifact.c:1689: `if (drain > 0)`). The `drain` from the earlier calculation is still non-zero but the code jumps to the fatal path before healing. Clarify that the fatal branch bypasses healing entirely. |

---

## Overall Assessment

**PASS** -- Excellent and thorough spec. The artifact table is complete and accurate against artilist.h. All major artifact.c functions are covered with correct pseudocode. The only actual error is Issue 1 (Banish dlord/dprince probability swap), which is a straightforward transposition. The remaining issues are minor precision improvements.

Recommended actions:
1. **Must fix**: Issue 1 (Banish dlord/dprince probability reversal)
2. **Should fix**: Issue 2 (Longbow REFLECT omission), Issue 3 (Magicbane confusion duration)
3. **Nice to have**: Issues 4-7 (annotation improvements)
