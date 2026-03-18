//! Equipment system: equip, unequip, query worn/wielded items, AC calculation,
//! magic cancellation, layering constraints, attribute effects, donning delay,
//! conferred properties, and erosion effects.
//!
//! Implements the full equipment model from NetHack (weapon, off-hand,
//! body armor, cloak, helmet, gloves, boots, shield, shirt, two rings, amulet).
//! Handles cursed item restrictions, two-handed weapon / shield conflicts,
//! layering order for suit/cloak/shirt, and comprehensive AC/MC computation.

use hecs::Entity;
use rand::Rng;

use nethack_babel_data::{
    ArmorCategory, BucStatus, Enchantment, Erosion, Material, ObjectClass, ObjectCore, ObjectDef,
};

use crate::combat::WeaponStats;
use crate::event::EngineEvent;
use crate::world::GameWorld;

// ---------------------------------------------------------------------------
// Equipment slot enum
// ---------------------------------------------------------------------------

/// Logical equipment slot for the player character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EquipSlot {
    Weapon,
    OffHand,
    Helmet,
    Cloak,
    BodyArmor,
    Shield,
    Gloves,
    Boots,
    Shirt,
    RingLeft,
    RingRight,
    Amulet,
}

/// All armor slots (for iteration in AC/MC calculations).
pub const ARMOR_SLOTS: &[EquipSlot] = &[
    EquipSlot::Helmet,
    EquipSlot::Cloak,
    EquipSlot::BodyArmor,
    EquipSlot::Shield,
    EquipSlot::Gloves,
    EquipSlot::Boots,
    EquipSlot::Shirt,
];

/// Take-off order for the "take off all" command (`A`).
/// Outer layers first, then inner layers.
pub const TAKEOFF_ORDER: &[EquipSlot] = &[
    EquipSlot::OffHand, // blindfold/towel in off-hand
    EquipSlot::Weapon,
    EquipSlot::Shield,
    EquipSlot::Gloves,
    EquipSlot::RingLeft,
    EquipSlot::RingRight,
    EquipSlot::Cloak,
    EquipSlot::Helmet,
    EquipSlot::Amulet,
    EquipSlot::BodyArmor,
    EquipSlot::Shirt,
    EquipSlot::Boots,
];

// ---------------------------------------------------------------------------
// EquipmentSlots component
// ---------------------------------------------------------------------------

/// Component tracking which item entities occupy each equipment slot.
/// Attached to the player entity.
#[derive(Debug, Clone, Default)]
pub struct EquipmentSlots {
    pub weapon: Option<Entity>,
    pub off_hand: Option<Entity>,
    pub helmet: Option<Entity>,
    pub cloak: Option<Entity>,
    pub body_armor: Option<Entity>,
    pub shield: Option<Entity>,
    pub gloves: Option<Entity>,
    pub boots: Option<Entity>,
    pub shirt: Option<Entity>,
    pub ring_left: Option<Entity>,
    pub ring_right: Option<Entity>,
    pub amulet: Option<Entity>,
}

impl EquipmentSlots {
    /// Get the item in a given slot.
    pub fn get(&self, slot: EquipSlot) -> Option<Entity> {
        match slot {
            EquipSlot::Weapon => self.weapon,
            EquipSlot::OffHand => self.off_hand,
            EquipSlot::Helmet => self.helmet,
            EquipSlot::Cloak => self.cloak,
            EquipSlot::BodyArmor => self.body_armor,
            EquipSlot::Shield => self.shield,
            EquipSlot::Gloves => self.gloves,
            EquipSlot::Boots => self.boots,
            EquipSlot::Shirt => self.shirt,
            EquipSlot::RingLeft => self.ring_left,
            EquipSlot::RingRight => self.ring_right,
            EquipSlot::Amulet => self.amulet,
        }
    }

    /// Set the item in a given slot.
    pub fn set(&mut self, slot: EquipSlot, entity: Option<Entity>) {
        match slot {
            EquipSlot::Weapon => self.weapon = entity,
            EquipSlot::OffHand => self.off_hand = entity,
            EquipSlot::Helmet => self.helmet = entity,
            EquipSlot::Cloak => self.cloak = entity,
            EquipSlot::BodyArmor => self.body_armor = entity,
            EquipSlot::Shield => self.shield = entity,
            EquipSlot::Gloves => self.gloves = entity,
            EquipSlot::Boots => self.boots = entity,
            EquipSlot::Shirt => self.shirt = entity,
            EquipSlot::RingLeft => self.ring_left = entity,
            EquipSlot::RingRight => self.ring_right = entity,
            EquipSlot::Amulet => self.amulet = entity,
        }
    }

    /// Find which slot (if any) a given item entity occupies.
    pub fn find_slot(&self, item: Entity) -> Option<EquipSlot> {
        if self.weapon == Some(item) {
            return Some(EquipSlot::Weapon);
        }
        if self.off_hand == Some(item) {
            return Some(EquipSlot::OffHand);
        }
        if self.helmet == Some(item) {
            return Some(EquipSlot::Helmet);
        }
        if self.cloak == Some(item) {
            return Some(EquipSlot::Cloak);
        }
        if self.body_armor == Some(item) {
            return Some(EquipSlot::BodyArmor);
        }
        if self.shield == Some(item) {
            return Some(EquipSlot::Shield);
        }
        if self.gloves == Some(item) {
            return Some(EquipSlot::Gloves);
        }
        if self.boots == Some(item) {
            return Some(EquipSlot::Boots);
        }
        if self.shirt == Some(item) {
            return Some(EquipSlot::Shirt);
        }
        if self.ring_left == Some(item) {
            return Some(EquipSlot::RingLeft);
        }
        if self.ring_right == Some(item) {
            return Some(EquipSlot::RingRight);
        }
        if self.amulet == Some(item) {
            return Some(EquipSlot::Amulet);
        }
        None
    }

    /// Returns all occupied slots and their entities.
    pub fn all_worn(&self) -> Vec<(EquipSlot, Entity)> {
        let mut result = Vec::new();
        let slots = [
            (EquipSlot::Weapon, self.weapon),
            (EquipSlot::OffHand, self.off_hand),
            (EquipSlot::Helmet, self.helmet),
            (EquipSlot::Cloak, self.cloak),
            (EquipSlot::BodyArmor, self.body_armor),
            (EquipSlot::Shield, self.shield),
            (EquipSlot::Gloves, self.gloves),
            (EquipSlot::Boots, self.boots),
            (EquipSlot::Shirt, self.shirt),
            (EquipSlot::RingLeft, self.ring_left),
            (EquipSlot::RingRight, self.ring_right),
            (EquipSlot::Amulet, self.amulet),
        ];
        for (slot, entity) in slots {
            if let Some(e) = entity {
                result.push((slot, e));
            }
        }
        result
    }

    /// Check if any equipment is worn at all.
    pub fn is_naked(&self) -> bool {
        self.all_worn().is_empty()
    }

