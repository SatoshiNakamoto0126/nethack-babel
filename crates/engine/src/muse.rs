//! Monster item use (from C `muse.c`).
//!
//! Handles monsters using items from their inventory: offensive items
//! (wands, thrown potions, scrolls), defensive items (healing, escape),
//! and miscellaneous items (armor, weapons, unicorn horns).
//!
//! All functions are pure: they operate on `GameWorld` plus RNG, mutate
//! world state, and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;
use rand::Rng;

use nethack_babel_data::{ObjectClass, ObjectCore, ObjectLocation};

use crate::event::{EngineEvent, HpSource, StatusEffect};
use crate::monster_ai::{PotionTypeTag, WandTypeTag};
use crate::potions::PotionType;
use crate::status::StatusEffects;
use crate::wands::{WandCharges, WandType};
use crate::world::{GameWorld, HitPoints, Positioned};

// ---------------------------------------------------------------------------
// Monster item use decision tree
// ---------------------------------------------------------------------------

/// Priority-ordered monster item use decisions.
/// Monsters check these in order when deciding what to do.
///
/// Mirrors C NetHack's `find_defensive()`, `find_offensive()`, and
/// `find_misc()` from `muse.c`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonsterItemAction {
    // Escape (highest priority when low HP)
    QuaffHealingPotion { item: Entity },
    ReadTeleportScroll,
    ZapTeleportWand { item: Entity },
    UseEscapeItem { item: Entity },

    // Offensive
    ZapAttackWand { item: Entity, wand_type: WandType },
    ThrowPotion { item: Entity, potion_type: PotionType },

    // Defensive
    WearBetterArmor { item: Entity },
    WieldBetterWeapon { item: Entity },
    QuaffSpeedPotion { item: Entity },

    // Utility
    UseUnicornHorn { item: Entity },

    NoAction,
}

/// Decide what item action a monster should take.
///
/// Checks HP, status ailments, threats, and available items in priority
/// order, matching the C decision tree from `muse.c`:
/// 1. Cure status ailments (unicorn horn)
/// 2. Heal when hurt (healing potions, priority: full > extra > regular)
/// 3. Escape when fleeing and low HP (teleport wand)
/// 4. Attack when has target (offensive wands, then thrown potions)
/// 5. Self-buff (speed potion)
/// 6. Equip better weapon/armor
pub fn monster_item_decision(
    world: &GameWorld,
    monster: Entity,
    is_fleeing: bool,
    has_target: bool,
) -> MonsterItemAction {
    // Check for status ailments that unicorn horn can cure.
    let has_status_ailment = world
        .get_component::<StatusEffects>(monster)
        .map(|s| s.confusion > 0 || s.stun > 0 || s.blindness > 0)
        .unwrap_or(false);

    if has_status_ailment {
        if let Some(horn) = find_unicorn_horn(world, monster) {
            return MonsterItemAction::UseUnicornHorn { item: horn };
        }
    }

    // Check HP ratio for healing/escape decisions.
    let (current_hp, max_hp) = world
        .get_component::<HitPoints>(monster)
        .map(|hp| (hp.current, hp.max))
        .unwrap_or((1, 1));
    let hp_ratio = if max_hp > 0 {
        (current_hp as f32) / (max_hp as f32)
    } else {
        1.0
    };

    // Low HP: try healing first (priority: full > extra > regular).
    if hp_ratio < 0.5 {
        if let Some(item) = find_healing_item(world, monster) {
            return MonsterItemAction::QuaffHealingPotion { item };
        }
    }

    // Fleeing and hurt: try escape via teleport wand.
    if is_fleeing && hp_ratio < 0.33 {
        if let Some(item) = find_escape_item(world, monster) {
            return MonsterItemAction::ZapTeleportWand { item };
        }
    }

    // Has target: try offensive items.
    if has_target {
        if let Some((item, wtype)) = find_attack_wand(world, monster) {
            return MonsterItemAction::ZapAttackWand {
                item,
                wand_type: wtype,
            };
        }
        if let Some((item, ptype)) = find_attack_potion(world, monster) {
            return MonsterItemAction::ThrowPotion {
                item,
                potion_type: ptype,
            };
        }
    }

    // Self-buff: speed potion.
    if let Some(item) = find_speed_potion(world, monster) {
        return MonsterItemAction::QuaffSpeedPotion { item };
    }

    // Equip better weapon.
    if let Some(item) = find_best_weapon(world, monster) {
        return MonsterItemAction::WieldBetterWeapon { item };
    }

    // Equip better armor.
    if let Some(item) = find_best_armor(world, monster) {
        return MonsterItemAction::WearBetterArmor { item };
    }

    MonsterItemAction::NoAction
}

/// Whether a monster should use an item now based on HP ratio and threat.
///
/// Matches C NetHack's threshold: monsters use healing when HP drops
/// below fraction of max (fraction = 5 at low levels, 3 at high levels).
pub fn should_use_item_now(hp_ratio: f32, threat_level: u32) -> bool {
    match threat_level {
        0 => hp_ratio < 0.2,       // No threat: only at critical HP
        1 => hp_ratio < 0.33,      // Low threat
        2..=3 => hp_ratio < 0.5,   // Medium threat
        _ => hp_ratio < 0.75,      // High threat: more willing to use items
    }
}

// ---------------------------------------------------------------------------
// Item scanning: specific searches for the decision tree
// ---------------------------------------------------------------------------

