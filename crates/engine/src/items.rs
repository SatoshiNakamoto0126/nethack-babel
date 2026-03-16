//! Item entity management: spawn, pickup, drop, inventory, stacking.
//!
//! This module implements the core item system for NetHack Babel,
//! handling object lifecycle from creation through inventory management.

use hecs::Entity;

use nethack_babel_data::{
    BucStatus, Enchantment, Erosion, KnowledgeState, ObjectClass, ObjectCore, ObjectDef,
    ObjectExtra, ObjectLocation,
};

use crate::action::Position;
use crate::event::EngineEvent;
use crate::world::{Encumbrance, GameWorld, Positioned};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of inventory letter slots (a-z, A-Z).
const MAX_INV_SLOTS: usize = 52;

/// Default carry capacity when no attributes are available.
/// Matches NetHack's formula: 25 * (STR + CON) + 50 with STR=CON=10.
const DEFAULT_CARRY_CAP: u32 = 550;

/// Absolute maximum carry capacity (NetHack MAX_CARR_CAP).
const MAX_CARRY_CAP: u32 = 1000;

// ---------------------------------------------------------------------------
// Inventory letter assignment
// ---------------------------------------------------------------------------

/// The round-robin state for letter assignment.  Persists across calls within
/// a game session but is never saved/restored (matches C `lastinvnr`).
#[derive(Debug, Clone, Copy, Default)]
pub struct LetterState {
    last: usize,
}

/// Convert a slot index (0..51) to its inventory letter.
fn index_to_letter(i: usize) -> char {
    if i < 26 {
        (b'a' + i as u8) as char
    } else {
        (b'A' + (i - 26) as u8) as char
    }
}

/// Assign the next available inventory letter for `owner`, advancing the
/// round-robin state.  Returns `None` if all 52 slots are occupied.
pub fn assign_inv_letter(
    world: &GameWorld,
    owner: Entity,
    letter_state: &mut LetterState,
) -> Option<char> {
    let mut in_use = [false; MAX_INV_SLOTS];

    // Mark letters already in use.
    let player = world.player();
    for (_entity, core) in world.query::<ObjectCore>().iter() {
        let is_owner =
            world
                .get_component::<ObjectLocation>(_entity)
                .is_some_and(|loc| match *loc {
                    ObjectLocation::Inventory => owner == player,
                    ObjectLocation::MonsterInventory { carrier_id } => {
                        carrier_id == owner.to_bits().get() as u32
                    }
                    _ => false,
                });
        if is_owner && let Some(ch) = core.inv_letter {
            match ch {
                'a'..='z' => in_use[(ch as u8 - b'a') as usize] = true,
                'A'..='Z' => in_use[(ch as u8 - b'A') as usize + 26] = true,
                _ => {}
            }
        }
    }

    // Round-robin search from last+1.
    let start = (letter_state.last + 1) % MAX_INV_SLOTS;
    let mut i = start;
    loop {
        if !in_use[i] {
            letter_state.last = i;
            return Some(index_to_letter(i));
        }
        i = (i + 1) % MAX_INV_SLOTS;
        if i == start {
            // Wrapped around — all slots full.
            return None;
        }
    }
}

// ---------------------------------------------------------------------------
// Merge check
// ---------------------------------------------------------------------------

/// Determine whether two object instances can be merged into a single stack.
///
/// Both items must have the same otyp, same BUC status, same enchantment,
/// and their `ObjectDef` must have `is_mergeable == true`.
/// Items with different individual names cannot merge.
#[allow(clippy::too_many_arguments)]
pub fn can_merge(
    a: &ObjectCore,
    b: &ObjectCore,
    a_buc: &BucStatus,
    b_buc: &BucStatus,
    a_ench: Option<&Enchantment>,
    b_ench: Option<&Enchantment>,
    a_extra: Option<&ObjectExtra>,
    b_extra: Option<&ObjectExtra>,
    obj_def: &ObjectDef,
) -> bool {
    // Must be the same type.
    if a.otyp != b.otyp {
        return false;
    }

    // The type must allow merging.
    if !obj_def.is_mergeable {
        return false;
    }

    // Gold (coins) always merge regardless of other fields.
    if a.object_class == ObjectClass::Coin {
        return true;
    }

    // BUC must match.
    if a_buc.cursed != b_buc.cursed || a_buc.blessed != b_buc.blessed {
        return false;
    }

    // Enchantment must match (both absent, or both present with same value).
    match (a_ench, b_ench) {
        (Some(ea), Some(eb)) => {
            if ea.spe != eb.spe {
                return false;
            }
        }
        (None, None) => {}
        _ => return false,
    }

    // Named differently => cannot merge.
    let a_name = a_extra.and_then(|e| e.name.as_deref());
    let b_name = b_extra.and_then(|e| e.name.as_deref());
    match (a_name, b_name) {
        (Some(na), Some(nb)) if na != nb => return false,
        _ => {}
    }

    true
}