    /// Count how many armor slots are occupied.
    pub fn armor_count(&self) -> usize {
        ARMOR_SLOTS
            .iter()
            .filter(|&&s| self.get(s).is_some())
            .count()
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur during equip/unequip operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EquipError {
    /// Item entity does not exist or lacks ObjectCore.
    InvalidItem,
    /// Item type cannot be equipped in the given slot.
    WrongSlot,
    /// The target slot is already occupied (and the occupant is not the same item).
    SlotOccupied,
    /// A cursed item in the slot prevents removal.
    CursedCannotRemove,
    /// Two-handed weapon blocks equipping a shield (or vice versa).
    TwoHandedConflict,
    /// The player has no EquipmentSlots component.
    NoEquipmentComponent,
    /// The item is not currently equipped.
    NotEquipped,
    /// Layering constraint: cloak must be removed first.
    CloakBlocksSuit,
    /// Layering constraint: suit/cloak must be removed first.
    SuitBlocksShirt,
    /// Welded weapon prevents removing gloves.
    WeldedWeapon,
    /// Cursed gloves prevent removing ring.
    CursedGlovesBlockRing,
}

// ---------------------------------------------------------------------------
// Slot determination
// ---------------------------------------------------------------------------

/// Determine which equipment slot an item should go in, based on its
/// ObjectDef.  Returns `None` if the item class cannot be equipped.
pub fn slot_for_item(def: &ObjectDef) -> Option<EquipSlot> {
    match def.class {
        ObjectClass::Weapon => Some(EquipSlot::Weapon),
        ObjectClass::Armor => {
            if let Some(ref armor) = def.armor {
                Some(match armor.category {
                    ArmorCategory::Suit => EquipSlot::BodyArmor,
                    ArmorCategory::Shield => EquipSlot::Shield,
                    ArmorCategory::Helm => EquipSlot::Helmet,
                    ArmorCategory::Gloves => EquipSlot::Gloves,
                    ArmorCategory::Boots => EquipSlot::Boots,
                    ArmorCategory::Cloak => EquipSlot::Cloak,
                    ArmorCategory::Shirt => EquipSlot::Shirt,
                })
            } else {
                // Armor class without ArmorInfo — fallback to body armor.
                Some(EquipSlot::BodyArmor)
            }
        }
        ObjectClass::Ring => Some(EquipSlot::RingLeft),
        ObjectClass::Amulet => Some(EquipSlot::Amulet),
        // Weapon-tools (e.g. pick-axe, unicorn horn) can be wielded.
        ObjectClass::Tool => {
            if def.weapon.is_some() {
                Some(EquipSlot::Weapon)
            } else {
                None
            }
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Layering checks
// ---------------------------------------------------------------------------

/// Check layering constraints for wearing armor.
///
/// Returns `None` if the layering is acceptable, or an `EquipError` if
/// an outer layer blocks the operation.
pub fn check_layering(
    world: &GameWorld,
    player: Entity,
    target_slot: EquipSlot,
) -> Option<EquipError> {
    let equip = world.get_component::<EquipmentSlots>(player)?;

    match target_slot {
        EquipSlot::Shirt if equip.body_armor.is_some() || equip.cloak.is_some() => {
            // Shirt is innermost: cannot put on if suit or cloak is worn.
            return Some(EquipError::SuitBlocksShirt);
        }
        EquipSlot::BodyArmor if equip.cloak.is_some() => {
            // Suit: cannot put on if cloak is worn.
            return Some(EquipError::CloakBlocksSuit);
        }
        _ => {}
    }
    None
}

/// Check layering constraints for removing armor.
///
/// To remove an inner layer, outer layers must be removable:
/// - Suit: cloak must be absent or removable (not cursed).
/// - Shirt: cloak and suit must be absent or removable.
/// - Gloves: weapon must not be welded.
/// - Ring: gloves must not be cursed.
pub fn check_removal_layering(
    world: &GameWorld,
    player: Entity,
    target_slot: EquipSlot,
) -> Option<EquipError> {
    let equip = world.get_component::<EquipmentSlots>(player)?;

    match target_slot {
        EquipSlot::BodyArmor => {
            // Suit removal: cloak must not be cursed.
            if let Some(cloak) = equip.cloak
                && is_item_cursed(world, cloak)
            {
                return Some(EquipError::CursedCannotRemove);
            }
        }
        EquipSlot::Shirt => {
            // Shirt removal: cloak and suit must not be cursed.
            if let Some(cloak) = equip.cloak
                && is_item_cursed(world, cloak)
            {
                return Some(EquipError::CursedCannotRemove);
            }
            if let Some(suit) = equip.body_armor
                && is_item_cursed(world, suit)
            {
                return Some(EquipError::CursedCannotRemove);
            }
        }
        EquipSlot::Gloves => {
            // Gloves removal: weapon must not be welded.
            if let Some(weapon) = equip.weapon
                && is_weapon_welded(world, weapon)
            {
                return Some(EquipError::WeldedWeapon);
            }
        }
        EquipSlot::RingLeft | EquipSlot::RingRight => {
            // Ring removal: gloves must not be cursed.
            if let Some(gloves) = equip.gloves
                && is_item_cursed(world, gloves)
            {
                return Some(EquipError::CursedGlovesBlockRing);
            }
        }
        _ => {}
    }
    None
}

/// Check if an item is cursed.
fn is_item_cursed(world: &GameWorld, item: Entity) -> bool {
    world
        .get_component::<BucStatus>(item)
        .is_some_and(|buc| buc.cursed)
}

/// Check if a wielded weapon is welded (cursed + wielded).
fn is_weapon_welded(world: &GameWorld, weapon: Entity) -> bool {
    is_item_cursed(world, weapon)
}

// ---------------------------------------------------------------------------
// ARM_BONUS — the core AC contribution formula
// ---------------------------------------------------------------------------

/// Compute the AC bonus for a single armor piece.
///
/// `ARM_BONUS(obj) = a_ac + spe - min(greatest_erosion, a_ac)`
///
/// Where:
/// - `a_ac` = base armor bonus from ObjectDef
/// - `spe` = enchantment level
/// - `greatest_erosion` = max(eroded, eroded2)
///
/// Erosion penalty is capped at `a_ac` (never makes base contribution negative).
/// Enchantment is NOT affected by erosion.
pub fn arm_bonus(a_ac: i32, spe: i32, eroded: u8, eroded2: u8) -> i32 {
    let greatest_erosion = eroded.max(eroded2) as i32;
    let erosion_penalty = greatest_erosion.min(a_ac);
    a_ac + spe - erosion_penalty
}

// ---------------------------------------------------------------------------
// Equip
// ---------------------------------------------------------------------------

/// Equip an item into a slot on the player.
///
/// Validates slot compatibility, checks for two-hand conflicts and layering,
/// and emits `ItemWielded` or `ItemWorn` events.
///
/// If the target slot is occupied by a non-cursed item, that item is
/// automatically unequipped first.
pub fn equip_item(
    world: &mut GameWorld,
    player: Entity,
    item: Entity,
    slot: EquipSlot,
    obj_defs: &[ObjectDef],
) -> Result<Vec<EngineEvent>, EquipError> {
    let mut events = Vec::new();

    // Validate item exists and has an ObjectCore.
    let (_item_class, item_otyp) = {
        let core = world
            .get_component::<ObjectCore>(item)
            .ok_or(EquipError::InvalidItem)?;
        (core.object_class, core.otyp)
    };

    // Look up the object definition.
    let obj_def = obj_defs
        .iter()
        .find(|d| d.id == item_otyp)
        .ok_or(EquipError::InvalidItem)?;

    // Validate the slot is compatible with the item.
    let natural_slot = slot_for_item(obj_def).ok_or(EquipError::WrongSlot)?;
    // Allow Ring to go into either RingLeft or RingRight.
    let slot_ok = match (natural_slot, slot) {
        (EquipSlot::RingLeft, EquipSlot::RingRight) => true,
        (EquipSlot::RingRight, EquipSlot::RingLeft) => true,
        (a, b) if a == b => true,
        _ => false,
    };
    if !slot_ok {
        return Err(EquipError::WrongSlot);
    }

    // Get or create EquipmentSlots component.
    let has_equip = world.get_component::<EquipmentSlots>(player).is_some();
    if !has_equip {
        return Err(EquipError::NoEquipmentComponent);
    }

    // Check layering constraints for armor.
    if matches!(slot, EquipSlot::Shirt | EquipSlot::BodyArmor)
        && let Some(err) = check_layering(world, player, slot)
    {
        return Err(err);
    }

    // Check for two-handed weapon / shield conflict.
    let is_bimanual = obj_def.is_bimanual;
    if slot == EquipSlot::Weapon && is_bimanual {
        // Two-handed weapon: must unequip shield first.
        let shield_item = world
            .get_component::<EquipmentSlots>(player)
            .unwrap()
            .shield;
        if let Some(shield_ent) = shield_item {
            // Check if shield is cursed.
            let shield_cursed = world
                .get_component::<BucStatus>(shield_ent)
                .is_some_and(|buc| buc.cursed);
            if shield_cursed {
                return Err(EquipError::TwoHandedConflict);
            }
            // Auto-unequip the shield.
            let unequip_events = unequip_slot(world, player, EquipSlot::Shield)?;
            events.extend(unequip_events);
        }
    }
    if slot == EquipSlot::Shield {
        // Equipping a shield: check if current weapon is two-handed.
        let weapon_item = world
            .get_component::<EquipmentSlots>(player)
            .unwrap()
            .weapon;
        if let Some(wep_ent) = weapon_item {
            let wep_otyp = world.get_component::<ObjectCore>(wep_ent).map(|c| c.otyp);
            if let Some(otyp) = wep_otyp {
                let wep_bimanual = obj_defs
                    .iter()
                    .find(|d| d.id == otyp)
                    .is_some_and(|d| d.is_bimanual);
                if wep_bimanual {
                    return Err(EquipError::TwoHandedConflict);
                }
            }
        }
    }

    // If slot is currently occupied, unequip the existing item first.
    let current_occupant = world
        .get_component::<EquipmentSlots>(player)
        .unwrap()
        .get(slot);
    if let Some(occupant) = current_occupant {
        if occupant == item {
            // Already equipped in this slot — nothing to do.
            return Ok(events);
        }
        // Check if occupant is cursed.
        let occupant_cursed = world
            .get_component::<BucStatus>(occupant)
            .is_some_and(|buc| buc.cursed);
        if occupant_cursed {
            return Err(EquipError::CursedCannotRemove);
        }
        let unequip_events = unequip_slot(world, player, slot)?;
        events.extend(unequip_events);
    }

    // Place item in slot.
    {
        let mut equip = world.get_component_mut::<EquipmentSlots>(player).unwrap();
        equip.set(slot, Some(item));
    }

    // Emit event.
    match slot {
        EquipSlot::Weapon | EquipSlot::OffHand => {
            events.push(EngineEvent::ItemWielded {
                actor: player,
                item,
            });
        }
        _ => {
            events.push(EngineEvent::ItemWorn {
                actor: player,
                item,
            });
        }
    }

    // Recalculate AC.
    let new_ac = calculate_ac(world, player, obj_defs);
    if let Some(mut ac) = world.get_component_mut::<crate::world::ArmorClass>(player) {
        ac.0 = new_ac;
    }

    Ok(events)
}

// ---------------------------------------------------------------------------
// Unequip
// ---------------------------------------------------------------------------

/// Unequip the item in the given slot.
///
/// Returns `EquipError::CursedCannotRemove` if the item is cursed.
/// Checks layering constraints for inner armor layers.
/// Emits `ItemRemoved` on success.
pub fn unequip_slot(
    world: &mut GameWorld,
    player: Entity,
    slot: EquipSlot,
) -> Result<Vec<EngineEvent>, EquipError> {
    let mut events = Vec::new();

    let item = {
        let equip = world
            .get_component::<EquipmentSlots>(player)
            .ok_or(EquipError::NoEquipmentComponent)?;
        equip.get(slot).ok_or(EquipError::NotEquipped)?
    };

    // Cursed items cannot be removed.
    let is_cursed = world
        .get_component::<BucStatus>(item)
        .is_some_and(|buc| buc.cursed);
    if is_cursed {
        return Err(EquipError::CursedCannotRemove);
    }

    // Check layering constraints for removal.
    if let Some(err) = check_removal_layering(world, player, slot) {
        return Err(err);
    }

    // Clear the slot.
    {
        let mut equip = world.get_component_mut::<EquipmentSlots>(player).unwrap();
        equip.set(slot, None);
    }

    events.push(EngineEvent::ItemRemoved {
        actor: player,
        item,
    });

    Ok(events)
}

/// Unequip an item by entity (finds which slot it is in).
pub fn unequip_item(
    world: &mut GameWorld,
    player: Entity,
    item: Entity,
) -> Result<Vec<EngineEvent>, EquipError> {
    let slot = {
        let equip = world
            .get_component::<EquipmentSlots>(player)
            .ok_or(EquipError::NoEquipmentComponent)?;
        equip.find_slot(item).ok_or(EquipError::NotEquipped)?
    };
    unequip_slot(world, player, slot)
}

// ---------------------------------------------------------------------------
// Donning delay
// ---------------------------------------------------------------------------

/// Turn cost for putting on or taking off an armor piece.
///
/// Returns the multi-turn delay from the object definition's `use_delay`.
pub fn donning_delay(obj_def: &ObjectDef) -> u32 {
    obj_def.use_delay as u32
}

/// Turn cost for removing armor. Same as donning for most items,
/// but some items take fewer turns to remove than to put on.
pub fn doffing_delay(obj_def: &ObjectDef) -> u32 {
    // In C NetHack, removal delay is the same as donning delay.
    obj_def.use_delay as u32
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

/// Get the currently equipped weapon entity for the given player.
pub fn get_equipped_weapon(world: &GameWorld, player: Entity) -> Option<Entity> {
    world
        .get_component::<EquipmentSlots>(player)
        .and_then(|equip| equip.weapon)
}

/// Extract `WeaponStats` for the currently wielded weapon.
///
/// Returns `None` if unarmed or if the weapon entity lacks required
/// components.
pub fn get_weapon_stats(
    world: &GameWorld,
    player: Entity,
    obj_defs: &[ObjectDef],
) -> Option<WeaponStats> {
    let weapon_entity = get_equipped_weapon(world, player)?;

    let core = world.get_component::<ObjectCore>(weapon_entity)?;
    let obj_def = crate::items::object_def_for_core(obj_defs, &core)?;
    let weapon_info = obj_def.weapon.as_ref()?;

    let spe = world
        .get_component::<Enchantment>(weapon_entity)
        .map(|e| e.spe as i32)
        .unwrap_or(0);

    let blessed = world
        .get_component::<BucStatus>(weapon_entity)
        .is_some_and(|buc| buc.blessed);

    let is_silver = obj_def.material == Material::Silver;

    let erosion = world
        .get_component::<Erosion>(weapon_entity)
        .map(|e| e.eroded.max(e.eroded2) as i32)
        .unwrap_or(0);

    Some(WeaponStats {
        spe,
        hit_bonus: weapon_info.hit_bonus as i32,
        damage_small: weapon_info.damage_small as i32,
        damage_large: weapon_info.damage_large as i32,
        is_weapon: true,
        blessed,
        is_silver,
        greatest_erosion: erosion,
    })
}

/// Calculate the player's AC using the full NetHack formula.
///
/// ```text
/// uac = base_ac
///     - ARM_BONUS(each armor piece)
///     - ring_of_protection.spe (each ring, if RIN_PROTECTION)
///     - 2 (if amulet of guarding)
///     - spell_protection
///     - divine_protection
/// Clamped to [-99, 99].
/// ```
///
/// `base_ac` is 10 for a normal human (from the player entity's ArmorClass
/// default). Each worn armor piece contributes via `ARM_BONUS`.
pub fn calculate_ac(world: &GameWorld, player: Entity, obj_defs: &[ObjectDef]) -> i32 {
    let base_ac = 10;

    let equip = match world.get_component::<EquipmentSlots>(player) {
        Some(e) => e,
        None => return base_ac,
    };

    let armor_slots = [
        equip.helmet,
        equip.cloak,
        equip.body_armor,
        equip.shield,
        equip.gloves,
        equip.boots,
        equip.shirt,
    ];

    let mut ac = base_ac;

    // Armor contributions via ARM_BONUS.
    for slot_item in armor_slots.iter().flatten() {
        let item = *slot_item;
        if let Some(core) = world.get_component::<ObjectCore>(item)
            && let Some(def) = crate::items::object_def_for_core(obj_defs, &core)
            && let Some(ref armor_info) = def.armor
        {
            let a_ac = -(armor_info.ac_bonus as i32); // ac_bonus is negative in data
            let spe = world
                .get_component::<Enchantment>(item)
                .map(|e| e.spe as i32)
                .unwrap_or(0);
            let (eroded, eroded2) = world
                .get_component::<Erosion>(item)
                .map(|e| (e.eroded, e.eroded2))
                .unwrap_or((0, 0));
            ac -= arm_bonus(a_ac, spe, eroded, eroded2);
        }
    }

    // Ring of protection contributions (by enchantment value).
    // We check if the ring's conferred_property indicates protection.
    for ring in [equip.ring_left, equip.ring_right].into_iter().flatten() {
        if let Some(core) = world.get_component::<ObjectCore>(ring)
            && let Some(def) = crate::items::object_def_for_core(obj_defs, &core)
            && def.conferred_property == Some(nethack_babel_data::Property::Protection)
        {
            let spe = world
                .get_component::<Enchantment>(ring)
                .map(|e| e.spe as i32)
                .unwrap_or(0);
            ac -= spe;
        }
    }

    // Amulet of guarding: flat -2 AC.
    if let Some(amulet) = equip.amulet
        && let Some(core) = world.get_component::<ObjectCore>(amulet)
        && let Some(def) = crate::items::object_def_for_core(obj_defs, &core)
        && def.name == "amulet of guarding"
    {
        ac -= 2;
    }

    // Clamp to [-99, 99].
    ac.clamp(-99, 99)
}

// ---------------------------------------------------------------------------
// Magic Cancellation (MC)
// ---------------------------------------------------------------------------

/// Calculate the Magic Cancellation level for an entity.
///
/// MC is the **highest** `magic_cancel` among all worn armor pieces,
/// not their sum. Additional bonuses from protection items/spells.
///
/// MC range is 0..3.
pub fn magic_cancellation(world: &GameWorld, player: Entity, obj_defs: &[ObjectDef]) -> u8 {
    let equip = match world.get_component::<EquipmentSlots>(player) {
        Some(e) => e,
        None => return 0,
    };

    let mut mc: i8 = 0;

    // Check all worn armor for the highest magic_cancel value.
    for (slot, item) in equip.all_worn() {
        // Only armor contributes to MC.
        if !matches!(
            slot,
            EquipSlot::Helmet
                | EquipSlot::Cloak
                | EquipSlot::BodyArmor
                | EquipSlot::Shield
                | EquipSlot::Gloves
                | EquipSlot::Boots
                | EquipSlot::Shirt
        ) {
            continue;
        }
        if let Some(core) = world.get_component::<ObjectCore>(item)
            && let Some(def) = crate::items::object_def_for_core(obj_defs, &core)
            && let Some(ref armor_info) = def.armor
            && armor_info.magic_cancel > mc
        {
            mc = armor_info.magic_cancel;
        }
    }

    mc.clamp(0, 3) as u8
}

/// Probability that a magic attack is negated by the given MC level.
///
/// MC 0 = 0%, MC 1 = 30%, MC 2 = 60%, MC 3 = 90%.
pub fn mc_negation_chance(mc: u8) -> u32 {
    match mc.min(3) {
        0 => 0,
        1 => 30,
        2 => 60,
        3 => 90,
        _ => unreachable!(),
    }
}

/// Roll whether a magic attack is negated by MC.
pub fn mc_negates<R: Rng>(rng: &mut R, mc: u8) -> bool {
    let threshold = 3 * mc.min(3) as u32;
    rng.random_range(0..10u32) < threshold
}

// ---------------------------------------------------------------------------
// AC_VALUE — randomized negative AC
// ---------------------------------------------------------------------------

/// When negative AC is used for hit determination, it is randomly weakened.
///
/// If AC >= 0, returns AC unchanged.
/// If AC < 0, returns -rnd(-AC), i.e., a random value between -1 and AC.
pub fn ac_value<R: Rng>(rng: &mut R, ac: i32) -> i32 {
    if ac >= 0 {
        ac
    } else {
        -(rng.random_range(1..=(-ac) as u32) as i32)
    }
}

/// Negative AC damage reduction (applied after a melee hit lands).
///
/// If AC < 0, reduces damage by rnd(|AC|), minimum 1 damage remaining.
pub fn ac_damage_reduction<R: Rng>(rng: &mut R, ac: i32, damage: u32) -> u32 {
    if ac >= 0 || damage == 0 {
        return damage;
    }
    let reduction = rng.random_range(1..=(-ac) as u32);
    (damage.saturating_sub(reduction)).max(1)
}

// ---------------------------------------------------------------------------
// Attribute-modifying equipment
// ---------------------------------------------------------------------------

/// Attribute modifications conferred by equipped items.
#[derive(Debug, Clone, Copy, Default)]
pub struct AttributeBonus {
    pub strength: i32,
    pub dexterity: i32,
    pub constitution: i32,
    pub intelligence: i32,
    pub wisdom: i32,
    pub charisma: i32,
}

/// Calculate total attribute bonuses from all worn equipment.
///
/// - Helm of brilliance: +spe to INT and WIS
/// - Gauntlets of dexterity: +spe to DEX
/// - Gauntlets of power: +spe to STR
pub fn equipment_attribute_bonuses(
    world: &GameWorld,
    player: Entity,
    obj_defs: &[ObjectDef],
) -> AttributeBonus {
    let mut bonus = AttributeBonus::default();
    let equip = match world.get_component::<EquipmentSlots>(player) {
        Some(e) => e,
        None => return bonus,
    };

    for (_slot, item) in equip.all_worn() {
        let core = match world.get_component::<ObjectCore>(item) {
            Some(c) => c,
            None => continue,
        };
        let def = match crate::items::object_def_for_core(obj_defs, &core) {
            Some(d) => d,
            None => continue,
        };
        let spe = world
            .get_component::<Enchantment>(item)
            .map(|e| e.spe as i32)
            .unwrap_or(0);

        // Match by item name to identify special attribute-granting items.
        match def.name.as_str() {
            "helm of brilliance" => {
                bonus.intelligence += spe;
                bonus.wisdom += spe;
            }
            "gauntlets of dexterity" => {
                bonus.dexterity += spe;
            }
            "gauntlets of power" => {
                bonus.strength += spe;
            }
            _ => {}
        }
    }

    bonus
}

// ---------------------------------------------------------------------------
// Convenience queries
// ---------------------------------------------------------------------------

/// Returns true if the player is wearing body armor.
pub fn wearing_body_armor(world: &GameWorld, player: Entity) -> bool {
    world
        .get_component::<EquipmentSlots>(player)
        .is_some_and(|equip| equip.body_armor.is_some())
}

/// Returns true if the player is wearing a shield.
pub fn wearing_shield(world: &GameWorld, player: Entity) -> bool {
    world
        .get_component::<EquipmentSlots>(player)
        .is_some_and(|equip| equip.shield.is_some())
}

/// Returns true if the player is wearing a cloak.
pub fn wearing_cloak(world: &GameWorld, player: Entity) -> bool {
    world
        .get_component::<EquipmentSlots>(player)
        .is_some_and(|equip| equip.cloak.is_some())
}

/// Returns true if the player is wearing gloves.
pub fn wearing_gloves(world: &GameWorld, player: Entity) -> bool {
    world
        .get_component::<EquipmentSlots>(player)
        .is_some_and(|equip| equip.gloves.is_some())
}

/// Returns true if the player is wearing boots.
pub fn wearing_boots(world: &GameWorld, player: Entity) -> bool {
    world
        .get_component::<EquipmentSlots>(player)
        .is_some_and(|equip| equip.boots.is_some())
}

/// Returns true if the player is wearing a helmet.
pub fn wearing_helmet(world: &GameWorld, player: Entity) -> bool {
    world
        .get_component::<EquipmentSlots>(player)
        .is_some_and(|equip| equip.helmet.is_some())
}

/// Returns true if the player is wearing a shirt.
pub fn wearing_shirt(world: &GameWorld, player: Entity) -> bool {
    world
        .get_component::<EquipmentSlots>(player)
        .is_some_and(|equip| equip.shirt.is_some())
}

/// Returns the display names of all currently worn armor pieces.
///
/// Used by the polymorph system to determine which pieces break when
/// transforming into a large form.
pub fn worn_armor_names(world: &GameWorld, player: Entity) -> Vec<String> {
    let equip = match world.get_component::<EquipmentSlots>(player) {
        Some(e) => e,
        None => return vec![],
    };
    let armor_slots = [
        equip.body_armor,
        equip.cloak,
        equip.shirt,
        equip.helmet,
        equip.gloves,
        equip.boots,
        equip.shield,
    ];
    armor_slots
        .iter()
        .filter_map(|slot| {
            let entity = (*slot)?;
            Some(world.entity_name(entity))
        })
        .collect()
}

/// Returns the material of the currently worn body armor, if any.
pub fn body_armor_material(
    world: &GameWorld,
    player: Entity,
    obj_defs: &[ObjectDef],
) -> Option<Material> {
    let equip = world.get_component::<EquipmentSlots>(player)?;
    let armor_entity = equip.body_armor?;
    let core = world.get_component::<ObjectCore>(armor_entity)?;
    let def = crate::items::object_def_for_core(obj_defs, &core)?;
    Some(def.material)
}

/// Check if the player has an erosion-proof item in a given slot.
pub fn slot_is_erodeproof(world: &GameWorld, player: Entity, slot: EquipSlot) -> bool {
    let equip = match world.get_component::<EquipmentSlots>(player) {
        Some(e) => e,
        None => return false,
    };
    let item = match equip.get(slot) {
        Some(e) => e,
        None => return false,
    };
    world
        .get_component::<Erosion>(item)
        .is_some_and(|e| e.erodeproof)
}

/// Check if the player has a greased item in a given slot.
pub fn slot_is_greased(world: &GameWorld, player: Entity, slot: EquipSlot) -> bool {
    let equip = match world.get_component::<EquipmentSlots>(player) {
        Some(e) => e,
        None => return false,
    };
    let item = match equip.get(slot) {
        Some(e) => e,
        None => return false,
    };
    world
        .get_component::<Erosion>(item)
        .is_some_and(|e| e.greased)
}

// ---------------------------------------------------------------------------
// Conferred properties from equipment
// ---------------------------------------------------------------------------

/// A property conferred by worn equipment.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConferredProperty {
    /// The slot conferring the property.
    pub slot: EquipSlot,
    /// Name of the property (e.g. "speed", "levitation", "invisibility").
    pub property: String,
    /// Whether this is from an artifact or a base item property.
    pub from_artifact: bool,
}

/// List all properties conferred by currently worn equipment.
///
/// Checks each equipped item's `conferred_property` field from its ObjectDef,
/// plus special item names that confer properties (speed boots, levitation
/// boots, cloak of invisibility, etc.).
pub fn conferred_properties(
    world: &GameWorld,
    player: Entity,
    obj_defs: &[ObjectDef],
) -> Vec<ConferredProperty> {
    let equip = match world.get_component::<EquipmentSlots>(player) {
        Some(e) => e,
        None => return Vec::new(),
    };

    let mut props = Vec::new();

    for (slot, item) in equip.all_worn() {
        let core = match world.get_component::<ObjectCore>(item) {
            Some(c) => c,
            None => continue,
        };

        // Look up the ObjectDef for this item.
        let def = crate::items::object_def_for_core(obj_defs, &core);
        if let Some(def) = def {
            // Check conferred_property field.
            if let Some(ref prop) = def.conferred_property {
                props.push(ConferredProperty {
                    slot,
                    property: format!("{:?}", prop),
                    from_artifact: core.artifact.is_some(),
                });
            }

            // Check name-based properties (special items).
            let name_props = name_based_properties(&def.name, slot);
            for prop_name in name_props {
                props.push(ConferredProperty {
                    slot,
                    property: prop_name,
                    from_artifact: core.artifact.is_some(),
                });
            }
        }
    }

    props
}

/// Properties inferred from item name and slot.
///
/// Handles NetHack's special equipment like speed boots, levitation boots,
/// gauntlets of power, etc.
fn name_based_properties(name: &str, slot: EquipSlot) -> Vec<String> {
    let mut props = Vec::new();
    let lower = name.to_lowercase();

    match slot {
        EquipSlot::Boots => {
            if lower.contains("speed") {
                props.push("speed".to_string());
            } else if lower.contains("levitation") {
                props.push("levitation".to_string());
            } else if lower.contains("water walking") {
                props.push("water_walking".to_string());
            } else if lower.contains("jumping") {
                props.push("jumping".to_string());
            } else if lower.contains("elven") {
                props.push("stealth".to_string());
            } else if lower.contains("fumble") {
                props.push("fumbling".to_string());
            }
        }
        EquipSlot::Cloak => {
            if lower.contains("invisibility") {
                props.push("invisibility".to_string());
            } else if lower.contains("displacement") {
                props.push("displacement".to_string());
            } else if lower.contains("magic resistance") {
                props.push("magic_resistance".to_string());
            } else if lower.contains("protection") {
                props.push("protection".to_string());
            }
        }
        EquipSlot::Helmet => {
            if lower.contains("brilliance") {
                props.push("brilliance".to_string());
            } else if lower.contains("telepathy") {
                props.push("telepathy".to_string());
            } else if lower.contains("opposite alignment") {
                props.push("opposite_alignment".to_string());
            }
        }
        EquipSlot::Gloves => {
            if lower.contains("power") {
                props.push("strength".to_string());
            } else if lower.contains("dexterity") {
                props.push("dexterity".to_string());
            }
        }
        EquipSlot::Shield if lower.contains("reflection") => {
            props.push("reflection".to_string());
        }
        EquipSlot::Amulet => {
            if lower.contains("reflection") {
                props.push("reflection".to_string());
            } else if lower.contains("life saving") {
                props.push("life_saving".to_string());
            } else if lower.contains("unchanging") {
                props.push("unchanging".to_string());
            } else if lower.contains("magical breathing") {
                props.push("magical_breathing".to_string());
            } else if lower.contains("strangulation") {
                props.push("strangulation".to_string());
            } else if lower.contains("restful sleep") {
                props.push("restful_sleep".to_string());
            }
        }
        EquipSlot::RingLeft | EquipSlot::RingRight => {
            if lower.contains("free action") {
                props.push("free_action".to_string());
            } else if lower.contains("see invisible") {
                props.push("see_invisible".to_string());
            } else if lower.contains("teleport control") {
                props.push("teleport_control".to_string());
            } else if lower.contains("slow digestion") {
                props.push("slow_digestion".to_string());
            } else if lower.contains("conflict") {
                props.push("conflict".to_string());
            } else if lower.contains("warning") {
                props.push("warning".to_string());
            } else if lower.contains("fire resistance") {
                props.push("fire_resistance".to_string());
            } else if lower.contains("cold resistance") {
                props.push("cold_resistance".to_string());
            } else if lower.contains("shock resistance") {
                props.push("shock_resistance".to_string());
            } else if lower.contains("poison resistance") {
                props.push("poison_resistance".to_string());
            }
        }
        _ => {}
    }

    props
}

// ---------------------------------------------------------------------------
// Cursed item penalties
// ---------------------------------------------------------------------------

/// Describe what cursed equipped items are preventing.
///
/// Returns a list of human-readable descriptions of restrictions imposed
/// by cursed equipment (e.g., "cursed cloak prevents removing body armor").
pub fn cursed_item_penalties(
    world: &GameWorld,
    player: Entity,
    obj_defs: &[ObjectDef],
) -> Vec<String> {
    let equip = match world.get_component::<EquipmentSlots>(player) {
        Some(e) => e,
        None => return Vec::new(),
    };

    let mut penalties = Vec::new();

    for (slot, item) in equip.all_worn() {
        let buc = world.get_component::<BucStatus>(item);
        let is_cursed = buc.is_some_and(|b| b.cursed);

        if !is_cursed {
            continue;
        }

        let core = world.get_component::<ObjectCore>(item);
        let item_name = core
            .and_then(|c| crate::items::object_def_for_core(obj_defs, &c))
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "unknown item".to_string());

        match slot {
            EquipSlot::Weapon => {
                penalties.push(format!("cursed {} cannot be unwielded", item_name));
            }
            EquipSlot::Cloak => {
                penalties.push(format!(
                    "cursed {} prevents removing body armor and shirt",
                    item_name
                ));
            }
            EquipSlot::BodyArmor => {
                penalties.push(format!("cursed {} cannot be removed", item_name));
            }
            EquipSlot::Helmet => {
                penalties.push(format!("cursed {} is welded to your head", item_name));
            }
            EquipSlot::Gloves => {
                penalties.push(format!("cursed {} are welded to your hands", item_name));
                penalties.push(format!("cursed {} prevent removing rings", item_name));
            }
            EquipSlot::Boots => {
                penalties.push(format!("cursed {} are welded to your feet", item_name));
            }
            EquipSlot::Shield => {
                penalties.push(format!("cursed {} is welded to your arm", item_name));
            }
            EquipSlot::RingLeft | EquipSlot::RingRight => {
                penalties.push(format!("cursed {} cannot be removed", item_name));
            }
            EquipSlot::Amulet => {
                penalties.push(format!("cursed {} is stuck to your neck", item_name));
            }
            _ => {
                penalties.push(format!("cursed {} cannot be removed", item_name));
            }
        }
    }

    penalties
}

/// Wear timing constants matching C NetHack's do_wear.c.
///
/// In C NetHack, `domulti` sets multi-turn delays:
/// - Body armor: 5 turns to don, 5 to doff
/// - Cloak: 1 turn
/// - Shirt: 5 turns (must remove cloak + body armor first)
/// - Boots: 1 turn
/// - Gloves: 1 turn
/// - Helmet: 1 turn
/// - Shield: 1 turn
/// - Ring: 1 turn
/// - Amulet: 1 turn
pub fn wear_time(slot: EquipSlot) -> u32 {
    match slot {
        EquipSlot::BodyArmor => 5,
        EquipSlot::Shirt => 5,
        _ => 1,
    }
}

/// Time to remove equipment from a slot.
pub fn remove_time(slot: EquipSlot) -> u32 {
    match slot {
        EquipSlot::BodyArmor => 5,
        EquipSlot::Shirt => 5,
        _ => 1,
    }
}

// ---------------------------------------------------------------------------
// Intrinsic changes from equipment (Ring_on/Ring_off, Amulet_on/Amulet_off,
// Boots_on/Boots_off, etc. from do_wear.c)
// ---------------------------------------------------------------------------

/// A change to the player's intrinsics when equipping or unequipping an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntrinsicChange {
    /// Gain an intrinsic property.
    Gain(&'static str),
    /// Lose an intrinsic property.
    Lose(&'static str),
    /// AC bonus from enchantment (ring of protection, cloak of protection).
    AcBonus,
    /// AC penalty removed (when removing protective equipment).
    AcPenalty,
    /// Damage bonus from ring of increase damage.
    DamageBonus,
    /// Accuracy bonus from ring of increase accuracy.
    AccuracyBonus,
    /// Stat bonus from enchantment (helm of brilliance, gauntlets of dexterity).
    StatBonus(&'static str),
    /// Set a stat to a fixed value (gauntlets of power: STR=25).
    SetStat(&'static str, i32),
    /// Alignment change (helm of opposite alignment).
    AlignmentChange,
}

/// Effects when putting on a ring.
///
/// Based on `Ring_on()` in `do_wear.c`. Returns the intrinsic changes
/// conferred by the ring.
pub fn ring_on_effect(ring_name: &str) -> Vec<IntrinsicChange> {
    match ring_name {
        "ring of fire resistance" => vec![IntrinsicChange::Gain("fire_resistance")],
        "ring of cold resistance" => vec![IntrinsicChange::Gain("cold_resistance")],
        "ring of poison resistance" => vec![IntrinsicChange::Gain("poison_resistance")],
        "ring of shock resistance" => vec![IntrinsicChange::Gain("shock_resistance")],
        "ring of free action" => vec![IntrinsicChange::Gain("free_action")],
        "ring of see invisible" => vec![IntrinsicChange::Gain("see_invisible")],
        "ring of invisibility" => vec![IntrinsicChange::Gain("invisibility")],
        "ring of teleportation" => vec![IntrinsicChange::Gain("teleportitis")],
        "ring of teleport control" => vec![IntrinsicChange::Gain("teleport_control")],
        "ring of levitation" => vec![IntrinsicChange::Gain("levitation")],
        "ring of stealth" => vec![IntrinsicChange::Gain("stealth")],
        "ring of regeneration" => vec![IntrinsicChange::Gain("regeneration")],
        "ring of searching" => vec![IntrinsicChange::Gain("searching")],
        "ring of protection" => vec![IntrinsicChange::AcBonus],
        "ring of increase damage" => vec![IntrinsicChange::DamageBonus],
        "ring of increase accuracy" => vec![IntrinsicChange::AccuracyBonus],
        "ring of sustain ability" => vec![IntrinsicChange::Gain("sustain_ability")],
        "ring of conflict" => vec![IntrinsicChange::Gain("conflict")],
        "ring of warning" => vec![IntrinsicChange::Gain("warning")],
        "ring of hunger" => vec![IntrinsicChange::Gain("hunger")],
        "ring of slow digestion" => vec![IntrinsicChange::Gain("slow_digestion")],
        "ring of polymorph" => vec![IntrinsicChange::Gain("polymorphitis")],
        "ring of polymorph control" => vec![IntrinsicChange::Gain("polymorph_control")],
        _ => vec![],
    }
}

/// Effects when removing a ring.
///
/// Inverse of `ring_on_effect` — `Lose` instead of `Gain`.
pub fn ring_off_effect(ring_name: &str) -> Vec<IntrinsicChange> {
    ring_on_effect(ring_name)
        .into_iter()
        .map(|change| match change {
            IntrinsicChange::Gain(prop) => IntrinsicChange::Lose(prop),
            IntrinsicChange::AcBonus => IntrinsicChange::AcPenalty,
            other => other,
        })
        .collect()
}

/// Effects when putting on an amulet.
///
/// Based on `Amulet_on()` in `do_wear.c`.
pub fn amulet_on_effect(amulet_name: &str) -> Vec<IntrinsicChange> {
    match amulet_name {
        "amulet of ESP" => vec![IntrinsicChange::Gain("telepathy")],
        "amulet of life saving" => vec![IntrinsicChange::Gain("life_saving")],
        "amulet of reflection" => vec![IntrinsicChange::Gain("reflection")],
        "amulet of magical breathing" => {
            vec![IntrinsicChange::Gain("magical_breathing")]
        }
        "amulet versus poison" => vec![IntrinsicChange::Gain("poison_resistance")],
        "amulet of unchanging" => vec![IntrinsicChange::Gain("unchanging")],
        "amulet of strangulation" => vec![IntrinsicChange::Gain("strangled")],
        "amulet of restful sleep" => vec![IntrinsicChange::Gain("sleeping")],
        _ => vec![],
    }
}

/// Effects when removing an amulet.
pub fn amulet_off_effect(amulet_name: &str) -> Vec<IntrinsicChange> {
    amulet_on_effect(amulet_name)
        .into_iter()
        .map(|change| match change {
            IntrinsicChange::Gain(prop) => IntrinsicChange::Lose(prop),
            other => other,
        })
        .collect()
}

/// Effects when putting on boots.
///
/// Based on `Boots_on()` in `do_wear.c`.
pub fn boots_on_effect(boot_name: &str) -> Vec<IntrinsicChange> {
    match boot_name {
        "speed boots" => vec![IntrinsicChange::Gain("speed")],
        "levitation boots" => vec![IntrinsicChange::Gain("levitation")],
        "jumping boots" => vec![IntrinsicChange::Gain("jumping")],
        "elven boots" => vec![IntrinsicChange::Gain("stealth")],
        "fumble boots" => vec![IntrinsicChange::Gain("fumbling")],
        "water walking boots" => vec![IntrinsicChange::Gain("water_walking")],
        _ => vec![],
    }
}

/// Effects when removing boots.
pub fn boots_off_effect(boot_name: &str) -> Vec<IntrinsicChange> {
    boots_on_effect(boot_name)
        .into_iter()
        .map(|change| match change {
            IntrinsicChange::Gain(prop) => IntrinsicChange::Lose(prop),
            other => other,
        })
        .collect()
}

/// Effects when putting on a helmet.
///
/// Based on `Helmet_on()` in `do_wear.c`.
pub fn helm_on_effect(helm_name: &str) -> Vec<IntrinsicChange> {
    match helm_name {
        "helm of brilliance" => vec![
            IntrinsicChange::StatBonus("intelligence"),
            IntrinsicChange::StatBonus("wisdom"),
        ],
        "helm of telepathy" => vec![IntrinsicChange::Gain("telepathy")],
        "helm of opposite alignment" => vec![IntrinsicChange::AlignmentChange],
        _ => vec![],
    }
}

/// Effects when removing a helmet.
pub fn helm_off_effect(helm_name: &str) -> Vec<IntrinsicChange> {
    helm_on_effect(helm_name)
        .into_iter()
        .map(|change| match change {
            IntrinsicChange::Gain(prop) => IntrinsicChange::Lose(prop),
            other => other,
        })
        .collect()
}

/// Effects when putting on a cloak.
///
/// Based on `Cloak_on()` in `do_wear.c`.
pub fn cloak_on_effect(cloak_name: &str) -> Vec<IntrinsicChange> {
    match cloak_name {
        "cloak of displacement" => vec![IntrinsicChange::Gain("displacement")],
        "cloak of invisibility" => vec![IntrinsicChange::Gain("invisibility")],
        "cloak of magic resistance" => {
            vec![IntrinsicChange::Gain("magic_resistance")]
        }
        "cloak of protection" => vec![IntrinsicChange::AcBonus],
        "mummy wrapping" => vec![IntrinsicChange::Gain("visible_to_undead")],
        "elven cloak" => vec![IntrinsicChange::Gain("stealth")],
        _ => vec![],
    }
}

/// Effects when removing a cloak.
pub fn cloak_off_effect(cloak_name: &str) -> Vec<IntrinsicChange> {
    cloak_on_effect(cloak_name)
        .into_iter()
        .map(|change| match change {
            IntrinsicChange::Gain(prop) => IntrinsicChange::Lose(prop),
            IntrinsicChange::AcBonus => IntrinsicChange::AcPenalty,
            other => other,
        })
        .collect()
}

/// Effects when putting on gloves.
///
/// Based on `Gloves_on()` in `do_wear.c`.
pub fn gloves_on_effect(glove_name: &str) -> Vec<IntrinsicChange> {
    match glove_name {
        "gauntlets of power" => vec![IntrinsicChange::SetStat("strength", 25)],
        "gauntlets of dexterity" => vec![IntrinsicChange::StatBonus("dexterity")],
        "gauntlets of fumbling" => vec![IntrinsicChange::Gain("fumbling")],
        _ => vec![],
    }
}

/// Effects when removing gloves.
pub fn gloves_off_effect(glove_name: &str) -> Vec<IntrinsicChange> {
    gloves_on_effect(glove_name)
        .into_iter()
        .map(|change| match change {
            IntrinsicChange::Gain(prop) => IntrinsicChange::Lose(prop),
            IntrinsicChange::SetStat(stat, _) => IntrinsicChange::StatBonus(stat),
            other => other,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::items::{SpawnLocation, spawn_item};
    use nethack_babel_data::{
        ArmorCategory, ArmorInfo, Color, Material, ObjectClass, ObjectTypeId, WeaponInfo,
        WeaponSkill,
    };

    /// Helper: build a minimal weapon ObjectDef.
    fn weapon_def(id: u16, name: &str, damage_small: i8, damage_large: i8) -> ObjectDef {
        ObjectDef {
            id: ObjectTypeId(id),
            name: name.to_string(),
            appearance: None,
            class: ObjectClass::Weapon,
            color: Color::White,
            material: Material::Iron,
            weight: 40,
            cost: 15,
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
            weapon: Some(WeaponInfo {
                skill: WeaponSkill::LongSword,
                hit_bonus: 0,
                damage_small,
                damage_large,
                strike_mode: nethack_babel_data::StrikeMode::empty(),
            }),
            armor: None,
            spellbook: None,
            conferred_property: None,
            use_delay: 0,
        }
    }

    /// Helper: build an armor ObjectDef.
    fn armor_def(id: u16, name: &str, category: ArmorCategory, ac_bonus: i8) -> ObjectDef {
        ObjectDef {
            id: ObjectTypeId(id),
            name: name.to_string(),
            appearance: None,
            class: ObjectClass::Armor,
            color: Color::White,
            material: Material::Iron,
            weight: 100,
            cost: 50,
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
            armor: Some(ArmorInfo {
                category,
                ac_bonus,
                magic_cancel: 0,
            }),
            spellbook: None,
            conferred_property: None,
            use_delay: 0,
        }
    }

    /// Helper: build an armor with MC.
    fn armor_def_with_mc(
        id: u16,
        name: &str,
        category: ArmorCategory,
        ac_bonus: i8,
        magic_cancel: i8,
    ) -> ObjectDef {
        let mut def = armor_def(id, name, category, ac_bonus);
        if let Some(ref mut armor) = def.armor {
            armor.magic_cancel = magic_cancel;
        }
        def
    }

    /// Helper: build a two-handed weapon ObjectDef.
    fn two_handed_weapon_def(id: u16, name: &str) -> ObjectDef {
        let mut def = weapon_def(id, name, 12, 12);
        def.is_bimanual = true;
        def
    }

    /// Helper: build a ring ObjectDef.
    fn ring_def(id: u16, name: &str) -> ObjectDef {
        ObjectDef {
            id: ObjectTypeId(id),
            name: name.to_string(),
            appearance: None,
            class: ObjectClass::Ring,
            color: Color::White,
            material: Material::Iron,
            weight: 3,
            cost: 100,
            nutrition: 0,
            prob: 10,
            is_magic: true,
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

    /// Helper: ring of protection.
    fn ring_of_protection_def(id: u16) -> ObjectDef {
        let mut def = ring_def(id, "ring of protection");
        def.conferred_property = Some(nethack_babel_data::Property::Protection);
        def
    }

    /// Helper: amulet of guarding.
    fn amulet_of_guarding_def(id: u16) -> ObjectDef {
        ObjectDef {
            id: ObjectTypeId(id),
            name: "amulet of guarding".to_string(),
            appearance: None,
            class: ObjectClass::Amulet,
            color: Color::White,
            material: Material::Iron,
            weight: 20,
            cost: 150,
            nutrition: 0,
            prob: 10,
            is_magic: true,
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
            conferred_property: None, // guarding identified by name, not property enum
            use_delay: 0,
        }
    }

    /// Helper: create a game world with EquipmentSlots on the player.
    fn test_world() -> GameWorld {
        let mut world = GameWorld::new(Position::new(40, 10));
        let player = world.player();
        let _ = world
            .ecs_mut()
            .insert_one(player, EquipmentSlots::default());
        world
    }

    // ── ARM_BONUS tests ──────────────────────────────────────────

    #[test]
    fn test_arm_bonus_no_erosion() {
        // Plate mail: a_ac=7, spe=2, no erosion
        assert_eq!(arm_bonus(7, 2, 0, 0), 9);
    }

    #[test]
    fn test_arm_bonus_with_erosion() {
        // Plate mail: a_ac=7, spe=2, erosion=2
        // ARM_BONUS = 7 + 2 - min(2, 7) = 7
        assert_eq!(arm_bonus(7, 2, 2, 0), 7);
    }

    #[test]
    fn test_arm_bonus_erosion_capped_at_base() {
        // Leather helm: a_ac=1, spe=0, erosion=3
        // ARM_BONUS = 1 + 0 - min(3, 1) = 0
        assert_eq!(arm_bonus(1, 0, 3, 0), 0);
    }

    #[test]
    fn test_arm_bonus_enchantment_not_eroded() {
        // Leather helm: a_ac=1, spe=3, erosion=3
        // ARM_BONUS = 1 + 3 - min(3, 1) = 3
        assert_eq!(arm_bonus(1, 3, 3, 0), 3);
    }

    #[test]
    fn test_arm_bonus_negative_enchantment() {
        // a_ac=7, spe=-2, no erosion
        assert_eq!(arm_bonus(7, -2, 0, 0), 5);
    }

    #[test]
    fn test_arm_bonus_greatest_erosion() {
        // eroded=1, eroded2=3 → greatest = 3
        assert_eq!(arm_bonus(7, 0, 1, 3), 4);
    }

    // ── Equip weapon ─────────────────────────────────────────────

    #[test]
    fn test_equip_weapon() {
        let mut world = test_world();
        let player = world.player();
        let def = weapon_def(1, "long sword", 8, 12);
        let defs = vec![def.clone()];
        let item = spawn_item(&mut world, &def, SpawnLocation::Inventory, Some(0));

        let result = equip_item(&mut world, player, item, EquipSlot::Weapon, &defs);
        assert!(result.is_ok());

        let events = result.unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::ItemWielded { .. }))
        );

        let equip = world.get_component::<EquipmentSlots>(player).unwrap();
        assert_eq!(equip.weapon, Some(item));
    }

    // ── Unequip weapon ───────────────────────────────────────────

    #[test]
    fn test_unequip_weapon() {
        let mut world = test_world();
        let player = world.player();
        let def = weapon_def(1, "long sword", 8, 12);
        let defs = vec![def.clone()];
        let item = spawn_item(&mut world, &def, SpawnLocation::Inventory, Some(0));

        equip_item(&mut world, player, item, EquipSlot::Weapon, &defs).unwrap();

        let result = unequip_item(&mut world, player, item);
        assert!(result.is_ok());

        let events = result.unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::ItemRemoved { .. }))
        );

        let equip = world.get_component::<EquipmentSlots>(player).unwrap();
        assert_eq!(equip.weapon, None);
    }

    // ── Cursed item cannot be removed ────────────────────────────

    #[test]
    fn test_cursed_cannot_remove() {
        let mut world = test_world();
        let player = world.player();
        let def = weapon_def(1, "long sword", 8, 12);
        let defs = vec![def.clone()];
        let item = spawn_item(&mut world, &def, SpawnLocation::Inventory, Some(0));

        {
            let mut buc = world.get_component_mut::<BucStatus>(item).unwrap();
            buc.cursed = true;
        }

        equip_item(&mut world, player, item, EquipSlot::Weapon, &defs).unwrap();

        let result = unequip_item(&mut world, player, item);
        assert!(matches!(result, Err(EquipError::CursedCannotRemove)));

        let equip = world.get_component::<EquipmentSlots>(player).unwrap();
        assert_eq!(equip.weapon, Some(item));
    }

    // ── Two-handed weapon blocks shield ──────────────────────────

    #[test]
    fn test_two_hand_weapon_blocks_shield() {
        let mut world = test_world();
        let player = world.player();
        let two_h = two_handed_weapon_def(1, "two-handed sword");
        let shield = armor_def(2, "small shield", ArmorCategory::Shield, -1);
        let defs = vec![two_h.clone(), shield.clone()];

        let weapon_item = spawn_item(&mut world, &two_h, SpawnLocation::Inventory, Some(0));
        equip_item(&mut world, player, weapon_item, EquipSlot::Weapon, &defs).unwrap();

        let shield_item = spawn_item(&mut world, &shield, SpawnLocation::Inventory, Some(0));
        let result = equip_item(&mut world, player, shield_item, EquipSlot::Shield, &defs);
        assert!(matches!(result, Err(EquipError::TwoHandedConflict)));
    }

    // ── Two-handed weapon auto-unequips non-cursed shield ────────

    #[test]
    fn test_two_hand_weapon_auto_unequips_shield() {
        let mut world = test_world();
        let player = world.player();
        let shield = armor_def(2, "small shield", ArmorCategory::Shield, -1);
        let two_h = two_handed_weapon_def(1, "two-handed sword");
        let defs = vec![two_h.clone(), shield.clone()];

        let shield_item = spawn_item(&mut world, &shield, SpawnLocation::Inventory, Some(0));
        equip_item(&mut world, player, shield_item, EquipSlot::Shield, &defs).unwrap();

        let weapon_item = spawn_item(&mut world, &two_h, SpawnLocation::Inventory, Some(0));
        let result = equip_item(&mut world, player, weapon_item, EquipSlot::Weapon, &defs);
        assert!(result.is_ok());

        let events = result.unwrap();
        assert!(
            events.iter().any(
                |e| matches!(e, EngineEvent::ItemRemoved { item, .. } if *item == shield_item)
            )
        );
        assert!(
            events.iter().any(
                |e| matches!(e, EngineEvent::ItemWielded { item, .. } if *item == weapon_item)
            )
        );

        let equip = world.get_component::<EquipmentSlots>(player).unwrap();
        assert_eq!(equip.weapon, Some(weapon_item));
        assert_eq!(equip.shield, None);
    }

    // ── Two-handed weapon blocked by cursed shield ───────────────

    #[test]
    fn test_two_hand_blocked_by_cursed_shield() {
        let mut world = test_world();
        let player = world.player();
        let shield = armor_def(2, "small shield", ArmorCategory::Shield, -1);
        let two_h = two_handed_weapon_def(1, "two-handed sword");
        let defs = vec![two_h.clone(), shield.clone()];

        let shield_item = spawn_item(&mut world, &shield, SpawnLocation::Inventory, Some(0));
        {
            let mut buc = world.get_component_mut::<BucStatus>(shield_item).unwrap();
            buc.cursed = true;
        }
        equip_item(&mut world, player, shield_item, EquipSlot::Shield, &defs).unwrap();

        let weapon_item = spawn_item(&mut world, &two_h, SpawnLocation::Inventory, Some(0));
        let result = equip_item(&mut world, player, weapon_item, EquipSlot::Weapon, &defs);
        assert!(matches!(result, Err(EquipError::TwoHandedConflict)));
    }

    // ── AC calculation ───────────────────────────────────────────

    #[test]
    fn test_ac_calculation() {
        let mut world = test_world();
        let player = world.player();

        let plate_mail = armor_def(1, "plate mail", ArmorCategory::Suit, -7);
        let shield = armor_def(2, "small shield", ArmorCategory::Shield, -1);
        let defs = vec![plate_mail.clone(), shield.clone()];

        assert_eq!(calculate_ac(&world, player, &defs), 10);

        let pm_item = spawn_item(&mut world, &plate_mail, SpawnLocation::Inventory, Some(2));
        equip_item(&mut world, player, pm_item, EquipSlot::BodyArmor, &defs).unwrap();

        // AC = 10 - (7 + 2 - 0) = 1.
        assert_eq!(calculate_ac(&world, player, &defs), 1);

        let sh_item = spawn_item(&mut world, &shield, SpawnLocation::Inventory, Some(0));
        equip_item(&mut world, player, sh_item, EquipSlot::Shield, &defs).unwrap();

        // AC = 10 - (7 + 2 - 0) - (1 + 0 - 0) = 0.
        assert_eq!(calculate_ac(&world, player, &defs), 0);
    }

    // ── AC with erosion ──────────────────────────────────────────

    #[test]
    fn test_ac_with_erosion() {
        let mut world = test_world();
        let player = world.player();
        let plate_mail = armor_def(1, "plate mail", ArmorCategory::Suit, -7);
        let defs = vec![plate_mail.clone()];

        let pm_item = spawn_item(&mut world, &plate_mail, SpawnLocation::Inventory, Some(0));
        // Set erosion to 2.
        {
            let mut erosion = world.get_component_mut::<Erosion>(pm_item).unwrap();
            erosion.eroded = 2;
        }
        equip_item(&mut world, player, pm_item, EquipSlot::BodyArmor, &defs).unwrap();

        // ARM_BONUS = 7 + 0 - min(2, 7) = 5
        // AC = 10 - 5 = 5
        assert_eq!(calculate_ac(&world, player, &defs), 5);
    }

    // ── AC with ring of protection ───────────────────────────────

    #[test]
    fn test_ac_with_ring_of_protection() {
        let mut world = test_world();
        let player = world.player();
        let ring = ring_of_protection_def(1);
        let defs = vec![ring.clone()];

        let ring_item = spawn_item(&mut world, &ring, SpawnLocation::Inventory, Some(3));
        equip_item(&mut world, player, ring_item, EquipSlot::RingLeft, &defs).unwrap();

        // AC = 10 - 3 = 7
        assert_eq!(calculate_ac(&world, player, &defs), 7);
    }

    // ── AC with amulet of guarding ───────────────────────────────

    #[test]
    fn test_ac_with_amulet_of_guarding() {
        let mut world = test_world();
        let player = world.player();
        let amulet = amulet_of_guarding_def(1);
        let defs = vec![amulet.clone()];

        let amulet_item = spawn_item(&mut world, &amulet, SpawnLocation::Inventory, None);
        equip_item(&mut world, player, amulet_item, EquipSlot::Amulet, &defs).unwrap();

        // AC = 10 - 2 = 8
        assert_eq!(calculate_ac(&world, player, &defs), 8);
    }

    // ── AC clamped ───────────────────────────────────────────────

    #[test]
    fn test_ac_clamp() {
        let mut world = test_world();
        let player = world.player();
        // Ring of protection with spe=120 should clamp AC to -99.
        let ring = ring_of_protection_def(1);
        let defs = vec![ring.clone()];

        let ring_item = spawn_item(&mut world, &ring, SpawnLocation::Inventory, Some(120));
        equip_item(&mut world, player, ring_item, EquipSlot::RingLeft, &defs).unwrap();

        // AC = 10 - 120 = -110, clamped to -99.
        assert_eq!(calculate_ac(&world, player, &defs), -99);
    }

    // ── Magic Cancellation tests ─────────────────────────────────

    #[test]
    fn test_magic_cancellation_highest_wins() {
        let mut world = test_world();
        let player = world.player();
        let cloak = armor_def_with_mc(1, "cloak", ArmorCategory::Cloak, -1, 1);
        let plate = armor_def_with_mc(2, "plate mail", ArmorCategory::Suit, -7, 2);
        let defs = vec![cloak.clone(), plate.clone()];

        let cloak_item = spawn_item(&mut world, &cloak, SpawnLocation::Inventory, None);
        let plate_item = spawn_item(&mut world, &plate, SpawnLocation::Inventory, None);

        // Must equip body armor before cloak (layering constraint).
        equip_item(&mut world, player, plate_item, EquipSlot::BodyArmor, &defs).unwrap();
        equip_item(&mut world, player, cloak_item, EquipSlot::Cloak, &defs).unwrap();

        assert_eq!(magic_cancellation(&world, player, &defs), 2);
    }

    #[test]
    fn test_mc_negation_probabilities() {
        assert_eq!(mc_negation_chance(0), 0);
        assert_eq!(mc_negation_chance(1), 30);
        assert_eq!(mc_negation_chance(2), 60);
        assert_eq!(mc_negation_chance(3), 90);
    }

    // ── AC_VALUE tests ───────────────────────────────────────────

    #[test]
    fn test_ac_value_positive_unchanged() {
        let mut rng = rand::rng();
        for _ in 0..100 {
            assert_eq!(ac_value(&mut rng, 5), 5);
            assert_eq!(ac_value(&mut rng, 0), 0);
        }
    }

    #[test]
    fn test_ac_value_negative_range() {
        let mut rng = rand::rng();
        for _ in 0..200 {
            let val = ac_value(&mut rng, -20);
            assert!(
                val >= -20 && val <= -1,
                "ac_value(-20) should be in [-20, -1], got {}",
                val
            );
        }
    }

    // ── AC damage reduction tests ────────────────────────────────

    #[test]
    fn test_ac_damage_reduction_positive_ac() {
        let mut rng = rand::rng();
        // Positive AC: no reduction.
        assert_eq!(ac_damage_reduction(&mut rng, 5, 10), 10);
    }

    #[test]
    fn test_ac_damage_reduction_negative_ac() {
        let mut rng = rand::rng();
        for _ in 0..200 {
            let result = ac_damage_reduction(&mut rng, -20, 8);
            assert!(result >= 1, "minimum damage is 1, got {}", result);
            assert!(result <= 8, "cannot exceed original damage, got {}", result);
        }
    }

    // ── slot_for_item ────────────────────────────────────────────

    #[test]
    fn test_slot_for_item() {
        let weapon = weapon_def(1, "long sword", 8, 12);
        assert_eq!(slot_for_item(&weapon), Some(EquipSlot::Weapon));

        let plate = armor_def(2, "plate mail", ArmorCategory::Suit, -7);
        assert_eq!(slot_for_item(&plate), Some(EquipSlot::BodyArmor));

        let helm = armor_def(3, "helmet", ArmorCategory::Helm, -1);
        assert_eq!(slot_for_item(&helm), Some(EquipSlot::Helmet));

        let shield = armor_def(4, "shield", ArmorCategory::Shield, -1);
        assert_eq!(slot_for_item(&shield), Some(EquipSlot::Shield));

        let ring = ring_def(5, "ring of protection");
        assert_eq!(slot_for_item(&ring), Some(EquipSlot::RingLeft));
    }

    // ── Equip weapon used in combat (via get_weapon_stats) ───────

    #[test]
    fn test_equip_weapon_used_in_combat() {
        let mut world = test_world();
        let player = world.player();
        let def = weapon_def(1, "long sword", 8, 12);
        let defs = vec![def.clone()];

        assert!(get_weapon_stats(&world, player, &defs).is_none());

        let item = spawn_item(&mut world, &def, SpawnLocation::Inventory, Some(3));
        equip_item(&mut world, player, item, EquipSlot::Weapon, &defs).unwrap();

        let stats = get_weapon_stats(&world, player, &defs);
        assert!(stats.is_some());
        let stats = stats.unwrap();
        assert_eq!(stats.spe, 3);
        assert_eq!(stats.damage_small, 8);
        assert_eq!(stats.damage_large, 12);
        assert!(stats.is_weapon);
    }

    // ── Ring equip into left/right ───────────────────────────────

    #[test]
    fn test_ring_equip_left_and_right() {
        let mut world = test_world();
        let player = world.player();
        let ring = ring_def(1, "ring of protection");
        let ring2 = ring_def(2, "ring of free action");
        let defs = vec![ring.clone(), ring2.clone()];

        let ring_item = spawn_item(&mut world, &ring, SpawnLocation::Inventory, None);
        let ring2_item = spawn_item(&mut world, &ring2, SpawnLocation::Inventory, None);

        equip_item(&mut world, player, ring_item, EquipSlot::RingLeft, &defs).unwrap();

        equip_item(&mut world, player, ring2_item, EquipSlot::RingRight, &defs).unwrap();

        let equip = world.get_component::<EquipmentSlots>(player).unwrap();
        assert_eq!(equip.ring_left, Some(ring_item));
        assert_eq!(equip.ring_right, Some(ring2_item));
    }

    // ── Wrong slot ───────────────────────────────────────────────

    #[test]
    fn test_wrong_slot() {
        let mut world = test_world();
        let player = world.player();
        let def = weapon_def(1, "long sword", 8, 12);
        let defs = vec![def.clone()];
        let item = spawn_item(&mut world, &def, SpawnLocation::Inventory, Some(0));

        let result = equip_item(&mut world, player, item, EquipSlot::Helmet, &defs);
        assert!(matches!(result, Err(EquipError::WrongSlot)));
    }

    // ── Wearing body armor sets AC ───────────────────────────────

    #[test]
    fn test_wearing_body_armor_flag() {
        let mut world = test_world();
        let player = world.player();
        let plate = armor_def(1, "plate mail", ArmorCategory::Suit, -7);
        let defs = vec![plate.clone()];

        assert!(!wearing_body_armor(&world, player));

        let item = spawn_item(&mut world, &plate, SpawnLocation::Inventory, Some(0));
        equip_item(&mut world, player, item, EquipSlot::BodyArmor, &defs).unwrap();

        assert!(wearing_body_armor(&world, player));
    }

    // ── Replace equipped item ────────────────────────────────────

    #[test]
    fn test_replace_equipped_item() {
        let mut world = test_world();
        let player = world.player();
        let sword = weapon_def(1, "long sword", 8, 12);
        let axe = weapon_def(2, "battle-axe", 8, 8);
        let defs = vec![sword.clone(), axe.clone()];

        let sword_item = spawn_item(&mut world, &sword, SpawnLocation::Inventory, Some(0));
        let axe_item = spawn_item(&mut world, &axe, SpawnLocation::Inventory, Some(0));

        equip_item(&mut world, player, sword_item, EquipSlot::Weapon, &defs).unwrap();

        let result = equip_item(&mut world, player, axe_item, EquipSlot::Weapon, &defs);
        assert!(result.is_ok());

        let events = result.unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::ItemRemoved { item, .. } if *item == sword_item))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::ItemWielded { item, .. } if *item == axe_item))
        );

        let equip = world.get_component::<EquipmentSlots>(player).unwrap();
        assert_eq!(equip.weapon, Some(axe_item));
    }

