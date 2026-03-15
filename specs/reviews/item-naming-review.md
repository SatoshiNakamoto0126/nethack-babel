# Review: item-naming.md

**Reviewer**: Claude Opus 4.6 (1M context)
**Date**: 2026-03-14
**Spec file**: `/Users/hz/Downloads/nethack-babel/specs/item-naming.md`
**Source files verified**: `src/objnam.c` (full file, 5738 lines), `src/o_init.c`

---

## Tier A: Format Compliance

| Check | Status | Notes |
|-------|--------|-------|
| A1: >= 10 test vectors | PASS | 39 test vectors across sections 12.1--12.10 |
| A2: >= 2 boundary conditions | PASS | TV #2 (known=T charged -> suppress uncursed for weapon), TV #3 (known=F -> show uncursed), TV #5 (ARMOR_CLASS forces uncursed), TV #6 (RING_CLASS forces uncursed), TV #9 ("eucalyptus" a-exception), TV #10 ("unicorn" a-exception), TV #11 ("+0" non-vowel), TV #24 (CRYSKNIFE erosion skip), TV #32 (artifact not fully identified) |
| A3: No C code blocks | PASS | One C macro (GemStone in SS10.2) appears with `#define` syntax. This is a **borderline case**: it's documenting a macro definition, not procedural C code. Acceptable since it's data-definitional. |
| A4: No Rust code | PASS | No Rust code present. |
| A5: Pseudocode present | PASS | Extensive pseudocode in SS2.1, SS3.2, SS5.1, SS6.1, etc. |
| A6: Exact formulas | PASS | Exact rules for a/an selection, uncursed display, erosion word selection, plural rules. |
| A7: [疑似 bug] markers | PASS | Markers at: SS2.2 SCROLL_CLASS oc_magic distinction, SS7 #39 RING called (self-corrected), SS10.8 I18N assembly order limitation. |

**A3 note**: The GemStone macro at SS10.2 uses C `#define` syntax. Strictly speaking this is a C code block, but it's a constant definition rather than procedural code, and the spec needs to document its exact semantics. **Recommend keeping but converting to pseudocode format** for full compliance.

---

## Tier B: Key Content Coverage (v2 spec SS15.3)

| Key Content | Covered? | Section | Notes |
|-------------|----------|---------|-------|
| 全名拼接 | YES | SS2, SS3 | xname() and doname() fully documented with prefix/suffix ordering. |
| BUC 显示规则 | YES | SS3.2, SS3.3, SS3.4 | Complete uncursed display logic with implicit_uncursed interaction. |
| 侵蚀描述 | YES | SS6 | add_erosion_words() fully documented with material-dependent word selection. |
| 材质与外观 | YES | SS4 | Shuffle classes, description priority (nn > un > dn). |
| 复数化 | YES | SS8 | makeplural() fully documented with as_is, one_off, badman lists. |
| 冠词 (a/an/the) | YES | SS5 | just_an(), the(), I18N skip all covered. |
| 知识标志位 | YES | SS1.1 | All 8 knowledge flags documented. |
| 命名命令 | YES | SS7 | #name command, ONAME vs oc_uname, artifact creation by naming. |

**Tier B verdict**: All key content areas from v2 spec SS15.3 are covered comprehensively.

---

## Tier C: Detailed Verification

### C1: Function Coverage

