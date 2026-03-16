//! Inventory management: entity-level inventory tracking, pickup, drop,
//! view, autopickup, weight calculation, packorder sorting, item splitting,
//! container interaction, and inventory counting.
//!
//! The `Inventory` component holds a Vec of item entities owned by a
//! carrier (typically the player).  It works in tandem with the per-item
//! `ObjectLocation` component — `Inventory` is the carrier's view,
//! `ObjectLocation::Inventory` is the item's view.
//!
//! Autopickup runs after every player move: gold is always collected,
//! and other classes are collected when they match the player's
//! configured autopickup types.

use hecs::Entity;
use rand::Rng;
use serde::{Deserialize, Serialize};

use nethack_babel_data::{BucStatus, ObjectClass, ObjectCore, ObjectDef, ObjectLocation};

use crate::action::Position;
use crate::event::EngineEvent;
use crate::items::{self, LetterState};
use crate::world::{Encumbrance, GameWorld, Positioned};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of inventory letter slots (a-z, A-Z).
const MAX_INV_SLOTS: usize = 52;

/// Default packorder: coin, amulet, weapon, armor, food, scroll, spellbook,
/// potion, ring, wand, tool, gem, rock, ball, chain.
pub const DEFAULT_PACKORDER: &[ObjectClass] = &[
    ObjectClass::Coin,
    ObjectClass::Amulet,
    ObjectClass::Weapon,
    ObjectClass::Armor,
    ObjectClass::Food,
    ObjectClass::Scroll,
    ObjectClass::Spellbook,
    ObjectClass::Potion,
    ObjectClass::Ring,
    ObjectClass::Wand,
    ObjectClass::Tool,
    ObjectClass::Gem,
    ObjectClass::Rock,
    ObjectClass::Ball,
    ObjectClass::Chain,
];

// ---------------------------------------------------------------------------
// Inventory component
// ---------------------------------------------------------------------------

/// An entity-level inventory: the ordered list of item entities carried
/// by a creature (player or monster).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    /// Item entities, in insertion order. Up to 52 entries (a-zA-Z).
    pub items: Vec<Entity>,
}

impl Inventory {
    /// Create an empty inventory.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Add an item entity to the inventory.
    /// Returns the assigned inventory letter, or `None` if full.
    pub fn add(&mut self, item: Entity) -> Option<char> {
        if self.items.len() >= MAX_INV_SLOTS {
            return None;
        }
        self.items.push(item);
        Some(self.letter_for(self.items.len() - 1))
    }

    /// Remove an item entity from the inventory.
    /// Returns `true` if the item was found and removed.
    pub fn remove(&mut self, item: Entity) -> bool {
        if let Some(pos) = self.items.iter().position(|&e| e == item) {
            self.items.remove(pos);
            true
        } else {
            false
        }
    }

    /// Map a slot index (0..51) to the corresponding inventory letter.
    pub fn letter_for(&self, index: usize) -> char {
        if index < 26 {
            (b'a' + index as u8) as char
        } else if index < 52 {
            (b'A' + (index - 26) as u8) as char
        } else {
            '?'
        }
    }

    /// Find the item entity assigned to the given inventory letter.
    pub fn find_by_letter(&self, letter: char) -> Option<Entity> {
        let idx = if letter.is_ascii_lowercase() {
            (letter as u8 - b'a') as usize
        } else if letter.is_ascii_uppercase() {
            (letter as u8 - b'A') as usize + 26
        } else {
            return None;
        };
        self.items.get(idx).copied()
    }

    /// Whether the inventory is completely full (52 items).
    pub fn is_full(&self) -> bool {
        self.items.len() >= MAX_INV_SLOTS
    }

    /// Number of items in the inventory.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the inventory is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Whether the inventory contains the given item entity.
    pub fn contains(&self, item: Entity) -> bool {
        self.items.contains(&item)
    }

    /// Swap the letters of two items in the inventory by index.
    /// Returns true if both indices were valid and the swap was performed.
    pub fn swap(&mut self, idx_a: usize, idx_b: usize) -> bool {
        if idx_a < self.items.len() && idx_b < self.items.len() && idx_a != idx_b {
            self.items.swap(idx_a, idx_b);
            true
        } else {
            false
        }
    }

    /// Sort inventory items by the given packorder class ordering.
    /// Items whose class appears earlier in `packorder` come first.
    /// Within the same class, original order is preserved (stable sort).
    pub fn sort_by_packorder(&mut self, world: &GameWorld, packorder: &[ObjectClass]) {
        self.items.sort_by(|&a, &b| {
            let class_a = world.get_component::<ObjectCore>(a).map(|c| c.object_class);
            let class_b = world.get_component::<ObjectCore>(b).map(|c| c.object_class);
            let rank_a = class_a
                .and_then(|c| packorder.iter().position(|p| *p == c))
                .unwrap_or(usize::MAX);
            let rank_b = class_b
                .and_then(|c| packorder.iter().position(|p| *p == c))
                .unwrap_or(usize::MAX);
            rank_a.cmp(&rank_b)
        });
    }
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Gold weight formula
// ---------------------------------------------------------------------------

/// NetHack gold weight formula: `(n + 50) / 100`.
/// Every 100 gold coins weighs 1 unit. The +50 rounds nicely.
pub fn gold_weight(quantity: u32) -> u32 {
    (quantity + 50) / 100
}

/// Minimum gold weight for `weight()` function: at least 1 unit.
/// `inv_weight()` uses `gold_weight()` directly (no minimum).
pub fn gold_weight_min1(quantity: u32) -> u32 {
    gold_weight(quantity).max(1)
}

/// Weight delta when merging gold: merged pile may weigh less than sum.
/// Returns the net weight change from merging `add_qty` into `existing_qty`.
pub fn gold_weight_delta(existing_qty: u32, add_qty: u32) -> i32 {
    let before = gold_weight(existing_qty);
    let after = gold_weight(existing_qty + add_qty);
    after as i32 - before as i32
}

// ---------------------------------------------------------------------------
// Container weight calculations
// ---------------------------------------------------------------------------

/// Bag of Holding weight adjustment for contents.
///
/// Adjusts the raw content weight based on BUC status:
/// - Cursed: `cwt * 2`
/// - Blessed: `(cwt + 3) / 4`
/// - Uncursed: `(cwt + 1) / 2`
pub fn boh_content_weight(content_weight: u32, buc: &BucStatus) -> u32 {
    if buc.cursed {
        content_weight.saturating_mul(2)
    } else if buc.blessed {
        (content_weight + 3) / 4
    } else {
        (content_weight + 1) / 2
    }
}

/// Calculate the actual weight change when removing an item from a
/// Bag of Holding. Due to the BoH weight reduction formula, removing
/// a 100-weight item from a blessed BoH reduces the container's total
/// weight by less than 100.
///
/// For non-BoH containers, returns the item's weight directly.
pub fn delta_cwt_boh(
    content_weight_before: u32,
    item_weight: u32,
    is_boh: bool,
    buc: &BucStatus,
) -> u32 {
    if !is_boh {
        return item_weight;
    }
    let adjusted_before = boh_content_weight(content_weight_before, buc);
    let content_after = content_weight_before.saturating_sub(item_weight);
    let adjusted_after = boh_content_weight(content_after, buc);
    adjusted_before.saturating_sub(adjusted_after)
}

// ---------------------------------------------------------------------------
// Inventory counting
// ---------------------------------------------------------------------------

/// Count inventory items, optionally including gold.
///
/// `inv_cnt(false)` counts non-gold items (for 52-slot limit checks).
/// `inv_cnt(true)` counts everything including gold.
pub fn inv_cnt(world: &GameWorld, player: Entity, include_gold: bool) -> usize {
    let inv = match world.get_component::<Inventory>(player) {
        Some(inv) => inv,
        None => return 0,
    };
    if include_gold {
        inv.len()
    } else {
        inv.items
            .iter()
            .filter(|&&item| {
                world
                    .get_component::<ObjectCore>(item)
                    .map(|c| c.object_class != ObjectClass::Coin)
                    .unwrap_or(true)
            })
            .count()
    }
}

// ---------------------------------------------------------------------------
// Item splitting
// ---------------------------------------------------------------------------

/// Result of splitting a stack of items.
pub struct SplitResult {
    /// The original entity (retains `quantity - num` items).
    pub parent: Entity,
    /// The new entity (holds `num` items).
    pub child: Entity,
}

/// Split a stack: the original entity keeps `quantity - num` items,
/// and a new entity is created with `num` items.
///
/// Panics if `num == 0`, `num >= obj.quantity`, or the object is a container.
pub fn split_stack(
    world: &mut GameWorld,
    item: Entity,
    num: u16,
    obj_defs: &[ObjectDef],
) -> SplitResult {
    let (otyp, old_qty, object_class, inv_letter, artifact, age) = {
        let core = world
            .get_component::<ObjectCore>(item)
            .expect("split_stack: item must have ObjectCore");
        assert!(num > 0, "split_stack: num must be > 0");
        assert!(
            (num as i32) < core.quantity,
            "split_stack: num must be < quantity"
        );
        (
            core.otyp,
            core.quantity,
            core.object_class,
            core.inv_letter,
            core.artifact,
            core.age,
        )
    };

    let unit_weight = obj_defs
        .iter()
        .find(|d| d.id == otyp)
        .map(|d| d.weight as u32)
        .unwrap_or(0);

    // Update parent: reduce quantity and weight.
    let remain = old_qty - num as i32;
    {
        let mut core = world.get_component_mut::<ObjectCore>(item).unwrap();
        core.quantity = remain;
        core.weight = unit_weight * remain as u32;
    }

    // Create child with the split-off portion.
    let child_core = ObjectCore {
        otyp,
        object_class,
        quantity: num as i32,
        weight: unit_weight * num as u32,
        age,
        inv_letter, // inherits parent letter initially
        artifact,
    };

    // Clone BUC from parent.
    let buc = world
        .get_component::<BucStatus>(item)
        .map(|b| BucStatus {
            cursed: b.cursed,
            blessed: b.blessed,
            bknown: b.bknown,
        })
        .unwrap_or(BucStatus {
            cursed: false,
            blessed: false,
            bknown: false,
        });

    let child = world.spawn((child_core, buc, ObjectLocation::Inventory));

    // Copy enchantment if present.
    let ench_copy = world
        .get_component::<nethack_babel_data::Enchantment>(item)
        .map(|e| nethack_babel_data::Enchantment { spe: e.spe });
    if let Some(ench) = ench_copy {
        let _ = world.ecs_mut().insert_one(child, ench);
    }

    // Copy erosion if present.
    let erosion_copy = world
        .get_component::<nethack_babel_data::Erosion>(item)
        .map(|e| nethack_babel_data::Erosion {
            eroded: e.eroded,
            eroded2: e.eroded2,
            erodeproof: e.erodeproof,
            greased: e.greased,
        });
    if let Some(erosion) = erosion_copy {
        let _ = world.ecs_mut().insert_one(child, erosion);
    }

    SplitResult {
        parent: item,
        child,
    }
}

// ---------------------------------------------------------------------------
// Encumbrance messages
// ---------------------------------------------------------------------------

/// Generate the appropriate encumbrance warning message for a given level.
pub fn encumbrance_message(level: Encumbrance) -> Option<&'static str> {
    match level {
        Encumbrance::Unencumbered => None,
        Encumbrance::Burdened => Some("encumbrance-burdened"),
        Encumbrance::Stressed => Some("encumbrance-stressed"),
        Encumbrance::Strained => Some("encumbrance-strained"),
        Encumbrance::Overtaxed => Some("encumbrance-overtaxed"),
        Encumbrance::Overloaded => Some("encumbrance-overloaded"),
    }
}

