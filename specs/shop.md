# Shop System — Mechanism Spec (NetHack 3.7)

Source: `src/shk.c` (6060 lines), `src/shknam.c`, `src/mkroom.c`,
`include/mextra.h` (struct eshk), `include/mkroom.h` (struct shclass),
`include/obj.h` (unpaid/no_charge bits), `include/monsters.h` (shopkeeper stats)

---

## 1. Shop Types

Shops are special rooms with `rtype >= SHOPBASE` (14). Each shop type is
defined in `shtypes[]` (shknam.c).

### 1.1 Randomly Generated Shops

| Index | rtype constant | Display Name | Annotation | Symbol Class | Prob% |
|-------|---------------|--------------|------------|-------------|-------|
| 0 | SHOPBASE (14) | general store | (same) | RANDOM_CLASS | 42 |
| 1 | ARMORSHOP (15) | used armor dealership | armor shop | ARMOR_CLASS | 14 |
| 2 | SCROLLSHOP (16) | second-hand bookstore | scroll shop | SCROLL_CLASS | 10 |
| 3 | POTIONSHOP (17) | liquor emporium | potion shop | POTION_CLASS | 10 |
| 4 | WEAPONSHOP (18) | antique weapons outlet | weapon shop | WEAPON_CLASS | 5 |
| 5 | FOODSHOP (19) | delicatessen | food shop | FOOD_CLASS | 5 |
| 6 | RINGSHOP (20) | jewelers | ring shop | RING_CLASS | 3 |
| 7 | WANDSHOP (21) | quality apparel and accessories | wand shop | WAND_CLASS | 3 |
| 8 | TOOLSHOP (22) | hardware store | tool shop | TOOL_CLASS | 3 |
| 9 | BOOKSHOP (23) | rare books | bookstore | SPBOOK_CLASS | 3 |
| 10 | FODDERSHOP (24) | health food store | vegetarian food shop | FOOD_CLASS | 2 |

Total probability: 100%.

### 1.2 Unique Shops (prob = 0, special level only)

| Index | rtype constant | Display Name | Annotation |
|-------|---------------|--------------|------------|
| 11 | CANDLESHOP (25) | lighting store | lighting shop |

`UNIQUESHOP == CANDLESHOP`. Shops at or above this index are never randomly
generated; they are only placed via the special level loader.

### 1.3 Item Probability Tables (iprobs)

Each shop type has up to 9 `(probability, type)` entries that total 100%.
Negative `itype` means a specific object enum; positive means a class.

```
general:    100% RANDOM_CLASS
armor:      90% ARMOR_CLASS, 10% WEAPON_CLASS
scroll:     90% SCROLL_CLASS, 10% SPBOOK_CLASS
potion:     100% POTION_CLASS
weapon:     90% WEAPON_CLASS, 10% ARMOR_CLASS
food:       83% FOOD_CLASS, 5% POT_FRUIT_JUICE, 4% POT_BOOZE,
            5% POT_WATER, 3% ICE_BOX
ring:       85% RING_CLASS, 10% GEM_CLASS, 5% AMULET_CLASS
wand:       90% WAND_CLASS, 5% LEATHER_GLOVES, 5% ELVEN_CLOAK
tool:       100% TOOL_CLASS
book:       90% SPBOOK_CLASS, 10% SCROLL_CLASS
health:     70% VEGETARIAN_CLASS, 20% POT_FRUIT_JUICE, 4% POT_HEALING,
            3% POT_FULL_HEALING, 2% SCR_FOOD_DETECTION, 1% LUMP_OF_ROYAL_JELLY
candle:     30% WAX_CANDLE, 44% TALLOW_CANDLE, 5% BRASS_LANTERN,
            9% OIL_LAMP, 3% MAGIC_LAMP, 5% POT_OIL, 2% WAN_LIGHT,
            1% SCR_LIGHT, 1% SPE_LIGHT
```

`VEGETARIAN_CLASS` is a pseudo-class handled by `veggy_item()`: any
FOOD_CLASS object with `oc_material == VEGGY`, plus eggs, plus vegetarian
tins/corpses.

### 1.4 Saleable Check

`saleable(shkp, obj)` returns TRUE if:
- Shop is general store (symb == RANDOM_CLASS): always TRUE
- Else: object matches any `iprobs[i]` entry (by class or specific otyp)
- VEGETARIAN_CLASS entries match via `veggy_item(obj, 0)`

---

## 2. Pricing Formulas

### 2.1 Base Price: `getprice(obj, shk_buying)`