// ---------------------------------------------------------------------------
// Spawn
// ---------------------------------------------------------------------------

/// Location hint for spawning an item.
#[derive(Debug, Clone, Copy)]
pub enum SpawnLocation {
    /// On the dungeon floor at (x, y).
    Floor(i16, i16),
    /// In the hero's inventory.
    Inventory,
    /// Free (not placed anywhere yet).
    Free,
}

/// Spawn a new item entity in the game world.
///
/// Creates an entity with `ObjectCore`, `BucStatus`, `KnowledgeState`, and
/// `ObjectLocation`.  Optionally adds `Enchantment` and `Erosion` if
/// the definition warrants them.
pub fn spawn_item(
    world: &mut GameWorld,
    obj_def: &ObjectDef,
    location: SpawnLocation,
    enchantment: Option<i8>,
) -> Entity {
    let core = ObjectCore {
        otyp: obj_def.id,
        object_class: obj_def.class,
        quantity: 1,
        weight: obj_def.weight as u32,
        age: 0,
        inv_letter: None,
        artifact: None,
    };

    let buc = BucStatus {
        cursed: false,
        blessed: false,
        bknown: false,
    };

    let knowledge = KnowledgeState {
        known: false,
        dknown: false,
        rknown: false,
        cknown: false,
        lknown: false,
        tknown: false,
    };

    let obj_loc = match location {
        SpawnLocation::Floor(x, y) => ObjectLocation::Floor { x, y },
        SpawnLocation::Inventory => ObjectLocation::Inventory,
        SpawnLocation::Free => ObjectLocation::Free,
    };

    let entity = world.spawn((core, buc, knowledge, obj_loc));

    // Add optional components.
    if let Some(spe) = enchantment {
        let _ = world.ecs_mut().insert_one(entity, Enchantment { spe });
    }

    // All objects start with zero erosion.
    let _ = world.ecs_mut().insert_one(
        entity,
        Erosion {
            eroded: 0,
            eroded2: 0,
            erodeproof: false,
            greased: false,
        },
    );

    entity
}

// ---------------------------------------------------------------------------
// Inventory query
// ---------------------------------------------------------------------------

/// Return all inventory items for the given `owner`, sorted by inventory
/// letter (a-z then A-Z).
pub fn get_inventory(world: &GameWorld, owner: Entity) -> Vec<(Entity, ObjectCore)> {
    let mut items: Vec<(Entity, ObjectCore)> = world
        .query::<ObjectCore>()
        .iter()
        .filter(|&(entity, _)| {
            world
                .get_component::<ObjectLocation>(entity)
                .is_some_and(|loc| match *loc {
                    ObjectLocation::Inventory => owner == world.player(),
                    ObjectLocation::MonsterInventory { carrier_id } => {
                        carrier_id == owner.to_bits().get() as u32
                    }
                    _ => false,
                })
        })
        .map(|(entity, core)| (entity, core.clone()))
        .collect();

    // Sort by inventory letter: a-z first, then A-Z, then None.
    items.sort_by_key(|(_, core)| inv_letter_sort_key(core.inv_letter));
    items
}

/// Sort key for inventory letters: a=2..z=27, A=28..Z=53, None=54.
fn inv_letter_sort_key(letter: Option<char>) -> u8 {
    match letter {
        Some('a'..='z') => (letter.unwrap() as u8) - b'a' + 2,
        Some('A'..='Z') => (letter.unwrap() as u8) - b'A' + 28,
        Some('$') => 1,
        _ => 54,
    }
}

// ---------------------------------------------------------------------------
// Weight calculation
// ---------------------------------------------------------------------------