    // ── Layering: cloak blocks suit ──────────────────────────────

    #[test]
    fn test_cloak_blocks_suit_equip() {
        let mut world = test_world();
        let player = world.player();
        let cloak = armor_def(1, "leather cloak", ArmorCategory::Cloak, -1);
        let suit = armor_def(2, "plate mail", ArmorCategory::Suit, -7);
        let defs = vec![cloak.clone(), suit.clone()];

        let cloak_item = spawn_item(&mut world, &cloak, SpawnLocation::Inventory, None);
        equip_item(&mut world, player, cloak_item, EquipSlot::Cloak, &defs).unwrap();

        // Now try to equip suit — should fail because cloak is worn.
        let suit_item = spawn_item(&mut world, &suit, SpawnLocation::Inventory, None);
        let result = equip_item(&mut world, player, suit_item, EquipSlot::BodyArmor, &defs);
        assert!(matches!(result, Err(EquipError::CloakBlocksSuit)));
    }

    // ── Layering: suit blocks shirt ──────────────────────────────

    #[test]
    fn test_suit_blocks_shirt_equip() {
        let mut world = test_world();
        let player = world.player();
        let suit = armor_def(1, "plate mail", ArmorCategory::Suit, -7);
        let shirt = armor_def(2, "Hawaiian shirt", ArmorCategory::Shirt, 0);
        let defs = vec![suit.clone(), shirt.clone()];

        let suit_item = spawn_item(&mut world, &suit, SpawnLocation::Inventory, None);
        equip_item(&mut world, player, suit_item, EquipSlot::BodyArmor, &defs).unwrap();

        // Now try to equip shirt — should fail.
        let shirt_item = spawn_item(&mut world, &shirt, SpawnLocation::Inventory, None);
        let result = equip_item(&mut world, player, shirt_item, EquipSlot::Shirt, &defs);
        assert!(matches!(result, Err(EquipError::SuitBlocksShirt)));
    }