/// Find a unicorn horn in the monster's inventory.
///
/// Unicorn horns cure confusion, stunning, and blindness.  Matches C
/// NetHack's `find_defensive()` unicorn horn logic.
fn find_unicorn_horn(
    world: &GameWorld,
    monster: Entity,
) -> Option<Entity> {
    let items = get_monster_inventory(world, monster);
    items.iter().copied().find(|&item| {
        world
            .get_component::<ObjectCore>(item)
            .is_some_and(|core| core.object_class == ObjectClass::Tool)
            && world
                .get_component::<UnicornHornTag>(item)
                .is_some()
    })
}

/// Find the best healing potion (full > extra > regular).
///
/// Matches C NetHack's `m_use_healing()` priority.
fn find_healing_item(
    world: &GameWorld,
    monster: Entity,
) -> Option<Entity> {
    let items = get_monster_inventory(world, monster);
    let priority = [
        PotionType::FullHealing,
        PotionType::ExtraHealing,
        PotionType::Healing,
    ];
    for &ptype in &priority {
        if let Some(potion) = find_potion_in_list(&items, world, ptype) {
            return Some(potion);
        }
    }
    None
}

/// Find a teleportation escape item (wand of teleportation with charges).
fn find_escape_item(
    world: &GameWorld,
    monster: Entity,
) -> Option<Entity> {
    let items = get_monster_inventory(world, monster);
    if let Some(wand) = find_wand_in_list(&items, world, WandType::Teleportation) {
        let has_charges = world
            .get_component::<WandCharges>(wand)
            .map(|c| c.spe > 0)
            .unwrap_or(false);
        if has_charges {
            return Some(wand);
        }
    }
    None
}

/// Find the best offensive wand with charges.
///
/// Priority: Death > Sleep > Fire > Cold > Lightning > MagicMissile.
/// Matches C NetHack's `find_offensive()` wand scanning.
fn find_attack_wand(
    world: &GameWorld,
    monster: Entity,
) -> Option<(Entity, WandType)> {
    let items = get_monster_inventory(world, monster);
    let wand_priority = [
        WandType::Death,
        WandType::Sleep,
        WandType::Fire,
        WandType::Cold,
        WandType::Lightning,
        WandType::MagicMissile,
    ];
    for &wtype in &wand_priority {
        if let Some(wand) = find_wand_in_list(&items, world, wtype) {
            let has_charges = world
                .get_component::<WandCharges>(wand)
                .map(|c| c.spe > 0)
                .unwrap_or(false);
            if has_charges {
                return Some((wand, wtype));
            }
        }
    }
    None
}

/// Find an offensive potion to throw at the player.
///
/// Priority: Paralysis > Blindness > Confusion > Sleeping > Acid.
fn find_attack_potion(
    world: &GameWorld,
    monster: Entity,
) -> Option<(Entity, PotionType)> {
    let items = get_monster_inventory(world, monster);
    let potion_priority = [
        PotionType::Paralysis,
        PotionType::Blindness,
        PotionType::Confusion,
        PotionType::Sleeping,
        PotionType::Acid,
    ];
    for &ptype in &potion_priority {
        if let Some(potion) = find_potion_in_list(&items, world, ptype) {
            return Some((potion, ptype));
        }
    }
    None
}

/// Find a speed potion for self-buffing.
fn find_speed_potion(
    world: &GameWorld,
    monster: Entity,
) -> Option<Entity> {
    let items = get_monster_inventory(world, monster);
    find_potion_in_list(&items, world, PotionType::Speed)
}

/// Marker component for unicorn horn objects.
#[derive(Debug, Clone)]
pub struct UnicornHornTag;

// ---------------------------------------------------------------------------
// Inventory scanning helpers
// ---------------------------------------------------------------------------

/// Get all item entities carried by a monster.
fn get_monster_inventory(world: &GameWorld, monster: Entity) -> Vec<Entity> {
    let carrier_id = monster.to_bits().get() as u32;
    world
        .query::<ObjectCore>()
        .iter()
        .filter(|&(entity, _)| {
            world
                .get_component::<ObjectLocation>(entity)
                .is_some_and(|loc| {
                    matches!(*loc, ObjectLocation::MonsterInventory { carrier_id: cid, .. }
                        if cid == carrier_id)
                })
        })
        .map(|(entity, _)| entity)
        .collect()
}

/// Find a wand of the given type in the item list.
fn find_wand_in_list(
    items: &[Entity],
    world: &GameWorld,
    wand_type: WandType,
) -> Option<Entity> {
    items.iter().copied().find(|&item| {
        world
            .get_component::<ObjectCore>(item)
            .is_some_and(|core| core.object_class == ObjectClass::Tool)
            && world
                .get_component::<WandTypeTag>(item)
                .is_some_and(|tag| tag.0 == wand_type)
    })
}

/// Find a potion of the given type in the item list.
fn find_potion_in_list(
    items: &[Entity],
    world: &GameWorld,
    potion_type: PotionType,
) -> Option<Entity> {
    items.iter().copied().find(|&item| {
        world
            .get_component::<ObjectCore>(item)
            .is_some_and(|core| core.object_class == ObjectClass::Potion)
            && world
                .get_component::<PotionTypeTag>(item)
                .is_some_and(|tag| tag.0 == potion_type)
    })
}

// ---------------------------------------------------------------------------
// Item scanning: offensive, defensive, misc
// ---------------------------------------------------------------------------

