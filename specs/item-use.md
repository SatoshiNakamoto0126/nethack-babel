# Item Use (Apply) Mechanism Spec

Source: `src/apply.c`, `src/lock.c`, `src/music.c`, `src/write.c`, `src/detect.c`, `src/steed.c`, `src/eat.c`, `src/potion.c`

---

## 1. Apply Command Dispatch (`doapply`)

The `#apply` command (`a`) prompts for an object, then dispatches by `otyp`. Preconditions:

- Hero must have hands (`!nohands(youmonst.data)`)
- Hero must not be over-capacity (`!check_capacity()`)
- `retouch_object()` is called (artifact blast check); failure costs a turn

Dispatch table (from `doapply` switch):

| otyp | Handler |
|------|---------|
| BLINDFOLD / LENSES | toggle wear |
| CREAM_PIE | `use_cream_pie` |
| LUMP_OF_ROYAL_JELLY | `use_royal_jelly` |
| BULLWHIP | `use_whip` |
| GRAPPLING_HOOK | `use_grapple` |
| LARGE_BOX / CHEST / ICE_BOX / SACK / BAG_OF_HOLDING / OILSKIN_SACK | `use_container` |
| BAG_OF_TRICKS | `bagotricks` |
| CAN_OF_GREASE | `use_grease` |
| LOCK_PICK / CREDIT_CARD / SKELETON_KEY | `pick_lock` |
| PICK_AXE / DWARVISH_MATTOCK | `use_pick_axe` |
| TINNING_KIT | `use_tinning_kit` |
| LEASH | `use_leash` |
| SADDLE | `use_saddle` |
| MAGIC_WHISTLE | `use_magic_whistle` |
| TIN_WHISTLE | `use_whistle` |
| EUCALYPTUS_LEAF | blessed: `use_magic_whistle` (1/49 chance unbless); else: `use_whistle` |
| STETHOSCOPE | `use_stethoscope` |
| MIRROR | `use_mirror` |
| BELL / BELL_OF_OPENING | `use_bell` |
| CANDELABRUM_OF_INVOCATION | `use_candelabrum` |
| WAX_CANDLE / TALLOW_CANDLE | `use_candle` |
| OIL_LAMP / MAGIC_LAMP / BRASS_LANTERN | `use_lamp` |
| POT_OIL | `light_cocktail` |
| EXPENSIVE_CAMERA | `use_camera` |
| TOWEL | `use_towel` |
| CRYSTAL_BALL | `use_crystal_ball` |
| MAGIC_MARKER | `dowrite` |
| TIN_OPENER | `use_tin_opener` |
| FIGURINE | `use_figurine` |
| UNICORN_HORN | `use_unicorn_horn` |
| Musical instruments | `do_play_instrument` |
| HORN_OF_PLENTY | `hornoplenty` |
| LAND_MINE / BEARTRAP | `use_trap` |
| Gray stones | `use_stone` |
| Polearms (`is_pole`) | `use_pole` |
| Pick-axes/axes (`is_pick` or `is_axe`) | `use_pick_axe` |
| Wands | `do_break_wand` |
| Spellbooks | `flip_through_book` |
| Coins | `flip_coin` |

---

## 2. Keys, Lock Picks, and Credit Cards

### 2.1 Lock-Picking Success Formula (`pick_lock` in `src/lock.c`)

Lock picking is a multi-turn occupation. Each turn, `rn2(100) >= chance` means "still busy." When the check passes, the lock opens.

**Chance value (per-turn success probability = `chance / 100`):**

#### For containers (boxes/chests):

| Tool | Formula |
|------|---------|
| CREDIT_CARD | `ACURR(A_DEX) + 20 * Role_if(PM_ROGUE)` |
| LOCK_PICK | `4 * ACURR(A_DEX) + 25 * Role_if(PM_ROGUE)` |
| SKELETON_KEY | `75 + ACURR(A_DEX)` |

#### For doors:

| Tool | Formula |
|------|---------|
| CREDIT_CARD | `2 * ACURR(A_DEX) + 20 * Role_if(PM_ROGUE)` |
| LOCK_PICK | `3 * ACURR(A_DEX) + 30 * Role_if(PM_ROGUE)` |
| SKELETON_KEY | `70 + ACURR(A_DEX)` |

**Modifiers:**
- If the container (not the tool) is cursed: `chance /= 2`
- Master Key of Thievery (`magic_key`): finds traps, +20 chance per discovery, auto-disarms traps
- Maximum 50 turns of picking before giving up
- Credit cards can only unlock, never lock

### 2.2 Forcing Locks (`doforce` / `forcelock` in `src/lock.c`)

Requires wielding a weapon. Valid weapons: weapon class with skill in range `[P_DAGGER..P_LANCE]` excluding `P_FLAIL`, or `ROCK_CLASS`.

**Chance for force:**
```
chance = objects[uwep->otyp].oc_wldam * 2
```

Each turn: `rn2(100) >= chance` means still busy. Max 50 turns.

**Blade forcing (edged weapon, `picktyp == 1`):**
- Each failed attempt, blade may break:
  ```
  break if: rn2(1000 - spe) > (992 - greatest_erosion * 10)
            AND !cursed AND !obj_resists(0, 99)
  ```
- For a +0 weapon with no erosion: break probability per attempt = 7/1000 = 0.7% (values {993..999})
- Over 50 attempts: survival = (993/1000)^50 = ~0.703 (29.7% cumulative break chance)

**Blunt forcing:** wakes nearby monsters each turn.

**On success with blunt weapon:** 1/3 chance (`!rn2(3)`) to totally destroy the box. Contents spill; each item has 1/3 chance to shatter (potions always shatter).

