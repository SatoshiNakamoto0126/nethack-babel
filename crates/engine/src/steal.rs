//! Theft mechanics: gold stealing (leprechaun), item stealing (nymph), and
//! multi-turn armor removal (foocubi/nymph seduction).
//!
//! Ported from C NetHack's `steal.c`.  All functions operate on the ECS
//! `GameWorld` and return `Vec<EngineEvent>` — no IO, no global state.

use hecs::Entity;
use rand::Rng;

use nethack_babel_data::{DamageType, MonsterDef, ObjectDef};

use crate::equipment::EquipmentSlots;
use crate::event::EngineEvent;
use crate::inventory::Inventory;
use crate::world::{GameWorld, Player};

// ---------------------------------------------------------------------------
// Gold theft
// ---------------------------------------------------------------------------

/// Calculate how much gold to steal: random portion of the total.
/// Mirrors C `somegold(umoney)`.
pub fn somegold<R: Rng>(rng: &mut R, gold: u32) -> u32 {
    if gold == 0 {
        return 0;
    }
    match gold {
        1..=10 => gold,
        11..=100 => {
            let r: u32 = rng.random_range(1..=10);
            if r > gold { gold } else { r }
        }
        101..=500 => {
            let r: u32 = rng.random_range(1..=20);
            if r > gold { gold } else { r }
        }
        501..=1000 => {
            let r: u32 = rng.random_range(1..=50);
            if r > gold { gold } else { r }
        }
        1001..=5000 => {
            let r: u32 = rng.random_range(1..=100);
            if r > gold { gold } else { r }
        }
        _ => {
            let r: u32 = rng.random_range(1..=500);
            if r > gold { gold } else { r }
        }
    }
}

/// A leprechaun-style gold theft: stealer takes some gold from the victim.
///
/// Returns events describing the theft and the amount stolen.
/// The caller is responsible for updating any gold-count components
/// on victim/thief — this function only emits events and computes amounts.
pub fn stealgold<R: Rng>(
    rng: &mut R,
    _thief: Entity,
    _victim: Entity,
    victim_gold: u32,
) -> (Vec<EngineEvent>, u32) {
    let mut events = Vec::new();

    if victim_gold == 0 {
        events.push(EngineEvent::msg("steal-no-gold"));
        return (events, 0);
    }

    let amount = somegold(rng, victim_gold);

    events.push(EngineEvent::msg_with(
        "steal-gold",
        vec![("amount", amount.to_string())],
    ));

    (events, amount)
}

// ---------------------------------------------------------------------------
// Item theft
// ---------------------------------------------------------------------------

/// Result of an item theft attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StealResult {
    /// Successfully stole an item.
    Stolen { item: Entity },
    /// Victim has no stealable items.
    NothingToSteal,
    /// Thief already has too many items.
    ThiefFull,
}

/// Attempt to steal a random item from the victim's inventory.
///
/// Mirrors the nymph theft logic from C `steal.c`.  Selects a random
/// item from the victim's inventory.  Does NOT remove the item — the
/// caller should transfer the item between inventories and emit events.
///
/// Returns which item was selected (or why theft failed).
pub fn pick_steal_target<R: Rng>(
    rng: &mut R,
    world: &GameWorld,
    victim: Entity,
) -> StealResult {
    let inv = match world.get_component::<Inventory>(victim) {
        Some(inv) => inv,
        None => return StealResult::NothingToSteal,
    };

    if inv.items.is_empty() {
        return StealResult::NothingToSteal;
    }

    let idx = rng.random_range(0..inv.items.len());
    StealResult::Stolen { item: inv.items[idx] }
}