| C function | Covered? | Accuracy |
|------------|----------|----------|
| `xname()` / `xname_flags()` | YES | Correct. Full flow including role substitution, I18N, dknown setting, class-specific formatting. |
| `doname()` / `doname_base()` | YES | Mostly correct; see issues below regarding prefix ordering. |
| `just_an()` | YES | Correct. All special cases (vowels, eu/uke/unicorn/uranium/useful, single char, x+consonant, the/lava/bars/ice). |
| `the()` | YES | Correct. CapitalMon, fruit, space/hyphen, "of" detection, Platinum Yendorian Express Card special case. |
| `add_erosion_words()` | YES | Correct. All material-dependent words, crysknife special case, erodeproof words. |
| `makeplural()` | YES | Correct. Compound splitting, pronoun mapping, as_is, one_off, all suffix rules. |
| `makesingular()` | YES (implied) | Mentioned in SS8.7 for I18N skip. |
| `corpse_xname()` | YES | Correct. Possessive for unique, "the" for unique non-pname, article for normal. |
| `obj_is_pname()` | YES | Correct (Appendix B). |
| `the_unique_obj()` | YES | Correct (Appendix C). |
| `armor_simple_name()` | YES | Correct. All armcat cases documented. |
| `xcalled()` | YES (implicit) | Used throughout xname; format "{pfx} called {sfx}" correctly described. |
| `observe_object()` | YES | Appendix D. |
| `erosion_matters()` | YES | SS6.7 correct. |
| `not_fully_identified()` | YES (implicit) | Referenced via obj_is_pname() in Appendix B. |
| `singplur_compound()` | YES | SS8.1 step 3 lists all compound separators. |
| `badman()` | YES | SS8.5 lists both pluralize and singularize exclusion prefixes. |
| `ch_ksound()` | YES | SS8.6 lists all k-sound words. |

### C2: Formula Spot-Checks (3)

**Spot-check 1: Uncursed display logic (SS3.3 vs objnam.c:1342-1361)**

Spec says uncursed is displayed when:
```
(a) flags.implicit_uncursed == false
OR
(b) ALL of:
    - (!known || !oc_charged || oclass == ARMOR_CLASS || oclass == RING_CLASS)
    - otyp != SCR_MAIL
    - otyp != FAKE_AMULET_OF_YENDOR
    - otyp != AMULET_OF_YENDOR
    - !Role_if(PM_CLERIC)
```

Source (objnam.c:1342-1361):
```
else if (!flags.implicit_uncursed
    || ((!known || !objects[obj->otyp].oc_charged
          || obj->oclass == ARMOR_CLASS
          || obj->oclass == RING_CLASS)
         && obj->otyp != SCR_MAIL
         && obj->otyp != FAKE_AMULET_OF_YENDOR
         && obj->otyp != AMULET_OF_YENDOR
         && !Role_if(PM_CLERIC)))
```

**Verdict: MATCH.** The spec correctly captures the OR / AND structure.

**Spot-check 2: RING_CLASS xname "called" behavior (SS2.2 vs objnam.c:918-927)**

Spec says for RING_CLASS:
```
!dknown -> "ring"
nn      -> "ring of {actualn}"
un      -> xcalled("ring", un)     // "ring called {un}" -- no dn
else    -> "{dn} ring"
```

Source (objnam.c:918-927):
```
case RING_CLASS:
    if (!dknown)
        Strcpy(buf, "ring");
    else if (nn)
        Sprintf(buf, "ring of %s", actualn);
    else if (un)
        xcalled(buf, BUFSZ - PREFIX, "ring", un);
    else
        Sprintf(buf, "%s ring", dn);
```

**Verdict: MATCH.** The spec correctly identifies that `dn` is NOT included when `un` is set for RING_CLASS (unlike WEAPON/TOOL which pass `dn` as the prefix to xcalled). The [疑似 bug] analysis at TV #39 is well-reasoned.

**Spot-check 3: POTION_CLASS holy/unholy water (SS2.2 vs objnam.c:846-866)**

Spec says:
```
if nn:
    buf += " of "
    if POT_WATER && bknown && (blessed || cursed):
        buf += blessed ? "holy " : "unholy "
    buf += actualn
```

Source (objnam.c:853-859):
```
if (nn) {
    Strcat(buf, " of ");
    if (typ == POT_WATER && bknown
        && (obj->blessed || obj->cursed)) {
        Strcat(buf, obj->blessed ? "holy " : "unholy ");
    }
    Strcat(buf, actualn);
}
```

**Verdict: MATCH.** The spec's holy water table in SS3.4 is also correct.

### C3: Missing Mechanics

1. **[MINOR] doname_base tknown condition**: The spec at SS3.2 section E says `if Is_box && otrapped && tknown && dknown`. The source (objnam.c:1370) confirms this requires `obj->dknown` in addition to `obj->tknown`. The spec correctly includes `dknown` here. **No issue.**

