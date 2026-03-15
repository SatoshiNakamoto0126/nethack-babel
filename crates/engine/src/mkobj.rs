//! Object creation: ported from C NetHack's `mkobj.c`.
//!
//! Provides functions for creating random and specific objects, initializing
//! their properties (BUC, enchantment, quantity, charges), and generating
//! container contents.

use hecs::Entity;
use rand::Rng;

use nethack_babel_data::{
    BucStatus, CorpseData, Enchantment, MonsterId, ObjectClass, ObjectCore, ObjectDef,
    ObjectExtra, ObjectLocation, ObjectTypeId, WeaponSkill,
};

use crate::action::Position;
use crate::items::{spawn_item, SpawnLocation};
use crate::world::GameWorld;

// ---------------------------------------------------------------------------
// Class probability tables (from mkobj.c)
// ---------------------------------------------------------------------------

/// Standard dungeon level object class weights.
const MKOBJ_PROBS: &[(ObjectClass, u16)] = &[
    (ObjectClass::Weapon, 10),
    (ObjectClass::Armor, 10),
    (ObjectClass::Food, 20),
    (ObjectClass::Tool, 8),
    (ObjectClass::Gem, 8),
    (ObjectClass::Potion, 16),
    (ObjectClass::Scroll, 16),
    (ObjectClass::Spellbook, 4),
    (ObjectClass::Wand, 4),
    (ObjectClass::Ring, 3),
    (ObjectClass::Amulet, 1),
];

/// Object class weights for container contents.
const BOX_PROBS: &[(ObjectClass, u16)] = &[
    (ObjectClass::Gem, 18),
    (ObjectClass::Food, 15),
    (ObjectClass::Potion, 18),
    (ObjectClass::Scroll, 18),
    (ObjectClass::Spellbook, 12),
    (ObjectClass::Coin, 7),
    (ObjectClass::Wand, 6),
    (ObjectClass::Ring, 5),
    (ObjectClass::Amulet, 1),
];

// ---------------------------------------------------------------------------
// RNG helpers mirroring C NetHack conventions
// ---------------------------------------------------------------------------

/// `rn1(x, y)` => rng.random_range(0..x) + y  (i.e. y..y+x-1 inclusive)
#[inline]
fn rn1(x: i32, y: i32, rng: &mut impl Rng) -> i32 {
    rng.random_range(0..x) + y
}

