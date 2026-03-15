# Pet System

Source: `src/dog.c`, `src/dogmove.c`, `src/steed.c`, `src/apply.c`, `src/hack.c`, `src/read.c`, `src/trap.c`
Data: `include/mextra.h` (edog struct), `include/monst.h` (mtame, mpeaceful fields), `src/role.c` (petnum per role)

---

## 1. Data Structures

### monst fields (pet-relevant)

| Field | Type | Description |
|---|---|---|
| `mtame` | `schar` | Tameness level, 0..20. Nonzero implies pet; also implies `mpeaceful=1` |
| `mpeaceful` | Bitfield(1) | Does not attack unprovoked. All tame monsters are peaceful; reverse not true |
| `mleashed` | Bitfield(1) | Monster is on a leash |

### edog struct

Allocated via `newedog()` when a monster becomes tame. Freed via `free_edog()`.

| Field | Type | Description |
|---|---|---|
| `parentmid` | `unsigned` | Clobber-detection; set to `mtmp->m_id` |
| `droptime` | `long` | Turn when pet last dropped an object |
| `dropdist` | `unsigned` | Distance^2 from hero when object was dropped |
| `apport` | `int` | Fetch training level; initialized to `ACURR(A_CHA)` |
| `whistletime` | `long` | Turn when hero last whistled |
| `hungrytime` | `long` | Turn at which pet was last "full"; hunger measured as `moves - hungrytime` |
| `ogoal` | `coord` | Cached previous goal location (for pathfinding when hero not visible) |
| `abuse` | `int` | Cumulative abuse counter; incremented by `abuse_dog()` |
| `revivals` | `int` | Number of times this pet has died and been revived |
| `mhpmax_penalty` | `int` | HP max reduction while starving; restored when pet eats |
| `killed_by_u` | Bitfield(1) | Hero attempted to kill this pet |

---

## 2. Pet Initialization

### 2.1 Starting Pet Selection

```
fn pet_type() -> MonsterIndex:
    if role.petnum != NON_PM:
        return role.petnum
    if preferred_pet == 'c':
        return PM_KITTEN
    if preferred_pet == 'd':
        return PM_LITTLE_DOG
    return random_choice(PM_KITTEN, PM_LITTLE_DOG)  // 50/50
```

Role-specific `petnum` values from `src/role.c`:

| Role | petnum |
|---|---|
| Caveperson | PM_LITTLE_DOG |
| Knight | PM_PONY |
| Ranger | PM_LITTLE_DOG |
| Samurai | PM_LITTLE_DOG |
| Wizard | PM_KITTEN |
| All others | NON_PM (use `preferred_pet` or random) |

Knight's pony starts with a saddle (unless pauper mode) and gets Basic riding skill.

### 2.2 Default Pet Names

Some roles assign default names to dogs:
- Caveperson: "Slasher"
- Samurai: "Hachi"
- Barbarian: "Idefix"
- Ranger: "Sirius"

(Only for dogs; cats and horses have no role-specific default names.)

### 2.3 `initedog()` -- Pet Initialization

Called for both new taming and re-taming. Parameter `everything=true` for new pets, `false` for re-taming existing ones.

```
fn initedog(mtmp, everything: bool):
    minimum_tame = if is_domestic(mtmp.data) then 10 else 5
    mtmp.mtame = max(minimum_tame, mtmp.mtame)
    mtmp.mpeaceful = 1
    mtmp.mavenge = 0

    if everything:
        edog.droptime = 0
        edog.dropdist = 10000
        edog.apport = player.CHA   // current Charisma stat
        edog.whistletime = 0
        edog.ogoal = (-1, -1)
        edog.abuse = 0
        edog.revivals = 0
        edog.mhpmax_penalty = 0
        edog.killed_by_u = 0
    else:
        if edog.apport <= 0:
            edog.apport = 1

    min_hungry = moves + 1000
    if edog.hungrytime < min_hungry:
        edog.hungrytime = min_hungry
```

Key facts:
- Domestic animals (dogs, cats, horses) start at tameness 10+
- Non-domestic tamed monsters start at tameness 5+
- Initial apport = hero's Charisma score
- Pet starts with 1000 turns of food

---

## 3. Tameness / Loyalty Mechanics

### 3.1 Tameness Range

- Range: 0..20
- 0 = not tame
- Maximum of 20, reachable only through eating (each feeding: `mtame++` if `mtame < 20`)
- Scroll/spell of taming can raise tameness toward 10 (but not above 10)

### 3.2 Tameness Increase

**Eating food** (`dog_eat()`):
```
if mtmp.mtame < 20:
    mtmp.mtame += 1
```

**Taming magic** (`tamedog()`) on already-tame pet with `mtame < 10`:
```
if mtmp.mtame < rnd(10):
    mtmp.mtame += 1
if blessed_scroll:
    mtmp.mtame += 2
    mtmp.mtame = min(mtmp.mtame, 10)
```