```
base = objects[obj.otyp].oc_cost

if obj.oartifact:
    base = arti_cost(obj)    // artifact-specific base from artilist[]
    if shk_buying:
        base /= 4

switch obj.oclass:
    FOOD_CLASS:
        base += corpsenm_price_adj(obj)  // see §2.1.1
        if hero.hunger >= HUNGRY and not shk_buying:
            base *= hero.hunger_level     // HUNGRY=2, WEAK=3, FAINTING=4
        if obj.oeaten:
            base = 0
    WAND_CLASS:
        if obj.spe == -1:  // empty/cancelled
            base = 0
    POTION_CLASS:
        if obj.otyp == POT_WATER and not blessed and not cursed:
            base = 0
    ARMOR_CLASS, WEAPON_CLASS:
        if obj.spe > 0:
            base += 10 * obj.spe
    TOOL_CLASS:
        if Is_candle(obj) and obj.age < 20 * oc_cost:
            base /= 2   // partially-burned candle
```

#### 2.1.1 Corpse/Tin/Egg Price Adjustment: `corpsenm_price_adj(obj)`

For tins, eggs, and corpses with valid `corpsenm`:

```
tmp = 1
for each intrinsic in [FIRE_RES(2), SLEEP_RES(3), COLD_RES(2),
    DISINT_RES(5), SHOCK_RES(4), POISON_RES(2), ACID_RES(1),
    STONE_RES(3), TELEPORT(2), TELEPORT_CONTROL(3), TELEPAT(5)]:
    if intrinsic_possible(intrinsic, monster):
        tmp += cost

if unique_corpstat(monster):
    tmp += 50

val = max(1, (monster.mlevel - 1) * 2)
if CORPSE:
    val += max(1, monster.cnutrit / 30)

return val * tmp
```

### 2.2 Buy Price: `get_cost(obj, shkp)`

This is the per-unit price the shopkeeper charges the hero.

```
tmp = getprice(obj, FALSE)
if tmp == 0: tmp = 5   // minimum 5 zm for "worthless" items

multiplier = 1
divisor = 1

// Glass gem deception: unidentified glass gems priced as real gems
if not obj.dknown or not oc_name_known:
    if GEM_CLASS and GLASS material:
        // pseudorandom mapping to real gem prices
        pseudorand = (ubirthday % obj.otyp) >= (obj.otyp / 2)
        tmp = objects[mapped_gem].oc_cost
    else if oid_price_adjustment(obj, obj.o_id) > 0:
        // 25% of unidentified items get surcharge
        // (obj.o_id % 4 == 0)
        multiplier *= 4; divisor *= 3    // +33%

// Tourist / dunce cap penalty
if wearing DUNCE_CAP:
    multiplier *= 4; divisor *= 3        // +33%
else if (Tourist and ulevel < 15) or (any shirt visible: uarmu && !uarm && !uarmc):
    multiplier *= 4; divisor *= 3        // +33%

// Charisma modifier
if CHA > 18:   divisor *= 2             // -50%
if CHA == 18:  multiplier *= 2; divisor *= 3  // -33%
if CHA 16..17: multiplier *= 3; divisor *= 4  // -25%
if CHA 11..15: (no adjustment)           // base
if CHA 8..10:  multiplier *= 4; divisor *= 3  // +33%
if CHA 6..7:   multiplier *= 3; divisor *= 2  // +50%
if CHA <= 5:   multiplier *= 2           // +100%

// Apply with banker's rounding
tmp = tmp * multiplier
if divisor > 1:
    tmp = ((tmp * 10 / divisor) + 5) / 10

if tmp <= 0: tmp = 1   // minimum 1 zm

if obj.oartifact:
    tmp *= 4            // artifacts cost 4x more in shops

// Anger surcharge (applied separately)
if shkp.surcharge:
    tmp += (tmp + 2) / 3    // +33% (rounds up from thirds)

return tmp
```

#### 2.2.1 Charisma Table (Buy Price Multiplier)

| CHA | Effect | Effective Ratio |
|-----|--------|----------------|
| <=5 | x2/1 | 200% of base |
| 6-7 | x3/2 | 150% of base |
| 8-10 | x4/3 | 133% of base |
| 11-15 | x1/1 | 100% of base |
| 16-17 | x3/4 | 75% of base |
| 18 | x2/3 | 67% of base |
| >18 | x1/2 | 50% of base |

#### 2.2.2 OID Price Adjustment (Sucker Surcharge)

```
fn oid_price_adjustment(obj, oid) -> {-1, 0, +1}:
    if (obj.dknown and oc_name_known):
        return 0   // identified items: no adjustment
    if GEM_CLASS and GLASS material:
        return 0   // glass gems have their own deception
    return if (oid % 4 == 0) then 1 else 0
```