    // ── Removal layering: cursed cloak blocks suit removal ───────

    #[test]
    fn test_cursed_cloak_blocks_suit_removal() {
        let mut world = test_world();
        let player = world.player();
        let suit = armor_def(1, "plate mail", ArmorCategory::Suit, -7);
        let cloak = armor_def(2, "leather cloak", ArmorCategory::Cloak, -1);
        let defs = vec![suit.clone(), cloak.clone()];

        // Equip suit first (no cloak yet).
        let suit_item = spawn_item(&mut world, &suit, SpawnLocation::Inventory, None);
        equip_item(&mut world, player, suit_item, EquipSlot::BodyArmor, &defs).unwrap();

        // Then equip cursed cloak.
        let cloak_item = spawn_item(&mut world, &cloak, SpawnLocation::Inventory, None);
        {
            let mut buc = world.get_component_mut::<BucStatus>(cloak_item).unwrap();
            buc.cursed = true;
        }
        equip_item(&mut world, player, cloak_item, EquipSlot::Cloak, &defs).unwrap();

        // Try to remove suit — should fail due to cursed cloak.
        let result = unequip_slot(&mut world, player, EquipSlot::BodyArmor);
        assert!(matches!(result, Err(EquipError::CursedCannotRemove)));
    }

    // ── All worn / armor count / is_naked ────────────────────────