### 3.3 Tameness Decrease

**`abuse_dog()`** -- called when hero hits pet, kicks pet, forces pet into trap, or zaps pet:
```
fn abuse_dog(mtmp):
    if Aggravate_monster or Conflict:
        mtmp.mtame /= 2    // integer division
    else:
        mtmp.mtame -= 1

    if mtmp.mtame > 0 and not mtmp.isminion:
        EDOG(mtmp).abuse += 1

    if mtmp.mtame == 0 and mtmp.mleashed:
        m_unleash(mtmp, true)
```

Abuse sources:
- Hero hits pet in melee (`uhitm.c`)
- Hero kicks pet (`dokick.c`)
- Hero zaps pet with harmful wand (`zap.c`)
- Hero forces pet into trap by moving into it (`hack.c`)
- Pet steps on bear trap set by hero (`trap.c`)

**Tameness decay while separated** (`mon_catchup_elapsed_time()`):
```
wilder = (time_away + 75) / 150    // 1 tameness per 150 turns
if mtmp.mtame > wilder:
    mtmp.mtame -= wilder
elif mtmp.mtame > rn2(wilder):
    mtmp.mtame = 0          // untame, stays peaceful
else:
    mtmp.mtame = 0
    mtmp.mpeaceful = 0      // hostile!
```

**Starvation check while separated** (same function):
```
if mtame > 0 and not isminion and (carnivorous or herbivorous):
    if (moves > hungrytime + 500 and mhp < 3) or (moves > hungrytime + 750):
        mtmp.mtame = 0
        mtmp.mpeaceful = 0
```

**Leash snapping during migration** (`migrate_to_level()`):
```
if mtmp.mleashed:
    mtmp.mtame -= 1
    m_unleash(mtmp, true)
```

**Mounting a non-Knight** (`mount_steed()`):
```
if not force and not Role_if(PM_KNIGHT):
    mtmp.mtame -= 1
    if mtmp.mtame == 0:
        // resists mounting, leash comes off
```

---

## 4. Pet Hunger System

### 4.1 Hunger Thresholds

Hunger is measured as `deficit = moves - edog.hungrytime`:

| Threshold | Constant | Deficit | Effect |
|---|---|---|---|
| Hungry | `DOG_HUNGRY` | 300 | Pet less likely to use breath attacks |
| Weak | `DOG_WEAK` | 500 | Starvation penalty triggered |
| Starve | `DOG_STARVE` | 750 | Pet dies |

### 4.2 Hunger Processing (`dog_hunger()`)

Called every pet turn from `dog_move()`:

```
fn dog_hunger(mtmp, edog) -> died:
    if moves > edog.hungrytime + DOG_WEAK:
        if not carnivorous and not herbivorous:
            edog.hungrytime = moves + DOG_WEAK  // non-eaters never starve
        elif not edog.mhpmax_penalty:
            // first starvation hit
            new_maxhp = mtmp.mhpmax / 3
            mtmp.mconf = 1
            edog.mhpmax_penalty = mtmp.mhpmax - new_maxhp
            mtmp.mhpmax = new_maxhp
            if mtmp.mhp > mtmp.mhpmax:
                mtmp.mhp = mtmp.mhpmax
            if DEAD(mtmp):
                dog_starve(mtmp)
                return true
        elif moves > edog.hungrytime + DOG_STARVE or DEAD(mtmp):
            dog_starve(mtmp)
            return true
    return false
```

### 4.3 Nutrition Calculation (`dog_nutrition()`)

```
fn dog_nutrition(mtmp, obj) -> int:
    if obj.oclass == FOOD_CLASS:
        if obj.otyp == CORPSE:
            meating = 3 + (mons[obj.corpsenm].cwt >> 6)
            nutrit = mons[obj.corpsenm].cnutrit
        else:
            meating = objects[obj.otyp].oc_delay
            nutrit = objects[obj.otyp].oc_nutrition

        // Scale by monster size
        match mtmp.data.msize:
            MZ_TINY     => nutrit *= 8
            MZ_SMALL    => nutrit *= 6
            MZ_MEDIUM   => nutrit *= 5
            MZ_LARGE    => nutrit *= 4
            MZ_HUGE     => nutrit *= 3
            MZ_GIGANTIC => nutrit *= 2

        if obj.oeaten:   // partially eaten
            nutrit = eaten_stat(nutrit, obj)
    elif obj.oclass == COIN_CLASS:
        meating = obj.quan / 2000 + 1
        nutrit = obj.quan / 20
    else:
        meating = obj.owt / 20 + 1
        nutrit = 5 * objects[obj.otyp].oc_nutrition

    return nutrit
```