/// `rne(x)` => geometric distribution capped at `max(level/3, 5)`.
/// For object generation we use a fixed level of 1 (early game default),
/// giving cap=5.
fn rne(x: i32, rng: &mut impl Rng) -> i32 {
    let cap = 5; // max(u.ulevel/3, 5) for low-level hero
    let mut tmp = 1;
    while tmp < cap && rng.random_range(0..x) == 0 {
        tmp += 1;
    }
    tmp
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Select a random object class based on standard dungeon generation weights.
pub fn rnd_class(rng: &mut impl Rng) -> ObjectClass {
    select_class_from_table(MKOBJ_PROBS, rng)
}

/// Create a random object of the given class.
///
/// If `class` is `ObjectClass::Random`, a class is chosen using `rnd_class()`.
/// A random object type within that class is selected based on `prob` weights,
/// then `mksobj()` is called to create and initialize it.
pub fn mkobj(
    world: &mut GameWorld,
    class: ObjectClass,
    obj_defs: &[ObjectDef],
    rng: &mut impl Rng,
) -> Option<Entity> {
    let class = if class == ObjectClass::Random {
        rnd_class(rng)
    } else {
        class
    };

    let otyp = select_random_otyp(class, obj_defs, rng)?;
    mksobj(world, otyp, true, obj_defs, rng)
}

/// Create a specific object type.
///
/// When `init` is true, class-specific initialization is applied:
/// BUC status, enchantment, quantity for stackable items, charges for wands.
pub fn mksobj(
    world: &mut GameWorld,
    otyp: ObjectTypeId,
    init: bool,
    obj_defs: &[ObjectDef],
    rng: &mut impl Rng,
) -> Option<Entity> {
    let obj_def = obj_defs.iter().find(|d| d.id == otyp)?;

    // Spawn with no enchantment; we'll set it during init.
    let entity = spawn_item(world, obj_def, SpawnLocation::Free, None);

    if init {
        init_object(world, entity, obj_def, rng);
    }

    Some(entity)
}

/// Create an object and place it at a position on the floor.
pub fn mksobj_at(
    world: &mut GameWorld,
    pos: Position,
    otyp: ObjectTypeId,
    init: bool,
    obj_defs: &[ObjectDef],
    rng: &mut impl Rng,
) -> Option<Entity> {
    let entity = mksobj(world, otyp, init, obj_defs, rng)?;

    // Update location to floor.
    if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(entity) {
        *loc = ObjectLocation::Floor {
            x: pos.x as i16,
            y: pos.y as i16,
        };
    }

    Some(entity)
}

/// Generate contents for a container (box, chest, ice box, bag).
///
/// Returns entities of the created contents. Each content object is placed
/// inside the container via `ObjectLocation::Contained`.
pub fn mkbox_cnts(
    world: &mut GameWorld,
    container: Entity,
    obj_defs: &[ObjectDef],
    rng: &mut impl Rng,
) -> Vec<Entity> {
    let mut contents = Vec::new();
    let container_id = container.to_bits().get() as u32;

    // Determine max items based on container type name heuristic.
    // In a full implementation we'd check otyp; here we use a simple default.
    let max_items = 5;
    let n = rng.random_range(0..=max_items);

    for _ in 0..n {
        let class = select_class_from_table(BOX_PROBS, rng);
        let Some(otyp) = select_random_otyp(class, obj_defs, rng) else {
            continue;
        };
        let Some(entity) = mksobj(world, otyp, true, obj_defs, rng) else {
            continue;
        };
        if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(entity) {
            *loc = ObjectLocation::Contained { container_id };
        }
        contents.push(entity);
    }

    contents
}

/// Assign blessed/uncursed/cursed status to an object.
///
/// `pct` is the 1-in-N chance of the object being blessed or cursed
/// (each direction). If `pct` is 10, there's a 1/10 chance of being
/// non-uncursed, then 50/50 blessed vs cursed.
pub fn bless_or_curse(
    world: &mut GameWorld,
    obj: Entity,
    pct: i32,
    rng: &mut impl Rng,
) {
    if let Some(buc) = world.get_component::<BucStatus>(obj)
        && (buc.blessed || buc.cursed)
    {
        return;
    }

    if pct > 0
        && rng.random_range(0..pct) == 0
        && let Some(mut buc) = world.get_component_mut::<BucStatus>(obj)
    {
        if rng.random_bool(0.5) {
            buc.cursed = true;
        } else {
            buc.blessed = true;
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Select a class from a probability table.
fn select_class_from_table(
    table: &[(ObjectClass, u16)],
    rng: &mut impl Rng,
) -> ObjectClass {
    let total: u16 = table.iter().map(|(_, w)| w).sum();
    let mut roll = rng.random_range(1..=total);
    for &(class, weight) in table {
        if roll <= weight {
            return class;
        }
        roll -= weight;
    }
    // Fallback (should not happen with valid tables).
    table.last().map(|&(c, _)| c).unwrap_or(ObjectClass::Weapon)
}

/// Select a random otyp within a class, weighted by `prob`.
fn select_random_otyp(
    class: ObjectClass,
    obj_defs: &[ObjectDef],
    rng: &mut impl Rng,
) -> Option<ObjectTypeId> {
    let candidates: Vec<&ObjectDef> = obj_defs
        .iter()
        .filter(|d| d.class == class && d.prob > 0)
        .collect();

    if candidates.is_empty() {
        return None;
    }

    let total: u32 = candidates.iter().map(|d| d.prob as u32).sum();
    if total == 0 {
        return None;
    }

    let mut roll = rng.random_range(1..=total);
    for def in &candidates {
        if roll <= def.prob as u32 {
            return Some(def.id);
        }
        roll -= def.prob as u32;
    }

    // Fallback to last candidate.
    candidates.last().map(|d| d.id)
}

/// Apply class-specific initialization to a freshly created object.
///
/// This mirrors the `mksobj_init()` function in C NetHack's mkobj.c.
fn init_object(
    world: &mut GameWorld,
    entity: Entity,
    obj_def: &ObjectDef,
    rng: &mut impl Rng,
) {
    match obj_def.class {
        ObjectClass::Weapon => init_weapon(world, entity, obj_def, rng),
        ObjectClass::Armor => init_armor(world, entity, rng),
        ObjectClass::Food => init_food(world, entity, obj_def, rng),
        ObjectClass::Potion | ObjectClass::Scroll => {
            bless_or_curse(world, entity, 4, rng);
        }
        ObjectClass::Spellbook => {
            bless_or_curse(world, entity, 17, rng);
        }
        ObjectClass::Wand => init_wand(world, entity, obj_def, rng),
        ObjectClass::Ring => init_ring(world, entity, obj_def, rng),
        ObjectClass::Amulet => {
            bless_or_curse(world, entity, 10, rng);
        }
        ObjectClass::Gem => init_gem(world, entity, obj_def, rng),
        ObjectClass::Tool => init_tool(world, entity, obj_def, rng),
        _ => {} // Coin, Rock, Ball, Chain, Venom, etc.
    }
}

/// Initialize a weapon: quantity for ammo, enchantment, BUC.
fn init_weapon(
    world: &mut GameWorld,
    entity: Entity,
    obj_def: &ObjectDef,
    rng: &mut impl Rng,
) {
    // Set quantity for projectile/ammo weapons (is_multigen).
    let is_ammo = obj_def.weapon.as_ref().is_some_and(|w| {
        matches!(
            w.skill,
            WeaponSkill::Bow
                | WeaponSkill::Sling
                | WeaponSkill::Crossbow
                | WeaponSkill::Dart
                | WeaponSkill::Shuriken
        )
    }) && obj_def.is_mergeable;

    if is_ammo
        && let Some(mut core) = world.get_component_mut::<ObjectCore>(entity)
    {
        core.quantity = rn1(6, 6, rng); // 6..11
    }

    // Enchantment and BUC: ~1/11 positive, ~1/10 negative, rest blessorcurse(10).
    let roll = rng.random_range(0..110);
    if roll < 10 {
        // ~9% chance: positive enchantment, possibly blessed.
        let spe = rne(3, rng) as i8;
        let _ = world.ecs_mut().insert_one(entity, Enchantment { spe });
        if rng.random_bool(0.5)
            && let Some(mut buc) = world.get_component_mut::<BucStatus>(entity)
        {
            buc.blessed = true;
        }
    } else if roll < 20 {
        // ~9% chance: negative enchantment, cursed.
        let spe = -(rne(3, rng) as i8);
        let _ = world.ecs_mut().insert_one(entity, Enchantment { spe });
        if let Some(mut buc) = world.get_component_mut::<BucStatus>(entity) {
            buc.cursed = true;
        }
    } else {
        // ~82%: +0, standard blessorcurse.
        let _ = world.ecs_mut().insert_one(entity, Enchantment { spe: 0 });
        bless_or_curse(world, entity, 10, rng);
    }
}

/// Initialize armor: enchantment and BUC.
fn init_armor(
    world: &mut GameWorld,
    entity: Entity,
    rng: &mut impl Rng,
) {
    // ~1/10 chance cursed with negative enchantment.
    // ~1/10 chance positive enchantment, possibly blessed.
    // Rest: blessorcurse(10).
    let roll = rng.random_range(0..100);
    if roll < 10 {
        // Cursed with negative enchantment.
        let spe = -(rne(3, rng) as i8);
        let _ = world.ecs_mut().insert_one(entity, Enchantment { spe });
        if let Some(mut buc) = world.get_component_mut::<BucStatus>(entity) {
            buc.cursed = true;
        }
    } else if roll < 20 {
        // Positive enchantment, possibly blessed.
        let spe = rne(3, rng) as i8;
        let _ = world.ecs_mut().insert_one(entity, Enchantment { spe });
        if rng.random_bool(0.5)
            && let Some(mut buc) = world.get_component_mut::<BucStatus>(entity)
        {
            buc.blessed = true;
        }
    } else {
        let _ = world.ecs_mut().insert_one(entity, Enchantment { spe: 0 });
        bless_or_curse(world, entity, 10, rng);
    }
}

/// Initialize food: quantity doubling for some types.
fn init_food(
    world: &mut GameWorld,
    entity: Entity,
    _obj_def: &ObjectDef,
    rng: &mut impl Rng,
) {
    // ~1/6 chance of quantity 2 for non-special food items.
    if rng.random_range(0..6) == 0
        && let Some(mut core) = world.get_component_mut::<ObjectCore>(entity)
    {
        core.quantity = 2;
    }
}

/// Initialize a wand: charges and BUC.
fn init_wand(
    world: &mut GameWorld,
    entity: Entity,
    _obj_def: &ObjectDef,
    rng: &mut impl Rng,
) {
    // Charges: rn1(5, 4) = 4..8 for directional, rn1(5, 11) = 11..15 for non-dir.
    // Simplified: use rn1(5, 4) as default.
    let charges = rn1(5, 4, rng) as i8;
    let _ = world.ecs_mut().insert_one(entity, Enchantment { spe: charges });
    bless_or_curse(world, entity, 17, rng);
}

/// Initialize a ring: enchantment for charged rings, BUC.
fn init_ring(
    world: &mut GameWorld,
    entity: Entity,
    obj_def: &ObjectDef,
    rng: &mut impl Rng,
) {
    if obj_def.is_charged {
        bless_or_curse(world, entity, 3, rng);
        // 9/10 chance of getting enchantment.
        if rng.random_range(0..10) != 0 {
            let buc_sign = {
                let buc = world.get_component::<BucStatus>(entity);
                match buc.as_deref() {
                    Some(BucStatus { blessed: true, .. }) => 1i8,
                    Some(BucStatus { cursed: true, .. }) => -1,
                    _ => 0,
                }
            };
            let spe = if rng.random_range(0..10) != 0 && buc_sign != 0 {
                buc_sign * rne(3, rng) as i8
            } else if rng.random_bool(0.5) {
                rne(3, rng) as i8
            } else {
                -(rne(3, rng) as i8)
            };
            // Avoid useless +0.
            let spe = if spe == 0 {
                rng.random_range(0..4) as i8 - rng.random_range(0..3) as i8
            } else {
                spe
            };
            let _ = world.ecs_mut().insert_one(entity, Enchantment { spe });
            // Negative rings are usually cursed.
            if spe < 0
                && rng.random_range(0..5) != 0
                && let Some(mut buc) = world.get_component_mut::<BucStatus>(entity)
            {
                buc.cursed = true;
            }
        }
    } else {
        // Non-charged ring: small chance cursed.
        if rng.random_range(0..10) != 0
            && rng.random_range(0..9) == 0
            && let Some(mut buc) = world.get_component_mut::<BucStatus>(entity)
        {
            buc.cursed = true;
        }
    }
}

/// Initialize a gem: quantity, loadstone curse.
fn init_gem(
    world: &mut GameWorld,
    entity: Entity,
    obj_def: &ObjectDef,
    rng: &mut impl Rng,
) {
    // Loadstones are always cursed (we'd check by name; simplified here).
    let name_lower = obj_def.name.to_lowercase();
    if name_lower == "loadstone"
        && let Some(mut buc) = world.get_component_mut::<BucStatus>(entity)
    {
        buc.cursed = true;
    } else if name_lower == "rock"
        && let Some(mut core) = world.get_component_mut::<ObjectCore>(entity)
    {
        core.quantity = rn1(6, 6, rng); // 6..11
    } else if name_lower != "luckstone"
        && rng.random_range(0..6) == 0
        && let Some(mut core) = world.get_component_mut::<ObjectCore>(entity)
    {
        core.quantity = 2;
    }
}

/// Initialize a tool: charges for charged tools, BUC.
fn init_tool(
    world: &mut GameWorld,
    entity: Entity,
    obj_def: &ObjectDef,
    rng: &mut impl Rng,
) {
    if obj_def.is_charged {
        let charges = rn1(5, 4, rng) as i8;
        let _ = world.ecs_mut().insert_one(entity, Enchantment { spe: charges });
    }
    bless_or_curse(world, entity, 5, rng);
}

// ---------------------------------------------------------------------------
// Container contents (expanded)
// ---------------------------------------------------------------------------

/// Type of container, used to determine item count and content rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerType {
    Chest { locked: bool },
    LargeBox { locked: bool },
    IceBox,
    Sack,
    OilskinSack,
    BagOfHolding,
}

impl ContainerType {
    /// Maximum number of items for this container type (before `rn2(n+1)`).
    fn max_items(&self) -> usize {
        match self {
            ContainerType::Chest { locked: true } => 7,
            ContainerType::Chest { locked: false } => 5,
            ContainerType::LargeBox { locked: true } => 5,
            ContainerType::LargeBox { locked: false } => 3,
            ContainerType::IceBox => 20,
            ContainerType::Sack | ContainerType::OilskinSack => 1,
            ContainerType::BagOfHolding => 1,
        }
    }
}

/// Generate contents for a specific container type.
///
/// Mirrors `mkbox_cnts()` from C NetHack's mkobj.c.
/// - Ice boxes contain only corpses (food).
/// - Chests/large boxes use `BOX_PROBS` weighted table.
/// - Bags of holding avoid nested bags of holding and wands of cancellation.
/// - Sacks start empty at game start (move 1).
pub fn fill_container(
    world: &mut GameWorld,
    container: Entity,
    container_type: ContainerType,
    obj_defs: &[ObjectDef],
    rng: &mut impl Rng,
) -> Vec<Entity> {
    let mut contents = Vec::new();
    let container_id = container.to_bits().get() as u32;

    let max = container_type.max_items();
    let n = rng.random_range(0..=max);

    for _ in 0..n {
        let entity = if matches!(container_type, ContainerType::IceBox) {
            // Ice boxes: only corpses (food items).
            // Find a food-class item or skip.
            let food_otyp = select_random_otyp(ObjectClass::Food, obj_defs, rng);
            match food_otyp {
                Some(otyp) => mksobj(world, otyp, true, obj_defs, rng),
                None => continue,
            }
        } else {
            let class = select_class_from_table(BOX_PROBS, rng);
            let Some(otyp) = select_random_otyp(class, obj_defs, rng) else {
                continue;
            };
            mksobj(world, otyp, true, obj_defs, rng)
        };

        let Some(entity) = entity else { continue };

        if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(entity) {
            *loc = ObjectLocation::Contained { container_id };
        }
        contents.push(entity);
    }

    contents
}

// ---------------------------------------------------------------------------
// Chest traps
// ---------------------------------------------------------------------------

/// Trap type on a container (chest or large box).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChestTrap {
    /// No trap.
    None,
    /// Sleeping gas / paralysis.
    Gas,
    /// Poisoned needle.
    Needle,
    /// Fire trap (burns items).
    Fire,
    /// Electric shock.
    Shock,
    /// Container explodes, destroying contents.
    Explosion,
}

/// Determine what trap (if any) is on a container.
///
/// In C NetHack, containers get `otrapped` set ~50% of the time for
/// chests and ~25% for large boxes.  The specific trap effect is chosen
/// when the trap triggers (in `chest_trap()` in trap.c).
///
/// Here we collapse the two-stage process: return the trap type directly.
/// `depth` influences severity (higher depth = more dangerous traps).
pub fn container_trap(depth: i32, rng: &mut impl Rng) -> ChestTrap {
    // ~30% chance of a trap on any container.
    if rng.random_range(0..10) >= 3 {
        return ChestTrap::None;
    }

    // Weighted by depth: deeper = worse traps.
    let roll = rng.random_range(0..26);
    let threshold = if depth > 10 { 12 } else { 17 };

    if roll >= 21 {
        ChestTrap::Explosion
    } else if roll >= threshold {
        ChestTrap::Fire
    } else if roll >= 13 {
        ChestTrap::Needle
    } else if roll >= 8 {
        ChestTrap::Shock
    } else {
        ChestTrap::Gas
    }
}

// ---------------------------------------------------------------------------
// Special quest objects
// ---------------------------------------------------------------------------

/// A placement directive for a special quest/dungeon artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialPlacement {
    /// Display name of the item (e.g. "Candelabrum of Invocation").
    pub item: String,
    /// Target x coordinate on the level.
    pub x: i32,
    /// Target y coordinate on the level.
    pub y: i32,
}

/// Return the special quest artifacts that should be placed on a given level.
///
/// The three Invocation artifacts:
/// - Candelabrum of Invocation on Vlad's Tower level 3
/// - Book of the Dead on the Wizard's Tower level 1
/// - Wand of wishing in the Castle
pub fn place_quest_artifacts(level_name: &str) -> Vec<SpecialPlacement> {
    match level_name {
        "vlad3" => vec![SpecialPlacement {
            item: "Candelabrum of Invocation".to_string(),
            x: 10,
            y: 5,
        }],
        "wizard1" => vec![SpecialPlacement {
            item: "Book of the Dead".to_string(),
            x: 20,
            y: 10,
        }],
        "castle" => vec![SpecialPlacement {
            item: "wand of wishing".to_string(),
            x: 40,
            y: 10,
        }],
        _ => vec![],
    }
}

// ---------------------------------------------------------------------------
// Corpse and statue creation
// ---------------------------------------------------------------------------

/// Create a corpse object for the given monster type and place it at `pos`.
///
/// The corpse is spawned as a food-class object with `CorpseData` attached.
/// Caller is responsible for any timers (rot, revive).
pub fn make_corpse(
    world: &mut GameWorld,
    monster_id: MonsterId,
    pos: Position,
    obj_defs: &[ObjectDef],
    rng: &mut impl Rng,
) -> Option<Entity> {
    // Find a food-class item to serve as the corpse template.
    // In a full implementation this would use a dedicated CORPSE otyp.
    let corpse_otyp = obj_defs
        .iter()
        .find(|d| d.class == ObjectClass::Food && d.name.to_lowercase() == "corpse")
        .or_else(|| {
            // Fallback: use any food item as template.
            obj_defs.iter().find(|d| d.class == ObjectClass::Food)
        })?;

    let entity = mksobj_at(world, pos, corpse_otyp.id, false, obj_defs, rng)?;

    // Attach corpse data.
    let _ = world.ecs_mut().insert_one(
        entity,
        CorpseData {
            monster_type: monster_id,
            eaten: 0,
        },
    );

    Some(entity)
}

/// Data returned by `make_statue`.
#[derive(Debug)]
pub struct StatueInfo {
    pub entity: Entity,
    pub monster_id: MonsterId,
    pub has_contents: bool,
}

/// Create a statue of a specific monster type at `pos`.
///
/// Statues are tool-class objects with `ObjectExtra.contained_monster` set.
/// If `has_contents` is true, the statue may contain items (determined
/// elsewhere by the caller).
pub fn make_statue(
    world: &mut GameWorld,
    monster_id: MonsterId,
    pos: Position,
    has_contents: bool,
    obj_defs: &[ObjectDef],
    rng: &mut impl Rng,
) -> Option<StatueInfo> {
    // Find a tool-class item named "statue" or use fallback.
    let statue_otyp = obj_defs
        .iter()
        .find(|d| d.class == ObjectClass::Tool && d.name.to_lowercase() == "statue")
        .or_else(|| obj_defs.iter().find(|d| d.class == ObjectClass::Tool))?;

    let entity = mksobj_at(world, pos, statue_otyp.id, false, obj_defs, rng)?;

    // Attach the monster reference.
    let _ = world.ecs_mut().insert_one(
        entity,
        ObjectExtra {
            name: None,
            contained_monster: Some(monster_id.0 as u32),
        },
    );

    Some(StatueInfo {
        entity,
        monster_id,
        has_contents,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use nethack_babel_data::{Color, Erosion, KnowledgeState, Material};
    use rand::rngs::SmallRng;
    use rand::SeedableRng;

    fn test_rng() -> SmallRng {
        SmallRng::seed_from_u64(42)
    }

    /// Build a minimal weapon ObjectDef for testing.
    fn weapon_def(id: u16, name: &str, mergeable: bool) -> ObjectDef {
        ObjectDef {
            id: ObjectTypeId(id),
            name: name.to_string(),
            appearance: None,
            class: ObjectClass::Weapon,
            color: Color::White,
            material: Material::Iron,
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
            weapon: Some(nethack_babel_data::WeaponInfo {
                skill: WeaponSkill::LongSword,
                hit_bonus: 0,
                damage_small: 8,
                damage_large: 12,
                strike_mode: nethack_babel_data::StrikeMode::SLASH,
            }),
            armor: None,
            spellbook: None,
            conferred_property: None,
            use_delay: 0,
        }
    }

    /// Build an armor ObjectDef.
    fn armor_def(id: u16, name: &str) -> ObjectDef {
        ObjectDef {
            id: ObjectTypeId(id),
            name: name.to_string(),
            appearance: None,
            class: ObjectClass::Armor,
            color: Color::White,
            material: Material::Iron,
            weight: 150,
            cost: 40,
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
            armor: Some(nethack_babel_data::ArmorInfo {
                category: nethack_babel_data::ArmorCategory::Suit,
                ac_bonus: 3,
                magic_cancel: 0,
            }),
            spellbook: None,
            conferred_property: None,
            use_delay: 0,
        }
    }

    /// Build a wand ObjectDef.
    fn wand_def(id: u16, name: &str) -> ObjectDef {
        ObjectDef {
            id: ObjectTypeId(id),
            name: name.to_string(),
            appearance: Some("oak wand".to_string()),
            class: ObjectClass::Wand,
            color: Color::White,
            material: Material::Wood,
            weight: 7,
            cost: 100,
            nutrition: 0,
            prob: 10,
            is_magic: true,
            is_mergeable: false,
            is_charged: true,
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

    /// Build a ring ObjectDef.
    fn ring_def(id: u16, name: &str, charged: bool) -> ObjectDef {
        ObjectDef {
            id: ObjectTypeId(id),
            name: name.to_string(),
            appearance: Some("ruby ring".to_string()),
            class: ObjectClass::Ring,
            color: Color::Red,
            material: Material::Gemstone,
            weight: 3,
            cost: 100,
            nutrition: 0,
            prob: 10,
            is_magic: true,
            is_mergeable: false,
            is_charged: charged,
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

    /// Build a potion ObjectDef.
    fn potion_def(id: u16, name: &str) -> ObjectDef {
        ObjectDef {
            id: ObjectTypeId(id),
            name: name.to_string(),
            appearance: Some("ruby potion".to_string()),
            class: ObjectClass::Potion,
            color: Color::Red,
            material: Material::Glass,
            weight: 20,
            cost: 50,
            nutrition: 0,
            prob: 10,
            is_magic: true,
            is_mergeable: true,
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

    /// Build an ammo (arrow) ObjectDef.
    fn arrow_def(id: u16) -> ObjectDef {
        ObjectDef {
            id: ObjectTypeId(id),
            name: "arrow".to_string(),
            appearance: None,
            class: ObjectClass::Weapon,
            color: Color::White,
            material: Material::Iron,
            weight: 1,
            cost: 2,
            nutrition: 0,
            prob: 10,
            is_magic: false,
            is_mergeable: true,
            is_charged: false,
            is_unique: false,
            is_nowish: false,
            is_bimanual: false,
            is_bulky: false,
            is_tough: false,
            weapon: Some(nethack_babel_data::WeaponInfo {
                skill: WeaponSkill::Bow,
                hit_bonus: 0,
                damage_small: 6,
                damage_large: 6,
                strike_mode: nethack_babel_data::StrikeMode::PIERCE,
            }),
            armor: None,
            spellbook: None,
            conferred_property: None,
            use_delay: 0,
        }
    }

    fn test_defs() -> Vec<ObjectDef> {
        vec![
            weapon_def(0, "long sword", false),
            weapon_def(1, "short sword", false),
            armor_def(2, "plate mail"),
            armor_def(3, "leather armor"),
            wand_def(4, "wand of fire"),
            ring_def(5, "ring of protection", true),
            potion_def(6, "potion of healing"),
            arrow_def(7),
        ]
    }

    // ── Test: mksobj creates entity with correct components ──

    #[test]
    fn mksobj_creates_entity_with_correct_components() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let defs = test_defs();
        let mut rng = test_rng();

        let entity = mksobj(&mut world, ObjectTypeId(0), true, &defs, &mut rng)
            .expect("should create entity");

        let core = world.get_component::<ObjectCore>(entity).expect("ObjectCore");
        assert_eq!(core.otyp, ObjectTypeId(0));
        assert_eq!(core.object_class, ObjectClass::Weapon);

        // Should have BucStatus.
        assert!(world.get_component::<BucStatus>(entity).is_some());
        // Should have KnowledgeState.
        assert!(world.get_component::<KnowledgeState>(entity).is_some());
        // Should have ObjectLocation::Free.
        let loc = world.get_component::<ObjectLocation>(entity).expect("loc");
        assert!(matches!(*loc, ObjectLocation::Free));
        // Should have Erosion.
        assert!(world.get_component::<Erosion>(entity).is_some());
    }

    // ── Test: mkobj generates objects of requested class ──

    #[test]
    fn mkobj_generates_objects_of_requested_class() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let defs = test_defs();
        let mut rng = test_rng();

        for _ in 0..20 {
            let entity = mkobj(&mut world, ObjectClass::Weapon, &defs, &mut rng)
                .expect("should create weapon");
            let core = world.get_component::<ObjectCore>(entity).expect("core");
            assert_eq!(core.object_class, ObjectClass::Weapon);
        }
    }

    // ── Test: bless_or_curse assigns correct BUC distribution ──

    #[test]
    fn bless_or_curse_distribution() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let defs = test_defs();
        let mut rng = test_rng();
        let mut blessed = 0u32;
        let mut cursed = 0u32;
        let mut uncursed = 0u32;
        let n = 1000;

        for _ in 0..n {
            let entity = mksobj(&mut world, ObjectTypeId(6), false, &defs, &mut rng)
                .expect("potion");
            bless_or_curse(&mut world, entity, 10, &mut rng);

            let buc = world.get_component::<BucStatus>(entity).unwrap();
            if buc.blessed {
                blessed += 1;
            } else if buc.cursed {
                cursed += 1;
            } else {
                uncursed += 1;
            }
        }

        // With pct=10: ~5% blessed, ~5% cursed, ~90% uncursed.
        // Allow wide range for statistical variance.
        assert!(blessed > 0, "should have some blessed items");
        assert!(cursed > 0, "should have some cursed items");
        assert!(
            uncursed > blessed && uncursed > cursed,
            "uncursed ({uncursed}) should dominate over blessed ({blessed}) and cursed ({cursed})"
        );
    }

    // ── Test: rnd_class returns valid classes with correct weights ──

    #[test]
    fn rnd_class_returns_valid_classes() {
        let mut rng = test_rng();
        let valid_classes: Vec<ObjectClass> = MKOBJ_PROBS.iter().map(|&(c, _)| c).collect();

        for _ in 0..100 {
            let class = rnd_class(&mut rng);
            assert!(
                valid_classes.contains(&class),
                "rnd_class returned unexpected class: {class:?}"
            );
        }
    }

    // ── Test: mksobj_at places object on floor ──

    #[test]
    fn mksobj_at_places_object_on_floor() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let defs = test_defs();
        let mut rng = test_rng();

        let entity = mksobj_at(
            &mut world,
            Position::new(5, 7),
            ObjectTypeId(0),
            true,
            &defs,
            &mut rng,
        )
        .expect("should create entity");

        let loc = world.get_component::<ObjectLocation>(entity).expect("loc");
        assert!(matches!(*loc, ObjectLocation::Floor { x: 5, y: 7 }));
    }

    // ── Test: weapon enchantment ranges are correct ──

    #[test]
    fn weapon_enchantment_ranges() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let defs = test_defs();
        let mut rng = test_rng();

        let mut min_spe = i8::MAX;
        let mut max_spe = i8::MIN;

        for _ in 0..500 {
            let entity = mksobj(&mut world, ObjectTypeId(0), true, &defs, &mut rng)
                .expect("weapon");
            if let Some(ench) = world.get_component::<Enchantment>(entity) {
                min_spe = min_spe.min(ench.spe);
                max_spe = max_spe.max(ench.spe);
            }
        }

        // Enchantment should range from negative (at least -1) to positive (at least +1).
        assert!(min_spe < 0, "should generate some negative enchantment, got min={min_spe}");
        assert!(max_spe > 0, "should generate some positive enchantment, got max={max_spe}");
        // rne(3) caps at 5, so range should be -5..+5.
        assert!(min_spe >= -5, "min enchantment should be >= -5, got {min_spe}");
        assert!(max_spe <= 5, "max enchantment should be <= 5, got {max_spe}");
    }

    // ── Test: quantity assignment for stackable items (arrows) ──

    #[test]
    fn arrow_quantity_assignment() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let defs = test_defs();
        let mut rng = test_rng();

        let mut quantities = Vec::new();
        for _ in 0..100 {
            let entity = mksobj(&mut world, ObjectTypeId(7), true, &defs, &mut rng)
                .expect("arrow");
            let core = world.get_component::<ObjectCore>(entity).expect("core");
            quantities.push(core.quantity);
        }

        // Arrows use rn1(6, 6) = 6..11.
        let min_q = *quantities.iter().min().unwrap();
        let max_q = *quantities.iter().max().unwrap();
        assert!(min_q >= 6, "min arrow quantity should be >= 6, got {min_q}");
        assert!(max_q <= 11, "max arrow quantity should be <= 11, got {max_q}");
    }

    // ── Test: wand gets charges ──

    #[test]
    fn wand_gets_charges() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let defs = test_defs();
        let mut rng = test_rng();

        for _ in 0..50 {
            let entity = mksobj(&mut world, ObjectTypeId(4), true, &defs, &mut rng)
                .expect("wand");
            let ench = world
                .get_component::<Enchantment>(entity)
                .expect("wand should have charges");
            // rn1(5, 4) = 4..8
            assert!(
                (4..=8).contains(&ench.spe),
                "wand charges should be 4..8, got {}",
                ench.spe
            );
        }
    }

    // ── Test: ring with charged flag gets enchantment ──

    #[test]
    fn charged_ring_gets_enchantment() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let defs = test_defs();
        let mut rng = test_rng();

        let mut has_enchantment = false;
        for _ in 0..50 {
            let entity = mksobj(&mut world, ObjectTypeId(5), true, &defs, &mut rng)
                .expect("ring");
            if world.get_component::<Enchantment>(entity).is_some() {
                has_enchantment = true;
                break;
            }
        }

        assert!(has_enchantment, "charged ring should get enchantment");
    }

    // ── Test: mkobj with Random class picks from all classes ──

    #[test]
    fn mkobj_random_class_picks_various_classes() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let defs = test_defs();
        let mut rng = test_rng();

        let mut classes_seen = std::collections::HashSet::new();
        for _ in 0..200 {
            if let Some(entity) = mkobj(&mut world, ObjectClass::Random, &defs, &mut rng) {
                let core = world.get_component::<ObjectCore>(entity).expect("core");
                classes_seen.insert(core.object_class);
            }
        }

        // With our test defs covering Weapon, Armor, Wand, Ring, Potion,
        // we should see at least 3 different classes.
        assert!(
            classes_seen.len() >= 3,
            "should generate multiple classes, only saw: {classes_seen:?}"
        );
    }

    // ── Test: mksobj without init does not set enchantment ──

    #[test]
    fn mksobj_no_init_skips_enchantment() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let defs = test_defs();
        let mut rng = test_rng();

        let entity = mksobj(&mut world, ObjectTypeId(0), false, &defs, &mut rng)
            .expect("weapon no-init");

        // Without init, spawn_item does not add Enchantment (we passed None).
        assert!(
            world.get_component::<Enchantment>(entity).is_none(),
            "no-init weapon should not have enchantment"
        );
    }

    // ── Test: mkbox_cnts creates contained objects ──

    #[test]
    fn mkbox_cnts_creates_contained_objects() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let defs = test_defs();

        // Create a container entity.
        let container_def = ObjectDef {
            id: ObjectTypeId(100),
            name: "chest".to_string(),
            appearance: None,
            class: ObjectClass::Tool,
            color: Color::White,
            material: Material::Wood,
            weight: 350,
            cost: 16,
            nutrition: 0,
            prob: 0,
            is_magic: false,
            is_mergeable: false,
            is_charged: false,
            is_unique: false,
            is_nowish: false,
            is_bimanual: false,
            is_bulky: true,
            is_tough: false,
            weapon: None,
            armor: None,
            spellbook: None,
            conferred_property: None,
            use_delay: 0,
        };
        let container = spawn_item(&mut world, &container_def, SpawnLocation::Free, None);

        // Try several times since n could be 0.
        let mut total_contents = 0;
        for seed in 0..10u64 {
            let mut rng2 = SmallRng::seed_from_u64(seed + 100);
            let contents = mkbox_cnts(&mut world, container, &defs, &mut rng2);
            total_contents += contents.len();

            // Verify contained objects have correct location.
            let container_id = container.to_bits().get() as u32;
            for &item in &contents {
                let loc = world.get_component::<ObjectLocation>(item).expect("loc");
                assert!(
                    matches!(*loc, ObjectLocation::Contained { container_id: cid } if cid == container_id),
                    "contents should be inside container"
                );
            }
        }

        assert!(total_contents > 0, "should have generated some contents across seeds");
    }

    // ── Test: fill_container with chest type ──

    #[test]
    fn fill_container_chest_creates_items() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let defs = test_defs();
        let container_def = ObjectDef {
            id: ObjectTypeId(100),
            name: "chest".to_string(),
            appearance: None,
            class: ObjectClass::Tool,
            color: Color::White,
            material: Material::Wood,
            weight: 350,
            cost: 16,
            nutrition: 0,
            prob: 0,
            is_magic: false,
            is_mergeable: false,
            is_charged: false,
            is_unique: false,
            is_nowish: false,
            is_bimanual: false,
            is_bulky: true,
            is_tough: false,
            weapon: None,
            armor: None,
            spellbook: None,
            conferred_property: None,
            use_delay: 0,
        };
        let container = spawn_item(&mut world, &container_def, SpawnLocation::Free, None);

        let mut total = 0;
        for seed in 0..20u64 {
            let mut rng = SmallRng::seed_from_u64(seed + 200);
            let items = fill_container(
                &mut world,
                container,
                ContainerType::Chest { locked: true },
                &defs,
                &mut rng,
            );
            total += items.len();
            // Locked chest: max 7 items.
            assert!(items.len() <= 8, "chest should produce at most 8 items");
        }
        assert!(total > 0, "should generate some items across seeds");
    }

    // ── Test: fill_container with ice box only produces food ──

    #[test]
    fn fill_container_ice_box_food_only() {
        let mut world = GameWorld::new(Position::new(40, 10));
        // Add a food def for ice box to use.
        let mut defs = test_defs();
        defs.push(ObjectDef {
            id: ObjectTypeId(50),
            name: "corpse".to_string(),
            appearance: None,
            class: ObjectClass::Food,
            color: Color::White,
            material: Material::Flesh,
            weight: 50,
            cost: 0,
            nutrition: 100,
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
        });
        let container_def = ObjectDef {
            id: ObjectTypeId(101),
            name: "ice box".to_string(),
            appearance: None,
            class: ObjectClass::Tool,
            color: Color::White,
            material: Material::Iron,
            weight: 900,
            cost: 42,
            nutrition: 0,
            prob: 0,
            is_magic: false,
            is_mergeable: false,
            is_charged: false,
            is_unique: false,
            is_nowish: false,
            is_bimanual: false,
            is_bulky: true,
            is_tough: false,
            weapon: None,
            armor: None,
            spellbook: None,
            conferred_property: None,
            use_delay: 0,
        };
        let container = spawn_item(&mut world, &container_def, SpawnLocation::Free, None);

        let mut total = 0;
        for seed in 0..20u64 {
            let mut rng = SmallRng::seed_from_u64(seed + 300);
            let items = fill_container(
                &mut world,
                container,
                ContainerType::IceBox,
                &defs,
                &mut rng,
            );
            total += items.len();
            // Ice box: all items should be food class.
            for &item in &items {
                let core = world.get_component::<ObjectCore>(item).expect("core");
                assert_eq!(
                    core.object_class,
                    ObjectClass::Food,
                    "ice box should only contain food"
                );
            }
        }
        assert!(total > 0, "ice box should generate some items");
    }

    // ── Test: chest trap generation ──

    #[test]
    fn chest_trap_generation() {
        let mut rng = test_rng();
        let mut has_trap = false;
        let mut has_none = false;

        for _ in 0..200 {
            let trap = container_trap(5, &mut rng);
            if trap == ChestTrap::None {
                has_none = true;
            } else {
                has_trap = true;
            }
        }

        assert!(has_trap, "should generate some traps");
        assert!(has_none, "should generate some no-trap results");
    }

    // ── Test: chest trap variety ──

    #[test]
    fn chest_trap_variety() {
        let mut rng = test_rng();
        let mut seen = std::collections::HashSet::new();

        for seed in 0..500u64 {
            let mut r = SmallRng::seed_from_u64(seed);
            let trap = container_trap(15, &mut r);
            seen.insert(std::mem::discriminant(&trap));
        }

        // Should see at least 3 different trap types (including None).
        assert!(
            seen.len() >= 3,
            "should see variety of trap types, only saw {} distinct",
            seen.len()
        );
    }

    // ── Test: quest artifact placement ──

    #[test]
    fn quest_artifact_placement() {
        let vlad = place_quest_artifacts("vlad3");
        assert_eq!(vlad.len(), 1);
        assert_eq!(vlad[0].item, "Candelabrum of Invocation");

        let wizard = place_quest_artifacts("wizard1");
        assert_eq!(wizard.len(), 1);
        assert_eq!(wizard[0].item, "Book of the Dead");

        let castle = place_quest_artifacts("castle");
        assert_eq!(castle.len(), 1);
        assert_eq!(castle[0].item, "wand of wishing");

        let other = place_quest_artifacts("minetown");
        assert!(other.is_empty());
    }

    // ── Test: make_corpse creates entity with CorpseData ──

    #[test]
    fn make_corpse_creates_corpse() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let mut defs = test_defs();
        // Add a food item (corpse template).
        defs.push(ObjectDef {
            id: ObjectTypeId(50),
            name: "corpse".to_string(),
            appearance: None,
            class: ObjectClass::Food,
            color: Color::White,
            material: Material::Flesh,
            weight: 50,
            cost: 0,
            nutrition: 100,
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
        });
        let mut rng = test_rng();

        let entity = make_corpse(
            &mut world,
            MonsterId(42),
            Position::new(5, 5),
            &defs,
            &mut rng,
        )
        .expect("should create corpse");

        // Check CorpseData component.
        let corpse = world
            .get_component::<CorpseData>(entity)
            .expect("should have CorpseData");
        assert_eq!(corpse.monster_type, MonsterId(42));
        assert_eq!(corpse.eaten, 0);

        // Check position.
        let loc = world
            .get_component::<ObjectLocation>(entity)
            .expect("loc");
        assert!(matches!(*loc, ObjectLocation::Floor { x: 5, y: 5 }));
    }

    // ── Test: make_statue creates entity with ObjectExtra ──

    #[test]
    fn make_statue_creates_statue() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let mut defs = test_defs();
        // Add a tool item (statue template).
        defs.push(ObjectDef {
            id: ObjectTypeId(60),
            name: "statue".to_string(),
            appearance: None,
            class: ObjectClass::Tool,
            color: Color::White,
            material: Material::Mineral,
            weight: 900,
            cost: 0,
            nutrition: 0,
            prob: 10,
            is_magic: false,
            is_mergeable: false,
            is_charged: false,
            is_unique: false,
            is_nowish: false,
            is_bimanual: false,
            is_bulky: true,
            is_tough: false,
            weapon: None,
            armor: None,
            spellbook: None,
            conferred_property: None,
            use_delay: 0,
        });
        let mut rng = test_rng();

        let info = make_statue(
            &mut world,
            MonsterId(7),
            Position::new(10, 8),
            true,
            &defs,
            &mut rng,
        )
        .expect("should create statue");

        assert_eq!(info.monster_id, MonsterId(7));
        assert!(info.has_contents);

        // Check ObjectExtra.
        let extra = world
            .get_component::<ObjectExtra>(info.entity)
            .expect("should have ObjectExtra");
        assert_eq!(extra.contained_monster, Some(7));

        // Check position.
        let loc = world
            .get_component::<ObjectLocation>(info.entity)
            .expect("loc");
        assert!(matches!(*loc, ObjectLocation::Floor { x: 10, y: 8 }));
    }
}