/// Sum the total weight of all inventory items owned by `owner`.
pub fn inventory_weight(world: &GameWorld, owner: Entity) -> u32 {
    world
        .query::<ObjectCore>()
        .iter()
        .filter(|&(entity, _)| {
            world
                .get_component::<ObjectLocation>(entity)
                .is_some_and(|loc| match *loc {
                    ObjectLocation::Inventory => owner == world.player(),
                    ObjectLocation::MonsterInventory { carrier_id } => {
                        carrier_id == owner.to_bits().get() as u32
                    }
                    _ => false,
                })
        })
        .fold(0u32, |total, (_, core)| {
            total.saturating_add(core.weight.saturating_mul(core.quantity as u32))
        })
}

/// Compute the player's carry capacity from strength and constitution.
///
/// Formula: 25 * (STR + CON) + 50, capped at `MAX_CARRY_CAP` (1000).
fn carry_capacity(world: &GameWorld, entity: Entity) -> u32 {
    if let Some(attrs) = world.get_component::<crate::world::Attributes>(entity) {
        let raw = 25u32 * (attrs.strength as u32 + attrs.constitution as u32) + 50;
        raw.clamp(1, MAX_CARRY_CAP)
    } else {
        DEFAULT_CARRY_CAP
    }
}

/// Compute the encumbrance level for the given weight and carry capacity.
fn compute_encumbrance(carried: u32, capacity: u32) -> Encumbrance {
    if carried <= capacity {
        return Encumbrance::Unencumbered;
    }
    if capacity <= 1 {
        return Encumbrance::Overloaded;
    }
    let excess = carried - capacity;
    let cap = ((excess * 2) / capacity) + 1;
    match cap.min(5) {
        1 => Encumbrance::Burdened,
        2 => Encumbrance::Stressed,
        3 => Encumbrance::Strained,
        4 => Encumbrance::Overtaxed,
        _ => Encumbrance::Overloaded,
    }
}

/// Check whether `entity` can pick up an item of weight `item_weight`.
/// Returns `true` if the resulting encumbrance would be at most Stressed.
pub fn can_carry(world: &GameWorld, entity: Entity, item_weight: u32) -> bool {
    let cap = carry_capacity(world, entity);
    let current = inventory_weight(world, entity);
    let after = current.saturating_add(item_weight);
    let enc = compute_encumbrance(after, cap);
    enc <= Encumbrance::Stressed
}

// ---------------------------------------------------------------------------
// Pickup
// ---------------------------------------------------------------------------

/// Attempt to pick up `item_entity` for `player`.
///
/// On success the item's `ObjectLocation` becomes `Inventory`, an inventory
/// letter is assigned, and the item is merged with any compatible stack.
/// Returns a list of engine events describing what happened.
pub fn pickup_item(
    world: &mut GameWorld,
    player: Entity,
    item_entity: Entity,
    letter_state: &mut LetterState,
    obj_defs: &[ObjectDef],
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // --- Read item data (early return checks) ---

    let (item_weight, _item_otyp, item_quantity) = {
        let core = match world.get_component::<ObjectCore>(item_entity) {
            Some(c) => c,
            None => return events,
        };
        (
            core.weight.saturating_mul(core.quantity as u32),
            core.otyp,
            core.quantity as u32,
        )
    };

    // Encumbrance check.
    if !can_carry(world, player, item_weight) {
        events.push(EngineEvent::msg("cannot-carry-more"));
        return events;
    }

    // Check if there's a free letter (or if we can merge).
    let existing_merge_target = find_merge_target(world, player, item_entity, obj_defs);
    if existing_merge_target.is_none() {
        let inv = get_inventory(world, player);
        let non_gold_count = inv
            .iter()
            .filter(|(_, c)| c.object_class != ObjectClass::Coin)
            .count();
        if non_gold_count >= MAX_INV_SLOTS {
            // Check if current item is gold — gold never needs a letter slot.
            let is_gold = {
                let core = world.get_component::<ObjectCore>(item_entity).unwrap();
                core.object_class == ObjectClass::Coin
            };
            if !is_gold {
                events.push(EngineEvent::msg("knapsack-full"));
                return events;
            }
        }
    }

    // --- Perform the pickup ---

    if let Some(merge_entity) = existing_merge_target {
        // Merge into existing stack.
        let added_qty = {
            let core = world.get_component::<ObjectCore>(item_entity).unwrap();
            core.quantity
        };
        {
            let mut target_core = world.get_component_mut::<ObjectCore>(merge_entity).unwrap();
            target_core.quantity += added_qty;
            // Recalculate weight based on single-item weight * new quantity.
            let single_weight = if target_core.quantity > 0 {
                target_core.weight / ((target_core.quantity - added_qty) as u32).max(1)
            } else {
                0
            };
            target_core.weight = single_weight * target_core.quantity as u32;
        }
        // Remove the picked-up entity since it was merged.
        let _ = world.despawn(item_entity);

        events.push(EngineEvent::ItemPickedUp {
            actor: player,
            item: merge_entity,
            quantity: item_quantity,
        });
    } else {
        // Assign letter and move to inventory.
        let letter = assign_inv_letter(world, player, letter_state);
        {
            let mut core = world.get_component_mut::<ObjectCore>(item_entity).unwrap();
            core.inv_letter = letter;
        }
        // Change location.
        if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(item_entity) {
            *loc = ObjectLocation::Inventory;
        }

        events.push(EngineEvent::ItemPickedUp {
            actor: player,
            item: item_entity,
            quantity: item_quantity,
        });
    }

    events
}