2. **[MINOR] Wizard mode gender display**: objnam.c:1563-1573 adds gender information `(male)/(female)/(neuter)/(unspecified gender)` for statues/corpses/figurines in wizard mode with `iflags.wizmgender`. Not mentioned in the spec. **Acceptable omission** -- wizard mode debugging feature.

3. **[MINOR] `cknown` for STATUE**: objnam.c:1328 includes `obj->otyp == STATUE` alongside `Is_container(obj)` for the "empty" prefix. The spec's SS3.2 section C says `(Is_container || STATUE) && !Has_contents` which is correct.

4. **[MINOR] Armor donning/doffing states**: objnam.c:1407-1410 shows `doffing(obj)` vs `donning(obj)` vs `"being worn"` and also `uskin` embedding. The spec covers this at SS3.2 section H under ARMOR_CLASS. **Adequately covered.**

5. **[MINOR] `for_menu` flag in doname_base**: objnam.c:1244 accepts a `DONAME_FOR_MENU` flag that affects bounds checking. Not mentioned in spec. **Acceptable omission** -- internal buffer management detail.

6. **[MINOR] `vague_quan` for "some" prefix**: objnam.c:1298-1301 shows that when `!dknown && vague_quan`, prefix becomes "some " instead of the count. The spec's SS3.2 section B shows `"some "` only as an alternative in the flow but doesn't explain the `vague_quan` flag trigger. However, `doname_vague_quan` is listed in SS11 function table. **Marginally covered.**

### C4: Test Vector Verification (3)

**TV #36 verification: Container prefix ordering**

Spec says:
```
LARGE_BOX, cknown=T, empty, lknown=T, olocked=T, bknown=T, uncursed, implicit_uncursed=F
Output: "an uncursed locked empty large box"
```

Source analysis (objnam.c:1296-1385, prefix accumulation order):
1. Article: `"a "` (line 1312)
2. Empty: `Strcat(prefix, "empty ")` (line 1330) -> prefix = `"a empty "`
3. BUC: `Strcat(prefix, "uncursed ")` (line 1362) -> prefix = `"a empty uncursed "`
4. Trapped: skipped (not trapped in this test)
5. Locked: `Strcat(prefix, "locked ")` (line 1379) -> prefix = `"a empty uncursed locked "`
6. Greased: skipped
7. just_an correction: checks `"empty "` (first word after "a ") -> 'e' is vowel -> `"an "`. Final prefix = `"an empty uncursed locked "`

**Correct output: `"an empty uncursed locked large box"`**

**Verdict: SPEC ERROR.** The spec has the wrong prefix order. It says `"an uncursed locked empty large box"` but the actual code produces `"an empty uncursed locked large box"`. The "empty" prefix is added BEFORE BUC and lock status in the code.

**TV #2 verification: Rusty long sword with implicit_uncursed**

Spec says:
```
long sword, quan=3, known=T, spe=+0, bknown=T, uncursed, dknown=T, nn=T, oeroded=1, IRON, implicit_uncursed=T
Output: "3 +0 rusty long swords"
```

Source analysis:
- quan=3: prefix = `"3 "` (line 1299)
- bknown=T, uncursed, implicit_uncursed=T: Check uncursed display: `!flags.implicit_uncursed` is false. Inner condition: `(!known || !oc_charged || ARMOR || RING)` = `(!T || !T || F || F)` = false. So uncursed is NOT displayed. **Correct.**
- WEAPON_CLASS: `add_erosion_words(obj, prefix)` -> oeroded=1, IRON -> `Strcat(prefix, "rusty ")` -> prefix = `"3 rusty "`
- known=T: `Sprintf(eos(prefix), "%+d ", 0)` -> prefix = `"3 rusty +0 "`
- xname returns "long sword", pluralized to "long swords"
- Final: `"3 rusty +0 long swords"`

Wait -- the spec says `"3 +0 rusty long swords"` but the actual code order is: erosion words are added BEFORE the enchantment. Let me re-read the code.

objnam.c:1432-1438:
```
case WEAPON_CLASS:
    if (ispoisoned)
        Strcat(prefix, "poisoned ");
    add_erosion_words(obj, prefix);
    if (known) {
        Sprintf(eos(prefix), "%+d ", obj->spe);
    }
```

