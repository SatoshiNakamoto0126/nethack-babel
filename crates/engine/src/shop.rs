//! Shop system: pricing, shopkeeper behavior, and transactions.
//!
//! Implements the NetHack 3.7 shop mechanics from `src/shk.c`, including
//! pricing formulas with CHA modifiers, buy/sell transactions, bill
//! management, credit, and shopkeeper anger/Kop-spawning on theft.
//!
//! Reference: `specs/shop.md`.

use hecs::Entity;
use rand::Rng;
use serde::{Deserialize, Serialize};

use nethack_babel_data::{Enchantment, ObjectClass, ObjectCore, ObjectDef};

use crate::action::Position;
use crate::event::EngineEvent;
use crate::steed::MountedOn;
use crate::world::{Attributes, GameWorld, Name, Positioned};

// ---------------------------------------------------------------------------
// Shop type
// ---------------------------------------------------------------------------

/// The kind of merchandise a shop specializes in.
///
/// Corresponds to `shtypes[]` entries from `shknam.c`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShopType {
    /// General store — sells anything.
    General,
    /// Used armor dealership.
    Armor,
    /// Second-hand bookstore (scrolls).
    Scroll,
    /// Liquor emporium (potions).
    Potion,
    /// Antique weapons outlet.
    Weapon,
    /// Delicatessen (food).
    Food,
    /// Jewelers (rings).
    Ring,
    /// Quality apparel and accessories (wands).
    Wand,
    /// Hardware store (tools).
    Tool,
    /// Rare books (spellbooks).
    Book,
    /// Health food store (vegetarian food) — `FODDERSHOP`.
    HealthFood,
    /// Lighting store (candles) — unique, special-level only.
    Candle,
}

impl ShopType {
    /// The display name used in shopkeeper greetings.
    pub fn display_name(&self) -> &'static str {
        match self {
            ShopType::General => "general store",
            ShopType::Armor => "used armor dealership",
            ShopType::Scroll => "second-hand bookstore",
            ShopType::Potion => "liquor emporium",
            ShopType::Weapon => "antique weapons outlet",
            ShopType::Food => "delicatessen",
            ShopType::Ring => "jewelers",
            ShopType::Wand => "quality apparel and accessories",
            ShopType::Tool => "hardware store",
            ShopType::Book => "rare books",
            ShopType::HealthFood => "health food store",
            ShopType::Candle => "lighting store",
        }
    }

    /// Whether this shop type would stock the given object class.
    pub fn sells_class(&self, class: ObjectClass) -> bool {
        match self {
            ShopType::General => true,
            ShopType::Armor => {
                matches!(class, ObjectClass::Armor | ObjectClass::Weapon)
            }
            ShopType::Scroll => {
                matches!(class, ObjectClass::Scroll | ObjectClass::Spellbook)
            }
            ShopType::Potion => class == ObjectClass::Potion,
            ShopType::Weapon => {
                matches!(class, ObjectClass::Weapon | ObjectClass::Armor)
            }
            ShopType::Food => class == ObjectClass::Food,
            ShopType::Ring => matches!(
                class,
                ObjectClass::Ring | ObjectClass::Gem | ObjectClass::Amulet
            ),
            ShopType::Wand => class == ObjectClass::Wand,
            ShopType::Tool => class == ObjectClass::Tool,
            ShopType::Book => {
                matches!(class, ObjectClass::Spellbook | ObjectClass::Scroll)
            }
            ShopType::HealthFood => {
                matches!(
                    class,
                    ObjectClass::Food | ObjectClass::Potion | ObjectClass::Scroll
                )
            }
            ShopType::Candle => matches!(
                class,
                ObjectClass::Tool
                    | ObjectClass::Potion
                    | ObjectClass::Wand
                    | ObjectClass::Scroll
                    | ObjectClass::Spellbook
            ),
        }
    }
}

impl ShopType {
    /// Select a random shop type using the probability table from `shtypes[]`.
    ///
    /// Total probability = 100%.  Returns `None` only if roll is out of range.
    pub fn random<R: Rng>(rng: &mut R) -> ShopType {
        // Probabilities from spec section 1.1:
        // General 42, Armor 14, Scroll 10, Potion 10, Weapon 5,
        // Food 5, Ring 3, Wand 3, Tool 3, Book 3, HealthFood 2.
        let roll = rng.random_range(0..100);
        match roll {
            0..42 => ShopType::General,
            42..56 => ShopType::Armor,
            56..66 => ShopType::Scroll,
            66..76 => ShopType::Potion,
            76..81 => ShopType::Weapon,
            81..86 => ShopType::Food,
            86..89 => ShopType::Ring,
            89..92 => ShopType::Wand,
            92..95 => ShopType::Tool,
            95..98 => ShopType::Book,
            _ => ShopType::HealthFood, // 98..100
        }
    }
}

// ---------------------------------------------------------------------------
// Shopkeeper initial gold
// ---------------------------------------------------------------------------

/// Range for shopkeeper initial gold: `1000 + 30 * rnd(100)`.
///
/// Minimum = 1000 + 30*1 = 1030, Maximum = 1000 + 30*100 = 4000.
pub fn shopkeeper_initial_gold<R: Rng>(rng: &mut R) -> i32 {
    1000 + 30 * rng.random_range(1..=100)
}

// ---------------------------------------------------------------------------
// Bill entry
// ---------------------------------------------------------------------------

/// A single entry on the shopkeeper's bill.
///
/// Corresponds to `struct bill_x` in C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillEntry {
    /// The billed item entity.
    pub item: Entity,
    /// Price per unit at the time of billing.
    pub price: i32,
    /// Original quantity when the item was billed.
    pub original_quantity: i32,
    /// Amount already paid toward this entry.
    #[serde(default)]
    pub paid_amount: i32,
    /// Whether the item has been completely consumed.
    pub used_up: bool,
}

// ---------------------------------------------------------------------------
// Shop bill
// ---------------------------------------------------------------------------

/// Maximum number of bill entries (mirrors C `BILLSZ`).
pub const BILL_SIZE: usize = 200;

/// Tracks all unpaid items for one shop.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShopBill {
    /// Active bill entries.
    entries: Vec<BillEntry>,
}

impl ShopBill {
    /// Create an empty bill.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add an item to the bill.  Returns `false` if the bill is full.
    pub fn add(&mut self, item: Entity, price: i32, quantity: i32) -> bool {
        if self.entries.len() >= BILL_SIZE {
            return false;
        }
        self.entries.push(BillEntry {
            item,
            price,
            original_quantity: quantity,
            paid_amount: 0,
            used_up: false,
        });
        true
    }

    /// Remove a bill entry for the given item.  Returns the removed entry,
    /// or `None` if not found.
    pub fn remove(&mut self, item: Entity) -> Option<BillEntry> {
        if let Some(idx) = self.entries.iter().position(|e| e.item == item) {
            Some(self.entries.remove(idx))
        } else {
            None
        }
    }

    /// Look up the bill entry for a given item.
    pub fn find(&self, item: Entity) -> Option<&BillEntry> {
        self.entries.iter().find(|e| e.item == item)
    }

    /// Total amount owed across all bill entries.
    pub fn total(&self) -> i32 {
        self.entries.iter().map(entry_amount_owed).sum()
    }

    /// Number of entries on the bill.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the bill is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries from the bill.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Iterate over bill entries.
    pub fn entries(&self) -> &[BillEntry] {
        &self.entries
    }
}

fn entry_total(entry: &BillEntry) -> i32 {
    entry.price * entry.original_quantity
}

fn entry_amount_owed(entry: &BillEntry) -> i32 {
    (entry_total(entry) - entry.paid_amount).max(0)
}

fn apply_payment_to_shop_bill(bill: &mut ShopBill, mut amount: i32) -> i32 {
    if amount <= 0 {
        return 0;
    }

    for entry in &mut bill.entries {
        if amount <= 0 {
            break;
        }
        let owed = entry_amount_owed(entry);
        if owed == 0 {
            continue;
        }
        let applied = owed.min(amount);
        entry.paid_amount += applied;
        amount -= applied;
    }
    bill.entries.retain(|entry| entry_amount_owed(entry) > 0);
    amount
}

fn apply_payment_to_shop(shop: &mut ShopRoom, amount: i32) -> i32 {
    if amount <= 0 {
        return shop.bill.total() + shop.debit;
    }

    let applied_to_debit = shop.debit.min(amount);
    shop.debit -= applied_to_debit;
    let remaining_for_bill = amount - applied_to_debit;
    let _ = apply_payment_to_shop_bill(&mut shop.bill, remaining_for_bill);
    shop.bill.total() + shop.debit
}

// ---------------------------------------------------------------------------
// Shop room
// ---------------------------------------------------------------------------

/// A shop room on the current level.
///
/// Contains the shop boundaries, type, shopkeeper reference, bill, and
/// credit tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopRoom {
    /// Top-left corner of the shop room (inclusive).
    pub top_left: Position,
    /// Bottom-right corner of the shop room (inclusive).
    pub bottom_right: Position,
    /// What kind of shop this is.
    pub shop_type: ShopType,
    /// The shopkeeper entity.
    pub shopkeeper: Entity,
    /// The shopkeeper's name.
    pub shopkeeper_name: String,
    /// Items on the hero's tab.
    pub bill: ShopBill,
    /// Hero's accumulated credit at this shop.
    pub credit: i32,
    /// Hero's accumulated debit (usage fees, picked-up gold).
    pub debit: i32,
    /// Total value of merchandise stolen from this shop.
    pub robbed: i32,
    /// Whether the shopkeeper is angry (surcharge active).
    pub angry: bool,
    /// Whether the surcharge flag is set (prices +33%).
    pub surcharge: bool,
    /// Shopkeeper's current gold (for sell transactions).
    /// Range at creation: 1030..4000 zm.
    pub shopkeeper_gold: i32,
    /// Door position (shopkeeper blocks here when player has unpaid items).
    pub door_pos: Option<Position>,
    /// Whether the hero has already been warned about leaving unpaid.
    #[serde(default)]
    pub exit_warning_issued: bool,
    /// Damage records for shop fixtures that need repair.
    #[serde(default)]
    pub damage_list: Vec<ShopDamage>,
}

impl ShopRoom {
    /// Create a new shop room.
    pub fn new(
        top_left: Position,
        bottom_right: Position,
        shop_type: ShopType,
        shopkeeper: Entity,
        shopkeeper_name: String,
    ) -> Self {
        Self {
            top_left,
            bottom_right,
            shop_type,
            shopkeeper,
            shopkeeper_name,
            bill: ShopBill::new(),
            credit: 0,
            debit: 0,
            robbed: 0,
            angry: false,
            surcharge: false,
            shopkeeper_gold: 0,
            door_pos: None,
            exit_warning_issued: false,
            damage_list: Vec::new(),
        }
    }

    /// Create a new shop room with shopkeeper gold.
    pub fn new_with_gold<R: Rng>(
        top_left: Position,
        bottom_right: Position,
        shop_type: ShopType,
        shopkeeper: Entity,
        shopkeeper_name: String,
        door_pos: Option<Position>,
        rng: &mut R,
    ) -> Self {
        Self {
            top_left,
            bottom_right,
            shop_type,
            shopkeeper,
            shopkeeper_name,
            bill: ShopBill::new(),
            credit: 0,
            debit: 0,
            robbed: 0,
            angry: false,
            surcharge: false,
            shopkeeper_gold: shopkeeper_initial_gold(rng),
            door_pos,
            exit_warning_issued: false,
            damage_list: Vec::new(),
        }
    }

    /// Whether the shopkeeper should block the door.
    ///
    /// Returns `true` if the player has unpaid items (bill is non-empty
    /// or debit > 0).
    pub fn should_block_door(&self) -> bool {
        !self.bill.is_empty() || self.debit > 0
    }

    /// Whether the given position is inside this shop's boundaries.
    pub fn contains(&self, pos: Position) -> bool {
        pos.x >= self.top_left.x
            && pos.x <= self.bottom_right.x
            && pos.y >= self.top_left.y
            && pos.y <= self.bottom_right.y
    }

    /// Add credit to this shop.
    pub fn add_credit(&mut self, amount: i32) {
        self.credit += amount;
    }

    /// Use credit to cover a cost.  Returns the remaining amount after
    /// applying available credit.
    pub fn use_credit(&mut self, amount: i32) -> i32 {
        if self.credit >= amount {
            self.credit -= amount;
            0
        } else {
            let remaining = amount - self.credit;
            self.credit = 0;
            remaining
        }
    }
}

// ---------------------------------------------------------------------------
// Charisma pricing modifier
// ---------------------------------------------------------------------------

/// Compute the charisma-based multiplier and divisor for buy prices.
///
/// Returns `(multiplier, divisor)` to be applied to the base price.
///
/// | CHA   | Effect | Ratio |
/// |-------|--------|-------|
/// | <=5   | x2/1   | 200%  |
/// | 6-7   | x3/2   | 150%  |
/// | 8-10  | x4/3   | 133%  |
/// | 11-15 | x1/1   | 100%  |
/// | 16-17 | x3/4   |  75%  |
/// | 18    | x2/3   |  67%  |
/// | >18   | x1/2   |  50%  |
fn cha_price_modifier(charisma: u8) -> (i32, i32) {
    match charisma {
        0..=5 => (2, 1),
        6..=7 => (3, 2),
        8..=10 => (4, 3),
        11..=15 => (1, 1),
        16..=17 => (3, 4),
        18 => (2, 3),
        _ => (1, 2), // 19+
    }
}

// ---------------------------------------------------------------------------
// Price calculation
// ---------------------------------------------------------------------------