/// Scan a monster's inventory for the best offensive item.
///
/// Priority: Wand of Death > Sleep > Fire > Cold > Lightning > MagicMissile,
/// then offensive potions (paralysis, blindness, confusion, acid).
pub fn find_offensive_item(
    world: &GameWorld,
    monster: Entity,
) -> Option<Entity> {
    let items = get_monster_inventory(world, monster);

    // Wand priority.
    let wand_priority = [
        WandType::Death,
        WandType::Sleep,
        WandType::Fire,
        WandType::Cold,
        WandType::Lightning,
        WandType::MagicMissile,
    ];
    for &wtype in &wand_priority {
        if let Some(wand) = find_wand_in_list(&items, world, wtype) {
            let has_charges = world
                .get_component::<WandCharges>(wand)
                .map(|c| c.spe > 0)
                .unwrap_or(false);
            if has_charges {
                return Some(wand);
            }
        }
    }

    // Offensive potion priority.
    let potion_priority = [
        PotionType::Paralysis,
        PotionType::Blindness,
        PotionType::Confusion,
        PotionType::Acid,
    ];
    for &ptype in &potion_priority {
        if let Some(potion) = find_potion_in_list(&items, world, ptype) {
            return Some(potion);
        }
    }

    None
}

/// Scan a monster's inventory for the best defensive item.
///
/// Priority: Potion of Full Healing > Extra Healing > Healing > Speed >
/// Invisibility, then wands of teleportation/digging.
pub fn find_defensive_item(
    world: &GameWorld,
    monster: Entity,
) -> Option<Entity> {
    let items = get_monster_inventory(world, monster);

    // Healing potion priority.
    let heal_priority = [
        PotionType::FullHealing,
        PotionType::ExtraHealing,
        PotionType::Healing,
        PotionType::Speed,
        PotionType::Invisibility,
    ];
    for &ptype in &heal_priority {
        if let Some(potion) = find_potion_in_list(&items, world, ptype) {
            return Some(potion);
        }
    }

    // Defensive wand priority.
    let wand_priority = [
        WandType::Teleportation,
    ];
    for &wtype in &wand_priority {
        if let Some(wand) = find_wand_in_list(&items, world, wtype) {
            let has_charges = world
                .get_component::<WandCharges>(wand)
                .map(|c| c.spe > 0)
                .unwrap_or(false);
            if has_charges {
                return Some(wand);
            }
        }
    }

    None
}