### 4.4 Eating Effect (`dog_eat()`)

```
fn dog_eat(mtmp, obj, ..., devour):
    if edog.hungrytime < moves:
        edog.hungrytime = moves   // floor at current time
    nutrit = dog_nutrition(mtmp, obj)

    if devour:
        meating /= 2
        nutrit = nutrit * 3 / 4

    edog.hungrytime += nutrit
    mtmp.mconf = 0                // clear confusion from hunger

    if edog.mhpmax_penalty:       // was starving
        mtmp.mhpmax += edog.mhpmax_penalty
        edog.mhpmax_penalty = 0

    if mtmp.mflee and mtmp.mfleetim > 1:
        mtmp.mfleetim /= 2

    if mtmp.mtame < 20:
        mtmp.mtame += 1

    // Apport training from player-provided food
    if dogfood(mtmp, obj) == DOGFOOD and obj.invlet != 0:
        edog.apport += 200 / (edog.dropdist + moves - edog.droptime)
```

### 4.5 Food Quality (`dogfood()`)

Returns an enum ranking how attractive food is to the pet:

| Value | Constant | Meaning |
|---|---|---|
| 0 | `DOGFOOD` | Preferred food (tripe, meat items for carnivores, apples/carrots for herbivores) |
| 1 | `CADAVER` | Acceptable corpse or egg |
| 2 | `ACCFOOD` | Acceptable but not preferred food |
| 3 | `MANFOOD` | Human food, pet won't seek it |
| 4 | `APPORT` | Non-food item, might fetch it |
| 5 | `POISON` | Harmful (rotten, poisonous, petrifying) |
| 6 | `UNDEF` | Unknown/uninteresting |
| 7 | `TABU` | Absolutely will not eat (Rider corpses, silver for silver-haters, etc.) |

Key food preferences:
- **Carnivores**: tripe/meat = DOGFOOD, veggy = MANFOOD
- **Herbivores**: apples/carrots = DOGFOOD, meat = MANFOOD
- **Blind carnivores**: carrots = DOGFOOD (cures blindness)
- **Metallivores**: rustprone non-proofed metal = DOGFOOD, other metal = ACCFOOD
- **Ghouls**: old corpses (age + 50 <= moves) = DOGFOOD, fresh = POISON
- **Rotten corpses** (age + 50 <= moves, not lizard/lichen): POISON for non-fungus
- **Poisonous/acidic corpses**: POISON if pet lacks resistance
- **Petrifying corpses**: POISON if pet lacks stone resistance
- **Starving pet**: will eat ACCFOOD items it would normally ignore
- **Polymorph corpses**: MANFOOD unless starving or abused (`mtame > 1` check)
- **Cannibalism**: TABU unless starving carnivore (and not elf)

---

## 5. Pet Movement AI

### 5.1 Movement Overview (`dog_move()`)

Each pet turn:
1. Check hunger (`dog_hunger()`) -- may starve and die
2. Handle inventory (`dog_invent()`) -- drop or pick up or eat items
3. Determine goal (`dog_goal()`) -- food, fetch, or follow player
4. Evaluate available positions (`mfndpos()`)
5. Select best move toward goal, considering traps, cursed items, combat
6. Attempt ranged attacks if no melee target
7. Move to selected position

### 5.2 Goal Selection (`dog_goal()`)

```
fn dog_goal(mtmp, edog, after, udist, whappr) -> approach_value:
    if mtmp == u.usteed:
        return -2  // steeds don't move independently

    if not edog or mtmp.mleashed:
        goal = player_position
        return approach_player

    // Search for food/items within SQSRCHRADIUS=5 squares
    for each obj on floor within 5 squares:
        food_quality = dogfood(mtmp, obj)
        if food_quality >= current_goal_quality or food_quality == UNDEF:
            continue
        if cursed_object_at(x,y) and not (starving and food < MANFOOD):
            continue
        if not reachable:
            continue

        if food_quality < MANFOOD:
            // Food: go for closest best-quality food
            if food_quality < goal_quality or closer:
                set goal to this object
        elif goal == UNDEF and in_master_sight and no_inventory
             and apport > rn2(8) and can_carry:
            // Fetchable item
            set goal to this object, type = APPORT

    // Follow player if no better goal
    if goal == UNDEF or (goal != DOGFOOD and goal != APPORT
                         and not hungry yet):
        goal = player_position
        if udist >= 9:
            approach = 1   // always approach if far
        elif mtmp.mflee:
            approach = -1  // flee
        else:
            approach = 0   // neutral

        // Conditions that force approach:
        // - not in a room, or 1/4 chance
        // - whappr (recently whistled)
        // - carrying items and rn2(apport)
        // - player on stairs/ladder
        // - player near magic portal
        // - player carrying DOGFOOD

    if mtmp.mconf:
        approach = 0  // confused pets move randomly

    // If player not visible, use tracking or cached goal
```