/// Execute an item theft: remove from victim, add to thief.
///
/// Returns events and whether the transfer succeeded.
pub fn execute_item_theft(
    world: &mut GameWorld,
    thief: Entity,
    victim: Entity,
    item: Entity,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Remove from victim
    let removed = {
        if let Some(mut inv) = world.get_component_mut::<Inventory>(victim) {
            inv.remove(item)
        } else {
            false
        }
    };

    if !removed {
        return events;
    }

    // Add to thief
    let added = {
        if let Some(mut inv) = world.get_component_mut::<Inventory>(thief) {
            inv.add(item).is_some()
        } else {
            false
        }
    };

    if added {
        let victim_name = world.entity_name(victim);
        let is_player = world.get_component::<Player>(victim).is_some();

        if is_player {
            events.push(EngineEvent::msg("steal-item-from-you"));
        } else {
            events.push(EngineEvent::msg_with(
                "steal-item",
                vec![("victim", victim_name)],
            ));
        }
    } else {
        // Thief full — return item to victim
        if let Some(mut inv) = world.get_component_mut::<Inventory>(victim) {
            inv.add(item);
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Monster steal capability check
// ---------------------------------------------------------------------------

/// Check whether a monster species has a steal attack (AD_SGLD or AD_SITM).
///
/// Mirrors the C steal.c logic for determining whether a monster can steal.
/// Leprechauns have AD_SGLD (GoldSteal), nymphs have AD_SITM (ItemSteal).
pub fn can_steal(monster_def: &MonsterDef) -> bool {
    monster_def.attacks.iter().any(|atk| {
        matches!(
            atk.damage_type,
            DamageType::GoldSteal | DamageType::ItemSteal | DamageType::Seduce
        )
    })
}

// ---------------------------------------------------------------------------
// Stolen value tracking (shopkeeper)
// ---------------------------------------------------------------------------

/// Calculate the shopkeeper-tracked value of a stolen item.
///
/// In NetHack, shopkeepers track the value of stolen goods so the player
/// can be charged if they return to the shop.  Value is base cost * quantity.
pub fn stolen_value(obj_def: &ObjectDef, quantity: u32) -> u32 {
    let base = obj_def.cost.unsigned_abs() as u32;
    base * quantity
}

// ---------------------------------------------------------------------------
// Remove worn item before stealing
// ---------------------------------------------------------------------------

/// Remove a worn/equipped item from an entity before it can be stolen.
///
/// Returns events describing the unequipping.  If the item is not
/// currently equipped, returns an empty vec.
pub fn remove_worn_item(
    world: &mut GameWorld,
    entity: Entity,
    item: Entity,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Check if item is equipped
    let is_equipped = {
        if let Some(equip) = world.get_component::<EquipmentSlots>(entity) {
            equip.find_slot(item).is_some()
        } else {
            false
        }
    };

    if !is_equipped {
        return events;
    }

    // Use equipment::unequip_item if available, otherwise manual removal
    match crate::equipment::unequip_item(world, entity, item) {
        Ok(unequip_events) => {
            events.extend(unequip_events);
        }
        Err(_) => {
            // Fallback: manually clear the slot
            if let Some(mut equip) = world.get_component_mut::<EquipmentSlots>(entity) {
                if equip.weapon == Some(item) {
                    equip.weapon = None;
                } else if equip.helmet == Some(item) {
                    equip.helmet = None;
                } else if equip.cloak == Some(item) {
                    equip.cloak = None;
                } else if equip.shield == Some(item) {
                    equip.shield = None;
                } else if equip.gloves == Some(item) {
                    equip.gloves = None;
                } else if equip.boots == Some(item) {
                    equip.boots = None;
                } else if equip.shirt == Some(item) {
                    equip.shirt = None;
                } else if equip.ring_left == Some(item) {
                    equip.ring_left = None;
                } else if equip.ring_right == Some(item) {
                    equip.ring_right = None;
                } else if equip.amulet == Some(item) {
                    equip.amulet = None;
                }
            }
        }
    }

    events.push(EngineEvent::ItemRemoved {
        actor: entity,
        item,
    });

    events
}

// ---------------------------------------------------------------------------
// Multi-turn armor stealing
// ---------------------------------------------------------------------------

/// Represents an in-progress armor theft (foocubus/nymph seduction).
///
/// In NetHack, seduction-based armor stealing takes multiple turns:
/// the monster asks the player to remove armor one piece at a time.
/// This struct tracks the state of that process.
#[derive(Debug, Clone)]
pub struct ArmorTheftState {
    /// The entity doing the stealing.
    pub thief: Entity,
    /// Number of turns the theft has been in progress.
    pub turns_elapsed: u32,
    /// Maximum turns before the thief gives up.
    pub max_turns: u32,
}

impl ArmorTheftState {
    pub fn new(thief: Entity) -> Self {
        Self {
            thief,
            turns_elapsed: 0,
            max_turns: 10,
        }
    }

    /// Advance the theft by one turn.
    /// Returns `true` if the theft should continue, `false` if it's done.
    pub fn tick(&mut self) -> bool {
        self.turns_elapsed += 1;
        self.turns_elapsed < self.max_turns
    }

    /// Whether the theft is complete (timed out).
    pub fn is_done(&self) -> bool {
        self.turns_elapsed >= self.max_turns
    }
}

// ---------------------------------------------------------------------------
// Thief-dies cleanup
// ---------------------------------------------------------------------------

/// When a thief dies, drop all stolen items at its position.
///
/// Returns events for each item dropped.  The caller is responsible for
/// actually spawning the items on the map.
pub fn thief_died_inventory(
    world: &mut GameWorld,
    thief: Entity,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let items: Vec<Entity> = {
        match world.get_component::<Inventory>(thief) {
            Some(inv) => inv.items.clone(),
            None => return events,
        }
    };

    if items.is_empty() {
        return events;
    }

    // Clear the thief's inventory
    if let Some(mut inv) = world.get_component_mut::<Inventory>(thief) {
        inv.items.clear();
    }

    events.push(EngineEvent::msg_with(
        "thief-drops-items",
        vec![("count", items.len().to_string())],
    ));

    for item in &items {
        events.push(EngineEvent::ItemDropped {
            actor: thief,
            item: *item,
        });
    }

    events
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::world::GameWorld;
    use rand::rngs::SmallRng;
    use rand::SeedableRng;

    fn test_world() -> GameWorld {
        GameWorld::new(Position::new(5, 5))
    }

    fn test_rng() -> SmallRng {
        SmallRng::seed_from_u64(42)
    }

    // ── somegold ──────────────────────────────────────────────

    #[test]
    fn somegold_zero_returns_zero() {
        let mut rng = test_rng();
        assert_eq!(somegold(&mut rng, 0), 0);
    }

    #[test]
    fn somegold_small_amount_returns_all() {
        let mut rng = test_rng();
        for gold in 1..=10 {
            assert_eq!(somegold(&mut rng, gold), gold);
        }
    }

    #[test]
    fn somegold_medium_returns_bounded() {
        let mut rng = test_rng();
        for _ in 0..100 {
            let amount = somegold(&mut rng, 50);
            assert!(amount >= 1 && amount <= 10);
        }
    }

    #[test]
    fn somegold_large_returns_bounded() {
        let mut rng = test_rng();
        for _ in 0..100 {
            let amount = somegold(&mut rng, 10000);
            assert!(amount >= 1 && amount <= 500);
        }
    }

    // ── stealgold ─────────────────────────────────────────────

    #[test]
    fn stealgold_no_gold_emits_no_gold_message() {
        let mut rng = test_rng();
        let world = test_world();
        let thief = world.player(); // using player as placeholder
        let victim = world.player();

        let (events, amount) = stealgold(&mut rng, thief, victim, 0);
        assert_eq!(amount, 0);
        assert!(!events.is_empty());
    }

    #[test]
    fn stealgold_with_gold_returns_positive_amount() {
        let mut rng = test_rng();
        let world = test_world();
        let thief = world.player();
        let victim = world.player();

        let (events, amount) = stealgold(&mut rng, thief, victim, 100);
        assert!(amount > 0);
        assert!(amount <= 100);
        assert!(!events.is_empty());
    }

    // ── pick_steal_target ─────────────────────────────────────

    #[test]
    fn pick_steal_empty_inventory() {
        let mut rng = test_rng();
        let world = test_world();

        // Player has an empty inventory by default
        let result = pick_steal_target(&mut rng, &world, world.player());
        assert_eq!(result, StealResult::NothingToSteal);
    }

    #[test]
    fn pick_steal_with_items() {
        let mut rng = test_rng();
        let mut world = test_world();

        // Add a dummy item to the player's inventory
        let item = world.spawn(());
        {
            let mut inv = world.get_component_mut::<Inventory>(world.player()).unwrap();
            inv.add(item);
        }

        let result = pick_steal_target(&mut rng, &world, world.player());
        assert_eq!(result, StealResult::Stolen { item });
    }

    // ── execute_item_theft ────────────────────────────────────

    #[test]
    fn execute_theft_transfers_item() {
        let mut world = test_world();

        // Create a thief with an inventory
        let thief = world.spawn((Inventory::new(),));
        let item = world.spawn(());

        // Add item to player inventory
        {
            let mut inv = world.get_component_mut::<Inventory>(world.player()).unwrap();
            inv.add(item);
        }

        let player = world.player();
        let events = execute_item_theft(&mut world, thief, player, item);
        assert!(!events.is_empty());

        // Verify item moved to thief
        let thief_inv = world.get_component::<Inventory>(thief).unwrap();
        assert!(thief_inv.items.contains(&item));

        // Verify item removed from victim
        let player_inv = world.get_component::<Inventory>(world.player()).unwrap();
        assert!(!player_inv.items.contains(&item));
    }

    #[test]
    fn execute_theft_no_inventory_on_victim() {
        let mut world = test_world();
        let thief = world.spawn((Inventory::new(),));
        let item = world.spawn(());

        // Thief tries to steal from entity with no inventory
        let no_inv_entity = world.spawn(());
        let events = execute_item_theft(&mut world, thief, no_inv_entity, item);
        assert!(events.is_empty());
    }

    // ── ArmorTheftState ───────────────────────────────────────

    #[test]
    fn armor_theft_state_tick() {
        let world = test_world();
        let mut state = ArmorTheftState::new(world.player());

        assert!(!state.is_done());
        for _ in 0..9 {
            assert!(state.tick());
        }
        assert!(!state.tick());
        assert!(state.is_done());
    }

    // ── thief_died_inventory ──────────────────────────────────

    #[test]
    fn thief_died_drops_items() {
        let mut world = test_world();
        let thief = world.spawn((Inventory::new(),));
        let item1 = world.spawn(());
        let item2 = world.spawn(());

        {
            let mut inv = world.get_component_mut::<Inventory>(thief).unwrap();
            inv.add(item1);
            inv.add(item2);
        }

        let events = thief_died_inventory(&mut world, thief);
        // Should have message + 2 ItemDropped events
        assert_eq!(events.len(), 3);

        // Thief inventory should be empty
        let inv = world.get_component::<Inventory>(thief).unwrap();
        assert!(inv.items.is_empty());
    }

    #[test]
    fn thief_died_no_items() {
        let mut world = test_world();
        let thief = world.spawn((Inventory::new(),));

        let events = thief_died_inventory(&mut world, thief);
        assert!(events.is_empty());
    }

    // ── can_steal ─────────────────────────────────────────────

    #[test]
    fn can_steal_with_gold_steal_attack() {
        use nethack_babel_data::*;

        let def = MonsterDef {
            id: MonsterId(0),
            names: MonsterNames { male: "leprechaun".to_string(), female: None },
            symbol: 'l',
            color: Color::Green,
            base_level: 5,
            speed: 15,
            armor_class: 8,
            magic_resistance: 20,
            alignment: 0,
            difficulty: 7,
            attacks: {
                let mut v = arrayvec::ArrayVec::new();
                v.push(AttackDef {
                    method: AttackMethod::Claw,
                    damage_type: DamageType::GoldSteal,
                    dice: DiceExpr { count: 1, sides: 2 },
                });
                v
            },
            geno_flags: GenoFlags::empty(),
            frequency: 4,
            corpse_weight: 60,
            corpse_nutrition: 40,
            sound: MonsterSound::Laugh,
            size: MonsterSize::Tiny,
            resistances: ResistanceSet::empty(),
            conveys: ResistanceSet::empty(),
            flags: MonsterFlags::empty(),
        };

        assert!(can_steal(&def));
    }

    #[test]
    fn can_steal_without_steal_attack() {
        use nethack_babel_data::*;

        let def = MonsterDef {
            id: MonsterId(1),
            names: MonsterNames { male: "gnome".to_string(), female: None },
            symbol: 'G',
            color: Color::Brown,
            base_level: 1,
            speed: 6,
            armor_class: 10,
            magic_resistance: 0,
            alignment: 0,
            difficulty: 1,
            attacks: {
                let mut v = arrayvec::ArrayVec::new();
                v.push(AttackDef {
                    method: AttackMethod::Weapon,
                    damage_type: DamageType::Physical,
                    dice: DiceExpr { count: 1, sides: 6 },
                });
                v
            },
            geno_flags: GenoFlags::empty(),
            frequency: 3,
            corpse_weight: 650,
            corpse_nutrition: 100,
            sound: MonsterSound::Orc,
            size: MonsterSize::Small,
            resistances: ResistanceSet::empty(),
            conveys: ResistanceSet::empty(),
            flags: MonsterFlags::empty(),
        };

        assert!(!can_steal(&def));
    }

    // ── stolen_value ──────────────────────────────────────────

    #[test]
    fn stolen_value_basic() {
        use nethack_babel_data::*;

        let def = ObjectDef {
            id: ObjectTypeId(0),
            name: "long sword".to_string(),
            appearance: None,
            class: ObjectClass::Weapon,
            color: Color::Gray,
            material: Material::Iron,
            weight: 40,
            cost: 15,
            nutrition: 0,
            prob: 50,
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
        };

        assert_eq!(stolen_value(&def, 1), 15);
        assert_eq!(stolen_value(&def, 3), 45);
    }

    // ── remove_worn_item ──────────────────────────────────────

    #[test]
    fn remove_worn_item_not_equipped() {
        let mut world = test_world();
        let player = world.player();
        let item = world.spawn(());

        let events = remove_worn_item(&mut world, player, item);
        assert!(events.is_empty());
    }

    #[test]
    fn remove_worn_item_equipped_weapon() {
        let mut world = test_world();
        let player = world.player();
        let weapon = world.spawn(());

        // Equip the weapon
        {
            let mut equip = world
                .get_component_mut::<EquipmentSlots>(player)
                .unwrap();
            equip.weapon = Some(weapon);
        }

        let events = remove_worn_item(&mut world, player, weapon);
        assert!(!events.is_empty());

        // Weapon should be removed
        let equip = world.get_component::<EquipmentSlots>(player).unwrap();
        assert!(equip.weapon.is_none());
    }
}