/// Check if picking up an item would worsen encumbrance beyond acceptable.
///
/// Returns the new encumbrance level if it would exceed the threshold,
/// or None if the pickup is acceptable.
pub fn pickup_encumbrance_check(
    current_enc: Encumbrance,
    new_enc: Encumbrance,
    pickup_burden: Encumbrance,
) -> Option<Encumbrance> {
    let threshold = if current_enc > pickup_burden {
        current_enc
    } else {
        pickup_burden
    };
    if new_enc > threshold {
        Some(new_enc)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Autopickup configuration
// ---------------------------------------------------------------------------

/// Configuration for the autopickup system.
#[derive(Debug, Clone)]
pub struct AutopickupConfig {
    /// Global autopickup toggle.
    pub enabled: bool,
    /// Classes to auto-pick (empty = pick all non-special).
    pub pickup_types: Vec<ObjectClass>,
    /// Auto-pick thrown items.
    pub pickup_thrown: bool,
    /// Auto-pick stolen items.
    pub pickup_stolen: bool,
    /// Don't auto-pick dropped items.
    pub nopick_dropped: bool,
    /// Maximum encumbrance level before prompting.
    pub pickup_burden: Encumbrance,
}

impl Default for AutopickupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            pickup_types: Vec::new(),
            pickup_thrown: true,
            pickup_stolen: false,
            nopick_dropped: false,
            pickup_burden: Encumbrance::Stressed,
        }
    }
}

/// How an item was lost (for autopickup priority overrides).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HowLost {
    None,
    Thrown,
    Stolen,
    Dropped,
    Exploding,
}

/// Check whether an item should be auto-picked up based on configuration.
pub fn should_autopickup(
    object_class: ObjectClass,
    how_lost: HowLost,
    config: &AutopickupConfig,
) -> bool {
    if !config.enabled {
        return false;
    }

    // Priority overrides from how_lost.
    if config.pickup_thrown && how_lost == HowLost::Thrown {
        return true;
    }
    if config.pickup_stolen && how_lost == HowLost::Stolen {
        return true;
    }
    if config.nopick_dropped && how_lost == HowLost::Dropped {
        return false;
    }
    if how_lost == HowLost::Exploding {
        return false;
    }

    // Gold is always auto-picked.
    if object_class == ObjectClass::Coin {
        return true;
    }

    // Type filtering.
    if config.pickup_types.is_empty() {
        true
    } else {
        config.pickup_types.contains(&object_class)
    }
}

// ---------------------------------------------------------------------------
// Inventory letter sort value
// ---------------------------------------------------------------------------

/// Sort key for inventory letters matching NetHack's `invletter_value()`.
/// `$`=1, `a`-`z`=2..27, `A`-`Z`=28..53, `#`=54, other=55.
pub fn invletter_value(ch: char) -> u8 {
    match ch {
        '$' => 1,
        'a'..='z' => (ch as u8) - b'a' + 2,
        'A'..='Z' => (ch as u8) - b'A' + 28,
        '#' => 54,
        _ => 55,
    }
}

/// Inventory rank for sorting: `invlet XOR 0x20`.
/// This sorts as: `$` < `a`-`z` < `A`-`Z`.
pub fn inv_rank(ch: char) -> u8 {
    (ch as u8) ^ 0x20
}

// ---------------------------------------------------------------------------
// Container interaction
// ---------------------------------------------------------------------------

/// Items that cannot be placed into any container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerRestriction {
    /// Ball and chain.
    BallOrChain,
    /// Trying to put a container into itself.
    SelfReference,
    /// Worn armor or accessory.
    WornEquipment,
    /// Cursed loadstone.
    CursedLoadstone,
    /// Quest artifact (Amulet of Yendor, etc.).
    QuestArtifact,
    /// Item is too large (boulder, large statue).
    TooLarge,
}

/// Check if an item can be placed into a container.
/// Returns `None` if allowed, or the restriction reason.
pub fn check_container_restriction(
    object_class: ObjectClass,
    is_worn: bool,
    is_cursed_loadstone: bool,
    is_quest_artifact: bool,
    is_boulder: bool,
) -> Option<ContainerRestriction> {
    if object_class == ObjectClass::Ball || object_class == ObjectClass::Chain {
        return Some(ContainerRestriction::BallOrChain);
    }
    if is_worn {
        return Some(ContainerRestriction::WornEquipment);
    }
    if is_cursed_loadstone {
        return Some(ContainerRestriction::CursedLoadstone);
    }
    if is_quest_artifact {
        return Some(ContainerRestriction::QuestArtifact);
    }
    if is_boulder {
        return Some(ContainerRestriction::TooLarge);
    }
    None
}

/// Bag of Holding explosion check.
///
/// When placing a dangerous item (BoH, wand of cancellation, bag of tricks
/// with charges) into a BoH, there's a depth-dependent explosion probability.
///
/// At depth 0 (direct placement): always explodes.
/// At depth 1: always explodes.
/// At depth 2: 75% chance.
/// Deeper: decreasing probability.
pub fn boh_explodes<R: Rng>(rng: &mut R, depth: u32) -> bool {
    let capped = depth.min(7);
    let range = 1u32 << capped;
    rng.random_range(0..range) <= depth
}

/// Damage from a Bag of Holding explosion: 6d6.
pub fn boh_explosion_damage<R: Rng>(rng: &mut R) -> u32 {
    (0..6).map(|_| rng.random_range(1..=6u32)).sum()
}

/// Chance that an individual item survives a BoH explosion: 12/13.
/// Returns true if the item is destroyed (1/13 chance).
pub fn boh_item_gone<R: Rng>(rng: &mut R) -> bool {
    rng.random_range(0..13u32) == 0
}

/// Cursed Bag of Holding item loss: 1/13 chance per item when opened.
pub fn boh_cursed_loss<R: Rng>(rng: &mut R) -> bool {
    rng.random_range(0..13u32) == 0
}

// ---------------------------------------------------------------------------
// Pickup from floor
// ---------------------------------------------------------------------------