### 5.3 Position Evaluation in `dog_move()`

For each candidate position:

1. **Leash constraint**: if leashed, skip positions where `dist2(pos, player) > 4`
2. **Guardian angel**: stay within dist2 = 16 if closer than current
3. **Combat evaluation** (see section 6)
4. **Trap avoidance**: if trap is seen by player, 39/40 chance to avoid; if leashed, whimper but might step on it. 1/40 chance to step on known trap anyway
5. **Cursed item avoidance**: reluctant to step on cursed items; if all available squares have cursed items, will step on them with probability `1/(13*uncursed_count)`
6. **Backtrack avoidance**: if not leashed and >5 tiles from player, reduces chance of revisiting recent positions (uses `mtrack[]`)
7. **Goal distance**: prefer positions closer to goal; random tie-breaking

### 5.4 Leash Snap-back

If leashed pet didn't move and `dist2(pet, player) > 4`:
```
// Teleport pet to adjacent-to-player position
target = sgn(pet - player) + player
find nearest goodpos around target
move pet there
```

---

## 6. Pet Combat Behavior

### 6.1 Melee Attack Decision

A pet will attack an adjacent monster if ALL of the following are true:

```
fn will_attack(pet, target) -> bool:
    // Balk threshold based on HP fraction
    balk = pet.m_lev + (5 * pet.mhp / pet.mhpmax) - 2
    //  100% HP: balk = m_lev + 3  (attacks up to level m_lev+2)
    //   80% HP: balk = m_lev + 2  (attacks up to level m_lev+1)
    //   60% HP: balk = m_lev + 1  (attacks up to level m_lev)
    //   40% HP: balk = m_lev + 0  (attacks up to level m_lev-1)
    //   25% HP: balk = m_lev - 1  (attacks up to level m_lev-2, won't attack peacefuls)
    //   20% HP: balk = m_lev - 1  (attacks up to level m_lev-3)

    if target.m_lev >= balk: return false
    if target.mtame and pet.mtame and not Conflict: return false
    if max_passive_dmg(target, pet) >= pet.mhp: return false
    if (pet.mhp*4 < pet.mhpmax or target is quest leader/guardian)
       and target.mpeaceful and not Conflict: return false

    // Special monster avoidance (melee only)
    if target is floating eye and pet can see (mcansee) and pet has eyes (haseyes)
       and target is visible to pet (!target.minvis or pet perceives invisible)
       and pet not reflecting: 9/10 avoid
    if target is gelatinous cube: 9/10 avoid
    if target touch-petrifies and pet not resistant: always avoid
```

### 6.2 Ranged Attack Decision (`pet_ranged_attk()`)

Pet considers ranged attacks (breath weapons, gaze, spit) when:
- Pet is not blind
- Valid target along a cardinal/diagonal line (up to 7 squares)
- Target scores positively (see `score_targ()`)
- If hungry, only 1/5 chance of using breath/spit

### 6.3 Target Scoring (`score_targ()`)

```
fn score_targ(pet, target) -> long:
    if pet.mconf and rn2(3): return score  // mostly random when confused

    // Absolute vetoes (return large negative)
    if target is quest leader or guardian: return -5000
    if same-alignment priest/minion and target peaceful: return -5000
    if target is adjacent: return -3000  // don't breathe on adjacent
    if target is pet or player: return -3000
    if friend (pet/player) behind target within 15 squares: return -3000

    // Scoring factors
    if target is hostile: score += 10
    if target is passive (AT_NONE): score -= 1000
    if target too weak relative to pet: score -= 25
    if target much stronger (m_lev > pet_lev + 4):
        score -= (target.m_lev - pet_lev) * 20

    // Prefer beefier targets (but not enough to override aversion)
    score += target.m_lev * 2 + target.mhp / 3
    score += rnd(5)  // fuzz factor

    if pet.mconf and not rn2(3): score -= 1000
    return score
```

### 6.4 Return Attacks

After a pet hits a monster in melee:
```
if hit but didn't kill and rn2(4) and target hasn't moved this turn
   and pet position not scary to target and target is near pet:
    target counterattacks pet
```

---

## 7. Pet Item Handling

### 7.1 Inventory Management (`dog_invent()`)

Pets can carry items. Each turn, if carrying droppable items:
```
if not rn2(udist + 1) or not rn2(apport):
    if rn2(10) < apport:
        drop all droppable items
        if apport > 1: apport -= 1
        record dropdist = udist, droptime = moves
```

### 7.2 Item Pickup

If pet has no droppable items and is on a tile with items:
```
for each item at pet's position:
    if item is cursed: note cursed
    elif edible with quality <= CADAVER (or == ACCFOOD if starving [mhpmax_penalty]):
        eat it immediately
    elif rn2(20) < apport + 3:
        if rn2(udist) or not rn2(apport):
            pick up item
```