### 2.3 Trapped Containers

When successfully picking a locked, trapped container:
- Without `magic_key`: trap triggers via `chest_trap()`
- With `magic_key`: trap is detected and can be auto-disarmed (always succeeds)

---

## 3. Musical Instruments (`src/music.c`)

### 3.1 Improvisation vs. Composed Play

When applying a musical instrument:
- Drums always improvise
- Other instruments: prompted "Improvise?" unless Stunned/Confused/Hallucinating (auto-improvise)
- If answer `n`: prompted for a 5-note tune (A-G)

### 3.2 Magic Instrument Special Effects

Magic instruments require: `!Stunned && !Confused` AND `spe > 0` (charges). A charge is consumed on use.

| Instrument | Effect | Area of Effect |
|------------|--------|----------------|
| MAGIC_FLUTE | `put_monsters_to_sleep(ulevel * 5)` -- sleep `d(10,10)` turns, resisted by `sleep_monst` | distance^2 < `ulevel * 5` |
| MAGIC_HARP | `charm_monsters((ulevel-1)/3 + 1)` -- tames via `tamedog()`, shopkeepers pacified | distance^2 <= `(ulevel-1)/3 + 1` |
| DRUM_OF_EARTHQUAKE | `do_earthquake((ulevel-1)/3 + 1)` + `awaken_monsters(ROWNO * COLNO)` | rect: `force * 2` from hero; pit chance per tile: `1 / (14 - force)` |
| FIRE_HORN | Wand of fire effect. Direction required. Damage: `rn1(6,6)` = 6..11 | beam |
| FROST_HORN | Wand of cold effect. Direction required. Damage: `rn1(6,6)` = 6..11 | beam |

### 3.3 Mundane Instrument Effects

Mundane instruments (or magic instruments with no charges): special effect conditional on DEX check.

| Instrument | Special Condition | Effect |
|------------|-------------------|--------|
| WOODEN_FLUTE | `rn2(ACURR(A_DEX)) + ulevel > 25` | `charm_snakes(ulevel * 3)` -- makes snakes peaceful (not tame) |
| WOODEN_HARP | `rn2(ACURR(A_DEX)) + ulevel > 25` | `calm_nymphs(ulevel * 3)` -- makes nymphs peaceful |
| TOOLED_HORN | always | `awaken_monsters(ulevel * 30)`, scare if within 1/3 of total distance |
| BUGLE | always | `awaken_soldiers()` -- all soldiers on level become hostile, others within `ulevel * 30` |
| LEATHER_DRUM (non-mundane) | always | deafen hero `rn1(20, 30)` = 30..49 turns, `awaken_monsters(ulevel * 40)` |
| LEATHER_DRUM (mundane fallback) | always | `awaken_monsters(ulevel * 5)` |

### 3.4 Awaken/Scare Mechanics

`awaken_scare(mtmp, scary)`:
- Wakes monster, unfreezes, clears `STRAT_WAITMASK`
- If `scary` AND `!mindless` AND fails `resist(TOOL_CLASS)` AND `onscary()` permits: `monflee(0, FALSE, TRUE)`
- Scare threshold: monster distance < `total_distance / 3`

### 3.5 Castle Drawbridge Tune

When playing near Stronghold drawbridge, a 5-note tune is compared to `svt.tune`:
- Exact match: opens/closes drawbridge
- Partial match: Mastermind-style feedback
  - "tumblers" = correct note, wrong position
  - "gears" = correct note, correct position

### 3.6 Earthquake Details (`do_earthquake`)

```
force = (ulevel - 1) / 3 + 1     // capped at 13
area: x in [ux - force*2, ux + force*2]
      y in [uy - force*2, uy + force*2]
```

Per-tile pit creation: `rn2(14 - force) == 0` => probability = `1 / (14 - force)`.

Terrain effects: fountains, sinks, altars (not sanctum), graves, thrones destroyed; doors collapse; secret doors/corridors revealed; corridors and rooms get pits.

Pit damage:
- Monster falls in: `rnd(6)` damage (or `rnd(4)` if already trapped in pit)
- Hero falls in: `rnd(6)` damage (half_phys), trapped `rn1(6, 2)` = 2..7 turns
- Hero already in pit: jostled, `rnd(2)` or `rnd(4)` damage depending on footing check

---

## 4. Camera (`use_camera`)

- Cannot use underwater
- Requires direction; costs a turn regardless of outcome
- Requires charges (`spe > 0`); if 0: "nothing happens" (still costs turn)
- Consumes 1 charge
- **Cursed camera:** 50% chance (`!rn2(2)`) of blinding self via `zapyourself()`
- **Normal use:** fires `FLASHED_LIGHT` beam via `bhit()`; `flash_hits_mon()` blinds first monster hit
- **Self-target (dx=0, dy=0):** blinds self via `zapyourself()`

---

## 5. Mirror (`use_mirror`)

**Cursed mirror:** 50% chance (`!rn2(2)`) to fog up -- no effect but costs turn.

### 5.1 Self-targeting Effects

| Condition | Effect |
|-----------|--------|
| Floating eye polyform | Paralyze: `rnd(MAXULEV + 6 - ulevel)` turns; Hallucination gives 25% immunity |
| Vampire/vampshifter | "You don't have a reflection" |
| Umber hulk polyform | Confusion: `d(3,4)` turns |
| Hallucination | See random color |
| Sick | "look peaked" |
| Weak from hunger | "look undernourished" |
| Polymorphed | "look like a <monster>" |
| Normal | "look as <charisma_adj> as ever" |

### 5.2 Monster-targeting Effects