/// Find all item entities on the floor at the given position.
pub fn items_at_position(world: &GameWorld, pos: Position) -> Vec<Entity> {
    let mut result = Vec::new();
    for (entity, (core, loc)) in world.ecs().query::<(&ObjectCore, &ObjectLocation)>().iter() {
        let _ = core; // we just need the entity
        if let ObjectLocation::Floor { x, y } = *loc
            && x as i32 == pos.x
            && y as i32 == pos.y
        {
            result.push(entity);
        }
    }
    result
}

/// Pick up an item from the floor and add it to the player's inventory.
///
/// Delegates to `items::pickup_item` for the heavy lifting (merge
/// detection, encumbrance check, letter assignment), then synchronizes
/// the `Inventory` component.
pub fn pickup_item(
    world: &mut GameWorld,
    player: Entity,
    item_entity: Entity,
    letter_state: &mut LetterState,
    obj_defs: &[ObjectDef],
) -> Vec<EngineEvent> {
    let events = items::pickup_item(world, player, item_entity, letter_state, obj_defs);

    // If the pickup succeeded (ItemPickedUp event present), synchronize
    // the Inventory component.
    for event in &events {
        if let EngineEvent::ItemPickedUp { item, .. } = event
            && let Some(mut inv) = world.get_component_mut::<Inventory>(player)
        {
            // For a merge, the item entity is the merge target which
            // may already be in the inventory.
            if !inv.contains(*item) {
                inv.add(*item);
            }
        }
    }

    events
}

/// Pick up all items at the player's current position.
pub fn pickup_all_at_player(
    world: &mut GameWorld,
    letter_state: &mut LetterState,
    obj_defs: &[ObjectDef],
) -> Vec<EngineEvent> {
    let player = world.player();
    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return vec![],
    };

    let floor_items = items_at_position(world, player_pos);
    if floor_items.is_empty() {
        return vec![EngineEvent::msg("nothing-here")];
    }

    let mut all_events = Vec::new();
    for item in floor_items {
        let events = pickup_item(world, player, item, letter_state, obj_defs);
        all_events.extend(events);
    }
    all_events
}

// ---------------------------------------------------------------------------
// Drop
// ---------------------------------------------------------------------------

/// Drop an item from the player's inventory onto the floor.
///
/// Delegates to `items::drop_item` and synchronizes the `Inventory`
/// component.
pub fn drop_item(world: &mut GameWorld, player: Entity, item_entity: Entity) -> Vec<EngineEvent> {
    // Remove from Inventory component first.
    if let Some(mut inv) = world.get_component_mut::<Inventory>(player) {
        inv.remove(item_entity);
    }

    items::drop_item(world, player, item_entity)
}

/// Reasons an item cannot be dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CannotDrop {
    /// Cursed wielded weapon (welded).
    Welded,
    /// Cursed loadstone.
    CursedLoadstone,
    /// Ball and chain.
    BallOrChain,
    /// Worn armor must be removed first.
    WornEquipment,
}

/// Check whether an item can be dropped.
/// Returns `None` if droppable, or the reason it cannot be dropped.
pub fn check_drop_restriction(
    object_class: ObjectClass,
    is_wielded_cursed: bool,
    is_cursed_loadstone: bool,
    is_worn: bool,
) -> Option<CannotDrop> {
    if is_wielded_cursed {
        return Some(CannotDrop::Welded);
    }
    if is_cursed_loadstone {
        return Some(CannotDrop::CursedLoadstone);
    }
    if object_class == ObjectClass::Ball || object_class == ObjectClass::Chain {
        return Some(CannotDrop::BallOrChain);
    }
    if is_worn {
        return Some(CannotDrop::WornEquipment);
    }
    None
}

// ---------------------------------------------------------------------------
// View inventory
// ---------------------------------------------------------------------------

/// Produce message events listing the player's inventory contents.
///
/// Each item gets a message with its letter and name.  If the inventory
/// is empty, a single "Not carrying anything." message is returned.
pub fn view_inventory(world: &GameWorld, player: Entity) -> Vec<EngineEvent> {
    let inv_items = items::get_inventory(world, player);
    if inv_items.is_empty() {
        return vec![EngineEvent::msg("not-carrying-anything")];
    }

    let mut events = Vec::with_capacity(inv_items.len());
    for (_entity, core) in &inv_items {
        let letter = core
            .inv_letter
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".to_string());
        events.push(EngineEvent::msg_with(
            "inventory-item",
            vec![
                ("letter", letter),
                ("name", format!("item(otyp={})", core.otyp.0)),
                ("quantity", core.quantity.to_string()),
            ],
        ));
    }
    events
}

/// View inventory grouped by packorder classes.
pub fn view_inventory_sorted(
    world: &GameWorld,
    player: Entity,
    packorder: &[ObjectClass],
) -> Vec<EngineEvent> {
    let inv_items = items::get_inventory(world, player);
    if inv_items.is_empty() {
        return vec![EngineEvent::msg("not-carrying-anything")];
    }

    // Sort items by packorder.
    let mut sorted = inv_items;
    sorted.sort_by(|(_, a), (_, b)| {
        let rank_a = packorder
            .iter()
            .position(|p| *p == a.object_class)
            .unwrap_or(usize::MAX);
        let rank_b = packorder
            .iter()
            .position(|p| *p == b.object_class)
            .unwrap_or(usize::MAX);
        rank_a.cmp(&rank_b)
    });

    let mut events = Vec::with_capacity(sorted.len());
    let mut last_class: Option<ObjectClass> = None;

    for (_entity, core) in &sorted {
        // Emit class header when class changes.
        if last_class != Some(core.object_class) {
            last_class = Some(core.object_class);
            events.push(EngineEvent::msg_with(
                "inventory-class-header",
                vec![("class", format!("{:?}", core.object_class))],
            ));
        }

        let letter = core
            .inv_letter
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".to_string());
        events.push(EngineEvent::msg_with(
            "inventory-item",
            vec![
                ("letter", letter),
                ("name", format!("item(otyp={})", core.otyp.0)),
                ("quantity", core.quantity.to_string()),
            ],
        ));
    }
    events
}

// ---------------------------------------------------------------------------
// Autopickup
// ---------------------------------------------------------------------------

/// Default autopickup classes: gold is always picked up.
/// Other classes require explicit opt-in via configuration.
const ALWAYS_AUTOPICKUP: &[ObjectClass] = &[ObjectClass::Coin];

/// Run autopickup logic after the player moves to a new tile.
///
/// - Gold (`ObjectClass::Coin`) is always auto-picked up.
/// - Other item classes are picked up only if they appear in
///   `autopickup_classes`.
/// - Stops early if the inventory becomes full.
pub fn autopickup(
    world: &mut GameWorld,
    letter_state: &mut LetterState,
    obj_defs: &[ObjectDef],
    autopickup_classes: &[ObjectClass],
) -> Vec<EngineEvent> {
    let player = world.player();
    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return vec![],
    };

    let floor_items = items_at_position(world, player_pos);
    let mut events = Vec::new();

    for item in floor_items {
        // Check class eligibility.
        let should_pickup = {
            let core = match world.get_component::<ObjectCore>(item) {
                Some(c) => c,
                None => continue,
            };
            ALWAYS_AUTOPICKUP.contains(&core.object_class)
                || autopickup_classes.contains(&core.object_class)
        };

        if should_pickup {
            let pickup_events = pickup_item(world, player, item, letter_state, obj_defs);
            // If we got a "knapsack-full" message, stop trying.
            let full = pickup_events
                .iter()
                .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "knapsack-full"));
            events.extend(pickup_events);
            if full {
                break;
            }
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Merge knowledge inference
// ---------------------------------------------------------------------------

/// When two items merge, if they have different knowledge states for a
/// dimension, the merged item gains knowledge of that dimension.
///
/// Returns true if any new knowledge was gained (the caller should emit
/// a "learn-by-comparing" message).
pub fn merge_knowledge_inference(
    a_known: bool,
    b_known: bool,
    a_bknown: bool,
    b_bknown: bool,
    a_rknown: bool,
    b_rknown: bool,
) -> bool {
    let mut learned = false;
    if a_known != b_known {
        learned = true;
    }
    if a_bknown != b_bknown {
        learned = true;
    }
    if a_rknown != b_rknown {
        learned = true;
    }
    learned
}

// ---------------------------------------------------------------------------
// Candle age merge
// ---------------------------------------------------------------------------

/// Check if two candles can merge based on age.
/// Candles can merge only if `age / 25` is the same for both.
pub fn candles_can_merge(age_a: u32, age_b: u32) -> bool {
    age_a / 25 == age_b / 25
}

/// Compute the weighted-average age after merging two stacks.
pub fn merge_age(age_a: u32, qty_a: u32, age_b: u32, qty_b: u32) -> u32 {
    if qty_a + qty_b == 0 {
        return 0;
    }
    (age_a * qty_a + age_b * qty_b) / (qty_a + qty_b)
}