### 7.3 Items Pets Keep (`droppables()`)

Intelligent pets keep useful tools and will not drop them:
- **Pick-axe / Dwarvish mattock**: if can tunnel (one per pet)
- **Unicorn horn**: uncursed only (one per pet, prefer artifact)
- **Skeleton key > Lock pick > Credit card**: if can open doors (one per pet)
- **Wielded weapon**: never dropped

Animals and mindless creatures don't keep any tools.

### 7.4 Apport Training

When pet eats DOGFOOD that the player dropped/threw (detected by `obj.invlet != 0`):
```
apport += 200 / (dropdist + moves - droptime)
```

This rewards:
- Short distances between drop and eat (small `dropdist`)
- Quick retrieval (small `moves - droptime`)

### 7.5 No-Fetch Items

Pets never pick up: balls, chains, rocks/boulders (class `BALL_CLASS`, `CHAIN_CLASS`, `ROCK_CLASS`).

---

## 8. Taming Methods

### 8.1 Thrown Food (`tamedog()`)

Throwing food at a wild monster:
```
fn tamedog(mtmp, obj):
    // Untameable monsters
    if mtmp is Wizard or Medusa or wants-artifact: return false
    if mtmp.isshk: pacify only, don't tame
    if mtmp is immobile, guard, priest, minion, covetous,
       human, or demon (unless hero is demon): return false
    if mtmp is quest leader: return false
    if dogfood(mtmp, obj) >= MANFOOD: return false  // won't eat it

    // Already tame: catch and eat food
    if mtmp.mtame and obj:
        if awake and not confused and not eating
           and (food is DOGFOOD or (food <= ACCFOOD and hungry)):
            pet catches and eats thrown food
            return true

    // New taming
    allocate edog, call initedog(everything=true)
    place food on floor, dog_eat with devour=true
```

### 8.2 Scroll of Taming / Spell of Charm Monster

- **Uncursed scroll**: affects adjacent 3x3 area (radius 1)
- **Confused scroll/spell**: affects 11x11 area (radius 5)
- **Cursed scroll**: makes monsters angry instead
- Affected monsters get `resist()` check; if they fail, `tamedog()` is called with scroll as obj parameter
- Shopkeepers: `make_happy_shk()` called but never truly tamed
- Already-tame pets with `mtame < 10`: chance of +1 tameness (if `mtame < rnd(10)`), blessed scroll gives +2 (capped at 10)

### 8.3 Magic Trap (case 19)

Rare magic trap effect (1/21 chance when stepping on magic trap):
```
adjattrib(A_CHA, 1)   // +1 Charisma
for each adjacent monster (3x3 area):
    tamedog(mtmp, NULL)  // no food, just magic
```

### 8.4 Figurines

Applying or breaking a figurine calls `make_familiar()`:
```
fn make_familiar(figurine, x, y):
    pm = figurine.corpsenm  // specific monster type
    create monster with MM_EDOG flag

    // Taming chance for figurines
    chance = rn2(10)   // 0..9
    if chance > 2:
        // 70% determined by BUC
        if blessed: chance = 0 (tame)
        elif uncursed: chance = 1 (peaceful)
        elif cursed: chance = 2 (hostile)
    // Raw distribution: 10% always tame + 70% * BUC-dependent
    // Blessed:  80% tame, 10% peaceful, 10% hostile
    // Uncursed: 10% tame, 80% peaceful, 10% hostile
    // Cursed:   10% tame, 10% peaceful, 80% hostile
```

### 8.5 Create Familiar Spell

If no figurine, `make_familiar(NULL, ...)`:
```
if rn2(3) == 0:   // 1/3 chance
    pm = pet_type()   // standard dog/cat/pony
else:              // 2/3 chance
    // random monster based on spell skill
    max_level = 3 * P_SKILL(skill)
    pm = rndmonst_adj(0, max_level)
```

---

## 9. Going Feral / Becoming Hostile

### 9.1 Revival (`wary_dog()`)

Called when a pet is revived (life-saving, revival):