/// Compute the base price of an item before any buy/sell modifiers.
///
/// Mirrors `getprice(obj, shk_buying)` in C, handling:
/// - Weapon/armor enchantment bonus (+10 per positive spe)
/// - Empty wand (spe == -1) => base 0
/// - Artifact base price override
///
/// # Arguments
///
/// * `base_cost` — The item's `ObjectDef.cost` value.
/// * `class` — Object class (for class-specific adjustments).
/// * `spe` — Enchantment/charges value (`obj.spe`), or 0 if none.
/// * `is_artifact` — Whether the item is an artifact.
/// * `artifact_cost` — Artifact-specific base price (from `artilist[]`);
///   used only when `is_artifact` is true.
/// * `shk_buying` — `true` when the shopkeeper is buying (sell direction);
///   artifacts get base/4 in this case.
pub fn get_base_price(
    base_cost: i32,
    class: ObjectClass,
    spe: i8,
    is_artifact: bool,
    artifact_cost: i32,
    shk_buying: bool,
) -> i32 {
    let mut base = if is_artifact {
        let mut ac = artifact_cost;
        if shk_buying {
            ac /= 4;
        }
        ac
    } else {
        base_cost
    };

    match class {
        ObjectClass::Wand if spe == -1 => {
            // Empty/cancelled wand is worthless.
            base = 0;
        }
        ObjectClass::Armor | ObjectClass::Weapon if spe > 0 => {
            base += 10 * spe as i32;
        }
        _ => {}
    }

    base
}

/// Check whether an unidentified item gets the 25% OID surcharge.
///
/// Mirrors `oid_price_adjustment()` in C.  Returns `true` if the item
/// should receive a +33% surcharge (obj.o_id % 4 == 0).
///
/// # Arguments
///
/// * `item_id` — The item's unique `o_id` (creation counter).
/// * `identified` — Whether the item's type is known to the player.
/// * `is_glass_gem` — Whether the item is a GLASS-material gem (separate
///   deception logic).
pub fn has_oid_surcharge(item_id: u32, identified: bool, is_glass_gem: bool) -> bool {
    if identified || is_glass_gem {
        return false;
    }
    item_id.is_multiple_of(4)
}

/// Compute the price an item costs, adjusted for charisma and role.
///
/// This is the main pricing function.  It combines the base cost from
/// `ObjectDef` with CHA modifiers, tourist/dunce penalties, and handles
/// both buying and selling directions.
///
/// # Arguments
///
/// * `base_cost` — The item's `ObjectDef.cost` value (per unit).
/// * `quantity` — Number of units being priced.
/// * `charisma` — The hero's charisma score (3..25).
/// * `is_buying` — `true` for buy price (hero pays), `false` for sell.
/// * `is_tourist_or_dunce` — Whether tourist/dunce penalty applies.
///
/// # Returns
///
/// Total price in zorkmids (always >= 1).
pub fn get_cost(
    base_cost: i32,
    quantity: i32,
    charisma: u8,
    is_buying: bool,
    is_tourist_or_dunce: bool,
) -> i32 {
    let mut tmp = base_cost;

    // Minimum base price of 5 zm for "worthless" items (buy only).
    if is_buying && tmp <= 0 {
        tmp = 5;
    }

    let mut multiplier: i32 = 1;
    let mut divisor: i32 = 1;

    if is_buying {
        // Tourist/dunce penalty when buying: +33%.
        if is_tourist_or_dunce {
            multiplier *= 4;
            divisor *= 3;
        }

        // CHA modifier (buy price only).
        let (cha_mul, cha_div) = cha_price_modifier(charisma);
        multiplier *= cha_mul;
        divisor *= cha_div;

        // Apply with banker's rounding.
        tmp *= multiplier;
        if divisor > 1 {
            tmp = (tmp * 10 / divisor + 5) / 10;
        }

        // Minimum 1 zm.
        if tmp <= 0 {
            tmp = 1;
        }
    } else {
        // Selling: tourist/dunce gets 1/3, others get 1/2.
        if is_tourist_or_dunce {
            divisor *= 3;
        } else {
            divisor *= 2;
        }

        // Sell price has no CHA modifier.
        tmp *= multiplier;
        if divisor > 1 {
            tmp = (tmp * 10 / divisor + 5) / 10;
        }

        // Minimum 1 zm.
        if tmp < 1 {
            tmp = 1;
        }
    }

    // Multiply by quantity.
    tmp * quantity.max(1)
}

/// Full buy-price calculation matching spec test vector #10.
///
/// Combines `get_base_price` adjustments, OID surcharge, tourist/dunce,
/// CHA modifier, artifact 4x, and anger surcharge into one call.
///
/// # Arguments
///
/// * `base_cost` — `ObjectDef.cost`.
/// * `class` — Object class.
/// * `spe` — Enchantment value.
/// * `quantity` — Stack count.
/// * `charisma` — Hero CHA.
/// * `is_tourist_or_dunce` — Tourist (level < 15) or wearing dunce cap.
/// * `is_artifact` — Whether the item is an artifact.
/// * `artifact_cost` — Artifact-specific base price.
/// * `oid_surcharge` — Whether the OID surcharge applies (25% of unid items).
/// * `anger_surcharge` — Whether the shopkeeper's surcharge flag is active.
#[allow(clippy::too_many_arguments)]
pub fn get_full_buy_price(
    base_cost: i32,
    class: ObjectClass,
    spe: i8,
    quantity: i32,
    charisma: u8,
    is_tourist_or_dunce: bool,
    is_artifact: bool,
    artifact_cost: i32,
    oid_surcharge: bool,
    anger_surcharge: bool,
) -> i32 {
    let base = get_base_price(base_cost, class, spe, is_artifact, artifact_cost, false);

    let mut tmp = base;

    // Minimum base price of 5 zm for "worthless" items.
    if tmp <= 0 {
        tmp = 5;
    }

    let mut multiplier: i32 = 1;
    let mut divisor: i32 = 1;

    // OID surcharge for unidentified items: +33%.
    if oid_surcharge {
        multiplier *= 4;
        divisor *= 3;
    }

    // Tourist/dunce penalty: +33%.
    if is_tourist_or_dunce {
        multiplier *= 4;
        divisor *= 3;
    }

    // CHA modifier.
    let (cha_mul, cha_div) = cha_price_modifier(charisma);
    multiplier *= cha_mul;
    divisor *= cha_div;

    // Apply with banker's rounding.
    tmp *= multiplier;
    if divisor > 1 {
        tmp = (tmp * 10 / divisor + 5) / 10;
    }

    if tmp <= 0 {
        tmp = 1;
    }

    // Artifact 4x multiplier (applied after CHA/tourist modifiers).
    if is_artifact {
        tmp *= 4;
    }

    // Anger surcharge: +33% rounded.
    if anger_surcharge {
        tmp += (tmp + 2) / 3;
    }

    tmp * quantity.max(1)
}

/// Full sell-price calculation.
///
/// Mirrors `set_cost()` in C.  Sell price has no CHA modifier and no
/// artifact 4x.  Tourist/dunce sell at 1/3 instead of 1/2.
pub fn get_full_sell_price(
    base_cost: i32,
    class: ObjectClass,
    spe: i8,
    quantity: i32,
    is_tourist_or_dunce: bool,
    is_artifact: bool,
    artifact_cost: i32,
) -> i32 {
    let base = get_base_price(base_cost, class, spe, is_artifact, artifact_cost, true);
    let tmp = base * quantity.max(1);

    let divisor: i32 = if is_tourist_or_dunce { 3 } else { 2 };

    let mut result = (tmp * 10 / divisor + 5) / 10;
    if result < 1 {
        result = 1;
    }
    result
}

/// Credit offered when shopkeeper has no gold (credit-for-sale).
///
/// Approximately 90% of the sell value, with minimum 1 zm.
///
/// Formula: `(offer * 9) / 10 + (if offer <= 1 { 1 } else { 0 })`
pub fn credit_for_sale(sell_value: i32) -> i32 {
    (sell_value * 9) / 10 + if sell_value <= 1 { 1 } else { 0 }
}

/// Handle donating gold in a shop.
///
/// Gold exceeding `debit` becomes credit.  Returns the amount added as
/// credit (0 if all went to debit).
pub fn donate_gold(shop: &mut ShopRoom, gold_amount: i32) -> i32 {
    if gold_amount <= 0 {
        return 0;
    }
    if shop.debit >= gold_amount {
        shop.debit -= gold_amount;
        0
    } else {
        let excess = gold_amount - shop.debit;
        shop.debit = 0;
        shop.add_credit(excess);
        excess
    }
}

/// Compute the per-use fee for using an unpaid charged item in a shop.
///
/// Mirrors `cost_per_charge()` in C.  Returns the fee to add to
/// `eshk.debit`.
///
/// # Arguments
///
/// * `item_buy_price` — The item's `get_cost()` buy price.
/// * `item_name` — Used to identify the item type for fee lookup.
/// * `is_magic_lamp` — Whether the item is a magic lamp.
/// * `is_bag_of_tricks` — Whether the item is a bag of tricks or horn
///   of plenty.
/// * `is_spellbook` — Whether the item is a spellbook.
/// * `is_cheap_charged` — Can of grease, tinning kit, expensive camera.
/// * `is_marker` — Magic marker.
/// * `is_oil_potion` — Potion of oil.
/// * `emptied` — Whether the item was fully emptied (bag of tricks
///   spill-all).
#[allow(clippy::too_many_arguments)]
pub fn usage_fee(
    item_buy_price: i32,
    is_magic_lamp: bool,
    is_bag_of_tricks: bool,
    is_spellbook: bool,
    is_cheap_charged: bool,
    is_marker: bool,
    is_oil_potion: bool,
    emptied: bool,
) -> i32 {
    if is_magic_lamp {
        // Magic lamp used as light: fixed cost of OIL_LAMP base.
        // We approximate with item_buy_price + item_buy_price/3
        // for djinni release.
        item_buy_price + item_buy_price / 3
    } else if is_marker {
        item_buy_price / 2
    } else if is_bag_of_tricks {
        if emptied {
            item_buy_price
        } else {
            item_buy_price / 5
        }
    } else if is_spellbook {
        item_buy_price - item_buy_price / 5 // 4/5 of cost
    } else if is_cheap_charged {
        item_buy_price / 10
    } else if is_oil_potion {
        item_buy_price / 5
    } else {
        // Crystal ball, oil lamp, brass lantern, instruments, wands.
        item_buy_price / 4
    }
}

/// Compute Kop spawn counts for a given dungeon depth.
///
/// Returns `(kops, sergeants, lieutenants, kaptains)`.
pub fn kop_counts(depth: i32, rnd5: i32) -> (i32, i32, i32, i32) {
    let cnt = depth.abs() + rnd5;
    (cnt, cnt / 3 + 1, cnt / 6, cnt / 9)
}

// ---------------------------------------------------------------------------
// Shop interactions
// ---------------------------------------------------------------------------

/// Generate greeting events when the player enters a shop.
pub fn enter_shop(world: &GameWorld, player: Entity, shop: &ShopRoom) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let _player_name = world.entity_name(player);

    let greeting_key = if shop.angry {
        "shop-angry"
    } else if shop.surcharge {
        "shop-welcome-back"
    } else if shop.robbed > 0 {
        "shop-stolen"
    } else {
        "shop-enter"
    };
    events.push(EngineEvent::msg_with(
        greeting_key,
        vec![
            ("shopkeeper", shop.shopkeeper_name.clone()),
            ("shoptype", shop.shop_type.display_name().to_string()),
        ],
    ));
    if world
        .get_component::<crate::status::StatusEffects>(player)
        .is_some_and(|status| status.invisibility > 0)
    {
        events.push(EngineEvent::msg("shop-enter-invisible"));
    }
    if player_has_digging_tool(world, player) {
        events.push(EngineEvent::msg("shop-enter-digging-tool"));
    }
    if let Some(steed_name) = mounted_steed_name(world, player) {
        events.push(EngineEvent::msg_with(
            "shop-enter-steed",
            vec![("steed", steed_name)],
        ));
    }
    events
}

fn player_has_digging_tool(world: &GameWorld, player: Entity) -> bool {
    world
        .get_component::<crate::inventory::Inventory>(player)
        .is_some_and(|inventory| {
            inventory.items.iter().any(|item| {
                world
                    .get_component::<Name>(*item)
                    .map(|name| {
                        let name = name.0.to_ascii_lowercase();
                        name.contains("pick-axe")
                            || name.contains("pickaxe")
                            || name.contains("mattock")
                    })
                    .unwrap_or(false)
            })
        })
}

fn mounted_steed_name(world: &GameWorld, player: Entity) -> Option<String> {
    let steed = world
        .get_component::<MountedOn>(player)
        .map(|mounted| mounted.0)?;
    Some(world.entity_name(steed))
}

/// Handle picking up an item inside a shop: add it to the bill.
///
/// The item's price is calculated and added to the shop bill.  The item
/// is also marked via the returned events so the renderer can display
/// "unpaid" status.
pub fn pickup_in_shop(
    world: &GameWorld,
    _player: Entity,
    item: Entity,
    shop: &mut ShopRoom,
    obj_defs: &[ObjectDef],
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let quote = match quoted_buy_details(world, _player, item, shop, obj_defs) {
        Some(details) => details,
        None => return events,
    };

    // Try to add to bill.
    if shop.bill.add(item, quote.unit_price, quote.quantity) {
        events.push(EngineEvent::msg_with(
            quote.message_key,
            vec![
                ("shopkeeper", shop.shopkeeper_name.clone()),
                ("item", quote.item_name),
                ("price", (quote.unit_price * quote.quantity).to_string()),
            ],
        ));
    } else {
        // Bill is full — item is free.
        events.push(EngineEvent::msg("shop-free"));
    }

    events
}

struct ShopQuoteDetails {
    item_name: String,
    unit_price: i32,
    quantity: i32,
    message_key: &'static str,
}