// ---------------------------------------------------------------------------
// Carry capacity (weight_cap from hack.c)
// ---------------------------------------------------------------------------

/// Maximum carrying capacity in weight units (cn).
///
/// Matches `MAX_CARR_CAP` from C NetHack.
pub const MAX_CARR_CAP: u32 = 1000;

/// Per-(STR+CON) multiplier for carry capacity.
pub const WT_WEIGHTCAP_STRCON: u32 = 25;

/// Additive constant in carry capacity formula.
pub const WT_WEIGHTCAP_SPARE: u32 = 50;

/// Reduction per wounded leg.
pub const WT_WOUNDEDLEG_REDUCT: u32 = 100;

/// Calculate carrying capacity in weight units.
///
/// Formula from C NetHack's `weight_cap()` (hack.c):
///   `carrcap = 25 * (STR + CON) + 50`
///
/// Adjustments:
/// - Levitating/flying: MAX_CARR_CAP
/// - Wounded legs: -100 per leg (unless flying)
/// - Capped at MAX_CARR_CAP (1000)
/// - Never returns 0 (minimum 1)
pub fn carry_capacity(
    strength: u8,
    constitution: u8,
    is_levitating: bool,
    is_flying: bool,
    wounded_legs: u8, // 0, 1, or 2
) -> u32 {
    if is_levitating {
        return MAX_CARR_CAP;
    }

    let mut cap =
        WT_WEIGHTCAP_STRCON * (strength as u32 + constitution as u32) + WT_WEIGHTCAP_SPARE;

    if cap > MAX_CARR_CAP {
        cap = MAX_CARR_CAP;
    }

    if !is_flying && wounded_legs > 0 {
        let reduction = WT_WOUNDEDLEG_REDUCT * wounded_legs as u32;
        cap = cap.saturating_sub(reduction);
    }

    cap.max(1)
}

/// Calculate encumbrance level from carried weight and capacity.
///
/// Matches C NetHack's `calc_capacity()`:
///   `excess = carried_weight - capacity`
///   if excess <= 0: Unencumbered
///   if capacity <= 1: Overloaded
///   `level = (excess * 2 / capacity) + 1`, capped at 5 (Overloaded)
pub fn encumbrance_level(carried_weight: u32, capacity: u32) -> Encumbrance {
    if carried_weight <= capacity {
        return Encumbrance::Unencumbered;
    }
    if capacity <= 1 {
        return Encumbrance::Overloaded;
    }

    let excess = carried_weight - capacity;
    let level = (excess * 2 / capacity) + 1;
    match level.min(5) {
        0 => Encumbrance::Unencumbered,
        1 => Encumbrance::Burdened,
        2 => Encumbrance::Stressed,
        3 => Encumbrance::Strained,
        4 => Encumbrance::Overtaxed,
        _ => Encumbrance::Overloaded,
    }
}

/// Calculate total weight carried by an entity.
///
/// Sums the weight of all items in the entity's inventory.  For containers,
/// applies bag-of-holding weight reduction recursively.
///
/// Items are looked up via `get_inventory()` — this requires the world to
/// have Inventory and ObjectCore components.
pub fn total_weight(world: &GameWorld, entity: Entity) -> u32 {
    let inv_items = items::get_inventory(world, entity);
    let mut wt: u32 = 0;

    for (_item_entity, core) in &inv_items {
        if core.object_class == ObjectClass::Coin {
            // Gold: weight = (quantity + 50) / 100 (C formula).
            wt += ((core.quantity as u32) + 50) / 100;
        } else {
            wt += core.weight;
        }
    }
    wt
}

/// Calculate the weight of a container and its contents.
///
/// The container's own weight is `own_weight`.  Contents weight is reduced
/// by bag-of-holding rules if applicable.
pub fn container_weight(
    own_weight: u32,
    contents_weight: u32,
    is_bag_of_holding: bool,
    buc: &BucStatus,
) -> u32 {
    let contents = if is_bag_of_holding {
        boh_content_weight(contents_weight, buc)
    } else {
        contents_weight
    };
    own_weight + contents
}

// ---------------------------------------------------------------------------
// Object merging
// ---------------------------------------------------------------------------

/// Check if two objects can be merged (stacked together).
///
/// Two objects are mergeable if they have the same:
/// - Object type (otyp)
/// - BUC status (if both are bknown, or both unknown)
/// - Erosion state
/// - Enchantment (for weapons/armor)
/// - Named status
/// - Object class must be inherently stackable
///
/// Returns true if the objects can merge.
pub fn can_merge(
    core_a: &ObjectCore,
    core_b: &ObjectCore,
    buc_a: Option<&BucStatus>,
    buc_b: Option<&BucStatus>,
) -> bool {
    // Must be same type.
    if core_a.otyp != core_b.otyp {
        return false;
    }

    // Must be same object class.
    if core_a.object_class != core_b.object_class {
        return false;
    }

    // Only stackable classes can merge.
    let stackable = matches!(
        core_a.object_class,
        ObjectClass::Weapon
            | ObjectClass::Food
            | ObjectClass::Potion
            | ObjectClass::Scroll
            | ObjectClass::Gem
            | ObjectClass::Coin
            | ObjectClass::Tool
    );
    if !stackable {
        return false;
    }

    // BUC must match (if known on both).
    if let (Some(a), Some(b)) = (buc_a, buc_b)
        && (a.blessed != b.blessed || a.cursed != b.cursed)
    {
        return false;
    }

    true
}

/// Merge two stackable objects.
///
/// Adds `source` quantity to `target`.  Returns the new quantity of `target`.
/// The caller is responsible for removing the source entity from the world.
pub fn merge_objects(target: &mut ObjectCore, source: &ObjectCore) -> i32 {
    target.quantity += source.quantity;
    target.quantity
}

// ---------------------------------------------------------------------------
// find_by_letter / sort_inventory
// ---------------------------------------------------------------------------

/// Find an inventory item by its assigned letter.
///
/// Scans the entity's inventory and returns the first item whose
/// `inv_letter` matches `letter`.  Returns `None` if no match is found.
/// This is equivalent to C NetHack's `carrying()` when called with a letter.
pub fn find_by_letter(world: &GameWorld, entity: Entity, letter: char) -> Option<Entity> {
    let inv_items = items::get_inventory(world, entity);
    for (item_entity, core) in &inv_items {
        if core.inv_letter == Some(letter) {
            return Some(*item_entity);
        }
    }
    None
}

/// Sort the entity's inventory in-place according to `packorder`.
///
/// Items are reordered so that classes appearing earlier in `packorder`
/// come first.  Within the same class, the original relative order is
/// preserved (stable sort).  This corresponds to C NetHack's `sortloot()`
/// and the `packorder` option.
pub fn sort_inventory(
    world: &GameWorld,
    entity: Entity,
    packorder: &[ObjectClass],
) -> Vec<(Entity, ObjectCore)> {
    let mut inv_items = items::get_inventory(world, entity);
    inv_items.sort_by(|(_, a), (_, b)| {
        let rank_a = packorder
            .iter()
            .position(|p| *p == a.object_class)
            .unwrap_or(usize::MAX);
        let rank_b = packorder
            .iter()
            .position(|p| *p == b.object_class)
            .unwrap_or(usize::MAX);
        rank_a
            .cmp(&rank_b)
            .then_with(|| inv_rank_cmp(a.inv_letter, b.inv_letter))
    });
    inv_items
}