```
fn wary_dog(mtmp, was_dead):
    // Undo starvation penalty
    if edog.mhpmax_penalty:
        mtmp.mhpmax += edog.mhpmax_penalty
        mtmp.mhp += edog.mhpmax_penalty
        edog.mhpmax_penalty = 0

    if edog.killed_by_u == 1 or edog.abuse > 2:
        // Killed by hero or heavily abused: goes wild
        mtmp.mpeaceful = 0
        mtmp.mtame = 0
        // Chance to stay peaceful based on abuse level
        if edog.abuse >= 0 and edog.abuse < 10:
            if not rn2(edog.abuse + 1):
                mtmp.mpeaceful = 1
    else:
        // Pet Sematary: random chance to go wild
        mtmp.mtame = rn2(mtmp.mtame + 1)
        // e.g., tameness 10: 10/11 chance to stay tame (as 1..10)
        //                     1/11 chance of tameness 0
        if mtmp.mtame == 0:
            mtmp.mpeaceful = rn2(2)  // 50% peaceful, 50% hostile

    // If still tame after revival
    if mtmp.mtame:
        edog.revivals += 1
        edog.killed_by_u = 0
        edog.abuse = 0
        if was_dead or edog.hungrytime < moves + 500:
            edog.hungrytime = moves + 500
        if was_dead:
            edog.droptime = 0
            edog.dropdist = 10000
            edog.whistletime = 0
            edog.apport = 5
        // else: lifesaved, retain current apport/drop/whistle values
```

### 9.2 Separation Decay

See section 3.3 -- tameness decreases by 1 per 150 turns away. Can go hostile.

### 9.3 Starvation While Away

If pet is carnivorous/herbivorous and moves > hungrytime + 750, goes feral and hostile.

### 9.4 Conflict

While Conflict is active:
- Pets may attack the hero (checked in `dog_move()`)
- Guardian angels disappear and send nasties instead
- `abuse_dog()` with Conflict halves tameness instead of -1
- Steeds throw their rider (`dismount_steed(DISMOUNT_THROWN)`)

---

## 10. Pet Displacement (Swapping Positions)

When the hero walks into a tame monster's square, positions are swapped.

### 10.1 Swap Conditions (`domove_swap_with_pet()`)

Swap is **blocked** if:
- Pet is pinned in a pit by a boulder
- Diagonal move and pet can only move cardinally (NODIAG)
- Boulder at hero's position and pet too large/loaded to share (`>600` load)
- Diagonal squeeze and pet too large/loaded to fit
- Pet is trapped (peaceful or tame) in a pit/bear trap/web
- Destination for pet is unsafe (`!goodpos`), has a trap, or pet is `mundisplaceable` (shopkeeper, priest, guard, Oracle, quest leader)

### 10.2 Swap Execution

```
remove monster from (x, y)
place monster at hero's old position (ux0, uy0)
hero moves to (x, y)
message: "You swap places with your <pet>."
```

---

## 11. Leash Mechanics

### 11.1 Leash Application (`use_leash()`)

- Maximum 2 leashed pets at once (`MAXLEASHED = 2`)
- Target must be tame, adjacent, and leashable
- `leashable()`: not a long worm, not unsolid, must have limbs or a head
- Leash object stores `leashmon = mtmp.m_id`
- Monster gets `mleashed = 1`

### 11.2 Leash Constraints

During movement:
- Leashed pet skips any position where `dist2(pos, player) > 4` (i.e., must stay within sqrt(4) = 2 squares)
- If leashed pet didn't move and is too far, snap-back teleport to adjacent position

### 11.3 Leash Breaking

- Pet becomes untame: leash released
- Pet dies: leash released
- Leashed pet can't follow to new level: leash goes slack, `mtame -= 1`
- Pet shapeshifts to unleashable form: leash falls off
- Hero and pet too far apart (dist > 5): leash snaps loose
- Pulling: if dist > 4, hero pulls on leash; if dist > 5, leash snaps

### 11.4 Leash Interaction with Movement

Leashed pets:
- Are forced to follow (approach=1 in `dog_goal()`)
- Whimper near traps instead of avoiding them
- Don't use backtrack-avoidance heuristic

---

## 12. Saddle and Riding

### 12.1 Saddling (`use_saddle()`)

**Saddleable monsters** (`can_saddle()`):
- Monster class in: `S_QUADRUPED`, `S_UNICORN`, `S_ANGEL`, `S_CENTAUR`, `S_DRAGON`, `S_JABBERWOCK`
- Size >= MZ_MEDIUM
- Not humanoid (exception: centaurs)
- Not amorphous, noncorporeal, whirly, or unsolid

**Saddle success chance**:
```
chance = DEX + CHA/2 + 2*mtame + ulevel*(if tame: 20, else: 5)
if not tame: chance -= 10 * monster_level
if Knight: chance += 20
match riding_skill:
    Unskilled/Restricted: chance -= 20
    Basic: +0
    Skilled: chance += 15
    Expert: chance += 30
if Confused or Fumbling or Glib: chance -= 20
elif wearing "riding gloves": chance += 10
elif wearing "riding boots": chance += 10
if saddle cursed: chance -= 50

success = rn2(100) < chance
```

### 12.2 Mounting (`mount_steed()`)

Requirements:
- Monster is tame (not a minion)
- Monster has a saddle
- Monster is not trapped
- Hero is humanoid, not very small, not big
- Not underwater (unless mount swims)