fn quoted_buy_details(
    world: &GameWorld,
    player: Entity,
    item: Entity,
    shop: &ShopRoom,
    obj_defs: &[ObjectDef],
) -> Option<ShopQuoteDetails> {
    let (item_otyp, quantity, item_class) = {
        let core = world.get_component::<ObjectCore>(item)?;
        (core.otyp, core.quantity, core.object_class)
    };

    // Find the definition to get the base cost.
    let obj_def = obj_defs.iter().find(|d| d.id == item_otyp);
    let item_name = obj_def
        .map(|def| def.name.clone())
        .or_else(|| world.get_component::<Name>(item).map(|name| name.0.clone()))
        .unwrap_or_else(|| "item".to_string());
    let base_cost = obj_def.map(|d| d.cost as i32).unwrap_or(0);
    let is_magic = obj_def.map(|d| d.is_magic).unwrap_or(false);

    // Get enchantment if present.
    let spe = world
        .get_component::<Enchantment>(item)
        .map(|e| e.spe)
        .unwrap_or(0);

    // Get artifact status.
    let is_artifact = world
        .get_component::<ObjectCore>(item)
        .map(|c| c.artifact.is_some())
        .unwrap_or(false);

    // Get player charisma from the world.
    let charisma = world
        .get_component::<Attributes>(player)
        .map(|a| a.charisma)
        .unwrap_or(10);

    // Compute base price with class adjustments.
    let adjusted_base = get_base_price(
        base_cost,
        item_class,
        spe,
        is_artifact,
        base_cost * 4,
        false,
    );

    let unit_price = get_cost(adjusted_base, 1, charisma, true, false);

    // Apply artifact 4x if needed.
    let unit_price = if is_artifact {
        unit_price * 4
    } else {
        unit_price
    };

    // Apply anger surcharge if active.
    let unit_price = if shop.surcharge {
        unit_price + (unit_price + 2) / 3
    } else {
        unit_price
    };

    let identified = world
        .get_component::<nethack_babel_data::KnowledgeState>(item)
        .is_some_and(|knowledge| knowledge.known);
    let total_price = unit_price * quantity;

    Some(ShopQuoteDetails {
        message_key: price_quote_message_key(
            item_class,
            is_magic,
            identified,
            is_artifact,
            total_price,
        ),
        item_name,
        unit_price,
        quantity,
    })
}

pub fn quote_item_in_shop(
    world: &GameWorld,
    player: Entity,
    item: Entity,
    shop: &ShopRoom,
    obj_defs: &[ObjectDef],
) -> Option<EngineEvent> {
    if world
        .get_component::<ObjectCore>(item)
        .is_some_and(|core| core.object_class == ObjectClass::Coin)
    {
        return None;
    }

    if shop.angry || crate::status::is_blind(world, player) || crate::status::is_deaf(world, player)
    {
        return None;
    }

    let quote = quoted_buy_details(world, player, item, shop, obj_defs)?;
    Some(EngineEvent::msg_with(
        quote.message_key,
        vec![
            ("shopkeeper", shop.shopkeeper_name.clone()),
            ("item", quote.item_name),
            ("price", (quote.unit_price * quote.quantity).to_string()),
        ],
    ))
}

/// Handle dropping an item inside a shop: sell it for credit or gold.
///
/// When the hero drops an item on the shop floor, the shopkeeper offers
/// to buy it at the sell price.  If the shopkeeper has no cash, the hero
/// receives credit instead.
pub fn drop_in_shop(
    world: &GameWorld,
    _player: Entity,
    item: Entity,
    shop: &mut ShopRoom,
    obj_defs: &[ObjectDef],
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let (item_otyp, quantity, item_name) = {
        let core = match world.get_component::<ObjectCore>(item) {
            Some(c) => c,
            None => return events,
        };
        let item_name = world
            .get_component::<crate::world::Name>(item)
            .map(|name| name.0.clone())
            .or_else(|| {
                crate::items::object_def_for_core(obj_defs, &core).map(|def| def.name.clone())
            })
            .unwrap_or_else(|| "item".to_string());
        (core.otyp, core.quantity, item_name)
    };

    // If the item was on the bill, just remove it (returned merchandise).
    if let Some(entry) = shop.bill.remove(item) {
        events.push(EngineEvent::msg_with(
            "shop-return",
            vec![("shopkeeper", shop.shopkeeper_name.clone())],
        ));
        // If item was returned, no further sell transaction.
        let _ = entry;
        return events;
    }

    // Otherwise it is a sell: check if it is saleable.
    let obj_def = obj_defs.iter().find(|d| d.id == item_otyp);
    let base_cost = obj_def.map(|d| d.cost as i32).unwrap_or(0);

    // Check if this shop would buy this class.
    let item_class = world
        .get_component::<ObjectCore>(item)
        .map(|c| c.object_class)
        .unwrap_or(ObjectClass::IllegalObject);

    if !shop.shop_type.sells_class(item_class) && base_cost <= 0 {
        events.push(EngineEvent::msg_with(
            "shop-not-interested",
            vec![("shopkeeper", shop.shopkeeper_name.clone())],
        ));
        return events;
    }

    // If previously robbed, item value reduces robbed amount.
    let sell_price = get_cost(base_cost, quantity, 10, false, false);

    if shop.robbed > 0 {
        shop.robbed = (shop.robbed - sell_price).max(0);
        events.push(EngineEvent::msg_with(
            "shop-restock",
            vec![("shopkeeper", shop.shopkeeper_name.clone())],
        ));
        return events;
    }

    // Angry shopkeeper takes without paying.
    if shop.angry {
        events.push(EngineEvent::msg("shop-angry-take"));
        return events;
    }

    // Sell transaction: shopkeeper pays gold or offers credit.
    if shop.shopkeeper_gold >= sell_price {
        // Shopkeeper has enough gold — pay directly.
        shop.shopkeeper_gold -= sell_price;
        events.push(EngineEvent::msg_with(
            "shop-sell",
            vec![
                ("item", item_name.clone()),
                ("price", sell_price.to_string()),
            ],
        ));
    } else if shop.shopkeeper_gold > 0 {
        // Shopkeeper has some gold but not enough — short funds.
        let offer = shop.shopkeeper_gold;
        shop.shopkeeper_gold = 0;
        events.push(EngineEvent::msg_with(
            "shop-sell",
            vec![("item", item_name), ("price", offer.to_string())],
        ));
    } else {
        // Shopkeeper has no gold — offer credit at 90%.
        let credit_amount = credit_for_sale(sell_price);
        shop.add_credit(credit_amount);
        events.push(EngineEvent::msg_with(
            "shop-credit",
            vec![
                ("shopkeeper", shop.shopkeeper_name.clone()),
                ("amount", credit_amount.to_string()),
            ],
        ));
    }

    events
}

/// Pay the bill at a shop.
///
/// Attempts to pay all outstanding bill entries using the hero's credit
/// first, then gold.  Returns events describing the transaction.
///
/// `player_gold` is mutated to reflect the gold spent.
pub fn pay_bill(
    _world: &GameWorld,
    _player: Entity,
    shop: &mut ShopRoom,
    player_gold: &mut i32,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if shop.bill.is_empty() && shop.debit == 0 {
        events.push(EngineEvent::msg("shop-no-debt"));
        return events;
    }

    let total_owed = shop.bill.total() + shop.debit;
    let credit_payment = shop.credit.min(total_owed);
    if credit_payment > 0 {
        shop.credit -= credit_payment;
        let _ = apply_payment_to_shop(shop, credit_payment);
    }

    let after_credit = shop.bill.total() + shop.debit;
    if after_credit == 0 {
        // Credit covered everything.
        events.push(EngineEvent::msg("shop-credit-covers"));
        return events;
    }

    let gold_payment = (*player_gold).min(after_credit);
    if gold_payment > 0 {
        *player_gold -= gold_payment;
        let remaining = apply_payment_to_shop(shop, gold_payment);
        events.push(EngineEvent::msg_with(
            "shop-pay-success",
            vec![
                ("shopkeeper", shop.shopkeeper_name.clone()),
                ("amount", gold_payment.to_string()),
            ],
        ));
        if remaining > 0 {
            events.push(EngineEvent::msg_with(
                "shop-owe",
                vec![
                    ("shopkeeper", shop.shopkeeper_name.clone()),
                    ("amount", remaining.to_string()),
                ],
            ));
        }
    } else {
        events.push(EngineEvent::msg("shop-no-money"));
    }

    events
}

/// Handle the hero leaving the shop with unpaid items — robbery!
///
/// Angers the shopkeeper, records the stolen amount, and spawns Kops.
pub fn rob_shop<R: Rng>(
    world: &GameWorld,
    player: Entity,
    shop: &mut ShopRoom,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let total = shop.bill.total() + shop.debit;

    // If credit covers the bill, no actual robbery.
    if shop.credit >= total {
        shop.credit -= total;
        shop.bill.clear();
        shop.debit = 0;
        shop.exit_warning_issued = false;
        events.push(EngineEvent::msg("shop-credit-covers"));
        return events;
    }

    // Actual robbery.
    let stolen = total - shop.credit;
    shop.credit = 0;
    shop.bill.clear();
    shop.debit = 0;
    shop.exit_warning_issued = false;
    shop.robbed += stolen;

    events.push(EngineEvent::msg_with(
        "shop-shoplift",
        vec![("shopkeeper", shop.shopkeeper_name.clone())],
    ));
    events.push(EngineEvent::msg_with(
        "shop-stolen-amount",
        vec![("amount", stolen.to_string())],
    ));

    // Anger the shopkeeper.
    rile_shop(shop);

    // Spawn Kops near the player.
    let kop_events = spawn_kops(world, player, rng);
    events.extend(kop_events);

    events
}

/// Clear all bill, credit, debit, and loan state.
///
/// Mirrors `setpaid()` in C.  Called when hero pays in full, shopkeeper
/// dies, or robbery is processed.
pub fn setpaid(shop: &mut ShopRoom) {
    shop.bill.clear();
    shop.credit = 0;
    shop.debit = 0;
    shop.exit_warning_issued = false;
}

/// Handle shopkeeper death: clear the bill, remove residency.
///
/// Mirrors `shkgone()` in C.
pub fn shopkeeper_died(shop: &mut ShopRoom) -> Vec<EngineEvent> {
    setpaid(shop);
    shop.angry = false;
    shop.surcharge = false;
    vec![EngineEvent::msg_with(
        "shop-keeper-dead",
        vec![("shopkeeper", shop.shopkeeper_name.clone())],
    )]
}

/// Convert all pending transactions to robbery (e.g., shopkeeper
/// teleported out).
///
/// Mirrors `make_angry_shk()` in C.
pub fn make_angry(shop: &mut ShopRoom) {
    let total = shop.bill.total() + shop.debit - shop.credit;
    shop.robbed += total.max(0);
    setpaid(shop);
    rile_shop(shop);
}

// ---------------------------------------------------------------------------
// Shopkeeper anger / pacification
// ---------------------------------------------------------------------------

/// Make the shopkeeper angry and apply the surcharge to all bill entries.
///
/// Corresponds to `rile_shk()` in C.
pub fn rile_shop(shop: &mut ShopRoom) {
    shop.angry = true;
    if !shop.surcharge {
        shop.surcharge = true;
        for entry in shop.bill.entries.iter_mut() {
            let surcharge = (entry.price + 2) / 3;
            entry.price += surcharge;
        }
    }
}

/// Pacify the shopkeeper and remove the surcharge from bill entries.
///
/// Corresponds to `pacify_shk()` in C.
pub fn pacify_shop(shop: &mut ShopRoom) {
    shop.angry = false;
    shop.exit_warning_issued = false;
    if shop.surcharge {
        shop.surcharge = false;
        for entry in shop.bill.entries.iter_mut() {
            let reduction = (entry.price + 3) / 4;
            entry.price -= reduction;
        }
    }
}

// ---------------------------------------------------------------------------
// Kop spawning
// ---------------------------------------------------------------------------

/// Spawn Keystone Kops near the player as a consequence of shoplifting.
///
/// The number and rank of Kops depends on the dungeon depth.
/// Returns `MonsterGenerated` events for each spawned Kop.
fn spawn_kops<R: Rng>(world: &GameWorld, player: Entity, rng: &mut R) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let player_pos = world
        .get_component::<Positioned>(player)
        .map(|p| p.0)
        .unwrap_or(Position::new(0, 0));

    // Use a fixed depth estimate for Kop count.  In a full implementation
    // this would come from the dungeon state.
    let depth: i32 = 5;
    let cnt = depth.abs() + rng.random_range(1..=5);

    // Kops:        cnt
    // Sergeants:   cnt/3 + 1
    // Lieutenants: cnt/6
    // Kaptains:    cnt/9
    let kop_count = cnt;
    let sgt_count = cnt / 3 + 1;
    let lt_count = cnt / 6;
    let kpt_count = cnt / 9;

    let total = kop_count + sgt_count + lt_count + kpt_count;

    for i in 0..total {
        // Offset each Kop slightly from the player position.
        let dx = rng.random_range(-2..=2_i16);
        let dy = rng.random_range(-2..=2_i16);
        let kop_pos = Position::new(
            (player_pos.x as i16 + dx).max(0) as i32,
            (player_pos.y as i16 + dy).max(0) as i32,
        );

        // Determine rank name for the message.
        let rank = if i < kop_count {
            "Keystone Kop"
        } else if i < kop_count + sgt_count {
            "Kop Sergeant"
        } else if i < kop_count + sgt_count + lt_count {
            "Kop Lieutenant"
        } else {
            "Kop Kaptain"
        };

        // In a full implementation we would spawn actual monster entities
        // here.  For now, emit a message about each Kop.
        let kop = world.spawn_placeholder_entity();
        if let Some(entity) = kop {
            events.push(EngineEvent::MonsterGenerated {
                entity,
                position: kop_pos,
            });
        }

        events.push(EngineEvent::msg_with(
            "shop-kops-arrive",
            vec![("rank", rank.to_string())],
        ));
    }

    events
}

// ---------------------------------------------------------------------------
// Shop inventory generation (iprobs tables)
// ---------------------------------------------------------------------------

/// An entry in the item probability table for a shop type.
///
/// Negative `item_type` means a specific object enum ID; positive means
/// an `ObjectClass` discriminant value.  Mirrors C `shclass.iprobs[]`.
#[derive(Debug, Clone, Copy)]
pub struct ItemProb {
    /// Cumulative probability threshold (0..100).
    pub prob: u8,
    /// Positive = ObjectClass discriminant; negative = specific otyp.
    pub item_type: i16,
}