Fires `INVIS_BEAM` via `bhit()`. Monster must have eyes and hero must hit its head.

| Monster Condition | Effect |
|-------------------|--------|
| Sleeping | No effect ("too tired to look") |
| Blind (`!mcansee`) | No effect |
| Invisible mirror and mon cannot perceive invisible | No effect |
| Seen only via infravision | "too far away to see" |
| Vampire / Ghost / vampshifter | "doesn't have a reflection" |
| Medusa (not cancelled, not invisible-unperceived) | Turned to stone (killed); mon_reflects() checked first |
| Floating eye (not cancelled) | Frozen: `d(m_lev, mattk[0].damd)` turns; 25% chance (`!rn2(4)`) 120 turns instead |
| Umber hulk (not cancelled) | Self-confusion (`mconf = 1`) |
| Nymph / Amorous demon (not cancelled) | Steals the mirror, teleports away |
| Non-unicorn, non-humanoid, non-demon, not invisible | 80% chance (`rn2(5)`): frightened, flee `d(2,4)` turns |
| All others | Ignores reflection |

---

## 6. Lamps, Lanterns, and Candles

### 6.1 Lighting (`use_lamp`)

- If already lit: toggle off (`end_burn`)
- If underwater: cannot light
- If out of fuel (`age == 0` for oil lamp/lantern/candles, or `spe == 0` for magic lamp): cannot light
- **Cursed lamp failure:** 50% chance (`!rn2(2)`)
  - Oil/magic lamp: additional 1/3 chance (`!rn2(3)`) to spill oil, making hands glib (`d(2,10)` turns added)
  - Otherwise: flickers and dies (no oil spill)

### 6.2 Magic Lamp Rubbing (`dorub`)

Must be wielded first. When rubbing a magic lamp with `spe > 0`:
- **1/3 chance** (`!rn2(3)`): releases djinni
  - Lamp transforms: `otyp = OIL_LAMP`, `spe = 0`, `age = rn1(500, 1000)` (1000..1499)
  - Calls `djinni_from_bottle()`
- **Otherwise:** 50% (`rn2(2)`) "puff of smoke", 50% nothing happens

### 6.3 Djinni Outcomes (`djinni_from_bottle` in `src/potion.c`)

Base outcome: `chance = rn2(5)` (0..4)

BUC adjustment:
- **Blessed:** if `chance == 4` (hostile), reroll to `rnd(4)` (1..4); else set to 0 (wish)
- **Cursed:** if `chance == 0` (wish), reroll to `rn2(4)` (0..3); else set to 4 (hostile)

| chance value | Result |
|-------------|--------|
| 0 | Wish granted (`mongrantswish`), djinni disappears |
| 1 | Djinni becomes tame |
| 2 | Djinni becomes peaceful |
| 3 | Djinni vanishes |
| 4 | Djinni becomes hostile |

**Probability table (from source comment):**

| Outcome | Blessed | Uncursed | Cursed |
|---------|---------|----------|--------|
| Wish (0) | 80% | 20% | 5% |
| Tame (1) | 5% | 20% | 5% |
| Peaceful (2) | 5% | 20% | 5% |
| Vanishes (3) | 5% | 20% | 5% |
| Hostile (4) | 5% | 20% | 80% |

### 6.4 Candelabrum of Invocation (`use_candelabrum`)

- Holds 0..7 candles (`spe` field)
- Cursed: candles flicker and die, cannot light
- If `spe < 7`: "only N candles", lights dimly
- **At invocation position:** normal fuel consumption; 7 candles emit "strange light"
- **Not at invocation position:** fuel consumed at double rate: `age = (age + 1) / 2` (round up)

### 6.5 Candles (`use_candle`)

When applied while carrying the Candelabrum:
- Prompted to attach candles to candelabrum
- If yes: candles consumed, `candelabrum.spe += quan` (capped at 7)
- Candelabrum age takes `min(existing_age, candle_age)`

### 6.6 Potion of Oil (`light_cocktail`)

- Split 1 from stack
- Light it: dim light (radius 1)
- Lit potions can be snuffed to merge back into stack

---

## 7. Crystal Ball (`use_crystal_ball` in `src/detect.c`)

**Cannot use while blind.**

### 7.1 Backfire Check

```
oops = is_quest_artifact ? 8 : blessed ? 16 : 20
backfire if: charged AND (cursed OR rnd(oops) > ACURR(A_INT))
```

For uncursed non-artifact: backfire when `rnd(20) > INT`. With INT=18: only `rnd(20) in {19,20}` triggers, so 10% backfire. With INT=20: never (rnd(20) max is 20, not > 20).

**Impairment on backfire:** `rnd(100 - 3 * ACURR(A_INT))` turns.

Backfire effects (choose `rnd(N)` where N=4 for artifact/blessed, N=5 otherwise):

| Roll | Effect |
|------|--------|
| 1 | "Too much to comprehend" (no mechanical effect) |
| 2 | Confusion: `(HConfusion & TIMEOUT) + impair` turns |
| 3 | Blindness: `BlindedTimeout + impair` turns (resists_blnd blocks) |
| 4 | Hallucination: `(HHallucination & TIMEOUT) + impair` turns |
| 5 | **Explosion**: ball destroyed, `rnd(30)` physical damage (half_phys) -- only possible for non-blessed, non-artifact |

On backfire, one charge consumed.

### 7.2 Normal Use (Charged)

Prompted for a symbol character to search for. Consumes one charge. Paralysis: `rnd(10)` turns if charged, `rnd(2)` if uncharged.