    #[test]
    fn test_equipment_queries() {
        let mut world = test_world();
        let player = world.player();
        let helm = armor_def(1, "helmet", ArmorCategory::Helm, -1);
        let boots = armor_def(2, "boots", ArmorCategory::Boots, -1);
        let defs = vec![helm.clone(), boots.clone()];

        {
            let equip = world.get_component::<EquipmentSlots>(player).unwrap();
            assert!(equip.is_naked());
            assert_eq!(equip.armor_count(), 0);
        }

        let helm_item = spawn_item(&mut world, &helm, SpawnLocation::Inventory, None);
        let boots_item = spawn_item(&mut world, &boots, SpawnLocation::Inventory, None);

        equip_item(&mut world, player, helm_item, EquipSlot::Helmet, &defs).unwrap();
        equip_item(&mut world, player, boots_item, EquipSlot::Boots, &defs).unwrap();

        let equip = world.get_component::<EquipmentSlots>(player).unwrap();
        assert!(!equip.is_naked());
        assert_eq!(equip.armor_count(), 2);

        let worn = equip.all_worn();
        assert_eq!(worn.len(), 2);

        assert!(wearing_helmet(&world, player));
        assert!(wearing_boots(&world, player));
        assert!(!wearing_cloak(&world, player));
        assert!(!wearing_gloves(&world, player));
        assert!(!wearing_shirt(&world, player));
    }