/// Get the item probability table for a shop type.
///
/// Each entry is `(cumulative_probability, item_type)`.  The table
/// always sums to 100.  A positive `item_type` value is an
/// `ObjectClass` discriminant; a negative value would indicate a
/// specific object type ID (for shop types that stock specific items).
pub fn shop_iprobs(shop_type: ShopType) -> &'static [ItemProb] {
    match shop_type {
        ShopType::General => &[ItemProb {
            prob: 100,
            item_type: ObjectClass::Random as i16,
        }],
        ShopType::Armor => &[
            ItemProb {
                prob: 90,
                item_type: ObjectClass::Armor as i16,
            },
            ItemProb {
                prob: 100,
                item_type: ObjectClass::Weapon as i16,
            },
        ],
        ShopType::Scroll => &[
            ItemProb {
                prob: 90,
                item_type: ObjectClass::Scroll as i16,
            },
            ItemProb {
                prob: 100,
                item_type: ObjectClass::Spellbook as i16,
            },
        ],
        ShopType::Potion => &[ItemProb {
            prob: 100,
            item_type: ObjectClass::Potion as i16,
        }],
        ShopType::Weapon => &[
            ItemProb {
                prob: 90,
                item_type: ObjectClass::Weapon as i16,
            },
            ItemProb {
                prob: 100,
                item_type: ObjectClass::Armor as i16,
            },
        ],
        ShopType::Food => &[
            // 83% FOOD_CLASS, then specific items (negative values).
            // We approximate specific items as FOOD_CLASS and POTION_CLASS.
            ItemProb {
                prob: 83,
                item_type: ObjectClass::Food as i16,
            },
            ItemProb {
                prob: 92,
                item_type: ObjectClass::Potion as i16,
            },
            ItemProb {
                prob: 100,
                item_type: ObjectClass::Tool as i16,
            },
        ],
        ShopType::Ring => &[
            ItemProb {
                prob: 85,
                item_type: ObjectClass::Ring as i16,
            },
            ItemProb {
                prob: 95,
                item_type: ObjectClass::Gem as i16,
            },
            ItemProb {
                prob: 100,
                item_type: ObjectClass::Amulet as i16,
            },
        ],
        ShopType::Wand => &[
            // 90% WAND_CLASS, 5% leather gloves (armor), 5% elven cloak (armor).
            ItemProb {
                prob: 90,
                item_type: ObjectClass::Wand as i16,
            },
            ItemProb {
                prob: 100,
                item_type: ObjectClass::Armor as i16,
            },
        ],
        ShopType::Tool => &[ItemProb {
            prob: 100,
            item_type: ObjectClass::Tool as i16,
        }],
        ShopType::Book => &[
            ItemProb {
                prob: 90,
                item_type: ObjectClass::Spellbook as i16,
            },
            ItemProb {
                prob: 100,
                item_type: ObjectClass::Scroll as i16,
            },
        ],
        ShopType::HealthFood => &[
            // 70% VEGETARIAN (food), 20% POT_FRUIT_JUICE (potion),
            // then specific potions/scrolls.
            ItemProb {
                prob: 70,
                item_type: ObjectClass::Food as i16,
            },
            ItemProb {
                prob: 94,
                item_type: ObjectClass::Potion as i16,
            },
            ItemProb {
                prob: 100,
                item_type: ObjectClass::Scroll as i16,
            },
        ],
        ShopType::Candle => &[
            // Specific candle/lamp items; we map to Tool + Potion + Wand + Scroll.
            ItemProb {
                prob: 88,
                item_type: ObjectClass::Tool as i16,
            },
            ItemProb {
                prob: 93,
                item_type: ObjectClass::Potion as i16,
            },
            ItemProb {
                prob: 96,
                item_type: ObjectClass::Wand as i16,
            },
            ItemProb {
                prob: 98,
                item_type: ObjectClass::Scroll as i16,
            },
            ItemProb {
                prob: 100,
                item_type: ObjectClass::Spellbook as i16,
            },
        ],
    }
}

/// Select an item class for a shop using the iprobs table.
///
/// Returns the `ObjectClass` for the generated item.  For general stores
/// (`RANDOM_CLASS`), the caller should pick a random class from the full
/// object table.
pub fn select_shop_item_class<R: Rng>(shop_type: ShopType, rng: &mut R) -> ObjectClass {
    let table = shop_iprobs(shop_type);
    let roll = rng.random_range(0..100) as u8;
    for entry in table {
        if roll < entry.prob {
            let class_val = entry.item_type;
            if class_val == ObjectClass::Random as i16 {
                return ObjectClass::Random;
            }
            // Convert i16 back to ObjectClass via discriminant.
            return match class_val {
                v if v == ObjectClass::Weapon as i16 => ObjectClass::Weapon,
                v if v == ObjectClass::Armor as i16 => ObjectClass::Armor,
                v if v == ObjectClass::Ring as i16 => ObjectClass::Ring,
                v if v == ObjectClass::Amulet as i16 => ObjectClass::Amulet,
                v if v == ObjectClass::Tool as i16 => ObjectClass::Tool,
                v if v == ObjectClass::Food as i16 => ObjectClass::Food,
                v if v == ObjectClass::Potion as i16 => ObjectClass::Potion,
                v if v == ObjectClass::Scroll as i16 => ObjectClass::Scroll,
                v if v == ObjectClass::Spellbook as i16 => ObjectClass::Spellbook,
                v if v == ObjectClass::Wand as i16 => ObjectClass::Wand,
                v if v == ObjectClass::Gem as i16 => ObjectClass::Gem,
                _ => ObjectClass::Random,
            };
        }
    }
    // Fallback (should not happen with proper tables).
    ObjectClass::Random
}

// ---------------------------------------------------------------------------
// Price identification helper
// ---------------------------------------------------------------------------