/// Scan a monster's inventory for miscellaneous usable items.
///
/// Looks for speed/invisibility potions (for self-buffing).
pub fn find_misc_item(
    world: &GameWorld,
    monster: Entity,
) -> Option<Entity> {
    let items = get_monster_inventory(world, monster);

    let misc_potions = [
        PotionType::Speed,
        PotionType::Invisibility,
    ];
    for &ptype in &misc_potions {
        if let Some(potion) = find_potion_in_list(&items, world, ptype) {
            return Some(potion);
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Item use: offensive
// ---------------------------------------------------------------------------

/// Monster uses an offensive item against the player.
///
/// Handles wand zapping (fire, cold, death, sleep, magic missile, striking)
/// and potion throwing (paralysis, blindness, confusion, acid).
pub fn use_offensive_item(
    world: &mut GameWorld,
    monster: Entity,
    item: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let player = world.player();
    let mon_name = world.entity_name(monster);

    // Copy tag values to release borrows before mutating world.
    let wand_type = world.get_component::<WandTypeTag>(item).map(|t| t.0);
    let potion_type = world.get_component::<PotionTypeTag>(item).map(|t| t.0);

    // Check if it's a wand.
    if let Some(wtype) = wand_type {
        let has_charges = world
            .get_component::<WandCharges>(item)
            .map(|c| c.spe > 0)
            .unwrap_or(false);
        if !has_charges {
            return events;
        }

        // Decrement charges.
        if let Some(mut charges) = world.get_component_mut::<WandCharges>(item) {
            charges.spe -= 1;
        }

        events.push(EngineEvent::msg_with(
            "monster-zaps-wand",
            vec![
                ("monster", mon_name.clone()),
                ("wand_type", format!("{wtype:?}")),
            ],
        ));

        let damage = match wtype {
            WandType::Death => 999i32,
            WandType::Sleep => {
                let dur = rng.random_range(1u32..=25);
                events.push(EngineEvent::StatusApplied {
                    entity: player,
                    status: StatusEffect::Sleeping,
                    duration: Some(dur),
                    source: Some(monster),
                });
                0
            }
            WandType::Fire | WandType::Cold | WandType::Lightning => {
                let nd = wtype.ray_nd();
                let total: i32 =
                    (0..nd).map(|_| rng.random_range(1i32..=6)).sum();
                total
            }
            WandType::MagicMissile => {
                let nd = wtype.ray_nd();
                let total: i32 =
                    (0..nd).map(|_| rng.random_range(1i32..=6)).sum();
                total
            }
            _ => 0,
        };

        if damage > 0 {
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(player)
            {
                hp.current -= damage;
                events.push(EngineEvent::HpChange {
                    entity: player,
                    amount: -damage,
                    new_hp: hp.current,
                    source: HpSource::Combat,
                });
            }
        }

        return events;
    }

    // Check if it's a potion (throw at player).
    if let Some(ptype) = potion_type {
        events.push(EngineEvent::msg_with(
            "monster-throws-potion",
            vec![("monster", mon_name.clone())],
        ));

        match ptype {
            PotionType::Paralysis => {
                let dur = rng.random_range(1u32..=10);
                events.push(EngineEvent::StatusApplied {
                    entity: player,
                    status: StatusEffect::Paralyzed,
                    duration: Some(dur),
                    source: Some(monster),
                });
            }
            PotionType::Blindness => {
                let dur = rng.random_range(10u32..=50);
                events.push(EngineEvent::StatusApplied {
                    entity: player,
                    status: StatusEffect::Blind,
                    duration: Some(dur),
                    source: Some(monster),
                });
            }
            PotionType::Confusion => {
                let dur = rng.random_range(5u32..=25);
                events.push(EngineEvent::StatusApplied {
                    entity: player,
                    status: StatusEffect::Confused,
                    duration: Some(dur),
                    source: Some(monster),
                });
            }
            PotionType::Acid => {
                let damage: i32 =
                    (0..2).map(|_| rng.random_range(1i32..=6)).sum();
                if let Some(mut hp) =
                    world.get_component_mut::<HitPoints>(player)
                {
                    hp.current -= damage;
                    events.push(EngineEvent::HpChange {
                        entity: player,
                        amount: -damage,
                        new_hp: hp.current,
                        source: HpSource::Combat,
                    });
                }
            }
            _ => {}
        }

        // Consume the potion.
        let _ = world.despawn(item);
        return events;
    }

    events
}

// ---------------------------------------------------------------------------
// Item use: defensive
// ---------------------------------------------------------------------------

/// Monster uses a defensive item (healing, escape).
///
/// Handles potion drinking (healing, extra healing, full healing, speed,
/// invisibility) and wand zapping self (teleportation).
pub fn use_defensive_item(
    world: &mut GameWorld,
    monster: Entity,
    item: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let mon_name = world.entity_name(monster);

    // Copy tag values to release borrows before mutating.
    let potion_type = world.get_component::<PotionTypeTag>(item).map(|t| t.0);
    let wand_type = world.get_component::<WandTypeTag>(item).map(|t| t.0);

    // Check if it's a healing potion.
    if let Some(ptype) = potion_type {
        let (current_hp, max_hp) = world
            .get_component::<HitPoints>(monster)
            .map(|hp| (hp.current, hp.max))
            .unwrap_or((1, 1));

        let heal_amount: i32 = match ptype {
            PotionType::FullHealing => max_hp - current_hp,
            PotionType::ExtraHealing => {
                (0..6).map(|_| rng.random_range(1i32..=8)).sum::<i32>() + 8
            }
            PotionType::Healing => {
                (0..6).map(|_| rng.random_range(1i32..=4)).sum::<i32>() + 8
            }
            PotionType::Speed => {
                events.push(EngineEvent::StatusApplied {
                    entity: monster,
                    status: StatusEffect::FastSpeed,
                    duration: Some(100),
                    source: None,
                });
                events.push(EngineEvent::msg_with(
                    "monster-quaffs-speed",
                    vec![("monster", mon_name.clone())],
                ));
                let _ = world.despawn(item);
                return events;
            }
            PotionType::Invisibility => {
                events.push(EngineEvent::StatusApplied {
                    entity: monster,
                    status: StatusEffect::Invisible,
                    duration: Some(200),
                    source: None,
                });
                events.push(EngineEvent::msg_with(
                    "monster-quaffs-invis",
                    vec![("monster", mon_name.clone())],
                ));
                let _ = world.despawn(item);
                return events;
            }
            _ => 0,
        };

        if heal_amount > 0 {
            if let Some(mut hp) =
                world.get_component_mut::<HitPoints>(monster)
            {
                let old = hp.current;
                hp.current = (hp.current + heal_amount).min(hp.max);
                let actual = hp.current - old;
                if actual > 0 {
                    events.push(EngineEvent::HpChange {
                        entity: monster,
                        amount: actual,
                        new_hp: hp.current,
                        source: HpSource::Potion,
                    });
                }
            }
            events.push(EngineEvent::msg_with(
                "monster-quaffs-healing",
                vec![("monster", mon_name.clone())],
            ));
        }

        let _ = world.despawn(item);
        return events;
    }

    // Check if it's a wand of teleportation (zap self).
    if let Some(wtype) = wand_type {
        if wtype == WandType::Teleportation {
            let has_charges = world
                .get_component::<WandCharges>(item)
                .map(|c| c.spe > 0)
                .unwrap_or(false);
            if has_charges {
                if let Some(mut charges) =
                    world.get_component_mut::<WandCharges>(item)
                {
                    charges.spe -= 1;
                }
                events.push(EngineEvent::msg_with(
                    "monster-zaps-teleport-self",
                    vec![("monster", mon_name.clone())],
                ));
                // Teleport effect: just emit event (actual position change
                // handled by caller or monster_ai).
                let from = world
                    .get_component::<Positioned>(monster)
                    .map(|p| p.0)
                    .unwrap_or(crate::action::Position::new(0, 0));
                events.push(EngineEvent::EntityTeleported {
                    entity: monster,
                    from,
                    to: from, // placeholder — caller adjusts position
                });
            }
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Item use: miscellaneous
// ---------------------------------------------------------------------------

/// Monster uses a miscellaneous item (self-buff potions).
///
/// Handles quaffing speed and invisibility potions, and using unicorn
/// horn to cure status effects.
pub fn use_misc_item(
    world: &mut GameWorld,
    monster: Entity,
    item: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let _ = rng;
    let mon_name = world.entity_name(monster);

    let potion_type = world.get_component::<PotionTypeTag>(item).map(|t| t.0);

    if let Some(ptype) = potion_type {
        match ptype {
            PotionType::Speed => {
                events.push(EngineEvent::StatusApplied {
                    entity: monster,
                    status: StatusEffect::FastSpeed,
                    duration: Some(100),
                    source: None,
                });
                events.push(EngineEvent::msg_with(
                    "monster-quaffs",
                    vec![("monster", mon_name.clone())],
                ));
                let _ = world.despawn(item);
            }
            PotionType::Invisibility => {
                events.push(EngineEvent::StatusApplied {
                    entity: monster,
                    status: StatusEffect::Invisible,
                    duration: Some(200),
                    source: None,
                });
                events.push(EngineEvent::msg_with(
                    "monster-quaffs",
                    vec![("monster", mon_name.clone())],
                ));
                let _ = world.despawn(item);
            }
            _ => {}
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Item use: unicorn horn
// ---------------------------------------------------------------------------

/// Monster uses a unicorn horn to cure status ailments.
///
/// Cures confusion, stunning, and blindness — matching C NetHack's
/// `use_defensive()` MUSE_UNICORN_HORN case.  The horn is not consumed.
pub fn use_unicorn_horn(
    world: &mut GameWorld,
    monster: Entity,
    _item: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let mon_name = world.entity_name(monster);

    // Determine which ailments to cure.  In C, unicorn horn has a
    // random chance per ailment; we cure one random ailment per use.
    let mut ailments = Vec::new();
    if let Some(status) = world.get_component::<StatusEffects>(monster) {
        if status.confusion > 0 {
            ailments.push(StatusEffect::Confused);
        }
        if status.stun > 0 {
            ailments.push(StatusEffect::Stunned);
        }
        if status.blindness > 0 {
            ailments.push(StatusEffect::Blind);
        }
    }

    if ailments.is_empty() {
        return events;
    }

    // Pick one ailment to cure (random, matching C's behavior).
    let idx = rng.random_range(0..ailments.len());
    let cured = ailments[idx];

    // Clear the timer.
    if let Some(mut status) = world.get_component_mut::<StatusEffects>(monster) {
        match cured {
            StatusEffect::Confused => status.confusion = 0,
            StatusEffect::Stunned => status.stun = 0,
            StatusEffect::Blind => status.blindness = 0,
            _ => {}
        }
    }

    events.push(EngineEvent::StatusRemoved {
        entity: monster,
        status: cured,
    });
    events.push(EngineEvent::msg_with(
        "monster-uses-unicorn-horn",
        vec![("monster", mon_name)],
    ));

    events
}

// ---------------------------------------------------------------------------
// Monster weapon wielding: select_hwep / mon_wield equivalent
// ---------------------------------------------------------------------------

/// Scan a monster's inventory for the best weapon to wield.
///
/// Priority: higher damage dice sum is better.  Prefers weapons over
/// non-weapons.  Returns the best weapon entity, if any.
///
/// Mirrors C NetHack's `select_hwep()` from `muse.c`.
pub fn find_best_weapon(
    world: &GameWorld,
    monster: Entity,
) -> Option<Entity> {
    let items = get_monster_inventory(world, monster);

    let mut best: Option<(Entity, i32)> = None;

    for &item in &items {
        let core = match world.get_component::<ObjectCore>(item) {
            Some(c) => c.clone(),
            None => continue,
        };

        // Only consider weapons and weapon-tools.
        if core.object_class != ObjectClass::Weapon && core.object_class != ObjectClass::Tool {
            continue;
        }

        // Use weight as a rough proxy for weapon quality when we don't
        // have full object definitions available.  Heavier weapons tend
        // to do more damage.  A proper implementation would look up
        // oc_wsdam/oc_wldam, but this gives reasonable results.
        let score = core.weight as i32;

        match best {
            Some((_, best_score)) if score <= best_score => {}
            _ => best = Some((item, score)),
        }
    }

    best.map(|(e, _)| e)
}

/// Scan a monster's inventory for armor to wear.
///
/// Returns the best armor piece to wear, preferring body armor with
/// lower (better) AC value.  Uses weight as AC proxy.
///
/// Mirrors C NetHack's `m_dowear()` from `worn.c`.
pub fn find_best_armor(
    world: &GameWorld,
    monster: Entity,
) -> Option<Entity> {
    let items = get_monster_inventory(world, monster);

    let mut best: Option<(Entity, i32)> = None;

    for &item in &items {
        let core = match world.get_component::<ObjectCore>(item) {
            Some(c) => c.clone(),
            None => continue,
        };

        // Only consider armor class items.
        if core.object_class != ObjectClass::Armor {
            continue;
        }

        // Heavier armor generally provides better AC.
        let score = core.weight as i32;

        match best {
            Some((_, best_score)) if score <= best_score => {}
            _ => best = Some((item, score)),
        }
    }

    best.map(|(e, _)| e)
}

/// Monster wields the best available weapon from its inventory.
///
/// Emits an `ItemWielded` event if a weapon is found.  This is a
/// simplified version that just marks the item as wielded — the
/// combat system checks for wielded weapons via EquipmentSlots.
pub fn monster_wield_weapon(
    world: &mut GameWorld,
    monster: Entity,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let weapon = match find_best_weapon(world, monster) {
        Some(w) => w,
        None => return events,
    };

    let mon_name = world.entity_name(monster);

    events.push(EngineEvent::ItemWielded {
        actor: monster,
        item: weapon,
    });
    events.push(EngineEvent::msg_with(
        "monster-wields-weapon",
        vec![("monster", mon_name)],
    ));

    events
}

/// Monster wears the best available armor from its inventory.
///
/// Emits an `ItemWorn` event if armor is found.
pub fn monster_wear_armor(
    world: &mut GameWorld,
    monster: Entity,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let armor = match find_best_armor(world, monster) {
        Some(a) => a,
        None => return events,
    };

    let mon_name = world.entity_name(monster);

    events.push(EngineEvent::ItemWorn {
        actor: monster,
        item: armor,
    });
    events.push(EngineEvent::msg_with(
        "monster-wears-armor",
        vec![("monster", mon_name)],
    ));

    events
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::world::{
        Attributes, ArmorClass, ExperienceLevel, HitPoints, Monster,
        MovementPoints, Name, Positioned, Speed, NORMAL_SPEED,
    };
    use nethack_babel_data::{ObjectClass, ObjectCore, ObjectLocation, ObjectTypeId};
    use rand::rngs::SmallRng;
    use rand::SeedableRng;

    fn test_rng() -> SmallRng {
        SmallRng::seed_from_u64(42)
    }

    fn make_test_world() -> GameWorld {
        let mut world = GameWorld::new(Position::new(8, 8));
        for y in 1..=15 {
            for x in 1..=15 {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(x, y), crate::dungeon::Terrain::Floor);
            }
        }
        world
    }

    fn spawn_monster(
        world: &mut GameWorld,
        pos: Position,
        current_hp: i32,
        max_hp: i32,
    ) -> Entity {
        world.spawn((
            Monster,
            Positioned(pos),
            HitPoints { current: current_hp, max: max_hp },
            ArmorClass(10),
            Attributes::default(),
            ExperienceLevel(1),
            Speed(12),
            MovementPoints(NORMAL_SPEED as i32),
            Name("test monster".to_string()),
        ))
    }

    fn give_potion(
        world: &mut GameWorld,
        monster: Entity,
        ptype: PotionType,
    ) -> Entity {
        let carrier_id = monster.to_bits().get() as u32;
        let core = ObjectCore {
            otyp: ObjectTypeId(100),
            object_class: ObjectClass::Potion,
            quantity: 1,
            weight: 20,
            age: 0,
            inv_letter: None,
            artifact: None,
        };
        let loc = ObjectLocation::MonsterInventory { carrier_id };
        world.spawn((core, loc, PotionTypeTag(ptype)))
    }

    fn give_wand(
        world: &mut GameWorld,
        monster: Entity,
        wtype: WandType,
        charges: i8,
    ) -> Entity {
        let carrier_id = monster.to_bits().get() as u32;
        let core = ObjectCore {
            otyp: ObjectTypeId(200),
            object_class: ObjectClass::Tool,
            quantity: 1,
            weight: 7,
            age: 0,
            inv_letter: None,
            artifact: None,
        };
        let loc = ObjectLocation::MonsterInventory { carrier_id };
        let wand_charges = WandCharges { spe: charges, recharged: 0 };
        world.spawn((core, loc, WandTypeTag(wtype), wand_charges))
    }

    // ── find_offensive_item ──────────────────────────────────────

    #[test]
    fn find_offensive_prefers_death_wand() {
        let mut world = make_test_world();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);

        let fire_wand = give_wand(&mut world, monster, WandType::Fire, 3);
        let death_wand = give_wand(&mut world, monster, WandType::Death, 1);

        let found = find_offensive_item(&world, monster);
        assert_eq!(found, Some(death_wand));
        let _ = fire_wand; // suppress warning
    }

    #[test]
    fn find_offensive_falls_back_to_potion() {
        let mut world = make_test_world();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);

        // No wands with charges.
        let _empty_wand = give_wand(&mut world, monster, WandType::Fire, 0);
        let potion = give_potion(&mut world, monster, PotionType::Paralysis);

        let found = find_offensive_item(&world, monster);
        assert_eq!(found, Some(potion));
    }

    #[test]
    fn find_offensive_nothing_available() {
        let mut world = make_test_world();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);

        assert_eq!(find_offensive_item(&world, monster), None);
    }

    // ── find_defensive_item ──────────────────────────────────────

    #[test]
    fn find_defensive_prefers_full_healing() {
        let mut world = make_test_world();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 5, 30);

        let _healing = give_potion(&mut world, monster, PotionType::Healing);
        let full = give_potion(&mut world, monster, PotionType::FullHealing);

        let found = find_defensive_item(&world, monster);
        assert_eq!(found, Some(full));
    }

    #[test]
    fn find_defensive_falls_back_to_teleport_wand() {
        let mut world = make_test_world();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 5, 30);

        let wand = give_wand(&mut world, monster, WandType::Teleportation, 2);

        let found = find_defensive_item(&world, monster);
        assert_eq!(found, Some(wand));
    }

    // ── find_misc_item ───────────────────────────────────────────

    #[test]
    fn find_misc_finds_speed_potion() {
        let mut world = make_test_world();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);

        let speed = give_potion(&mut world, monster, PotionType::Speed);

        let found = find_misc_item(&world, monster);
        assert_eq!(found, Some(speed));
    }

    // ── use_offensive_item ───────────────────────────────────────

    #[test]
    fn use_offensive_wand_damages_player() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);
        let wand = give_wand(&mut world, monster, WandType::Fire, 3);

        let player = world.player();
        let orig_hp = world.get_component::<HitPoints>(player).unwrap().current;

        let events = use_offensive_item(&mut world, monster, wand, &mut rng);

        let new_hp = world.get_component::<HitPoints>(player).unwrap().current;
        assert!(new_hp < orig_hp, "fire wand should damage player");
        assert!(events.iter().any(|e| matches!(e, EngineEvent::HpChange { .. })));
    }

    #[test]
    fn use_offensive_wand_sleep_applies_status() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);
        let wand = give_wand(&mut world, monster, WandType::Sleep, 3);

        let events = use_offensive_item(&mut world, monster, wand, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied { status: StatusEffect::Sleeping, .. }
        )));
    }

    #[test]
    fn use_offensive_potion_paralysis() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);
        let potion = give_potion(&mut world, monster, PotionType::Paralysis);

        let events = use_offensive_item(&mut world, monster, potion, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied { status: StatusEffect::Paralyzed, .. }
        )));
        // Potion should be consumed.
        assert!(world.get_component::<ObjectCore>(potion).is_none());
    }

    #[test]
    fn use_offensive_potion_acid_damages() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);
        let potion = give_potion(&mut world, monster, PotionType::Acid);

        let player = world.player();
        let orig_hp = world.get_component::<HitPoints>(player).unwrap().current;

        use_offensive_item(&mut world, monster, potion, &mut rng);

        let new_hp = world.get_component::<HitPoints>(player).unwrap().current;
        assert!(new_hp < orig_hp, "acid should damage player");
    }

    // ── use_defensive_item ───────────────────────────────────────

    #[test]
    fn use_defensive_healing_restores_hp() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 5, 30);
        let potion = give_potion(&mut world, monster, PotionType::Healing);

        let events = use_defensive_item(&mut world, monster, potion, &mut rng);

        let hp = world.get_component::<HitPoints>(monster).unwrap();
        assert!(hp.current > 5, "healing should restore HP");
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::HpChange { amount, .. } if *amount > 0
        )));
    }

    #[test]
    fn use_defensive_full_healing_maxes_hp() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 5, 30);
        let potion = give_potion(&mut world, monster, PotionType::FullHealing);

        use_defensive_item(&mut world, monster, potion, &mut rng);

        let hp = world.get_component::<HitPoints>(monster).unwrap();
        assert_eq!(hp.current, 30, "full healing should restore to max");
    }

    #[test]
    fn use_defensive_speed_grants_fast() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);
        let potion = give_potion(&mut world, monster, PotionType::Speed);

        let events = use_defensive_item(&mut world, monster, potion, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                entity,
                status: StatusEffect::FastSpeed,
                ..
            } if *entity == monster
        )));
    }

    // ── use_misc_item ────────────────────────────────────────────

    #[test]
    fn use_misc_speed_potion() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);
        let potion = give_potion(&mut world, monster, PotionType::Speed);

        let events = use_misc_item(&mut world, monster, potion, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied { status: StatusEffect::FastSpeed, .. }
        )));
        // Consumed.
        assert!(world.get_component::<ObjectCore>(potion).is_none());
    }

    #[test]
    fn use_misc_invisibility_potion() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);
        let potion = give_potion(&mut world, monster, PotionType::Invisibility);

        let events = use_misc_item(&mut world, monster, potion, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied { status: StatusEffect::Invisible, .. }
        )));
    }

    #[test]
    fn use_offensive_empty_wand_does_nothing() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);
        let wand = give_wand(&mut world, monster, WandType::Fire, 0);

        let events = use_offensive_item(&mut world, monster, wand, &mut rng);
        assert!(events.is_empty(), "empty wand should do nothing");
    }

    // ── find_best_weapon ─────────────────────────────────────────

    fn give_weapon(
        world: &mut GameWorld,
        monster: Entity,
        weight: u32,
    ) -> Entity {
        let carrier_id = monster.to_bits().get() as u32;
        let core = ObjectCore {
            otyp: ObjectTypeId(300),
            object_class: ObjectClass::Weapon,
            quantity: 1,
            weight,
            age: 0,
            inv_letter: None,
            artifact: None,
        };
        let loc = ObjectLocation::MonsterInventory { carrier_id };
        world.spawn((core, loc))
    }

    fn give_armor(
        world: &mut GameWorld,
        monster: Entity,
        weight: u32,
    ) -> Entity {
        let carrier_id = monster.to_bits().get() as u32;
        let core = ObjectCore {
            otyp: ObjectTypeId(400),
            object_class: ObjectClass::Armor,
            quantity: 1,
            weight,
            age: 0,
            inv_letter: None,
            artifact: None,
        };
        let loc = ObjectLocation::MonsterInventory { carrier_id };
        world.spawn((core, loc))
    }

    #[test]
    fn find_best_weapon_prefers_heavier() {
        let mut world = make_test_world();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);

        let _light = give_weapon(&mut world, monster, 10);
        let heavy = give_weapon(&mut world, monster, 40);

        let found = find_best_weapon(&world, monster);
        assert_eq!(found, Some(heavy));
    }

    #[test]
    fn find_best_weapon_no_weapons_returns_none() {
        let mut world = make_test_world();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);

        // Give a potion, not a weapon.
        let _potion = give_potion(&mut world, monster, PotionType::Healing);

        assert_eq!(find_best_weapon(&world, monster), None);
    }

    #[test]
    fn find_best_armor_finds_heaviest() {
        let mut world = make_test_world();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);

        let _light = give_armor(&mut world, monster, 50);
        let heavy = give_armor(&mut world, monster, 150);

        let found = find_best_armor(&world, monster);
        assert_eq!(found, Some(heavy));
    }

    #[test]
    fn monster_wield_weapon_emits_event() {
        let mut world = make_test_world();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);

        let weapon = give_weapon(&mut world, monster, 30);

        let events = monster_wield_weapon(&mut world, monster);

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::ItemWielded { actor, item }
                if *actor == monster && *item == weapon
        )), "should emit ItemWielded event");
    }

    #[test]
    fn monster_wear_armor_emits_event() {
        let mut world = make_test_world();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);

        let armor = give_armor(&mut world, monster, 100);

        let events = monster_wear_armor(&mut world, monster);

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::ItemWorn { actor, item }
                if *actor == monster && *item == armor
        )), "should emit ItemWorn event");
    }

    #[test]
    fn monster_wield_no_weapon_returns_empty() {
        let mut world = make_test_world();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);

        let events = monster_wield_weapon(&mut world, monster);
        assert!(events.is_empty(), "no weapon → no events");
    }

    // ── monster_item_decision ────────────────────────────────────

    fn give_unicorn_horn(world: &mut GameWorld, monster: Entity) -> Entity {
        let carrier_id = monster.to_bits().get() as u32;
        let core = ObjectCore {
            otyp: ObjectTypeId(500),
            object_class: ObjectClass::Tool,
            quantity: 1,
            weight: 20,
            age: 0,
            inv_letter: None,
            artifact: None,
        };
        let loc = ObjectLocation::MonsterInventory { carrier_id };
        world.spawn((core, loc, UnicornHornTag))
    }

    #[test]
    fn decision_low_hp_chooses_healing() {
        let mut world = make_test_world();
        // Monster at 5/30 HP — below 50% threshold.
        let monster = spawn_monster(&mut world, Position::new(12, 8), 5, 30);
        let _potion = give_potion(&mut world, monster, PotionType::FullHealing);

        let action = monster_item_decision(&world, monster, false, false);
        assert!(
            matches!(action, MonsterItemAction::QuaffHealingPotion { .. }),
            "low HP should choose healing, got {:?}",
            action
        );
    }

    #[test]
    fn decision_fleeing_low_hp_chooses_escape() {
        let mut world = make_test_world();
        // Monster at 3/30 HP (10%) — below 33% threshold and fleeing.
        let monster = spawn_monster(&mut world, Position::new(12, 8), 3, 30);
        let _wand = give_wand(&mut world, monster, WandType::Teleportation, 2);

        let action = monster_item_decision(&world, monster, true, false);
        assert!(
            matches!(action, MonsterItemAction::ZapTeleportWand { .. }),
            "fleeing at low HP should choose escape, got {:?}",
            action
        );
    }

    #[test]
    fn decision_with_target_chooses_attack_wand() {
        let mut world = make_test_world();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);
        let _wand = give_wand(&mut world, monster, WandType::Death, 1);

        let action = monster_item_decision(&world, monster, false, true);
        assert!(
            matches!(
                action,
                MonsterItemAction::ZapAttackWand {
                    wand_type: WandType::Death,
                    ..
                }
            ),
            "with target should choose attack wand, got {:?}",
            action
        );
    }

    #[test]
    fn decision_attack_wand_prioritization() {
        let mut world = make_test_world();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);
        // Give fire wand first, then death wand — death should be preferred.
        let _fire = give_wand(&mut world, monster, WandType::Fire, 3);
        let _death = give_wand(&mut world, monster, WandType::Death, 1);

        let action = monster_item_decision(&world, monster, false, true);
        assert!(
            matches!(
                action,
                MonsterItemAction::ZapAttackWand {
                    wand_type: WandType::Death,
                    ..
                }
            ),
            "death wand should be preferred over fire, got {:?}",
            action
        );
    }

    #[test]
    fn decision_confused_monster_uses_unicorn_horn() {
        let mut world = make_test_world();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);

        // Add StatusEffects with confusion.
        let _ = world
            .ecs_mut()
            .insert_one(
                monster,
                StatusEffects {
                    confusion: 10,
                    ..StatusEffects::default()
                },
            );
        let _horn = give_unicorn_horn(&mut world, monster);

        let action = monster_item_decision(&world, monster, false, false);
        assert!(
            matches!(action, MonsterItemAction::UseUnicornHorn { .. }),
            "confused monster with horn should use it, got {:?}",
            action
        );
    }

    #[test]
    fn decision_no_items_returns_no_action() {
        let mut world = make_test_world();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);

        let action = monster_item_decision(&world, monster, false, false);
        assert_eq!(action, MonsterItemAction::NoAction);
    }

    #[test]
    fn decision_healing_priority_full_over_regular() {
        let mut world = make_test_world();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 5, 30);
        // Give regular healing first, then full healing.
        let _regular = give_potion(&mut world, monster, PotionType::Healing);
        let full = give_potion(&mut world, monster, PotionType::FullHealing);

        let action = monster_item_decision(&world, monster, false, false);
        match action {
            MonsterItemAction::QuaffHealingPotion { item } => {
                assert_eq!(item, full, "should prefer full healing over regular");
            }
            other => panic!("expected QuaffHealingPotion, got {:?}", other),
        }
    }

    // ── use_unicorn_horn ─────────────────────────────────────────

    #[test]
    fn use_unicorn_horn_cures_confusion() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);
        let _ = world.ecs_mut().insert_one(
            monster,
            StatusEffects {
                confusion: 10,
                ..StatusEffects::default()
            },
        );
        let horn = give_unicorn_horn(&mut world, monster);

        let events = use_unicorn_horn(&mut world, monster, horn, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved {
                status: StatusEffect::Confused,
                ..
            }
        )));
        let status = world.get_component::<StatusEffects>(monster).unwrap();
        assert_eq!(status.confusion, 0, "confusion should be cured");
    }

    #[test]
    fn use_unicorn_horn_no_ailment_does_nothing() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let monster = spawn_monster(&mut world, Position::new(12, 8), 20, 20);
        let _ = world
            .ecs_mut()
            .insert_one(monster, StatusEffects::default());
        let horn = give_unicorn_horn(&mut world, monster);

        let events = use_unicorn_horn(&mut world, monster, horn, &mut rng);
        assert!(events.is_empty(), "no ailment → no events");
    }

    // ── should_use_item_now ──────────────────────────────────────

    #[test]
    fn should_use_item_threshold_checks() {
        // No threat: only at critical HP.
        assert!(!should_use_item_now(0.3, 0));
        assert!(should_use_item_now(0.1, 0));

        // High threat: willing at higher HP.
        assert!(should_use_item_now(0.5, 5));
        assert!(!should_use_item_now(0.8, 5));
    }
}