    // ── Name-based property detection ────────────────────────────

    #[test]
    fn test_name_props_speed_boots() {
        let props = name_based_properties("speed boots", EquipSlot::Boots);
        assert_eq!(props, vec!["speed"]);
    }

    #[test]
    fn test_name_props_levitation_boots() {
        let props = name_based_properties("levitation boots", EquipSlot::Boots);
        assert_eq!(props, vec!["levitation"]);
    }

    #[test]
    fn test_name_props_cloak_invisibility() {
        let props = name_based_properties("cloak of invisibility", EquipSlot::Cloak);
        assert_eq!(props, vec!["invisibility"]);
    }

    #[test]
    fn test_name_props_cloak_displacement() {
        let props = name_based_properties("cloak of displacement", EquipSlot::Cloak);
        assert_eq!(props, vec!["displacement"]);
    }

    #[test]
    fn test_name_props_cloak_magic_resistance() {
        let props = name_based_properties("cloak of magic resistance", EquipSlot::Cloak);
        assert_eq!(props, vec!["magic_resistance"]);
    }

    #[test]
    fn test_name_props_helm_telepathy() {
        let props = name_based_properties("helm of telepathy", EquipSlot::Helmet);
        assert_eq!(props, vec!["telepathy"]);
    }

    #[test]
    fn test_name_props_helm_brilliance() {
        let props = name_based_properties("helm of brilliance", EquipSlot::Helmet);
        assert_eq!(props, vec!["brilliance"]);
    }