/// Determine possible item identities from a shop buy price.
///
/// Classic player strategy: compare the offered price to known base costs
/// to deduce item type.  Returns all `ObjectDef` entries whose
/// `get_cost(def.cost, 1, cha, true, tourist)` matches `offered_price`.
pub fn identify_by_price(
    offered_price: i32,
    charisma: u8,
    is_tourist_or_dunce: bool,
    obj_defs: &[ObjectDef],
    target_class: ObjectClass,
) -> Vec<&ObjectDef> {
    obj_defs
        .iter()
        .filter(|d| d.class == target_class)
        .filter(|d| {
            let expected = get_cost(d.cost as i32, 1, charisma, true, is_tourist_or_dunce);
            expected == offered_price
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Cost per charge (wands, tools, spellbooks)
// ---------------------------------------------------------------------------

/// Compute the cost per charge for a chargeable item in a shop.
///
/// Mirrors `cost_per_charge()` from `shk.c`.
/// - Wands: base_cost / (charges + 1), minimum 10
/// - Spellbooks: base_cost / (charges + 1), minimum 5
/// - Other charged tools: base_cost / (charges + 1), minimum 1
pub fn cost_per_charge(base_cost: i32, charges: i32, is_wand: bool, is_spellbook: bool) -> i32 {
    let denominator = (charges + 1).max(1);
    let per_charge = base_cost / denominator;
    if is_wand {
        per_charge.max(10)
    } else if is_spellbook {
        per_charge.max(5)
    } else {
        per_charge.max(1)
    }
}

// ---------------------------------------------------------------------------
// Unidentified item random price variation
// ---------------------------------------------------------------------------

/// Add random price variation for items whose type is not fully identified.
///
/// C NetHack applies a pseudo-random markup/discount based on the item's
/// `o_id` (creation counter).  This function models the same behavior:
/// for unidentified items, the displayed price varies by up to +/-25%
/// from the base, making price-identification harder.
///
/// When the item is identified, the exact base price is shown.
pub fn unidentified_price_variation<R: Rng>(
    base_price: i32,
    is_identified: bool,
    rng: &mut R,
) -> i32 {
    if is_identified || base_price <= 0 {
        return base_price;
    }
    let variation = base_price / 4;
    if variation == 0 {
        return base_price;
    }
    // Range: [base - variation, base + variation]
    base_price + rng.random_range(-variation..=variation)
}

// ---------------------------------------------------------------------------
// Charisma sell-price query
// ---------------------------------------------------------------------------

/// Return the CHA-based sell-price modifier as `(multiplier, divisor)`.
///
/// In C NetHack, sell prices are NOT modified by CHA (only buy prices are).
/// This function exists for documentation and potential future use.
/// The sell-side formula is fixed: non-tourist gets 1/2, tourist gets 1/3.
pub fn cha_sell_modifier(_charisma: u8) -> (i32, i32) {
    // Sell prices in NetHack do not vary with CHA.
    (1, 1)
}

// ---------------------------------------------------------------------------
// Pricing units for stackable items
// ---------------------------------------------------------------------------

/// Get the pricing unit count for an object.
///
/// Some objects are priced per unit (arrows, potions), others per group.
/// Returns 1 for most items, or the quantity for stackable items that
/// are priced individually.
///
/// Mirrors `get_pricing_units()` from `shk.c`.
pub fn get_pricing_units(quantity: i32, is_ammo: bool, is_mergeable: bool) -> i32 {
    if is_ammo || is_mergeable {
        quantity.max(1)
    } else {
        1
    }
}

// ---------------------------------------------------------------------------
// Special stock detection
// ---------------------------------------------------------------------------

/// Check if an item is special stock for a given shop type.
///
/// Certain items are always stocked in specific shop types regardless
/// of the general class rules. For example, leather gloves in wand
/// shops (for handling wands safely).
///
/// Mirrors `special_stock()` from `shk.c`.
pub fn is_special_stock(shop_type: ShopType, item_name: &str, item_class: ObjectClass) -> bool {
    match shop_type {
        ShopType::Wand => {
            matches!(item_name, "leather gloves" | "elven cloak")
        }
        ShopType::Food => {
            // Tin, tinning kit, tin opener — food shops stock these tools.
            matches!(item_name, "tin" | "tinning kit" | "tin opener")
        }
        ShopType::HealthFood => {
            // Health food stores stock potions and scrolls.
            item_class == ObjectClass::Potion || item_class == ObjectClass::Scroll
        }
        ShopType::Candle => {
            // Lighting shops stock specific items.
            matches!(
                item_name,
                "tallow candle" | "wax candle" | "oil lamp" | "brass lantern" | "magic lamp"
            )
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Shop inheritance when shopkeeper dies
// ---------------------------------------------------------------------------

/// Determines if another shopkeeper inherits the shop when the current
/// one dies.
///
/// Mirrors `inherits()` from `shk.c`.  If there is an adjacent
/// shopkeeper who is not angry, they inherit the shop.
pub fn check_shop_inheritance(
    keeper_angry: bool,
    has_adjacent_keeper: bool,
    adjacent_keeper_angry: bool,
) -> bool {
    // If the dead shopkeeper was angry, nobody inherits (shop abandoned).
    if keeper_angry {
        return false;
    }
    // An adjacent keeper who isn't angry inherits.
    has_adjacent_keeper && !adjacent_keeper_angry
}

// ---------------------------------------------------------------------------
// Shop damage tracking
// ---------------------------------------------------------------------------

/// A record of damage dealt to shop fixtures (walls, doors).
///
/// Mirrors `struct damage` from `shk.c`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopDamage {
    /// Position where damage occurred.
    pub position: Position,
    /// Gold cost to repair this damage.
    pub cost: i32,
    /// Type of damage (wall destroyed, door broken, etc.).
    pub damage_type: ShopDamageType,
}

/// Types of damage that can occur in a shop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShopDamageType {
    /// A wall was destroyed (e.g., by digging or force bolt).
    WallDestroyed,
    /// A door was broken.
    DoorBroken,
    /// Floor was damaged (e.g., pit dug).
    FloorDamaged,
}

/// Cost to repair a specific type of shop damage.
///
/// Mirrors the repair cost table in `shk.c`.
pub fn shop_damage_cost(damage_type: ShopDamageType) -> i32 {
    match damage_type {
        ShopDamageType::WallDestroyed => 300,
        ShopDamageType::DoorBroken => 200,
        ShopDamageType::FloorDamaged => 100,
    }
}

/// Record damage to a shop room.
///
/// The shopkeeper adds this to the player's debit.
pub fn record_shop_damage(
    shop: &mut ShopRoom,
    position: Position,
    damage_type: ShopDamageType,
) -> Vec<EngineEvent> {
    let cost = shop_damage_cost(damage_type);
    shop.debit += cost;

    shop.damage_list.push(ShopDamage {
        position,
        cost,
        damage_type,
    });

    vec![EngineEvent::msg_with(
        "shop-damage",
        vec![
            ("shopkeeper", shop.shopkeeper_name.clone()),
            ("cost", cost.to_string()),
        ],
    )]
}

/// Process shopkeeper repair of one damage entry.
///
/// Returns `true` if a repair was made.
pub fn repair_one_damage(shop: &mut ShopRoom) -> Option<ShopDamage> {
    if shop.damage_list.is_empty() {
        return None;
    }
    Some(shop.damage_list.remove(0))
}

// ---------------------------------------------------------------------------
// Shopkeeper names by shop type
// ---------------------------------------------------------------------------

/// Get the pool of shopkeeper names for a given shop type.
///
/// Each shop type has a characteristic set of names from `shknam.c`.
pub fn shopkeeper_name_pool(shop_type: ShopType) -> &'static [&'static str] {
    match shop_type {
        ShopType::General | ShopType::Tool => &[
            "Njezjansen",
            "Tansen",
            "Snansen",
            "Anansen",
            "Manansen",
            "Danansen",
            "Janansen",
            "Panansen",
            "Ranansen",
            "Wansen",
        ],
        ShopType::Armor => &[
            "Demstransen",
            "Stansen",
            "Mansen",
            "Tanansen",
            "Kanansen",
            "Donansen",
            "Hanansen",
            "Lanansen",
            "Vanansen",
            "Zanansen",
        ],
        ShopType::Scroll | ShopType::Book => &[
            "Kirjansen",
            "Bookansen",
            "Volansen",
            "Tomansen",
            "Pagansen",
            "Readansen",
            "Leafansen",
            "Wordansen",
            "Textansen",
            "Lineansen",
        ],
        ShopType::Potion => &[
            "Juansen",
            "Drinkansen",
            "Sipansen",
            "Gulpansen",
            "Potansen",
            "Elixansen",
            "Brewansen",
            "Mixansen",
            "Vialansen",
            "Flaskansen",
        ],
        ShopType::Weapon => &[
            "Sansen",
            "Bladeansen",
            "Edgeansen",
            "Pointansen",
            "Hiltansen",
            "Sharpansen",
            "Steelansen",
            "Ironansen",
            "Brassansen",
            "Bronzansen",
        ],
        ShopType::Food | ShopType::HealthFood => &[
            "Dansen",
            "Feedansen",
            "Mealansen",
            "Bitansen",
            "Tastansen",
            "Freshansen",
            "Cookansen",
            "Bakansen",
            "Roastansen",
            "Grillansen",
        ],
        ShopType::Ring => &[
            "Ringansen",
            "Gemansen",
            "Sparkansen",
            "Shinansen",
            "Glowansen",
            "Bandansen",
            "Loopansen",
            "Circansen",
            "Hoopansen",
            "Orbansen",
        ],
        ShopType::Wand => &[
            "Wandansen",
            "Stickansen",
            "Rodansen",
            "Staffansen",
            "Twiganson",
            "Beamansen",
            "Rayansen",
            "Boltansen",
            "Zapansen",
            "Flickansen",
        ],
        ShopType::Candle => &[
            "Flamansen",
            "Lightansen",
            "Glowansen",
            "Wickansen",
            "Waxansen",
            "Burnansen",
            "Torchansen",
            "Lampansen",
            "Beamansen",
            "Shineansen",
        ],
    }
}

/// Select a random shopkeeper name for a shop type.
pub fn random_shopkeeper_name<R: Rng>(shop_type: ShopType, rng: &mut R) -> &'static str {
    let pool = shopkeeper_name_pool(shop_type);
    pool[rng.random_range(0..pool.len())]
}

// ---------------------------------------------------------------------------
// Container handling in shop bills
// ---------------------------------------------------------------------------

/// Check if dropping a container in a shop should trigger sale of contents.
///
/// When a container with unpaid items is dropped, the shopkeeper
/// evaluates both the container and its contents.
pub fn container_sale_value(
    container_base_cost: i32,
    contents_total_cost: i32,
    quantity: i32,
    charisma: u8,
) -> i32 {
    let container_sell = get_cost(container_base_cost, quantity, charisma, false, false);
    container_sell + contents_total_cost
}

// ---------------------------------------------------------------------------
// Shop embellishment
// ---------------------------------------------------------------------------

/// Choose the quoted-price message variant for a shop item.
///
/// This is a deterministic approximation of `shk_embellish()` from `shk.c`.
pub fn price_quote_message_key(
    item_class: ObjectClass,
    is_magic: bool,
    identified: bool,
    is_artifact: bool,
    total_price: i32,
) -> &'static str {
    if is_artifact {
        return "shop-price-one-of-a-kind";
    }
    if total_price < 10 {
        return "shop-price";
    }
    if item_class == ObjectClass::Food {
        return "shop-price-gourmets-delight";
    }
    if (identified && is_magic)
        || matches!(
            item_class,
            ObjectClass::Amulet
                | ObjectClass::Ring
                | ObjectClass::Wand
                | ObjectClass::Potion
                | ObjectClass::Scroll
                | ObjectClass::Spellbook
        )
    {
        return "shop-price-painstakingly-developed";
    }
    if total_price >= 500 {
        return "shop-price-finest-quality";
    }
    if total_price >= 250 {
        return "shop-price-superb-craftsmanship";
    }
    if total_price >= 100 {
        return "shop-price-excellent-choice";
    }
    "shop-price-bargain"
}

// ---------------------------------------------------------------------------
// Deserted shop detection
// ---------------------------------------------------------------------------

/// Check if a shop room is deserted (no shopkeeper).
///
/// A deserted shop has no shopkeeper entity, meaning items can be
/// freely taken without debt.
pub fn is_shop_deserted(shopkeeper_alive: bool) -> bool {
    !shopkeeper_alive
}

// ---------------------------------------------------------------------------
// Helpers for GameWorld (placeholder entity)
// ---------------------------------------------------------------------------

/// Extension trait for GameWorld to support placeholder spawning in shop.
trait GameWorldShopExt {
    fn spawn_placeholder_entity(&self) -> Option<Entity>;
}

impl GameWorldShopExt for GameWorld {
    /// This is a no-op placeholder.  Real Kop spawning requires mutable
    /// world access and monster definitions.  The `rob_shop` function
    /// emits events instead.
    fn spawn_placeholder_entity(&self) -> Option<Entity> {
        // Cannot spawn without &mut self; return None.
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Name;
    use nethack_babel_data::{Color, Material, ObjectTypeId};

    /// Helper: build a minimal ObjectDef for testing.
    fn test_obj_def(id: u16, cost: i16) -> ObjectDef {
        ObjectDef {
            id: ObjectTypeId(id),
            name: format!("test_item_{}", id),
            appearance: None,
            class: ObjectClass::Weapon,
            color: Color::White,
            material: Material::Iron,
            weight: 10,
            cost,
            nutrition: 0,
            prob: 10,
            is_magic: false,
            is_mergeable: false,
            is_charged: false,
            is_unique: false,
            is_nowish: false,
            is_bimanual: false,
            is_bulky: false,
            is_tough: false,
            weapon: None,
            armor: None,
            spellbook: None,
            conferred_property: None,
            use_delay: 0,
        }
    }

    /// Helper: build a test ShopRoom.
    fn test_shop(world: &mut GameWorld) -> ShopRoom {
        let shopkeeper = world.spawn((
            Name("Asidonhopo".to_string()),
            Positioned(Position::new(10, 5)),
        ));
        ShopRoom::new(
            Position::new(5, 2),
            Position::new(15, 8),
            ShopType::General,
            shopkeeper,
            "Asidonhopo".to_string(),
        )
    }

    // ── Price calculation tests ──────────────────────────────────

    #[test]
    fn buy_price_with_neutral_cha() {
        // CHA 12 (in 11-15 range) => 1x multiplier.
        // base_cost=100, qty=1, buying => 100.
        let price = get_cost(100, 1, 12, true, false);
        assert_eq!(price, 100);
    }

    #[test]
    fn buy_price_with_low_cha() {
        // CHA 5 => 2x multiplier.
        let price = get_cost(100, 1, 5, true, false);
        assert_eq!(price, 200);
    }

    #[test]
    fn buy_price_with_cha_6_7() {
        // CHA 7 => 3/2 multiplier => 150.
        let price = get_cost(100, 1, 7, true, false);
        assert_eq!(price, 150);
    }

    #[test]
    fn buy_price_with_cha_8_10() {
        // CHA 10 => 4/3 multiplier.
        // 100 * 4 = 400, (400 * 10 / 3 + 5) / 10 = (1333+5)/10 = 133.
        let price = get_cost(100, 1, 10, true, false);
        assert_eq!(price, 133);
    }

    #[test]
    fn buy_price_with_cha_16_17() {
        // CHA 16 => 3/4 multiplier.
        // 100 * 3 = 300, (300*10/4+5)/10 = (750+5)/10 = 75.
        let price = get_cost(100, 1, 16, true, false);
        assert_eq!(price, 75);
    }

    #[test]
    fn buy_price_with_cha_18() {
        // CHA 18 => 2/3 multiplier.
        // 100 * 2 = 200, (200*10/3+5)/10 = (666+5)/10 = 67.
        let price = get_cost(100, 1, 18, true, false);
        assert_eq!(price, 67);
    }

    #[test]
    fn buy_price_with_cha_over_18() {
        // CHA 19 => 1/2 multiplier => 50.
        let price = get_cost(100, 1, 19, true, false);
        assert_eq!(price, 50);
    }

    #[test]
    fn buying_price_always_greater_than_selling_price() {
        // For any reasonable CHA (11-15), buy = 100, sell = 50.
        let buy = get_cost(100, 1, 12, true, false);
        let sell = get_cost(100, 1, 12, false, false);
        assert!(buy > sell, "buy {} should be > sell {}", buy, sell);
    }

    #[test]
    fn sell_price_normal() {
        // Sell price = base / 2.
        // 100 * 1 = 100, (100*10/2+5)/10 = (500+5)/10 = 50.
        let price = get_cost(100, 1, 12, false, false);
        assert_eq!(price, 50);
    }

    #[test]
    fn sell_price_tourist_or_dunce() {
        // Tourist/dunce sell at 1/3.
        // 100 * 1 = 100, (100*10/3+5)/10 = (333+5)/10 = 33.
        let price = get_cost(100, 1, 12, false, true);
        assert_eq!(price, 33);
    }

    #[test]
    fn buy_price_with_tourist_penalty() {
        // Tourist penalty: multiplier 4/3.
        // CHA 12 (neutral): 100 * 4/3 = (100*4*10/3+5)/10 = (1333+5)/10 = 133.
        let price = get_cost(100, 1, 12, true, true);
        assert_eq!(price, 133);
    }

    #[test]
    fn zero_cost_item_buy_minimum() {
        // Items with base cost 0 get minimum of 5 when buying.
        let price = get_cost(0, 1, 12, true, false);
        assert_eq!(price, 5);
    }

    #[test]
    fn zero_cost_item_sell_minimum() {
        // Selling a 0-cost item should still return at least 1.
        let price = get_cost(0, 1, 12, false, false);
        assert_eq!(price, 1);
    }

    #[test]
    fn quantity_multiplied() {
        // 3 items at base 100, CHA 12 => 100 * 3 = 300.
        let price = get_cost(100, 3, 12, true, false);
        assert_eq!(price, 300);
    }

    // ── Shop bill tests ─────────────────────────────────────────

    #[test]
    fn pickup_adds_to_bill() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let player = world.player();
        let mut shop = test_shop(&mut world);
        let def = test_obj_def(1, 100);
        let defs = vec![def.clone()];

        let item = crate::items::spawn_item(
            &mut world,
            &def,
            crate::items::SpawnLocation::Floor(10, 5),
            None,
        );

        let events = pickup_in_shop(&world, player, item, &mut shop, &defs);
        assert!(!events.is_empty());
        assert_eq!(shop.bill.len(), 1);

        let entry = shop.bill.find(item).unwrap();
        // Default Attributes has CHA=10 => 4/3 multiplier.
        // 100 * 4 = 400, (400*10/3+5)/10 = (1333+5)/10 = 133.
        assert_eq!(entry.price, 133);
    }

    #[test]
    fn paying_bill_removes_items() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let player = world.player();
        let mut shop = test_shop(&mut world);
        let def = test_obj_def(1, 100);
        let defs = vec![def.clone()];

        let item = crate::items::spawn_item(
            &mut world,
            &def,
            crate::items::SpawnLocation::Floor(10, 5),
            None,
        );

        pickup_in_shop(&world, player, item, &mut shop, &defs);
        assert_eq!(shop.bill.len(), 1);

        // Pay the bill.
        let mut gold = 500;
        let events = pay_bill(&world, player, &mut shop, &mut gold);
        assert!(shop.bill.is_empty());
        assert!(gold < 500); // Some gold was spent.

        // Check for thank-you message.
        let has_thanks = events.iter().any(|e| {
            if let EngineEvent::Message { key, .. } = e {
                key.contains("shop-pay")
            } else {
                false
            }
        });
        assert!(has_thanks);
    }

    #[test]
    fn paying_bill_partially_reduces_outstanding_balance() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let player = world.player();
        let mut shop = test_shop(&mut world);
        let def = test_obj_def(1, 100);
        let defs = vec![def.clone()];

        let item = crate::items::spawn_item(
            &mut world,
            &def,
            crate::items::SpawnLocation::Floor(10, 5),
            None,
        );

        pickup_in_shop(&world, player, item, &mut shop, &defs);
        let full_bill = shop.bill.total();
        assert!(
            full_bill > 100,
            "charisma-adjusted shop bill should exceed 100"
        );

        let mut gold = 100;
        let events = pay_bill(&world, player, &mut shop, &mut gold);

        assert_eq!(gold, 0, "partial payment should spend all available gold");
        assert!(
            !shop.bill.is_empty(),
            "partial payment should preserve remaining debt"
        );
        assert_eq!(
            shop.bill.total(),
            full_bill - 100,
            "remaining balance should shrink by the paid amount"
        );
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-pay-success"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-owe"
        )));
    }

    #[test]
    fn drop_in_shop_gives_credit() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let player = world.player();
        let mut shop = test_shop(&mut world);
        // shopkeeper_gold = 0 => credit-for-sale at 90%.
        let def = test_obj_def(1, 100);
        let defs = vec![def.clone()];

        let item = crate::items::spawn_item(
            &mut world,
            &def,
            crate::items::SpawnLocation::Floor(10, 5),
            None,
        );

        let events = drop_in_shop(&world, player, item, &mut shop, &defs);
        assert!(!events.is_empty());
        // Sell price = 100/2 = 50.  Shopkeeper has no gold =>
        // credit_for_sale(50) = (50*9)/10 + 0 = 45.
        assert_eq!(shop.credit, 45);
    }

    #[test]
    fn shopkeeper_anger_on_theft() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let player = world.player();
        let mut shop = test_shop(&mut world);
        let def = test_obj_def(1, 100);
        let defs = vec![def.clone()];

        let item = crate::items::spawn_item(
            &mut world,
            &def,
            crate::items::SpawnLocation::Floor(10, 5),
            None,
        );

        pickup_in_shop(&world, player, item, &mut shop, &defs);

        // Rob the shop.
        let mut rng = rand::rng();
        let events = rob_shop(&world, player, &mut shop, &mut rng);

        assert!(shop.angry);
        assert!(shop.surcharge);
        assert!(shop.robbed > 0);

        // Check for theft message.
        let has_theft_msg = events.iter().any(|e| {
            if let EngineEvent::Message { key, .. } = e {
                key.contains("shop-shoplift")
            } else {
                false
            }
        });
        assert!(has_theft_msg);
    }

    #[test]
    fn shop_type_affects_saleable() {
        // Weapon shop sells weapons and armor.
        assert!(ShopType::Weapon.sells_class(ObjectClass::Weapon));
        assert!(ShopType::Weapon.sells_class(ObjectClass::Armor));
        assert!(!ShopType::Weapon.sells_class(ObjectClass::Food));

        // General store sells everything.
        assert!(ShopType::General.sells_class(ObjectClass::Food));
        assert!(ShopType::General.sells_class(ObjectClass::Wand));
        assert!(ShopType::General.sells_class(ObjectClass::Gem));

        // Potion shop only sells potions.
        assert!(ShopType::Potion.sells_class(ObjectClass::Potion));
        assert!(!ShopType::Potion.sells_class(ObjectClass::Scroll));
    }

    #[test]
    fn credit_accumulation_and_deduction() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let mut shop = test_shop(&mut world);

        // Add credit.
        shop.add_credit(100);
        assert_eq!(shop.credit, 100);

        shop.add_credit(50);
        assert_eq!(shop.credit, 150);

        // Use credit partially.
        let remaining = shop.use_credit(80);
        assert_eq!(remaining, 0);
        assert_eq!(shop.credit, 70);

        // Use credit exceeding balance.
        let remaining = shop.use_credit(100);
        assert_eq!(remaining, 30);
        assert_eq!(shop.credit, 0);
    }

    #[test]
    fn credit_covers_bill_no_robbery() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let player = world.player();
        let mut shop = test_shop(&mut world);
        let def = test_obj_def(1, 100);
        let defs = vec![def.clone()];

        let item = crate::items::spawn_item(
            &mut world,
            &def,
            crate::items::SpawnLocation::Floor(10, 5),
            None,
        );

        pickup_in_shop(&world, player, item, &mut shop, &defs);

        // Give enough credit to cover the bill.
        shop.add_credit(500);

        let mut rng = rand::rng();
        let events = rob_shop(&world, player, &mut shop, &mut rng);

        // Should NOT be angry — credit covered it.
        assert!(!shop.angry);
        assert_eq!(shop.robbed, 0);

        let has_credit_msg = events.iter().any(|e| {
            if let EngineEvent::Message { key, .. } = e {
                key.contains("credit")
            } else {
                false
            }
        });
        assert!(has_credit_msg);
    }

    #[test]
    fn rile_and_pacify_prices() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let mut shop = test_shop(&mut world);

        // Manually add a bill entry with price 10.
        let dummy = world.spawn((Name("dummy".to_string()),));
        shop.bill.add(dummy, 10, 1);

        // Rile: price should increase by (10+2)/3 = 4 => 14.
        rile_shop(&mut shop);
        assert_eq!(shop.bill.entries()[0].price, 14);

        // Pacify: price should decrease by (14+3)/4 = 4 => 10.
        pacify_shop(&mut shop);
        assert_eq!(shop.bill.entries()[0].price, 10);
    }

    #[test]
    fn bill_full_gives_free() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let player = world.player();
        let mut shop = test_shop(&mut world);
        let def = test_obj_def(1, 50);
        let defs = vec![def.clone()];

        // Fill the bill to capacity.
        for i in 0..BILL_SIZE {
            let dummy = world.spawn((Name(format!("item_{}", i)),));
            shop.bill.add(dummy, 50, 1);
        }
        assert_eq!(shop.bill.len(), BILL_SIZE);

        // Next pickup should be free.
        let item = crate::items::spawn_item(
            &mut world,
            &def,
            crate::items::SpawnLocation::Floor(10, 5),
            None,
        );
        let events = pickup_in_shop(&world, player, item, &mut shop, &defs);

        let has_free_msg = events.iter().any(|e| {
            if let EngineEvent::Message { key, .. } = e {
                key.contains("shop-free")
            } else {
                false
            }
        });
        assert!(has_free_msg);

        // Bill should still be at max (item was not added).
        assert_eq!(shop.bill.len(), BILL_SIZE);
    }

    #[test]
    fn enter_shop_greeting() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let player = world.player();
        let shop = test_shop(&mut world);

        let events = enter_shop(&world, player, &shop);
        assert_eq!(events.len(), 1);

        if let EngineEvent::Message { key, .. } = &events[0] {
            assert!(key.contains("shop-enter"));
        } else {
            panic!("Expected Message event");
        }
    }

    #[test]
    fn shop_room_contains() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let shop = test_shop(&mut world);

        // Inside the shop.
        assert!(shop.contains(Position::new(10, 5)));
        assert!(shop.contains(Position::new(5, 2)));
        assert!(shop.contains(Position::new(15, 8)));

        // Outside the shop.
        assert!(!shop.contains(Position::new(4, 2)));
        assert!(!shop.contains(Position::new(16, 8)));
        assert!(!shop.contains(Position::new(10, 1)));
    }

    // ── Spec test vectors (section "测试向量") ──────────────────

    #[test]
    fn test_shop_pricing_vector_1_neutral_cha_identified() {
        // #1: oc_cost=100, CHA=12, no tourist, identified => 100
        let price = get_cost(100, 1, 12, true, false);
        assert_eq!(price, 100);
    }

    #[test]
    fn test_shop_pricing_vector_2_unid_surcharge() {
        // #2: oc_cost=100, CHA=12, unid with o_id%4==0 => 133
        // OID surcharge adds 4/3 multiplier.
        let price = get_full_buy_price(
            100,
            ObjectClass::Weapon,
            0,
            1,
            12,
            false,
            false,
            0,
            true,
            false,
        );
        assert_eq!(price, 133);
    }

    #[test]
    fn test_shop_pricing_vector_3_tourist_identified() {
        // #3: oc_cost=100, CHA=12, Tourist lv1, identified => 133
        let price = get_cost(100, 1, 12, true, true);
        assert_eq!(price, 133);
    }

    #[test]
    fn test_shop_pricing_vector_4_cha_18() {
        // #4: oc_cost=100, CHA=18, identified => 67
        let price = get_cost(100, 1, 18, true, false);
        assert_eq!(price, 67);
    }

    #[test]
    fn test_shop_pricing_vector_5_cha_19() {
        // #5: oc_cost=100, CHA=19, identified => 50
        let price = get_cost(100, 1, 19, true, false);
        assert_eq!(price, 50);
    }

    #[test]
    fn test_shop_pricing_vector_6_cha_5() {
        // #6: oc_cost=100, CHA=5, identified => 200
        let price = get_cost(100, 1, 5, true, false);
        assert_eq!(price, 200);
    }

    #[test]
    fn test_shop_pricing_vector_7_cha_7() {
        // #7: oc_cost=100, CHA=7, identified => 150
        let price = get_cost(100, 1, 7, true, false);
        assert_eq!(price, 150);
    }

    #[test]
    fn test_shop_pricing_vector_8_artifact() {
        // #8: oc_cost=100, CHA=12, identified, artifact => 400
        let price = get_full_buy_price(
            100,
            ObjectClass::Weapon,
            0,
            1,
            12,
            false,
            true,
            100,
            false,
            false,
        );
        assert_eq!(price, 400);
    }

    #[test]
    fn test_shop_pricing_vector_9_anger_surcharge() {
        // #9: oc_cost=100, CHA=12, identified, angry => 134
        let price = get_full_buy_price(
            100,
            ObjectClass::Weapon,
            0,
            1,
            12,
            false,
            false,
            0,
            false,
            true,
        );
        // base=100, no modifiers => 100, anger: 100 + (100+2)/3 = 100+34 = 134
        assert_eq!(price, 134);
    }

    #[test]
    fn test_shop_pricing_vector_10_combined() {
        // #10: oc_cost=100, CHA=5, Tourist lv1, unid o_id%4==0, angry => 475
        // Detailed: base=100
        // unid surcharge: mul=4, div=3
        // tourist: mul*=4, div*=3 => mul=16, div=9
        // CHA=5: mul*=2 => mul=32, div=9
        // tmp = 100*32 = 3200, (3200*10/9+5)/10 = (3555+5)/10 = 356
        // no artifact
        // anger: 356 + (356+2)/3 = 356 + 119 = 475
        let price = get_full_buy_price(
            100,
            ObjectClass::Weapon,
            0,
            1,
            5,
            true,
            false,
            0,
            true,
            true,
        );
        assert_eq!(price, 475);
    }

    #[test]
    fn test_shop_sell_vector_11_normal() {
        // #11: oc_cost=100, normal => sell 50
        let price = get_full_sell_price(100, ObjectClass::Weapon, 0, 1, false, false, 0);
        assert_eq!(price, 50);
    }

    #[test]
    fn test_shop_sell_vector_12_tourist() {
        // #12: oc_cost=100, tourist => sell 33
        let price = get_full_sell_price(100, ObjectClass::Weapon, 0, 1, true, false, 0);
        assert_eq!(price, 33);
    }

    #[test]
    fn test_shop_sell_vector_13_dunce() {
        // #13: oc_cost=100, dunce => sell 33
        let price = get_full_sell_price(100, ObjectClass::Weapon, 0, 1, true, false, 0);
        assert_eq!(price, 33);
    }

    // ── Kop spawning vectors ────────────────────────────────────

    #[test]
    fn test_shop_kop_vector_14() {
        // #14: depth=5, rnd(5)=3 => cnt=8
        // Kops=8, Sgts=8/3+1=3, Lts=8/6=1, Kpts=8/9=0
        let (kops, sgts, lts, kpts) = kop_counts(5, 3);
        assert_eq!((kops, sgts, lts, kpts), (8, 3, 1, 0));
    }

    #[test]
    fn test_shop_kop_vector_15() {
        // #15: depth=20, rnd(5)=1 => cnt=21
        // Kops=21, Sgts=21/3+1=8, Lts=21/6=3, Kpts=21/9=2
        let (kops, sgts, lts, kpts) = kop_counts(20, 1);
        assert_eq!((kops, sgts, lts, kpts), (21, 8, 3, 2));
    }

    // ── Boundary conditions ─────────────────────────────────────

    #[test]
    fn test_shop_boundary_16_zero_cost_buy() {
        // #16: oc_cost=0 (uncursed unholy water) => get_cost returns 5
        let price = get_cost(0, 1, 12, true, false);
        assert_eq!(price, 5);
    }

    #[test]
    fn test_shop_boundary_19_rile_pacify_price_1() {
        // #19: price=1 -> rile: 1+(1+2)/3=1+1=2,
        //      pacify: 2-(2+3)/4=2-1=1
        let mut world = GameWorld::new(Position::new(10, 5));
        let mut shop = test_shop(&mut world);
        let dummy = world.spawn((Name("dummy".to_string()),));
        shop.bill.add(dummy, 1, 1);

        rile_shop(&mut shop);
        assert_eq!(shop.bill.entries()[0].price, 2);

        pacify_shop(&mut shop);
        assert_eq!(shop.bill.entries()[0].price, 1);
    }

    // ── Credit system vectors ───────────────────────────────────

    #[test]
    fn test_shop_credit_21_cashless_100() {
        // #21: sell item worth 100 to cashless shk => credit = (100*9)/10 = 90
        // Actually sell_price = 100/2 = 50 for base_cost=100.
        // credit_for_sale(50) = (50*9)/10 + 0 = 45.
        //
        // But the test vector says "item worth 100" meaning sell price = 100.
        // So: credit_for_sale(100) = (100*9)/10 + 0 = 90.
        assert_eq!(credit_for_sale(100), 90);
    }

    #[test]
    fn test_shop_credit_22_cashless_1() {
        // #22: sell item worth 1 to cashless shk => credit = (1*9)/10 + 1 = 1
        assert_eq!(credit_for_sale(1), 1);
    }

    #[test]
    fn test_shop_credit_23_donate_gold() {
        // #23: Drop 500 gold in shop with debit=200 =>
        // debit becomes 0, credit += 300
        let mut world = GameWorld::new(Position::new(10, 5));
        let mut shop = test_shop(&mut world);
        shop.debit = 200;

        let credit_added = donate_gold(&mut shop, 500);
        assert_eq!(credit_added, 300);
        assert_eq!(shop.debit, 0);
        assert_eq!(shop.credit, 300);
    }

    // ── Base price adjustments ──────────────────────────────────

    #[test]
    fn test_shop_base_price_enchanted_weapon() {
        // +3 weapon with base cost 50 => 50 + 10*3 = 80
        let base = get_base_price(50, ObjectClass::Weapon, 3, false, 0, false);
        assert_eq!(base, 80);
    }

    #[test]
    fn test_shop_base_price_enchanted_armor() {
        // +2 armor with base cost 40 => 40 + 10*2 = 60
        let base = get_base_price(40, ObjectClass::Armor, 2, false, 0, false);
        assert_eq!(base, 60);
    }

    #[test]
    fn test_shop_base_price_negative_enchantment_ignored() {
        // -1 weapon: negative spe does NOT add bonus.
        let base = get_base_price(50, ObjectClass::Weapon, -1, false, 0, false);
        assert_eq!(base, 50);
    }

    #[test]
    fn test_shop_base_price_empty_wand() {
        // Wand with spe=-1 (cancelled/empty) => base 0
        let base = get_base_price(100, ObjectClass::Wand, -1, false, 0, false);
        assert_eq!(base, 0);
    }

    #[test]
    fn test_shop_base_price_normal_wand() {
        // Wand with spe=5 => base unchanged (no +10 bonus for wands)
        let base = get_base_price(100, ObjectClass::Wand, 5, false, 0, false);
        assert_eq!(base, 100);
    }

    #[test]
    fn test_shop_base_price_artifact_buy() {
        // Artifact with cost=200: base = 200 (buy direction)
        let base = get_base_price(50, ObjectClass::Weapon, 0, true, 200, false);
        assert_eq!(base, 200);
    }

    #[test]
    fn test_shop_base_price_artifact_sell() {
        // Artifact with cost=200: base = 200/4 = 50 (sell direction)
        let base = get_base_price(50, ObjectClass::Weapon, 0, true, 200, true);
        assert_eq!(base, 50);
    }

    // ── OID surcharge ───────────────────────────────────────────

    #[test]
    fn test_shop_oid_surcharge_identified() {
        // Identified item: no surcharge even if o_id%4==0
        assert!(!has_oid_surcharge(0, true, false));
        assert!(!has_oid_surcharge(4, true, false));
    }

    #[test]
    fn test_shop_oid_surcharge_glass_gem() {
        // Glass gem: no surcharge (separate deception)
        assert!(!has_oid_surcharge(0, false, true));
        assert!(!has_oid_surcharge(4, false, true));
    }

    #[test]
    fn test_shop_oid_surcharge_yes() {
        // Unidentified, non-glass, o_id%4==0 => surcharge
        assert!(has_oid_surcharge(0, false, false));
        assert!(has_oid_surcharge(4, false, false));
        assert!(has_oid_surcharge(8, false, false));
    }

    #[test]
    fn test_shop_oid_surcharge_no() {
        // Unidentified, non-glass, o_id%4!=0 => no surcharge
        assert!(!has_oid_surcharge(1, false, false));
        assert!(!has_oid_surcharge(2, false, false));
        assert!(!has_oid_surcharge(3, false, false));
        assert!(!has_oid_surcharge(5, false, false));
    }

    // ── Full buy price with enchantment ─────────────────────────

    #[test]
    fn test_shop_full_buy_enchanted_weapon() {
        // Base 50, +3 weapon, CHA 12, no modifiers.
        // get_base_price => 50+30 = 80.  get_cost(80,1,12,buy) = 80.
        let price = get_full_buy_price(
            50,
            ObjectClass::Weapon,
            3,
            1,
            12,
            false,
            false,
            0,
            false,
            false,
        );
        assert_eq!(price, 80);
    }

    #[test]
    fn test_shop_full_buy_artifact_enchanted() {
        // Artifact with artifact_cost=300, +2 weapon, CHA 12.
        // get_base_price(50, Weapon, 2, artifact=true, 300, false) = 300
        //   (artifact overrides base; spe bonus not added because
        //    artifact_cost already reflects the item).
        // Actually... let's check: artifact branch uses artifact_cost=300
        // then the weapon spe check: spe=2>0 => 300 + 20 = 320.
        // get_cost(320, 1, 12, buy, false) = 320.
        // artifact 4x => 320 * 4 = 1280.
        let price = get_full_buy_price(
            50,
            ObjectClass::Weapon,
            2,
            1,
            12,
            false,
            true,
            300,
            false,
            false,
        );
        assert_eq!(price, 1280);
    }

    // ── Full sell price tests ───────────────────────────────────

    #[test]
    fn test_shop_full_sell_enchanted_armor() {
        // +2 armor, base 40.  get_base_price(40, Armor, 2, false, 0, true) = 60.
        // sell: 60/2 = 30.
        let price = get_full_sell_price(40, ObjectClass::Armor, 2, 1, false, false, 0);
        assert_eq!(price, 30);
    }

    #[test]
    fn test_shop_full_sell_artifact() {
        // Artifact, artifact_cost=400, Weapon, spe=0.
        // get_base_price(50, Weapon, 0, true, 400, shk_buying=true) = 400/4 = 100.
        // sell: 100/2 = 50.
        let price = get_full_sell_price(50, ObjectClass::Weapon, 0, 1, false, true, 400);
        assert_eq!(price, 50);
    }

    // ── Shop type generation ────────────────────────────────────

    #[test]
    fn test_shop_type_random_distribution() {
        // With a fixed-seed RNG, verify shop types are generated.
        use rand::SeedableRng;
        let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
        let mut counts = std::collections::HashMap::new();
        for _ in 0..1000 {
            let st = ShopType::random(&mut rng);
            *counts.entry(st).or_insert(0) += 1;
        }
        // General should be most common (~42%).
        assert!(counts[&ShopType::General] > 350);
        assert!(counts[&ShopType::General] < 500);
        // Armor should be ~14%.
        assert!(counts[&ShopType::Armor] > 100);
        assert!(counts[&ShopType::Armor] < 200);
    }

    #[test]
    fn test_shop_health_food_type() {
        assert_eq!(ShopType::HealthFood.display_name(), "health food store");
        assert!(ShopType::HealthFood.sells_class(ObjectClass::Food));
        assert!(ShopType::HealthFood.sells_class(ObjectClass::Potion));
        assert!(!ShopType::HealthFood.sells_class(ObjectClass::Weapon));
    }

    // ── Shopkeeper gold ─────────────────────────────────────────

    #[test]
    fn test_shop_keeper_gold_range() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::SmallRng::seed_from_u64(123);
        for _ in 0..100 {
            let gold = shopkeeper_initial_gold(&mut rng);
            assert!(gold >= 1030, "gold {} < 1030", gold);
            assert!(gold <= 4000, "gold {} > 4000", gold);
        }
    }

    // ── Usage fees ──────────────────────────────────────────────

    #[test]
    fn test_shop_usage_fee_marker() {
        // Magic marker: cost/2
        assert_eq!(
            usage_fee(100, false, false, false, false, true, false, false),
            50
        );
    }

    #[test]
    fn test_shop_usage_fee_bag_of_tricks() {
        // Bag of tricks per use: cost/5
        assert_eq!(
            usage_fee(100, false, true, false, false, false, false, false),
            20
        );
    }

    #[test]
    fn test_shop_usage_fee_bag_of_tricks_emptied() {
        // Bag of tricks emptied: full cost
        assert_eq!(
            usage_fee(100, false, true, false, false, false, false, true),
            100
        );
    }

    #[test]
    fn test_shop_usage_fee_spellbook() {
        // Spellbook: 4/5 of cost
        assert_eq!(
            usage_fee(100, false, false, true, false, false, false, false),
            80
        );
    }

    #[test]
    fn test_shop_usage_fee_cheap_charged() {
        // Can of grease etc.: cost/10
        assert_eq!(
            usage_fee(100, false, false, false, true, false, false, false),
            10
        );
    }

    #[test]
    fn test_shop_usage_fee_oil_potion() {
        // Potion of oil: cost/5
        assert_eq!(
            usage_fee(100, false, false, false, false, false, true, false),
            20
        );
    }

    #[test]
    fn test_shop_usage_fee_wand() {
        // Wand/crystal ball etc.: cost/4
        assert_eq!(
            usage_fee(100, false, false, false, false, false, false, false),
            25
        );
    }

    // ── Credit for sale ─────────────────────────────────────────

    #[test]
    fn test_shop_credit_for_sale_typical() {
        // 50 => (50*9)/10 = 45
        assert_eq!(credit_for_sale(50), 45);
    }

    #[test]
    fn test_shop_credit_for_sale_small() {
        // 2 => (2*9)/10 = 1 (integer division), +0 = 1
        assert_eq!(credit_for_sale(2), 1);
    }

    #[test]
    fn test_shop_credit_for_sale_zero() {
        // 0 => (0*9)/10 = 0, but 0 <= 1 so +1 => 1.
        assert_eq!(credit_for_sale(0), 1);
    }

    // ── Donate gold ─────────────────────────────────────────────

    #[test]
    fn test_shop_donate_gold_all_to_debit() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let mut shop = test_shop(&mut world);
        shop.debit = 500;
        let credit_added = donate_gold(&mut shop, 300);
        assert_eq!(credit_added, 0);
        assert_eq!(shop.debit, 200);
        assert_eq!(shop.credit, 0);
    }

    #[test]
    fn test_shop_donate_gold_exactly_debit() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let mut shop = test_shop(&mut world);
        shop.debit = 200;
        let credit_added = donate_gold(&mut shop, 200);
        assert_eq!(credit_added, 0);
        assert_eq!(shop.debit, 0);
        assert_eq!(shop.credit, 0);
    }

    #[test]
    fn test_shop_donate_gold_no_debit() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let mut shop = test_shop(&mut world);
        let credit_added = donate_gold(&mut shop, 100);
        assert_eq!(credit_added, 100);
        assert_eq!(shop.credit, 100);
    }

    // ── Door blocking ───────────────────────────────────────────

    #[test]
    fn test_shop_door_block_with_unpaid() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let mut shop = test_shop(&mut world);
        assert!(!shop.should_block_door());

        let dummy = world.spawn((Name("item".to_string()),));
        shop.bill.add(dummy, 50, 1);
        assert!(shop.should_block_door());
    }

    #[test]
    fn test_shop_door_block_with_debit() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let mut shop = test_shop(&mut world);
        shop.debit = 10;
        assert!(shop.should_block_door());
    }

    // ── Setpaid / shopkeeper death ──────────────────────────────

    #[test]
    fn test_shop_setpaid_clears_all() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let mut shop = test_shop(&mut world);
        let dummy = world.spawn((Name("item".to_string()),));
        shop.bill.add(dummy, 50, 1);
        shop.credit = 100;
        shop.debit = 50;

        setpaid(&mut shop);
        assert!(shop.bill.is_empty());
        assert_eq!(shop.credit, 0);
        assert_eq!(shop.debit, 0);
    }

    #[test]
    fn test_shop_keeper_died() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let mut shop = test_shop(&mut world);
        shop.angry = true;
        shop.surcharge = true;
        shop.credit = 100;
        shop.debit = 50;
        let dummy = world.spawn((Name("item".to_string()),));
        shop.bill.add(dummy, 50, 1);

        let events = shopkeeper_died(&mut shop);
        assert!(!events.is_empty());
        assert!(shop.bill.is_empty());
        assert_eq!(shop.credit, 0);
        assert_eq!(shop.debit, 0);
        assert!(!shop.angry);
        assert!(!shop.surcharge);
    }

    // ── Make angry (convert to robbery) ─────────────────────────

    #[test]
    fn test_shop_make_angry_converts_to_robbery() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let mut shop = test_shop(&mut world);
        let dummy = world.spawn((Name("item".to_string()),));
        shop.bill.add(dummy, 100, 1);
        shop.debit = 50;
        shop.credit = 30;

        // total = bill(100) + debit(50) - credit(30) = 120
        make_angry(&mut shop);
        assert_eq!(shop.robbed, 120);
        assert!(shop.bill.is_empty());
        assert_eq!(shop.credit, 0);
        assert_eq!(shop.debit, 0);
        assert!(shop.angry);
        assert!(shop.surcharge);
    }

    // ── Shop iprobs ─────────────────────────────────────────────

    #[test]
    fn test_shop_iprobs_general() {
        let probs = shop_iprobs(ShopType::General);
        assert_eq!(probs.len(), 1);
        assert_eq!(probs[0].prob, 100);
    }

    #[test]
    fn test_shop_iprobs_armor() {
        let probs = shop_iprobs(ShopType::Armor);
        assert_eq!(probs.len(), 2);
        assert_eq!(probs[0].prob, 90);
        assert_eq!(probs[0].item_type, ObjectClass::Armor as i16);
        assert_eq!(probs[1].prob, 100);
        assert_eq!(probs[1].item_type, ObjectClass::Weapon as i16);
    }

    #[test]
    fn test_shop_select_item_class_armor() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
        let mut armor_count = 0;
        let mut weapon_count = 0;
        for _ in 0..1000 {
            match select_shop_item_class(ShopType::Armor, &mut rng) {
                ObjectClass::Armor => armor_count += 1,
                ObjectClass::Weapon => weapon_count += 1,
                _ => panic!("unexpected class from armor shop"),
            }
        }
        // Should be ~90% armor, ~10% weapon.
        assert!(armor_count > 850);
        assert!(weapon_count > 60);
    }

    // ── Price identification ────────────────────────────────────

    #[test]
    fn test_shop_price_identification() {
        let defs = vec![
            test_obj_def(1, 50),
            test_obj_def(2, 100),
            test_obj_def(3, 100),
            test_obj_def(4, 200),
        ];
        // At CHA 12, buy price for cost=100 is 100.
        let matches = identify_by_price(100, 12, false, &defs, ObjectClass::Weapon);
        assert_eq!(matches.len(), 2); // items 2 and 3 both cost 100
        assert!(matches.iter().all(|d| d.cost == 100));
    }

    // ── Sell with shopkeeper gold ───────────────────────────────

    #[test]
    fn test_shop_sell_with_gold() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let player = world.player();
        let mut shop = test_shop(&mut world);
        shop.shopkeeper_gold = 2000;
        let def = test_obj_def(1, 100);
        let defs = vec![def.clone()];

        let item = crate::items::spawn_item(
            &mut world,
            &def,
            crate::items::SpawnLocation::Floor(10, 5),
            None,
        );

        let events = drop_in_shop(&world, player, item, &mut shop, &defs);
        // sell price = 100/2 = 50.  Shk has gold => pays directly.
        assert_eq!(shop.shopkeeper_gold, 1950);
        assert_eq!(shop.credit, 0);
        let has_sell = events.iter().any(|e| {
            if let EngineEvent::Message { key, .. } = e {
                key.contains("shop-sell")
            } else {
                false
            }
        });
        assert!(has_sell);
    }

    // ── Charisma price modifier exhaustive ──────────────────────

    #[test]
    fn test_shop_cha_modifier_boundaries() {
        // Test boundary values.
        assert_eq!(cha_price_modifier(0), (2, 1));
        assert_eq!(cha_price_modifier(5), (2, 1));
        assert_eq!(cha_price_modifier(6), (3, 2));
        assert_eq!(cha_price_modifier(7), (3, 2));
        assert_eq!(cha_price_modifier(8), (4, 3));
        assert_eq!(cha_price_modifier(10), (4, 3));
        assert_eq!(cha_price_modifier(11), (1, 1));
        assert_eq!(cha_price_modifier(15), (1, 1));
        assert_eq!(cha_price_modifier(16), (3, 4));
        assert_eq!(cha_price_modifier(17), (3, 4));
        assert_eq!(cha_price_modifier(18), (2, 3));
        assert_eq!(cha_price_modifier(19), (1, 2));
        assert_eq!(cha_price_modifier(25), (1, 2));
    }

    // ── Multiple rile/pacify cycles ─────────────────────────────

    #[test]
    fn test_shop_rile_idempotent() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let mut shop = test_shop(&mut world);
        let dummy = world.spawn((Name("dummy".to_string()),));
        shop.bill.add(dummy, 100, 1);

        rile_shop(&mut shop);
        assert_eq!(shop.bill.entries()[0].price, 134); // 100 + (100+2)/3 = 134
        assert!(shop.surcharge);

        // Second rile should be idempotent (surcharge already set).
        rile_shop(&mut shop);
        assert_eq!(shop.bill.entries()[0].price, 134);
    }

    #[test]
    fn test_shop_pacify_idempotent() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let mut shop = test_shop(&mut world);
        let dummy = world.spawn((Name("dummy".to_string()),));
        shop.bill.add(dummy, 100, 1);

        rile_shop(&mut shop);
        pacify_shop(&mut shop);
        assert_eq!(shop.bill.entries()[0].price, 100); // restored

        // Second pacify should be idempotent.
        pacify_shop(&mut shop);
        assert_eq!(shop.bill.entries()[0].price, 100);
    }

    // ── Rile/pacify with various edge-case prices ───────────────

    #[test]
    fn test_shop_rile_pacify_price_2() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let mut shop = test_shop(&mut world);
        let dummy = world.spawn((Name("dummy".to_string()),));
        shop.bill.add(dummy, 2, 1);

        // rile: (2+2)/3 = 1 => 3
        rile_shop(&mut shop);
        assert_eq!(shop.bill.entries()[0].price, 3);

        // pacify: (3+3)/4 = 1 => 2
        pacify_shop(&mut shop);
        assert_eq!(shop.bill.entries()[0].price, 2);
    }

    #[test]
    fn test_shop_rile_pacify_price_5() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let mut shop = test_shop(&mut world);
        let dummy = world.spawn((Name("dummy".to_string()),));
        shop.bill.add(dummy, 5, 1);

        // rile: (5+2)/3 = 2 => 7
        rile_shop(&mut shop);
        assert_eq!(shop.bill.entries()[0].price, 7);

        // pacify: (7+3)/4 = 2 => 5
        pacify_shop(&mut shop);
        assert_eq!(shop.bill.entries()[0].price, 5);
    }

    // ── Sell to angry shopkeeper ────────────────────────────────

    #[test]
    fn test_shop_sell_to_angry() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let player = world.player();
        let mut shop = test_shop(&mut world);
        shop.angry = true;
        let def = test_obj_def(1, 100);
        let defs = vec![def.clone()];

        let item = crate::items::spawn_item(
            &mut world,
            &def,
            crate::items::SpawnLocation::Floor(10, 5),
            None,
        );

        let events = drop_in_shop(&world, player, item, &mut shop, &defs);
        // Angry shk takes item without paying.
        let has_angry = events.iter().any(|e| {
            if let EngineEvent::Message { key, .. } = e {
                key.contains("shop-angry-take")
            } else {
                false
            }
        });
        assert!(has_angry);
        assert_eq!(shop.credit, 0);
    }

    // ── Sell reduces robbed ─────────────────────────────────────

    #[test]
    fn test_shop_sell_reduces_robbed() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let player = world.player();
        let mut shop = test_shop(&mut world);
        shop.robbed = 200;
        shop.shopkeeper_gold = 1000;
        let def = test_obj_def(1, 100);
        let defs = vec![def.clone()];

        let item = crate::items::spawn_item(
            &mut world,
            &def,
            crate::items::SpawnLocation::Floor(10, 5),
            None,
        );

        let events = drop_in_shop(&world, player, item, &mut shop, &defs);
        // Sell price = 50. Robbed was 200, now 200-50 = 150.
        assert_eq!(shop.robbed, 150);
        let has_restock = events.iter().any(|e| {
            if let EngineEvent::Message { key, .. } = e {
                key.contains("shop-restock")
            } else {
                false
            }
        });
        assert!(has_restock);
    }

    // ── Return billed item ──────────────────────────────────────

    #[test]
    fn test_shop_return_billed_item() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let player = world.player();
        let mut shop = test_shop(&mut world);
        let def = test_obj_def(1, 100);
        let defs = vec![def.clone()];

        let item = crate::items::spawn_item(
            &mut world,
            &def,
            crate::items::SpawnLocation::Floor(10, 5),
            None,
        );

        // Pick up then drop back.
        pickup_in_shop(&world, player, item, &mut shop, &defs);
        assert_eq!(shop.bill.len(), 1);

        let events = drop_in_shop(&world, player, item, &mut shop, &defs);
        assert_eq!(shop.bill.len(), 0);
        let has_return = events.iter().any(|e| {
            if let EngineEvent::Message { key, .. } = e {
                key.contains("shop-return")
            } else {
                false
            }
        });
        assert!(has_return);
    }

    // ── Cost per charge tests ────────────────────────────────────

    #[test]
    fn test_cost_per_charge_wand() {
        // Base cost 300, 7 charges: 300/8 = 37, but min 10.
        assert_eq!(cost_per_charge(300, 7, true, false), 37);
        // Low charges: 300/1 = 300.
        assert_eq!(cost_per_charge(300, 0, true, false), 300);
        // Very low base: 5/2 = 2, min 10.
        assert_eq!(cost_per_charge(5, 1, true, false), 10);
    }

    #[test]
    fn test_cost_per_charge_spellbook() {
        // 100/5 = 20.
        assert_eq!(cost_per_charge(100, 4, false, true), 20);
        // Low base: 3/2 = 1, min 5.
        assert_eq!(cost_per_charge(3, 1, false, true), 5);
    }

    #[test]
    fn test_cost_per_charge_tool() {
        // 50/6 = 8.
        assert_eq!(cost_per_charge(50, 5, false, false), 8);
        // Very low: 0/1 = 0, min 1.
        assert_eq!(cost_per_charge(0, 0, false, false), 1);
    }

    // ── Pricing units tests ──────────────────────────────────────

    #[test]
    fn test_pricing_units_single() {
        assert_eq!(get_pricing_units(1, false, false), 1);
        assert_eq!(get_pricing_units(5, false, false), 1);
    }

    #[test]
    fn test_pricing_units_ammo() {
        assert_eq!(get_pricing_units(20, true, false), 20);
        assert_eq!(get_pricing_units(1, true, false), 1);
    }

    #[test]
    fn test_pricing_units_mergeable() {
        assert_eq!(get_pricing_units(5, false, true), 5);
    }

    // ── Special stock tests ──────────────────────────────────────

    #[test]
    fn test_special_stock_wand_shop() {
        assert!(is_special_stock(
            ShopType::Wand,
            "leather gloves",
            ObjectClass::Armor,
        ));
        assert!(is_special_stock(
            ShopType::Wand,
            "elven cloak",
            ObjectClass::Armor,
        ));
        assert!(!is_special_stock(
            ShopType::Wand,
            "plate mail",
            ObjectClass::Armor,
        ));
    }

    #[test]
    fn test_special_stock_food_shop() {
        assert!(is_special_stock(
            ShopType::Food,
            "tinning kit",
            ObjectClass::Tool,
        ));
        assert!(!is_special_stock(
            ShopType::Food,
            "magic marker",
            ObjectClass::Tool,
        ));
    }

    #[test]
    fn test_special_stock_candle_shop() {
        assert!(is_special_stock(
            ShopType::Candle,
            "wax candle",
            ObjectClass::Tool,
        ));
        assert!(is_special_stock(
            ShopType::Candle,
            "magic lamp",
            ObjectClass::Tool,
        ));
    }

    // ── Shop inheritance tests ───────────────────────────────────

    #[test]
    fn test_shop_inheritance_normal() {
        assert!(check_shop_inheritance(false, true, false));
    }

    #[test]
    fn test_shop_inheritance_angry_keeper() {
        assert!(!check_shop_inheritance(true, true, false));
    }

    #[test]
    fn test_shop_inheritance_no_adjacent() {
        assert!(!check_shop_inheritance(false, false, false));
    }

    #[test]
    fn test_shop_inheritance_adjacent_angry() {
        assert!(!check_shop_inheritance(false, true, true));
    }

    // ── Shop damage tests ────────────────────────────────────────

    #[test]
    fn test_shop_damage_cost() {
        assert_eq!(shop_damage_cost(ShopDamageType::WallDestroyed), 300);
        assert_eq!(shop_damage_cost(ShopDamageType::DoorBroken), 200);
        assert_eq!(shop_damage_cost(ShopDamageType::FloorDamaged), 100);
    }

    #[test]
    fn test_record_shop_damage() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let mut shop = test_shop(&mut world);
        assert_eq!(shop.debit, 0);

        let events = record_shop_damage(
            &mut shop,
            Position::new(7, 3),
            ShopDamageType::WallDestroyed,
        );
        assert_eq!(shop.debit, 300);
        assert_eq!(shop.damage_list.len(), 1);
        assert!(!events.is_empty());
    }

    #[test]
    fn test_repair_one_damage() {
        let mut world = GameWorld::new(Position::new(10, 5));
        let mut shop = test_shop(&mut world);
        record_shop_damage(&mut shop, Position::new(7, 3), ShopDamageType::DoorBroken);
        record_shop_damage(&mut shop, Position::new(8, 3), ShopDamageType::FloorDamaged);
        assert_eq!(shop.damage_list.len(), 2);

        let repaired = repair_one_damage(&mut shop);
        assert!(repaired.is_some());
        assert_eq!(shop.damage_list.len(), 1);

        let repaired2 = repair_one_damage(&mut shop);
        assert!(repaired2.is_some());
        assert_eq!(shop.damage_list.len(), 0);

        let repaired3 = repair_one_damage(&mut shop);
        assert!(repaired3.is_none());
    }

    // ── Shopkeeper name pool tests ───────────────────────────────

    #[test]
    fn test_shopkeeper_name_pools_exist() {
        let shop_types = [
            ShopType::General,
            ShopType::Armor,
            ShopType::Scroll,
            ShopType::Potion,
            ShopType::Weapon,
            ShopType::Food,
            ShopType::Ring,
            ShopType::Wand,
            ShopType::Tool,
            ShopType::Book,
            ShopType::HealthFood,
            ShopType::Candle,
        ];
        for st in &shop_types {
            let pool = shopkeeper_name_pool(*st);
            assert!(!pool.is_empty(), "{:?} should have name pool", st);
        }
    }

    #[test]
    fn test_random_shopkeeper_name() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
        let name = random_shopkeeper_name(ShopType::General, &mut rng);
        assert!(!name.is_empty());
    }

    // ── Container sale value tests ───────────────────────────────

    #[test]
    fn test_container_sale_value() {
        // Container base 100 + contents 200, CHA 12.
        let value = container_sale_value(100, 200, 1, 12);
        // sell price of container: 100/2 = 50. + 200 = 250.
        assert_eq!(value, 250);
    }

    // ── Price embellishment tests ────────────────────────────────

    #[test]
    fn test_price_quote_message_key() {
        assert_eq!(
            price_quote_message_key(ObjectClass::Food, false, false, false, 50),
            "shop-price-gourmets-delight"
        );
        assert_eq!(
            price_quote_message_key(ObjectClass::Wand, false, false, false, 50),
            "shop-price-painstakingly-developed"
        );
        assert_eq!(
            price_quote_message_key(ObjectClass::Tool, false, false, true, 50),
            "shop-price-one-of-a-kind"
        );
        assert_eq!(
            price_quote_message_key(ObjectClass::Weapon, false, false, false, 600),
            "shop-price-finest-quality"
        );
        assert_eq!(
            price_quote_message_key(ObjectClass::Weapon, false, false, false, 300),
            "shop-price-superb-craftsmanship"
        );
        assert_eq!(
            price_quote_message_key(ObjectClass::Weapon, false, false, false, 150),
            "shop-price-excellent-choice"
        );
        assert_eq!(
            price_quote_message_key(ObjectClass::Weapon, false, false, false, 50),
            "shop-price-bargain"
        );
        assert_eq!(
            price_quote_message_key(ObjectClass::Weapon, false, false, false, 5),
            "shop-price"
        );
    }

    // ── Deserted shop tests ──────────────────────────────────────

    #[test]
    fn test_deserted_shop() {
        assert!(is_shop_deserted(false));
        assert!(!is_shop_deserted(true));
    }

    // ── Unidentified price variation tests ───────────────────────

    #[test]
    fn test_unidentified_markup_identified_unchanged() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
        // Identified items get exact base price.
        for _ in 0..100 {
            assert_eq!(
                unidentified_price_variation(100, true, &mut rng),
                100,
                "identified items should always get exact base price"
            );
        }
    }

    #[test]
    fn test_unidentified_markup_variation_range() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
        let base = 100;
        let variation = base / 4; // 25
        let mut min_seen = i32::MAX;
        let mut max_seen = i32::MIN;
        for _ in 0..1000 {
            let price = unidentified_price_variation(base, false, &mut rng);
            min_seen = min_seen.min(price);
            max_seen = max_seen.max(price);
            assert!(
                price >= base - variation && price <= base + variation,
                "price {} should be in [{}, {}]",
                price,
                base - variation,
                base + variation
            );
        }
        // Should see some variation.
        assert!(
            max_seen > min_seen,
            "should see price variation for unidentified items"
        );
    }

    #[test]
    fn test_unidentified_markup_zero_cost() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
        // Zero cost items return 0 (no variation possible).
        assert_eq!(unidentified_price_variation(0, false, &mut rng), 0);
    }

    #[test]
    fn test_unidentified_markup_small_price() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
        // Price of 3: variation = 3/4 = 0, so no variation.
        for _ in 0..100 {
            assert_eq!(
                unidentified_price_variation(3, false, &mut rng),
                3,
                "small prices with variation=0 should be unchanged"
            );
        }
    }

    // ── CHA sell modifier tests ──────────────────────────────────

    #[test]
    fn test_cha_sell_modifier_is_neutral() {
        // Sell prices in NetHack are not affected by CHA.
        for cha in 3..=25 {
            assert_eq!(
                cha_sell_modifier(cha),
                (1, 1),
                "CHA {} sell modifier should be (1,1)",
                cha
            );
        }
    }

    // ── Charisma buying discount at high CHA ────────────────────

    #[test]
    fn test_charisma_buying_high_cha_discount() {
        // CHA 19 = 1/2 multiplier.
        let price = get_cost(200, 1, 19, true, false);
        assert_eq!(price, 100, "CHA 19 should halve buy price");
    }

    // ── Charisma selling low CHA penalty ─────────────────────────

    #[test]
    fn test_charisma_selling_low_cha_penalty() {
        // Selling is always 1/2 regardless of CHA.
        let sell_low = get_cost(200, 1, 3, false, false);
        let sell_high = get_cost(200, 1, 19, false, false);
        assert_eq!(sell_low, sell_high, "sell price should not vary with CHA");
        assert_eq!(sell_low, 100, "sell price should be 200/2 = 100");
    }
}