**Knight bonus**: Knights never lose tameness from mounting. Non-Knights:
```
mtame -= 1
if mtame == 0: mount fails, "resists!"
```

**Mounting success check**:
```
if Confused or Fumbling or Glib or Wounded_legs or saddle_cursed
   or saddle_greased or (ulevel + mtame < rnd(MAXULEV/2 + 5)):
    // Slip and fall
    damage = rn1(5, 10)  // 10..14
```

### 12.3 Riding Behavior

While mounted:
- Steed is at hero's position (`u.usteed`)
- Steed does not move independently
- `dog_move()` returns immediately for steeds (with Conflict exception)
- Steed eats as normal (hunger still ticks)
- During Conflict: steed throws rider (`DISMOUNT_THROWN`)

### 12.4 Dismounting

Causes: voluntary (#ride again), falling, polymorph, steed death, Conflict, steed teleport.

---

## 13. Pet Growth

Pets grow through the standard monster growth system (`grow_up()`):

| Baby | Adult | Elder |
|---|---|---|
| PM_LITTLE_DOG | PM_DOG | PM_LARGE_DOG |
| PM_KITTEN | PM_HOUSECAT | PM_LARGE_CAT |
| PM_PONY | PM_HORSE | PM_WARHORSE |

Growth occurs when a pet kills a monster (same as wild monsters). Pet-specific cap: pet level must be < `data.mlevel + 15`.

---

## 14. Whistles

### 14.1 Tin Whistle (`use_whistle()`)

- Wakes nearby monsters
- No pet-specific effect
- Cursed: also summons vault guard

### 14.2 Magic Whistle (`use_magic_whistle()`)

- **Uncursed/Blessed**: teleports all tame monsters on the level to hero-adjacent positions
  - Frees trapped pets (clears `mtrapped`, fills pits)
  - Does NOT affect steeds
  - Sets `edog.whistletime = moves` (used for `whappr` in `dog_goal()`)
- **Cursed**: 50% chance of teleporting hero to a random pet instead; 50% chance of normal wake-nearby

### 14.3 Blessed Eucalyptus Leaf

Acts as a magic whistle. 1/49 chance to lose the blessing per use.

### 14.4 Whistle Approach Bonus

```
whappr = (moves - edog.whistletime < 5)
```
If `whappr` is true, pet is more eager to approach the player (forced approach in `dog_goal()`).

---

## 疑似 Bug

1. **[疑似 bug] `ranged_only` flag is set but then immediately causes `continue`**: In `dog_move()` at the combat evaluation loop, when a pet determines it should only use ranged attacks against a floating eye / gelatinous cube / petrifying monster, `ranged_only` is set to `TRUE`, but then the very next line `if (ranged_only) continue;` skips the position entirely. The code comment says `/** FIXME: 'ranged_only' isn't used as intended yet **/`. This means pets never attempt ranged attacks against dangerous melee targets they identified -- the feature is incomplete.

2. **[疑似 bug] Starvation penalty HP reduction can kill**: In `dog_hunger()`, after reducing mhpmax to 1/3 and clamping mhp, the code checks `if (DEADMONSTER(mtmp))` and calls `dog_starve()`. However, a monster with exactly 0 mhpmax/3 (e.g., mhpmax=2 -> new=0) would have mhp reduced to 0, triggering death from the starvation penalty calculation rather than from the DOG_STARVE timeout. The death message says "starves" which is correct but the timing is premature.

3. **[疑似 bug] `abuse_dog()` with Conflict halves tameness even for a single hit**: The `mtame /= 2` path is triggered whenever `Aggravate_monster || Conflict` is true, meaning a single accidental hit during Conflict can cut tameness from 10 to 5, while without Conflict it would only go from 10 to 9. This makes Conflict disproportionately punishing for pet loyalty.

4. **[疑似 bug] Mounting non-Knight decrements tameness before checking**: In `mount_steed()`, `--mtmp->mtame` is used in the `if` condition. If mtame was 1, it becomes 0 and the mount fails. But the tameness has already been decremented, so the pet is now feral regardless of whether the hero intended to actually ride. There is no undo.

---

## 测试向量

### TV-1: Starting pet selection (Knight)
- Input: role=Knight, preferred_pet=any
- Expected: `pet_type()` returns PM_PONY (role.petnum overrides preference)

### TV-2: Starting pet selection (Valkyrie, no preference)
- Input: role=Valkyrie (petnum=NON_PM), preferred_pet='\0'
- Expected: 50/50 PM_KITTEN or PM_LITTLE_DOG

### TV-3: initedog for domestic animal
- Input: dog (is_domestic=true), CHA=14
- Expected: mtame=10, apport=14, hungrytime=moves+1000

### TV-4: initedog for non-domestic tamed monster
- Input: winter wolf (is_domestic=false), CHA=10
- Expected: mtame=5, apport=10, hungrytime=moves+1000

### TV-5: Tameness decay from 150-turn separation
- Input: mtame=10, time_away=300
- Expected: wilder=(300+75)/150=2, mtame=10-2=8

### TV-6: Boundary -- tameness decay exactly at threshold
- Input: mtame=2, time_away=300 (wilder=2)
- Expected: mtame > wilder is false, so: rn2(2) check. If mtame > rn2(wilder=2): 50% chance mtame=0 (peaceful). Else mtame=0, mpeaceful=0 (hostile).

### TV-7: Boundary -- tameness decay with 0 turns away
- Input: mtame=5, time_away=0
- Expected: wilder=(0+75)/150=0, no tameness change

### TV-8: Abuse with Conflict active
- Input: mtame=10, Conflict=true, single hit
- Expected: mtame = 10/2 = 5, abuse incremented by 1

### TV-9: Abuse without Conflict
- Input: mtame=10, single hit
- Expected: mtame=9, abuse incremented by 1

### TV-10: Dog food nutrition (tiny monster eating corpse)
- Input: PM_KITTEN (MZ_SMALL) eats newt corpse (cnutrit=20)
- Expected: nutrit = 20 * 6 = 120 added to hungrytime

### TV-11: Starvation sequence
- Input: hungrytime=1000, current_moves=1501 (deficit=501, > DOG_WEAK=500)
- Expected: mhpmax_penalty set, mhpmax reduced to mhpmax/3, mconf=1

### TV-12: Starvation death
- Input: hungrytime=1000, current_moves=1751 (deficit=751, > DOG_STARVE=750), mhpmax_penalty already set
- Expected: pet dies (dog_starve called)

### TV-13: Boundary -- wary_dog revival with killed_by_u
- Input: edog.killed_by_u=1, edog.abuse=0
- Expected: mtame=0, mpeaceful = !rn2(0+1) = always 1 (peaceful) [since abuse=0, rn2(1)=0, so !0=true]

### TV-14: Wary_dog revival without abuse, high tameness
- Input: killed_by_u=0, abuse=0, mtame=10
- Expected: mtame = rn2(11), range 0..10. If 0: mpeaceful = rn2(2) = 50/50.

### TV-15: Boundary -- mount_steed non-Knight with mtame=1
- Input: role=Wizard, mtame=1, not forcing
- Expected: --mtame makes mtame=0, mount fails, pet becomes feral. [疑似 bug: no undo]

### TV-16: Apport training from eating player-thrown food
- Input: dogfood=DOGFOOD, obj.invlet!=0, dropdist=4, droptime=100, moves=110
- Expected: apport += 200 / (4 + 110 - 100) = 200/14 = 14

### TV-17: Saddle success chance (Knight, tame pony)
- Input: DEX=14, CHA=12, mtame=10, ulevel=5, Knight, Basic riding, no impairments, uncursed saddle
- Expected: chance = 14 + 6 + 20 + 100 + 20 + 0 = 160 -> clamped by rn2(100) -> always succeeds

### TV-18: Saddle success chance (Wizard, wild horse)
- Input: DEX=10, CHA=10, mtame=0, ulevel=3, horse m_lev=5, Unskilled, uncursed
- Expected: chance = 10 + 5 + 0 + 15 - 50 - 20 = -40 -> rn2(100) always > -40 -> always fails

### TV-19: Boundary -- max leash count
- Input: 2 pets already leashed, attempt to leash 3rd
- Expected: "You cannot leash any more pets." (MAXLEASHED=2)

### TV-20: Scroll of taming radius
- Input: uncursed scroll, not confused
- Expected: affects 3x3 area (bd=1), so positions (ux-1..ux+1, uy-1..uy+1)

### TV-21: Boundary -- scroll of taming, confused
- Input: confused, uncursed scroll
- Expected: affects 11x11 area (bd=5), so positions (ux-5..ux+5, uy-5..uy+5)

### TV-22: Combat balk threshold at full HP
- Input: pet m_lev=5, mhp=20, mhpmax=20, target m_lev=7
- Expected: balk = 5 + (5*20)/20 - 2 = 5+5-2 = 8. target.m_lev(7) < 8 -> attacks

### TV-23: Combat balk threshold at 20% HP
- Input: pet m_lev=5, mhp=4, mhpmax=20, target m_lev=3
- Expected: balk = 5 + (5*4)/20 - 2 = 5+1-2 = 4. target.m_lev(3) < 4 -> attacks

### TV-24: Boundary -- combat balk at exact threshold
- Input: pet m_lev=5, mhp=10, mhpmax=20, target m_lev=6
- Expected: balk = 5 + (5*10)/20 - 2 = 5+2-2 = 5. target.m_lev(6) >= 5 is TRUE -> refuses to attack