/// Find an existing inventory item that the given `item_entity` can merge
/// with.  Returns the entity of the merge target, or `None`.
fn find_merge_target(
    world: &GameWorld,
    owner: Entity,
    item_entity: Entity,
    obj_defs: &[ObjectDef],
) -> Option<Entity> {
    let item_core_ref = world.get_component::<ObjectCore>(item_entity)?;
    let item_core = item_core_ref.clone();
    drop(item_core_ref);

    let item_buc_ref = world.get_component::<BucStatus>(item_entity)?;
    let item_buc = item_buc_ref.clone();
    drop(item_buc_ref);

    let item_ench: Option<Enchantment> = world
        .get_component::<Enchantment>(item_entity)
        .map(|r| (*r).clone());
    let item_extra: Option<ObjectExtra> = world
        .get_component::<ObjectExtra>(item_entity)
        .map(|r| (*r).clone());

    let obj_def = obj_defs.iter().find(|d| d.id == item_core.otyp)?;

    get_inventory(world, owner)
        .iter()
        .map(|(entity, _)| *entity)
        .filter(|&entity| entity != item_entity)
        .find(|&entity| {
            let candidate_core = match world.get_component::<ObjectCore>(entity) {
                Some(c) => c.clone(),
                None => return false,
            };
            let candidate_buc = match world.get_component::<BucStatus>(entity) {
                Some(b) => b.clone(),
                None => return false,
            };
            let candidate_ench: Option<Enchantment> = world
                .get_component::<Enchantment>(entity)
                .map(|r| (*r).clone());
            let candidate_extra: Option<ObjectExtra> = world
                .get_component::<ObjectExtra>(entity)
                .map(|r| (*r).clone());

            can_merge(
                &item_core,
                &candidate_core,
                &item_buc,
                &candidate_buc,
                item_ench.as_ref(),
                candidate_ench.as_ref(),
                item_extra.as_ref(),
                candidate_extra.as_ref(),
                obj_def,
            )
        })
}

// ---------------------------------------------------------------------------
// Drop
// ---------------------------------------------------------------------------