    #[test]
    fn test_name_props_gauntlets_power() {
        let props = name_based_properties("gauntlets of power", EquipSlot::Gloves);
        assert_eq!(props, vec!["strength"]);
    }

    #[test]
    fn test_name_props_gauntlets_dexterity() {
        let props = name_based_properties("gauntlets of dexterity", EquipSlot::Gloves);
        assert_eq!(props, vec!["dexterity"]);
    }

    #[test]
    fn test_name_props_shield_reflection() {
        let props = name_based_properties("shield of reflection", EquipSlot::Shield);
        assert_eq!(props, vec!["reflection"]);
    }

    #[test]
    fn test_name_props_amulet_life_saving() {
        let props = name_based_properties("amulet of life saving", EquipSlot::Amulet);
        assert_eq!(props, vec!["life_saving"]);
    }

    #[test]
    fn test_name_props_ring_free_action() {
        let props = name_based_properties("ring of free action", EquipSlot::RingLeft);
        assert_eq!(props, vec!["free_action"]);
    }

    #[test]
    fn test_name_props_ring_see_invisible() {
        let props = name_based_properties("ring of see invisible", EquipSlot::RingRight);
        assert_eq!(props, vec!["see_invisible"]);
    }

    #[test]
    fn test_name_props_no_match() {
        let props = name_based_properties("ordinary boots", EquipSlot::Boots);
        assert!(props.is_empty());
    }