Detection dispatch:
- Furniture symbol (identified by `def_char_is_furniture`): `furniture_detect()`
- Object class symbol: `object_detect(class)`
- Monster class symbol: `monster_detect(class)`
- Boulder custom symbol: `object_detect(ROCK_CLASS)`
- `^`: `trap_detect()`
- Other: random level name shown ("you see <place>, <distance>")
- 1% chance (`!rn2(100)`) to see Wizard of Yendor gazing back

### 7.3 Cancelled Ball

If `spe < 0` (cancelled): implodes on use. Destroyed, no damage to hero (but paralysis from gazing applies).

### 7.4 Hallucination

If hallucinating: paralysis `rnd(4)` turns charged, `rnd(2)` uncharged. Random flavor messages. If cancelled (`spe < 0`): implodes.

---

## 8. Magic Marker (`dowrite` in `src/write.c`)

### 8.1 Ink Cost (Base Cost by Scroll Type)

| Scroll | Base Cost |
|--------|-----------|
| SCR_LIGHT, SCR_GOLD_DETECTION, SCR_FOOD_DETECTION, SCR_MAGIC_MAPPING, SCR_AMNESIA, SCR_FIRE, SCR_EARTH | 8 |
| SCR_DESTROY_ARMOR, SCR_CREATE_MONSTER, SCR_PUNISHMENT | 10 |
| SCR_CONFUSE_MONSTER | 12 |
| SCR_IDENTIFY | 14 |
| SCR_ENCHANT_ARMOR, SCR_REMOVE_CURSE, SCR_ENCHANT_WEAPON, SCR_CHARGING | 16 |
| SCR_SCARE_MONSTER, SCR_STINKING_CLOUD, SCR_TAMING, SCR_TELEPORTATION | 20 |
| SCR_GENOCIDE | 30 |
| Spellbooks | `10 * oc_level` |

### 8.2 Writing Process

1. **Minimum ink check:** `pen.spe < basecost / 2` => "marker is too dry" (writing aborted)
2. **Actual cost:** `rn1(basecost/2, basecost/2)` = range `[basecost/2, basecost - 1]`
3. **Ink dries during writing:** if `pen.spe < actualcost`, marker dries out (`spe = 0`); scroll disappears, spellbook left blank with faded writing
4. Otherwise: `pen.spe -= actualcost`

### 8.3 Knowledge Requirements

Writing succeeds automatically if the target type is formally identified (`oc_name_known`).

For writing by user-assigned name or description:
- Scrolls: must have been encountered (`oc_encountered`)
- Spellbooks by description: always fails ("not enough information")
- Spellbooks by name: fresh spell knowledge (`spe_Fresh`) works; `spe_GoingStale` has better luck factor

**Luck override:** `rnl(N)` where N=5 for (Wizard role writing scrolls) or (GoingStale spell knowledge), N=15 otherwise. If `rnl(N) == 0`, writing succeeds regardless of knowledge.

**Special rejections:**
- Cannot write SCR_BLANK_PAPER or SPE_BLANK_PAPER ("obscene")
- Cannot write SPE_NOVEL (humorous failure)
- Cannot write SPE_BOOK_OF_THE_DEAD ("no mere dungeon adventurer")

### 8.4 BUC of Result

```
curseval = bcsign(pen) + bcsign(paper)
result.blessed = (curseval > 0)
result.cursed  = (curseval < 0)
// uncursed when curseval == 0
```

### 8.5 Blind Writing

- Cannot write spellbooks while blind at all ("can't create braille text")
- Writing scrolls while blind: additional `rnl(3)` check; fails unless `rnl(3) == 0` (Luck helps)

---

## 9. Unicorn Horn (`use_unicorn_horn`)

### 9.1 Cursed Horn

Effect selected by `rn2(13) / 2` (case 6 is half as likely as cases 0-5):

| Case | Effect | Duration |
|------|--------|----------|
| 0 | Sickness | if already sick: `(Sick & TIMEOUT) / 3 + 1`; else: `rn1(CON, 20)` = 20..20+CON-1 |
| 1 | Blindness | +`rn1(90, 10)` = 10..99 turns |
| 2 | Confusion | +`rn1(90, 10)` = 10..99 turns |
| 3 | Stun | +`rn1(90, 10)` = 10..99 turns |
| 4 | Vomiting | if already vomiting: immediate vomit; else: `make_vomiting(14)` |
| 5 | Hallucination | +`rn1(90, 10)` = 10..99 turns |
| 6 | Deafness | +`rn1(90, 10)` = 10..99 turns |

### 9.2 Uncursed/Blessed Horn

Cures **timed** instances of these 7 properties (order shuffled randomly):
1. Sick
2. Blinded (only the timed portion exceeding `ucreamed`)
3. Hallucination
4. Vomiting
5. Confusion
6. Stunned
7. Deaf

**Number of ailments cured:**
```
val_limit = rn2(d(2, blessed ? 4 : 2))
// val_limit capped at trouble_count
```

**Probability distribution (from source comments):**

| # cured | Blessed | Uncursed |
|---------|---------|----------|
| 0 | 22.7% | 35.4% |
| 1 | 22.7% | 35.4% |
| 2 | 19.5% | 22.9% |
| 3 | 15.4% | 6.3% |
| 4 | 10.7% | 0% |
| 5 | 5.7% | 0% |
| 6 | 2.6% | 0% |
| 7 | 0.8% | 0% |

If no timed troubles exist: "nothing happens."

### 9.3 Non-timed Troubles

The horn only fixes properties with `TIMEOUT` component. The `TimedTrouble(P)` macro returns `P & TIMEOUT` only if **all** bits of P are in the TIMEOUT range (i.e., `(P & ~TIMEOUT) == 0`). Properties from intrinsics, equipment, or polymorph are not fixable.