25% of unidentified non-glass items get a 4/3 surcharge. The `o_id`
is assigned at object creation time, so this is deterministic per item.

#### 2.2.3 Pricing Units: `get_pricing_units(obj)`

```
if obj.globby:
    unit_weight = objects[obj.otyp].oc_weight
    wt = obj.owt if > 0 else weight(obj)
    units = (wt + unit_weight - 1) / unit_weight   // ceiling division
else:
    units = obj.quan
```

**Total buy cost = `get_cost(obj, shkp) * get_pricing_units(obj)`**

### 2.3 Sell Price: `set_cost(obj, shkp)`

This is the per-unit price the shopkeeper pays the hero.

```
unit_price = getprice(obj, TRUE)   // shk_buying=TRUE (artifacts /4)
tmp = get_pricing_units(obj) * unit_price

multiplier = 1
divisor = 1

// Tourist / dunce cap: sell at worse rate
if wearing DUNCE_CAP:
    divisor *= 3        // sell for 1/3
else if (Tourist and ulevel < 15) or (any shirt visible: uarmu && !uarm && !uarmc):
    divisor *= 3        // sell for 1/3
else:
    divisor *= 2        // normal: sell for 1/2

// Unidentified gem special handling
if not obj.dknown or not oc_name_known:
    if GEM_CLASS and (GEMSTONE or GLASS material):
        tmp = ((obj.otyp - FIRST_REAL_GEM) % (6 - shkp.m_id % 3))
        tmp = (tmp + 3) * obj.quan
        divisor = 1     // override: no further division
    else if tmp > 1 and (shkp.m_id % 4 == 0):
        multiplier *= 3; divisor *= 4    // 25% of shks pay less

// Apply with banker's rounding
tmp *= multiplier
if divisor > 1:
    tmp = ((tmp * 10 / divisor) + 5) / 10
if tmp < 1: tmp = 1

// NOTE: no anger surcharge on sell price
return tmp
```

**Key difference from buy price:**
- Normal sell divisor is /2 (you get half the base price)
- Tourist/dunce sell divisor is /3 (you get a third)
- No Charisma modifier on sell price
- No artifact 4x multiplier on sell price (artifacts already /4 from `getprice`)

### 2.4 Credit-for-Sale (Shopkeeper Has No Cash)

When shopkeeper has no gold:

```
credit_offered = (offer * 9) / 10 + (offer <= 1 ? 1 : 0)
```

This is approximately 90% of the sell value, with minimum credit of 1.

### 2.5 Short Funds

When shopkeeper's gold < offer, the offer is capped at shopkeeper's gold:

```
if offer > shkmoney:
    offer = shkmoney
```

---

## 3. Buy/Sell Transaction Flow

### 3.1 Buying (Picking Up Shop Items)

When hero picks up an item on a shop floor (non-freespot):

1. `addtobill()` is called
2. Item is checked via `billable()` — must not be already billed, not eaten
   food, not `no_charge`, shopkeeper must exist and be in shop
3. Item is added to `eshk.bill_p[]` via `add_one_tobill()`
4. `obj.unpaid = 1` is set
5. Shopkeeper announces the price

### 3.2 Paying (`dopay`)

1. Hero must be inside shop or adjacent to shopkeeper
2. Debit (usage fees, picked-up gold) is paid first, using credit then gold
3. Bill items are paid via itemized or bulk flow
4. For each item: `dopayobj()` deducts `bp.price * quantity` from gold/credit
5. Upon full payment, shopkeeper thanks the hero

### 3.3 Selling (Dropping Items in Shop)

`sellobj()` flow:

1. Check item is saleable and has value
2. If shopkeeper is angry: takes item without paying ("Thank you, scum!")
3. If `eshk.robbed > 0` (bones level): item value reduces robbed amount
4. If shopkeeper has gold: offers `set_cost()` price
5. If shopkeeper has no gold: offers credit at 90% of sell value
6. If `sell_how == SELL_NORMAL` (accidental drop): auto-accepts
7. If deliberate: prompts hero with y/n/a/q

---

## 4. Shopkeeper Data Structure