    // ── Wear/remove timing ───────────────────────────────────────

    #[test]
    fn test_wear_time_body_armor() {
        assert_eq!(wear_time(EquipSlot::BodyArmor), 5);
    }

    #[test]
    fn test_wear_time_shirt() {
        assert_eq!(wear_time(EquipSlot::Shirt), 5);
    }

    #[test]
    fn test_wear_time_quick() {
        assert_eq!(wear_time(EquipSlot::Cloak), 1);
        assert_eq!(wear_time(EquipSlot::Boots), 1);
        assert_eq!(wear_time(EquipSlot::Helmet), 1);
        assert_eq!(wear_time(EquipSlot::Gloves), 1);
        assert_eq!(wear_time(EquipSlot::Shield), 1);
        assert_eq!(wear_time(EquipSlot::RingLeft), 1);
        assert_eq!(wear_time(EquipSlot::Amulet), 1);
    }

    #[test]
    fn test_remove_time_body_armor() {
        assert_eq!(remove_time(EquipSlot::BodyArmor), 5);
        assert_eq!(remove_time(EquipSlot::Shirt), 5);
        assert_eq!(remove_time(EquipSlot::Cloak), 1);
    }

    // =======================================================================
    // Intrinsic change tests (ring/amulet/boot/helm/cloak/glove effects)
    // =======================================================================

    #[test]
    fn test_ring_fire_resistance_on() {
        let effects = ring_on_effect("ring of fire resistance");
        assert_eq!(effects, vec![IntrinsicChange::Gain("fire_resistance")]);
    }

    #[test]
    fn test_ring_fire_resistance_off() {
        let effects = ring_off_effect("ring of fire resistance");
        assert_eq!(effects, vec![IntrinsicChange::Lose("fire_resistance")]);
    }

    #[test]
    fn test_ring_cold_resistance() {
        let effects = ring_on_effect("ring of cold resistance");
        assert_eq!(effects, vec![IntrinsicChange::Gain("cold_resistance")]);
    }

    #[test]
    fn test_ring_poison_resistance() {
        let effects = ring_on_effect("ring of poison resistance");
        assert_eq!(effects, vec![IntrinsicChange::Gain("poison_resistance")]);
    }

    #[test]
    fn test_ring_levitation_on_off() {
        let on = ring_on_effect("ring of levitation");
        assert_eq!(on, vec![IntrinsicChange::Gain("levitation")]);
        let off = ring_off_effect("ring of levitation");
        assert_eq!(off, vec![IntrinsicChange::Lose("levitation")]);
    }

    #[test]
    fn test_ring_protection_ac_bonus() {
        let effects = ring_on_effect("ring of protection");
        assert_eq!(effects, vec![IntrinsicChange::AcBonus]);
        let off = ring_off_effect("ring of protection");
        assert_eq!(off, vec![IntrinsicChange::AcPenalty]);
    }

    #[test]
    fn test_ring_increase_damage() {
        let effects = ring_on_effect("ring of increase damage");
        assert_eq!(effects, vec![IntrinsicChange::DamageBonus]);
    }

    #[test]
    fn test_ring_increase_accuracy() {
        let effects = ring_on_effect("ring of increase accuracy");
        assert_eq!(effects, vec![IntrinsicChange::AccuracyBonus]);
    }

    #[test]
    fn test_ring_teleportation() {
        let effects = ring_on_effect("ring of teleportation");
        assert_eq!(effects, vec![IntrinsicChange::Gain("teleportitis")]);
    }

    #[test]
    fn test_ring_unknown_no_effect() {
        let effects = ring_on_effect("ring of unknown");
        assert!(effects.is_empty());
    }

    #[test]
    fn test_amulet_esp() {
        let effects = amulet_on_effect("amulet of ESP");
        assert_eq!(effects, vec![IntrinsicChange::Gain("telepathy")]);
    }

    #[test]
    fn test_amulet_life_saving() {
        let effects = amulet_on_effect("amulet of life saving");
        assert_eq!(effects, vec![IntrinsicChange::Gain("life_saving")]);
    }

    #[test]
    fn test_amulet_reflection() {
        let on = amulet_on_effect("amulet of reflection");
        assert_eq!(on, vec![IntrinsicChange::Gain("reflection")]);
        let off = amulet_off_effect("amulet of reflection");
        assert_eq!(off, vec![IntrinsicChange::Lose("reflection")]);
    }

    #[test]
    fn test_amulet_strangulation() {
        let effects = amulet_on_effect("amulet of strangulation");
        assert_eq!(effects, vec![IntrinsicChange::Gain("strangled")]);
    }

    #[test]
    fn test_amulet_versus_poison() {
        let effects = amulet_on_effect("amulet versus poison");
        assert_eq!(effects, vec![IntrinsicChange::Gain("poison_resistance")]);
    }

    #[test]
    fn test_boots_speed_on_off() {
        let on = boots_on_effect("speed boots");
        assert_eq!(on, vec![IntrinsicChange::Gain("speed")]);
        let off = boots_off_effect("speed boots");
        assert_eq!(off, vec![IntrinsicChange::Lose("speed")]);
    }

    #[test]
    fn test_boots_levitation() {
        let effects = boots_on_effect("levitation boots");
        assert_eq!(effects, vec![IntrinsicChange::Gain("levitation")]);
    }

    #[test]
    fn test_boots_jumping() {
        let effects = boots_on_effect("jumping boots");
        assert_eq!(effects, vec![IntrinsicChange::Gain("jumping")]);
    }

    #[test]
    fn test_boots_elven_stealth() {
        let effects = boots_on_effect("elven boots");
        assert_eq!(effects, vec![IntrinsicChange::Gain("stealth")]);
    }

    #[test]
    fn test_boots_fumble() {
        let effects = boots_on_effect("fumble boots");
        assert_eq!(effects, vec![IntrinsicChange::Gain("fumbling")]);
    }

    #[test]
    fn test_boots_water_walking() {
        let effects = boots_on_effect("water walking boots");
        assert_eq!(effects, vec![IntrinsicChange::Gain("water_walking")]);
    }

    #[test]
    fn test_helm_brilliance() {
        let effects = helm_on_effect("helm of brilliance");
        assert_eq!(
            effects,
            vec![
                IntrinsicChange::StatBonus("intelligence"),
                IntrinsicChange::StatBonus("wisdom"),
            ]
        );
    }

    #[test]
    fn test_helm_telepathy() {
        let effects = helm_on_effect("helm of telepathy");
        assert_eq!(effects, vec![IntrinsicChange::Gain("telepathy")]);
    }

    #[test]
    fn test_helm_opposite_alignment() {
        let effects = helm_on_effect("helm of opposite alignment");
        assert_eq!(effects, vec![IntrinsicChange::AlignmentChange]);
    }

    #[test]
    fn test_cloak_displacement() {
        let effects = cloak_on_effect("cloak of displacement");
        assert_eq!(effects, vec![IntrinsicChange::Gain("displacement")]);
    }

    #[test]
    fn test_cloak_invisibility() {
        let on = cloak_on_effect("cloak of invisibility");
        assert_eq!(on, vec![IntrinsicChange::Gain("invisibility")]);
        let off = cloak_off_effect("cloak of invisibility");
        assert_eq!(off, vec![IntrinsicChange::Lose("invisibility")]);
    }

    #[test]
    fn test_cloak_magic_resistance() {
        let effects = cloak_on_effect("cloak of magic resistance");
        assert_eq!(effects, vec![IntrinsicChange::Gain("magic_resistance")]);
    }

    #[test]
    fn test_cloak_protection_ac() {
        let on = cloak_on_effect("cloak of protection");
        assert_eq!(on, vec![IntrinsicChange::AcBonus]);
        let off = cloak_off_effect("cloak of protection");
        assert_eq!(off, vec![IntrinsicChange::AcPenalty]);
    }

    #[test]
    fn test_cloak_elven_stealth() {
        let effects = cloak_on_effect("elven cloak");
        assert_eq!(effects, vec![IntrinsicChange::Gain("stealth")]);
    }

    #[test]
    fn test_gloves_power() {
        let effects = gloves_on_effect("gauntlets of power");
        assert_eq!(effects, vec![IntrinsicChange::SetStat("strength", 25)]);
    }

    #[test]
    fn test_gloves_dexterity() {
        let effects = gloves_on_effect("gauntlets of dexterity");
        assert_eq!(effects, vec![IntrinsicChange::StatBonus("dexterity")]);
    }

    #[test]
    fn test_gloves_fumbling() {
        let effects = gloves_on_effect("gauntlets of fumbling");
        assert_eq!(effects, vec![IntrinsicChange::Gain("fumbling")]);
    }

    #[test]
    fn test_gloves_power_off() {
        let effects = gloves_off_effect("gauntlets of power");
        // Removing gauntlets of power should restore normal strength
        assert_eq!(effects, vec![IntrinsicChange::StatBonus("strength")]);
    }

    #[test]
    fn test_unknown_equipment_no_effect() {
        assert!(boots_on_effect("ordinary boots").is_empty());
        assert!(helm_on_effect("orcish helm").is_empty());
        assert!(cloak_on_effect("leather cloak").is_empty());
        assert!(gloves_on_effect("leather gloves").is_empty());
        assert!(amulet_on_effect("fake amulet").is_empty());
    }
}