---

## 10. Figurines

### 10.1 Applying a Figurine (`use_figurine`)

Direction prompt. Places figurine at `(u.ux + u.dx, u.uy + u.dy)`. Calls `make_familiar()` which creates a tame monster of the figurine's `corpsenm` type. Figurine is consumed.

Location checks:
- Cannot place in solid rock or trees (unless monster `passes_walls`)
- Cannot fit on boulder (unless monster `throws_rocks` or `passes_walls`)
- Cannot place while swallowed

### 10.2 Automatic Transformation (`fig_transform`)

Figurines have a timer (`FIG_TRANSFORM`) that fires after a random delay. When it fires:
- Checks location validity
- If invalid: reschedules for `rnd(5000)` more turns
- If valid: creates the monster, consumes the figurine

### 10.3 Gender

Figurine `spe` field bits 0-1 store gender (`CORPSTAT_GENDER` mask, values 0..3).

---

## 11. Tinning Kit (`use_tinning_kit`)

- Consumes 1 charge (`spe`); if `spe <= 0`: "out of tins"
- Requires a corpse (on floor via `floorfood()`, not partly eaten, has nutrition)
- Touching petrifying corpse without gloves: instapetrify
- Rider corpses: revive instead (cannot tin)
- Creates a TIN with `HOMEMADE_TIN` variety, inherits `corpsenm`
- Tin's BUC inherits from the tinning kit
- Takes 1 move

---

## 12. Leash (`use_leash`)

- Max 2 leashed pets simultaneously (`MAXLEASHED = 2`)
- Target must be tame, solid (not `unsolid`), and have limbs or a head
- Long worms cannot be leashed (`leashable()` check)
- Cursed leash cannot be removed
- Leash distance mechanics (`check_leash`):
  - Distance <= 3 squares: no effect
  - Distance 4-5, cursed leash, breathable monster: chokes (`rnd(2)` damage per turn; tameness decreases: if `rn2(mtame)` fails, `mtame--`)
  - Distance > 5, cursed: choke to death
  - Distance > 5, uncursed: leash snaps loose

---

## 13. Saddle (`use_saddle`)

Handled in `src/steed.c`. Applies saddle to adjacent tame monster that can be ridden.

---

## 14. Stethoscope (`use_stethoscope`)

- Requires hands, hearing (`!Deaf`), free hand
- **Cursed:** 50% chance (`!rn2(2)`) to hear own heartbeat (no useful info)
- **Time cost:** First use per hero-turn is free; second use costs a turn (tracked by `context.stethoscope_seq == hero_seq`)
- **Adjacent monster:** reveals hidden/disguised monsters, shows `mstatusline`
- **Adjacent wall:** detects secret doors (`SDOOR` -> `DOOR`) and corridors (`SCORR` -> `CORR`) via "hollow sound"
- **Downward:** corpse/statue examination; Healer role can detect revivers ("mostly dead"), trapped statues ("extraordinary"), or statues with contents ("remarkable")
- **Self (dx=dy=0):** shows `ustatusline`
- **Swallowed + whirly engulfer:** interference check `!rn2(Role_if(PM_HEALER) ? 10 : 3)`

---

## 15. Bullwhip Mechanics (`use_whip`)

Must be wielded. Requires direction.

### 15.1 Proficiency Calculation

```
proficient = 0
if Role_if(PM_ARCHEOLOGIST): proficient += 1
if ACURR(A_DEX) < 6: proficient -= 1
else if ACURR(A_DEX) >= 14: proficient += (ACURR(A_DEX) - 14)
if Fumbling: proficient -= 1
proficient = clamp(proficient, 0, 3)
```

### 15.2 Self-targeting (down or same square)

- If riding steed and `!rn2(proficient + 2)`: whip steed (kick)
- If levitating/flying/riding + item on floor + proficient > 0: attempt to snag item
  - Fails on `rnl(6)` (nonzero result) [疑似 bug: `rnl(6)` with positive Luck makes lower values more likely, meaning good luck makes snagging *more* likely to fail since the `||` short-circuits -- actually on inspection, `rnl(6) || pickup_object(...) < 1` means: if `rnl(6)` is nonzero (truthy) OR pickup fails, then slip free. With good luck, `rnl` biases toward 0, so the snag succeeds more often. This is actually correct behavior.]
- Otherwise: hit own foot for `rnd(2) + dbon() + spe` damage (minimum 1)

### 15.3 From Pit

If in a pit and target square has boulder, furniture, or visible big monster:
- Escape attempt: succeeds if `proficient > 0 && rn2(proficient + 2)` is nonzero
- Failure: "slips free"

### 15.4 Disarming Monsters

If target monster is visible and wielding a weapon:
- `gotit = proficient > 0 && (!Fumbling || !rn2(10))`
- If weapon is welded (`mwelded`): cannot pull free, sets `bknown`
- On success, `rn2(proficient + 1)` determines weapon destination:

| Roll | Result |
|------|--------|
| 0 | Weapon falls at monster's feet |
| 1 | Weapon falls at monster's feet |
| 2 | Weapon yanked to hero's square |
| 3 | Weapon snatched into hero's inventory (cockatrice corpse = instapetrify without gloves) |

### 15.5 Fumbling/Glib Check

If `(Fumbling || Glib) && !rn2(5)`: bullwhip slips from hand and is dropped. Checked before monster interaction.

---

## 16. Cream Pie (`use_cream_pie`)

- Splits 1 from stack
- Self-applied to face
- If `can_blnd`: `blindinc = rnd(25)`, `ucreamed += blindinc`, `make_blinded(BlindedTimeout + blindinc)`
- Pie is consumed via `costly_alteration` + `delobj`
- Returns `ECMD_OK` (does **not** cost a turn)