```
struct eshk {
    robbed: long     // total stolen by most recent customer
    credit: long     // hero's credit at this shop
    debit: long      // hero's debt for usage fees
    loan: long       // gold picked up from shop floor (subset of debit)
    shoptype: int    // rooms[shoproom].rtype
    shoproom: schar  // index in rooms[]
    following: bool  // chasing a shoplifter
    surcharge: bool  // anger price increase active
    shk: coord       // shopkeeper's home position (inside shop, near door)
    shd: coord       // shop door position
    billct: int      // number of active bill entries
    bill[BILLSZ]: bill_x  // BILLSZ = 200
    visitct: int     // number of visits by current customer
    customer: char[PL_NSIZ]  // name of most recent customer
    shknam: char[PL_NSIZ]    // shopkeeper's name
}

struct bill_x {
    bo_id: uint      // object's o_id
    useup: bool      // completely used up
    price: long      // price per unit at time of billing
    bquan: long      // original quantity when billed
}
```

### 4.1 Object Flags

- `obj.unpaid`: set on items in hero's inventory owned by shop
- `obj.no_charge`: set on shop floor items not for sale (hero-dropped, etc.)

---

## 5. Shopkeeper Behavior

### 5.1 Monster Stats

From `include/monsters.h`:

```
Name: "shopkeeper"
Symbol: S_HUMAN ('@')
Level: 12, Speed: 16, AC: 0, MR: 50%, Alignment: 0
Attacks: 2x weapon (4d4 physical)
Weight: WT_HUMAN, Nutrition: 400, Sound: MS_SELL, Size: MZ_HUMAN
Flags: M2_NOPOLY | M2_HUMAN | M2_PEACEFUL | M2_STRONG | M2_COLLECT | M2_MAGIC
Generation: G_NOGEN (never random; only from shkinit)
Difficulty: 15
```

Key: speed 16 (was 18 in 3.6; reduced to make haste less oppressive when
blocking doors). M2_MAGIC grants magic resistance/effects. M2_COLLECT
means will pick up items.

### 5.2 Initial Gold

```
mkmonmoney(shk, 1000 + 30 * rnd(100))   // range: 1030..4000
```

### 5.3 Initial Inventory

- Ring shops: shopkeeper gets a touchstone
- Tool/wand shops: shopkeeper gets a scroll of charging
- Ring shops: 50% chance of scroll of charging
- General stores: 20% chance of scroll of charging

### 5.4 Greeting on Entry

```
if hero is invisible:
    "Invisible customers are not welcome!"
elif shopkeeper is angry:
    "So, <name>, you dare return to <shop>?"
elif surcharge flag set (watched):
    "Back again, <name>?  I've got my eye on you."
elif robbed flag set:
    "<shk> mutters imprecations against shoplifters."
else:
    "<Hello>, <name>!  Welcome [again] to <shk>'s <shop type>!"
```

### 5.5 Door Blocking

When hero enters from the door (not teleported inside), the shopkeeper may
block if hero is carrying:
- A pick-axe or dwarvish mattock
- Riding a steed

If hero has `Fast` property and there is a pick-axe/mattock on the
doorway square, the shopkeeper also blocks.

When blocking, `dochug(shkp)` gives the shopkeeper an extra move.

### 5.6 Following

`ESHK.following = 1` is set via `hot_pursuit()`. The shopkeeper will chase
the hero out of the shop, potentially across the level.

### 5.7 Waking Up