So erosion words come BEFORE the +N enchantment in the prefix. The prefix accumulates as:
1. `"3 "` (quantity)
2. Then in WEAPON_CLASS: erosion -> `"3 rusty "`, then +N -> `"3 rusty +0 "`

So the actual output is `"3 rusty +0 long swords"`, NOT `"3 +0 rusty long swords"` as the spec claims.

Wait, but the spec's SS3.1 ordering diagram shows:
```
[erosion_words]  [erodeproof]  [+N enchantment]
```

This is correct! So the spec's ordering diagram is right but TV #2's expected output is wrong. Let me double-check...

Actually, looking again at TV #2's output: `"3 +0 rusty long swords"`. If the spec's own SS3.1 says erosion comes before enchantment, then TV #2 contradicts the spec's own documentation. Let me verify once more against the code.

objnam.c:1435: `add_erosion_words(obj, prefix)` -- adds "rusty " to prefix
objnam.c:1437: `Sprintf(eos(prefix), "%+d ", obj->spe)` -- adds "+0 " to prefix

So prefix is definitely `"3 rusty +0 "`. The output is `"3 rusty +0 long swords"`.

**Verdict: SPEC ERROR in TV #2.** The expected output should be `"3 rusty +0 long swords"`, not `"3 +0 rusty long swords"`.

**TV #31 verification: Fully identified Excalibur**

Spec says:
```
LONG_SWORD, oartifact=EXCALIBUR, known=T, spe=+5, bknown=T, blessed=T, rknown=T, oerodeproof=1, fully_identified
Output: "the blessed rustproof +5 Excalibur"
```

Source analysis:
- `obj_is_pname` returns true (fully identified artifact with oname)
- xname skips to `nameit:` label, writes ONAME = "Excalibur"
- doname prefix: `the_unique_obj` or `obj_is_pname` -> prefix = `"the "`
- BUC: blessed -> `"the blessed "`
- ARMOR falls through to WEAPON_CLASS: erosion words: oerodeproof=1, rknown=T, is_rustprone (IRON) -> `"rustproof "` -> prefix = `"the blessed rustproof "`
- known=T: `"+5 "` -> prefix = `"the blessed rustproof +5 "`
- But wait: xname returned "Excalibur" (via pname goto). bp = "Excalibur".
- Final: `"the blessed rustproof +5 Excalibur"`

**Verdict: CORRECT.**

---

## Issues Found

### Accuracy Errors

1. **[ERROR] TV #36: Wrong prefix ordering for container.** The spec says `"an uncursed locked empty large box"` but the correct output is `"an empty uncursed locked large box"`. In doname_base(), the "empty" prefix is added (line 1330) BEFORE the BUC prefix (lines 1338-1362) and BEFORE the lock status prefix (lines 1372-1381). The spec's own SS3.1 ordering diagram correctly shows `[empty]` before `[BUC]` before `[locked/unlocked/broken]`, so the diagram is right but the test vector contradicts it.

2. **[ERROR] TV #2: Wrong prefix ordering for erosion+enchantment.** The spec says `"3 +0 rusty long swords"` but the correct output is `"3 rusty +0 long swords"`. In doname_base(), `add_erosion_words()` is called (line 1435) BEFORE the enchantment is appended (line 1437). The spec's own SS3.1 ordering diagram correctly shows `[erosion_words] [erodeproof] [+N enchantment]`, so again the diagram is right but the test vector contradicts it.

3. **[MINOR] SS3.3 references `show_uncursed()` function.** The spec's pseudocode at SS3.2 section D uses `show_uncursed()` as a function call, but the actual code in objnam.c inlines this logic directly (lines 1342-1361). There is no `show_uncursed()` function. This is a naming convention issue only; the logic is correctly documented.

### Cosmetic / Clarity Issues

4. **[MINOR] SS10.2 GemStone macro in C syntax.** Uses `#define` C syntax. Recommend converting to pseudocode for A3 compliance. Example:
   ```
   GemStone(typ) = (typ == FLINT) OR (material == GEMSTONE AND typ NOT IN {DILITHIUM_CRYSTAL, RUBY, ...})
   ```