[疑似 bug: Applying a cream pie to your face performs a visible action (splits object, blinds hero, destroys pie) but returns `ECMD_OK` instead of `ECMD_TIME`, meaning it costs no game time. This appears intentional as a self-inflicted comedy action.]

---

## 17. Can of Grease (`use_grease`)

- If Glib: drops the can (costs turn)
- If `spe > 0`:
  - **Fumble check:** if `(cursed || Fumbling) && !rn2(2)`: consumes charge, drops can
  - Otherwise: prompted for target object, consumes charge
  - Target becomes `greased = 1`
  - **Cursed can side-effect:** makes hands glib `rn1(6, 10)` = 10..15 extra turns
  - **Greasing hands directly** (choosing `hands_obj`): glib `rn1(11, 5)` = 5..15 turns

---

## 18. Towel (`use_towel`)

- Requires free hand; cannot use while worn as blindfold
- **Cursed towel:** `rn2(3)` selects effect:
  - Case 2: make hands glib +`rn1(10, 3)` = 3..12 turns
  - Case 1: smear face with gunk (blind) or push off blindfold
  - Case 0: falls through to normal use

**Normal use priority:**
1. If Glib: wipe hands clean (cure Glib)
2. If `ucreamed`: wipe face, reduce blindness by `ucreamed`, set `ucreamed = 0`
3. Otherwise: "already clean" (no turn cost)

---

## 19. Touchstone (`use_stone`)

- **Blessed touchstone** (or uncursed + Archeologist/Gnome): fully identifies rubbed gem (`makeknown`)
- **Cursed touchstone + non-graystone gem:** `!obj_resists(80, 100)` = 20% chance to shatter the gem
- **Blind:** "scritch, scritch" (no useful information)
- **Hallucination:** "Fractals!" (no useful information)
- Non-gem objects: produce scratch marks or colored streaks based on material

---

## 20. Containers

Applying a container dispatches to `use_container()` which provides the loot interface (put in, take out). Key container types:

- **ICE_BOX:** corpses inside have rot timers stopped
- **BAG_OF_HOLDING:** weight reduction; explodes if cancellation wand/bag of tricks/another bag of holding placed inside
- **OILSKIN_SACK:** waterproof
- **CHEST/LARGE_BOX:** can be locked/trapped; waterproof
- **BAG_OF_TRICKS:** `bagotricks()` instead of container interface -- consumes charge, creates random monster

---

## 21. Traps (Land Mine, Bear Trap)

### 21.1 Time to Set

```
time = (DEX > 17) ? 2 : (DEX > 12) ? 3 : (DEX > 7) ? 4 : 5
if Blind: time *= 2
if BEAR_TRAP AND STR < 18: time += (STR > 12) ? 1 : (STR > 7) ? 2 : 4
```

### 21.2 Fumble/Bungle on Completion

On completion: if `(cursed || Fumbling) && rnl(10) > 5`: trigger trap on self.

When riding with `P_RIDING < P_BASIC`:
- Extra bungle chance: `rnl(10) > 3` if cursed/fumbling, `rnl(10) > 5` otherwise
- Bear trap bungle: drops unarmed
- Land mine bungle: `force_bungle = TRUE` -> detonates on you

---

## 22. Polearm Use (`use_pole`)

### 22.1 Range (distance squared)

```
min_range = 4
max_range:
  P_NONE or P_BASIC: 4
  P_SKILLED: 5
  P_EXPERT: 8
```

Distance-squared 4 = orthogonally 2 squares. Distance-squared 5 = knight's-move. Distance-squared 8 = diagonal 2 squares.

### 22.2 Snickersnee Special

Artifact Snickersnee allows one free hit per turn from distance (`freehit`). Tracked by `context.snickersnee_turn`. Subsequent distance attacks in same turn: "The blade doesn't reach there!"

---

## 23. Grappling Hook (`use_grapple`)

Must be wielded. Range: same formula as polearm, but no minimum range.

If skill >= P_SKILLED: player chooses target type via menu. Chosen target succeeds with probability `rn2(P > P_SKILLED ? 20 : 2)` (i.e., 95% for Expert, 50% for Skilled).

Otherwise: random `rn2(5)`.

| tohit | Result |
|-------|--------|
| 0 | Trap (FIXME -- not implemented) |
| 1 | Object: pick up from floor |
| 2 | Monster: if very small + `!rn2(4)`: pull to hero; elif not big/strong or `rn2(4)`: attack; else fall through to surface |
| 3 | Surface: if solid, yank hero toward target (`hurtle` 1 square) |
| default | Self (unskilled/basic only): `rn1(10, 10)` = 10..19 damage |

---

## 24. Wand Breaking (`do_break_wand`)

Applying a wand triggers breaking. Requires hands, free hand, `STR >= 10` (or `>= 5` for fragile wands: balsa/glass description). Paranoid confirmation required.

### 24.1 Charge Mechanics

```
dmg = spe * 4
```
If spe was brought to 0 via wresting, breaking gives it `rnd(3)` charges for explosion purposes.

### 24.2 Effects by Wand Type