/// Compare two inventory letters for ordering.  Letters sort alphabetically
/// with lowercase before uppercase (a-z then A-Z), matching C NetHack's
/// `invletter_value()` ordering.
fn inv_rank_cmp(a: Option<char>, b: Option<char>) -> std::cmp::Ordering {
    let va = a.map(invletter_value).unwrap_or(u8::MAX);
    let vb = b.map(invletter_value).unwrap_or(u8::MAX);
    va.cmp(&vb)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use nethack_babel_data::ObjectTypeId;

    /// Helper: build a minimal ObjectDef for testing.
    fn test_obj_def(id: u16, mergeable: bool) -> ObjectDef {
        ObjectDef {
            id: ObjectTypeId(id),
            name: format!("test_item_{}", id),
            appearance: None,
            class: ObjectClass::Weapon,
            color: nethack_babel_data::Color::White,
            material: nethack_babel_data::Material::Iron,
            weight: 10,
            cost: 5,
            nutrition: 0,
            prob: 10,
            is_magic: false,
            is_mergeable: mergeable,
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

    /// Helper: spawn a floor item.
    fn spawn_floor_item(world: &mut GameWorld, def: &ObjectDef, x: i16, y: i16) -> Entity {
        items::spawn_item(world, def, items::SpawnLocation::Floor(x, y), None)
    }

    /// Helper: create a GameWorld with an Inventory on the player.
    fn world_with_inventory(start: Position) -> GameWorld {
        let mut world = GameWorld::new(start);
        let player = world.player();
        let _ = world.ecs_mut().insert_one(player, Inventory::new());
        world
    }

    // ── Inventory component tests ────────────────────────────────

    #[test]
    fn test_add_item_to_inventory() {
        let mut inv = Inventory::new();
        let mut world = GameWorld::new(Position::new(0, 0));
        let e = world.spawn((ObjectCore {
            otyp: ObjectTypeId(1),
            object_class: ObjectClass::Weapon,
            quantity: 1,
            weight: 10,
            age: 0,
            inv_letter: None,
            artifact: None,
        },));

        let letter = inv.add(e);
        assert_eq!(letter, Some('a'));
        assert_eq!(inv.len(), 1);
        assert!(inv.contains(e));
    }

    #[test]
    fn test_inventory_52_limit() {
        let mut inv = Inventory::new();
        let mut world = GameWorld::new(Position::new(0, 0));

        for i in 0..52u16 {
            let e = world.spawn((ObjectCore {
                otyp: ObjectTypeId(i),
                object_class: ObjectClass::Weapon,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: None,
                artifact: None,
            },));
            let letter = inv.add(e);
            assert!(letter.is_some(), "slot {} should succeed", i);
        }

        assert!(inv.is_full());

        let overflow = world.spawn((ObjectCore {
            otyp: ObjectTypeId(999),
            object_class: ObjectClass::Weapon,
            quantity: 1,
            weight: 10,
            age: 0,
            inv_letter: None,
            artifact: None,
        },));
        assert_eq!(inv.add(overflow), None);
    }

    #[test]
    fn test_remove_item() {
        let mut inv = Inventory::new();
        let mut world = GameWorld::new(Position::new(0, 0));

        let e = world.spawn((ObjectCore {
            otyp: ObjectTypeId(1),
            object_class: ObjectClass::Weapon,
            quantity: 1,
            weight: 10,
            age: 0,
            inv_letter: None,
            artifact: None,
        },));

        inv.add(e);
        assert!(inv.contains(e));
        assert!(inv.remove(e));
        assert!(!inv.contains(e));
        assert!(inv.is_empty());
    }

    #[test]
    fn test_find_by_letter() {
        let mut inv = Inventory::new();
        let mut world = GameWorld::new(Position::new(0, 0));

        let mut entities = Vec::new();
        for i in 0..3u16 {
            let e = world.spawn((ObjectCore {
                otyp: ObjectTypeId(i),
                object_class: ObjectClass::Weapon,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: None,
                artifact: None,
            },));
            inv.add(e);
            entities.push(e);
        }

        assert_eq!(inv.find_by_letter('a'), Some(entities[0]));
        assert_eq!(inv.find_by_letter('b'), Some(entities[1]));
        assert_eq!(inv.find_by_letter('c'), Some(entities[2]));
        assert_eq!(inv.find_by_letter('d'), None);
    }

    // ── Gold weight tests ────────────────────────────────────────

    #[test]
    fn test_gold_weight_formula() {
        // TV7 from spec
        assert_eq!(gold_weight(1), 0); // (1+50)/100 = 0
        assert_eq!(gold_weight(50), 1); // (50+50)/100 = 1
        assert_eq!(gold_weight(100), 1); // (100+50)/100 = 1
        assert_eq!(gold_weight(149), 1); // (149+50)/100 = 1
        assert_eq!(gold_weight(150), 2); // (150+50)/100 = 2
        assert_eq!(gold_weight(0), 0); // edge case
    }

    #[test]
    fn test_gold_weight_min1() {
        assert_eq!(gold_weight_min1(1), 1); // min(0, 1) = 1
        assert_eq!(gold_weight_min1(49), 1); // 0 -> 1
        assert_eq!(gold_weight_min1(50), 1); // already 1
        assert_eq!(gold_weight_min1(100), 1);
    }

    #[test]
    fn test_gold_weight_delta() {
        // Merging 50 gold into 50 gold: was weight 1, now weight 1 = delta 0
        assert_eq!(gold_weight_delta(50, 50), 0);
        // Merging 100 gold into 100 gold: was 1, now 2 = delta 1
        assert_eq!(gold_weight_delta(100, 100), 1);
        // Merging 1 gold into 0: was 0, now 0 = delta 0
        assert_eq!(gold_weight_delta(0, 1), 0);
    }

    // ── Bag of Holding weight tests ──────────────────────────────

    #[test]
    fn test_boh_content_weight_uncursed() {
        let buc = BucStatus {
            cursed: false,
            blessed: false,
            bknown: false,
        };
        // TV12: uncursed BoH, cwt=1 → (1+1)/2 = 1
        assert_eq!(boh_content_weight(1, &buc), 1);
        assert_eq!(boh_content_weight(100, &buc), 50); // (100+1)/2 = 50
        assert_eq!(boh_content_weight(0, &buc), 0); // (0+1)/2 = 0
    }

    #[test]
    fn test_boh_content_weight_blessed() {
        let buc = BucStatus {
            cursed: false,
            blessed: true,
            bknown: false,
        };
        // TV12: blessed BoH, cwt=1 → (1+3)/4 = 1
        assert_eq!(boh_content_weight(1, &buc), 1);
        assert_eq!(boh_content_weight(0, &buc), 0); // (0+3)/4 = 0
        assert_eq!(boh_content_weight(100, &buc), 25); // (100+3)/4 = 25
    }

    #[test]
    fn test_boh_content_weight_cursed() {
        let buc = BucStatus {
            cursed: true,
            blessed: false,
            bknown: false,
        };
        // TV12: cursed BoH, cwt=100 → 100*2 = 200
        assert_eq!(boh_content_weight(100, &buc), 200);
        assert_eq!(boh_content_weight(0, &buc), 0);
    }

    #[test]
    fn test_delta_cwt_boh_blessed() {
        // TV17: blessed BoH with 400 content weight, removing 100 weight item
        let buc = BucStatus {
            cursed: false,
            blessed: true,
            bknown: false,
        };
        // Before: (400+3)/4 = 100. After: (300+3)/4 = 75. Delta = 25.
        assert_eq!(delta_cwt_boh(400, 100, true, &buc), 25);
    }

    #[test]
    fn test_delta_cwt_boh_cursed() {
        // TV17: cursed BoH with 100 content weight, removing 50 weight item
        let buc = BucStatus {
            cursed: true,
            blessed: false,
            bknown: false,
        };
        // Before: 100*2 = 200. After: 50*2 = 100. Delta = 100.
        assert_eq!(delta_cwt_boh(100, 50, true, &buc), 100);
    }

    #[test]
    fn test_delta_cwt_normal_container() {
        let buc = BucStatus {
            cursed: false,
            blessed: false,
            bknown: false,
        };
        // Non-BoH: delta = item weight directly.
        assert_eq!(delta_cwt_boh(200, 50, false, &buc), 50);
    }

    // ── Inventory counting tests ─────────────────────────────────

    #[test]
    fn test_inv_cnt_excludes_gold() {
        let mut world = world_with_inventory(Position::new(0, 0));
        let player = world.player();

        // Add a weapon and a gold piece.
        let weapon = world.spawn((ObjectCore {
            otyp: ObjectTypeId(1),
            object_class: ObjectClass::Weapon,
            quantity: 1,
            weight: 10,
            age: 0,
            inv_letter: Some('a'),
            artifact: None,
        },));
        let gold = world.spawn((ObjectCore {
            otyp: ObjectTypeId(500),
            object_class: ObjectClass::Coin,
            quantity: 100,
            weight: 1,
            age: 0,
            inv_letter: Some('$'),
            artifact: None,
        },));
        {
            let mut inv = world.get_component_mut::<Inventory>(player).unwrap();
            inv.items.push(weapon);
            inv.items.push(gold);
        }

        assert_eq!(inv_cnt(&world, player, false), 1); // weapon only
        assert_eq!(inv_cnt(&world, player, true), 2); // weapon + gold
    }

    // ── Invletter value tests ────────────────────────────────────

    #[test]
    fn test_invletter_value() {
        // TV13
        assert_eq!(invletter_value('$'), 1);
        assert_eq!(invletter_value('a'), 2);
        assert_eq!(invletter_value('z'), 27);
        assert_eq!(invletter_value('A'), 28);
        assert_eq!(invletter_value('Z'), 53);
        assert_eq!(invletter_value('#'), 54);
        assert_eq!(invletter_value('?'), 55);
    }

    // ── Autopickup configuration tests ───────────────────────────

    #[test]
    fn test_should_autopickup_gold_always() {
        let config = AutopickupConfig::default();
        assert!(should_autopickup(ObjectClass::Coin, HowLost::None, &config));
    }

    #[test]
    fn test_should_autopickup_thrown_override() {
        let config = AutopickupConfig {
            enabled: true,
            pickup_thrown: true,
            pickup_types: vec![], // normally would pick all
            ..Default::default()
        };
        // Thrown items should be picked up regardless
        assert!(should_autopickup(
            ObjectClass::Weapon,
            HowLost::Thrown,
            &config
        ));
    }

    #[test]
    fn test_should_autopickup_dropped_nopick() {
        let config = AutopickupConfig {
            enabled: true,
            nopick_dropped: true,
            pickup_types: vec![ObjectClass::Weapon],
            ..Default::default()
        };
        // Dropped weapons should NOT be picked up
        assert!(!should_autopickup(
            ObjectClass::Weapon,
            HowLost::Dropped,
            &config
        ));
    }

    #[test]
    fn test_should_autopickup_exploding_never() {
        let config = AutopickupConfig::default();
        assert!(!should_autopickup(
            ObjectClass::Weapon,
            HowLost::Exploding,
            &config
        ));
    }

    #[test]
    fn test_should_autopickup_disabled() {
        let config = AutopickupConfig {
            enabled: false,
            ..Default::default()
        };
        assert!(!should_autopickup(
            ObjectClass::Coin,
            HowLost::None,
            &config
        ));
    }

    #[test]
    fn test_should_autopickup_type_filter() {
        let config = AutopickupConfig {
            enabled: true,
            pickup_types: vec![ObjectClass::Potion],
            ..Default::default()
        };
        assert!(should_autopickup(
            ObjectClass::Potion,
            HowLost::None,
            &config
        ));
        assert!(!should_autopickup(
            ObjectClass::Weapon,
            HowLost::None,
            &config
        ));
    }

    // ── Encumbrance message tests ────────────────────────────────

    #[test]
    fn test_encumbrance_messages() {
        assert!(encumbrance_message(Encumbrance::Unencumbered).is_none());
        assert_eq!(
            encumbrance_message(Encumbrance::Burdened),
            Some("encumbrance-burdened")
        );
        assert_eq!(
            encumbrance_message(Encumbrance::Overloaded),
            Some("encumbrance-overloaded")
        );
    }

    #[test]
    fn test_pickup_encumbrance_check() {
        // Current = Unencumbered, new = Burdened, threshold = Stressed
        // Burdened <= Stressed, so no problem.
        assert!(
            pickup_encumbrance_check(
                Encumbrance::Unencumbered,
                Encumbrance::Burdened,
                Encumbrance::Stressed,
            )
            .is_none()
        );

        // Current = Unencumbered, new = Strained, threshold = Stressed
        // Strained > Stressed → warn.
        assert_eq!(
            pickup_encumbrance_check(
                Encumbrance::Unencumbered,
                Encumbrance::Strained,
                Encumbrance::Stressed,
            ),
            Some(Encumbrance::Strained)
        );

        // Current = Overtaxed, new = Overloaded, threshold = Stressed
        // Threshold becomes max(Overtaxed, Stressed) = Overtaxed
        // Overloaded > Overtaxed → warn.
        assert_eq!(
            pickup_encumbrance_check(
                Encumbrance::Overtaxed,
                Encumbrance::Overloaded,
                Encumbrance::Stressed,
            ),
            Some(Encumbrance::Overloaded)
        );
    }

    // ── Container restriction tests ──────────────────────────────

    #[test]
    fn test_container_restrictions() {
        assert_eq!(
            check_container_restriction(ObjectClass::Ball, false, false, false, false),
            Some(ContainerRestriction::BallOrChain)
        );
        assert_eq!(
            check_container_restriction(ObjectClass::Weapon, true, false, false, false),
            Some(ContainerRestriction::WornEquipment)
        );
        assert_eq!(
            check_container_restriction(ObjectClass::Gem, false, true, false, false),
            Some(ContainerRestriction::CursedLoadstone)
        );
        assert_eq!(
            check_container_restriction(ObjectClass::Amulet, false, false, true, false),
            Some(ContainerRestriction::QuestArtifact)
        );
        assert!(
            check_container_restriction(ObjectClass::Weapon, false, false, false, false).is_none()
        );
    }

    // ── Drop restriction tests ───────────────────────────────────

    #[test]
    fn test_drop_restrictions() {
        assert_eq!(
            check_drop_restriction(ObjectClass::Weapon, true, false, false),
            Some(CannotDrop::Welded)
        );
        assert_eq!(
            check_drop_restriction(ObjectClass::Gem, false, true, false),
            Some(CannotDrop::CursedLoadstone)
        );
        assert_eq!(
            check_drop_restriction(ObjectClass::Ball, false, false, false),
            Some(CannotDrop::BallOrChain)
        );
        assert_eq!(
            check_drop_restriction(ObjectClass::Armor, false, false, true),
            Some(CannotDrop::WornEquipment)
        );
        assert!(check_drop_restriction(ObjectClass::Weapon, false, false, false).is_none());
    }

    // ── Candle merge age tests ───────────────────────────────────

    #[test]
    fn test_candles_can_merge() {
        // TV9: age=100 and age=130 → 100/25=4, 130/25=5 → different
        assert!(!candles_can_merge(100, 130));
        // age=100 and age=120 → 100/25=4, 120/25=4 → same
        assert!(candles_can_merge(100, 120));
    }

    #[test]
    fn test_merge_age_weighted() {
        // Merging 100 (qty 3) and 120 (qty 2): (100*3 + 120*2) / 5 = 540/5 = 108
        assert_eq!(merge_age(100, 3, 120, 2), 108);
    }

    // ── Merge knowledge inference tests ──────────────────────────

    #[test]
    fn test_merge_knowledge_inference() {
        // Different known states → learned
        assert!(merge_knowledge_inference(
            true, false, false, false, false, false
        ));
        assert!(merge_knowledge_inference(
            false, false, true, false, false, false
        ));
        assert!(merge_knowledge_inference(
            false, false, false, false, true, false
        ));
        // Same states → not learned
        assert!(!merge_knowledge_inference(
            false, false, false, false, false, false
        ));
        assert!(!merge_knowledge_inference(
            true, true, true, true, true, true
        ));
    }

    // ── BoH explosion probability tests ──────────────────────────

    #[test]
    fn test_boh_explodes_depth_0_always() {
        // At depth 0, rng.random_range(0..1) always returns 0, and 0 <= 0 is true.
        let mut rng = rand::rng();
        for _ in 0..100 {
            assert!(boh_explodes(&mut rng, 0));
        }
    }

    #[test]
    fn test_boh_explodes_depth_1_always() {
        // At depth 1, rng.random_range(0..2) returns 0 or 1, both <= 1.
        let mut rng = rand::rng();
        for _ in 0..100 {
            assert!(boh_explodes(&mut rng, 1));
        }
    }

    #[test]
    fn test_boh_explosion_damage_range() {
        let mut rng = rand::rng();
        for _ in 0..100 {
            let dmg = boh_explosion_damage(&mut rng);
            assert!(dmg >= 6, "6d6 minimum is 6, got {}", dmg);
            assert!(dmg <= 36, "6d6 maximum is 36, got {}", dmg);
        }
    }

    // ── Item splitting tests ─────────────────────────────────────

    #[test]
    fn test_split_stack() {
        let mut world = GameWorld::new(Position::new(0, 0));
        let def = test_obj_def(1, true);
        let defs = vec![def.clone()];
        let item = items::spawn_item(&mut world, &def, items::SpawnLocation::Inventory, None);
        {
            let mut core = world.get_component_mut::<ObjectCore>(item).unwrap();
            core.quantity = 20;
            core.weight = 200; // 20 * 10
        }

        let result = split_stack(&mut world, item, 5, &defs);

        // Parent should have 15.
        let parent_core = world.get_component::<ObjectCore>(result.parent).unwrap();
        assert_eq!(parent_core.quantity, 15);
        assert_eq!(parent_core.weight, 150);

        // Child should have 5.
        let child_core = world.get_component::<ObjectCore>(result.child).unwrap();
        assert_eq!(child_core.quantity, 5);
        assert_eq!(child_core.weight, 50);
    }

    #[test]
    #[should_panic(expected = "num must be > 0")]
    fn test_split_stack_zero_panics() {
        let mut world = GameWorld::new(Position::new(0, 0));
        let def = test_obj_def(1, true);
        let defs = vec![def.clone()];
        let item = items::spawn_item(&mut world, &def, items::SpawnLocation::Inventory, None);
        {
            let mut core = world.get_component_mut::<ObjectCore>(item).unwrap();
            core.quantity = 10;
        }
        split_stack(&mut world, item, 0, &defs);
    }

    #[test]
    #[should_panic(expected = "num must be < quantity")]
    fn test_split_stack_all_panics() {
        let mut world = GameWorld::new(Position::new(0, 0));
        let def = test_obj_def(1, true);
        let defs = vec![def.clone()];
        let item = items::spawn_item(&mut world, &def, items::SpawnLocation::Inventory, None);
        {
            let mut core = world.get_component_mut::<ObjectCore>(item).unwrap();
            core.quantity = 10;
        }
        split_stack(&mut world, item, 10, &defs);
    }

    // ── Pickup from floor tests ──────────────────────────────────

    #[test]
    fn test_pickup_from_floor() {
        let mut world = world_with_inventory(Position::new(5, 5));
        let player = world.player();
        let def = test_obj_def(1, false);
        let defs = vec![def.clone()];
        let mut ls = LetterState::default();

        let item = spawn_floor_item(&mut world, &def, 5, 5);

        let events = pickup_item(&mut world, player, item, &mut ls, &defs);

        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::ItemPickedUp { .. })),
            "expected ItemPickedUp event"
        );

        let inv = world
            .get_component::<Inventory>(player)
            .expect("player should have Inventory");
        assert!(inv.contains(item));

        let loc = world
            .get_component::<ObjectLocation>(item)
            .expect("item should have ObjectLocation");
        assert!(matches!(*loc, ObjectLocation::Inventory));
    }

    #[test]
    fn test_drop_to_floor() {
        let mut world = world_with_inventory(Position::new(10, 10));
        let player = world.player();
        let def = test_obj_def(1, false);
        let defs = vec![def.clone()];
        let mut ls = LetterState::default();

        let item = spawn_floor_item(&mut world, &def, 10, 10);
        pickup_item(&mut world, player, item, &mut ls, &defs);

        let events = drop_item(&mut world, player, item);

        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::ItemDropped { .. })),
            "expected ItemDropped event"
        );

        let inv = world
            .get_component::<Inventory>(player)
            .expect("player should have Inventory");
        assert!(!inv.contains(item));

        let loc = world
            .get_component::<ObjectLocation>(item)
            .expect("item should have ObjectLocation");
        assert!(
            matches!(*loc, ObjectLocation::Floor { x: 10, y: 10 }),
            "expected Floor(10,10), got {:?}",
            *loc
        );
    }

    #[test]
    fn test_merge_stackable() {
        let mut world = world_with_inventory(Position::new(5, 5));
        let player = world.player();
        let def = test_obj_def(1, true);
        let defs = vec![def.clone()];
        let mut ls = LetterState::default();

        let item1 = spawn_floor_item(&mut world, &def, 5, 5);
        {
            let mut core = world.get_component_mut::<ObjectCore>(item1).unwrap();
            core.quantity = 5;
            core.weight = 50;
        }
        pickup_item(&mut world, player, item1, &mut ls, &defs);

        {
            let inv = world.get_component::<Inventory>(player).unwrap();
            assert!(inv.contains(item1));
        }

        let item2 = spawn_floor_item(&mut world, &def, 5, 5);
        {
            let mut core = world.get_component_mut::<ObjectCore>(item2).unwrap();
            core.quantity = 3;
            core.weight = 30;
        }
        pickup_item(&mut world, player, item2, &mut ls, &defs);

        let core = world
            .get_component::<ObjectCore>(item1)
            .expect("item1 should still exist");
        assert_eq!(core.quantity, 8);

        assert!(world.get_component::<ObjectCore>(item2).is_none());

        let inv = world.get_component::<Inventory>(player).unwrap();
        assert!(inv.contains(item1));
        assert!(!inv.contains(item2));
        assert_eq!(inv.len(), 1);
    }

    #[test]
    fn test_gold_autopickup() {
        let mut world = world_with_inventory(Position::new(5, 5));
        let mut ls = LetterState::default();

        let gold_def = ObjectDef {
            id: ObjectTypeId(500),
            name: "gold piece".to_string(),
            class: ObjectClass::Coin,
            is_mergeable: true,
            ..test_obj_def(500, true)
        };
        let defs = vec![gold_def.clone()];

        let gold = items::spawn_item(
            &mut world,
            &gold_def,
            items::SpawnLocation::Floor(5, 5),
            None,
        );
        {
            let mut core = world.get_component_mut::<ObjectCore>(gold).unwrap();
            core.object_class = ObjectClass::Coin;
            core.quantity = 100;
        }

        let events = autopickup(&mut world, &mut ls, &defs, &[]);

        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::ItemPickedUp { .. })),
            "gold should be auto-picked up"
        );

        let player = world.player();
        let inv = world
            .get_component::<Inventory>(player)
            .expect("player should have Inventory");
        assert!(inv.contains(gold));
    }

    // ── Packorder sorting test ───────────────────────────────────

    #[test]
    fn test_sort_by_packorder() {
        let mut world = world_with_inventory(Position::new(0, 0));
        let player = world.player();

        // Add items of different classes.
        let weapon = world.ecs_mut().spawn((ObjectCore {
            otyp: ObjectTypeId(1),
            object_class: ObjectClass::Weapon,
            quantity: 1,
            weight: 10,
            age: 0,
            inv_letter: Some('a'),
            artifact: None,
        },));
        let food = world.ecs_mut().spawn((ObjectCore {
            otyp: ObjectTypeId(2),
            object_class: ObjectClass::Food,
            quantity: 1,
            weight: 5,
            age: 0,
            inv_letter: Some('b'),
            artifact: None,
        },));
        let coin = world.ecs_mut().spawn((ObjectCore {
            otyp: ObjectTypeId(3),
            object_class: ObjectClass::Coin,
            quantity: 100,
            weight: 1,
            age: 0,
            inv_letter: Some('$'),
            artifact: None,
        },));

        {
            let mut inv = world.get_component_mut::<Inventory>(player).unwrap();
            inv.items.push(weapon);
            inv.items.push(food);
            inv.items.push(coin);
        }

        // Sort by default packorder: Coin < Weapon < Food
        {
            let mut items: Vec<Entity> = {
                let inv = world.get_component::<Inventory>(player).unwrap();
                inv.items.clone()
            };
            items.sort_by(|&a, &b| {
                let class_a = world.get_component::<ObjectCore>(a).map(|c| c.object_class);
                let class_b = world.get_component::<ObjectCore>(b).map(|c| c.object_class);
                let rank_a = class_a
                    .and_then(|c| DEFAULT_PACKORDER.iter().position(|p| *p == c))
                    .unwrap_or(usize::MAX);
                let rank_b = class_b
                    .and_then(|c| DEFAULT_PACKORDER.iter().position(|p| *p == c))
                    .unwrap_or(usize::MAX);
                rank_a.cmp(&rank_b)
            });
            let mut inv = world.get_component_mut::<Inventory>(player).unwrap();
            inv.items = items;
        }

        let inv = world.get_component::<Inventory>(player).unwrap();
        let classes: Vec<ObjectClass> = inv
            .items
            .iter()
            .map(|&e| world.get_component::<ObjectCore>(e).unwrap().object_class)
            .collect();

        assert_eq!(
            classes,
            vec![ObjectClass::Coin, ObjectClass::Weapon, ObjectClass::Food]
        );
    }

    // ── Inv rank sorting test ────────────────────────────────────

    #[test]
    fn test_inv_rank_order() {
        // $ < a < z < A < Z
        assert!(inv_rank('$') < inv_rank('a'));
        assert!(inv_rank('a') < inv_rank('z'));
        assert!(inv_rank('z') < inv_rank('A'));
        assert!(inv_rank('A') < inv_rank('Z'));
    }

    // ── Carry capacity tests ─────────────────────────────────────

    #[test]
    fn test_carry_capacity_basic() {
        // STR=10, CON=10: 25*(10+10)+50 = 550
        assert_eq!(carry_capacity(10, 10, false, false, 0), 550);
    }

    #[test]
    fn test_carry_capacity_strong() {
        // STR=18, CON=18: 25*(18+18)+50 = 950
        assert_eq!(carry_capacity(18, 18, false, false, 0), 950);
    }

    #[test]
    fn test_carry_capacity_max_cap() {
        // STR=25, CON=25: 25*(25+25)+50 = 1300, capped to 1000
        assert_eq!(carry_capacity(25, 25, false, false, 0), MAX_CARR_CAP);
    }

    #[test]
    fn test_carry_capacity_levitating() {
        // Levitating always gives MAX_CARR_CAP
        assert_eq!(carry_capacity(1, 1, true, false, 0), MAX_CARR_CAP);
    }

    #[test]
    fn test_carry_capacity_wounded_legs() {
        // STR=10, CON=10: 550, minus 100 per wounded leg
        assert_eq!(carry_capacity(10, 10, false, false, 1), 450);
        assert_eq!(carry_capacity(10, 10, false, false, 2), 350);
    }

    #[test]
    fn test_carry_capacity_flying_ignores_wounded() {
        // Flying ignores wounded legs
        assert_eq!(carry_capacity(10, 10, false, true, 2), 550);
    }

    #[test]
    fn test_carry_capacity_min_one() {
        // Even with terrible stats and wounded, never returns 0
        assert!(carry_capacity(1, 1, false, false, 2) >= 1);
    }

    // ── Encumbrance level tests ──────────────────────────────────

    #[test]
    fn test_encumbrance_unencumbered() {
        assert_eq!(encumbrance_level(400, 550), Encumbrance::Unencumbered);
    }

    #[test]
    fn test_encumbrance_at_capacity() {
        assert_eq!(encumbrance_level(550, 550), Encumbrance::Unencumbered);
    }

    #[test]
    fn test_encumbrance_burdened() {
        // excess=50, cap=550: level = (50*2/550)+1 = 1 => Burdened
        assert_eq!(encumbrance_level(600, 550), Encumbrance::Burdened);
    }

    #[test]
    fn test_encumbrance_overloaded() {
        // Way over capacity
        assert_eq!(encumbrance_level(2000, 550), Encumbrance::Overloaded);
    }

    #[test]
    fn test_encumbrance_tiny_capacity() {
        // Capacity <= 1 with any excess => Overloaded
        assert_eq!(encumbrance_level(2, 1), Encumbrance::Overloaded);
    }

    // ── Container weight tests ───────────────────────────────────

    #[test]
    fn test_container_weight_normal() {
        assert_eq!(
            container_weight(
                15,
                100,
                false,
                &BucStatus {
                    blessed: false,
                    cursed: false,
                    bknown: false
                }
            ),
            115
        );
    }

    #[test]
    fn test_container_weight_boh_uncursed() {
        // Uncursed BoH: contents weight halved
        let buc = BucStatus {
            blessed: false,
            cursed: false,
            bknown: false,
        };
        assert_eq!(container_weight(15, 100, true, &buc), 15 + 50);
    }

    #[test]
    fn test_container_weight_boh_blessed() {
        // Blessed BoH: contents weight reduced by 75%
        let buc = BucStatus {
            blessed: true,
            cursed: false,
            bknown: true,
        };
        assert_eq!(container_weight(15, 100, true, &buc), 15 + 25);
    }

    // ── Object merge tests ───────────────────────────────────────

    #[test]
    fn test_can_merge_same_weapon() {
        let core_a = ObjectCore {
            otyp: ObjectTypeId(1),
            quantity: 5,
            weight: 10,
            age: 0,
            inv_letter: None,
            artifact: None,
            object_class: ObjectClass::Weapon,
        };
        let core_b = ObjectCore {
            otyp: ObjectTypeId(1),
            quantity: 3,
            weight: 10,
            age: 0,
            inv_letter: None,
            artifact: None,
            object_class: ObjectClass::Weapon,
        };
        assert!(can_merge(&core_a, &core_b, None, None));
    }

    #[test]
    fn test_cannot_merge_different_type() {
        let core_a = ObjectCore {
            otyp: ObjectTypeId(1),
            quantity: 5,
            weight: 10,
            age: 0,
            inv_letter: None,
            artifact: None,
            object_class: ObjectClass::Weapon,
        };
        let core_b = ObjectCore {
            otyp: ObjectTypeId(2),
            quantity: 3,
            weight: 10,
            age: 0,
            inv_letter: None,
            artifact: None,
            object_class: ObjectClass::Weapon,
        };
        assert!(!can_merge(&core_a, &core_b, None, None));
    }

    #[test]
    fn test_cannot_merge_armor() {
        // Armor is not stackable.
        let core_a = ObjectCore {
            otyp: ObjectTypeId(1),
            quantity: 1,
            weight: 50,
            age: 0,
            inv_letter: None,
            artifact: None,
            object_class: ObjectClass::Armor,
        };
        let core_b = core_a.clone();
        assert!(!can_merge(&core_a, &core_b, None, None));
    }

    #[test]
    fn test_cannot_merge_different_buc() {
        let core_a = ObjectCore {
            otyp: ObjectTypeId(1),
            quantity: 5,
            weight: 10,
            age: 0,
            inv_letter: None,
            artifact: None,
            object_class: ObjectClass::Potion,
        };
        let core_b = core_a.clone();
        let buc_a = BucStatus {
            blessed: true,
            cursed: false,
            bknown: true,
        };
        let buc_b = BucStatus {
            blessed: false,
            cursed: true,
            bknown: true,
        };
        assert!(!can_merge(&core_a, &core_b, Some(&buc_a), Some(&buc_b)));
    }

    #[test]
    fn test_merge_objects_quantity() {
        let mut target = ObjectCore {
            otyp: ObjectTypeId(1),
            quantity: 5,
            weight: 10,
            age: 0,
            inv_letter: Some('a'),
            artifact: None,
            object_class: ObjectClass::Weapon,
        };
        let source = ObjectCore {
            otyp: ObjectTypeId(1),
            quantity: 3,
            weight: 10,
            age: 0,
            inv_letter: Some('b'),
            artifact: None,
            object_class: ObjectClass::Weapon,
        };
        let new_qty = merge_objects(&mut target, &source);
        assert_eq!(new_qty, 8);
        assert_eq!(target.quantity, 8);
    }

    // ── Gold weight tests (extended) ─────────────────────────────

    #[test]
    fn test_gold_weight_formula_extended() {
        // C formula: (quantity + 50) / 100
        assert_eq!(gold_weight(0), 0);
        assert_eq!(gold_weight(50), 1);
        assert_eq!(gold_weight(100), 1);
        assert_eq!(gold_weight(150), 2);
        assert_eq!(gold_weight(1000), 10);
    }

    // ── find_by_letter tests ────────────────────────────────────

    #[test]
    fn test_find_by_letter_found() {
        let mut world = world_with_inventory(Position { x: 1, y: 1 });
        let player = world.player();
        let def = test_obj_def(1, false);
        let item = items::spawn_item(&mut world, &def, items::SpawnLocation::Inventory, None);
        // Assign letter 'a'.
        world
            .get_component_mut::<ObjectCore>(item)
            .unwrap()
            .inv_letter = Some('a');
        assert_eq!(find_by_letter(&world, player, 'a'), Some(item));
    }

    #[test]
    fn test_find_by_letter_not_found() {
        let mut world = world_with_inventory(Position { x: 1, y: 1 });
        let player = world.player();
        let def = test_obj_def(1, false);
        let item = items::spawn_item(&mut world, &def, items::SpawnLocation::Inventory, None);
        world
            .get_component_mut::<ObjectCore>(item)
            .unwrap()
            .inv_letter = Some('a');
        assert_eq!(find_by_letter(&world, player, 'z'), None);
    }

    #[test]
    fn test_find_by_letter_empty_inventory() {
        let world = world_with_inventory(Position { x: 1, y: 1 });
        let player = world.player();
        assert_eq!(find_by_letter(&world, player, 'a'), None);
    }

    // ── sort_inventory tests ────────────────────────────────────

    #[test]
    fn test_sort_inventory_by_packorder() {
        let mut world = world_with_inventory(Position { x: 1, y: 1 });
        let player = world.player();

        // Spawn items of different classes.
        let mut def_weapon = test_obj_def(1, false);
        def_weapon.class = ObjectClass::Weapon;
        let mut def_food = test_obj_def(2, false);
        def_food.class = ObjectClass::Food;
        let mut def_potion = test_obj_def(3, false);
        def_potion.class = ObjectClass::Potion;

        let w = items::spawn_item(
            &mut world,
            &def_weapon,
            items::SpawnLocation::Inventory,
            None,
        );
        world.get_component_mut::<ObjectCore>(w).unwrap().inv_letter = Some('c');

        let f = items::spawn_item(&mut world, &def_food, items::SpawnLocation::Inventory, None);
        world.get_component_mut::<ObjectCore>(f).unwrap().inv_letter = Some('a');

        let p = items::spawn_item(
            &mut world,
            &def_potion,
            items::SpawnLocation::Inventory,
            None,
        );
        world.get_component_mut::<ObjectCore>(p).unwrap().inv_letter = Some('b');

        // Packorder: Food, Potion, Weapon.
        let packorder = vec![ObjectClass::Food, ObjectClass::Potion, ObjectClass::Weapon];
        let sorted = sort_inventory(&world, player, &packorder);

        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].1.object_class, ObjectClass::Food);
        assert_eq!(sorted[1].1.object_class, ObjectClass::Potion);
        assert_eq!(sorted[2].1.object_class, ObjectClass::Weapon);
    }

    #[test]
    fn test_sort_inventory_empty() {
        let world = world_with_inventory(Position { x: 1, y: 1 });
        let player = world.player();
        let sorted = sort_inventory(&world, player, &[ObjectClass::Weapon]);
        assert!(sorted.is_empty());
    }

    // ── inv_rank_cmp tests ──────────────────────────────────────

    #[test]
    fn test_inv_rank_cmp_lowercase_before_uppercase() {
        use std::cmp::Ordering;
        // 'a' should come before 'A'.
        assert_eq!(inv_rank_cmp(Some('a'), Some('A')), Ordering::Less);
        assert_eq!(inv_rank_cmp(Some('z'), Some('A')), Ordering::Less);
    }

    #[test]
    fn test_inv_rank_cmp_none_sorts_last() {
        use std::cmp::Ordering;
        assert_eq!(inv_rank_cmp(None, Some('a')), Ordering::Greater);
        assert_eq!(inv_rank_cmp(Some('a'), None), Ordering::Less);
    }
}