5. **[MINOR] TV #39 self-correction.** The spec initially gives a wrong output for TV #39 (`"a jade ring called teleport"`) then self-corrects inline to `"a ring called teleport"`. The self-correction is accurate, but a clean final answer without the erroneous first attempt would be clearer. The accompanying analysis is valuable and should be preserved as a note.

6. **[MINOR] SS3.2 doname_base flow omits `for_menu` flag.** The `DONAME_FOR_MENU` flag is not mentioned. This only affects bounds-checking behavior, not output format, so omission is acceptable.

### Missing Content

7. **[MINOR] `mshot_xname()` not documented.** This function (objnam.c:1103-1116) prefixes "the Nth " for multishot missiles (e.g., "the 2nd arrow"). Not included in SS11 function table or elsewhere.

8. **[MINOR] Shield undknown simplification.** The spec at SS2.2 ARMOR_CLASS correctly documents the `!dknown` shield simplification (ELVEN_SHIELD..ORCISH_SHIELD -> "shield", SHIELD_OF_REFLECTION -> "smooth shield"). This matches objnam.c:742-748. **No issue, just confirming.**

9. **[MINOR] `paydoname()` and `shk_names_obj()` not documented.** These are shop-specific formatting variants. Acceptable omission for scope.

---

## Test Vector Status Summary

| TV # | Status | Issue |
|------|--------|-------|
| 1 | CORRECT | |
| 2 | **ERROR** | Output should be `"3 rusty +0 long swords"` (erosion before enchantment) |
| 3 | CORRECT | |
| 4 | CORRECT | |
| 5 | CORRECT | |
| 6 | CORRECT | |
| 7 | CORRECT | |
| 8 | CORRECT | |
| 9 | CORRECT | |
| 10 | CORRECT | |
| 11 | CORRECT | |
| 12-20 | CORRECT | All plural rules verified against makeplural() source |
| 21 | CORRECT | |
| 22 | CORRECT | |
| 23 | CORRECT | Erosion+erodeproof can coexist; comment in source confirms (objnam.c:1194-1196) |
| 24 | CORRECT | CRYSKNIFE skips erosion, shows "fixed" |
| 25 | CORRECT | |
| 26 | CORRECT | |
| 27 | CORRECT | |
| 28 | CORRECT | |
| 29 | CORRECT | |
| 30 | CORRECT | |
| 31 | CORRECT | |
| 32 | CORRECT | |
| 33 | CORRECT | |
| 34 | CORRECT | |
| 35 | CORRECT | |
| 36 | **ERROR** | Output should be `"an empty uncursed locked large box"` (empty before BUC before locked) |
| 37 | CORRECT | |
| 38 | CORRECT | |
| 39 | CORRECTED | Spec self-corrects inline; final answer is right |

---

## Summary

| Category | Score |
|----------|-------|
| Format compliance (Tier A) | 6.5/7 (A3 borderline with GemStone C macro) |
| Key content coverage (Tier B) | All areas covered |
| Function coverage (C1) | 18/18 key functions covered |
| Formula accuracy (C2) | 3/3 spot-checks MATCH |
| Missing mechanics (C3) | 3 minor omissions |
| Test vector accuracy (C4) | 1/3 CORRECT, 2/3 found errors (TV #2, TV #36 ordering) |

**Overall assessment**: HIGH QUALITY with two test vector errors. The specification is comprehensive, well-structured, and demonstrates deep understanding of the objnam.c naming pipeline. The pseudocode flows accurately capture the branching logic. The uncursed display rules, erosion word selection, and pluralization mechanics are all correctly documented. The two test vector errors (TV #2 and TV #36) involve prefix ordering -- ironically, the spec's own ordering diagram (SS3.1) is CORRECT, but two test vectors contradict it. These are likely transcription errors rather than misunderstandings of the logic.

**Recommendation**: APPROVE after fixing:
1. TV #2: Change expected output to `"3 rusty +0 long swords"`
2. TV #36: Change expected output to `"an empty uncursed locked large box"`
3. Optional: Convert GemStone macro from C to pseudocode format
4. Optional: Clean up TV #39 self-correction to present final answer clearly