| Wand | Explosion Damage | Type |
|------|-----------------|------|
| WAN_DEATH / WAN_LIGHTNING | `dmg * 4` | EXPL_MAGICAL |
| WAN_FIRE | `dmg * 2` | EXPL_FIERY |
| WAN_COLD | `dmg * 2` | EXPL_FROSTY |
| WAN_MAGIC_MISSILE | `dmg` | EXPL_MAGICAL |
| WAN_STRIKING | `d(1 + spe, 6)` (normally 2d12) + "wall of force" | EXPL_MAGICAL |
| WAN_WISHING / NOTHING / LOCKING / PROBING / ENLIGHTENMENT / SECRET_DOOR_DETECTION | Nothing else happens | -- |
| WAN_OPENING (while grabbed) | Releases hold, nothing else | -- |
| WAN_DIGGING | Pits/holes in all 8 adjacent + hero square | -- |
| WAN_CREATE_MONSTER | Creates monsters | -- |
| WAN_CANCELLATION / POLYMORPH / TELEPORTATION / UNDEAD_TURNING | `rnd(dmg)` explosion + affects objects and monsters in 8 adjacent squares, then hero | EXPL_MAGICAL |
| WAN_LIGHT | `rnd(dmg)` explosion + `litroom(TRUE)` | EXPL_MAGICAL |

---

## 25. Whistle

### 25.1 Tin Whistle (`use_whistle`)

- Wakes nearby monsters
- **Cursed:** produces "shrill" sound; also summons vault guard (`vault_summon_gd`)

### 25.2 Magic Whistle (`use_magic_whistle`)

- **Cursed:** 50% (`!rn2(2)`) malfunction: wakes nearby + 50% chance (`!rn2(2)`) teleport random pet to hero
- **Normal/blessed:** teleports all tame monsters to squares adjacent to hero
  - If pet lands on trap and dies: lose 1 Luck
  - If magic whistle not yet discovered: discovery happens when any pets move within/into view

---

## 26. Bell of Opening (`use_bell`)

- If ordinary bell (not BoO, or `spe <= 0`): normal bell ring
  - **Cursed + `!rn2(4)` (25%):** summon nymph; bell has 7% chance to shatter (`!obj_resists(93, 100)`)
  - If bell survives shatter check: 1/3 chance nymph is sped up, 1/3 chance hero paralyzed `rnd(2)` turns, 1/3 nothing extra