`rouse_shk()`: when hero owes money and attempts to pay, or when hero drops
items, sleeping/paralyzed shopkeepers wake up immediately ("greed induced
recovery").

---

## 6. Shoplifting Mechanics

### 6.1 Leaving With Unpaid Items

When hero leaves the shop boundary while owing money:

1. **On boundary (not fully left):** Shopkeeper warns:
   - Peaceful: "Please pay before leaving."
   - Surcharge: "Don't you leave without paying!"

2. **Fully left (newlev or past edge):** `rob_shop()` is called

### 6.2 `rob_shop()` Logic

```
total = addupbill(shkp) + eshk.debit
if eshk.credit >= total:
    "Your credit is used to cover your shopping bill."
    setpaid()
    return FALSE  // no actual robbery
else:
    total -= eshk.credit
    "You escaped the shop without paying!"
    setpaid()
    eshk.robbed += total
    "You stole <total> zm worth of merchandise."
    if not Rogue: adjalign(-sgn(alignment))
    hot_pursuit(shkp)
    return TRUE  // triggers call_kops()
```

### 6.3 Calling the Kops

`call_kops()` sounds an alarm, calls `angry_guards()`, then spawns Keystone
Kops if any are available (not genocided).

Kop spawning via `makekops(coord)`:

```
cnt = abs(depth) + rnd(5)
Keystone Kop:     cnt        spawns
Kop Sergeant:     cnt/3 + 1  spawns (at least 1)
Kop Lieutenant:   cnt/6      spawns
Kop Kaptain:      cnt/9      spawns
```

Kops are spawned at:
- **Near shop (just stepped out):** around hero's position
- **Far from shop:** near down staircase AND near shopkeeper

#### Kop Stats

| Monster | Lvl | Spd | AC | MR | Attack | Diff |
|---------|-----|-----|----|----|--------|------|
| Keystone Kop | 1 | 6 | 10 | 10% | 1d4 weapon | 3 |
| Kop Sergeant | 2 | 8 | 10 | 10% | 1d6 weapon | 4 |
| Kop Lieutenant | 3 | 10 | 10 | 20% | 1d8 weapon | 5 |
| Kop Kaptain | 4 | 12 | 10 | 20% | 2d6 weapon | 6 |

All Kops: `G_GENO | G_NOGEN`, humanoid, hostile, human, male, collect.

### 6.4 Remote Burglary

`remote_burglary(x, y)` is called when hero uses telekinesis or grappling
hook to grab items from inside a shop without entering. Triggers `rob_shop()`
and `call_kops(FALSE)`.

---

## 7. Anger and Pacification

### 7.1 `rile_shk()` — Making Shopkeeper Angry

```
shkp.mpeaceful = FALSE
if not eshk.surcharge:
    eshk.surcharge = TRUE
    for each bill entry bp:
        surcharge = (bp.price + 2) / 3
        bp.price += surcharge        // +33% rounded up
```

### 7.2 `pacify_shk()` — Calming Shopkeeper

```
shkp.mpeaceful = TRUE
if clear_surcharge and eshk.surcharge:
    eshk.surcharge = FALSE
    for each bill entry bp:
        reduction = (bp.price + 3) / 4
        bp.price -= reduction        // undo the +33%
```

[疑似 bug] The surcharge adds `(price+2)/3` and pacify removes
`(price+3)/4`. Due to integer rounding, these are not exact inverses.
After rile+pacify, some prices may differ by 1 zm from their original value.
Example: original price 10 -> rile adds (10+2)/3=4 -> new price 14 -> pacify
removes (14+3)/4=4 -> restored to 10. But for price 5: rile adds
(5+2)/3=2 -> 7, pacify removes (7+3)/4=2 -> 5. Works out in most cases,
but edge case: price 2 -> rile (2+2)/3=1 -> 3, pacify (3+3)/4=1 -> 2. OK.
Price 1 -> rile (1+2)/3=1 -> 2, pacify (2+3)/4=1 -> 1. OK.
Actually, algebraically: after rile, price_new = price + ceil(price/3).
After pacify, price_restored = price_new - ceil(price_new/4). The comment
says "undo 33% increase" but the formula is price_new * 3/4 (approximately),
which is the correct inverse of 4/3.

### 7.3 `hot_pursuit()`

```
rile_shk(shkp)
eshk.customer = plname
eshk.following = 1
// clear no_charge on ALL objects on current level floor
clear_no_charge(NULL, fobj)
clear_no_charge_pets(shkp)
```

### 7.4 `make_angry_shk()`

Called when shopkeeper is teleported/falls out of shop, or player damages
shop structure from outside. Converts all pending transactions to robbery:

```
eshk.robbed += addupbill() + eshk.debit + eshk.loan - eshk.credit
if eshk.robbed < 0: eshk.robbed = 0
setpaid()    // clears bill, credit, debit, loan
hot_pursuit()
```

### 7.5 Paying Off Angry Shopkeeper

When `dopay()` is called with an angry shopkeeper:

- **If robbed > 0:** Hero can pay `robbed` amount. If hero pays >= half,
  shopkeeper becomes happy (`make_happy_shk`).
- **If robbed == 0 (attacked/damaged):** Hero must pay 1000 zm.
  2/3 chance of success if shopkeeper recognizes the customer.
  1/3 chance shopkeeper stays angry even after payment.
- **Minimum gold check:** Hero needs >= `ltmp/2` gold (or >= 1000 for
  non-robbery anger) to even attempt payment.

---

## 8. Credit System

### 8.1 Gaining Credit

Credit accumulates via:

1. **Selling items:** `sellobj()` — sell price goes to hero as gold (or
   credit if shk has no cash)
2. **Dropping gold in shop:** `donate_gold()` — gold exceeding `debit`
   becomes credit
3. **Credit-for-sale:** When shopkeeper has no gold, offers
   `(sell_price * 9/10) + (sell_price <= 1 ? 1 : 0)` as credit

### 8.2 Using Credit

`check_credit(amount, shkp)`:

```
if eshk.credit >= amount:
    "The price is deducted from your credit."
    eshk.credit -= amount
    return 0       // nothing left to pay
else:
    "The price is partially covered by your credit."
    remaining = amount - eshk.credit
    eshk.credit = 0
    return remaining
```

Credit is applied before gold for all payments (buying items, paying for
damage, paying off debts).

### 8.3 Credit Reset

`setpaid()` clears: `billct = 0, credit = 0, debit = 0, loan = 0`.
Called when hero pays in full, shopkeeper dies, or robbery is processed.

---

## 9. Damage to Shop Inventory

### 9.1 Usage Fees: `cost_per_charge()`

Using unpaid charged items incurs a per-use debit:

| Item | Fee Formula |
|------|-------------|
| Magic lamp (as light) | `objects[OIL_LAMP].oc_cost` (fixed) |
| Magic lamp (release djinni) | `get_cost() + get_cost()/3` |
| Magic marker | `get_cost() / 2` |
| Bag of tricks / Horn of plenty (per use) | `get_cost() / 5` |
| Bag of tricks / Horn of plenty (empty all) | `get_cost()` (full) |
| Crystal ball, oil lamp, brass lantern, musical instruments, wands (spe>1) | `get_cost() / 4` |
| Spellbook | `get_cost() - get_cost()/5` (= 4/5 of cost) |
| Can of grease, tinning kit, expensive camera | `get_cost() / 10` |
| Potion of oil | `get_cost() / 5` |

The fee is added to `eshk.debit` (not the bill).

### 9.2 Breakage and Destruction

When shop items are destroyed (e.g., broken potions, burnt scrolls):

1. `bill_dummy_object()` creates a dummy object on `billobjs` list with
   `useup = TRUE`
2. The item remains on the bill at its original price
3. Hero must pay for it during `dopay()`

### 9.3 Structure Damage: `pay_for_damage()`

When hero damages shop structure (digging walls, breaking doors):

1. `add_damage(x, y, cost)` records the damage
2. When shopkeeper notices (same turn): `pay_for_damage()` is called
3. Shopkeeper demands payment for `cost_of_damage`
4. If hero can pay and agrees: gold is transferred, shopkeeper is mollified
5. If hero refuses or can't pay: `hot_pursuit()` — shopkeeper becomes angry

Payment conditions checked:
```
if (distance > 1 and not in shop) or cant_mollify
   or (gold + credit < cost_of_damage)
   or !rn2(50):      // 2% chance of refusing payment even if offered
    -> anger (hot_pursuit)
```

[疑似 bug] The `!rn2(50)` check means there is a 2% chance the shopkeeper
will refuse to accept payment and become angry, even when the hero has
enough gold and is willing to pay. This seems intentional for flavor but
could be frustrating.

### 9.4 Shop Repair

Shopkeepers repair damage after `REPAIR_DELAY = 5` turns minimum.
`shk_fixes_damage()` / `repair_damage()` restore walls, doors, and floor.

---

## 10. Robbing Shops

### 10.1 Teleporting Out

Teleporting out of a shop with unpaid items triggers `u_left_shop()` ->
`rob_shop()` -> `call_kops()`.

### 10.2 Digging Out

Digging through shop walls triggers `pay_for_damage("dig into")`.
If hero escapes with unpaid items, the normal shoplifting flow applies.

### 10.3 Levelporting

Changing levels while in a shop with unpaid items triggers `u_left_shop()`
with `newlev = TRUE`.

### 10.4 Stealing from Outside

Using telekinesis or grappling hook from outside: `remote_burglary()`.

### 10.5 Pet Theft

Items carried by pets out of the shop: `stolen_value()` adds to `eshk.debit`
(peaceful) or `eshk.robbed` (angry). If shopkeeper sees it: "You are a
thief!" and `hot_pursuit()`.

---

## 11. Shop Generation

### 11.1 Level Eligibility

`mkshop()` in mkroom.c selects a room to become a shop:

Requirements:
- Room type must be OROOM (ordinary)
- No upstairs or downstairs in the room
- Exactly 1 door (or wizard mode with any doorcount)
- Valid shop shape (shopkeeper must have at least 2 squares to move between)
- Room is lit after conversion (all squares set to `lit = 1`)

### 11.2 Shop Shape Validation: `invalid_shop_shape()`

A room is invalid for a shop if the shopkeeper standing just inside the door
can only move to 1 other square (would get permanently stuck).

### 11.3 Big Room Restriction

Rooms with area > 20 cannot become wand or book shops. They default to
general stores instead.

### 11.4 Stocking

`stock_room()` in shknam.c:

1. Create shopkeeper via `shkinit()`:
   - Place shopkeeper at position adjacent to door, inside the room
   - Generate gold and optional items
2. Ensure door is not trapped; if locked, add "Closed for inventory" engraving
3. For book/scroll shops: 1 guaranteed novel (tribute book)
4. For each valid floor square (not adjacent to door row): call `mkshobj_at()`
   - `depth` % chance of placing a mimic instead of an item
   - Otherwise, select item type from shop's `iprobs[]` table

### 11.5 Orcus Town Exception

On Orcus's level, after stocking, the shopkeeper is removed (`mongone()`),
leaving a "deserted" shop.

---

## 12. One-of-a-Kind Items in Shops

### 12.1 Candelabrum of Invocation

`special_stock()`: The lighting shop (Izchak's) refuses to buy the
Candelabrum of Invocation. If Izchak is present and invocation hasn't
happened, he advises keeping it and mentions how many more candles are needed.

### 12.2 Novel / Tribute Book

Book and scroll shops guarantee one `SPE_NOVEL` placement via
`svc.context.tribute.bookstock`.

### 12.3 Artifacts

All artifacts can appear in shops. Their buy price is:
`get_cost() * 4` (the artifact multiplier in `get_cost()`), applied on top
of `arti_cost()` base.

Their sell price uses `arti_cost() / 4` as the base (from `getprice()` with
`shk_buying=TRUE`), then `/2` for normal sell divisor.

---

## 13. Special Shopkeeper: Izchak

The Minetown lighting shop always has a shopkeeper named "Izchak" (male).
Checked via `is_izchak(shkp, TRUE)`.

Izchak has unique dialog when chatted with (9 possible Izchak_speaks[]
messages). He also has special behavior for the Candelabrum (see §12.1).

---

## 14. Shopkeeper Names

Each shop type has a regional name list (shknam.c):

| Shop Type | Region | Array |
|-----------|--------|-------|
| General | Suriname, Greenland, Canada, Iceland | shkgeneral |
| Armor | Turkey | shkarmors |
| Scroll | Ireland | shkbooks (shared) |
| Potion | Ukraine, Belarus, Russia, etc. | shkliquors |
| Weapon | Perigord (France) | shkweapons |
| Food | Indonesia | shkfoods |
| Ring | Netherlands, Scandinavia | shkrings |
| Wand | Wales, Scotland | shkwands |
| Tool | Backwards spellings | shktools |
| Book | Ireland | shkbooks (shared) |
| Health food | Tibet, hippie names | shkhealthfoods |
| Lighting | Romania, Bulgaria | shklight |

Name prefix codes:
- `-` female personal name
- `_` female general name
- `+` male personal name
- `|` male general name
- `=` gender-unspecified personal name
- (no prefix) male general name (implied for most shktools)

Personal names do not receive the "Mr." or "Ms." honorific prefix.

---

## 15. Bill Management Details

### 15.1 Bill Size Limit

`BILLSZ = 200`. If the bill is full, hero gets the item for free:
"You got that for free!"

### 15.2 Bill Dummy Objects

When an unpaid item is completely consumed (eaten, drunk, read to dust),
`bill_dummy_object()` creates a replacement object on the `billobjs` chain
with `useup = TRUE` and `where = OBJ_ONBILL`.

### 15.3 Partly Used Items

If `obj.quan < bp.bquan`, the item is partly used. During payment:
- Used portion must be paid first (pass 0)
- Intact portion is paid next (pass 1)
- Shopkeeper rejects buying the intact portion until the used portion is paid

### 15.4 Containers

Picking up a container in a shop bills both the container and its non-no_charge
contents. Containers with unknown contents (cknown == 0) are treated as
"undisclosed containers" during billing.

---

## 16. Miscellaneous

### 16.1 Bones Level Shops

When entering a bones level shop that was previously robbed (`eshk.robbed > 0`):
- Dropping/selling items reduces `eshk.robbed` instead of earning gold/credit
- Shopkeeper thanks you for "restocking the recently plundered shop"

### 16.2 Shopkeeper Death

`shkgone()`:
- Clears `sroom.resident`
- Removes `no_charge` from all items on shop floor
- Removes shop from `u.ushops`
- Calls `setpaid()` to clear bill

### 16.3 Glob Pricing

Globs (pudding/ooze/slime) are priced by weight, not by quantity (which is
always 1). `get_pricing_units()` computes
`ceil(owt / objects[otyp].oc_weight)`.

When a glob is added to the bill, its price includes the weight factor, and
`OMID(obj) = obj.owt` records the weight at billing time.

### 16.4 Eaten Food

Partly eaten food (`obj.oeaten > 0`): `getprice()` returns 0 for food with
`oeaten` set. Shopkeeper is "uninterested" in buying partially eaten food.

### 16.5 Burned Candles

Candles with `age < 20 * oc_cost`: base price halved. Also, shopkeeper
refuses to buy candles below this threshold.

---

## 测试向量

### Basic Buy Price

| # | obj.oc_cost | CHA | Tourist? | Dunce? | Identified? | o_id%4 | Artifact? | Surcharge? | Expected get_cost() |
|---|-----------|-----|----------|--------|-------------|--------|-----------|-----------|-------------------|
| 1 | 100 | 12 | No | No | Yes | N/A | No | No | 100 |
| 2 | 100 | 12 | No | No | No | 0 | No | No | 133 |
| 3 | 100 | 12 | Yes(lv1) | No | Yes | N/A | No | No | 133 |
| 4 | 100 | 18 | No | No | Yes | N/A | No | No | 67 |
| 5 | 100 | 19 | No | No | Yes | N/A | No | No | 50 |
| 6 | 100 | 5 | No | No | Yes | N/A | No | No | 200 |
| 7 | 100 | 7 | No | No | Yes | N/A | No | No | 150 |
| 8 | 100 | 12 | No | No | Yes | N/A | Yes | No | 400 |
| 9 | 100 | 12 | No | No | Yes | N/A | No | Yes | 134 |
| 10 | 100 | 5 | Yes(lv1) | No | No | 0 | No | Yes | 475 |

#### Computation for #10
```
base = 100
unid surcharge: multiplier=4, divisor=3
tourist surcharge: multiplier*=4, divisor*=3 -> multiplier=16, divisor=9
CHA=5: multiplier*=2 -> multiplier=32, divisor=9
tmp = 100 * 32 = 3200
tmp = (3200 * 10 / 9 + 5) / 10 = (3555 + 5) / 10 = 356
artifact: no
anger surcharge: tmp += (356 + 2) / 3 = 119 -> 356 + 119 = 475
```

Derivation: 100 * 32 = 3200. 3200*10 = 32000. 32000/9 = 3555 (integer).
3555+5 = 3560. 3560/10 = 356. Surcharge: (356+2)/3 = 358/3 = 119.
356+119 = 475.

### Sell Price

| # | obj.oc_cost | Tourist? | Dunce? | Identified? | Expected set_cost() for quan=1 |
|---|-----------|----------|--------|-------------|------------------------------|
| 11 | 100 | No | No | Yes | 50 |
| 12 | 100 | Yes(lv1) | No | Yes | 33 |
| 13 | 100 | No | Yes | Yes | 33 |

#### Computation for #11
```
unit_price = getprice(obj, TRUE) = 100  (non-artifact)
tmp = 1 * 100 = 100
divisor = 2 (normal)
tmp = (100 * 10 / 2 + 5) / 10 = (500 + 5) / 10 = 50
```

#### Computation for #12
```
unit_price = 100
tmp = 100
divisor = 3 (tourist)
tmp = (100 * 10 / 3 + 5) / 10 = (333 + 5) / 10 = 33
```

### Kop Spawning

| # | Depth | rnd(5) | Kops | Sgts | Lts | Kpts |
|---|-------|--------|------|------|-----|------|
| 14 | 5 | 3 | 8 | 3 | 1 | 0 |
| 15 | 20 | 1 | 21 | 8 | 3 | 2 |

### Boundary Conditions

| # | Scenario | Expected |
|---|----------|----------|
| 16 | oc_cost=0 item (uncursed unholy water) | get_cost returns 5 (minimum) |
| 17 | BILLSZ items already on bill, pick up another | "You got that for free!", item not billed |
| 18 | rob_shop with credit >= total debt | Credit covers bill, no robbery, no kops |
| 19 | Angry shk with surcharge, then pacified: price 1 | rile: 1+(1+2)/3=2, pacify: 2-(2+3)/4=2-1=1. Restored correctly. |
| 20 | pay_for_damage with !rn2(50) true | Shopkeeper refuses payment and becomes angry despite hero having enough gold |

### Credit System

| # | Scenario | Expected |
|---|----------|----------|
| 21 | Sell item worth 100 to cashless shopkeeper | Credit offered: (100*9)/10 + 0 = 90 |
| 22 | Sell item worth 1 to cashless shopkeeper | Credit offered: (1*9)/10 + 1 = 1 |
| 23 | Drop 500 gold in shop with debit=200 | debit becomes 0, credit += 300 |