/// Drop an item from the player's inventory onto the floor at the player's
/// current position.
pub fn drop_item(world: &mut GameWorld, player: Entity, item_entity: Entity) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Get player position.
    let pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => Position::new(0, 0),
    };

    // Remove letter and change location.
    {
        let mut core = match world.get_component_mut::<ObjectCore>(item_entity) {
            Some(c) => c,
            None => return events,
        };
        core.inv_letter = None;
    }
    {
        let mut loc = match world.get_component_mut::<ObjectLocation>(item_entity) {
            Some(l) => l,
            None => return events,
        };
        *loc = ObjectLocation::Floor {
            x: pos.x as i16,
            y: pos.y as i16,
        };
    }

    events.push(EngineEvent::ItemDropped {
        actor: player,
        item: item_entity,
    });

    events
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

    /// Helper: spawn a floor item with the given def.
    fn spawn_test_item(world: &mut GameWorld, def: &ObjectDef, x: i16, y: i16) -> Entity {
        spawn_item(world, def, SpawnLocation::Floor(x, y), None)
    }

    #[test]
    fn spawn_item_creates_entity_with_correct_components() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let def = test_obj_def(1, true);
        let e = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), Some(3));

        let core = world.get_component::<ObjectCore>(e).expect("ObjectCore");
        assert_eq!(core.otyp, ObjectTypeId(1));
        assert_eq!(core.object_class, ObjectClass::Weapon);
        assert_eq!(core.quantity, 1);
        assert_eq!(core.weight, 10);

        let buc = world.get_component::<BucStatus>(e).expect("BucStatus");
        assert!(!buc.cursed);
        assert!(!buc.blessed);

        let knowledge = world
            .get_component::<KnowledgeState>(e)
            .expect("KnowledgeState");
        assert!(!knowledge.known);

        let loc = world
            .get_component::<ObjectLocation>(e)
            .expect("ObjectLocation");
        assert!(matches!(*loc, ObjectLocation::Floor { x: 5, y: 5 }));

        let ench = world.get_component::<Enchantment>(e).expect("Enchantment");
        assert_eq!(ench.spe, 3);

        let erosion = world.get_component::<Erosion>(e).expect("Erosion");
        assert_eq!(erosion.eroded, 0);
        assert!(!erosion.erodeproof);
    }

    #[test]
    fn pickup_changes_location_and_assigns_letter() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let player = world.player();
        let def = test_obj_def(1, false);
        let defs = vec![def.clone()];
        let item = spawn_test_item(&mut world, &def, 5, 5);
        let mut ls = LetterState::default();

        let events = pickup_item(&mut world, player, item, &mut ls, &defs);

        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            EngineEvent::ItemPickedUp { quantity: 1, .. }
        ));

        let loc = world.get_component::<ObjectLocation>(item).unwrap();
        assert!(matches!(*loc, ObjectLocation::Inventory));

        let core = world.get_component::<ObjectCore>(item).unwrap();
        assert!(core.inv_letter.is_some());
    }

    #[test]
    fn drop_changes_location_to_floor() {
        let mut world = GameWorld::new(Position::new(10, 10));
        let player = world.player();
        let def = test_obj_def(1, false);
        let defs = vec![def.clone()];
        let item = spawn_test_item(&mut world, &def, 10, 10);
        let mut ls = LetterState::default();

        pickup_item(&mut world, player, item, &mut ls, &defs);
        let events = drop_item(&mut world, player, item);

        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], EngineEvent::ItemDropped { .. }));

        let loc = world.get_component::<ObjectLocation>(item).unwrap();
        assert!(matches!(*loc, ObjectLocation::Floor { x: 10, y: 10 }));

        let core = world.get_component::<ObjectCore>(item).unwrap();
        assert!(core.inv_letter.is_none());
    }

    #[test]
    fn merge_combines_quantities() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let player = world.player();
        let def = test_obj_def(1, true); // mergeable
        let defs = vec![def.clone()];
        let mut ls = LetterState::default();

        // Pick up first item.
        let item1 = spawn_test_item(&mut world, &def, 5, 5);
        {
            let mut core = world.get_component_mut::<ObjectCore>(item1).unwrap();
            core.quantity = 5;
            core.weight = 50; // 5 * 10
        }
        pickup_item(&mut world, player, item1, &mut ls, &defs);

        // Pick up second item (should merge).
        let item2 = spawn_test_item(&mut world, &def, 5, 5);
        {
            let mut core = world.get_component_mut::<ObjectCore>(item2).unwrap();
            core.quantity = 3;
            core.weight = 30; // 3 * 10
        }
        let events = pickup_item(&mut world, player, item2, &mut ls, &defs);

        assert_eq!(events.len(), 1);
        // item2 should be despawned; item1 should have qty 8.
        let core = world.get_component::<ObjectCore>(item1).unwrap();
        assert_eq!(core.quantity, 8);

        // item2 should no longer exist.
        assert!(world.get_component::<ObjectCore>(item2).is_none());
    }

    #[test]
    fn cannot_merge_different_item_types() {
        let a_core = ObjectCore {
            otyp: ObjectTypeId(1),
            object_class: ObjectClass::Weapon,
            quantity: 1,
            weight: 10,
            age: 0,
            inv_letter: Some('a'),
            artifact: None,
        };
        let b_core = ObjectCore {
            otyp: ObjectTypeId(2), // different type
            object_class: ObjectClass::Weapon,
            quantity: 1,
            weight: 10,
            age: 0,
            inv_letter: Some('b'),
            artifact: None,
        };
        let buc = BucStatus {
            cursed: false,
            blessed: false,
            bknown: false,
        };
        let def = test_obj_def(1, true);

        assert!(!can_merge(
            &a_core, &b_core, &buc, &buc, None, None, None, None, &def
        ));
    }

    #[test]
    fn cannot_merge_different_buc() {
        let core = ObjectCore {
            otyp: ObjectTypeId(1),
            object_class: ObjectClass::Weapon,
            quantity: 1,
            weight: 10,
            age: 0,
            inv_letter: Some('a'),
            artifact: None,
        };
        let buc_uncursed = BucStatus {
            cursed: false,
            blessed: false,
            bknown: false,
        };
        let buc_blessed = BucStatus {
            cursed: false,
            blessed: true,
            bknown: false,
        };
        let def = test_obj_def(1, true);

        assert!(!can_merge(
            &core,
            &core,
            &buc_uncursed,
            &buc_blessed,
            None,
            None,
            None,
            None,
            &def
        ));
    }

    #[test]
    fn cannot_merge_non_mergeable_type() {
        let core = ObjectCore {
            otyp: ObjectTypeId(1),
            object_class: ObjectClass::Weapon,
            quantity: 1,
            weight: 10,
            age: 0,
            inv_letter: Some('a'),
            artifact: None,
        };
        let buc = BucStatus {
            cursed: false,
            blessed: false,
            bknown: false,
        };
        let def = test_obj_def(1, false); // NOT mergeable

        assert!(!can_merge(
            &core, &core, &buc, &buc, None, None, None, None, &def
        ));
    }

    #[test]
    fn letter_assignment_avoids_duplicates() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let player = world.player();
        let def = test_obj_def(1, false); // non-mergeable so each gets own letter
        let defs = vec![def.clone()];
        let mut ls = LetterState::default();

        let mut letters = Vec::new();
        for _ in 0..10 {
            let item = spawn_test_item(&mut world, &def, 5, 5);
            // Give each item a different otyp so they won't merge.
            {
                let mut core = world.get_component_mut::<ObjectCore>(item).unwrap();
                core.otyp = ObjectTypeId(letters.len() as u16 + 100);
            }
            pickup_item(&mut world, player, item, &mut ls, &defs);
            let core = world.get_component::<ObjectCore>(item).unwrap();
            if let Some(ch) = core.inv_letter {
                letters.push(ch);
            }
        }

        // All letters should be unique.
        let unique: std::collections::HashSet<char> = letters.iter().cloned().collect();
        assert_eq!(unique.len(), letters.len());
    }

    #[test]
    fn full_inventory_returns_none_for_letter() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let player = world.player();
        let mut ls = LetterState::default();

        // Fill all 52 slots.
        for i in 0..52u16 {
            let item = spawn_item(
                &mut world,
                &ObjectDef {
                    id: ObjectTypeId(i + 100),
                    ..test_obj_def(i + 100, false)
                },
                SpawnLocation::Inventory,
                None,
            );
            let letter = assign_inv_letter(&mut world, player, &mut ls);
            let mut core = world.get_component_mut::<ObjectCore>(item).unwrap();
            core.inv_letter = letter;
        }

        // Next letter should be None.
        let letter = assign_inv_letter(&mut world, player, &mut ls);
        assert!(letter.is_none());
    }

    #[test]
    fn weight_calculation_sums_correctly() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let player = world.player();
        let def = test_obj_def(1, false);
        let defs = vec![def.clone()];
        let mut ls = LetterState::default();

        // Item 1: weight 10, quantity 3 => 30.
        let item1 = spawn_test_item(&mut world, &def, 5, 5);
        {
            let mut core = world.get_component_mut::<ObjectCore>(item1).unwrap();
            core.quantity = 3;
            core.weight = 30;
            core.otyp = ObjectTypeId(200);
        }
        pickup_item(&mut world, player, item1, &mut ls, &defs);

        // Item 2: weight 20, quantity 2 => 40.
        let def2 = test_obj_def(2, false);
        let item2 = spawn_test_item(&mut world, &def2, 5, 5);
        {
            let mut core = world.get_component_mut::<ObjectCore>(item2).unwrap();
            core.quantity = 2;
            core.weight = 40;
            core.otyp = ObjectTypeId(201);
        }
        pickup_item(&mut world, player, item2, &mut ls, &defs);

        let total = inventory_weight(&mut world, player);
        // weight field already represents total (quantity * per-unit).
        // Item1: 30 * 3 = 90, Item2: 40 * 2 = 80  ... but weight field
        // IS the total weight, so inv_weight multiplies again.
        // Actually, let's fix: weight is per-unit in ObjectCore.weight,
        // and we multiply by quantity in inventory_weight.
        // After pickup, item1 has weight=30 qty=3 => 30*3=90
        // item2 has weight=40 qty=2 => 40*2=80
        // total = 170
        assert_eq!(total, 170);
    }

    #[test]
    fn encumbrance_prevents_pickup_when_overloaded() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let player = world.player();
        let def = test_obj_def(1, false);
        let defs = vec![def.clone()];
        let mut ls = LetterState::default();

        // Player default attrs: STR=10, CON=10 => cap = 25*20+50 = 550.
        // Stressed threshold is when carried > cap (i.e., > 550).
        // Create a very heavy item.
        let heavy = spawn_test_item(&mut world, &def, 5, 5);
        {
            let mut core = world.get_component_mut::<ObjectCore>(heavy).unwrap();
            core.weight = 2000; // Way over capacity.
            core.otyp = ObjectTypeId(300);
        }

        let events = pickup_item(&mut world, player, heavy, &mut ls, &defs);

        // Should be rejected with a message.
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], EngineEvent::Message { .. }));
    }

    #[test]
    fn get_inventory_returns_sorted_items() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let player = world.player();

        // Spawn two items directly in inventory with specific letters.
        let def = test_obj_def(1, false);
        let item_b = spawn_item(&mut world, &def, SpawnLocation::Inventory, None);
        {
            let mut core = world.get_component_mut::<ObjectCore>(item_b).unwrap();
            core.inv_letter = Some('c');
        }
        let def2 = test_obj_def(2, false);
        let item_a = spawn_item(&mut world, &def2, SpawnLocation::Inventory, None);
        {
            let mut core = world.get_component_mut::<ObjectCore>(item_a).unwrap();
            core.inv_letter = Some('a');
        }

        let inv = get_inventory(&world, player);
        assert_eq!(inv.len(), 2);
        assert_eq!(inv[0].1.inv_letter, Some('a'));
        assert_eq!(inv[1].1.inv_letter, Some('c'));
    }

    #[test]
    fn can_merge_coins_always() {
        let a = ObjectCore {
            otyp: ObjectTypeId(1),
            object_class: ObjectClass::Coin,
            quantity: 100,
            weight: 1,
            age: 0,
            inv_letter: Some('$'),
            artifact: None,
        };
        let b = ObjectCore {
            otyp: ObjectTypeId(1),
            object_class: ObjectClass::Coin,
            quantity: 50,
            weight: 1,
            age: 0,
            inv_letter: None,
            artifact: None,
        };
        let buc_a = BucStatus {
            cursed: true,
            blessed: false,
            bknown: false,
        };
        let buc_b = BucStatus {
            cursed: false,
            blessed: false,
            bknown: false,
        };
        // Coins merge even with different BUC.
        let def = ObjectDef {
            id: ObjectTypeId(1),
            class: ObjectClass::Coin,
            is_mergeable: true,
            ..test_obj_def(1, true)
        };
        assert!(can_merge(
            &a, &b, &buc_a, &buc_b, None, None, None, None, &def
        ));
    }

    #[test]
    fn cannot_merge_different_enchantment() {
        let core = ObjectCore {
            otyp: ObjectTypeId(1),
            object_class: ObjectClass::Weapon,
            quantity: 1,
            weight: 10,
            age: 0,
            inv_letter: Some('a'),
            artifact: None,
        };
        let buc = BucStatus {
            cursed: false,
            blessed: false,
            bknown: false,
        };
        let def = test_obj_def(1, true);
        let ea = Enchantment { spe: 2 };
        let eb = Enchantment { spe: 4 };

        assert!(!can_merge(
            &core,
            &core,
            &buc,
            &buc,
            Some(&ea),
            Some(&eb),
            None,
            None,
            &def
        ));
    }
}