- **Charged Bell of Opening:**
  - **At invocation site:** unsettling shrill sound, sets `obj->age = moves` (for invocation ritual)
  - **Blessed:** unpunish + `openit()` (opens doors/containers)
  - **Uncursed:** `findit()` (reveals secret doors/corridors)
  - **Cursed:** `mkundead()` (creates undead at hero's location)
  - **Swallowed + not cursed:** `openit()`

---

## 27. Royal Jelly (`use_royal_jelly`)

Applied to eggs:
- Killer bee egg -> Queen bee egg (corpsenm changed)
- **Cursed jelly:** kills the egg (`kill_egg`)
- **Uncursed/blessed:** starts hatch timer if not already ticking (`attach_egg_hatch_timeout`)
- **Blessed + egg not laid by hero (`spe != 1`):** sets `spe = 2` (hatched creature treats hero as parent)

---

## 28. Eucalyptus Leaf

- **Blessed:** acts as magic whistle; 1/49 chance (`!rn2(49)`) to lose blessed status (becomes uncursed, "glows brown")
- **Otherwise (uncursed/cursed):** acts as tin whistle

---

## 29. Horn of Plenty (`hornoplenty`)

Not a musical instrument. Produces food (or potions if blessed) when applied. Consumes charges. If no charges: empty.

---

## 30. Bag of Tricks (`bagotricks`)

Consumes a charge. Creates a random monster near hero. If `spe <= 0`: nothing happens (empty).

---

## 31. Digging Tools (Pick-axe, Dwarvish Mattock)

Dispatched to `use_pick_axe()` (defined in `src/dig.c`). Allows digging into walls, floors, etc. See movement/digging spec for details.

---

## 32. Tin Opener (`use_tin_opener`)

Defined in `src/eat.c`. Opens tins from inventory or floor. If no tin available: "can't find anything to use that on."

---

## 33. Flip Through Book (`flip_through_book`)

No mechanical effect. Flavor text:
- Book of the Dead: sound/glow/tremble
- Blank paper: identified
- Novel: "might be interesting to read"
- Regular spellbook: shows ink fading based on `spestudied`:
  - 0: "fresh"
  - 1: "slightly faded"
  - 2: "very faded"
  - 3: "extremely faded"
  - 4+: "barely visible"

---

## 34. Flip Coin (`flip_coin`)

- 50/50 heads or tails
- **Drop risk:** if `Glib || Fumbling || (ACURR(A_DEX) < 10 && !rn2(DEX))`: coin slips and is dropped
- **Underwater:** coin tumbles away (dropped)
- **Hallucination:** 99% "double header!", 1% "lands on its edge!"

---

## 测试向量

### TV1: Lock Pick on Container -- Rogue, DEX 18
```
Input:  tool=LOCK_PICK, target=container, DEX=18, is_rogue=true, container_cursed=false
Calc:   chance = 4*18 + 25*1 = 97
Output: 97% per-turn success probability
```

### TV2: Credit Card on Door -- Non-Rogue, DEX 10
```
Input:  tool=CREDIT_CARD, target=door, DEX=10, is_rogue=false
Calc:   chance = 2*10 + 20*0 = 20
Output: 20% per-turn success probability; expected ~5 turns to open
```

### TV3: Skeleton Key on Cursed Container -- DEX 15
```
Input:  tool=SKELETON_KEY, target=container, DEX=15, container_cursed=true
Calc:   chance = (75 + 15) / 2 = 45
Output: 45% per-turn success probability
```

### TV4: Unicorn Horn (blessed) -- 3 ailments active
```
Input:  horn blessed=true, troubles=[SICK, CONFUSED, STUNNED]
Calc:   d(2,4) e.g. = 5, val_limit = rn2(5) e.g. = 3
        capped at trouble_count = 3, so cures 3
Output: All 3 ailments cured (probability depends on d(2,4) and rn2 rolls)
```

### TV5: Unicorn Horn (uncursed) -- 1 ailment active
```
Input:  horn blessed=false, cursed=false, troubles=[HALLUC]
Calc:   d(2,2) range [2,4], e.g. = 2
        val_limit = rn2(2) = 0 or 1
        If 0: "nothing seems to happen"; If 1: cure hallucination
Output: ~50% chance cure, ~50% nothing (boundary: only 1 trouble)
```

### TV6: Magic Lamp Rub (blessed) -- Wish outcome
```
Input:  lamp.spe=1, lamp.blessed=true, rn2(3)=0 (djinni released)
Calc:   chance = rn2(5), suppose = 2
        blessed adjustment: chance != 4, so chance = 0 (wish)
Output: Wish granted. P(wish | blessed) = 80%
```

### TV7: Magic Lamp Rub (cursed) -- Hostile outcome
```
Input:  lamp.spe=1, lamp.cursed=true, rn2(3)=0 (djinni released)
Calc:   chance = rn2(5), suppose = 3
        cursed adjustment: chance != 0, so chance = 4 (hostile)
Output: Hostile djinni. P(hostile | cursed) = 80%
```

### TV8: Crystal Ball -- INT 18, Uncursed (safe boundary)
```
Input:  spe=3, blessed=false, cursed=false, INT=18
Calc:   oops = 20, backfire check: rnd(20) > 18?
        Only rnd(20) in {19, 20} triggers = 10% backfire
Output: 10% backfire chance
```

### TV9: Crystal Ball -- INT 20, Uncursed (boundary: never backfires)
```
Input:  spe=1, blessed=false, cursed=false, INT=20
Calc:   oops = 20, backfire check: rnd(20) > 20? => never (max rnd(20) = 20)
Output: 0% backfire -- completely safe
```

### TV10: Crystal Ball -- Blessed, INT 15
```
Input:  spe=2, blessed=true, cursed=false, INT=15
Calc:   oops = 16, backfire check: rnd(16) > 15? => only rnd(16)=16, i.e., 1/16 = 6.25%
        If backfire: rnd(4) effect (no explosion possible for blessed)
Output: 6.25% backfire, explosion impossible
```

### TV11: Magic Marker Ink -- Writing SCR_GENOCIDE (boundary)
```
Input:  pen.spe=15, target=SCR_GENOCIDE
Calc:   basecost = 30, min ink = 30/2 = 15
        pen.spe = 15 >= 15, passes minimum check
        actualcost = rn1(15, 15) = [15, 29]
        Dry-out check: pen.spe < actualcost (strict less-than)
        If actualcost = 15: 15 < 15 is FALSE => scroll IS written successfully,
            pen.spe = 15 - 15 = 0 (marker empties but scroll created)
        If actualcost > 15: 15 < actualcost is TRUE => marker dries out,
            scroll disappears (spellbook left blank)
Output: pen.spe=15 is exact minimum to attempt; 1/15 chance (actualcost=15)
        the scroll is written successfully (pen empties via subtraction);
        14/15 chance marker dries out mid-write and scroll is lost
```

### TV12: Magic Marker Ink -- Writing SCR_GENOCIDE (comfortable)
```
Input:  pen.spe=30, target=SCR_GENOCIDE
Calc:   basecost = 30, min ink = 15, pen.spe=30 >= 15
        actualcost = rn1(15, 15) = [15, 29]
        pen.spe after = [30-29, 30-15] = [1, 15]
Output: Always succeeds, pen retains 1..15 charges
```

### TV13: Wooden Flute Charm -- DEX 18, Level 8 (boundary failure)
```
Input:  DEX=18, ulevel=8
Calc:   do_spec requires rn2(18) + 8 > 25
        rn2(18) range [0,17], max sum = 17+8 = 25
        25 > 25 is false
Output: NEVER charms snakes at this DEX/level combination
```

[疑似 bug: The wooden flute/harp charm check uses strict `>` instead of `>=`. With DEX=18 and level=8, `rn2(18)+8` maxes at 25 which does not satisfy `> 25`. The threshold is unreachable at these exact values. Using `>=` would be more natural. This means minimum requirement for any charm chance is DEX+level > 26 in the best case (e.g., DEX=19 level=8, or DEX=18 level=9).]

### TV14: Force Lock -- Blade Break Probability (+0, no erosion)
```
Input:  uwep.spe=0, greatest_erosion=0
Calc:   break if rn2(1000 - 0) > (992 - 0*10) AND !cursed AND !obj_resists(0,99)
        = rn2(1000) > 992, i.e., rn2(1000) in {993..999} = 7 values
        P(break per attempt) = 7/1000 = 0.7%
        P(survive 50 attempts) = (993/1000)^50 = 0.703
        P(break over 50 attempts) = 1 - 0.703 = 29.7%
Output: ~0.7% break/attempt, ~30% cumulative over full 50-turn attempt
```

### TV15: Bullwhip Proficiency -- Archeologist, DEX 18 (max proficiency boundary)
```
Input:  Role=ARCHEOLOGIST, DEX=18, Fumbling=false
Calc:   proficient = 0 + 1(Arch) + (18-14)(DEX bonus) = 5
        clamp(5, 0, 3) = 3
        Disarm: gotit = (3 > 0) && true = true (guaranteed if not welded)
        Weapon placement: rn2(3+1) = rn2(4) = 0..3
Output: proficient=3, guaranteed disarm, weapon destination uniform 0..3
```
